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
import androidx.compose.foundation.layout.Box
import app.getvela.wallet.dev.ParallelSpaceHook
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

    Box {
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
            // The holdings are this person's own from spec 041; the activity
            // feed and the flows are still fixtures until phases 5 and 6.
            val wallet = application.container.wallet
            val balances by wallet.balances.collectAsStateWithLifecycle()
            val feed by wallet.feed.collectAsStateWithLifecycle()
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

            val flows = rememberFlowNavState()
            // Which body the signed-in shell is showing. Survives rotation for
            // the same reason the flow stack does.
            var section by rememberSaveable { mutableStateOf(VelaTab.Wallet) }
            // Back unwinds the flow stack first, then leaves 探索 for 钱包 —
            // Back out of a browser should land on the wallet, not on Welcome.
            BackHandler(enabled = flows.isOpen) { flows.back() }
            BackHandler(enabled = !flows.isOpen && section == VelaTab.Explore) {
                section = VelaTab.Wallet
            }

            val flowState = flows.top
            if (flowState != null) {
                val request by wallet.request.collectAsStateWithLifecycle()
                // The receive screen shows an ADDRESS. Every other fixture that
                // leaks shows somebody the wrong information; a fixture address
                // here sends their money to a stranger, permanently — so the
                // watch and the request machine start the moment this opens.
                LaunchedEffect(flowState, session.address) {
                    if (session.address.isNotEmpty() && flowState in RECEIVE_STATES) {
                        wallet.openReceive(session.address, PAY_LINK_BASE)
                    }
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
                        )
                    }
                }
                FlowHost(
                    model = flowModel,
                    onBack = { flows.back() },
                    onNavigate = { flows.push(it) },
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
                    val signing = remember(strings, session.address, session.activeName) {
                        SigningFixtures.build(SigningScreenState.CS12, strings)
                            .withSignerIdentity(session.activeName, session.address)
                    }
                    ExploreScreen(
                        model = exploreModel,
                        signing = signing,
                        onSelectTab = select,
                    )
                } else {
                    // The holdings, the feed and the currency are this device's
                    // own (spec 041); the fixture `model` only carries the
                    // labels the live builder cannot compute.
                    WalletScreen(
                        model = WalletLive.home(model, balances, feed, currency, strings, chainNames),
                        onSelectTab = select,
                        // The id says WHICH row was tapped. Without it every row
                        // opened the same detail, so this person's own POL showed
                        // somebody else's transaction.
                        onFlow = { entry, id -> flows.enter(entry, id) },
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

            BackHandler(enabled = openContact != null) {
                confirmingDelete = false
                openContact = null
            }

            ContactsRoute(
                model = model,
                actions = ContactsActions(
                    onAction = { id ->
                        when (id) {
                            "contacts.addContact" -> menuOpen = true
                            "contacts.deleteContact" -> confirmingDelete = true
                            "contacts.delete" -> selected?.let { contact ->
                                contacts.delete(contact.address)
                                confirmingDelete = false
                                openContact = null
                            }
                            "contacts.back" -> {
                                confirmingDelete = false
                                openContact = null
                            }
                            "contacts.searchClear" -> query = ""
                            else -> Unit
                        }
                    },
                    onContact = { contact -> openContact = contact.addressFull },
                    onQueryChange = { typed -> query = typed },
                    onDismissMenu = {
                        menuOpen = false
                        confirmingDelete = false
                    },
                    onTab = { tab ->
                        if (tab == VelaTab.Wallet) navController.popBackStack()
                    },
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
            LaunchedEffect(Unit) {
                settings.refreshCurrency()
                settings.startNetworks()
            }
            SettingsRoute(
                model = SettingsLive.withWizard(
                    SettingsLive.withNetworks(
                        SettingsLive.withCurrency(model, currency),
                        networks,
                        strings,
                    ),
                    networks,
                    strings,
                ),
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
                        chainId.toLongOrNull()?.let { settings.selectChain(it) }
                    },
                    onOpenNetwork = { id ->
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
): FlowScreenModel = drawn.copy(
    base = when (val base = drawn.base) {
        is FlowBase.Receive -> FlowBase.Receive(
            // The same per-chain colour the wallet's own asset rows use, so a
            // network is the same colour wherever it appears.
            FlowLive.receiveNetworks(base.model, networks, address, WalletLive::badge),
        )
        is FlowBase.History ->
            FlowBase.History(FlowLive.history(base.model, feed, strings))
        is FlowBase.Assets ->
            FlowBase.Assets(FlowLive.assets(base.model, balances, chainNames, currency))
        else -> drawn.base
    },
    sheet = when (val sheet = drawn.sheet) {
        is FlowSheet.ReceiveQr ->
            FlowSheet.ReceiveQr(FlowLive.receiveQr(sheet.model, address, name, request))
        // **A detail with no target shows nothing, not a fixture.** A screen
        // about the wrong payment and one about the right payment look equally
        // authoritative, and only one of them is wrong.
        is FlowSheet.TxDetail ->
            FlowLive.txDetail(sheet.model, feed, selected, strings)?.let(FlowSheet::TxDetail)
        is FlowSheet.TokenDetail -> FlowLive.tokenDetail(
            fallback = sheet.model,
            view = balances,
            feed = feed,
            id = selected,
            chainNames = chainNames,
            currency = currency,
            strings = strings,
        )?.let(FlowSheet::TokenDetail)
        else -> drawn.sheet
    },
)
