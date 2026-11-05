package app.getvela.wallet.feature.multisig

import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.getvela.wallet.AppContainer
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.multisig.core.MultisigSheet
import app.getvela.wallet.feature.multisig.core.ev
import app.getvela.wallet.feature.onboarding.core.SessionRoute
import kotlinx.coroutines.launch
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.ui.platform.testTag
import androidx.compose.runtime.setValue
import app.getvela.wallet.feature.multisig.core.get
import app.getvela.wallet.feature.multisig.core.int
import app.getvela.wallet.feature.multisig.core.str

/**
 * The multisig, as one layer over the signed-in app (native multisig M1–M6):
 * the hub booted once a wallet is in front and told this device's accounts
 * again when they change, a link that reached the app handed to the accounts
 * machine's one link flow (kept for later when no wallet is here yet), the
 * app's coming and going ticked into the machines' timers, and whichever
 * multisig sheet is open drawn over every route.
 */
@Composable
fun MultisigAppLayer(
    container: AppContainer,
    route: SessionRoute,
    accountsKey: List<String>,
    onAddNetwork: (Int) -> Unit,
    onFund: () -> Unit,
) {
    val hub = container.multisig
    val strings = LocalVelaStrings.current
    remember(strings) { container.i18nRuntime.include(I18nCatalogs.MULTISIG) }
    val accounts by hub.accountsView.collectAsStateWithLifecycle()
    val session by container.session.view.collectAsStateWithLifecycle()
    val networks by container.settings.networks.collectAsStateWithLifecycle()
    val space by container.spaceNetworks.collectAsStateWithLifecycle()
    val contacts by container.contacts.view.collectAsStateWithLifecycle()
    val pendingLink by container.pendingMultisigLink.collectAsStateWithLifecycle()

    // Booted once a wallet is in front; this device's accounts said again when they change.
    LaunchedEffect(route == SessionRoute.Wallet, accountsKey) {
        if (route != SessionRoute.Wallet) return@LaunchedEffect
        hub.boot()
        hub.refreshLocal()
    }
    // r13-6: keyed on the ids (a space switch can keep the count), and said
    // only once booted — a change while the boot waits is said by the boot.
    val networkIds = networks.networks.map { it.chain_id }
    val spaceIds = space.networks.map { it.chain_id }
    LaunchedEffect(networkIds, spaceIds) {
        hub.networksChanged()
    }
    // The app's coming and going: the machines' timers follow it (§1.4).
    LifecycleResumeEffect(hub) {
        hub.appActive(true)
        onPauseOrDispose { hub.appActive(false) }
    }
    // r8-8: a transaction link opened on a device with no Vela account — most
    // often an outside member's phone. Kept for an account made later, AND
    // said at once with the way an outside member signs (the connect page,
    // where its wallet is) — never a silent Welcome.
    var waitingConnect by remember { androidx.compose.runtime.mutableStateOf<String?>(null) }
    // A link that reached the app: the accounts machine's one link flow.
    LaunchedEffect(pendingLink, route) {
        val hash = pendingLink ?: return@LaunchedEffect
        when (route) {
            SessionRoute.Loading -> return@LaunchedEffect
            SessionRoute.Onboarding -> {
                // No Vela account yet: the machine keeps it (one day) for the first one.
                hub.boot()
                hub.send(ev("keep_link", "hash" to hash))
                VelaLog.event("multisig.link", "kept until there is an account")
                // The core's reading of the link: a transaction's is the outside way in.
                waitingConnect = waitingConnectLink(hash, MultisigWiring.connectPage(container))
            }
            SessionRoute.Wallet -> {
                hub.boot()
                // r14-2: the newest link wins — its sheet over any sheet open,
                // then the accounts machine's one link flow.
                hub.takeLink(hash)
            }
        }
        container.pendingMultisigLink.compareAndSet(hash, null)
    }
    // A link kept before there was an account: taken once there is one.
    LaunchedEffect(route, hub.loaded) {
        if (route == SessionRoute.Wallet && hub.loaded && accounts.link == null) {
            hub.openLink("", takeKept = true)
            if (hub.accounts.until(3_000) { v -> v["link"]?.takeUnless { it is kotlinx.serialization.json.JsonNull } } != null && hub.sheet.value == null) {
                hub.sheet.value = MultisigSheet.Link
            }
        }
    }
    // A tapped notification (M6): its multisig shown, its transaction's sheet open.
    val tapped by container.pendingMultisigOpen.collectAsStateWithLifecycle()
    LaunchedEffect(tapped, route) {
        val (chainId, hash, address) = tapped ?: return@LaunchedEffect
        if (route != SessionRoute.Wallet) return@LaunchedEffect
        hub.boot()
        if (!address.isNullOrEmpty() && hub.isMultisig(address)) hub.activate(address)
        hub.showProposal(chainId, hash)
        container.pendingMultisigOpen.value = null
    }
    // r14-2 / r14-6: a network the pool had out of reach answers again, or the
    // device comes back online — what could not be read is asked again now
    // (a link's transaction, the sheet's fee look), as iOS and the web recover
    // on their own; never a stale "can't check" left until someone presses.
    val pool by container.pool.view.collectAsStateWithLifecycle()
    val online by app.getvela.wallet.core.net.NetHealth.online.collectAsStateWithLifecycle()
    val unreachable = remember(pool) { (pool.out_of_reach_chains + pool.silent_chains).toSet() }
    var wasUnreachable by remember { androidx.compose.runtime.mutableStateOf(emptySet<Int>()) }
    var wasOnline by remember { androidx.compose.runtime.mutableStateOf(true) }
    LaunchedEffect(unreachable, online) {
        val back = MultisigReach.back(wasUnreachable, unreachable, wasOnline, online)
        wasUnreachable = unreachable
        wasOnline = online
        if (back != null && route == SessionRoute.Wallet) hub.networkBack(back)
    }
    // M6: the notification permission, asked when there is first something to
    // say (a multisig created or added, the first proposal) — once per install.
    val context = androidx.compose.ui.platform.LocalContext.current
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val askLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.RequestPermission(),
    ) { granted ->
        VelaLog.event("multisig.notify", "permission", "granted" to granted, "limited" to MultisigNotifier.backgroundLimited(context))
        scope.launch { MultisigNotifyAsk.answered(app.getvela.wallet.core.data.VelaStore(context)) }
    }
    val askNotifications: () -> Unit = {
        if (android.os.Build.VERSION.SDK_INT < 33) {
            VelaLog.event("multisig.notify", "no ask before Android 13", "enabled" to MultisigNotifier.allowed(context))
        } else if (androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) ==
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            // Said, so a device pass can tell "never asked" from "nothing to ask".
            VelaLog.event("multisig.notify", "already allowed", "enabled" to MultisigNotifier.allowed(context))
        } else {
            scope.launch {
                // r9-8: the multisig's own once — a receipt's earlier ask (087
                // F14) was about a send's outcome; this one is about members
                // waiting on this device, asked when there first is a multisig.
                if (MultisigNotifyAsk.claim(app.getvela.wallet.core.data.VelaStore(context))) {
                    VelaLog.event("multisig.notify", "asking")
                    askLauncher.launch(android.Manifest.permission.POST_NOTIFICATIONS)
                }
            }
        }
    }
    androidx.compose.runtime.DisposableEffect(hub) {
        hub.onProposed = { scope.launch { askNotifications() } }
        onDispose { hub.onProposed = null }
    }
    // r9-8: the moment there is something to say — a multisig on this device
    // (created, added, an invite taken, or one this device became a member of
    // by a member change), shown on the wallet. Asked once; the create and
    // add sheets' own asks are the same once.
    val hasMultisig = accounts.loaded && accounts.accounts.isNotEmpty()
    LaunchedEffect(hasMultisig, route) {
        if (hasMultisig && route == SessionRoute.Wallet) askNotifications()
    }
    // The link flow's own outcomes: a member's transaction opens as its sheet
    // (the router's `open_proposal`); an invite is the add sheet (Link).
    if (route == SessionRoute.Onboarding) {
        waitingConnect?.let { link ->
            MultisigWaitingLinkSheet(
                words = MultisigWiring.words(container, hub),
                link = link,
                onOpen = { container.trustedSignerTab?.open(link) ?: false },
                onClose = { waitingConnect = null },
            )
        }
    } else if (waitingConnect != null && route == SessionRoute.Wallet) {
        waitingConnect = null
    }
    if (route != SessionRoute.Wallet) return

    // r8-18: the words name members from what the multisigs here keep
    // (`known_members`) — read again whenever the accounts view moves. Built
    // once (the view not read in composition), a cold start kept the empty
    // first view for good: every member a bare address twice, the invite's
    // names lost on the sheets.
    val words = remember(accounts, session.accounts, networks, strings) { MultisigWiring.words(container, hub) }
    val hostContext = MultisigHostContext(
        words = words,
        accounts = session.accounts.map { MsLocal(it.address, it.name) },
        contacts = contacts.contacts.filter { ADDRESS.matches(it.address) }.map { MsLocal(it.address, it.name ?: it.resolved_name ?: "") },
        spaceNetworks = space.networks.map { it.chain_id.toInt() },
        otherSpaceName = strings.t(
            if (container.networkSpace.space.value.testnet) app.getvela.wallet.core.i18n.I18nKeys.NetworkSpace.MAINNET
            else app.getvela.wallet.core.i18n.I18nKeys.NetworkSpace.TESTNET,
        ),
        origin = app.getvela.wallet.feature.multisig.core.MsRules.shareOrigin(),
        locale = MultisigWiring.pageLocale(container),
        tokenInfo = { chainId, token -> MultisigWiring.tokenInfoOf(container, chainId, token, hub) },
        explorerTx = { chainId, tx ->
            networks.networks.firstOrNull { it.chain_id.toInt() == chainId }?.explorer_url?.trimEnd('/')?.takeIf { it.isNotBlank() }?.let { "$it/tx/$tx" }
        },
        onAddNetwork = onAddNetwork,
        onFund = onFund,
        askNotifications = askNotifications,
    )
    MultisigHost(hub, hostContext)
}

private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

/** r14-2 / r14-6: when what could not be read is worth asking again. Pure. */
object MultisigReach {
    /**
     * The networks that answer again since the last look — those the pool
     * let go of — or the empty set when the device itself came back online
     * (every network); `null`: nothing came back.
     */
    fun back(before: Set<Int>, now: Set<Int>, wasOnline: Boolean, online: Boolean): Set<Int>? = when {
        online && !wasOnline -> emptySet()
        else -> (before - now).takeIf { it.isNotEmpty() && online }
    }
}

/**
 * r8-8: the connect page for a transaction link opened where there is no
 * Vela account — the core reads the link (`parseProposalLink`) and writes
 * the page's address (`connectLink`, the transaction in its fragment, which
 * the page reads, reviews and signs from alone). `null`: not a transaction
 * link (an invite needs an account to add it: Welcome, the link kept).
 */
internal fun waitingConnectLink(hash: String, connectPage: String): String? {
    val tx = app.getvela.wallet.feature.multisig.core.MsRules.call("parseProposalLink", mapOf("hash" to hash)) ?: return null
    val chainId = tx["chainId"].int ?: return null
    val multisig = tx["multisig"].str ?: return null
    val safeTxHash = tx["safeTxHash"].str ?: return null
    return app.getvela.wallet.feature.multisig.core.MsRules.connectLink(connectPage, null, emptyList(), Triple(chainId, multisig, safeTxHash))
}

/**
 * r8-8: a multisig transaction's link reached a device with no Vela account.
 * An outside member approves where its wallet is: the connect page, opened
 * in that browser or scanned with the wallet app — said here, with the link
 * and its code, over Welcome (where the link also waits for an account).
 */
@Composable
fun MultisigWaitingLinkSheet(words: MultisigWords, link: String, onOpen: () -> Boolean, onClose: () -> Unit) {
    val colors = app.getvela.wallet.core.designsystem.theme.VelaTheme.colors
    MsSheet(words.t("external.sheetTitle"), words.raw("common.done"), onClose = onClose, tag = "multisig.waitingLink") {
        MsNote(words.t("external.outsideLinkHint"))
        MsNote(words.t("external.scanBody"))
        app.getvela.wallet.feature.flows.components.QrCard(
            label = words.t("external.sheetTitle"),
            payload = link,
            modifier = androidx.compose.ui.Modifier.testTag("multisig.waitingLink.qr"),
        )
        MsText(link, MsType.Mono, androidx.compose.ui.Modifier.testTag("multisig.waitingLink.link"), color = colors.fgMuted, maxLines = 4)
        ShareRow(words, link, "multisig.waitingLink")
        app.getvela.wallet.core.designsystem.components.VelaSecondaryButton(
            text = words.t("external.signWithWallet"),
            onClick = { onOpen() },
            modifier = androidx.compose.ui.Modifier.fillMaxWidth().testTag("multisig.waitingLink.open"),
        )
        MsNote(words.t("external.linkLater"))
    }
}

/**
 * r9-8: the notification permission asked for the multisig at most ONCE per
 * install, apart from the send receipt's once (087 F14: never on every
 * screen, never again after an answer). Android itself shows the system's
 * ask at most twice before it stops asking; this is the second context.
 */
object MultisigNotifyAsk {
    const val KEY = "vela.multisigNotificationsAsked"

    /** The system's dialog answered (allowed or not): never asked again. */
    const val ANSWERED = "answered"

    /** Shown, not answered yet (the process may die under it): asked again on a later run. */
    const val ASKING = "asking"

    /**
     * One ask per process: a screen recreated under the dialog (a language
     * switch) does not ask twice — nor do the add sheet's own ask and the
     * layer's "there is a multisig now" ask, which come in the same frame.
     * On the Redmi (MIUI) those two launched the system request twice
     * 250 ms apart and the second came back "denied" at once, the dialog
     * still up: set before anything suspends, so only one gets through.
     */
    private val askedThisRun = java.util.concurrent.atomic.AtomicBoolean(false)

    /**
     * Whether the system's ask may be shown now — once per install, counted
     * from its ANSWER (real device, r10: the mark used to be written before
     * the dialog, so an ask the system never showed — a screen not in front
     * yet, a process gone — was never made again; the store said "asked" and
     * the phone had never seen the question).
     */
    suspend fun claim(store: app.getvela.wallet.core.data.KeyValueStore): Boolean {
        if (!askedThisRun.compareAndSet(false, true)) return false
        if (store.read(KEY) == ANSWERED) return false
        return store.write(KEY, ASKING)
    }

    /** The system's dialog answered: the mark for good. */
    suspend fun answered(store: app.getvela.wallet.core.data.KeyValueStore) {
        store.write(KEY, ANSWERED)
    }

    /** A test's fresh process. */
    internal fun forgetRun() {
        askedThisRun.set(false)
    }
}
