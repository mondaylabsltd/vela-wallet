package app.getvela.wallet

import app.getvela.wallet.core.format.Formats
import kotlinx.coroutines.flow.MutableStateFlow
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.flows.WalletFlowEntry
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.core.diagnostics.CrashReport
import app.getvela.wallet.core.data.Preferences
import app.getvela.wallet.feature.send.core.SendAddNetworkOutcome
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.feature.signing.core.SimDeltas
import android.app.Application
import app.getvela.wallet.core.data.ThemePreferenceRepository
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.send.core.SendReceiptOutcome
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.wallet.core.TrackerWorker
import app.getvela.wallet.feature.wallet.core.TrackerNotifier
import app.getvela.wallet.feature.send.core.ClearSignerLabels
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerChannel
import app.getvela.wallet.feature.signing.clearsigner.ClearSignerTab
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
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocaleResolver
import app.getvela.wallet.feature.contacts.core.ContactsController
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.SessionController
import app.getvela.wallet.feature.settings.core.SettingsController
import app.getvela.wallet.core.data.DebugSeed
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.core.platform.Haptics
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.NetworkEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.WalletController
import java.util.Locale
import java.util.concurrent.Executors
import kotlinx.coroutines.CoroutineScope
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

    val i18nRuntime = I18nRuntime { tag ->
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
    /** Spec 048: split rows to seed into the send machine right after it opens (a group's 群发转账). */
    val pendingSplitSeed = MutableStateFlow<List<SendRecipientDraft>?>(null)

    /**
     * A code scanned from 探索 that is a payment (an address, an `ethereum:`
     * request — spec 070): the send opens on it as if its own scanner had read
     * it, so the core's `scan_resolved` decides what it means.
     */
    val pendingScan = MutableStateFlow<String?>(null)
    /** Spec 048: the home's status line opens the matching rescue sheet on the settings page. */
    val pendingSettingsOverlay = MutableStateFlow<SettingsOverlay?>(null)
    /** Spec 048: the add-token 原生币 tab opens the settings' add-network page. */
    val pendingSettingsPage = MutableStateFlow<SettingsPage?>(null)

    /** Spec 047 D1: the preferences that have no machine — the web's keys, applied app-wide. */
    val preferences = Preferences(
        store = VelaStore(app),
        scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate),
        locale = { app.resources.configuration.locales[0] ?: java.util.Locale.getDefault() },
    ).also { it.load() }

    val accountStore = AccountStore(app)

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
    val identity: IdentityResolver by lazy {
        IdentityResolver(
            store = VelaStore(app),
            ownAccounts = { session.view.value.accounts.map { it.address to it.name } },
            registryName = { address -> registryNames.nameFor(address) },
            ethCall = { chainId, to, data ->
                (pool.call(chainId, "eth_call", listOf(JSONObject().put("to", to).put("data", data), "latest")) as? RpcResult.Body)
                    ?.json?.optString("result")?.takeIf { it.startsWith("0x") && it != "0x" }
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
    val wallet: WalletController by lazy {
        WalletController(
            context = app,
            networks = settings.networks,
            pool = pool,
            scope = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO),
            // A payment from one of this person's OWN wallets should say so by
            // name, and that costs no network call at all.
            ownAccounts = {
                session.view.value.accounts.map {
                    FeedExecutor.FeedOwnAccount(address = it.address, name = it.name)
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
        )
    }

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

    /** Spec 071: the Clear Signer's tab, attached by the activity in onCreate. */
    @Volatile
    var clearSignerTab: ClearSignerTab? = null

    /**
     * Spec 071: the fourth "Sign with" — one channel per process, one ceremony
     * at a time. The page is `sign_pref`'s; the words are the corpus'.
     */
    val clearSigner: ClearSignerChannel by lazy {
        ClearSignerChannel(
            signerUrl = { settings.signPref.value.signer_url },
            openPage = { url -> clearSignerTab?.open(url) ?: false },
            bringBack = { clearSignerTab?.bringBack() },
            words = {
                ClearSignerChannel.Words(
                    closed = i18nRuntime.t("componentsUi.signing.clearSignerClosed"),
                    refused = i18nRuntime.t("componentsUi.signing.clearSignerRefused"),
                    mismatch = i18nRuntime.t("componentsUi.signing.clearSignerMismatch"),
                    timeout = i18nRuntime.t("componentsUi.signing.clearSignerTimeout"),
                    relayDown = i18nRuntime.t("componentsUi.signing.clearSignerRelayDown"),
                )
            },
            // Spec 075: the relay a cross-device pairing goes through, and how
            // this app names itself to the page.
            relayUrl = { settings.signPref.value.relay_url },
            appName = "vela-android/" + BuildConfig.VERSION_NAME,
            labels = { chainId, account ->
                val network = settings.networks.value.networks.firstOrNull { it.chain_id.toInt() == chainId }
                ClearSignerLabels(
                    chainName = network?.display_name,
                    nativeSymbol = network?.native_symbol,
                    accountName = session.view.value.accounts
                        .firstOrNull { it.address.equals(account, ignoreCase = true) }?.name,
                )
            },
        )
    }

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
            networks = { settings.networks.value },
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
            refreshBalances = { wallet.refresh() },
            feedChanged = { wallet.feedReconciled() },
            identity = { address -> identity.resolve(address)?.let { SendRecipientIdentity(name = it.name, source = it.source) } },
            currencyCode = { settings.currency.value.code },
            fiatRate = { code -> settings.fiatRate(code) },
            documents = { documents },
            addNetwork = { chainId ->
                settings.addNetworkByChainId(chainId)
                kotlinx.coroutines.withTimeoutOrNull(10_000L) { settings.networks.first { it.last_added_chain_id == chainId } }
                    ?.let { SendAddNetworkOutcome.Added } ?: SendAddNetworkOutcome.NotFound
            },
            preferredTier = { settings.feeTier.value.tier },
            numberPreset = { Formats.current.resolvedNumber().wire },
            signMethod = { settings.signPref.value.method },
            clearSigner = { clearSigner },
        ).also { controller ->
            // Spec 069: the stored default speed, read now and followed after —
            // Settings changing it reaches a send already open.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate).launch {
                settings.refreshSignPref()
                settings.refreshFeeTier()
                settings.feeTier.collect { controller.preferenceChanged() }
            }
            // The two halves of the handoff: the send hands the tracker a
            // hash; the tracker hands the send its verdict.
            controller.onTrackSubmitted = { hash, ids, chain -> wallet.trackSubmitted(hash, ids, chain) }
            wallet.onTrackVerdict = { hash, status, tx ->
                val outcome = when (status) {
                    TrackStatus.Confirmed -> SendReceiptOutcome.Confirmed(tx_hash = tx.orEmpty())
                    TrackStatus.Dropped -> SendReceiptOutcome.Failed(rejected = false)
                    TrackStatus.Rejected -> SendReceiptOutcome.Failed(rejected = true)
                    TrackStatus.FeeHeld -> SendReceiptOutcome.FeeHeld
                    else -> null
                }
                if (outcome != null) controller.receiptUpdate(hash, outcome)
            }
        }
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
        ).also { controller ->
            // The core forwards one signature at a time: the four signing
            // machines are born for it, answer it, and die with it.
            controller.onForwardToSigning = { operation -> openSigning(controller, operation) }
            // The page behind the open sheet is gone and already answered.
            controller.onCancelSigning = { tab, id ->
                signing.value?.takeIf { open -> open.request.value?.let { it.transportId == tab && it.id == id } == true }?.cancel()
            }
            val follow = CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Main.immediate)
            // Every wallet address and the active one: every grant follows it.
            follow.launch {
                session.view.collect { view ->
                    if (!view.loading) controller.accountsChanged(view.accounts.map { it.address }, view.address.takeIf { it.isNotBlank() })
                }
            }
            // The chains a page may switch to are the wallet's networks, live.
            follow.launch {
                settings.networks.collect { view -> controller.networksChanged(view.networks.map { it.chain_id.toInt() }) }
            }
        }
    }

    /** The signing sheet's controller while a page's request is open (spec 044). */
    val signing = kotlinx.coroutines.flow.MutableStateFlow<SigningController?>(null)

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
     * The account record's keys in founding order, as the keys walk wants them.
     * The record keeps no per-key label, so only key 0 — whose name IS the
     * wallet's — arrives named; the registry's metadata names the rest.
     */
    suspend fun deviceKeysOf(address: String, walletName: String): List<WalletKeys.DeviceKey> =
        StoreAccountPort(AccountStore(app)).keysOf(address).mapIndexed { index, key ->
            WalletKeys.DeviceKey(key.publicKeyHex, if (index == 0) walletName else "", "")
        }

    /** The active account's FIRST founding key — the one the registry files its groups under. */
    suspend fun foundingKeyOf(address: String): String? =
        StoreAccountPort(AccountStore(app)).keysOf(address).firstOrNull()?.publicKeyHex

    private fun openSigningRequest(
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
            controller = SigningController(
                scope = signingScope,
                relay = relay,
                feed = wallet.feedExecutor,
                accounts = accountPort,
                signer = { ParallelSpaceHook.signer() ?: passkeySigner ?: error("no signer bound") },
                knownChains = { settings.networks.value.networks.map { it.chain_id.toInt() } },
                wallet = SignAccountRef(address = address, credential_id = credential),
                preferredTier = { settings.feeTier.value.tier },
                numberPreset = { Formats.current.resolvedNumber().wire },
                defaultMethod = { settings.signPref.value.method },
                clearSigner = { clearSigner },
                // The inner calls' own gas floor (spec 062): without it an undeployed
                // Safe's first contract call goes out with the relay's "no code here" figure.
                measureCall = { chainId, from, to, valueHex, data ->
                    (pool.call(chainId, "eth_estimateGas", listOf(JSONObject().put("from", from).put("to", to).put("value", valueHex).put("data", data))) as? RpcResult.Body)
                        ?.json?.takeIf { it.has("result") && !it.isNull("result") }?.optString("result")?.takeIf { it.startsWith("0x") }
                },
                ports = object : SigningController.Ports {
                    override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                        answer(payload)
                        // Answered either way: the sheet closes off this, page or no page.
                        controller.markAnswered()
                    }
                    override fun opSubmitted(id: String, userOpHash: String) = rememberUserOp(userOpHash)
                    override fun signingStarted() = Unit
                    override fun recordsPersisted() = wallet.feedReconciled()
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
                    override fun trackSubmitted(userOpHash: String, recordIds: List<String>, chainId: Int) = wallet.trackSubmitted(userOpHash, recordIds, chainId)
                    override fun dataBase(): String = settings.endpointUrl(NetEndpointField.EthereumData)
                    override suspend fun simulate(chainId: Int, wallet: String, calls: List<SimDeltas.Call>): List<TrustSimJudgment>? {
                        val body = SimDeltas.body(wallet, calls) ?: return null
                        val answer = (pool.call(chainId, "eth_simulateV1", listOf(body, "latest")) as? RpcResult.Body)?.json ?: return null
                        val logs = SimDeltas.logsOf(answer) ?: return null
                        return this@AppContainer.wallet.judgeSimDeltas(wallet, chainId, SimDeltas.deriveDeltas(logs, wallet))
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
            identity = { address -> identity.resolve(address)?.let { ContactIdentity(name = it.name, source = it.source) } },
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

    /**
     * The one place this app learns it has been put away.
     *
     * `registerActivityLifecycleCallbacks` rather than a lifecycle observer in
     * a composable: the question is about the PROCESS, and a composable that
     * leaves the composition answers nothing at all.
     */
    private val activityCounter = object : Application.ActivityLifecycleCallbacks {
        override fun onActivityStarted(activity: android.app.Activity) {
            foregroundActivities += 1
            wallet.focused()
        }

        override fun onActivityStopped(activity: android.app.Activity) {
            foregroundActivities = (foregroundActivities - 1).coerceAtLeast(0)
            if (foregroundActivities == 0) wallet.backgrounded()
        }

        override fun onActivityCreated(activity: android.app.Activity, state: android.os.Bundle?) = Unit
        override fun onActivityResumed(activity: android.app.Activity) = Unit
        override fun onActivityPaused(activity: android.app.Activity) = Unit
        override fun onActivitySaveInstanceState(activity: android.app.Activity, out: android.os.Bundle) = Unit
        override fun onActivityDestroyed(activity: android.app.Activity) = Unit
    }

    fun start() {
        // Spec 047: logos come from the chain-data endpoint (the founder's ruling); the base follows the endpoints table.
        CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.Default).launch {
            settings.ethereumDataBase().collect { base -> Marks.base = base }
        }
        // Spec 071: how this device signs by default — read before any sheet can open.
        settings.refreshSignPref()
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
        }
        if (BuildConfig.DEBUG) {
            // Spec 048: a raw store seed for the device pass, applied before the
            // session reads (see DebugSeed). Release builds boot straight away.
            CoroutineScope(SupervisorJob() + kotlinx.coroutines.Dispatchers.IO).launch {
                DebugSeed.apply(app, VelaStore(app))
                session.boot()
            }
        } else {
            session.boot()
        }
    }

    /** Re-resolves the system locale (activity recreation on locale change). */
    fun applySystemLocale() {
        i18nExecutor.execute {
            i18nRuntime.setLocale(LocaleResolver.resolve(currentLocales()))
        }
    }

    /** Spec 047: the language preference — a tag, or `auto` (the OS's locales through the resolver). */
    fun applyLanguage(choice: String) {
        if (choice == Preferences.AUTO_LANGUAGE || choice == "system" || choice.isBlank()) return applySystemLocale()
        i18nExecutor.execute {
            runCatching { i18nRuntime.setLocale(LocaleResolver.resolve(listOf(java.util.Locale.forLanguageTag(choice)))) }
                .onFailure { VelaLog.failure("i18n", "language preference failed", it) }
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
        // First, so a crash in composition is itself on the record. Debug
        // builds only; see VelaLog.
        VelaLog.install(this)
        // Spec 047 D10: a crash is written first, then rethrown; the next launch raises it.
        CrashReport.install(this, BuildConfig.VERSION_NAME)
        VelaLog.onFault = { scope, error -> CrashReport.recordFault(this, scope, error, BuildConfig.VERSION_NAME) }
        // The parallel space's door, if this build type has one (spec 043,
        // research D2): a release build installs nothing here.
        ParallelSpaceBinding.install(this)
        container = AppContainer(this)
        container.start()
    }
}

/** `network_admin::DEFAULT_BUNDLER_SERVICE_URL` — the relay every client ships with. */
private const val DEFAULT_BUNDLER_SERVICE_URL = "https://vela-relay-cf.getvela.app"

/** The transport id of a request the WALLET made of itself: its answer has no page to go to. */
private const val WALLET_TRANSPORT = "wallet"
