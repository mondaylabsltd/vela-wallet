package app.getvela.wallet

import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.multisig.MultisigWiring
import app.getvela.wallet.feature.multisig.MultisigNotifier
import app.getvela.wallet.feature.multisig.MultisigWords
import app.getvela.wallet.feature.multisig.core.MultisigHub
import app.getvela.wallet.core.format.Formats
import kotlinx.coroutines.flow.MutableStateFlow
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.flows.WalletFlowEntry
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.core.diagnostics.CrashReport
import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.feature.signing.SigningAftercare
import android.app.Application
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.feature.agent.AgentStore
import app.getvela.wallet.feature.agent.AgentWallet
import app.getvela.wallet.feature.agent.KeystoreTokenVault
import app.getvela.wallet.core.data.ThemePreferenceRepository
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.net.NetHealth
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.send.core.SendReceiptOutcomes
import app.getvela.wallet.feature.wallet.core.TrackerWorker
import app.getvela.wallet.feature.wallet.core.TrackerNotifier
import app.getvela.wallet.feature.send.core.TrustedSignerLabels
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.SigningPage
import kotlinx.serialization.builtins.ListSerializer
import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import app.getvela.wallet.feature.signing.trustedsigner.SigningPlan
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerChannel
import app.getvela.wallet.feature.signing.trustedsigner.TrustedSignerTab
import app.getvela.wallet.feature.send.core.SendHapticKind
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.browser.core.BrowserController
import app.getvela.wallet.feature.contacts.core.ContactIdentity
import app.getvela.wallet.feature.contacts.core.IdentityResolver
import app.getvela.wallet.feature.contacts.core.RegistryNameLookup
import app.getvela.wallet.feature.send.core.SendRecipientIdentity
import org.json.JSONObject
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.RegistryBackup
import app.getvela.wallet.feature.settings.core.WalletKeys
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.PoolRelayPort
import app.getvela.wallet.dev.ParallelSpaceHook
import app.getvela.wallet.dev.ParallelSpaceBinding
import app.getvela.wallet.core.i18n.AppLanguage
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.LocaleResolver
import app.getvela.wallet.feature.contacts.core.ContactsController
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.settings.core.SettingsController
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetworkSpacePreference
import app.getvela.wallet.feature.settings.core.NetworkSpaces
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.stateIn
import app.getvela.wallet.core.data.DebugSeed
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.feature.settings.core.DeviceStorage
import app.getvela.wallet.core.platform.Haptics
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.NetworkEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.core.net.VelaHttp
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.WalletController
import java.util.Locale
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
import app.getvela.wallet.feature.send.core.PaymentHandOff
import app.getvela.wallet.feature.settings.core.FeedbackReporter
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.SupervisorJob

/**
 * Manual composition root (research D8) — no DI framework at this scale.
 * Engine construction is blocking file+FFI work, so it runs on a dedicated
 * single-thread executor; locale updates queue behind it, which makes the
 * ready-before-setLocale ordering structural.
 */
class AppContainer(private val app: Application) {

    val i18nRuntime = I18nRuntime(
        onLanguage = { AppLanguage.tag = it },
        // P2.6: an on-demand namespace (`assets/i18n/<name>/<tag>.json`), read
        // only when a screen that draws it asks (I18nRuntime.include).
        readOnDemand = { namespace, tag ->
            runCatching {
                app.assets.open(I18nCatalogs.assetPath(tag, namespace)).use { it.readBytes() }
            }.getOrNull()
        },
    ) { tag ->
        app.assets.open("i18n/$tag.json").use { it.readBytes() }
    }

    val themeRepository = ThemePreferenceRepository(
        app,
        VelaStore(app),
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
    )

    /** Spec 047 D9: online or not, from the platform. */

    /** Spec 047 D8: a `/pay` link handed to the wallet route, consumed once. */
    val pendingPayLink = MutableStateFlow<android.net.Uri?>(null)

    /** A validated `/pay` request waiting for the send flow to open (locked), consumed once. */
    val pendingSendParams = MutableStateFlow<SendOpenParams?>(null)
    /** Spec 048: a wallet flow another route asked for (contacts: 转账 / 收款 / 群发转账), entered once the wallet is back on top. */
    val pendingFlow = MutableStateFlow<WalletFlowEntry?>(null)
    /** Phase 3: the flow a launcher shortcut or the Scan tile asked for — entered once a wallet is in front. */
    val pendingLaunch = MutableStateFlow<app.getvela.wallet.core.platform.LaunchEntry.Flow?>(null)
    /** Phase 3: an address or `ethereum:` request shared into Vela — Send's `scan_resolved` reads it once a wallet is in front. */
    val pendingShare = MutableStateFlow<String?>(null)
    /** Spec 048: split rows to seed into the send machine right after it opens (a group's 群发转账). */
    val pendingSplitSeed = MutableStateFlow<List<SendRecipientDraft>?>(null)

    /**
     * A code scanned from 探索 that is a payment (an address, an `ethereum:`
     * request — spec 070): the send opens on it as if its own scanner had read
     * it, so the core's `scan_resolved` decides what it means.
     */
    val pendingScan = MutableStateFlow<String?>(null)

    /**
     * Spec 078 round 3: a payment request from outside Send (a `/pay` link, a
     * code scanned in 探索) reaches Send once — now, if Send is open; parked
     * for the open otherwise.
     */
    val paymentHandOff = PaymentHandOff(pendingSendParams, pendingScan)
    /**
     * Spec 078 round 3: the report in flight, on the app's scope — closing the
     * sheet (or leaving the tab) mid-send loses neither the send nor its answer.
     */
    val feedback = FeedbackReporter(
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default),
    )
    /** Spec 048: the add-token 原生币 tab opens the settings' add-network page. */
    val pendingSettingsPage = MutableStateFlow<SettingsPage?>(null)

    /** Spec 047 D1: the preferences that have no machine — the web's keys, applied app-wide. */
    val preferences = Preferences(
        store = VelaStore(app),
        scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
        locale = { app.resources.configuration.locales[0] ?: java.util.Locale.getDefault() },
    ).also { it.load() }

    /**
     * Spec 105: the device's space — mainnet or testnet, one switch for every
     * wallet. Read at launch, so the home's label and the testnet bar are right
     * from the first frame.
     */
    val networkSpace = NetworkSpacePreference(
        store = VelaStore(app),
        scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
    ).also { it.load() }

    /**
     * Spec 105: the networks of the space in front — the network machine's
     * view with only that space's rows (the core's `network_space_chain_ids`).
     * Every list a person sees reads this: the balances and the total, assets,
     * receive, send's chains and tokens, adding a token, the chains a dApp and
     * the signing sheet may use. What must know every chain — the pool's
     * endpoints, a chain's name or coin, Settings' own pages — reads
     * [SettingsController.networks].
     */
    val spaceNetworks: StateFlow<NetView> by lazy {
        combine(settings.networks, networkSpace.space) { view, space -> NetworkSpaces.view(space, view) }
            .stateIn(
                CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
                SharingStarted.Eagerly,
                NetworkSpaces.view(networkSpace.space.value, settings.networks.value),
            )
    }

    val accountStore = AccountStore(app)

    /**
     * This app's own mark, as the small inline PNG the signer page accepts.
     *
     * Rendered once and kept: it cannot change while the app is installed, and
     * a handshake travels in the clear in 244-byte frames, so the size matters
     * more than the fidelity — 64 px is what the page draws.
     */
    private val appMarkOnce: String by lazy {
        runCatching {
            val size = 64
            val drawable = app.packageManager.getApplicationIcon(app.packageName)
            val bitmap = android.graphics.Bitmap.createBitmap(
                size, size, android.graphics.Bitmap.Config.ARGB_8888,
            )
            val canvas = android.graphics.Canvas(bitmap)
            drawable.setBounds(0, 0, size, size)
            drawable.draw(canvas)
            val bytes = java.io.ByteArrayOutputStream()
            bitmap.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, bytes)
            bitmap.recycle()
            val encoded = android.util.Base64.encodeToString(
                bytes.toByteArray(), android.util.Base64.NO_WRAP,
            )
            val uri = "data:image/png;base64,\u0024encoded"
            // The core refuses anything over its cap anyway; not building a
            // useless string is cheaper than having it dropped there.
            if (uri.length <= 6144) uri else ""
        }.getOrDefault("")
    }

    fun appMark(): String = appMarkOnce

    /**
     * The session machine lives HERE, not in a ViewModel.
     *
     * It is the route guard for the whole app and it outlives every screen. A
     * guard rebuilt whenever a screen is rebuilt would spend the first frame
     * after each rotation reporting `loading` and bouncing a signed-in person
     * back to onboarding.
     *
     * `SupervisorJob` so one failed effect cannot take the session down with it:
     * every operation already answers with its own failure variant, and a scope
     * that cancelled on the first exception would leave the app permanently in
     * `loading`.
     */
    val session = SessionController(
        store = accountStore,
        scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
    )

    /**
     * The settings machines, built on first use and never torn down.
     *
     * Lazy for a reason the session machine does not share: the route guard is
     * needed before the first frame, while a person who never opens Settings
     * should not pay for its storage read. Never torn down for the reason the
     * session machine *does* share: a machine rebuilt whenever a screen is
     * rebuilt spends the frame after every rotation reporting its placeholder
     * over a settled choice.
     */
    val settings: SettingsController by lazy {
        SettingsController(
            context = app,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            pool = pool,
            // "Clear caches" forgets what the relay client learned too —
            // deployment, nonces, resolved base URLs (spec 043 T049).
            clearBundlerCache = { relay.clearCaches() },
        )
    }

    /**
     * The registered name behind an ADDRESS (issue 191). The v2 index cannot be
     * asked that directly, so the core walks chain → founding key → units and
     * this only carries the requests. One instance: its cache is one document.
     */
    val registryNames: RegistryNameLookup by lazy {
        val client = RegistryClient()
        RegistryNameLookup(
            store = VelaStore(app),
            // The RAW result, bare `0x` included: a chain without the signer
            // contract is an answer ("not here"), not a silence.
            ethCall = ::rawEthCall,
            indexGet = { path ->
                // Spec 081 FR-002: the configured index, per lookup. This
                // client was built once with the default and never told
                // otherwise, so Settings → Service Endpoints could name a
                // self-hosted index and every name still came from ours.
                client.baseUrl = accountStore.registryUrlOrDefault()
                val got = client.rawGet(path)
                when {
                    got == null -> RegistryNameLookup.IndexAnswer.Failed
                    got.first == 404 -> RegistryNameLookup.IndexAnswer.NotFound
                    got.first in 200..299 -> RegistryNameLookup.IndexAnswer.Ok(got.second)
                    else -> RegistryNameLookup.IndexAnswer.Failed
                }
            },
            step = { address, answers -> uniffi.vela_core_uniffi.registryNameStep(address, answers) },
        )
    }

    /**
     * A name for an address (spec 043 T048): own accounts → cache → the
     * passkey index → the name services. Contacts and send ask the same one.
     */
    /**
     * [identity]'s waterfall on a background dispatcher. Its callers (the
     * contacts and send machines' executors) run on the main thread; the
     * lookup's namehash and verdict steps are core calls, and measured at
     * launch they were a tenth of the main thread's time (Phase 4).
     */
    private suspend fun resolveIdentity(address: String): IdentityResolver.Identity? =
        kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.Default) { identity.resolve(address) }

    val identity: IdentityResolver by lazy {
        IdentityResolver(
            store = VelaStore(app),
            ownAccounts = { session.view.value.accounts.map { it.address to (memberLabel(it.address) ?: it.name) } },
            registryName = { address -> registryNames.nameFor(address) },
            ethCall = { chainId, to, data ->
                (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", to).put("data", data), "latest")) as? RpcResult.Body)
                    ?.json?.optString("result")?.takeIf { it.startsWith("0x") && it != "0x" }
            },
            // The wallet's networks — the list Settings → Networks shows, the
            // one the pool seeds its endpoints from, waited for the same way
            // (a lookup made before the rows exist would otherwise ask nobody).
            walletChains = {
                kotlinx.coroutines.withTimeoutOrNull(15_000) {
                    settings.networks.first { it.loaded && it.networks.isNotEmpty() }
                }
                settings.networks.value.networks.map { it.chain_id.toInt() }.toSet()
            },
        )
    }

    /**
     * The one way this app reads a chain.
     *
     * App-resident, and app-resident is not a style choice: the ban map and the
     * endpoint statistics are facts about the network that every read-path
     * machine shares. Two pools would mean two opinions about a dead endpoint,
     * and the second one would keep asking.
     *
     * Its endpoint list comes from the settings machine — which in turn needs
     * this pool to price a currency. The source reads that list lazily, which
     * is what lets both be built without either waiting for the other.
     *
     * `Dispatchers.IO`: the transport blocks, and confines itself, but the
     * driver's own JSON work has no business on the main thread either.
     */
    val pool: RpcPool by lazy {
        RpcPool(
            store = VelaStore(app),
            endpoints = NetworkEndpointSource(
                networks = { settings.networks.value },
                // A chain asked for before the rows exist would be configured
                // empty for the whole process (spec 043 phase 4).
                ready = {
                    kotlinx.coroutines.withTimeoutOrNull(15_000) {
                        settings.networks.first { it.loaded && it.networks.isNotEmpty() }
                    }
                },
            ),
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
        )
    }

    /**
     * The wallet home's machines — the RPC pool and the balance dashboard — on
     * the same terms as [settings].
     *
     * The pool lives here rather than on a screen because its ban map and
     * endpoint statistics are facts about the network that every read-path
     * machine shares. Two pools would mean two opinions about a dead endpoint.
     *
     * It reads its endpoints from the settings machine's own view, so there is
     * exactly one reader of what networks this person has.
     */
    private val walletOnce = lazy {
        WalletController(
            context = app,
            networks = spaceNetworks,
            pool = pool,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
            // A payment from one of this person's OWN wallets should say so by
            // name, and that costs no network call at all.
            ownAccounts = {
                session.view.value.accounts.map {
                    // r9-12 (core §9.1): Activity names an own account as everything does.
                    FeedExecutor.FeedOwnAccount(address = it.address, name = memberLabel(it.address) ?: it.name)
                }
            },
            haptic = { Haptics.moneyIn(app) },
            foreground = { foregroundActivities > 0 },
            // Spec 043: the tracker follows a submitted send to its verdict.
            relay = relay,
            notifyConfirmed = { hash, chain, tx ->
                TrackerNotifier.notifyConfirmed(
                    app, hash, chain, tx,
                    title = i18nRuntime.t(I18nKeys.Flows.TX_CONFIRMED_NOTICE),
                    body = i18nRuntime.t(I18nKeys.Flows.TX_CONFIRMED_NOTICE_BODY),
                )
            },
            backgroundPoll = { TrackerWorker.enqueue(app) },
            testnet = { networkSpace.space.value.testnet },
            // r9-17 (core §9.4): a multisig lives on its record's networks — its
            // first receipt on one where it holds nothing yet is still seen.
            accountNetworks = { address ->
                if (multisigBuilt) multisig.accountsView.value.row(address)?.account?.chainIds.orEmpty() else emptyList()
            },
            // r17-15: the first holdings round is noted — read the network record
            // again now, so the core settles the shown networks this session
            // (every list was every Vela network until the next launch).
            onHoldingsFirstRead = { CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch { settings.startNetworks() } },
        ).also { controller ->
            // r9-17: back online — what failed offline can be read now (the core decides).
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
                var was = true
                NetHealth.online.collect { online ->
                    if (online && !was) controller.online()
                    was = online
                }
            }
        }
    }
    val wallet: WalletController by walletOnce

    /** The wallet controller when it exists — never built for a touch. */
    fun walletIfBuilt(): WalletController? = if (walletOnce.isInitialized()) wallet else null

    /** The address book, on the same terms as [settings]. */
    /**
     * Spec 043: the relay as the send path talks to it — through the pool,
     * so the relay's endpoints are banned and cooled down like a chain's.
     * App-resident for its two small caches.
     */
    val relay: RelayClient by lazy {
        RelayClient(
            port = PoolRelayPort(pool),
            // The pool names the relay for every chain the settings machine
            // knows; this is the built-in service for one it does not.
            builtinBase = { DEFAULT_BUNDLER_SERVICE_URL },
        )
    }

    /**
     * The passkey signer, once an activity has attached the ceremony
     * (`OnboardingViewModel.attach`). Inside the parallel space the fixture
     * signer takes precedence; outside it, this is the only signer.
     */
    @Volatile
    var passkeySigner: UserOpSigner? = null

    /** Spec 071: the Trusted Signer's tab, attached by the activity in onCreate. */
    @Volatile
    var trustedSignerTab: TrustedSignerTab? = null

    /**
     * Spec 102 R6: the phone's integrity checks — every signing page is
     * fetched, hashed and admitted by the core before it can open. Shared by
     * the channel (which opens only an admitted page), the hand-off card and
     * Settings → Signing pages (which draw each page's line).
     */
    val signerPages: SignerPageChecks by lazy {
        val store = VelaStore(app)
        SignerPageChecks(
            fetch = SignerPageChecks::httpGet,
            store = store,
            // D-15: each page's own trusted versions — the pages core's list
            // as it stands once read, else the stored list itself.
            savedPages = {
                settings.signingPages.value.takeIf { it.loaded }?.saved
                    ?.let { saved ->
                        Wire.json.encodeToString(ListSerializer(SigningPage.serializer()), saved)
                    }
                    ?: store.read(KeyValueStore.Keys.SIGNING_PAGES)
            },
            recordTrust = { url, version -> settings.trustSigningPageVersion(url, version) },
        )
    }

    private val signerRefreshScope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO)

    /**
     * Spec 102 D-14: re-check the signing pages in use whose check the core
     * says is due (`signerPageRefreshDue` — older than half a day, resting
     * after an attempt that could not complete). In use: every saved page and
     * each account's page venue (its `signing_plan`). The activity calls this
     * on every resume — the first one is the start — and hourly while it
     * runs (`signerPageRefreshSchedule().pollMs`). A page not due costs a
     * comparison.
     */
    fun refreshSignerPages() {
        signerRefreshScope.launch {
            runCatching {
                if (!settings.signingPages.value.loaded) {
                    settings.refreshSigningPages()
                    kotlinx.coroutines.withTimeoutOrNull(5_000L) { settings.signingPages.first { it.loaded } }
                }
                val saved = settings.signingPages.value.saved.map { it.url }
                val port = StoreAccountPort(AccountStore(app))
                val venues = session.view.value.accounts.mapNotNull { row ->
                    SigningPlan.of(port.accountJson(row.address))?.page
                }
                signerPages.refresh(saved + venues)
            }.onFailure { VelaLog.failure("signer page", "background refresh", it) }
        }
    }

    /**
     * Spec 071/102: the signing page — one channel per process, one ceremony
     * at a time. Which page is always the caller's (the account's venue, or
     * a ceremony's op); the words are the corpus'.
     */
    val trustedSigner: TrustedSignerChannel by lazy {
        TrustedSignerChannel(
            checks = signerPages,
            openPage = { url -> trustedSignerTab?.open(url) ?: false },
            bringBack = { trustedSignerTab?.bringBack() },
            words = {
                TrustedSignerChannel.Words(
                    closed = i18nRuntime.t("componentsUi.signing.trustedSignerClosed"),
                    refused = i18nRuntime.t("componentsUi.signing.trustedSignerRefused"),
                    mismatch = i18nRuntime.t("componentsUi.signing.trustedSignerMismatch"),
                    timeout = i18nRuntime.t("componentsUi.signing.trustedSignerTimeout"),
                    unchecked = { line -> SignerPageChecks.words(line, i18nRuntime) },
                )
            },
            // Read by a person on the signer page, beside "the name and mark
            // it gave for itself" — so both are written for a person, not for
            // a log. Neither is proof of anything and the page says so; what
            // they buy is telling THIS wallet apart from another one that
            // connected to the same page.
            appName = "Vela Wallet " + BuildConfig.VERSION_NAME,
            appIcon = { appMark() },
            // Spec 079: the page's address alone — the request rides in the
            // fragment and never goes to a server, and neither does it here.
            reachable = { url: String ->
                kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                    runCatching {
                        val probe = url.substringBefore('#').substringBefore('?')
                        VelaHttp.client.newBuilder()
                            .callTimeout(5, java.util.concurrent.TimeUnit.SECONDS)
                            .build()
                            .newCall(okhttp3.Request.Builder().url(probe).head().build())
                            .execute()
                            .use { true }
                    }.getOrDefault(false)
                }
            },
            // Spec 102: the page speaks the app's language, not the browser's.
            lang = { i18nRuntime.state.value.language },
            labels = { chainId, account ->
                val network = settings.networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }
                TrustedSignerLabels(
                    chainName = network?.display_name,
                    nativeSymbol = network?.native_symbol,
                    accountName = session.view.value.accounts
                        .firstOrNull { it.address.equals(account, ignoreCase = true) }?.name,
                )
            },
        )
    }

    /**
     * Native multisig (M1–M6): the core's multisig machines over one shell —
     * the same core the web drives, over uniffi. Built on first use; booted
     * once a wallet is signed in (the wallet route), so a link that opened the
     * app, the switcher and the notifications find it loaded.
     */
    val multisig: MultisigHub by lazy { MultisigWiring.build(this, app).also { MultisigWiring.attachNotifier(this, app, it); multisigBuilt = true } }

    @Volatile
    private var multisigBuilt = false

    /** A page's request, as the proposals machine holds it: one id across tabs. */
    private fun multisigDappId(tab: String, id: String) = "$tab:$id"

    /** M6: what changed while the app was away, said (local polling; no push service). */
    val multisigNotifier: MultisigNotifier by lazy {
        MultisigNotifier(
            store = VelaStore(app),
            words = { MultisigWiring.words(this) },
            post = { note -> MultisigNotifier.postToSystem(app, MultisigWiring.words(this), note) },
        )
    }

    /**
     * r9-12 (core §9.1): the one name of [address] — the accounts machine's
     * `member_labels` (the published name, a multisig's kept label, the
     * local name). `null` before the multisig layer is built, or with no label.
     */
    fun memberLabel(address: String): String? =
        if (multisigBuilt) multisig.accountsView.value.memberLabels[address.lowercase()]?.takeIf { it.isNotBlank() } else null

    /** An activity of this app is on screen (a notification is noise then: the screens say it). */
    fun inForeground(): Boolean = foregroundActivities > 0

    /**
     * r11-1: an activity of this app is RESUMED — in front of the person, not
     * merely started. What the views hold now is what the person can see;
     * Home pauses at once, the stop comes later, and a read landing in
     * between was recorded "seen" by a screen nobody was looking at.
     */
    fun onScreen(): Boolean = resumedActivities > 0

    /**
     * A multisig link (`#ms=…` / `#msa=…`, the fragment with its `#`) that
     * reached the app — a verified App Link, text shared in, a code scanned —
     * for the accounts machine's one link flow once the wallet is in front.
     */
    val pendingMultisigLink = MutableStateFlow<String?>(null)

    /** A tapped multisig notification (M6): chain, safeTxHash, the multisig. */
    val pendingMultisigOpen = MutableStateFlow<Triple<Int, String, String?>?>(null)

    /** Spec 045: the platform's documents (picker, creator, share sheet); the activity attaches them in onCreate. */
    @Volatile
    var documents: app.getvela.wallet.feature.documents.DocumentPorts? = null

    /** A notification was tapped: the row to open once the wallet is showing (phase 4). */
    val pendingReceipt = kotlinx.coroutines.flow.MutableStateFlow<String?>(null)

    /** Spec 043: the send path's host — one per process, one attempt per open. */
    val send: SendController by lazy {
        SendController(
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
            relay = relay,
            pool = pool,
            feed = wallet.feedExecutor,
            accountStore = accountStore,
            balances = { wallet.balances.value },
            // Spec 105: send offers the space in front's chains only.
            networks = { spaceNetworks.value },
            signer = {
                ParallelSpaceHook.signer()
                    ?: passkeySigner
                    ?: error("no signer attached: the send began before an activity attached the passkey ceremony")
            },
            haptic = { kind ->
                when (kind) {
                    SendHapticKind.Success -> Haptics.success(app)
                    SendHapticKind.Error -> Haptics.error(app)
                }
            },
            // `clear_token_cache`: the token list must be read again, not
            // throttled away; the new round reaches Send as HoldingsUpdated.
            refreshBalances = { wallet.refresh(force = true) },
            // Spec 082 RG3: a written row is one resolved — the feed re-reads
            // now, so the pending row shows at submit, not at the next tick.
            feedChanged = { wallet.feedReconciled(1) },
            // Phase 4: off the main thread — the reverse lookup hashes through the core.
            identity = { address -> resolveIdentity(address)?.let { SendRecipientIdentity(name = it.name, source = it.source) } },
            // r4a-18 / vis-24 (the web's `knownName` / `ownAddress`): one of the
            // person's accounts on this device, else a contact they named — a
            // resolved name is a service's word, not theirs.
            knownName = { address ->
                session.view.value.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) }?.name?.ifBlank { null }
                    ?: contacts.view.value.contacts.firstOrNull { it.address.equals(address, ignoreCase = true) }?.name?.ifBlank { null }
            },
            // r10 S4: a multisig on this device is one of the person's own too —
            // sending to it was labelled "first time sending here".
            ownAddress = { address ->
                session.view.value.accounts.any { it.address.equals(address, ignoreCase = true) } ||
                    (multisigBuilt && multisig.isMultisig(address))
            },
            // The importer is opened in the person's currency when that is known —
            // committed, or the stored choice on its way (F8); only before either
            // does it get the placeholder it needs to exist, which its sheet does
            // not say and the wallet route replaces the moment the real one is known.
            currencyCode = { settings.currency.value.let { SendLive.batchCurrency(it) ?: it.code } },
            fiatRate = { code -> settings.fiatRate(code) },
            documents = { documents },
            // F6 / F27: answered by the network machine's own verdict the moment
            // it has one — added, not found, or why it stopped in the core's
            // sentence. It was a ten-second wait that only "added" ended early,
            // and every other ending was then reported as "not found".
            addNetwork = { chainId -> settings.addNetworkSettled(chainId) { key -> i18nRuntime.t(key) } },
            preferredTier = { settings.feeTier.value.tier },
            numberPreset = { Formats.current.resolvedNumber().wire },
            trustedSigner = { trustedSigner },
            words = { key, vars -> i18nRuntime.t(key, vars) },
            // Spec 078: one source for the picker and the asset list.
            holdings = wallet.holdings,
        ).also { controller ->
            // Spec 069: the stored default speed, read now and followed after —
            // Settings changing it reaches a send already open.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
                settings.refreshFeeTier()
                settings.feeTier.collect { controller.preferenceChanged() }
            }
            // The two halves of the handoff: the send hands the tracker a
            // hash; the tracker hands the send its verdict.
            controller.onTrackSubmitted = { handoff -> wallet.trackSubmitted(handoff) }
            // Spec 082 RJ1: rows written ahead of a POST that never went.
            controller.onTrackWithdrawn = { hash, ids -> wallet.trackWithdrawn(hash, ids) }
            // What a tracker entry means for the receipt on screen is the
            // core's one mapping (`sendReceiptOutcomeOf`, spec 082): a slow,
            // unreachable or may-have-been-sent op says nothing; a relay that
            // acknowledges a lost reply turns it back to "submitted".
            wallet.onTrackVerdict = { entry ->
                SendReceiptOutcomes.of(entry)?.let { outcome -> controller.receiptUpdate(entry.user_op_hash, outcome) }
            }
            // One in flight per account and network: every tracker render's
            // ops, as the core read them from the tracker's own view JSON —
            // a second send on that chain waits for the first.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
                wallet.inFlightOps.collect { ops -> controller.inFlightOps(ops) }
            }
            // P2.5: a send opening reads the book's send history again — the
            // picker's suggestions come from it and a send may have landed
            // since (web `openSend` → `history_changed`). The book stays.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
                controller.opens.collect { opened -> if (opened > 0) contacts.historyChanged() }
            }
            // The passkey's ceremony, for the AI agent's transfers to follow:
            // a transfer is awaiting its passkey once the prompt is up.
            controller.onCeremony = { event -> agent.ceremony(event) }
            // Native multisig M2: a multisig's confirm proposes — the proposals
            // machine's draft, its review, then one passkey.
            controller.onProposeSafeTx = { op -> MultisigWiring.proposeFromSend(multisig, op) }
        }
    }

    /**
     * The AI agent (spec 103, spec 104 phase 7): the core's, answering through
     * this container's controllers. Off unless the person turns it on.
     */
    val agent: AgentStore by lazy {
        AgentStore(
            VelaStore(app),
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            KeystoreTokenVault(VelaStore(app)),
            KeystoreTokenVault(VelaStore(app), KeystoreTokenVault.AGENT_URL_KEY, KeystoreTokenVault.AGENT_URL_ALIAS),
        )
    }

    /** What the agent reaches, here — resolved as it is asked for, so nothing is built early. */
    private val agentWallet = object : AgentWallet {
        override val session get() = this@AppContainer.session
        override val contacts get() = this@AppContainer.contacts
        override val wallet get() = this@AppContainer.wallet
        override val settings get() = this@AppContainer.settings
        override val send get() = this@AppContainer.send
        override val preferences get() = this@AppContainer.preferences
        override val networkSpace get() = this@AppContainer.networkSpace
        override val theme get() = themeRepository.current
        override suspend fun setTheme(value: ThemePreference) = themeRepository.setThemePreference(value)
        override fun applyLanguage(tag: String) = this@AppContainer.applyLanguage(tag)
        // The request with its sheet's facts (spec 103): what the agent reviews,
        // and the fee its opinion is bound to.
        override fun signingJson(): String? = signing.value?.agentSigningJson()
        override val browser get() = this@AppContainer.browser
        override fun signingSheet() = signing.value
        override fun agentWallets() = agent.config.value.wallets
        override suspend fun erase(): List<String> = eraseDevice()
        override suspend fun clearStorageItem(item: String) = this@AppContainer.clearStorageItem(item)
        override suspend fun clearCaches() = this@AppContainer.clearCaches()
        override suspend fun switchAccount(address: String): Boolean {
            val current = session.view.value
            if (current.address.equals(address, ignoreCase = true)) return true
            val row = current.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) } ?: return false
            session.switchAccount(row.index)
            return kotlinx.coroutines.withTimeoutOrNull(5_000L) {
                session.view.first { it.address.equals(address, ignoreCase = true) }
            } != null
        }
    }

    /**
     * Erase This Device: what survived (empty when everything went). What an
     * erase removes is the CORE's rule (`is_erasable_key`); what it sweeps is
     * `eraseDevice`'s — the second theme store, the crash record, the logs, the
     * caches and every browsed site's data (spec 081 FR-017). Live pages hear
     * their sites disconnected first. On a clean sweep the session ends.
     */
    suspend fun eraseDevice(): List<String> {
        browser.revokeAll()
        val left = DeviceStorage.eraseDevice(app, VelaStore(app))
        // The AI agent's token: its sealed bytes went with the sweep; its key
        // leaves the Keystore.
        agent.forgetToken()
        // Spec 091: the debug-mode key went with the rest — About hides the
        // switch again, and the browser follows.
        preferences.reloadDebugMode()
        // Spec 105: and the space with it — the device starts over in mainnet.
        networkSpace.reload()
        if (left.isEmpty()) {
            session.signOut()
            session.signOutConfirmed()
        }
        return left
    }

    /** One of the storage page's items, cleared (spec 058). */
    suspend fun clearStorageItem(itemId: String) {
        when {
            // One connected site (spec 070): the core revokes it, writes the
            // store and tells any open page.
            "://" in itemId -> browser.revoke(itemId)
            // Every grant: gone from the store AND from the live core, which
            // used to keep them until a restart.
            itemId == "dapps" -> {
                DeviceStorage.clear(VelaStore(app), itemId)
                browser.revokeAll()
            }
            else -> DeviceStorage.clear(VelaStore(app), itemId)
        }
    }

    /** The caches this app can rebuild, cleared; the balances read again. */
    suspend fun clearCaches() {
        DeviceStorage.clearCaches(VelaStore(app))
        wallet.refresh()
    }

    /** Set once [browser] exists: a memory warning has no engines to let go of before that. */
    @Volatile
    private var browserBuilt = false

    /**
     * The system asked for memory back (spec 099 FR-004): the browser lets go
     * of the engines the core's plan under pressure says to. Never builds the
     * browser to do it.
     */
    fun memoryPressure() {
        if (browserBuilt) browser.memoryPressure()
    }

    /** The in-app browser (spec 044, on the core's `dapp_browser` since 070): the tab engines and what they carry. */
    val browser: BrowserController by lazy {
        BrowserController(
            context = app,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            store = VelaStore(app),
            pool = pool,
            feed = wallet.feedExecutor,
            relay = relay,
            debuggable = BuildConfig.DEBUG,
            debugMode = preferences.view.value.debugMode.on,
        ).also { controller ->
            browserBuilt = true
            // The core forwards one signature at a time: the four signing
            // machines are born for it, answer it, and die with it.
            controller.onForwardToSigning = { operation ->
                // M5: a request to a multisig is the proposals machine's, never the
                // Vela signing pipeline's — held until executed, answered as it formed it.
                if (multisig.isMultisig(operation.granted_address)) {
                    val params = runCatching { kotlinx.serialization.json.Json.parseToJsonElement(operation.params_json) }
                        .getOrDefault(kotlinx.serialization.json.JsonNull)
                    multisig.dappRequest(
                        id = multisigDappId(operation.tab, operation.id), address = operation.granted_address,
                        method = operation.method, params = params, origin = operation.origin, chainId = operation.chain_id,
                    ) { answer -> controller.multisigAnswered(operation.tab, operation.id, answer) }
                } else {
                    openSigning(controller, operation)
                }
            }
            // The page behind the open sheet is gone and already answered.
            controller.onCancelSigning = { tab, id ->
                signing.value?.takeIf { open -> open.request.value?.let { it.transportId == tab && it.id == id } == true }?.cancel()
                if (multisigBuilt) multisig.dappGone(multisigDappId(tab, id))
            }
            // core-ms-2: a multisig batch's status is the proposals machine's.
            controller.onMultisigCallsStatus = { chainId, batch -> multisig.callsStatus(chainId, batch) }
            // Spec 100: a page asks to add a network. The sheet is
            // `network_admin`'s (the same check and save as Settings'); this
            // container only carries the request there and its ending back.
            controller.onForwardToAddNetwork = { operation ->
                // The core holds the request until the ledger is read; make
                // sure it is being read.
                if (!settings.networks.value.loaded) settings.startNetworks()
                settings.dappAddRequested(operation.tab, operation.id, operation.origin, operation.ask)
            }
            controller.onCancelAddNetwork = { tab, id -> settings.dappAddCancelled(tab, id) }
            settings.onDappAddSettled = { tab, id, outcome -> controller.addNetworkAnswered(tab, id, outcome) }
            val follow = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate)
            // Every wallet address and the active one: every grant follows it.
            follow.launch {
                // A multisig shown is the account sites see, as on the wallet (M5).
                kotlinx.coroutines.flow.combine(session.view, multisig.accountsView, accountChains()) { view, ms, own -> Triple(view, ms, own) }
                    .collect { (view, ms, own) ->
                        if (!view.loading) {
                            val shown = ms.activeRow?.account?.address
                            controller.accountsChanged(
                                view.accounts.map { it.address } + ms.accounts.map { it.account.address },
                                shown ?: view.address.takeIf { it.isNotBlank() },
                                // r8-21: a multisig's own networks (its record's), so a site starts on one.
                                // r9 (core `site_start_chain_for`): a Vela account's are where it
                                // holds something or has sent from — a new site starts on the first
                                // of them the space offers, never Sepolia for an Arc-only wallet.
                                networks = ms.activeRow?.account?.chainIds ?: own,
                            )
                        }
                    }
            }
            // The chains a page may switch to are the space in front's
            // networks, live (spec 105): a switch of space is a change of them.
            // r10 follow-up (core §10.3): said with the space and every network
            // Vela has, so the other space's network is refused by name and a
            // space switch moves the sites it leaves behind.
            follow.launch {
                combine(settings.networks, networkSpace.space) { all, space -> all to space }
                    .collect { (all, space) ->
                        controller.spaceNetworksChanged(
                            space = space,
                            chainIds = NetworkSpaces.view(space, all).networks.map { it.chain_id.toInt() },
                            knownChainIds = all.networks.map { it.chain_id.toInt() },
                        )
                    }
            }
            // Spec 091: Settings' debug mode, live — revealed, switched, or
            // hidden again by an erase, it is the preferences' to store and
            // reaches the core's page gate and every open tab from here.
            follow.launch {
                preferences.view.collect { view -> controller.debugModeChanged(view.debugMode.on) }
            }
        }
    }

    /** The signing sheet's controller while a page's request is open (spec 044). */
    val signing = kotlinx.coroutines.flow.MutableStateFlow<SigningController?>(null)

    /**
     * Spec 079: how the last request ended, kept on screen after the core
     * closes its sheet (the tick, or "not landed yet"). Cleared by its sheet.
     */
    val signingAftercare = kotlinx.coroutines.flow.MutableStateFlow<SigningAftercare?>(null)

    private val signingScope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate)

    private fun openSigning(browser: BrowserController, operation: DbrOperation.ForwardToSigning) {
        // The user operation this request submitted, if any: the core translates
        // the page's later receipt lookups by it.
        var userOpHash: String? = null
        openSigningRequest(
            request = IncomingRequest(
                id = operation.id,
                method = operation.method,
                paramsJson = operation.params_json,
                origin = operation.origin,
                transportId = operation.tab,
                chainId = operation.chain_id,
                grantedAddress = operation.granted_address,
                // r10 follow-up (core §10.4): a space switch moved this site
                // since its last request — the browser's view of the request
                // it forwards says so, committed before the forward runs.
                spaceNoteChainId = browser.dapp.value.signing
                    ?.takeIf { it.tab == operation.tab && it.id == operation.id }
                    ?.space_note?.chain_id,
            ),
            answer = { payload -> browser.signingAnswered(operation.tab, operation.id, payload, userOpHash) },
            rememberUserOp = { userOpHash = it },
        )
    }

    /**
     * The Ethereum backup (spec 062): ONE transaction the wallet asks ITSELF to
     * sign, through the same controller a page's request opens — so the estimate
     * of the real operation, the fee, the funding guidance, the passkey and the
     * receipt are the existing ones. Its answer has no page to go to; the sheet
     * closing is the whole acknowledgement. Anyone may submit these bytes — the
     * registry re-verifies every signature in them — so the person's Safe is
     * simply the most convenient payer.
     */
    fun openEthereumBackup(call: RegistryBackup.Call) {
        val address = session.view.value.address
        if (address.isBlank()) return
        val params = org.json.JSONArray().put(JSONObject().put("from", address).put("to", call.to).put("value", "0x0").put("data", call.data))
        openSigningRequest(
            request = IncomingRequest(
                id = "vela-ethereum-backup-${System.currentTimeMillis()}",
                method = "eth_sendTransaction",
                paramsJson = params.toString(),
                origin = "https://getvela.app",
                transportId = WALLET_TRANSPORT,
                chainId = call.chainId,
            ),
            answer = {},
            rememberUserOp = {},
        )
    }

    /**
     * One `eth_call` through the pool: the RAW result, bare `0x` included — a
     * chain without the contract is an answer ("not here"), not a silence — or
     * `null` when nobody answered. The registry walks all tell the two apart.
     */
    suspend fun rawEthCall(chainId: Int, to: String, data: String): String? =
        (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", to).put("data", data), "latest")) as? RpcResult.Body)
            ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")
            ?.takeIf { it.startsWith("0x") }

    /** Where the active wallet's founding record stands on Ethereum (spec 062). */
    val registryBackup: RegistryBackup by lazy {
        RegistryBackup(
            // The RAW result, bare `0x` included: a chain without the registry is
            // an answer ("not here"), not a silence.
            ethCall = ::rawEthCall,
            step = { address, key, answers, target -> uniffi.vela_core_uniffi.registryBackupStep(address, key, answers, target) },
        )
    }

    /** Which passkeys control a wallet (spec 062): the registry contract's answer, or the device's. */
    val walletKeys: WalletKeys by lazy {
        WalletKeys(ethCall = ::rawEthCall)
    }

    /**
     * The account record's keys in founding order, as the keys walk wants them
     * — each with its OWN name and transports as the record keeps them (r10
     * follow-up, core §10.5). The account's name is never handed in as a key's:
     * the walk takes it beside the keys ([WalletKeys.read]'s `accountName`), and
     * a key labelled with it is "Key n". A legacy record (no `keys` array) is
     * its one scalar key, unnamed.
     */
    suspend fun deviceKeysOf(address: String): List<WalletKeys.DeviceKey> {
        val port = StoreAccountPort(AccountStore(app))
        val record = port.accountJson(address)?.let { runCatching { JSONObject(it) }.getOrNull() }
        val named = record?.optJSONArray("keys")
        return port.keysOf(address).map { key ->
            val stored = named?.let { keys ->
                (0 until keys.length()).asSequence().mapNotNull { keys.optJSONObject(it) }
                    .firstOrNull { it.optString("credential_id") == key.credentialId }
            }
            WalletKeys.DeviceKey(
                publicKeyHex = key.publicKeyHex,
                name = stored?.optString("name").orEmpty(),
                transports = stored?.optString("transports").orEmpty(),
                credentialId = key.credentialId,
            )
        }
    }

    /**
     * The credential this account signs with — its key route's (the core's
     * `signing_plan` over the stored record, spec 102) — or empty for a record
     * from before the sign-in key. The keys walk marks that key's row
     * (2026-09-26).
     */
    suspend fun signInCredentialOf(address: String): String =
        SigningPlan.of(StoreAccountPort(AccountStore(app)).accountJson(address))?.credentialId.orEmpty()

    /** The active account's FIRST founding key — the one the registry files its groups under. */
    suspend fun foundingKeyOf(address: String): String? =
        StoreAccountPort(AccountStore(app)).keysOf(address).firstOrNull()?.publicKeyHex

    /** Internal, not private: the benchmark build types' sheet journey opens one too (src/benchmark). */
    internal fun openSigningRequest(
        request: IncomingRequest,
        answer: (SignResponsePayload) -> Unit,
        rememberUserOp: (String) -> Unit,
    ) {
        signingScope.launch {
            val address = session.view.value.address
            val accountPort = StoreAccountPort(AccountStore(app))
            val credential = accountPort.keysOf(address).firstOrNull()?.credentialId
            if (address.isBlank() || credential == null) {
                answer(SignResponsePayload.Err(4100, SignErrorKind.UnauthorizedAccount, "No wallet account available"))
                return@launch
            }
            // The browser core forwards one page request at a time; what can
            // still be open here is the wallet's OWN request (the Ethereum
            // backup). The page is refused plainly rather than queued behind it.
            signing.value?.let { open ->
                answer(SignResponsePayload.Err(-32002, SignErrorKind.SubmitFailed, "Another request is open"))
                return@launch
            }
            lateinit var controller: SigningController
            // Spec 079: the operation this request submitted, to tell "landed"
            // from "the wait ran out" when the core answers the page.
            var submittedUserOp: String? = null
            signingAftercare.value = null
            controller = SigningController(
                scope = signingScope,
                relay = relay,
                feed = wallet.feedExecutor,
                accounts = accountPort,
                signer = { ParallelSpaceHook.signer() ?: passkeySigner ?: error("no signer bound") },
                // Spec 105: the space in front's chains, as the page's are.
                knownChains = { spaceNetworks.value.networks.map { it.chain_id.toInt() } },
                wallet = SignAccountRef(address = address, credential_id = credential),
                preferredTier = { settings.feeTier.value.tier },
                numberPreset = { Formats.current.resolvedNumber().wire },
                trustedSigner = { trustedSigner },
                words = { key, vars -> i18nRuntime.t(key, vars) },
                // The inner calls' own gas floor (spec 062): without it an undeployed
                // Safe's first contract call goes out with the relay's "no code here" figure.
                measureCall = { chainId, from, to, valueHex, data ->
                    (pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
                        ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
                },
                ports = object : SigningController.Ports {
                    override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                        answer(payload)
                        // Closed by the person after approving: the answer still
                        // goes, the ending does not reappear (spec 079).
                        signingAftercare.value = if (controller.landingClosed) {
                            null
                        } else {
                            SigningAftercare.of(
                                method = request.method,
                                chainId = request.chainId,
                                payload = payload,
                                submittedUserOp = submittedUserOp,
                            )
                        }
                        // Answered either way: the sheet closes off this, page or no page.
                        controller.markAnswered()
                    }
                    override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) {
                        submittedUserOp = submitted.userOpHash
                        rememberUserOp(submitted.userOpHash)
                    }
                    // Spec 082 RA9: the ceremony's two edges reach the core
                    // through the controller (`CeremonyStarted` / `CeremonyDone`),
                    // so the sheet's words follow the passkey, not a guess.
                    // Spec 082 RG3: a record write re-reads the feed at once —
                    // one row resolved, so the dApp's row shows in seconds.
                    override fun recordsPersisted() = wallet.feedReconciled(1)
                    override fun recordPersisted(recordId: String) = Unit
                    // By address: the machine's row index is not the session's.
                    // `true` only once the session's active address IS it.
                    override suspend fun switchAccount(address: String): Boolean {
                        val current = session.view.value
                        if (current.address.equals(address, ignoreCase = true)) return true
                        val row = current.accounts.firstOrNull { it.address.equals(address, ignoreCase = true) } ?: return false
                        session.switchAccount(row.index)
                        return kotlinx.coroutines.withTimeoutOrNull(5_000L) {
                            session.view.first { it.address.equals(address, ignoreCase = true) }
                        } != null
                    }
                    override fun nativeSymbol(chainId: Int): String =
                        settings.networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }?.native_symbol ?: "ETH"
                    override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = wallet.trackSubmitted(handoff)
                    // Spec 082 RJ1: a written-ahead op proven never sent.
                    override fun trackWithdrawn(userOpHash: String, recordIds: List<String>) = wallet.trackWithdrawn(userOpHash, recordIds)
                    // Spec 082 RJ4: the answer follows the tracker (`OpTracked`).
                    override fun trackerView() = wallet.tracker
                    // One in flight per account and network (the core's read of the tracker's own JSON).
                    override fun inFlightOps() = wallet.inFlightOps
                    override fun dataBase(): String = settings.endpointUrl(NetEndpointField.EthereumData)
                    // The pool's answer as it came; the core reads it (`simOutcome`, RG6).
                    override suspend fun simulate(chainId: Int, params: List<Any?>): RpcResult =
                        pool.call(chainId, "eth_simulateV1", params)
                    // dapp-15: is the chain answering a plain read again?
                    override suspend fun nodeAnswers(chainId: Int): Boolean =
                        (pool.call(chainId, "eth_blockNumber", emptyList()) as? RpcResult.Body)
                            ?.json?.optString("result")?.startsWith("0x") == true
                    override suspend fun judgeDeltas(chainId: Int, wallet: String, deltas: List<app.getvela.wallet.feature.wallet.core.TrustAssetDelta>): app.getvela.wallet.feature.wallet.core.TrustSimView? =
                        this@AppContainer.wallet.judgeSimDeltas(wallet, chainId, deltas)
                    // cs-13: the raw code as the chain gave it; an error or no answer is `null`.
                    override suspend fun getCode(chainId: Int, address: String): String? {
                        val body = (pool.call(chainId, "eth_getCode", listOf(address, "latest")) as? RpcResult.Body)?.json ?: return null
                        if (body.optJSONObject("error") != null) return null
                        return body.optString("result").takeIf { it.startsWith("0x") }
                    }
                    override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> {
                        val body = (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", to).put("data", data), "latest")) as? RpcResult.Body)?.json
                            ?: return null to false
                        val error = body.optJSONObject("error")
                        if (error != null) return null to error.optString("message").contains("revert", ignoreCase = true)
                        return body.optString("result").takeIf { it.startsWith("0x") } to false
                    }
                },
            )
            signing.value = controller
            controller.open(request)
            // Spec 069: the stored default, read now and followed while the
            // sheet is up — a request can arrive before anything else read it.
            // A child of this coroutine, so it ends with the sheet.
            launch {
                settings.refreshFeeTier()
                settings.feeTier.collect { controller.preferenceChanged() }
            }
            controller.closed.collect { closed -> if (closed) { if (signing.value === controller) signing.value = null; throw kotlinx.coroutines.CancellationException("answered") } }
        }
    }

    val contacts: ContactsController by lazy {
        ContactsController(
            context = app,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
            // Paying another Vela user should show their name, not forty hex
            // characters. The index this asks is the same one onboarding
            // publishes to, through the same client.
            registryName = { address -> registryNames.nameFor(address) },
            // Phase 4: off the main thread (this controller runs on it) — a history
            // of 200 counterparties is 200 reverse lookups as the wallet opens.
            identity = { address -> resolveIdentity(address)?.let { ContactIdentity(name = it.name, source = it.source) } },
            code = { chainId, address ->
                (pool.call(chainId, "eth_getCode", listOf(address, "latest")) as? RpcResult.Body)
                    ?.json?.optString("result")?.takeIf { it.startsWith("0x") }
            },
            documents = { documents },
        )
    }

    private val i18nExecutor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "vela-i18n")
    }

    /**
     * How many of this app's screens are in front of somebody.
     *
     * Counted rather than a boolean, because a rotation stops one activity and
     * starts another and the count never reaches zero — a boolean set on stop
     * would report "backgrounded" for the instant in between, and the receive
     * watcher would end its session every time somebody turned their phone.
     */
    @Volatile
    private var foregroundActivities = 0

    /** r11-1: activities resumed now ([onScreen]). */
    @Volatile
    private var resumedActivities = 0

    /**
     * The one place this app learns it has been put away.
     *
     * `registerActivityLifecycleCallbacks` rather than a lifecycle observer in
     * a composable: the question is about the PROCESS, and a composable that
     * leaves the composition answers nothing at all.
     */
    /** An activity has been in front once: a later first start is a return. */
    private var everStarted = false

    private val activityCounter = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityStarted(activity: android.app.Activity) {
            foregroundActivities += 1
            // ob-2: back in front — a wallet saved here whose registration has
            // not landed may land now (the web's `online`). The core ignores it
            // when nothing is owed or a round is already running. Not on the
            // first start: the boot's own round is that one.
            if (foregroundActivities == 1 && everStarted) session.syncPending()
            everStarted = true
            wallet.focused()
            // The AI agent answers only while the wallet is in front.
            agent.sceneActive(true)
        }

        override fun onActivityStopped(activity: android.app.Activity) {
            foregroundActivities = (foregroundActivities - 1).coerceAtLeast(0)
            if (foregroundActivities == 0) {
                wallet.backgrounded()
                // In the background it hangs up, and what waited is withdrawn.
                agent.sceneActive(false)
                // M6 (r10): put away with a multisig to follow — one expedited
                // round now, the one a restrictive scheduler still runs.
                if (multisigBuilt && multisig.accountsView.value.accounts.isNotEmpty()) {
                    runCatching { MultisigNotifier.checkOnLeaving(app) }
                        .onFailure { VelaLog.failure("multisig.poll", "leaving round", it) }
                }
            }
        }

        override fun onActivityCreated(activity: android.app.Activity, state: android.os.Bundle?) = Unit
        override fun onActivityResumed(activity: android.app.Activity) {
            resumedActivities += 1
        }

        override fun onActivityPaused(activity: android.app.Activity) {
            resumedActivities = (resumedActivities - 1).coerceAtLeast(0)
        }
        override fun onActivitySaveInstanceState(activity: android.app.Activity, out: android.os.Bundle) = Unit
        override fun onActivityDestroyed(activity: android.app.Activity) = Unit
    }

    fun start() {
        // Spec 047: logos come from the chain-data endpoint (the founder's ruling); the base follows the endpoints table.
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default).launch {
            settings.ethereumDataBase().collect { base -> Marks.base = base }
        }
        // Spec 102: the saved signing pages — read (and the 071 page imported)
        // before any account's venue choices are drawn.
        settings.refreshSigningPages()
        // Spec 105: a switch of space — the person's or an approved agent's —
        // reads the balances again, forced, so the total is the new space's.
        // The lists follow [spaceNetworks] by themselves. Nothing to read for a
        // wallet that has not been opened.
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
            networkSpace.space.drop(1).collect { if (walletOnce.isInitialized()) wallet.spaceChanged() }
        }
        // Debug trace of the pool's chain verdicts (spec 043 phase 4).
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default).launch {
            pool.view.collect { view ->
                VelaLog.event("rpc.pool", "verdicts", "failed" to view.failed_chains, "rateLimited" to view.rate_limited_chains, "banned" to view.banned.map { "${it.url} at=${it.banned_at_ms} permanent=${it.permanent}" })
            }
        }
        // The balance machine has had a focus-driven auto-refresh since spec
        // 041 phase 4 and nothing was telling it; the receive watcher needs the
        // same signal to stop polling from a pocket.
        app.registerActivityLifecycleCallbacks(activityCounter)
        i18nExecutor.execute {
            i18nRuntime.initialize(LocaleResolver.resolve(currentLocales()))
            multisigChannels()
        }
        if (BuildConfig.DEBUG) {
            // Spec 048: a raw store seed for the device pass, applied before the
            // session reads (see DebugSeed). Release builds boot straight away.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO).launch {
                DebugSeed.apply(app, VelaStore(app))
                session.boot()
                startAgent()
            }
        } else {
            session.boot()
            startAgent()
        }
    }

    /**
     * The AI agent connects as set up (spec 103) — off unless the person
     * turned it on. After the session's boot, whose view its first request
     * reads (and after a debug seed, which may hold its settings).
     */
    private fun startAgent() = agent.start(agentWallet)

    /**
     * r9-8: the multisig's notification channels, made at the app's start and
     * named again in a new language (on the i18n thread, after its catalog):
     * there before the first post, and listed in the system's settings.
     */
    private fun multisigChannels() {
        runCatching {
            i18nRuntime.include(I18nCatalogs.MULTISIG)
            MultisigNotifier.ensureChannels(app, MultisigWords(i18nRuntime))
        }.onFailure { VelaLog.failure("multisig.notify", "channels", it) }
    }

    /**
     * r9 (`site_start_chain_for`'s `account_chains`): the networks the Vela
     * account in front lives on, most telling first — where it holds
     * something, by worth, then where it has sent from. Facts the wallet
     * read; where a site starts is the browser machine's.
     */
    private fun accountChains(): kotlinx.coroutines.flow.Flow<List<Int>> =
        kotlinx.coroutines.flow.combine(wallet.balances, wallet.feed) { balances, feed ->
            val held = balances.tokens
                .filter { !it.spam && (it.balance.toBigDecimalOrNull()?.signum() ?: 0) > 0 }
                .sortedByDescending { (it.balance.toDoubleOrNull() ?: 0.0) * (it.price_usd ?: 0.0) }
                .map { it.chain_id }
            (held + feed.transactions.map { it.chain_id }).filter { it > 0 }.distinct()
        }.distinctUntilChanged()

    /** Re-resolves the system locale (activity recreation on locale change). */
    fun applySystemLocale() {
        i18nExecutor.execute {
            i18nRuntime.setLocale(LocaleResolver.resolve(currentLocales()))
            multisigChannels()
        }
    }

    /** Spec 047: the language preference — a tag, or `auto` (the OS's locales through the resolver). */
    fun applyLanguage(choice: String) {
        if (choice == Preferences.AUTO_LANGUAGE || choice == "system" || choice.isBlank()) return applySystemLocale()
        i18nExecutor.execute {
            runCatching { i18nRuntime.setLocale(LocaleResolver.resolve(listOf(java.util.Locale.forLanguageTag(choice)))) }
                .onFailure { VelaLog.failure("i18n", "language preference failed", it) }
            multisigChannels()
        }
    }

    private fun currentLocales(): List<Locale> {
        val localeList = app.resources.configuration.locales
        return (0 until localeList.size()).map { localeList[it] }
    }
}

class VelaWalletApplication : Application() {

    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        // core-perf-1: the core's library is loaded — JNA's registration of
        // every export, uniffi's contract check — by whichever thread touches
        // it first. Touched here, on a worker, before anything else runs, so
        // that is not the main thread on its way to the first frame.
        warmCore()
        // First, so a crash in composition is itself on the record. Debug
        // builds only; see VelaLog.
        VelaLog.install(this)
        // Spec 047 D10: a crash is written first, then rethrown; the next launch raises it.
        CrashReport.install(this, BuildConfig.VERSION_NAME)
        VelaLog.onFault = { scope, error -> CrashReport.recordFault(this, scope, error, BuildConfig.VERSION_NAME) }
        // The parallel space's door, if this build type has one (spec 043,
        // research D2): a release build installs nothing here.
        ParallelSpaceBinding.install(this)
        // Phase 4: the two halves of the launch's main-thread work, named in a
        // Perfetto trace (the startup benchmark's); a no-op when nobody traces.
        android.os.Trace.beginSection("vela.AppContainer.init")
        try { container = AppContainer(this) } finally { android.os.Trace.endSection() }
        android.os.Trace.beginSection("vela.AppContainer.start")
        try { container.start() } finally { android.os.Trace.endSection() }
    }

    private fun warmCore() {
        Thread({
            android.os.Trace.beginSection("vela.core.warm")
            try {
                // Any export loads the library; this one is a constant.
                runCatching { uniffi.vela_core_uniffi.launchAnimationStorageKey() }
            } finally {
                android.os.Trace.endSection()
            }
        }, "vela-core-warm").start()
    }

    /**
     * Spec 099 FR-004: memory is short — the in-app browser keeps only the
     * tab on screen and the busy ones (the core's plan under pressure).
     * `UI_HIDDEN` is not shortage, only the app leaving the screen: a person
     * who steps out for a moment comes back to every tab as it was.
     */
    override fun onTrimMemory(level: Int) {
        super.onTrimMemory(level)
        @Suppress("DEPRECATION")
        val short = level >= android.content.ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW &&
            level != android.content.ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN
        if (short && ::container.isInitialized) {
            VelaLog.event("app.memory", "trim", "level" to level)
            container.memoryPressure()
        }
    }

    override fun onLowMemory() {
        super.onLowMemory()
        if (::container.isInitialized) {
            VelaLog.event("app.memory", "low memory")
            container.memoryPressure()
        }
    }
}

/** The relay every client ships with — the core's default (`service_defaults`, §28). */
private val DEFAULT_BUNDLER_SERVICE_URL: String get() = app.getvela.wallet.core.net.CoreServiceDefaults.value.bundlerServiceUrl

/** The transport id of a request the WALLET made of itself: its answer has no page to go to. */
private const val WALLET_TRANSPORT = "wallet"
