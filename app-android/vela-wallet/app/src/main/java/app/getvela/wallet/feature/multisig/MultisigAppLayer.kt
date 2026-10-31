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
    LaunchedEffect(networks.networks.size, space.networks.size) {
        if (hub.loaded) hub.networksChanged()
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
                hub.openLink(hash, takeKept = false)
                hub.sheet.value = MultisigSheet.Link
                VelaLog.event("multisig.link", "opened")
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
        hub.sheet.value = MultisigSheet.Proposal(chainId, hash)
        container.pendingMultisigOpen.value = null
    }
    // M6: the notification permission, asked when there is first something to
    // say (a multisig created or added, the first proposal) — once per install.
    val context = androidx.compose.ui.platform.LocalContext.current
    val askLauncher = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.RequestPermission(),
    ) { granted -> VelaLog.event("multisig.notify", "permission", "granted" to granted) }
    val scope = androidx.compose.runtime.rememberCoroutineScope()
    val askNotifications: () -> Unit = {
        if (android.os.Build.VERSION.SDK_INT >= 33 &&
            androidx.core.content.ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) !=
            android.content.pm.PackageManager.PERMISSION_GRANTED
        ) {
            scope.launch {
                if (app.getvela.wallet.core.data.NotificationAsk.claim(app.getvela.wallet.core.data.VelaStore(context))) {
                    askLauncher.launch(android.Manifest.permission.POST_NOTIFICATIONS)
                }
            }
        }
    }
    androidx.compose.runtime.DisposableEffect(hub) {
        hub.onProposed = { scope.launch { askNotifications() } }
        onDispose { hub.onProposed = null }
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
        tokenInfo = { chainId, token -> MultisigWiring.tokenInfoOf(container, chainId, token) },
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
