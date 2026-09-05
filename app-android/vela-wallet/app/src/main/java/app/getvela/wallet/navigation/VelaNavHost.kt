package app.getvela.wallet.navigation

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
import app.getvela.wallet.feature.contacts.ContactsActions
import app.getvela.wallet.feature.contacts.ContactsFixtures
import app.getvela.wallet.feature.contacts.ContactsRoute
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
import app.getvela.wallet.feature.settings.SettingsRoute
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.gallery.SettingsGalleryScreen
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.components.VelaTab
import app.getvela.wallet.feature.wallet.gallery.GalleryScreen

object VelaDestinations {
    const val WELCOME = "welcome"
    const val CREATE = "create"
    const val IMPORT = "import"

    // Spec 015: fixture-driven wallet home + preview gallery (research D4).
    const val WALLET = "wallet"
    const val GALLERY = "gallery"

    // Spec 018: fixture-driven contacts screens + their preview gallery (D1).
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

    // Credential Manager raises system UI, which needs an Activity — not the
    // application context the ViewModel was constructed with.
    LaunchedEffect(context) { onboarding.attach(context) }

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
                        navController.navigateSingleTop(VelaDestinations.WELCOME)
                    }
                SessionRoute.Loading -> Unit
            }
        }
    }

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
                    navController.popBackStack()
                },
                onOpenPrivacy = { context.openUrl(PRIVACY_URL) },
                onOpenTerms = { context.openUrl(TERMS_URL) },
            )
        }

        composable(VelaDestinations.IMPORT) {
            ImportPlaceholderScreen(
                darkTheme = darkTheme,
                onBack = { navController.popBackStack() },
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
            val flows = rememberFlowNavState()
            BackHandler(enabled = flows.isOpen) { flows.back() }

            val flowState = flows.top
            if (flowState != null) {
                val flowModel = remember(flowState, strings) {
                    FlowFixtures.build(flowState, strings)
                }
                FlowHost(
                    model = flowModel,
                    onBack = { flows.back() },
                    onNavigate = { flows.push(it) },
                )
            } else {
                WalletScreen(
                    model = model,
                    onSelectTab = { tab ->
                        // 设置 has a screen now (spec 023), and the 退出登录 row
                        // inside it is where signing out lives. Until then the
                        // TAB itself signed you out — which meant tapping 设置
                        // to change your language logged you out instead. 通讯录
                        // and 探索 stay on this screen rather than navigating to
                        // fixtures a signed-in person would read as their real
                        // data.
                        if (tab == VelaTab.Settings) {
                            navController.push(VelaDestinations.SETTINGS)
                        }
                    },
                    onFlow = { flows.enter(it) },
                )
            }
        }

        composable(VelaDestinations.GALLERY) {
            GalleryScreen(systemDarkTheme = darkTheme)
        }

        composable(VelaDestinations.CONTACTS) {
            val strings = LocalVelaStrings.current
            var menuOpen by rememberSaveable { mutableStateOf(false) }
            val model = remember(strings, menuOpen) {
                ContactsFixtures.buildMobileState(
                    if (menuOpen) ContactsScreenState.C5 else ContactsScreenState.C1,
                    strings,
                )
            }
            ContactsRoute(
                model = model,
                actions = ContactsActions(
                    onAction = { id -> if (id == "contacts.addContact") menuOpen = true },
                    onDismissMenu = { menuOpen = false },
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
            // The display currency is the person's own, from spec 040's first
            // live machine. Everything else on this screen is still the ST1
            // fixture — the networks, endpoints and providers arrive in phase
            // 5, and the RPC health tiles need the network layer spec 041
            // brings.
            val settings = application.container.settings
            val currency by settings.currency.collectAsStateWithLifecycle()
            LaunchedEffect(Unit) { settings.refreshCurrency() }
            SettingsRoute(
                model = SettingsLive.withCurrency(model, currency),
                actions = SettingsActions(
                    onSelectTab = { tab ->
                        if (tab == VelaTab.Wallet) navController.popBackStack()
                    },
                    // The way out of a signed-in wallet, on the row a person
                    // would look for it.
                    onSignOut = { application.container.session.signOut() },
                    onOpenContacts = { navController.push(VelaDestinations.CONTACTS) },
                    onSheetSelect = { sheet, id ->
                        // Only the currency sheet has a machine behind it yet.
                        // The others still render their fixture rows and are
                        // deliberately inert rather than pretending to save.
                        if (sheet == SettingsOverlay.Currency) settings.chooseCurrency(id)
                    },
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

    // Hosted OUTSIDE the NavHost, deliberately. A prompt can be raised by either
    // machine, and the login machine runs while Welcome is on screen — so a
    // sheet nested in one route's composable would vanish the moment the route
    // guard moved, taking the question with it and leaving the core waiting for
    // an answer nobody can give.
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
            onConfirm = { application.container.session.signOutConfirmed() },
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

/** Reached only by the `vela.startDestination` extra; the guard leaves them alone. */
internal val DEVELOPER_ROUTES = setOf(
    VelaDestinations.GALLERY,
    VelaDestinations.CONTACTS_GALLERY,
    VelaDestinations.CONTACTS,
    VelaDestinations.FLOWS_GALLERY,
    VelaDestinations.SETTINGS_GALLERY,
    VelaDestinations.IMPORT,
)

private const val PRIVACY_URL = "https://getvela.app/privacy"
private const val TERMS_URL = "https://getvela.app/terms"
