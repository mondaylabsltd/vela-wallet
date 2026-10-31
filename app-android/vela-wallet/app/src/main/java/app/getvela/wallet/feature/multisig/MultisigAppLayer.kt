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
    if (route != SessionRoute.Wallet) return

    val words = MultisigWiring.words(container, hub)
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
