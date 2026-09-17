package app.getvela.wallet.navigation

import app.getvela.wallet.core.diagnostics.CrashSheet
import app.getvela.wallet.feature.settings.SettingsPage
import app.getvela.wallet.feature.settings.core.NetOverrideField
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.contacts.core.ContactFileFormat
import app.getvela.wallet.feature.contacts.components.ExportFormatSheet
import app.getvela.wallet.core.marks.Marks
import app.getvela.wallet.feature.flows.components.ChainFilterSheet
import app.getvela.wallet.feature.flows.components.ChainFilterRow
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Dispatchers
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.contacts.core.ContactGroupInput
import app.getvela.wallet.feature.contacts.components.GroupMenuSheet
import app.getvela.wallet.feature.contacts.components.GroupEditSheet
import app.getvela.wallet.feature.contacts.components.GroupDeleteConfirmSheet
import app.getvela.wallet.feature.contacts.components.ContactQrSheet
import app.getvela.wallet.core.platform.rememberVelaHaptic
import app.getvela.wallet.core.platform.VelaHaptic
import app.getvela.wallet.core.platform.Gallery
import app.getvela.wallet.core.platform.Clipboard
import app.getvela.wallet.feature.wallet.components.IdenticonViewerSheet
import app.getvela.wallet.core.identicon.LocalIdenticonViewer
import androidx.compose.runtime.CompositionLocalProvider
import app.getvela.wallet.core.net.NetHealth
import app.getvela.wallet.feature.wallet.components.AccountSwitcherSheet
import app.getvela.wallet.feature.wallet.BalanceStatusKind
import app.getvela.wallet.feature.wallet.BalanceStatusModel
import app.getvela.wallet.feature.flows.ShareCardCapture
import app.getvela.wallet.feature.flows.ShareCardModel
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.wallet.core.PayLink
import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope
import app.getvela.wallet.feature.settings.core.DeviceStorage
import app.getvela.wallet.feature.send.core.SendTreasuryStatus
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.core.format.TimeFormatKey
import app.getvela.wallet.core.format.TextScaleLevel
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.data.VelaStore
import app.getvela.wallet.BuildConfig
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import app.getvela.wallet.VelaWalletApplication
import app.getvela.wallet.core.data.ThemePreference
import app.getvela.wallet.core.i18n.LocalVelaStrings
import androidx.compose.foundation.layout.Box
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.rememberLauncherForActivityResult
import app.getvela.wallet.dev.ParallelSpaceHook
import app.getvela.wallet.feature.flows.FlowStep
import app.getvela.wallet.feature.send.core.MtokView
import app.getvela.wallet.feature.flows.AddTokenCallbacks
import app.getvela.wallet.feature.flows.RecipientAction
import app.getvela.wallet.feature.flows.SendCallbacks
import app.getvela.wallet.MainActivity
import app.getvela.wallet.feature.scan.ScanCallbacks
import app.getvela.wallet.feature.send.core.BatchUnit as WireBatchUnit
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendAmountWarning
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreen
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.withIdentity
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.withIdentity as withSignerIdentity
import app.getvela.wallet.feature.contacts.ContactsActions
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsLive
import app.getvela.wallet.feature.contacts.ContactsRoute
import app.getvela.wallet.feature.contacts.core.ContactSaveInput
import app.getvela.wallet.feature.contacts.MenuItemModel
import app.getvela.wallet.feature.contacts.ContactsIcon
import app.getvela.wallet.feature.contacts.ActionMenuModel
import app.getvela.wallet.feature.contacts.ContactsScreenState
import app.getvela.wallet.feature.contacts.gallery.ContactsGalleryScreen
import app.getvela.wallet.feature.onboarding.OnboardingIntent
import app.getvela.wallet.feature.onboarding.OnboardingViewModel
import app.getvela.wallet.feature.onboarding.ThemeSettingsSheet
import app.getvela.wallet.feature.onboarding.WelcomeScreen
import app.getvela.wallet.feature.onboarding.WelcomeViewModel
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.RegistryClient
import app.getvela.wallet.feature.onboarding.core.SessionRoute
import app.getvela.wallet.feature.onboarding.flow.CableQrSheet
import app.getvela.wallet.feature.onboarding.flow.CreateFlowScreen
import app.getvela.wallet.feature.onboarding.flow.EndpointSheet
import app.getvela.wallet.feature.onboarding.flow.FlowSheet
import app.getvela.wallet.feature.onboarding.flow.InsertKeySheet
import app.getvela.wallet.feature.onboarding.flow.LocationAskSheet
import app.getvela.wallet.feature.onboarding.flow.SignInMethodSheet
import app.getvela.wallet.feature.onboarding.flow.SignOutSheet
import app.getvela.wallet.feature.onboarding.flow.UsbPinDialog
import app.getvela.wallet.feature.onboarding.flow.UsbTouchIndicator
import app.getvela.wallet.feature.onboarding.flow.UsbWalletPicker
import app.getvela.wallet.feature.onboarding.placeholder.ImportPlaceholderScreen
import androidx.activity.compose.BackHandler
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowHost
import app.getvela.wallet.feature.flows.gallery.FlowGalleryScreen
import app.getvela.wallet.feature.flows.WalletFlowEntry
import app.getvela.wallet.feature.flows.rememberFlowNavState
import app.getvela.wallet.feature.settings.SettingsActions
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetProviderId
import app.getvela.wallet.feature.settings.SettingsRoute
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.gallery.SettingsGalleryScreen
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.FlowLive
import app.getvela.wallet.feature.flows.FlowScreenModel
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.PaymentRequestView
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.gallery.GalleryScreen
import kotlinx.coroutines.flow.first

object VelaDestinations {
    const val WELCOME = "welcome"
    const val CREATE = "create"
    const val IMPORT = "import"

    // Spec 015: fixture-driven wallet home + preview gallery (research D4).
    const val WALLET = "wallet"
    const val GALLERY = "gallery"

    // Spec 018: fixture-driven contacts screens + their preview gallery (D1).
    // Spec 022: the explore browser, reachable in the app from the Explore tab
    // and directly by intent extra for review.
    const val EXPLORE = "explore"
    const val CONTACTS = "contacts"
    const val CONTACTS_GALLERY = "contacts-gallery"

    /**
     * Spec 021: the wallet-flow preview gallery. The flows THEMSELVES are not
     * destinations — they are a stack inside [WALLET], because they are still
     * fixtures and a `composable(...)` per state would put them in the app's
     * real back stack and let `vela.startDestination` launch one.
     */
    const val FLOWS_GALLERY = "flows-gallery"
    // Spec 023: the settings screen a signed-in person reaches from the tab bar,
    // plus its preview gallery.
    const val SETTINGS = "settings"
    const val SETTINGS_GALLERY = "settings-gallery"

    /** Routes the `vela.startDestination` intent extra may select. */
    val ALL = setOf(
        WELCOME,
        CREATE,
        IMPORT,
        WALLET,
        GALLERY,
        CONTACTS,
        CONTACTS_GALLERY,
        EXPLORE,
        FLOWS_GALLERY,
        SETTINGS,
        SETTINGS_GALLERY,
    )
}

@Composable
fun VelaNavHost(
    darkTheme: Boolean,
    themePreference: ThemePreference,
    onThemeSelected: (ThemePreference) -> Unit,
    startDestination: String = VelaDestinations.WELCOME,
    startFlowState: String? = null,
    settingsState: String? = null,
    settingsDark: Boolean? = null,
) {
    val navController = rememberNavController()
    val context = LocalContext.current
    val application = context.applicationContext as VelaWalletApplication
    val session by application.container.session.view.collectAsStateWithLifecycle()
    val onboarding: OnboardingViewModel = viewModel()

    // Which body the signed-in shell shows. 钱包 and 探索 are SECTIONS of the
    // wallet route; 通讯录 and 设置 are routes pushed over it. Hoisted out of
    // the wallet route (spec 047) so a pushed route's tab bar can pick a
    // section on its way back — before this, 探索 on 通讯录/设置 answered
    // nothing at all (device-found 2026-09-12: 「点击探索没反应」). Survives
    // rotation for the same reason the flow stack does.
    var section by rememberSaveable { mutableStateOf(VelaTab.Wallet) }

    /**
     * A tab tapped on a route pushed over the wallet (通讯录, 设置). The two
     * sections are reached by leaving the route; the other pushed route is
     * swapped in, so the stack never grows past wallet + one.
     *
     * `popBackStack(WALLET, …)` and not a bare `popBackStack()`: the bare one
     * pops whatever is on top, and a second tap during the exit fade — the
     * fading screen still takes taps — popped the WALLET itself and left an
     * empty NavHost. That is the blank app under the parallel-space badge
     * (device-found 2026-09-12); with predictive back moving the task to the
     * back rather than finishing, only a swipe-kill got out of it.
     */
    val selectFromPushed: (VelaTab) -> Unit = { tab ->
        when (tab) {
            VelaTab.Wallet, VelaTab.Explore -> {
                section = tab
                navController.popBackStack(VelaDestinations.WALLET, inclusive = false)
            }
            VelaTab.Contacts -> navController.swapOverWallet(VelaDestinations.CONTACTS)
            VelaTab.Settings -> navController.swapOverWallet(VelaDestinations.SETTINGS)
        }
    }

    // Credential Manager raises system UI, which needs an Activity — not the
    // application context the ViewModel was constructed with.
    LaunchedEffect(context) {
        onboarding.attach(context)
        // Spec 043: the send path signs with the same ceremony sign-in uses.
        application.container.passkeySigner = onboarding.signer()
    }

    /**
     * The route guard.
     *
     * `allowed_route` is the core's ruling about WHAT is allowed; when to move is
     * this host's call, and it moves only for the two settled routes. `loading`
     * is deliberately not navigated to: the launch animation is already covering
     * this frame, and bouncing to a spinner route and back would make a cold
     * start flicker through three screens.
     *
     * The developer routes are exempt. They are reached by an intent extra that
     * release builds never receive, and a guard that yanked the gallery back to
     * onboarding would make every fixture screen unreachable on a device with no
     * wallet — which is every device the gallery is useful on.
     */
    LaunchedEffect(session.allowedRoute, startDestination) {
        if (startDestination !in DEVELOPER_ROUTES) {
            when (session.allowedRoute) {
                SessionRoute.Wallet -> {
                    // The flow ended in a wallet, so the machine that built it
                    // is done — the other real end, beside the exit affordance.
                    onboarding.disposeCreate()
                    navController.navigateSingleTop(VelaDestinations.WALLET)
                }
                // NOT while a create is in flight.
                //
                // `Onboarding` is where somebody building a wallet already IS,
                // so this branch has nothing to correct for them — but it fires
                // again whenever this host re-enters composition (an activity
                // relaunch does that, and plugging in a USB security key
                // relaunches the activity). It then cleared the back stack down
                // to Welcome, taking the create screen and its live ceremony
                // with it: the person watched the app walk out of the flow they
                // were three keys into (device-found 2026-08-26).
                SessionRoute.Onboarding ->
                    if (onboarding.createView == null) {
                        // The next wallet opens on 钱包, as it did when the
                        // section lived (and died) with the wallet route.
                        section = VelaTab.Wallet
                        navController.navigateSingleTop(VelaDestinations.WELCOME)
                    }
                SessionRoute.Loading -> Unit
            }
        }
    }

    Box {
    // Spec 048: the identicon viewer, hosted once — every artwork drawn from an
    // address opens it (see IdenticonImage.tappable), so twelve screens do not
    // each carry a sheet of their own.
    var identiconViewer by remember { mutableStateOf<Pair<String, String?>?>(null) }
    CompositionLocalProvider(LocalIdenticonViewer provides { seed, name -> identiconViewer = seed to name }) {
        NavHost(navController = navController, startDestination = startDestination) {
            composable(VelaDestinations.WELCOME) {
                val welcome: WelcomeViewModel = viewModel()
                var showSignInMethods by rememberSaveable { mutableStateOf(false) }
                WelcomeScreen(
                    darkTheme = darkTheme,
                    signingIn = onboarding.loginView.busy,
                    onIntent = { intent ->
                        when (intent) {
                            // PUSHED, not swapped: `navigateSingleTop` clears the
                            // back stack down to and including the start
                            // destination, which is right for the session guard's
                            // jumps and wrong here — it left the create flow as the
                            // only entry, so its own back affordance had nothing to
                            // pop and did nothing at all (device-found 2026-08-25).
                            OnboardingIntent.CreateWallet ->
                                navController.push(VelaDestinations.CREATE)
                            // Signing in offers the same three authenticators
                            // creating does — the picker opens, and the chosen
                            // method runs the "who are you?" ceremony on that route.
                            OnboardingIntent.RecoverWallet -> showSignInMethods = true
                        }
                    },
                    onLongPressLogo = welcome::showSettings,
                )
                if (showSignInMethods) {
                    SignInMethodSheet(
                        onPick = { method ->
                            showSignInMethods = false
                            onboarding.beginSignIn(method)
                        },
                        onDismiss = { showSignInMethods = false },
                    )
                }
                if (welcome.settingsSheetVisible) {
                    ThemeSettingsSheet(
                        current = themePreference,
                        onSelect = onThemeSelected,
                        onDismiss = welcome::hideSettings,
                    )
                }
            }

            composable(VelaDestinations.CREATE) {
                CreateFlowScreen(
                    model = onboarding,
                    // Leaving the flow, which is a different event from the screen
                    // leaving composition — see the DisposableEffect in
                    // CreateFlowScreen. The machine holds drafted passkeys, so it is
                    // dropped HERE, where somebody actually said they were done.
                    onExit = {
                        onboarding.disposeCreate()
                        navController.popUnlessRoot()
                    },
                    onOpenPrivacy = { context.openUrl(PRIVACY_URL) },
                    onOpenTerms = { context.openUrl(TERMS_URL) },
                )
            }

            composable(VelaDestinations.IMPORT) {
                ImportPlaceholderScreen(
                    darkTheme = darkTheme,
                    onBack = { navController.popUnlessRoot() },
                )
            }

            composable(VelaDestinations.WALLET) {
                val strings = LocalVelaStrings.current
                // Fixture-driven still (spec 015 FR-005) apart from the two things
                // that identify the wallet — its address and its name, both now the
                // real ones. A home screen showing a fixture address after a real
                // create would be the app telling the person their money is
                // somewhere it is not; a fixture NAME over their own address and
                // identicon told them they were signed in as somebody else
                // (device-found 2026-08-26).
                // The chain-select sheet used to open from the header's network
                // pill; the pill is gone (founder call, 2026-08-26 — it cost the
                // name and address their width), and with it this screen's H8
                // state. The sheet keeps its fixtures for the gallery.
                val model = remember(strings, session.address, session.activeName) {
                    WalletFixtures.buildMobileState(WalletScreenState.H1, strings)
                        .withAddress(session.address).withName(session.activeName)
                }
                // Spec 021: Receive / Send / Activity / Assets, as pushed screens
                // inside this route. The system Back unwinds the flow stack before
                // it leaves the wallet — on a phone that is the most common way out
                // of a flow, and losing the whole wallet from four screens deep is
                // not what the gesture means.
                // The holdings are this person's own from spec 041; the activity
                // feed and the flows are still fixtures until phases 5 and 6.
                val wallet = application.container.wallet
                val balances by wallet.balances.collectAsStateWithLifecycle()
                val feed by wallet.feed.collectAsStateWithLifecycle()
                // Spec 047: the header's account switcher (the founder, 2026-09-12).
                var switcherOpen by remember { mutableStateOf(false) }
                val manageTokens by wallet.manageTokens.collectAsStateWithLifecycle()
                // The display currency the person chose, and the rate that makes it
                // showable. Without a rate the core leaves the figure in dollars.
                val currency by application.container.settings.currency.collectAsStateWithLifecycle()
                val networks by application.container.settings.networks.collectAsStateWithLifecycle()
                val chainNames = remember(networks.networks) {
                    networks.networks.associate { it.chain_id.toInt() to it.display_name }
                }
                LaunchedEffect(session.address) {
                    if (session.address.isEmpty()) return@LaunchedEffect
                    // ORDER MATTERS. The pool asks the network machine which
                    // endpoints a chain has, so a fetch dispatched before that
                    // machine has read storage finds no chains at all and settles
                    // with nothing — silently, and with no second chance, because
                    // nothing re-triggers when the list later arrives.
                    val settings = application.container.settings
                    settings.startNetworks()
                    settings.networks.first { it.loaded }
                    // The hero is a money figure, and which currency it is in is
                    // this machine's answer. Refreshed here rather than only on the
                    // settings screen, because somebody who never opens settings
                    // still has a display currency — and until this runs the core
                    // has no rate and the wallet shows dollars.
                    settings.refreshCurrency()
                    wallet.open(session.address)
                }

                // Spec 048: the 全部网络 pill and the SD1 class chips are shell render
            // state (as on the web); the feed machine is told the chain, the asset
            // and send lists narrow themselves; a sweep's pinned chain wins.
            var classFilter by rememberSaveable { mutableStateOf("all") }
            var chainFilter by rememberSaveable { mutableStateOf<Int?>(null) }
            var chainSheetOpen by remember { mutableStateOf(false) }
            // Spec 048: a token detail's 转账 — that token, chosen once the send machine lists it.
            var pendingSelectToken by remember { mutableStateOf<Pair<Int, String?>?>(null) }
            // Spec 048: a token detail's 收款 — that asset, applied once the receive flow has started the request machine.
            var pendingReceiveAsset by remember { mutableStateOf<ReceiveAssetPick?>(null) }
            val haptic = rememberVelaHaptic()
            val explorers = remember(networks.networks) { networks.networks.associate { it.chain_id.toInt() to it.explorer_url } }
            if (chainSheetOpen) {
                ChainFilterSheet(
                    title = strings.t(I18nKeys.Wallet.ALL_NETWORKS),
                    allLabel = strings.t(I18nKeys.Wallet.ALL_NETWORKS),
                    rows = networks.networks.map { ChainFilterRow(it.chain_id.toInt(), it.display_name, Marks.chainLogoUrl(it.chain_id.toInt()), WalletLive.badge(it.chain_id)) },
                    selected = chainFilter,
                    onPick = { id ->
                        chainFilter = id
                        wallet.filterChain(id)
                        haptic(VelaHaptic.Select)
                        chainSheetOpen = false
                    },
                    onDismiss = { chainSheetOpen = false },
                )
            }
            val flows = rememberFlowNavState()
            // Spec 048: a flow another route asked for (contacts: 转账 / 收款 / 群发转账)
            // opens once the wallet is back on top. Entered first, cleared after —
            // clearing the trigger first would cancel this very effect.
            val pendingFlow by application.container.pendingFlow.collectAsStateWithLifecycle()
            LaunchedEffect(pendingFlow) {
                val entry = pendingFlow ?: return@LaunchedEffect
                VelaLog.event("flows", "pending flow", "entry" to entry.name, "top" to flows.top?.name)
                flows.enter(entry)
                application.container.pendingFlow.value = null
            }
                val scope = rememberCoroutineScope()
                // Spec 047 FR-006, under the manifest's rule (no ACCESS_NETWORK_STATE:
                // "offline" is what the calls did, not what the radio claims): three
                // calls in a row that never reached a server, cleared by the first
                // answered one. Coming back refreshes what went stale meanwhile.
                val online by NetHealth.online.collectAsStateWithLifecycle()
                var wasOffline by remember { mutableStateOf(false) }
                LaunchedEffect(online) {
                    if (!online) wasOffline = true else if (wasOffline) { wasOffline = false; wallet.refresh() }
                }
                val payLink by application.container.pendingPayLink.collectAsStateWithLifecycle()
                LaunchedEffect(payLink) {
                    val uri = payLink ?: return@LaunchedEffect
                    // Consumed at the END: clearing the trigger here re-keys this
                    // effect to null and cancels the very coroutine doing the work
                    // (device-found 2026-09-12 — the link was "received" and then
                    // nothing, because the wait for the core's verdict was cancelled).
                    VelaLog.event("paylink", "received", "uri" to uri.toString().take(80))
                    val pay = PayLink.parse(uri.toString()) as? PayLink.Pay
                    val request = pay?.let { wallet.validatePayLink(it.to, it.chain, it.token, it.amount, it.sym, it.dec, it.net) }
                    if (request == null) {
                        VelaLog.event("paylink", "refused by the core", "uri" to uri.toString().take(80))
                        application.container.pendingPayLink.compareAndSet(uri, null)
                        return@LaunchedEffect
                    }
                    VelaLog.event("paylink", "validated", "chain" to request.chain_id, "amount" to request.amount)
                    application.container.pendingSendParams.value = SendOpenParams(
                        prefilled_recipient = request.recipient,
                        prefilled_chain_id = request.chain_id.toString(),
                        prefilled_token_address = request.token_address,
                        prefilled_amount_base = request.amount_base,
                        locked = true,
                    )
                    flows.enter(WalletFlowEntry.Send)
                    application.container.pendingPayLink.compareAndSet(uri, null)
                }
                var captureShare by remember { mutableStateOf<ShareCardModel?>(null) }
                captureShare?.let { card ->
                    ShareCardCapture(card) { bytes ->
                        captureShare = null
                        if (bytes == null) {
                            VelaLog.event("share", "render failed")
                        } else {
                            scope.launch {
                                val name = "vela-receive-${session.address.takeLast(6)}.png"
                                // Spec 048: 保存图片 saves — into the gallery, the way the web
                                // downloads a file; the share sheet is the way out when the
                                // gallery refuses.
                                val saved = withContext(Dispatchers.IO) { Gallery.savePng(context, name, bytes) }
                                if (saved != null) {
                                    android.widget.Toast.makeText(context, strings.t(I18nKeys.Flows.SAVED_BODY), android.widget.Toast.LENGTH_SHORT).show()
                                } else {
                                    application.container.documents?.share(name, "image/png", bytes)
                                }
                            }
                        }
                    }
                }
                if (switcherOpen) {
                    AccountSwitcherSheet(
                        sheet = WalletLive.accountSwitcher(
                            session.accounts.map { it.name to it.address },
                            session.activeIndex,
                            balances.switcher,
                            currency,
                            strings,
                        ),
                        onDismiss = { switcherOpen = false; wallet.switcherClosed() },
                        onSelect = { index ->
                            switcherOpen = false
                            wallet.switcherClosed()
                            haptic(VelaHaptic.Select)
                            application.container.session.switchAccount(index)
                        },
                        onPrimary = { switcherOpen = false; wallet.switcherClosed(); navController.push(VelaDestinations.CREATE) },
                        onSecondary = { switcherOpen = false; wallet.switcherClosed(); navController.push(VelaDestinations.WELCOME) },
                    )
                }
                // Spec 044: a page opened from outside 探索 (a deep link, the dev seam) shows itself.
                val browserOpenRequested by application.container.browser.openRequested.collectAsStateWithLifecycle()
                LaunchedEffect(browserOpenRequested) {
                    if (browserOpenRequested) {
                        section = VelaTab.Explore
                        application.container.browser.openRequested.value = false
                    }
                }
                // Back unwinds the flow stack first, then leaves 探索 for 钱包 —
                // Back out of a browser should land on the wallet, not on Welcome.
                BackHandler(enabled = flows.isOpen) { flows.back() }
                // Spec 044: a page asked for a signature — the sheet over whatever is
                // showing, read from the four machines; dismissing is the refusal.
                val signingController by application.container.signing.collectAsStateWithLifecycle()
                signingController?.let { controller ->
                    val signView by controller.sign.collectAsStateWithLifecycle()
                    val clearView by controller.clear.collectAsStateWithLifecycle()
                    val guardView by controller.guard.collectAsStateWithLifecycle()
                    val signFee by controller.fee.collectAsStateWithLifecycle()
                    val signSim by controller.sim.collectAsStateWithLifecycle()
                    val signRequest by controller.request.collectAsStateWithLifecycle()
                    val signChain = signRequest?.chainId ?: 0
                    val signCtx = app.getvela.wallet.feature.signing.SigningLive.Context(
                        strings = strings,
                        chainName = chainNames[signChain] ?: signChain.toString(),
                        chainDot = WalletLive.badge(signChain.toLong()),
                        nativeSymbol = networks.networks.firstOrNull { it.chain_id.toInt() == signChain }?.native_symbol ?: "ETH",
                        walletName = session.activeName,
                        walletAddress = session.address,
                        money = WalletLive.Money.of(currency),
                        origin = signRequest?.origin?.substringAfter("://")?.substringBefore('/'),
                    )
                    signRequest?.let { request ->
                        if (signView.surface != app.getvela.wallet.feature.signing.core.SignSurface.Hidden) {
                            val drawn = remember(strings) { SigningFixtures.build(SigningScreenState.CS1, strings) }
                            app.getvela.wallet.feature.signing.SigningSheet(
                                model = app.getvela.wallet.feature.signing.SigningLive.model(drawn, request, signView, clearView, guardView, signFee, signCtx, signSim),
                                // The swipe: a reject before the commitment point, a dismiss after — the core routes it.
                                onDismiss = { controller.swipeDismissed() },
                                onConfirm = { controller.approve() },
                                onChip = { id ->
                                    when (id) {
                                        "requested" -> controller.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Requested)
                                        "balance" -> controller.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Balance)
                                        "custom" -> controller.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Custom)
                                        "revoke" -> controller.guardPreset(app.getvela.wallet.feature.signing.core.GuardEditorMode.Revoke)
                                    }
                                },
                                onCustomAmount = { controller.guardCustomAmount(it) },
                            )
                        }
                    }
                }
                BackHandler(enabled = !flows.isOpen && section == VelaTab.Explore) {
                    section = VelaTab.Wallet
                }

                // Spec 043: the send is live. Its screens are drawn by the same
                // fixtures as before, but which screen is on and every figure on
                // it come from the send machine — the flow stack only marks that
                // the send is open.
                val send = application.container.send
                val sendView by send.send.collectAsStateWithLifecycle()
                LaunchedEffect(sendView.tokens.size, pendingSelectToken) {
                    val wanted = pendingSelectToken ?: return@LaunchedEffect
                    val token = sendView.tokens.firstOrNull { it.chain_id.toInt() == wanted.first && (it.token_address ?: "").equals(wanted.second ?: "", ignoreCase = true) }
                    if (token != null) {
                        pendingSelectToken = null
                        VelaLog.event("send", "token preselected", "symbol" to token.symbol)
                        send.selectToken(SendLive.tokenId(token))
                    }
                }
                val feeView by send.fee.collectAsStateWithLifecycle()
                val sendClosed by send.closed.collectAsStateWithLifecycle()
                val sendAlert by send.alert.collectAsStateWithLifecycle()
                val contactsBook by application.container.contacts.view.collectAsStateWithLifecycle()
                var feeSheetOpen by rememberSaveable { mutableStateOf(false) }
                val sweepPicking by send.sweepPicking.collectAsStateWithLifecycle()
                val mainActivity = LocalContext.current.let { ctx ->
                    generateSequence(ctx) { (it as? android.content.ContextWrapper)?.baseContext }.firstOrNull { it is MainActivity } as? MainActivity
                }
                val batchView by send.batch.collectAsStateWithLifecycle()
                val sendOpen = flows.top in SEND_STATES
                LaunchedEffect(sendOpen, session.address) {
                    if (sendOpen && session.address.isNotEmpty()) {
                        feeSheetOpen = false
                        val money = WalletLive.Money.of(currency)
                        // Spec 047 D8: a validated /pay link opens the send locked, once.
                        val linkParams = application.container.pendingSendParams.value?.also { application.container.pendingSendParams.value = null }
                        send.open(
                            params = linkParams ?: SendOpenParams(),
                            account = SendAccountRef(id = session.address, address = session.address, name = session.activeName),
                            display = SendDisplayContext(
                                code = money.code,
                                rate = currency.rate?.takeIf { currency.committed && it.isFinite() && it > 0.0 },
                                fiat_decimals = 2,
                            ),
                        )
                        // Spec 048: a group's 群发转账 arrives as split rows to seed right
                        // after the machine opens (the web seeds the same way).
                        application.container.pendingSplitSeed.value?.let { seed ->
                            application.container.pendingSplitSeed.value = null
                            send.seedSplit(seed)
                        }
                        // The picker lists this device's book, which only the
                        // contacts screen used to load (device-found, phase 6).
                        application.container.contacts.open(session.address)
                        // Spec 046 US3: the home's 扫码 enters here with the scanner on top.
                        if (flows.top == FlowState.S1) send.openScanner()
                    }
                }
                LaunchedEffect(sendClosed) {
                    if (sendClosed && flows.top in SEND_STATES) flows.close()
                }
                // The notification permission, asked at the first submit and never at
                // launch (research D6): a refusal degrades to the in-app receipt.
                val askNotifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
                LaunchedEffect(sendView.stage) {
                    if (sendView.stage == SendStage.Receipt && android.os.Build.VERSION.SDK_INT >= 33) {
                        val granted = androidx.core.content.ContextCompat.checkSelfPermission(
                            context, android.Manifest.permission.POST_NOTIFICATIONS,
                        ) == android.content.pm.PackageManager.PERMISSION_GRANTED
                        if (!granted) askNotifications.launch(android.Manifest.permission.POST_NOTIFICATIONS)
                    }
                }
                // A tapped notification opens the wallet on that row.
                val pendingReceipt by application.container.pendingReceipt.collectAsStateWithLifecycle()
                LaunchedEffect(pendingReceipt) {
                    pendingReceipt?.let { hash ->
                        application.container.pendingReceipt.value = null
                        flows.enter(WalletFlowEntry.TxDetail, hash)
                    }
                }
                sendAlert?.let { kind ->
                    SendAlertDialog(kind = kind, strings = strings, onDismiss = send::dismissAlert)
                }

                val flowState = flows.top
                if (flowState != null && flowState in SEND_STATES) {
                    val liveState = SendLive.flowState(sendView, feeSheetOpen)
                    // The system back key on a live send page is the header's back —
                    // the drawn stack's `flows.back()` closed the whole flow from
                    // any page, confirm included (device-found, phase 6).
                    BackHandler(enabled = sendOpen) {
                        when {
                            feeSheetOpen -> feeSheetOpen = false
                            // The scanner closes first; entered from home, the whole flow goes with it.
                            sendView.show_scanner -> {
                                send.closeScanner()
                                if (sendView.stage == SendStage.SelectToken && flowState == FlowState.S1) flows.close()
                            }
                            // Back out of the sweep pick first: the tick boxes go, the list stays.
                            sweepPicking && sendView.stage == SendStage.SelectToken -> send.cancelSweep()
                            sendView.stage == SendStage.SelectToken || sendView.stage == SendStage.Receipt -> flows.close()
                            else -> send.back()
                        }
                    }
                    val explorers = remember(networks.networks) {
                        networks.networks.associate { it.chain_id.toInt() to it.explorer_url }
                    }
                    val flowModel = remember(liveState, sendView, feeView, batchView, contactsBook, strings, currency, chainNames, explorers, session.address, sweepPicking, chainFilter, classFilter, Formats.current) {
                        val drawn = FlowFixtures.build(liveState, strings)
                        val ctx = SendLive.Context(
                            strings = strings,
                            chainNames = chainNames,
                            explorers = explorers,
                            money = WalletLive.Money.of(currency),
                            fromName = session.activeName,
                            fromAddress = session.address,
                        )
                        VelaLog.event("send.base", drawn.base::class.simpleName ?: "?", "state" to liveState.name, "top" to flowState.name)
                        val base = when (val base = drawn.base) {
                            is FlowBase.SendPick -> FlowBase.SendPick(SendLive.pick(base.model, sendView, ctx, sweepPicking, sendView.multi_chain_id ?: chainFilter, classFilter))
                            is FlowBase.SendForm -> FlowBase.SendForm(SendLive.form(base.model, sendView, feeView, ctx))
                            is FlowBase.SendConfirm -> FlowBase.SendConfirm(SendLive.confirm(base.model, sendView, ctx, feeView))
                            is FlowBase.SendReceipt -> FlowBase.SendReceipt(SendLive.receipt(base.model, sendView, ctx))
                            else -> base
                        }
                        val sheet = when (val sheet = drawn.sheet) {
                            is FlowSheet.FeeToken -> FlowSheet.FeeToken(SendLive.feeSheet(sheet.model, feeView, ctx))
                            is FlowSheet.ContactPick -> FlowSheet.ContactPick(SendLive.contactSheet(sheet.model, contactsBook))
                            is FlowSheet.BatchImport -> FlowSheet.BatchImport(SendLive.batchImport(sheet.model, batchView, sendView, ctx))
                            else -> sheet
                        }
                        drawn.copy(base = base, sheet = sheet)
                    }
                    val activity = context
                    FlowHost(
                        model = flowModel,
                        onBack = {
                            if (sendView.stage == SendStage.SelectToken || sendView.stage == SendStage.Receipt) {
                                flows.close()
                            } else {
                                send.back()
                            }
                        },
                        onOpen = { step, id -> VelaLog.event("flows", "open", "step" to step.name, "id" to id.take(24)); flows.push(step, id) },
                        onOpenUrl = { context.openUrl(it) },
                        onNavigate = { step ->
                            when (step) {
                                FlowStep.FeeToken -> feeSheetOpen = true
                                FlowStep.ContactPick -> send.openContactPicker()
                                FlowStep.Chains -> chainSheetOpen = true
                                else -> Unit
                            }
                        },
                        send = SendCallbacks(
                            onSelectToken = { index ->
                                val visible = SendLive.visibleTokens(sendView, sendView.multi_chain_id ?: chainFilter, classFilter)
                                sendView.tokens.getOrNull(visible.getOrNull(index) ?: -1)?.let { token ->
                                    val id = SendLive.tokenId(token)
                                    if (sweepPicking) send.toggleSweep(id) else send.selectToken(id)
                                }
                            },
                            onScanOpen = { send.openScanner() },
                            onFilter = { id -> classFilter = id; haptic(VelaHaptic.Select) },
                            onGroup = { index ->
                                VelaLog.event("send", "group seed", "index" to index, "groups" to contactsBook.groups.size)
                                contactsBook.groups.getOrNull(index)?.let { g ->
                                    send.seedSplit(g.members.mapIndexed { k, member -> SendRecipientDraft(id = "group-${g.id}-$k", address = member.address, amount = "", name = member.name) })
                                }
                            },
                            onRecipientPick = { index -> sendView.recipients.getOrNull(index)?.id?.let { send.openRowPicker(it) } },
                            scan = ScanCallbacks(
                                onDecoded = { text -> send.scanned(text) },
                                onClose = {
                                    send.closeScanner()
                                    if (sendView.stage == SendStage.SelectToken && flowState == FlowState.S1) flows.close()
                                },
                                requestPermission = { mainActivity?.requestCameraPermission() ?: false },
                                pickImage = { application.container.documents?.pick(listOf("image/*"))?.bytes },
                                permissionText = strings.t(I18nKeys.Flows.SCAN_PERMISSION_TEXT),
                                grantLabel = strings.t(I18nKeys.Flows.SCAN_GRANT),
                                noQrFound = strings.t(I18nKeys.Flows.SCAN_NO_QR),
                                cameraUnavailable = strings.t(I18nKeys.Flows.SCAN_CAMERA_UNAVAILABLE),
                                decodeFailed = strings.t(I18nKeys.Flows.SCAN_ERROR_IMAGE),
                            ),
                            onSelectAll = { send.selectAllValuable(sendView.tokens.map(SendLive::tokenId)) },
                            onPickCta = { if (sweepPicking) send.confirmSweep() else send.startSweep() },
                            onAmountChange = { send.setAmount(it) },
                            onRecipientChange = { send.setRecipient(it.trim()) },
                            onMax = { send.tapMax() },
                            onDenom = { send.toggleFiatInput() },
                            onContinue = { send.continueTapped() },
                            onConfirm = { send.slideConfirm() },
                            onFeeSelect = { index ->
                                haptic(VelaHaptic.Select)
                                feeView.options.getOrNull(index)?.let { send.chooseFeeToken(it.contract) }
                                feeSheetOpen = false
                            },
                            onContactSelect = { index ->
                                contactsBook.contacts.getOrNull(index)?.let { send.pickedAddress(it.address) }
                            },
                            onSheetDismissed = {
                                if (feeSheetOpen) feeSheetOpen = false
                                if (sendView.show_contact_picker) send.closeContactPicker()
                                if (sendView.show_batch_import) send.closeBatch()
                            },
                            // Spec 045 US3: the batch sheet's taps go to the batch machine.
                            onBatchUnit = { id -> send.batchUnit(if (id == "fiat") WireBatchUnit.Fiat else WireBatchUnit.Token) },
                            onBatchPaste = { text -> send.batchText(text) },
                            onBatchFile = { send.batchPickFile() },
                            onBatchTemplate = { send.batchTemplate() },
                            onBatchRate = { text -> send.batchRate(text) },
                            onBatchRateReset = { send.batchResetRate() },
                            onBatchApply = { send.batchApply() },
                            // Spec 045 US1: the split's rows, by index on screen → id in the core.
                            onAddRecipient = { send.enterSplit() },
                            onRecipientAction = { action ->
                                when (action) {
                                    RecipientAction.Add -> send.splitAdd()
                                    RecipientAction.Contacts -> send.openContactPicker()
                                    RecipientAction.Import -> send.openBatch()
                                }
                            },
                            onRemoveRecipient = { index -> sendView.recipients.getOrNull(index)?.let { send.splitRemove(it.id) } },
                            onRecipientAmount = { index, text -> sendView.recipients.getOrNull(index)?.let { send.splitAmount(it.id, text) } },
                            onRecipientAddress = { index, text -> sendView.recipients.getOrNull(index)?.let { send.splitAddress(it.id, text.trim()) } },
                            onReceiptCta = {
                                // The receipt's one button: Cancel while the ceremony is up (the
                                // core's checkpoint), Done or "keep running" otherwise.
                                if (sendView.tx_status == SendTxStatus.Signing) send.cancelSigning() else send.done()
                            },
                            onNoticeAction = {
                                when {
                                    sendView.treasury_bootstrap != null -> send.retryAfterBootstrap()
                                    sendView.tx_error != null -> send.retryAfterError()
                                    sendView.tx_status == SendTxStatus.Signing -> send.cancelSigning()
                                }
                            },
                            onNoticeSecondary = { send.dismissTreasurySheet() },
                            onExplorer = {
                                val ctx = SendLive.Context(strings, chainNames, explorers, WalletLive.Money.of(currency), session.activeName, session.address)
                                SendLive.explorerUrl(sendView, ctx)?.let { url ->
                                    runCatching {
                                        activity.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)))
                                    }
                                }
                            },
                        ),
                    )
                } else if (flowState != null) {
                    val request by wallet.request.collectAsStateWithLifecycle()
                    // The receive screen shows an ADDRESS. Every other fixture that
                    // leaks shows somebody the wrong information; a fixture address
                    // here sends their money to a stranger, permanently — so the
                    // watch and the request machine start the moment this opens.
                    // Keyed on being IN the receive flow, not on the step: a Start per
                    // step reset the asset a token detail's 收款 had just picked (spec 048).
                    val receiving = flowState in RECEIVE_STATES
                    LaunchedEffect(receiving, session.address) {
                        if (session.address.isNotEmpty() && receiving) {
                            wallet.openReceive(session.address, PAY_LINK_BASE)
                        }
                    }
                    // Spec 048: a token detail's 收款 — its asset, picked once the request
                    // machine has started (picking during the start was overwritten).
                    LaunchedEffect(request.gate_loading, pendingReceiveAsset) {
                        val t = pendingReceiveAsset ?: return@LaunchedEffect
                        if (request.gate_loading) return@LaunchedEffect
                        pendingReceiveAsset = null
                        wallet.assetPicked(t.chainId, t.tokenAddress, t.symbol, t.decimals, t.networkName)
                    }
                    val flowModel = remember(
                        flowState,
                        strings,
                        session.address,
                        networks,
                        request,
                        balances,
                        feed,
                        currency,
                        flows.selected,
                        manageTokens,
                        // Spec 049: a figure baked into this model follows the presets.
                        Formats.current,
                    ) {
                        FlowFixtures.build(flowState, strings).let { drawn ->
                            liveFlow(
                                drawn = drawn,
                                address = session.address,
                                name = session.activeName,
                                networks = networks,
                                request = request,
                                balances = balances,
                                feed = feed,
                                currency = currency,
                                chainNames = chainNames,
                                selected = flows.selected,
                                strings = strings,
                                manageTokens = manageTokens,
                                chainFilter = chainFilter,
                                explorers = explorers,
                            )
                        }
                    }
                    // The add-token sheet is the `manage_tokens` machine's: opening
                    // it loads the already-added rows (spec 043 T046).
                    LaunchedEffect(flowState) { if (flowState == FlowState.T3) wallet.openAddToken() }
                    FlowHost(
                        model = flowModel,
                        onBack = { flows.back() },
                        onOpen = { step, id -> VelaLog.event("flows", "open", "step" to step.name, "id" to id.take(24)); flows.push(step, id) },
                        onOpenUrl = { context.openUrl(it) },
                        onNavigate = { step -> if (step == FlowStep.Chains) chainSheetOpen = true else flows.push(step) },
                        selected = flows.selected,
                        // 删除记录 (spec 058): the feed tombstones the record and
                        // drops the row at once, so the detail has nothing left
                        // to show and steps back to the list it came from — what
                        // the web's `deleteSelectedTx` does. The CHAIN keeps the
                        // transaction; this is the wallet forgetting it.
                        onDeleteTx = flows.selected?.let { id ->
                            {
                                wallet.deleteActivity(id)
                                flows.back()
                            }
                        },
                        onReceiveNetwork = { index ->
                            networks.networks.getOrNull(index)?.let { row ->
                                pendingReceiveAsset = ReceiveAssetPick(row.chain_id.toInt(), null, row.native_symbol, 18, row.display_name)
                                flows.push(FlowStep.ReceiveQr)
                            }
                        },
                        onSendToken = { id ->
                            balances.tokens.firstOrNull { WalletLive.holdingId(it.chain_id, it.token_address) == id }?.let { t ->
                                pendingSelectToken = t.chain_id to t.token_address
                                flows.enter(WalletFlowEntry.Send)
                            }
                        },
                        onReceiveToken = { id ->
                            balances.tokens.firstOrNull { WalletLive.holdingId(it.chain_id, it.token_address) == id }?.let { t ->
                                pendingReceiveAsset = ReceiveAssetPick(t.chain_id, t.token_address, t.symbol, t.decimals, chainNames[t.chain_id] ?: t.name)
                                flows.enter(WalletFlowEntry.Receive)
                                flows.push(FlowStep.ReceiveQr, id)
                            }
                        },
                        onSaveImage = {
                            val drawn = (FlowFixtures.build(FlowState.R4, strings).base as? FlowBase.Share)?.model
                            if (drawn != null) captureShare = FlowLive.shareCard(drawn, session.address, session.activeName, request.asset.network_name, strings, request.asset.chain_id)
                        },
                        addToken = AddTokenCallbacks(
                            onInput = { wallet.addTokenInput(it.trim()) },
                            onTab = { id ->
                                if (id == "native") {
                                    application.container.pendingSettingsPage.value = SettingsPage.AddNetwork
                                    navController.push(VelaDestinations.SETTINGS)
                                }
                            },
                            onSubmit = { manageTokens.found.firstOrNull()?.let { wallet.addTokenSave(it.chain_id) } },
                        ),
                    )
                } else {
                    // Spec 022/029: 探索 is a real destination now rather than an
                    // inert chip — the same signed-in shell with a different body,
                    // which is why it is a SECTION of this route and not a route of
                    // its own. A browser tab is not somewhere a person should be
                    // able to deep-link into before they have a wallet.
                    val select: (VelaTab) -> Unit = { tab ->
                        when (tab) {
                            // 设置 has a screen now (spec 023), and the 退出登录 row
                            // inside it is where signing out lives. Until then the
                            // TAB itself signed you out — which meant tapping 设置
                            // to change your language logged you out instead. That
                            // regression must not come back through this `when`.
                            VelaTab.Settings -> navController.push(VelaDestinations.SETTINGS)
                            VelaTab.Explore -> section = VelaTab.Explore
                            VelaTab.Wallet -> section = VelaTab.Wallet
                            // 通讯录 refused to navigate for a good reason: it
                            // would have shown a signed-in person six strangers.
                            // That reason expired with spec 040 — the screen reads
                            // this device's own book now — so the tab works.
                            VelaTab.Contacts -> navController.push(VelaDestinations.CONTACTS)
                        }
                    }
                    if (section == VelaTab.Explore) {
                        // The browser shows the SIGNED-IN account, never the
                        // fixture one: a connection panel naming a stranger's
                        // account would be the wallet lying about what it just
                        // granted.
                        val exploreModel = remember(strings, session.address, session.activeName) {
                            ExploreFixtures.buildState(ExploreScreenState.E2, strings)
                                .withIdentity(session.activeName, session.address)
                        }
                        // Spec 044: the tab is a browser with a memory — the core's
                        // favourites, groups, tabs and recents; a live page where
                        // the demo page was drawn.
                        val browser = application.container.browser
                        LaunchedEffect(Unit) { browser.start() }
                        val engine by browser.current.collectAsStateWithLifecycle()
                        val engineState by (engine?.state ?: kotlinx.coroutines.flow.MutableStateFlow(app.getvela.wallet.feature.browser.core.EngineState())).collectAsStateWithLifecycle()
                        val exploreView by browser.explore.collectAsStateWithLifecycle()
                        val historyView by browser.history.collectAsStateWithLifecycle()
                        val permissions by browser.permissions.collectAsStateWithLifecycle()
                        val browserChain by browser.browserChain.collectAsStateWithLifecycle()
                        val identity = app.getvela.wallet.feature.browser.ExploreLive.Identity(
                            accountName = session.activeName,
                            accountAddress = session.address,
                            chainName = chainNames[browserChain] ?: browserChain.toString(),
                            chainDot = WalletLive.badge(browserChain.toLong()),
                        )
                        val liveModel = remember(exploreModel, exploreView, historyView, engineState, engine, strings, permissions, identity) {
                            app.getvela.wallet.feature.browser.ExploreLive.home(exploreModel, exploreView, historyView, engine?.let { engineState }, strings, permissions, identity)
                        }
                        val consentCard = permissions.consent?.let { c ->
                            app.getvela.wallet.feature.browser.ExploreLive.consent(exploreModel.connection, c, engine?.let { engineState }, strings, identity)
                        }
                        ExploreScreen(
                            model = liveModel,
                            // The drawn CS12 sheet belongs to the demo page, which a
                            // live route never draws (SC-007): a request's sheet is
                            // raised from the container's signing state instead.
                            signing = null,
                            onSelectTab = select,
                            page = engine?.let { e -> { app.getvela.wallet.feature.explore.components.BrowserPage(e) } },
                            initialView = if (engine != null) app.getvela.wallet.feature.explore.ExploreView.Browsing else null,
                            onOpenUrl = { browser.open(it) },
                            onClosePage = { browser.close() },
                            onPageBack = { browser.back() },
                            onPageForward = { browser.forward() },
                            live = app.getvela.wallet.feature.explore.ExploreCallbacks(
                                onOpenSite = { url -> browser.open(url) },
                                onTabOpen = { id -> browser.selectTab(id) },
                                onTabClose = { id -> browser.closeTab(id) },
                                onTabNew = { browser.newTab() },
                                onTabsCloseAll = { browser.closeAllTabs() },
                                onGroupToggle = { id, hidden ->
                                    when (id) {
                                        "favorites" -> browser.setSystemGroupHidden(app.getvela.wallet.feature.browser.core.ExploreSystemGroup.Favorites, hidden)
                                        "recent" -> browser.setSystemGroupHidden(app.getvela.wallet.feature.browser.core.ExploreSystemGroup.Recent, hidden)
                                        else -> browser.setGroupHidden(id, hidden)
                                    }
                                },
                                onGroupNew = { browser.createGroup(strings.t("explore.newGroup")) },
                                onSiteMenuPick = { id ->
                                    when (id) {
                                        "refresh" -> browser.reload()
                                        "favorite" -> browser.addFavorite()
                                        "close" -> browser.close()
                                    }
                                },
                                onBookmark = { browser.addFavorite() },
                                onRecentClear = { browser.clearRecent() },
                                onDisconnect = { browser.revoke() },
                                onConsent = { approved -> if (approved) browser.consentApproved() else browser.consentRejected() },
                            ),
                            consent = consentCard,
                        )
                    } else {
                        // The holdings, the feed and the currency are this device's
                        // own (spec 041); the fixture `model` only carries the
                        // labels the live builder cannot compute.
                        WalletScreen(
                            model = WalletLive.home(model, balances, feed, currency, strings, chainNames).let { home ->
                                // Spec 047 D9: no network at all is said on the hero, not guessed from a slow pool.
                                if (online) home else home.copy(balance = home.balance.copy(status = BalanceStatusModel(BalanceStatusKind.Warning, strings.t(I18nKeys.SettingsUi.NETWORK_OFFLINE))))
                            },
                            onSelectTab = select,
                            // The id says WHICH row was tapped. Without it every row
                            // opened the same detail, so this person's own POL showed
                            // somebody else's transaction.
                            onFlow = { entry, id -> flows.enter(entry, id) },
                            onSwitcher = {
                                wallet.switcherOpened(session.accounts.map { it.address })
                                switcherOpen = true
                            },
                            onToggleVisibility = { wallet.togglePrivacy(); haptic(VelaHaptic.Select) },
                            onStatusClick = {
                                // Spec 048: the status line's rescue — the RPC fix when a network failed, the per-chain detail otherwise.
                                application.container.pendingSettingsOverlay.value =
                                    if (application.container.pool.view.value.failed_chains.isNotEmpty()) SettingsOverlay.RpcFix else SettingsOverlay.BalanceDetail
                                navController.push(VelaDestinations.SETTINGS)
                            },
                        )
                    }
                }
            }

            // Spec 022: the browser on its own route, for review. The in-app entry
            // is the 探索 tab inside WALLET; this is the `vela.startDestination`
            // door, which is why it is in DEVELOPER_ROUTES and exempt from the
            // route guard.
            composable(VelaDestinations.EXPLORE) {
                val strings = LocalVelaStrings.current
                val model = remember(strings) {
                    ExploreFixtures.buildState(ExploreScreenState.E2, strings)
                }
                val signing = remember(strings) {
                    SigningFixtures.build(SigningScreenState.CS12, strings)
                }
                ExploreScreen(model = model, signing = signing)
            }

            composable(VelaDestinations.GALLERY) {
                GalleryScreen(systemDarkTheme = darkTheme)
            }

            composable(VelaDestinations.CONTACTS) {
                val strings = LocalVelaStrings.current
                val contacts = application.container.contacts
                val book by contacts.view.collectAsStateWithLifecycle()
                var menuOpen by rememberSaveable { mutableStateOf(false) }
                var query by rememberSaveable { mutableStateOf("") }
                // Spec 045 US5/US6: the form (null = closed, false = add, true = edit),
                // its two fields, the open group, and the two pickers.
                var formEdit by rememberSaveable { mutableStateOf<Boolean?>(null) }
                var formName by rememberSaveable { mutableStateOf("") }
                var formAddress by rememberSaveable { mutableStateOf("") }
                var openGroup by rememberSaveable { mutableStateOf<String?>(null) }
                var memberPicker by rememberSaveable { mutableStateOf(false) }
                var groupPicker by rememberSaveable { mutableStateOf(false) }
                var openContact by rememberSaveable { mutableStateOf<String?>(null) }

                // Read the book for THIS account: the core keys history-derived
                // entries by whose wallet they belong to.
                LaunchedEffect(session.address) {
                    contacts.open(session.address.takeIf { it.isNotEmpty() })
                }

                // The labels — titles, empty-state copy, the tab bar — come from
                // the fixture builder, which has already resolved them through the
                // i18n engine. The PEOPLE come from the core.
                val labels = remember(strings, menuOpen) {
                    ContactsFixtures.buildMobileState(
                        if (menuOpen) ContactsScreenState.C5 else ContactsScreenState.C1,
                        strings,
                    )
                }
                // The NO-ACTIVITY detail variant, not C2. C2 is Alice-the-fixture
                // and she has two transactions; a contact saved a minute ago has
                // none, and until spec 041 there is no local store to ask.
                val detailLabels = remember(strings) {
                    ContactsFixtures.contactDetailNoActivity(strings)
                }
                // C3 is the empty book: the only fixture state that carries the
                // "no contacts yet" copy and its two calls to action.
                val emptyState = remember(strings) {
                    ContactsFixtures.buildMobileState(ContactsScreenState.C3, strings).empty
                }

                var confirmingDelete by rememberSaveable { mutableStateOf(false) }
                // Spec 048: the doors the contacts machine had no drawing for on the phone.
                var contactQr by remember { mutableStateOf<String?>(null) }
                var groupEdit by remember { mutableStateOf<Pair<String?, String>?>(null) }
                var groupMenu by remember { mutableStateOf(false) }
                var confirmingGroupDelete by remember { mutableStateOf(false) }
                var exportChoice by remember { mutableStateOf(false) }
                val haptic = rememberVelaHaptic()
                val openSendTo = { address: String ->
                    VelaLog.event("contacts", "open send to", "address" to address.take(10))
                    application.container.pendingSendParams.value = SendOpenParams(prefilled_recipient = address)
                    application.container.pendingFlow.value = WalletFlowEntry.Send
                    navController.popBackStack(VelaDestinations.WALLET, inclusive = false)
                }

                // ONE lookup, and everything below reads it. The detail page, the
                // confirm sheet's wording and the delete itself all take the same
                // `selected` — the desktop sibling shipped a page that displayed
                // contact A while its delete acted on contact B, because the target
                // was looked up twice.
                val selected = openContact?.let { address ->
                    book.contacts.firstOrNull { it.address == address }
                }
                val listModel = ContactsLive.home(labels, book, query, emptyState)
                val model = if (selected != null) {
                    listModel.copy(
                        detail = ContactsLive.detail(
                            fallback = detailLabels,
                            contact = selected,
                            view = book,
                            // The activity this device holds, filtered to this
                            // person by the page itself.
                            feed = application.container.wallet.feed.value,
                            strings = strings,
                        ),
                        deleteConfirm = if (confirmingDelete) {
                            // Named after the contact on screen: a confirmation that
                            // says "delete Alice?" while removing Bob is worse than
                            // no confirmation at all.
                            ContactsFixtures.deleteConfirm(
                                strings,
                                ContactsLive.displayName(selected),
                            )
                        } else {
                            null
                        },
                    )
                } else {
                    listModel
                }

                // Spec 045 US7: opening a contact asks the core about the address —
                // the parallel space's chain, Gnosis, is where this wallet pays.
                LaunchedEffect(openContact) {
                    openContact?.let { contacts.inspect(chainId = 100, address = it) }
                }
                val group = openGroup?.let { id -> book.groups.firstOrNull { it.id == id } }
                val groupLabels = remember(strings) { ContactsFixtures.groupDetail(strings) }
                val menuCancel = remember(strings) { ContactsFixtures.addMenu(strings).cancel }
                val formLabels = remember(strings, formEdit) { formEdit?.let { ContactsFixtures.contactForm(strings, edit = it) } }
                val shown = model.copy(
                    groupDetail = if (selected == null && group != null) ContactsLive.groupDetail(groupLabels, group) else model.groupDetail,
                    form = formLabels?.let { ContactsLive.form(it, edit = formEdit == true, name = formName, address = formAddress) },
                    notice = ContactsLive.importNotice(book, strings, close = strings.t(I18nKeys.Flows.CLOSE)),
                    menu = when {
                        // The group's member picker: the whole book, ticked where it is
                        // in the group; a tap adds (AddGroupMembers) or removes (RemoveGroupMember).
                        memberPicker && group != null -> ActionMenuModel(
                            items = book.contacts.map { c ->
                                val member = group.members.any { it.address == c.address }
                                MenuItemModel(id = "contacts.member.toggle:" + c.address, icon = ContactsIcon.AddContact, label = (if (member) "✓ " else "") + ContactsLive.displayName(c))
                            },
                            cancel = menuCancel,
                        )
                        // The contact's groups: every group, ticked when it holds this person.
                        groupPicker && selected != null -> ActionMenuModel(
                            items = book.groups.map { g ->
                                val member = g.members.any { it.address == selected.address }
                                MenuItemModel(id = "contacts.group.toggle:" + g.id, icon = ContactsIcon.MoveGroup, label = if (member) "✓ " + g.name else g.name)
                            },
                            cancel = menuCancel,
                        )
                        else -> model.menu
                    },
                )
                BackHandler(enabled = openContact != null || openGroup != null || formEdit != null) {
                    when {
                        formEdit != null -> formEdit = null
                        openContact != null -> {
                            confirmingDelete = false
                            openContact = null
                        }
                        else -> openGroup = null
                    }
                }

                if (exportChoice) {
                    ExportFormatSheet(
                        title = strings.t(I18nKeys.Contacts.EXPORT_TITLE),
                        cancelLabel = ContactsFixtures.addMenu(strings).cancel,
                        onPick = { format ->
                            exportChoice = false
                            contacts.exportBook(format = if (format == "csv") ContactFileFormat.Csv else ContactFileFormat.Json)
                        },
                        onDismiss = { exportChoice = false },
                    )
                }
                contactQr?.let { address ->
                    ContactQrSheet(
                        name = book.contacts.firstOrNull { it.address == address }?.name ?: address,
                        address = address,
                        copyLabel = strings.t(I18nKeys.Flows.COPY_ADDRESS),
                        copiedLabel = strings.t(I18nKeys.Flow.COPIED),
                        closeLabel = strings.t(I18nKeys.Flows.DONE),
                        onDismiss = { contactQr = null },
                    )
                }
                groupEdit?.let { (id, initial) ->
                    GroupEditSheet(
                        title = strings.t(if (id == null) I18nKeys.Contacts.GROUP_NEW else I18nKeys.Contacts.GROUP_RENAME),
                        initialName = initial,
                        nameLabel = strings.t(I18nKeys.Contacts.GROUP_EDIT),
                        saveLabel = strings.t(I18nKeys.Flows.DONE),
                        cancelLabel = ContactsFixtures.addMenu(strings).cancel,
                        onSave = { name ->
                            contacts.saveGroup(ContactGroupInput(id = id, name = name))
                            haptic(VelaHaptic.Select)
                            groupEdit = null
                        },
                        onDismiss = { groupEdit = null },
                    )
                }
                if (groupMenu) {
                    GroupMenuSheet(
                        title = group?.name ?: "",
                        renameLabel = strings.t(I18nKeys.Contacts.GROUP_RENAME),
                        deleteLabel = strings.t(I18nKeys.Contacts.GROUP_DELETE),
                        cancelLabel = ContactsFixtures.addMenu(strings).cancel,
                        onRename = { groupMenu = false; group?.let { groupEdit = it.id to it.name } },
                        onDelete = { groupMenu = false; confirmingGroupDelete = true },
                        onDismiss = { groupMenu = false },
                    )
                }
                if (confirmingGroupDelete) {
                    GroupDeleteConfirmSheet(
                        title = strings.t(I18nKeys.Contacts.GROUP_DELETE) + (group?.let { " · " + it.name } ?: ""),
                        confirmLabel = strings.t(I18nKeys.Contacts.GROUP_DELETE),
                        cancelLabel = ContactsFixtures.addMenu(strings).cancel,
                        onConfirm = {
                            group?.let { contacts.deleteGroup(it.id) }
                            confirmingGroupDelete = false
                            openGroup = null
                        },
                        onDismiss = { confirmingGroupDelete = false },
                    )
                }
                ContactsRoute(
                    model = shown,
                    actions = ContactsActions(
                        onAction = { id ->
                        VelaLog.event("contacts", "action", "id" to id)
                            when (id) {
                                "contacts.addContact" -> menuOpen = true
                                // Spec 045 US5: the form — add from the menu, edit from the detail.
                                "contacts.addTitle" -> {
                                    menuOpen = false
                                    formName = ""
                                    formAddress = ""
                                    formEdit = false
                                }
                                "contacts.edit" -> selected?.let { contact ->
                                    formName = contact.name ?: contact.resolved_name ?: ""
                                    formAddress = contact.address
                                    formEdit = true
                                }
                                "contacts.form.save" -> {
                                    contacts.save(ContactSaveInput(address = formAddress.trim(), name = formName.trim()))
                                    formEdit = null
                                }
                                "contacts.form.cancel" -> formEdit = null
                                "contacts.favourite" -> selected?.let { haptic(VelaHaptic.Select); contacts.toggleFavourite(it.address) }
                                // Spec 045 US6: the book travels through the picker and the share sheet.
                                "contacts.importFile" -> {
                                    menuOpen = false
                                    contacts.importBook()
                                }
                                "contacts.exportTitle" -> {
                                    menuOpen = false
                                    exportChoice = true
                                }
                                "contacts.notice.close" -> contacts.acknowledgeImport()
                                // Groups, both ways.
                                "contacts.addMember" -> memberPicker = true
                                "contacts.moveGroup", "contacts.sectionGroups" -> groupPicker = true
                                "contacts.deleteContact" -> confirmingDelete = true
                                "contacts.delete" -> selected?.let { contact ->
                                    contacts.delete(contact.address)
                                    confirmingDelete = false
                                    openContact = null
                                }
                                "contacts.copyAddress" -> selected?.let { contact ->
                                if (Clipboard.copy(context, "address", contact.address)) haptic(VelaHaptic.Select)
                            }
                            "contacts.action.Send" -> selected?.let { contact -> openSendTo(contact.address) }
                            "contacts.action.Receive" -> {
                                application.container.pendingFlow.value = WalletFlowEntry.Receive
                                navController.popBackStack(VelaDestinations.WALLET, inclusive = false)
                            }
                            "contacts.action.Qr" -> contactQr = selected?.address
                            "contacts.batchSend" -> group?.let { g ->
                                val members = g.members
                                if (members.size == 1) {
                                    openSendTo(members.first().address)
                                } else {
                                    application.container.pendingSplitSeed.value = members.mapIndexed { i, member ->
                                        SendRecipientDraft(
                                            id = "group-${g.id}-$i",
                                            address = member.address,
                                            amount = "",
                                            name = member.name,
                                        )
                                    }
                                    application.container.pendingSendParams.value = SendOpenParams(preselected_multi = "split")
                                    application.container.pendingFlow.value = WalletFlowEntry.Send
                                    navController.popBackStack(VelaDestinations.WALLET, inclusive = false)
                                }
                            }
                            "contacts.manage" -> groupEdit = null to ""
                            "history.filterAll" -> {
                                // The detail already lists every row with this person; 全部 opens the history itself.
                                application.container.pendingFlow.value = WalletFlowEntry.Activity
                                navController.popBackStack(VelaDestinations.WALLET, inclusive = false)
                            }
                            "contacts.groupMenu" -> groupMenu = true
                            "contacts.back" -> {
                                    confirmingDelete = false
                                    openContact = null
                                }
                                "contacts.searchClear" -> query = ""
                                else -> when {
                                    id.startsWith("contacts.member.toggle:") -> openGroup?.let { gid ->
                                        val address = id.removePrefix("contacts.member.toggle:")
                                        val member = group?.members?.any { it.address == address } == true
                                        if (member) contacts.removeGroupMember(gid, address) else contacts.addGroupMembers(gid, listOf(address))
                                        memberPicker = false
                                    }
                                    id.startsWith("contacts.swipeSend:") -> openSendTo(id.removePrefix("contacts.swipeSend:"))
                                    id.startsWith("contacts.swipeDelete:") -> {
                                        openContact = id.removePrefix("contacts.swipeDelete:")
                                        confirmingDelete = true
                                    }
                                    id.startsWith("contacts.group.toggle:") -> selected?.let { contact ->
                                        val gid = id.removePrefix("contacts.group.toggle:")
                                        val current = book.groups.filter { g -> g.members.any { it.address == contact.address } }.map { it.id }
                                        contacts.setContactGroups(contact.address, if (gid in current) current - gid else current + gid)
                                        groupPicker = false
                                    }
                                    else -> Unit
                                }
                            }
                        },
                        onContact = { contact -> openContact = contact.addressFull },
                        onGroup = { row -> openGroup = book.groups.firstOrNull { it.name == row.name }?.id },
                        onFormName = { formName = it },
                        onFormAddress = { formAddress = it },
                        onQueryChange = { typed -> query = typed },
                        onDismissMenu = {
                            menuOpen = false
                            confirmingDelete = false
                            memberPicker = false
                            groupPicker = false
                        },
                        onTab = selectFromPushed,
                    ),
                )
            }

            composable(VelaDestinations.CONTACTS_GALLERY) {
                ContactsGalleryScreen(systemDarkTheme = darkTheme)
            }

            composable(VelaDestinations.SETTINGS) {
                val strings = LocalVelaStrings.current
                // Fixture-driven still (spec 023 scope) apart from the two things
                // that identify the account — its name and address, both the real
                // ones. A fixture name over a real address would tell somebody they
                // are signed in as a stranger (spec 019's finding).
                val model = remember(strings, session.address, session.activeName) {
                    SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
                        .withIdentity(
                            name = session.activeName,
                            address = session.address,
                            display = shortenAddress(session.address),
                        )
                }
                // The display currency, the networks, the service endpoints and the
                // provider keys are the person's own. What is still the ST1 fixture
                // is everything that needs a network to be true: RPC health and
                // latency pills, the storage figures, the relayer panel. Those wait
                // for spec 041 and render their unknown state until then.
                val settings = application.container.settings
                val currency by settings.currency.collectAsStateWithLifecycle()
                val networks by settings.networks.collectAsStateWithLifecycle()
                // Spec 047 US1: the rows read the device — preferences, the pool,
                // the session, the store's own keys, the relay's treasury.
                val scope = rememberCoroutineScope()
                val prefs by application.container.preferences.view.collectAsStateWithLifecycle()
                val poolView by application.container.pool.view.collectAsStateWithLifecycle()
                // Spec 048: the network whose detail page is open (its overrides are written for it).
                var openNetworkId by rememberSaveable { mutableStateOf<String?>(null) }
                val settingsHaptic = rememberVelaHaptic()
                var requestedPage by rememberSaveable { mutableStateOf<SettingsPage?>(null) }
                val sessionView = session
                val i18nState by application.container.i18nRuntime.state.collectAsStateWithLifecycle()
                var storageReport by remember { mutableStateOf<DeviceStorage.Report?>(null) }
                var storageTick by remember { mutableStateOf(0) }
                var treasury by remember { mutableStateOf<SendTreasuryStatus?>(null) }
                var eraseFailed by remember { mutableStateOf<List<String>?>(null) }
                val chainNamesNow = remember(networks.networks) { networks.networks.associate { it.chain_id.toInt() to it.display_name } }
                LaunchedEffect(Unit) {
                    settings.refreshCurrency()
                    settings.startNetworks()
                }
                LaunchedEffect(storageTick) { storageReport = DeviceStorage.measure(VelaStore(context)) }
                LaunchedEffect(Unit) {
                    treasury = (application.container.relay.probeTreasury(100) as? SendTreasuryProbe.LowFloat)?.status
                }
                val liveModel = run {
                    var m = SettingsLive.withWizard(
                        SettingsLive.withNetworks(SettingsLive.withCurrency(model, currency), networks, strings),
                        networks,
                        strings,
                    )
                    m = SettingsLive.withPreferences(
                        m, prefs, i18nState.language, strings,
                        theme = when (themePreference) { ThemePreference.Light -> "light"; ThemePreference.Dark -> "dark"; else -> "auto" },
                    )
                    storageReport?.let { m = SettingsLive.withStorage(m, it, strings) }
                    m = SettingsLive.withAbout(m, BuildConfig.VERSION_NAME, BuildConfig.GIT_COMMIT, networks.networks.size, strings)
                    m = SettingsLive.withFeedback(
                        m, BuildConfig.VERSION_NAME, BuildConfig.GIT_COMMIT, "Android ${android.os.Build.VERSION.RELEASE}", i18nState.language,
                        poolView.failed_chains.map { chainNamesNow[it] ?: it.toString() }, VelaLog.recentFailures(), strings,
                    )
                    m = SettingsLive.withRelayer(m, chainNamesNow[100] ?: "Gnosis", 100, "xDAI", treasury, strings)
                    m = SettingsLive.withBanner(m, poolView.failed_chains, chainNamesNow, strings)
                    m = SettingsLive.withAccounts(m, sessionView.accounts.map { it.name to it.address }, sessionView.activeIndex, strings)
                    // Spec 048: the network detail is THIS network's, not the fixture's.
                    openNetworkId?.let { id ->
                        networks.networks.firstOrNull { it.id == id }?.let { row ->
                            m = m.copy(networkDetail = SettingsLive.networkDetail(m.networkDetail, row, strings))
                        }
                    }
                    // Spec 048: a page another route asked for (the add-token 原生代币 tab). Kept
                    // in state: the flow is consumed once, and the page must hold across the
                    // recompositions that follow (a key that flips back re-initialised the screen).
                    application.container.pendingSettingsPage.value?.let { requested ->
                        application.container.pendingSettingsPage.value = null
                        VelaLog.event("settings", "page requested", "page" to requested.name)
                        requestedPage = requested
                    }
                    requestedPage?.let { m = m.copy(page = it) }
                    // Spec 048: the home's status line asked for a rescue sheet.
                    application.container.pendingSettingsOverlay.value?.let { requested ->
                        application.container.pendingSettingsOverlay.value = null
                        m = m.copy(overlay = requested)
                    }
                    // A partial wipe names what stayed, in the sheet itself (028's rule: the person stays signed in).
                    eraseFailed?.let { left -> m = m.copy(eraseSheet = m.eraseSheet.copy(body = m.eraseSheet.body + "\n\n" + left.joinToString(", "))) }
                    m
                }
                val feedbackUrl = remember(liveModel.feedback.previewLines) {
                    "https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=" +
                        java.net.URLEncoder.encode("[android] ", "UTF-8") +
                        "&body=" + java.net.URLEncoder.encode(liveModel.feedback.previewLines.joinToString("\n"), "UTF-8")
                }
                SettingsRoute(
                    model = liveModel,
                    actions = SettingsActions(
                        onSegment = { group, id ->
                            when (group) {
                                "theme" -> onThemeSelected(when (id) { "light" -> ThemePreference.Light; "dark" -> ThemePreference.Dark; else -> ThemePreference.Auto })
                                "avatar" -> application.container.preferences.setAvatarStyle(id)
                            }
                        },
                        onTextScale = { index -> TextScaleLevel.entries.getOrNull(index)?.let { application.container.preferences.setTextScale(it) } },
                        onStorageClear = { itemId -> scope.launch { DeviceStorage.clear(VelaStore(context), itemId); storageTick++ } },
                        onClearCaches = { scope.launch { DeviceStorage.clearCaches(VelaStore(context)); application.container.wallet.refresh(); storageTick++ } },
                        onErase = {
                            scope.launch {
                                // The keep-list: the account records and the pending-upload
                                // ledger, which sign-out's own path owns (028's one exception).
                                val left = DeviceStorage.erase(VelaStore(context), keep = setOf("vela.accounts", "vela.activeAccountIndex", "vela.pendingUploads"))
                                if (left.isEmpty()) {
                                    eraseFailed = null
                                    application.container.session.signOut()
                                    application.container.session.signOutConfirmed()
                                } else {
                                    eraseFailed = left
                                }
                            }
                        },
                        onFeedbackSend = { text ->
                            // Spec 048: what was typed leads the report; the device lines follow.
                            val body = listOf(text.trim(), liveModel.feedback.previewLines.joinToString("\n")).filter { it.isNotBlank() }.joinToString("\n\n")
                            val url = "https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=" +
                                java.net.URLEncoder.encode("[android] ", "UTF-8") + "&body=" + java.net.URLEncoder.encode(body, "UTF-8")
                            VelaLog.event("feedback", "send", "typed" to text.length)
                            runCatching { context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url))) }
                        },
                        onOverrideEdited = { chainId, field, value ->
                            settings.editOverride(chainId, if (field == "explorer") NetOverrideField.Explorer else NetOverrideField.Rpc, value)
                        },
                        onOverrideCommitted = { chainId -> settings.commitOverride(chainId) },
                        onCustomRpc = { settings.editCustomRpc(it) },
                        onRecheckNetwork = { liveModel.addNetwork.query.toLongOrNull()?.let { settings.addNetworkByChainId(it) } },
                        onProviderTest = { id -> NetProviderId.entries.firstOrNull { it.name.equals(id, ignoreCase = true) }?.let { settings.testProvider(it) } },
                        onFeedbackGithub = { runCatching { context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse("https://github.com/mondaylabsltd/vela-wallet/issues"))) } },
                        onOpenLink = { value ->
                            val url = if (value.startsWith("http")) value else "https://$value"
                            runCatching { context.startActivity(android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url))) }
                        },
                        onRelayerRetry = { scope.launch { treasury = (application.container.relay.probeTreasury(100) as? SendTreasuryProbe.LowFloat)?.status } },
                        onAccountSelect = { index -> settingsHaptic(VelaHaptic.Select); application.container.session.switchAccount(index) },
                        onAccountPrimary = { navController.push(VelaDestinations.CREATE) },
                        onAccountSecondary = { navController.push(VelaDestinations.WELCOME) },
                        onSelectTab = selectFromPushed,
                        // The way out of a signed-in wallet, on the row a person
                        // would look for it.
                        onSignOut = { application.container.session.signOut() },
                        onOpenContacts = { navController.push(VelaDestinations.CONTACTS) },
                        onSheetSelect = { sheet, id ->
                            val prefsStore = application.container.preferences
                            when (sheet) {
                                SettingsOverlay.Currency -> settings.chooseCurrency(id)
                                SettingsOverlay.Language -> {
                                    prefsStore.setLanguage(id)
                                    application.container.applyLanguage(id)
                                }
                                // Spec 049: the row id IS the wire key (the web's `formatSheet`).
                                SettingsOverlay.NumberFormat -> prefsStore.setNumberFormat(NumberFormatKey.of(id))
                                SettingsOverlay.DateFormat -> prefsStore.setDateFormat(DateFormatKey.of(id))
                                SettingsOverlay.TimeFormat -> prefsStore.setTimeFormat(TimeFormatKey.of(id))
                                else -> Unit
                            }
                        },
                        // The field ids are the core's own enum names, put there by
                        // SettingsLive. The screen reports "this box changed"; which
                        // machine event that is stays here, so a new editable field
                        // is a line in one place rather than a callback per box.
                        onFieldEdited = { fieldId, value ->
                            endpointField(fieldId)?.let { settings.editEndpoint(it, value) }
                            providerId(fieldId)?.let { settings.editProviderKey(it, value) }
                        },
                        onFieldCommitted = { fieldId ->
                            endpointField(fieldId)?.let { settings.commitEndpoint(it) }
                            providerId(fieldId)?.let { settings.commitProviderKey(it) }
                        },
                        onRemoveNetwork = { id -> settings.deleteNetwork(id) },
                        // The row carries a display id; the machine speaks chain
                        // ids. The network list already holds both, so the lookup
                        // is a read rather than a second identity to keep in sync.
                        onSearchNetwork = { query -> settings.searchNetworks(query) },
                        onConfirmAddNetwork = { settings.confirmAddNetwork() },
                        onPickNetwork = { chainId ->
                            settingsHaptic(VelaHaptic.Select)
                            chainId.toLongOrNull()?.let { settings.selectChain(it) }
                        },
                        onOpenNetwork = { id ->
                            openNetworkId = id
                            settings.networks.value.networks
                                .firstOrNull { it.id == id }
                                ?.let { settings.expandOverride(it.chain_id) }
                        },
                        onResetEndpoints = { settings.resetEndpoints() },
                    ),
                )
            }

            composable(VelaDestinations.SETTINGS_GALLERY) {
                SettingsGalleryScreen(
                    systemDarkTheme = settingsDark ?: darkTheme,
                    initialState = settingsState,
                )
            }

            composable(VelaDestinations.FLOWS_GALLERY) {
                FlowGalleryScreen(systemDarkTheme = darkTheme, initialState = startFlowState)
            }
        }
    }
    identiconViewer?.let { (seed, name) ->
        IdenticonViewerSheet(address = seed, name = name, onDismiss = { identiconViewer = null })
    }

    // Hosted OUTSIDE the NavHost, deliberately. A prompt can be raised by either
    // machine, and the login machine runs while Welcome is on screen — so a
    // sheet nested in one route's composable would vanish the moment the route
    // guard moved, taking the question with it and leaving the core waiting for
    // an answer nobody can give.
    // Spec 047 D10: the last run's crash, or a core fault, raised once.
    CrashSheet(strings = LocalVelaStrings.current, version = BuildConfig.VERSION_NAME)
    onboarding.pending?.let { prompt ->
        FlowSheet(
            kind = prompt.kind,
            confirmable = prompt.confirmable,
            onAnswer = onboarding::answerPrompt,
        )
    }

    // The app-owned CTAP-over-USB path's own ceremony dialogs — the PIN, the
    // which-wallet picker and the touch prompt a system passkey sheet would
    // otherwise draw. Hosted OUTSIDE the NavHost for the same reason the flow
    // sheet is: the login machine can raise them while Welcome is on screen.
    onboarding.pendingPin?.let { pin ->
        UsbPinDialog(
            product = pin.product,
            retries = pin.retries,
            isRetry = pin.isRetry,
            onSubmit = onboarding::answerPin,
        )
    }
    onboarding.pendingWalletPick?.let { pick ->
        UsbWalletPicker(
            choices = pick.choices,
            onPick = onboarding::answerWalletPick,
        )
    }
    onboarding.usbTouchWaiting?.let { touch ->
        UsbTouchIndicator(kind = touch.kind, product = touch.product)
    }
    onboarding.cableQr?.let { payload ->
        CableQrSheet(payload = payload)
    }
    // The scan method's "Location needs to be on" explainer (API ≤30) — ABOVE
    // the QR sheet, since the ceremony that raised it is the one showing the QR.
    onboarding.pendingLocationAsk?.let {
        LocationAskSheet(onAnswer = onboarding::answerLocationAsk)
    }
    // The USB method's "insert your key" waiter — it dismisses itself the
    // moment a key enumerates; closing it cancels the ceremony.
    onboarding.pendingInsertKey?.let { ask ->
        InsertKeySheet(
            otgLooksOff = ask.otgLooksOff,
            onCancel = onboarding::cancelInsertKey,
        )
    }

    // The way back out of a signed-in wallet.
    //
    // Rendered from `session.signOut`, which is non-null only after the machine
    // has ASKED STORAGE whether any public key is still unconfirmed — so the
    // warning inside is an answer rather than this screen's guess, and the sheet
    // cannot open before there is one.
    session.signOut?.let { sheet ->
        SignOutSheet(
            pendingUploadWarning = sheet.pendingUploadWarning,
            onConfirm = {
                ParallelSpaceHook.leave()
                application.container.session.signOutConfirmed()
            },
            onDismiss = { application.container.session.signOutDismissed() },
        )
    }

    if (onboarding.endpointSheetOpen) {
        EndpointSheet(
            current = onboarding.endpointUrl,
            defaultUrl = RegistryClient.DEFAULT_REGISTRY_URL,
            onSave = onboarding::saveEndpoint,
            onDismiss = onboarding::dismissEndpointSheet,
        )
    }
    // Spec 043: the parallel space's badge, over every route while active;
    // nothing in release, nothing outside the space.
    ParallelSpaceHook.Badge()
    }
}

/**
 * Move without stacking.
 *
 * The route guard can fire more than once for one decision — a recomposition, a
 * second view from the same dispatch — and each firing must be idempotent, or
 * the back stack fills with copies of the wallet and the system back button
 * walks through them one at a time.
 */
/**
 * Forward navigation INSIDE the app: the screen we came from stays on the
 * stack, so both the screen's own back affordance and the system's return to
 * it. [navigateSingleTop] is the other kind — a swap the session guard makes
 * when the core says a whole different route is allowed now.
 */
private fun NavHostController.push(route: String) {
    if (currentDestination?.route == route) return
    navigate(route) { launchSingleTop = true }
}

private fun NavHostController.navigateSingleTop(route: String) {
    if (currentDestination?.route == route) return
    navigate(route) {
        launchSingleTop = true
        popUpTo(graph.startDestinationId) { inclusive = true }
    }
}

/**
 * Replace whatever sits above the wallet with [route]: 通讯录 ⇄ 设置 from each
 * other's tab bar, without stacking one on the other. A no-op on the route
 * itself, like [push].
 */
private fun NavHostController.swapOverWallet(route: String) {
    if (currentDestination?.route == route) return
    navigate(route) {
        launchSingleTop = true
        popUpTo(VelaDestinations.WALLET)
    }
}

/**
 * Pop the route on top, never the last one. A bare `popBackStack()` on the
 * only entry empties the NavHost, and an empty NavHost is a blank app that
 * neither Back (the task moves to the back) nor a relaunch (`singleTop`)
 * recovers — a double-tap on a back affordance during the exit fade is all
 * it takes (spec 047, device-found 2026-09-12).
 */
private fun NavHostController.popUnlessRoot() {
    if (previousBackStackEntry != null) popBackStack()
}

/**
 * `0x14fB1f…D1eA5c` — the phone's short form, matching the wallet header's own
 * truncation so one account never reads as two.
 */
private fun shortenAddress(address: String): String =
    if (address.length <= 14) address else "${address.take(6)}…${address.takeLast(4)}"

private fun android.content.Context.openUrl(url: String) {
    runCatching {
        startActivity(
            android.content.Intent(android.content.Intent.ACTION_VIEW, android.net.Uri.parse(url)),
        )
    }
}

/**
 * Which core field a settings box belongs to, by the id its model carries.
 *
 * `null` for a box no machine owns yet — the search fields, the add-network
 * form. Those still render and still accept typing; nothing listens, and that
 * is honest until spec 041 brings the chain index they need.
 */
private fun endpointField(fieldId: String): NetEndpointField? =
    NetEndpointField.entries.firstOrNull { it.name == fieldId }

private fun providerId(fieldId: String): NetProviderId? =
    NetProviderId.entries.firstOrNull { it.name == fieldId }

/** Reached only by the `vela.startDestination` extra; the guard leaves them alone. */
internal val DEVELOPER_ROUTES = setOf(
    VelaDestinations.GALLERY,
    VelaDestinations.CONTACTS_GALLERY,
    VelaDestinations.CONTACTS,
    VelaDestinations.EXPLORE,
    VelaDestinations.FLOWS_GALLERY,
    VelaDestinations.SETTINGS_GALLERY,
    VelaDestinations.IMPORT,
)

private const val PRIVACY_URL = "https://getvela.app/privacy"
private const val TERMS_URL = "https://getvela.app/terms"

/** The flow states that show somebody their own address. */
private val RECEIVE_STATES = setOf(FlowState.R1, FlowState.R2)

/** Spec 043: the drawn send states; the flow stack holds SD1 while the send is live. */
private val SEND_STATES = setOf(
    FlowState.S1,
    FlowState.SD1, FlowState.SD1B, FlowState.SD2, FlowState.SD2B, FlowState.SD2C, FlowState.SD2D,
    FlowState.SD2E, FlowState.SD2F, FlowState.SD3, FlowState.SD3B, FlowState.SD3C,
    FlowState.SD4A, FlowState.SD4B, FlowState.SD4C,
)

/**
 * The core's refusal, in the core's words (spec 043 phase 3; phase 5 gives
 * every kind its own sentence). Never a spinner: the machine already stopped.
 */
@Composable
private fun SendAlertDialog(kind: SendAlertKind, strings: VelaStrings, onDismiss: () -> Unit) {
    val (title, body) = SendLive.alertText(kind, strings)
    androidx.compose.material3.AlertDialog(
        onDismissRequest = onDismiss,
        confirmButton = {
            androidx.compose.material3.TextButton(onClick = onDismiss) {
                androidx.compose.material3.Text(strings.t(I18nKeys.Flows.DONE))
            }
        },
        title = { androidx.compose.material3.Text(title) },
        text = { if (body.isNotEmpty()) androidx.compose.material3.Text(body) },
    )
}

/**
 * Where a pay link points.
 *
 * A phone has no origin to read, so the shell supplies the public one — the
 * core never touches a location.
 */
private const val PAY_LINK_BASE = "https://getvela.app/pay"

/**
 * The drawn flow screens, showing this device's facts.
 *
 * Only the receive screens are live in spec 041; sending is 042, and the rest
 * keep their fixtures rather than pretending. **The address is replaced
 * unconditionally** — a receive screen is the one place in this app where a
 * stale value is unrecoverable, so an empty session renders no address rather
 * than the drawn one.
 */
@Suppress("LongParameterList")
private fun liveFlow(
    drawn: FlowScreenModel,
    address: String,
    name: String,
    networks: NetView,
    request: PaymentRequestView,
    balances: BalanceView,
    feed: FeedView,
    currency: CurrencyView,
    chainNames: Map<Int, String>,
    selected: String?,
    strings: VelaStrings,
    manageTokens: MtokView = MtokView(),
    chainFilter: Int? = null,
    explorers: Map<Int, String> = emptyMap(),
): FlowScreenModel = drawn.copy(
    base = when (val base = drawn.base) {
        is FlowBase.Receive -> FlowBase.Receive(
            // The same per-chain colour the wallet's own asset rows use, so a
            // network is the same colour wherever it appears.
            FlowLive.receiveNetworks(base.model, networks, address, WalletLive::badge),
        )
        is FlowBase.History ->
            FlowBase.History(FlowLive.history(base.model, feed, strings, chainFilter = chainFilter, chainNames = chainNames))
        is FlowBase.Assets ->
            FlowBase.Assets(FlowLive.assets(base.model, balances, chainNames, currency, chainFilter))
        else -> drawn.base
    },
    sheet = when (val sheet = drawn.sheet) {
        is FlowSheet.ReceiveQr ->
            FlowSheet.ReceiveQr(FlowLive.receiveQr(sheet.model, address, name, request, explorers, strings))
        // **A detail with no target shows nothing, not a fixture.** A screen
        // about the wrong payment and one about the right payment look equally
        // authoritative, and only one of them is wrong.
        is FlowSheet.TxDetail ->
            FlowLive.txDetail(sheet.model, feed, selected, strings, chainNames, explorers, WalletLive.Money.of(currency))?.let(FlowSheet::TxDetail)
        is FlowSheet.TokenDetail -> FlowLive.tokenDetail(
            fallback = sheet.model,
            view = balances,
            feed = feed,
            id = selected,
            chainNames = chainNames,
            currency = currency,
            strings = strings,
            explorers = explorers,
        )?.let(FlowSheet::TokenDetail)
        is FlowSheet.AddToken -> FlowSheet.AddToken(FlowLive.addToken(sheet.model, manageTokens, strings))
        else -> drawn.sheet
    },
)

/** Spec 048: the asset a receive code is asked for — a token detail's 收款, or a network row's QR. */
private data class ReceiveAssetPick(val chainId: Int, val tokenAddress: String?, val symbol: String, val decimals: Int, val networkName: String)
