//
//  RootView.swift
//  VelaWallet
//
//  App-level composition: navigation stack, theme injection, and the
//  welcome content resolved through the localization layer.
//

import SwiftUI

/// Navigation state. Since spec 019 the create journey is a pushed route
/// rather than a presented sheet — `presentedFlow` is gone with the 014 sheet.
@Observable
final class Router {
    var path: [AppRoute] = []
}

struct RootView: View {
    @Environment(\.colorScheme) private var systemScheme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    let loc: Loc
    @State private var router: Router
    @State private var model: WelcomeModel
    @State private var session: SessionController
    @State private var onboarding: OnboardingModel
    /// The address book's machine (spec 050), app-resident like the session's.
    ///
    /// Constructed here rather than inside the contacts section because a Crux
    /// core's model IS the app's state for that surface: rebuilding it whenever
    /// somebody switches tabs would re-read storage on every visit and throw
    /// away the identity and classification caches the core keeps.
    @State private var contacts: ContactsStore

    // Spec 012. `true` only for the first construction in this process — a
    // cold start (FR-008); SwiftUI never rebuilds `RootView`'s State on a
    // navigation, so there is no path back once it flips.
    /// The wallet-flow stack (spec 021). Lives here, beside the session, so
    /// it survives the wallet body's own re-renders.
    @State private var flows = FlowNav()
    /// Which section of the signed-in shell is showing (spec 050).
    @State private var section: WalletSection = .wallet
    /// Where the contacts section is, inside itself.
    @State private var contactsRoute: ContactsRoute?
    @State private var launching = !LaunchAnimation.isDisabled
    /// Welcome content fades IN as the launch lockup fades OUT (FR-012).
    @State private var pageOpacity: Double = LaunchAnimation.isDisabled ? 1 : 0

    init(loc: Loc) {
        self.loc = loc
        let router = Router()
        _router = State(initialValue: router)
        let store = AccountStore()
        let session = SessionController(store: store)
        let onboarding = OnboardingModel(session: session, store: store)
        _session = State(initialValue: session)
        _onboarding = State(initialValue: onboarding)
        _contacts = State(initialValue: ContactsStore(store: VelaStore()))
        _model = State(initialValue: WelcomeModel(content: WelcomeContentBuilder.build(loc: loc)) { intent in
            switch intent {
            case .createWallet:
                router.path.append(.create)
            case .openSignIn:
                // Open the sign-in method picker — the person then chooses the
                // authenticator, and the login machine runs the "who are you?"
                // ceremony on that route. The picker rides the ONE onboarding
                // sheet, so its choice can hand straight to the PIN/touch prompts
                // without a sheet dismissing between them.
                onboarding.showSignInMethods = true
            }
        })
    }

    private var scheme: ColorScheme {
        ThemeOverride.launchScheme ?? systemScheme
    }

    var body: some View {
        // Spec 014: dev-only state gallery replaces the app when launched
        // with VELA_GALLERY=1. Debug-only compile + env gate (FR-013).
        #if DEBUG
        if GalleryMode.isEnabled {
            OnboardingGalleryScreen(loc: loc)
        } else {
            appBody
        }
        #else
        appBody
        #endif
    }

    private var appBody: some View {
        @Bindable var router = router
        // One continuous surface. Both the launch screen and Welcome sit on this
        // exact colour, which is what lets them cross-dissolve without a
        // washed-out middle where both layers are half-transparent (FR-012).
        return ZStack {
            themedBackground

            content(router: $router.path)
                // Composed from the first frame, hidden by the opaque overlay,
                // so the hand-off has nothing left to build (FR-013a).
                .opacity(pageOpacity)
                // Opacity alone does NOT remove it from the accessibility tree:
                // during the animation VoiceOver would happily read out a
                // Welcome screen the user cannot see, and XCUITest could find
                // its buttons. Both are the same defect (FR-013/FR-021).
                .accessibilityHidden(launching)

            if launching {
                LaunchAnimationView(
                    appearance: scheme == .dark ? .dark : .light,
                    formFactor: LaunchAnimation.formFactor(for: screenSize),
                    reduceMotion: reduceMotion,
                    onDissolveStart: {
                        // The other half of the cross-dissolve: same curve, same
                        // duration, started in the same instant as the overlay's
                        // fade-out (FR-012).
                        withAnimation(.easeInOut(duration: LaunchAnimation.exitCrossfade)) {
                            pageOpacity = 1
                        }
                    },
                    onFinished: {
                        pageOpacity = 1
                        launching = false
                    }
                )
            }
        }
        .themed(scheme)
        .preferredColorScheme(ThemeOverride.launchScheme)
    }

    private var themedBackground: some View {
        Theme(scheme: scheme).bgBase.ignoresSafeArea()
    }

    private var screenSize: CGSize {
        #if canImport(UIKit)
        UIScreen.main.bounds.size
        #else
        CGSize(width: 390, height: 844)
        #endif
    }

    @ViewBuilder
    private func content(router path: Binding<[AppRoute]>) -> some View {
        switch PageOverride.page {
        case .wallet:
            WalletScreen(model: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc)
        case .gallery:
            GalleryScreen(loc: loc)
        case .contacts:
            ContactsStateHost(state: .c1, loc: loc)
        case .contactsLive:
            contactsSection
        case .contactsGallery:
            ContactsGalleryScreen(loc: loc)
        case .flowsGallery:
            FlowGalleryScreen(loc: loc)
        case .settings:
            SettingsScreen(model: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        case .settingsGallery:
            SettingsGalleryScreen(loc: loc)
        case nil:
            NavigationStack(path: path) {
                signedInOrWelcome
                    .navigationDestination(for: AppRoute.self) { route in
                        switch route {
                        case .create:
                            CreateFlowScreen(
                                loc: loc,
                                model: onboarding,
                                onExit: { router.path.removeLast() },
                                onLink: openPolicy
                            )
                            .navigationBarBackButtonHidden()
                        case .settings:
                            settingsScreen
                                .navigationBarBackButtonHidden()
                        }
                    }
            }
            // The route guard.
            //
            // `allowedRoute` is the core's ruling about WHAT is allowed; when to
            // move is this view's call. It moves only for the two settled
            // routes: `loading` is deliberately not navigated to, because the
            // launch animation already covers that frame and bouncing through a
            // spinner route would make a cold start flicker.
            .onChange(of: session.view.allowedRoute) { _, route in
                if route == .onboarding { router.path.removeAll() }
            }
            .onChange(of: onboarding.finished) { _, finished in
                if finished {
                    router.path.removeAll()
                    onboarding.consumeFinished()
                }
            }
            // The ONE onboarding sheet. Every app-owned ceremony prompt — the
            // sign-in method picker, the "connecting…" hold, the security key's
            // PIN, the touch, the which-wallet picker, and the create/login flow
            // prompts — shares this single `.sheet`, its content chosen by
            // priority. Presenting a second sheet while a first is dismissing
            // fails silently on iOS (the nesting bug the founder hit on iPhone,
            // 2026-08-27): the PIN would arrive as the method picker dismissed
            // and simply never appear. One sheet whose CONTENT swaps never
            // dismisses between steps, so nothing is dropped.
            //
            // Hosted at the ROOT deliberately: a prompt can be raised by either
            // machine, and the login machine runs while Welcome is on screen, so
            // a sheet attached to one route's view would vanish the moment the
            // guard moved and leave the core waiting for an answer nobody can
            // give.
            .sheet(isPresented: onboardingSheet) {
                onboardingSheetContent
            }
            // The way back out of a signed-in wallet.
            //
            // Rendered from `session.view.signOut`, which is non-null only after
            // the machine has ASKED STORAGE whether any public key is still
            // unconfirmed — so the warning inside is an answer rather than this
            // screen's guess, and the sheet cannot open before there is one.
            .sheet(item: signOutSheet) { sheet in
                SignOutSheet(
                    loc: loc,
                    pendingUploadWarning: sheet.pendingUploadWarning,
                    onConfirm: { session.signOutConfirmed() },
                    onDismiss: { session.signOutDismissed() }
                )
                .themed(scheme)
            }
            .sheet(isPresented: endpointSheet) {
                EndpointSheet(
                    loc: loc,
                    defaultURL: RegistryClient.defaultURL,
                    draft: onboarding.endpointURL,
                    onSave: onboarding.saveEndpoint
                )
                .themed(scheme)
            }
            .task { session.boot() }
        }
    }

    /// The wallet when the core says there is one, Welcome otherwise.
    ///
    /// The wallet body is still the spec-015 fixture layer apart from the two
    /// things that identify the wallet — its address and its name, both now
    /// the real ones. A home screen showing a fixture address after a real
    /// create would be the app telling the person their money is somewhere it
    /// is not; a fixture NAME over their own address and identicon told them
    /// they were signed in as somebody else (device-found 2026-08-26).
    @ViewBuilder
    private var signedInOrWelcome: some View {
        if session.view.allowedRoute == .wallet {
            // Spec 021: Receive / Send / Activity / Assets, as pushed screens
            // over this one. A stack rather than NavigationStack destinations
            // because these are still fixtures — and because the flows push
            // full-bleed surfaces (the scanner, the share card) that a
            // navigation bar would frame wrongly.
            if let state = flows.top {
                FlowHost(
                    model: WalletFlowFixtures.build(state, loc: loc),
                    onBack: { flows.back() },
                    onNavigate: { flows.push($0) }
                )
                .transition(.move(edge: .trailing))
            } else {
                switch section {
                case .wallet:
                    WalletScreen(
                        model: WalletFixtures
                            .buildMobileState(.h1, loc: loc)
                            .withAddress(session.view.address)
                            .withName(session.view.activeName),
                        loc: loc,
                        onSelectTab: selectTab,
                        onFlow: { flows.enter($0) }
                    )
                case .contacts:
                    contactsSection
                }
            }
        } else {
            WelcomeScreen(loc: loc, model: model, signingIn: onboarding.loginView.busy)
        }
    }

    /// The tab bar's four destinations.
    ///
    /// 设置 has a screen (spec 023), and the 退出登录 row inside it is where
    /// signing out lives. Until spec 023 the TAB itself signed you out — which
    /// meant tapping 设置 to change your language logged you out instead. That
    /// regression must not come back through this switch.
    ///
    /// 通讯录 became a real destination in spec 050; before it, the tab was
    /// drawn, tappable, and did nothing at all. 探索 still does nothing, and
    /// stays that way until its machines are wired.
    private func selectTab(_ tab: WalletTab) {
        switch tab {
        case .settings: router.path.append(.settings)
        case .wallet: section = .wallet
        case .contacts: section = .contacts
        case .explore: break
        }
    }

    /// The address book, live (spec 050).
    ///
    /// Detail and group are pushes *within* the section rather than app routes,
    /// for the reason the flows are: an address book is not somewhere a person
    /// should be able to deep-link into before they have a wallet. They are
    /// keyed by the core's own identifiers, so a book that reloads under the
    /// screen cannot leave it pointing at a row that no longer exists — it
    /// falls back to the list, which is what a deleted contact should do.
    @ViewBuilder private var contactsSection: some View {
        Group {
            if let view = contacts.view, view.loaded {
                switch contactsRoute {
                case .detail(let address):
                    if let contact = contacts.contact(at: address) {
                        ContactDetailScreen(
                            model: ContactsLive.detail(contact, view: view, loc: loc),
                            onBack: { contactsRoute = nil }
                        )
                    } else {
                        contactsHome(view)
                    }
                case .group(let id):
                    if let group = contacts.group(id: id) {
                        GroupDetailScreen(
                            model: ContactsLive.group(group, view: view, loc: loc),
                            onBack: { contactsRoute = nil },
                            onOpenMember: { member in
                                contactsRoute = .detail(address: member.addressFull)
                            }
                        )
                    } else {
                        contactsHome(view)
                    }
                case nil:
                    contactsHome(view)
                }
            } else {
                // The core has not ruled yet. Real chrome, empty list — never
                // fixture contacts, and never the "you have none" state.
                ContactsScreen(
                    model: ContactsLive.waiting(loc: loc),
                    onSelectTab: selectTab
                )
            }
        }
        // Idempotent: the first appearance boots the machine, later ones speak
        // up only if the signed-in account actually changed.
        .task(id: session.view.address) {
            contacts.open(myAddress: session.view.address)
        }
    }

    private func contactsHome(_ view: ContactsViewWire) -> some View {
        ContactsScreen(
            model: ContactsLive.home(view, loc: loc, query: contacts.query),
            onOpenContact: { contactsRoute = .detail(address: $0.addressFull) },
            onOpenGroup: { row in
                // The row model carries a display name, not the core's id — so
                // the id is looked up here rather than smuggled through the
                // drawing.
                if let group = view.groups.first(where: { $0.name == row.name }) {
                    contactsRoute = .group(id: group.id)
                }
            },
            onSelectTab: selectTab,
            onDelete: { contacts.delete(address: $0) }
        )
    }

    /// Settings, wearing the signed-in identity.
    ///
    /// Fixture-driven still (spec 023 scope) apart from the two things that
    /// identify the account — its name and address, both the real ones. A
    /// fixture name over a real address would tell somebody they are signed in
    /// as a stranger (spec 019's finding).
    @ViewBuilder private var settingsScreen: some View {
        SettingsScreen(
            model: SettingsFixtures.build(.st1, loc: loc)
                .withIdentity(
                    name: session.view.activeName,
                    address: session.view.address,
                    display: Self.shortenAddress(session.view.address)
                ),
            loc: loc,
            onSelectTab: { tab in
                if tab == .wallet { router.path.removeLast() }
            },
            // The way out of a signed-in wallet, on the row a person would
            // look for it.
            onSignOut: { session.signOut() }
        )
    }

    /// `0x14fB1f…D1eA5c` — the phone's short form, matching the wallet header's
    /// own truncation so one account never reads as two.
    private static func shortenAddress(_ address: String) -> String {
        guard address.count > 14 else { return address }
        return "\(address.prefix(6))…\(address.suffix(4))"
    }

    /// The single onboarding sheet's presentation. A swipe-to-dismiss routes to
    /// `dismissOnboardingSheet`, which cancels whatever the active prompt is
    /// waiting for; the touch and connecting states disable interactive dismiss,
    /// so they never reach it.
    private var onboardingSheet: Binding<Bool> {
        Binding(
            get: { onboarding.onboardingSheetPresented },
            set: { if !$0 { onboarding.dismissOnboardingSheet() } }
        )
    }

    /// The onboarding sheet's content, chosen by priority so a later step wins
    /// over the state it replaces: pin > wallet pick > touch > flow prompt >
    /// connecting hold > method picker.
    @ViewBuilder private var onboardingSheetContent: some View {
        if let pin = onboarding.pendingPin {
            UsbPinSheet(loc: loc, pending: pin, onSubmit: onboarding.answerPin)
                .themed(scheme)
        } else if let pick = onboarding.pendingWalletPick {
            UsbWalletPickerSheet(loc: loc, pending: pick, onPick: onboarding.answerWalletPick)
                .themed(scheme)
        } else if let touch = onboarding.usbTouch {
            UsbTouchSheet(loc: loc, touch: touch)
                .themed(scheme)
        } else if let payload = onboarding.cableQr {
            // Below touch on purpose: once the phone connects and the ceremony
            // is waiting on ITS sheet, "look at your phone" replaces the QR.
            CableQrSheet(loc: loc, payload: payload)
                .themed(scheme)
        } else if let prompt = onboarding.pending {
            FlowSheet(
                loc: loc,
                kind: prompt.kind,
                confirmable: prompt.confirmable,
                onAnswer: onboarding.answerPrompt
            )
            .themed(scheme)
        } else if onboarding.signInConnecting {
            UsbConnectingSheet(loc: loc, method: onboarding.signInMethod)
                .themed(scheme)
        } else if onboarding.showSignInMethods {
            SignInMethodSheet(loc: loc, onPick: onboarding.pickSignInMethod)
                .themed(scheme)
        }
    }

    private var signOutSheet: Binding<SessionSignOutView?> {
        Binding(
            get: { session.view.signOut },
            set: { if $0 == nil { session.signOutDismissed() } }
        )
    }

    private var endpointSheet: Binding<Bool> {
        Binding(get: { onboarding.endpointSheetOpen }, set: { onboarding.endpointSheetOpen = $0 })
    }

    private func openPolicy(_ action: ActionId) {
        let url = switch action {
        case .openPrivacyPolicy: URL(string: "https://getvela.app/privacy")
        case .openTerms: URL(string: "https://getvela.app/terms")
        }
        if let url { UIApplication.shared.open(url) }
    }
}

/// Which section of the signed-in shell is showing (spec 050).
///
/// A section, not a route: `design/contacts/C1` draws the tab bar with 通讯录
/// **selected**, so it is a peer of 钱包 rather than something pushed over it.
/// 设置 is the opposite case — its drawing has a back affordance — and stays an
/// `AppRoute`. 探索 joins this enum when its machines are wired.
enum WalletSection { case wallet, contacts }

/// Where the contacts section is, inside itself (spec 050).
///
/// Keyed by the core's own identifiers rather than by a row's SwiftUI `id`,
/// which is a `UUID()` minted per render: a book that reloads under the screen
/// would otherwise leave it pointing at a row that no longer exists.
enum ContactsRoute: Equatable {
    case detail(address: String)
    case group(id: String)
}

/// `VELA_PAGE` launch override (spec 015 research D4, extended by spec 018
/// research D1) — same idiom as `VELA_THEME`/`VELA_LANG`: `wallet` mounts
/// the fixture-driven home, `gallery` the wallet preview gallery,
/// `contacts` the contacts home, `contacts-gallery` the contacts preview
/// gallery; unset keeps the Welcome flow. Never part of production
/// navigation (FR-004).
enum PageOverride {
    enum Page {
        case wallet, gallery, contacts, contactsLive, contactsGallery, flowsGallery
        case settings, settingsGallery
    }

    static let page: Page? = {
        switch ProcessInfo.processInfo.environment["VELA_PAGE"] {
        case "wallet": .wallet
        case "gallery": .gallery
        case "contacts": .contacts
        // The LIVE address book, without a wallet (spec 050) — the desktop's
        // `VELA_SECTION` by another name. `contacts` mounts the fixture host;
        // this mounts the real machine over whatever `vela.contacts` actually
        // holds, which is the only way to look at the wired screen without
        // first completing a passkey ceremony.
        case "contacts-live": .contactsLive
        case "contacts-gallery": .contactsGallery
        case "flows-gallery": .flowsGallery
        case "settings": .settings
        case "settings-gallery": .settingsGallery
        default: nil
        }
    }()
}

/// Resolves every welcome-screen string from the corpus — existing keys only,
/// no new corpus entries.
///
/// `featureNoMnemonic*` … `featureStablecoinGas*` are no longer resolved: the
/// v2 screen has no cards to put them on (spec 019). They stay in the corpus
/// rather than being deleted, because they are written marketing copy and the
/// page they belong on may yet exist — the same call the web made.
enum WelcomeContentBuilder {
    static func build(loc: Loc) -> WelcomeContent {
        WelcomeContent(
            heroTitle: loc.t("onboarding.welcome.heroTitle"),
            heroTitleFit: HeroFit(corpusValue: loc.t("onboarding.welcome.heroTitleFit")),
            heroSubtitle: loc.t("onboarding.welcome.heroSubtitle"),
            createWallet: loc.t("onboarding.welcome.createWallet"),
            alreadyHaveWallet: loc.t("onboarding.welcome.alreadyHaveWallet")
        )
    }
}

/// `VELA_THEME` launch override (FR-003 / D7) — read once; `nil` follows the
/// system appearance.
enum ThemeOverride {
    static let launchScheme: ColorScheme? = {
        switch ProcessInfo.processInfo.environment["VELA_THEME"] {
        case "light": .light
        case "dark": .dark
        default: nil
        }
    }()
}
