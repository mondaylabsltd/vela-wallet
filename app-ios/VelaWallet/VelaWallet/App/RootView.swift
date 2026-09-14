//
//  RootView.swift
//  VelaWallet
//
//  App-level composition: navigation stack, theme injection, and the
//  welcome content resolved through the localization layer.
//

import SwiftUI
import UIKit

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
    /// The `vela.*` shelf, for the reads that are not a machine's — the
    /// explorer bases of custom networks.
    private let shelf: VelaStore
    /// The account list, kept rather than passed and forgotten: the parallel
    /// space's door needs it at `.task` time, after `init` has finished.
    private let accounts: AccountStore
    @State private var router: Router
    @State private var model: WelcomeModel
    @State private var session: SessionController
    @State private var onboarding: OnboardingModel
    /// The routing authority (spec 051). One session, app-wide: a ban is a fact
    /// about the network, and two callers with their own endpoint lists is how
    /// the Expo client got a ban map that disagreed with itself.
    @State private var pool: RpcPool
    /// The home screen's money (spec 051), app-resident: it holds the fetched
    /// holdings, the cached total and a retry timer, and one that died with the
    /// screen would re-read twelve chains on every visit.
    @State private var wallet: WalletStore
    /// The anti-scam core behind the receipt scan — resident, because the
    /// trusted-token set it builds is shared by everything that reads a chain.
    @State private var trust: TokenTrustStore
    @State private var activity: ActivityStore
    @State private var tokens: ManageTokensStore
    @State private var deposits: ReceiveWatchStore
    /// Which network row opened the receive code.
    @State private var receiveNetwork = 0
    /// Which history row opened the transaction sheet, and which assets row
    /// opened the token sheet. The drawn sheets show ONE of each; without the
    /// tap travelling with the navigation they would show the first.
    @State private var activityRow: (group: Int, row: Int) = (0, 0)
    @State private var assetRow = 0
    /// The network filter on the history screen. `nil` is every chain — the
    /// core owns the filtering; this is only which row was picked.
    @State private var chainFilter: Int?
    /// What 保存图片 did, once it has done it. The corpus has the words; the
    /// alert is the platform's.
    @State private var saveOutcome: ShareCardExport.Outcome?
    /// The networks machine (spec 050), app-resident: it probes endpoints and
    /// holds a search debounce, and one that died with the settings route would
    /// re-probe every chain on each visit.
    @State private var settings: SettingsStore
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
    /// The money machines. Resident, because a send that died with its screen
    /// would lose a submitted operation the moment somebody swiped back.
    @State private var fees: FeeStore
    @State private var send: SendStore
    /// What the person is typing. Held locally and echoed to the core, which
    /// owns the value: a field bound straight to a machine loses characters on
    /// the round trip (Android found it on the device).
    @State private var recipientDraft = ""
    @State private var amountDraft = ""
    @State private var feeSheetOpen = false
    @State private var sendAlert: (title: String, body: String)?
    @State private var flows: FlowNav
    /// Which section of the signed-in shell is showing (spec 050).
    @State private var section: WalletSection = .wallet
    /// Where the contacts section is, inside itself.
    @State private var contactsRoute: ContactsRoute?
    /// Whether the app is inside the parallel space, for the badge.
    ///
    /// Mirrored into view state rather than read from the hook on every render:
    /// the hook is the authority, and a `@State` copy is what makes SwiftUI
    /// redraw when the door opens during `.task`.
    @State private var parallelSpace = false
    @State private var launching = !LaunchAnimation.isDisabled
    /// Welcome content fades IN as the launch lockup fades OUT (FR-012).
    @State private var pageOpacity: Double = LaunchAnimation.isDisabled ? 1 : 0

    init(loc: Loc) {
        self.loc = loc
        let router = Router()
        _router = State(initialValue: router)
        let flowNav = FlowNav()
        _flows = State(initialValue: flowNav)
        let store = AccountStore()
        self.accounts = store
        let session = SessionController(store: store)
        let onboarding = OnboardingModel(session: session, store: store)
        _session = State(initialValue: session)
        _onboarding = State(initialValue: onboarding)
        let shelf = VelaStore()
        self.shelf = shelf
        // Before the session machine boots: it reads `vela.accounts` on its
        // first event, and a record written after that is not seen until a
        // relaunch. DEBUG-only, env-gated, and key-less (spec 051 D3).
        DevAccountSeed.applyIfRequested(store: shelf)
        // One pool, built before anything that reads a chain — the settings
        // machines included, since spec 051 put the fiat feeds behind it.
        let pool = RpcPool(store: shelf, accounts: store)
        _pool = State(initialValue: pool)
        // ONE name resolver for the whole app: the address book and the
        // activity feed ask the same question about the same addresses, and two
        // resolvers would mean two caches and two names for one person.
        let identity = RecipientIdentity(store: shelf, pool: pool, accounts: store)
        // The money path. One relay client, one spine, one fee session — the
        // spine is shared with 053's dApp transactions, which is why it is
        // built here rather than inside the send store.
        let relay = RelayClient(port: PoolRelayPort(pool: pool))
        let port = SendAccountPort(accounts: store)
        let spine = UserOpSpine(
            relay: relay,
            accounts: port,
            signer: { ParallelSpaceHook.signer(passkey: PasskeyExecutor()) }
        )
        let feeStore = FeeStore(relay: relay, accounts: port)
        _fees = State(initialValue: feeStore)
        _contacts = State(initialValue: ContactsStore(
            store: shelf, identity: identity, pool: pool
        ))
        // `vela.serviceEndpoints` has two writers; the executor reaches it
        // through this same `AccountStore` so onboarding's endpoint override
        // survives a settings write (data-model §5).
        let settingsStore = SettingsStore(store: shelf, accounts: store, pool: pool)
        _settings = State(initialValue: settingsStore)
        // The balance read publishes what it found here, and the receipt scan
        // reads it: which chains this account uses, which tokens it holds, and
        // what they were worth. Web gets the same three facts from its
        // `fetchTokens` cache; there is no such cache here, so it is explicit.
        let held = HeldTokens()
        let trust = TokenTrustStore(store: shelf, pool: pool, accounts: store, held: held)
        _trust = State(initialValue: trust)
        _activity = State(initialValue: ActivityStore(
            store: shelf, accounts: store, held: held, trust: trust, identity: identity
        ))
        // A saved token has to reach the balances, so the core's
        // "invalidate the token cache" becomes a re-read here — there is no
        // cache on this client, only a fetch.
        let wallet = WalletStore(store: shelf, pool: pool, held: held)
        _wallet = State(initialValue: wallet)
        _tokens = State(initialValue: ManageTokensStore(
            store: shelf, pool: pool,
            onInvalidate: { [weak wallet] in wallet?.refresh(pull: false) }
        ))
        // The send machine, last: it reads the holdings the balance machine
        // found and asks the fee session for a quote, so both must exist.
        let metadata = TokenMetadata(store: shelf, pool: pool)
        let sendExecutor = SendExecutor(
            store: shelf, relay: relay, pool: pool, spine: spine, accounts: port,
            fees: feeStore, identity: identity, metadata: metadata, accountStore: store,
            balances: { [weak wallet] in wallet?.balance },
            networks: { [weak settingsStore] in settingsStore?.networkAdmin },
            ports: SendExecutor.Ports(
                // The core's own exit. `Done` on a receipt, and `close` on any
                // refusal that ends the attempt, both land here.
                closed: { [weak flowNav] in flowNav?.close() },
                // Leaving is what re-arms `Open`. Without it a second visit to
                // 转账 would render the machine's last state instead of a
                // fresh picker.
                refreshBalances: { [weak wallet] in wallet?.refresh(pull: false) }
            )
        )
        _send = State(initialValue: SendStore(executor: sendExecutor))
        // The receive screen's watcher. A detected deposit buzzes and re-reads
        // the balances, so the figure behind the code is the new one.
        _deposits = State(initialValue: ReceiveWatchStore(
            store: shelf, pool: pool, held: held,
            onDeposit: { [weak wallet] in wallet?.refresh(pull: false) }
        ))
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
        return ZStack(alignment: .top) {
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
        // A screenshot taken in the parallel space must never be mistaken for
        // one taken in the real app. As a `safeAreaInset` rather than an
        // overlay it MOVES the content down instead of covering it — an
        // overlay clipped the account name, which is the opposite of what a
        // marker is for. It answers to neither gate (not the theme, not the
        // launch animation) and is absent from Release by construction.
        .safeAreaInset(edge: .top, spacing: 0) {
            #if DEBUG
            if parallelSpace { ParallelSpaceBadge() }
            #endif
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
        case .settingsLive:
            // `VELA_STATE=st9` opens straight on the networks page, which
            // is otherwise two taps inside a route.
            settingsScreen(SettingsStateId(rawValue: PageOverride.state ?? "st1") ?? .st1)
        case .contactsGallery:
            ContactsGalleryScreen(loc: loc)
        case .flowsGallery:
            FlowGalleryScreen(loc: loc)
        case .settings:
            SettingsScreen(model: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        case .settingsGallery:
            SettingsGalleryScreen(loc: loc)
        case .explore:
            ExploreScreen(
                model: ExploreFixtures.buildMobileState(
                    ExploreStateId(rawValue: PageOverride.state ?? "e2") ?? .e2, loc: loc
                ),
                loc: loc,
                signing: SigningFixtures.build(.cs12, loc: loc)
            )
        case .signing:
            SigningSheet(
                model: SigningFixtures.build(
                    SigningStateId(rawValue: PageOverride.state ?? "cs1") ?? .cs1, loc: loc
                )
            )
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
                            settingsScreen()
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
            .task {
                // Before the session machine's first event, which is when it
                // reads `vela.accounts`: a record written after that is not
                // seen until a relaunch. The space upserts ONE record and
                // removes exactly that one on the way out (FR-003) — this door
                // is opened on a phone that holds the founder's real wallet.
                await ParallelSpaceHook.applyIfRequested(store: shelf, accounts: accounts)
                parallelSpace = ParallelSpaceHook.isActive
                session.boot()
            }
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
            if let drawn = flows.top {
                // The CORE owns which send screen is showing. A back-swipe and
                // a stage change must agree, and only one of them can be the
                // authority — on every other client it is the machine.
                let state = sendStates.contains(drawn)
                    ? (send.view.map { SendLive.flowState($0, feeSheetOpen: feeSheetOpen) } ?? drawn)
                    : drawn
                FlowHost(
                    model: flowModel(state),
                    onBack: { flows.back() },
                    onNavigate: { flows.push($0) },
                    addTokenInput: addTokenInput(for: state),
                    onAddToken: addTokenAction(for: state),
                    addTokenError: addTokenError(for: state),
                    onReceiveNetwork: { receiveNetwork = $0 },
                    onSelectActivity: { activityRow = ($0, $1) },
                    onSelectAsset: { assetRow = $0 },
                    chainSheet: activity.feed.map {
                        FlowsLive.chainSheet($0, selected: chainFilter, loc: loc)
                    },
                    onPickChain: { chainId in
                        chainFilter = chainId
                        activity.chainFilter(chainId)
                    },
                    onExplorer: explorerLink(for: state).map { url in
                        { UIApplication.shared.open(url) }
                    },
                    onSaveCard: session.view.address.isEmpty ? nil : { saveShareCard() },
                    // The core's refusal outranks the save alert: one is an
                    // answer to something the person just did with money, the
                    // other is about a picture.
                    alert: sendRefusal ?? saveAlert,
                    onDismissAlert: {
                        if send.alert != nil { send.alert = nil } else { saveOutcome = nil }
                    },
                    sendAmount: sendStates.contains(state) ? $amountDraft : nil,
                    sendRecipient: sendStates.contains(state) ? $recipientDraft : nil,
                    sendWarning: send.view.flatMap { SendLive.formWarning($0, loc: loc) },
                    sendCtaDisabled: sendCtaDisabled(state),
                    onSelectToken: { index in
                        guard let token = send.view?.tokens[safe: index] else { return }
                        send.selectToken(id: token.id)
                    },
                    onMax: { send.tapMax() },
                    onConfirm: { send.slideConfirm() },
                    onReceiptDone: { send.done() },
                    onContinueSend: { send.advance() }
                )
                .transition(.move(edge: .trailing))
                // The field holds what is typed and the core holds the value:
                // a field bound straight to a machine loses characters on the
                // round trip. The echo back is ignored while the person types.
                .onChange(of: recipientDraft) { _, value in send.setRecipient(value) }
                .onChange(of: amountDraft) { _, value in send.setAmount(value) }
                .onChange(of: send.view?.amount) { _, value in
                    if let value, value != amountDraft { amountDraft = value }
                }
                // The bridge between the two money machines. The core keeps
                // them apart on purpose — the signing sheet uses the fee
                // machine without the send machine existing — so every client
                // joins them in the shell, and this is that joint.
                .onChange(of: fees.view?.busy) { _, busy in
                    if let busy { send.feeBusyChanged(busy) }
                }
                .onChange(of: fees.view?.fee) { _, estimate in
                    if let estimate { send.feeUpdated(estimate) }
                }
                // The picker is a READ of what the balance machine had at the
                // instant the flow opened — and on a cold start that instant
                // is before the chains have answered. Without this the list is
                // empty forever, which is what the device showed: not the
                // fixture's tokens, not the wallet's, nothing at all.
                .onChange(of: wallet.balance?.tokens.count) { _, _ in
                    if sendStates.contains(state) { send.refreshTokens() }
                }
                .task(id: state) {
                    if state == .t3 { tokens.open() }
                    // The watcher runs while a code is on screen — five
                    // minutes, at the core's own cadence, and it stops itself.
                    if state == .r2 || state == .r3 {
                        deposits.open(address: session.view.address)
                    }
                }
                // Keyed on the DRAWN state, not the derived one.
                //
                // `Open` re-enters the flow, so firing it whenever the derived
                // state changes reset the machine to the picker the instant it
                // reached the form — the screen flickered to SD2 and came
                // straight back. `flows.top` stays `.sd1` for the whole
                // journey, which is exactly the lifetime this event has.
                .task(id: drawn) {
                    if sendStates.contains(drawn) { await openSend() }
                }
            } else {
                switch section {
                case .wallet:
                    WalletScreen(
                        model: walletModel,
                        loc: loc,
                        onSelectTab: selectTab,
                        onFlow: { flows.enter($0) },
                        onToggleBalance: { wallet.togglePrivacy() },
                        onRefresh: RefreshAction {
                            // `pull: true` is carried so the core can tell a
                            // person's own gesture from the 30-second tick and
                            // answer it differently.
                            wallet.refresh(pull: true)
                            activity.focusTick()
                            await wallet.settled()
                        }
                    )
                    // The two machines have to agree about hiding: the balance
                    // core owns the state, and the feed core suppresses its
                    // receipt toast on it. Forwarding the COMMITTED value
                    // rather than a guess made at tap time is what keeps them
                    // from disagreeing by one tap.
                    .onChange(of: wallet.balance?.hidden ?? false) { _, hidden in
                        activity.privacyChanged(hidden: hidden)
                    }
                    .task {
                        pool.boot()
                        activity.open(address: session.view.address,
                                      hidden: wallet.balance?.hidden ?? false)
                        // The display currency is app-wide: the hero is the
                        // figure it matters most on, and it must not wait for a
                        // visit to 设置 to learn the person chose CNY.
                        settings.openCurrency()
                        // So is the NETWORK list, and for a sharper reason: the
                        // send machine resolves every holding against it, so a
                        // `network_admin` that had not been opened yet made the
                        // token picker EMPTY — device-found, and the same shape
                        // as Android's contact picker, which was empty because
                        // its machine only booted on the contacts page.
                        settings.open()
                        wallet.open(address: session.view.address)
                    }
                case .contacts:
                    contactsSection
                case .explore:
                    // Spec 022/029: 探索 is a real destination now rather than an
                    // inert chip. Its body is a fixture layer exactly like the
                    // wallet's, but the account it shows a site is the REAL one —
                    // a connection panel naming a stranger's account would be the
                    // wallet lying about what it just granted.
                    ExploreScreen(
                        model: ExploreFixtures.buildMobileState(.e2, loc: loc)
                            .withIdentity(name: session.view.activeName,
                                          address: session.view.address),
                        loc: loc,
                        signing: SigningFixtures.build(.cs12, loc: loc)
                            .withIdentity(name: session.view.activeName,
                                          address: session.view.address),
                        onSelectTab: selectTab
                    )
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
    /// drawn, tappable, and did nothing at all. 探索 opens the browser's
    /// fixture layer (spec 022/029) over the real identity; its machines are
    /// not wired yet.
    private func selectTab(_ tab: WalletTab) {
        switch tab {
        case .settings: router.path.append(.settings)
        case .wallet: section = .wallet
        case .contacts: section = .contacts
        case .explore: section = .explore
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
                            },
                            onDeleteGroup: {
                                contacts.deleteGroup(id: group.id)
                                // Back to the list: the page this was is gone.
                                contactsRoute = nil
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

    /// The home screen: the person's own address, name — and, since spec 051,
    /// their own money.
    ///
    /// Before the core has ruled there is nothing truthful to swap in, so the
    /// drawing stands as drawn. That is the same rule the settings screen
    /// follows, and it matters more here: a fixture total under a real address
    /// is the app telling somebody their money is somewhere it is not.
    private var walletModel: WalletHomeModel {
        let base = WalletFixtures
            .buildMobileState(.h1, loc: loc)
            .withAddress(session.view.address)
            .withName(session.view.activeName)
        guard let view = wallet.balance else { return base }
        return WalletLive.apply(view, currency: settings.currency, feed: activity.feed,
                                feedRead: activity.hasRead, on: base, loc: loc)
    }

    /// A flow screen, with the parts that have machines behind them swapped
    /// in and the rest still drawn.
    ///
    /// Two so far: the assets list is the same holdings the home shows, and the
    /// add-token sheet is `manage_tokens`. Every other flow is a picture, and
    /// visibly so.
    private func flowModel(_ state: FlowStateId) -> FlowScreenModel {
        var model = WalletFlowFixtures.build(state, loc: loc)
        let address = session.view.address
        // The receive screens first, because they are the ones where a fixture
        // is not embarrassing but dangerous: money sent to the drawn address is
        // money gone.
        if case .receive(let list) = model.base {
            model.base = .receive(FlowsLive.receiveList(address, on: list, loc: loc))
        }
        if case .receiveQr(let qr)? = model.sheet {
            model.sheet = .receiveQr(FlowsLive.receiveQr(
                address, name: session.view.activeName,
                chain: ChainCatalog.chains.indices.contains(receiveNetwork)
                    ? ChainCatalog.chains[receiveNetwork] : nil,
                on: qr, loc: loc
            ))
        }
        if case .assets(let assets) = model.base, let balance = wallet.balance {
            model.base = .assets(FlowsLive.assets(
                balance, currency: settings.currency, on: assets, loc: loc
            ))
        }
        // The history screen the home's 全部 opens — the same feed, not a
        // second one built from another source.
        if case .history(let history) = model.base, let feed = activity.feed {
            model.base = .history(FlowsLive.history(
                feed, selected: chainFilter, on: history, loc: loc,
                hidden: wallet.balance?.hidden ?? false
            ))
        }
        if case .txDetail(let detail)? = model.sheet, let feed = activity.feed,
           let item = selectedItem(in: feed) {
            model.sheet = .txDetail(FlowsLive.txDetail(
                item,
                record: feed.transactions.first { $0.id == item.id },
                on: detail, loc: loc
            ))
        }
        if case .tokenDetail(let detail)? = model.sheet,
           let balance = wallet.balance, balance.tokens.indices.contains(assetRow) {
            model.sheet = .tokenDetail(FlowsLive.tokenDetail(
                balance.tokens[assetRow],
                feed: activity.feed,
                display: WalletLive.Display.from(settings.currency),
                on: detail, loc: loc
            ))
        }
        // The send journey. SD1's rows are the holdings the balance machine
        // already found; SD2 and SD3 are the core's figures, its refusals and
        // its gates.
        if let view = send.view {
            let display = WalletLive.Display.from(settings.currency)
            if case .sendPick(let pick) = model.base {
                model.base = .sendPick(SendLive.pick(view, on: pick, loc: loc))
            }
            if case .sendForm(let form) = model.base {
                model.base = .sendForm(SendLive.form(
                    view, fee: fees.view, display: display, on: form, loc: loc
                ))
            }
            if case .sendReceipt(let receipt) = model.base {
                model.base = .sendReceipt(SendLive.receipt(
                    view, display: display, on: receipt, loc: loc
                ))
            }
            if case .sendConfirm(let confirm) = model.base {
                model.base = .sendConfirm(SendLive.confirm(
                    view, from: (session.view.address, session.view.activeName),
                    display: display, on: confirm, loc: loc
                ))
            }
            if case .feeToken(let sheet)? = model.sheet, let fee = fees.view {
                model.sheet = .feeToken(SendLive.feeSheet(fee, on: sheet, loc: loc))
            }
        }
        // The ERC-20 tab only: the native tab adds a NETWORK, which is
        // `network_admin`'s wizard and not this machine's.
        if case .addToken(let sheet) = model.sheet, sheet.tab == .erc20,
           let view = tokens.view {
            model.sheet = .addToken(FlowsLive.addToken(view, on: sheet, loc: loc))
        }
        return model
    }

    /// The states the send machine owns. Anything else is still a drawing.
    private var sendStates: Set<FlowStateId> {
        [.sd1, .sd1b, .sd2, .sd2b, .sd2c, .sd2d, .sd2e, .sd2f, .sd3, .sd3b, .sd3c, .sd4a, .sd4b, .sd4c]
    }

    /// Open the flow for the signed-in account. Idempotent.
    ///
    /// The account **id** is the founding credential's, and the session view
    /// does not carry it — it carries what a header needs, a name and an
    /// address. So it is read from the store, which is also the only place that
    /// holds the key set the signature is packed against.
    private func openSend() async {
        let record = await accounts.loadAccounts().first {
            ($0["address"] as? String)?.lowercased() == session.view.address.lowercased()
        }
        let display = WalletLive.Display.from(settings.currency)
        send.open(
            accountId: record?["id"] as? String ?? "",
            address: session.view.address,
            name: session.view.activeName.isEmpty ? nil : session.view.activeName,
            displayCode: display.code,
            displayRate: display.rate,
            fiatDecimals: 2
        )
    }

    /// The CTA's gate is the core's, never a conjunction assembled here.
    private func sendCtaDisabled(_ state: FlowStateId) -> Bool {
        guard let view = send.view else { return false }
        switch state {
        case .sd2, .sd2b, .sd2d: return !view.canContinue
        case .sd3, .sd3b, .sd3c: return !view.canConfirm
        default: return false
        }
    }

    /// Render the receive card and put it in the album.
    ///
    /// The card is built here rather than inside the sheet because it needs the
    /// signed-in identity and the chain the person picked — the same two facts
    /// the code on screen is built from, so the image and the screen can never
    /// disagree.
    private func saveShareCard() {
        let card = FlowsLive.shareCard(
            session.view.address,
            name: session.view.activeName,
            chain: ChainCatalog.chains.indices.contains(receiveNetwork)
                ? ChainCatalog.chains[receiveNetwork] : nil,
            on: drawnShareCard,
            loc: loc
        )
        Task {
            saveOutcome = await ShareCardExport.save(
                card, scheme: scheme, scale: UIScreen.main.scale
            )
        }
    }

    /// The drawn card (R4), as the base every live one is built on — its
    /// headline, wordmark and layout are the drawing's.
    private var drawnShareCard: ShareCardModel {
        guard case .share(let card) = WalletFlowFixtures.build(.r4, loc: loc).base else {
            // The fixture cannot lose its own card, but a crash here would be a
            // crash on somebody's receive screen.
            return ShareCardModel(
                headline: loc.t("receive.shareCardHeadline"),
                name: "", lines: [], networkNote: "",
                networkMark: TokenMarkModel(ticker: "", badgeColor: .clear),
                identiconSeed: "", wordmark: "Vela Wallet"
            )
        }
        return card
    }

    /// What the save had to say, if it has said anything yet.
    /// The refusal the core raised, in the core's words.
    private var sendRefusal: FlowAlertModel? {
        guard let kind = send.alert else { return nil }
        let text = SendLive.alertText(kind, loc: loc)
        return FlowAlertModel(title: text.title, message: text.body)
    }

    private var saveAlert: FlowAlertModel? {
        switch saveOutcome {
        case .saved:
            FlowAlertModel(title: loc.t("receive.request.savedTitle"),
                           message: loc.t("receive.request.savedBody"))
        case .denied:
            FlowAlertModel(title: loc.t("receive.request.permTitle"),
                           message: loc.t("receive.request.permBody"))
        case .failed:
            FlowAlertModel(title: loc.t("addToken.errorTitle"),
                           message: loc.t("receive.request.shareError"))
        case nil:
            nil
        }
    }

    /// What 在区块浏览器中查看 opens, for whichever sheet is up.
    ///
    /// `nil` on a chain with no explorer — the button is then absent rather
    /// than sending somebody to another chain's explorer, where they would find
    /// nothing and reasonably conclude their money had vanished.
    private func explorerLink(for state: FlowStateId) -> URL? {
        switch state {
        case .a2, .a3:
            guard let feed = activity.feed, let item = selectedItem(in: feed) else { return nil }
            let hash = item.txHash ?? feed.transactions.first { $0.id == item.id }?.txHash ?? ""
            return ExplorerLinks.tx(chainId: item.chainId, hash: hash, store: shelf)
        case .t2:
            guard let balance = wallet.balance,
                  balance.tokens.indices.contains(assetRow) else { return nil }
            let token = balance.tokens[assetRow]
            // A native coin has no token page; the account's own page is the
            // honest thing to show for it.
            return ExplorerLinks.token(chainId: token.chainId, contract: token.tokenAddress,
                                       store: shelf)
                ?? ExplorerLinks.address(chainId: token.chainId, session.view.address,
                                         store: shelf)
        case .r2, .r3:
            let chainId = ChainCatalog.chains.indices.contains(receiveNetwork)
                ? ChainCatalog.chains[receiveNetwork].chainId : 0
            return ExplorerLinks.address(chainId: chainId, session.view.address, store: shelf)
        default:
            return nil
        }
    }

    /// The feed item behind the tapped history row.
    ///
    /// Resolved against the SAME grouping the screen rendered, so the sheet
    /// cannot open a different transaction than the one that was tapped.
    private func selectedItem(in feed: FeedViewWire) -> FeedItemWire? {
        let groups = WalletLive.activityGroups(
            feed, loc: loc, hidden: wallet.balance?.hidden ?? false
        )
        guard groups.indices.contains(activityRow.group) else { return nil }
        let before = groups[..<activityRow.group].reduce(0) { $0 + $1.rows.count }
        let flat = before + activityRow.row
        let items = FlowsLive.items(feed)
        return items.indices.contains(flat) ? items[flat] : nil
    }

    /// The address field, owned by the core — and only on the sheet that has
    /// a machine behind it.
    ///
    /// The text lives in the machine's model rather than in a `@State` beside
    /// it, so validity, the found cards and the field can never disagree about
    /// what was typed.
    private func addTokenInput(for state: FlowStateId) -> Binding<String>? {
        guard state == .t3 else { return nil }
        return Binding(
            get: { tokens.view?.inputAddress ?? "" },
            set: { tokens.input($0) }
        )
    }

    /// 添加到钱包 — the first card the core found, which is the one drawn.
    private func addTokenAction(for state: FlowStateId) -> (() -> Void)? {
        guard state == .t3 else { return nil }
        return {
            guard let chainId = tokens.view?.found.first?.chainId else { return }
            tokens.save(chainId: chainId)
        }
    }

    private func addTokenError(for state: FlowStateId) -> String? {
        guard state == .t3, let view = tokens.view else { return nil }
        return FlowsLive.saveErrorText(view, loc: loc)
    }

    /// Settings, wearing the signed-in identity and the real networks.
    ///
    /// Two layers are live and the rest is still the spec-023 fixture, visibly
    /// so. The identity, because a fixture name over a real address tells
    /// somebody they are signed in as a stranger (spec 019's finding). The
    /// **networks**, because a settings screen listing somebody else's chains
    /// is the same lie one level down (spec 050).
    @ViewBuilder private func settingsScreen(_ state: SettingsStateId = .st1) -> some View {
        SettingsScreen(
            model: settingsModel(state),
            loc: loc,
            onSelectTab: { tab in
                if tab == .wallet { router.path.removeLast() }
            },
            // The way out of a signed-in wallet, on the row a person would
            // look for it.
            onSignOut: { session.signOut() },
            networkActions: SettingsNetworkActions(
                onOpenNetwork: { settings.expandNetwork(chainId: $0) },
                onSearch: { settings.search($0) },
                onSelectChain: { settings.selectChain($0) },
                onEditCustomRpc: { settings.editCustomRpc($0) },
                onConfirmAdd: { settings.confirmAdd() },
                isLive: true
            )
        )
        .task { settings.open() }
    }

    private func settingsModel(_ state: SettingsStateId) -> SettingsScreenModel {
        let base = SettingsFixtures.build(state, loc: loc)
            .withIdentity(
                name: session.view.activeName,
                address: session.view.address,
                display: Self.shortenAddress(session.view.address)
            )
        // Before a core has ruled there is nothing truthful to swap in, so the
        // drawing stands as drawn — never half-live, and never a fixture list
        // wearing a live pill. The two machines are independent: one can be
        // ready while the other is not.
        var model = base
        if let view = settings.networkAdmin, view.loaded {
            model = SettingsLive.withNetworks(view, on: model, loc: loc)
        }
        if let view = settings.currency {
            model = SettingsLive.withCurrency(view, on: model, loc: loc)
        }
        return model
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

/// Which section of the signed-in shell is showing (spec 022 / spec 050).
///
/// A section, not a route: `docs/design/contacts/C1` draws the tab bar with
/// 通讯录 **selected**, so it is a peer of 钱包 rather than something pushed
/// over it, and a browser tab is not somewhere a person should be able to
/// deep-link into before they have a wallet. 设置 is the opposite case — its
/// drawing has a back affordance — and stays an `AppRoute`.
enum WalletSection { case wallet, contacts, explore }

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
        case settings, settingsLive, settingsGallery, explore, signing
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
        // The settings surface with its LIVE network list — the same purpose
        // `contacts-live` serves, for the other machine in this cut.
        case "settings-live": .settingsLive
        case "flows-gallery": .flowsGallery
        case "settings": .settings
        case "settings-gallery": .settingsGallery
        case "explore": .explore
        case "signing": .signing
        default: nil
        }
    }()

    /// WHICH state the overridden page opens on — `VELA_STATE=e4`,
    /// `VELA_STATE=cs5`, or `VELA_STATE=st9` to put `settings-live` straight
    /// on the networks page, which is otherwise two taps inside a route. The
    /// same env-pin family as `VELA_PAGE`, and the same reason the desktop grew
    /// `VELA_SETTINGS_STATE`: without it a screenshot pass can only ever see
    /// the first state.
    static let state: String? = ProcessInfo.processInfo.environment["VELA_STATE"]
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
