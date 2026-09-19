//
//  RootView.swift
//  VelaWallet
//
//  App-level composition: navigation stack, theme injection, and the
//  welcome content resolved through the localization layer.
//

import AVFoundation
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
    /// The money path, shared with 053's dApp transactions: one relay client,
    /// one account port, one submit spine. A second spine would be a second
    /// set of rules about the same Safe.
    private let relayClient: RelayClient
    private let sendAccountPort: SendAccountPort
    private let userOpSpine: UserOpSpine
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
    /// The home header's account switcher (spec 047's rule, reached here at
    /// last). A flag rather than a route: it is a sheet over the wallet, the
    /// way the rescue overlays are.
    @State private var homeSwitcherOpen = false
    /// The anti-scam core behind the receipt scan — resident, because the
    /// trusted-token set it builds is shared by everything that reads a chain.
    @State private var trust: TokenTrustStore
    @State private var activity: ActivityStore
    @State private var tokens: ManageTokensStore
    @State private var deposits: ReceiveWatchStore
    /// Which network row opened the receive code.
    @State private var receiveNetwork = 0
    /// The identicon viewer, hosted once for the whole app.
    @State private var identiconViewer: IdenticonSubject?
    /// Bumped when storage is cleared, so the measured page re-reads the store.
    @State private var storageTick = 0
    /// Naming a group — new, or renaming the one it names.
    ///
    /// The platform's own prompt, which is the shape the explore tab's
    /// 新建分组 already uses on this client (053). Android draws a bottom
    /// sheet for it; the alert is this client's existing precedent for "ask
    /// for one short string", and no new sheet is invented here.
    @State private var groupNaming: GroupNaming?
    @State private var groupNameDraft = ""
    /// The rescue the hero's status line opened, and what it is about.
    @State private var rescue: SettingsOverlay?
    @State private var rescueChain: Int?
    @State private var rpcDraft = ""
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
    /// The payroll importer (spec 054 US3). Its own machine, beside the send
    /// rather than inside it: parsing and pricing a file is 1,698 lines that
    /// have nothing to do with the send journey, and the two meet at one point
    /// — the applied list of recipients.
    @State private var batch: BatchStore
    /// Money in flight, followed for as long as the app exists.
    @State private var tracker: TrackerStore
    @State private var notifier: TrackerNotifier
    /// The browser (spec 053). App-resident like every other wallet-state
    /// machine, and for a sharper reason than most: a page keeps running, a
    /// request stays in flight, and a dApp that reloaded every time somebody
    /// glanced at their balance would lose a half-finished swap.
    @State private var browser: BrowserController
    /// The request a page is asking about right now. Born with the request
    /// and dropped when the page has its answer — four machines that must not
    /// outlive the question they were asked.
    @State private var signing: SigningController?
    /// Whether this wallet's founding keys are on Ethereum too (spec 062),
    /// and for WHICH wallet that was asked — an answer about the previous
    /// account must not be drawn under the next one's name.
    @State private var backupCheck: (address: String, check: RegistryBackup.Check)?
    /// Which passkeys control that wallet, and for which wallet it was asked.
    @State private var walletKeys: (address: String, result: WalletKeys.Result)?
    /// What the person is typing. Held locally and echoed to the core, which
    /// owns the value: a field bound straight to a machine loses characters on
    /// the round trip (Android found it on the device).
    @State private var recipientDraft = ""
    @State private var amountDraft = ""
    /// The importer's two fields. Local for the same reason every other field
    /// here is: a field bound straight to a machine loses characters on the
    /// round trip.
    @State private var batchPaste = ""
    @State private var batchRate = ""
    @State private var feeSheetOpen = false
    /// Whether the token picker is showing tick boxes.
    ///
    /// **The shell's**, and deliberately so: the core's `multiSelectMode` flips
    /// at CONFIRM, not when the boxes appear, so it cannot answer "are we
    /// picking". Every other question about a sweep — which rows are valuable,
    /// how much of each moves — goes to the core.
    @State private var sweepPicking = false
    /// Which class of token the picker is showing: `all`, `stable`, `gas` or
    /// `other`. The SHELL's, like the sweep's picking flag — the core lists
    /// every holding and which subset is on screen is a render decision.
    @State private var sendClassFilter = "all"
    @State private var sendAlert: (title: String, body: String)?
    @State private var flows: FlowNav
    /// Which section of the signed-in shell is showing (spec 050).
    @State private var section: WalletSection = .wallet
    /// Where the contacts section is, inside itself.
    @State private var contactsRoute: ContactsRoute?
    /// The add/edit form, while it is open. `nil` means no form — the presence
    /// of the draft IS the presence of the sheet, so one state answers both.
    ///
    /// It carries the form's IDENTITY (new, or which contact is being edited)
    /// and the core's refusal. The two typed strings live beside it as plain
    /// state: a field bound through an optional chain into a struct loses
    /// characters as they are typed, and the device showed exactly that —
    /// "Vela 054 探针" arrived as "Va 054 探针" and an address lost its 0x.
    @State private var contactDraft: ContactsLive.ContactDraft?
    @State private var contactName = ""
    @State private var contactAddress = ""
    /// What is being typed into the address book's search field.
    @State private var contactQuery = ""
    /// The membership picker, while it is open. A SET, held here until 保存 —
    /// the core takes a whole membership at once, and a sheet that emitted one
    /// event per tap would leave a half-applied grouping behind if somebody
    /// closed it midway.
    @State private var groupPick: Set<String>?
    @State private var memberPick: Set<String>?
    /// The one document layer: pickers, save panels and the share sheet. Shared
    /// by the payroll importer and the address book, because they are the same
    /// platform affordance asked for twice.
    @State private var documents: UIKitDocumentPorts
    /// The `/pay` link machine (spec 056). Resident: a link can arrive before
    /// the wallet has finished opening, and the verdict has to survive that.
    @State private var paymentRequest: PaymentRequestStore
    /// What this person chose about how the app looks and counts (spec 056).
    /// No machine owns it — storage keys with a reader, on the same spellings
    /// every client writes.
    @State private var preferences: Preferences
    /// The camera behind the scanner (spec 055). App-resident so the session
    /// survives the surface's own rebuilds — reconfiguring it drops frames for
    /// a beat, which on a viewfinder reads as the camera stuttering.
    @State private var camera = CameraScanner()
    /// What the photo library had to say, when it had nothing.
    @State private var scanNotice: (title: String, body: String)?
    /// Whether the app is inside the parallel space, for the badge.
    ///
    /// Mirrored into view state rather than read from the hook on every render:
    /// the hook is the authority, and a `@State` copy is what makes SwiftUI
    /// redraw when the door opens during `.task`.
    @Environment(\.scenePhase) private var scenePhase
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
        _session = State(initialValue: session)
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
        // Through the three layers (064): the index's answers are PROVED against
        // the registry contract, and the contract answers when the index cannot.
        let onboarding = OnboardingModel(
            session: session, store: store,
            registry: RegistryClient(resolver: RegistryResolver(ethCall: { [pool] chainId, to, data in
                let outcome = await pool.call(
                    chainId: chainId, method: "eth_call",
                    params: [["to": to, "data": data], "latest"]
                )
                guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x")
                else { return nil }
                return hex
            }))
        )
        _onboarding = State(initialValue: onboarding)
        // ONE name resolver for the whole app: the address book and the
        // activity feed ask the same question about the same addresses, and two
        // resolvers would mean two caches and two names for one person.
        let identity = RecipientIdentity(store: shelf, pool: pool, accounts: store)
        // The money path. One relay client, one spine, one fee session — the
        // spine is shared with 053's dApp transactions, which is why it is
        // built here rather than inside the send store.
        let relay = RelayClient(port: PoolRelayPort(pool: pool))
        self.relayClient = relay
        let port = SendAccountPort(accounts: store)
        self.sendAccountPort = port
        let spine = UserOpSpine(
            relay: relay,
            accounts: port,
            signer: { ParallelSpaceHook.signer(passkey: PasskeyExecutor()) },
            measureCall: { chainId, from, to, valueHex, data in
                let outcome = await pool.call(
                    chainId: chainId, method: "eth_estimateGas",
                    params: [["from": from, "to": to, "value": valueHex, "data": data]]
                )
                guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x") else { return nil }
                return hex
            }
        )
        self.userOpSpine = spine
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
        let activityStore = ActivityStore(
            store: shelf, accounts: store, held: held, trust: trust, identity: identity
        )
        _activity = State(initialValue: activityStore)
        // A saved token has to reach the balances, so the core's
        // "invalidate the token cache" becomes a re-read here — there is no
        // cache on this client, only a fetch.
        let wallet = WalletStore(store: shelf, pool: pool, held: held)
        _wallet = State(initialValue: wallet)
        _tokens = State(initialValue: ManageTokensStore(
            store: shelf, pool: pool,
            onInvalidate: { [weak wallet] in wallet?.refresh(pull: false) }
        ))
        // The tracker, before the send machine that hands off to it.
        let notify = TrackerNotifier(loc: loc)
        _notifier = State(initialValue: notify)
        let trackerExecutor = TrackerExecutor(
            store: shelf, relay: relay,
            ports: TrackerExecutor.Ports(
                notifyConfirmed: { [weak notify] hash, chain, tx in
                    notify?.confirmed(userOpHash: hash, chainId: chain, txHash: tx)
                },
                // The authentic receipt logs, to the ONE entry point that may
                // admit a token. A sign-time simulation never may.
                receiptLogs: { [weak trust] from, chain, logs in
                    trust?.receiptLogsConfirmed(from: from, chainId: chain, logs: logs)
                },
                recordsPatched: { [weak activityStore] in activityStore?.reconciled() }
            )
        )
        let trackerStore = TrackerStore(executor: trackerExecutor)
        _tracker = State(initialValue: trackerStore)
        // The browser. Its reads go through the SAME pool the wallet uses, so
        // a page asking a chain something gets this person's endpoints, their
        // bans and their cooldowns — never an endpoint the page named.
        let browserController = BrowserController(store: shelf)
        browserController.ports = BrowserController.Ports(
            knownChains: { [weak settingsStore] in
                settingsStore?.networkAdmin?.networks.map(\.chainId) ?? []
            },
            poolCall: { [weak pool] chainId, method, params, bundler in
                guard let pool else { return nil }
                switch await pool.call(
                    chainId: chainId, method: method, params: params,
                    kind: bundler ? "bundler" : "rpc"
                ) {
                case .ok(let body):
                    return ["result": body ?? NSNull()]
                case .rpcError(let code, let message):
                    // The node's own sentence, verbatim. A page that shows its
                    // user "execution reverted: insufficient allowance" is a
                    // page that can be debugged; one that shows "-32603" is not.
                    return ["error": ["code": code ?? -32603, "message": message]]
                default:
                    return nil
                }
            },
            writeRecords: { [weak activityStore] rows in
                TxRecords.writeRecords(rows, store: shelf)
                activityStore?.reconciled()
            }
        )
        _browser = State(initialValue: browserController)
        // The send machine, last: it reads the holdings the balance machine
        // found and asks the fee session for a quote, so both must exist.
        let metadata = TokenMetadata(store: shelf, pool: pool)
        let sendExecutor = SendExecutor(
            store: shelf, relay: relay, pool: pool, spine: spine, accounts: port,
            fees: feeStore, identity: identity, metadata: metadata, accountStore: store,
            balances: { [weak wallet] in wallet?.balance },
            networks: { [weak settingsStore] in settingsStore?.networkAdmin },
            ports: SendExecutor.Ports(
                // A send the relay accepted is the tracker's from that moment.
                // The permission is asked HERE — at the first submit, never at
                // launch — because this is the first time there is anything to
                // notify about.
                trackSubmitted: { [weak trackerStore, weak notify] hash, ids, chain in
                    notify?.askOnceIfNeeded()
                    trackerStore?.submitted(userOpHash: hash, recordIds: ids, chainId: chain)
                },
                // The core's own exit. `Done` on a receipt, and `close` on any
                // refusal that ends the attempt, both land here.
                closed: { [weak flowNav] in flowNav?.close() },
                // Leaving is what re-arms `Open`. Without it a second visit to
                // 转账 would render the machine's last state instead of a
                // fresh picker.
                refreshBalances: { [weak wallet] in wallet?.refresh(pull: false) }
            )
        )
        let sendStore = SendStore(executor: sendExecutor)
        _send = State(initialValue: sendStore)
        // The tracker's verdict, back to the SEND machine — the other half of
        // this wallet watching the same operation.
        //
        // Installed here rather than in the tracker's own initialiser because
        // the tracker is built FIRST (the send machine hands off to it), which
        // is the same ordering `SendStore` solves for its own two ports.
        //
        // Without this the receipt screen sat on "submitted" while the
        // notification said "confirmed": two answers about one payment, on one
        // phone.
        trackerExecutor.ports.notifyConfirmed = { [weak notify, weak sendStore] hash, chain, tx in
            notify?.confirmed(userOpHash: hash, chainId: chain, txHash: tx)
            sendStore?.receiptConfirmed(userOpHash: hash, txHash: tx)
        }
        // The payroll importer. Its fiat column is priced through the DISPLAY
        // machine's own waterfall — chain feed, then endpoint, then nothing —
        // so a currency the wallet cannot price stays unpriced here too. A
        // fallback of 1 would pay out the fiat figure in tokens.
        let documentPorts = UIKitDocumentPorts()
        _documents = State(initialValue: documentPorts)
        // Read before the first frame: a theme or a text size adopted one
        // render late is a visible flash of the wrong one.
        _paymentRequest = State(initialValue: PaymentRequestStore(
            executor: PaymentRequestExecutor(store: shelf)
        ))
        let prefs = Preferences(store: shelf)
        prefs.boot()
        // Before the first frame, and before the welcome model is built from
        // it: the stored language decides which words this launch uses. Until
        // 058 nothing read `vela.language` at all.
        loc.apply(prefs.language)
        // Which chain-data endpoint the logos come from (058). The person's
        // own endpoint wins; the default is what every other read uses, so a
        // wallet that can fetch balances can fetch the pictures beside them.
        Marks.adopt(accounts.loadServiceEndpoints())
        Formats.apply(prefs)
        UiScale.apply(prefs)
        AvatarPreference.apply(prefs)
        _preferences = State(initialValue: prefs)
        _batch = State(initialValue: BatchStore(executor: BatchExecutor(
            fiatRate: { [weak settingsStore] code in await settingsStore?.usdRate(code) },
            documents: { documentPorts }
        )))
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
        // The launch pin (a screenshot sweep) outranks everything; then the
        // person's own choice; then the OS. `system` pins NOTHING — that is the
        // whole meaning of the choice, and a resolved "dark" would stop
        // following an OS that changes at sunset.
        ThemeOverride.launchScheme ?? chosenScheme ?? systemScheme
    }

    private var chosenScheme: ColorScheme? {
        switch preferences.theme {
        case .light: .light
        case .dark: .dark
        case .system: nil
        }
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
        // Observation, not decoration.
        //
        // The formatters are STATICS — `Formats.current`, `UiScale` — and a
        // static cannot invalidate a view. Every model this body builds reads
        // them, so unless the body itself depends on the CHOICES, a new number
        // format ticks its own settings row and leaves the wallet's figures in
        // yesterday's shape. Reading them here is that dependency, and it is
        // deliberately not an `.id(…)`: re-identifying the root would tear the
        // whole tree down and take every sheet, flow and scroll position with
        // it. (049's lesson, on this client.)
        _ = preferences.numberFormat
        _ = preferences.dateFormat
        _ = preferences.timeFormat
        _ = preferences.textScale
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
        // Every avatar in the app draws from here, so a change to the choice
        // invalidates them — the static alone changed nothing on screen.
        .environment(\.avatarStyle, preferences.avatarStyle)
        .preferredColorScheme(ThemeOverride.launchScheme ?? chosenScheme)
        // A link, from anywhere: the scheme, a universal link, a page.
        .onOpenURL { url in openLink(url) }
        // The core's verdict on a `/pay` link. Watched rather than awaited,
        // because a link can arrive before the wallet has finished opening and
        // the answer has to survive that.
        .onChange(of: paymentRequest.view?.pay) { _, request in
            guard let request else { return }
            prefillSend(from: request)
        }
        // Every avatar in the app opens the viewer (row 10 of 057's audit).
        //
        // Provided ONCE, here, and reached by the avatar component itself —
        // Android's `LocalIdenticonViewer` arrangement. The alternative, a
        // callback threaded through twelve call sites, is how the viewer came
        // to open from the wallet header and nowhere else: eleven sites had
        // nothing to thread.
        .environment(\.identiconViewer, { seed, name in
            identiconViewer = IdenticonSubject(seed: seed, name: name)
        })
        // 新建分组 / 重命名分组.
        .alert(groupNaming?.title ?? "", isPresented: Binding(
            get: { groupNaming != nil },
            set: { if !$0 { groupNaming = nil } }
        )) {
            TextField(groupNaming?.title ?? "", text: $groupNameDraft)
            Button(loc.t("contacts.cancel"), role: .cancel) { groupNaming = nil }
            Button(loc.t("contacts.save")) {
                let name = groupNameDraft.trimmingCharacters(in: .whitespacesAndNewlines)
                let target = groupNaming
                groupNaming = nil
                groupNameDraft = ""
                // An empty name is not a group. The core would refuse it, and
                // asking it to is how a blank row appears in a list.
                guard !name.isEmpty else { return }
                contacts.saveGroup(id: target?.id, name: name)
            }
        }
        .sheet(item: $identiconViewer) { subject in
            IdenticonViewerSheet(
                loc: loc,
                address: subject.seed,
                name: subject.name,
                onClose: { identiconViewer = nil }
            )
            // `.large`, not `.medium`: the content is a big circle, a
            // title, three lines of caption, a wrapped 42-character address
            // and two buttons — taller than half a phone, so `.medium` put
            // 关闭 under the bottom edge and the way OUT of the sheet was the
            // part you could not reach (founder, 2026-09-16). The ScrollView
            // inside covers the rest: a longer translation or a larger text
            // size cannot push anything out of reach again.
            .presentationDetents([.large])
            .themed(scheme)
        }
    }

    /// Which group is being named. `id == nil` is a new one.
    struct GroupNaming: Identifiable, Equatable {
        let id: String?
        let current: String
        var title: String
    }

    /// Whose artwork the viewer is showing. A value, not a pair of `@State`s,
    /// so `sheet(item:)` can key the presentation on it.
    struct IdenticonSubject: Identifiable, Equatable {
        let seed: String
        let name: String?
        var id: String { seed }
    }

    // MARK: - Deep links (spec 056 US3)

    /// A link this app was opened with.
    ///
    /// Two shapes act, and everything else is ignored in silence — an app that
    /// showed an error for a link it does not handle would be an app that can
    /// be made to say things by anybody with a URL.
    private func openLink(_ url: URL) {
        switch PayLink.parse(url.absoluteString) {
        case .pay:
            guard let event = PayLink.parse(url.absoluteString)?.linkOpened else { return }
            // The CORE validates. Seven strings and whether they add up to a
            // request this wallet can honour is 600 lines that already exist.
            paymentRequest.linkOpened(event)
        case .open(let page):
            section = .explore
            browser.open(page)
        case nil:
            break
        }
    }

    /// A `/pay` link the core accepted, as a send with the fields already in.
    ///
    /// The amount rides only when the link named one: an OPEN request is
    /// somebody asking to be paid without saying how much, and filling in a
    /// figure there would be the wallet inventing the ask.
    private func prefillSend(from request: PayRequestWire) {
        section = .wallet
        flows.enter(.send)
        Task {
            await openSend()
            // The link's own `ethereum:` URI, through the SAME door a scanned
            // code uses — so a link and a QR code of the same request land on
            // exactly the same screen.
            send.scanned(request.eip681Uri)
        }
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
                // At LAUNCH, not with a screen: what the tracker follows
                // outlives every screen. The pending set is derived from the
                // transaction store, so a force-quit mid-send loses nothing —
                // the next launch picks it up here.
                tracker.boot()
            }
            .onChange(of: scenePhase) { _, phase in
                switch phase {
                case .active: tracker.foregrounded()
                case .background: tracker.backgrounded()
                default: break
                }
            }
            .onChange(of: notifier.pendingReceipt) { _, hash in
                // A tapped notification opens the transaction it is about. It
                // waits for a wallet to exist: a cold-start tap arrives before
                // the session machine has ruled on where the app may be.
                guard hash != nil, session.view.allowedRoute == .wallet else { return }
                section = .wallet
                flows.enter(.activity)
                notifier.clearPendingReceipt()
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
                flowScreen(state)
                .transition(.move(edge: .trailing))
                // The field holds what is typed and the core holds the value:
                // a field bound straight to a machine loses characters on the
                // round trip. The echo back is ignored while the person types.
                .onChange(of: batchPaste) { _, value in batch.setText(value) }
                .onChange(of: batchRate) { _, value in batch.editRate(value) }
                // A PICKED file writes the paste box — the core puts the file's
                // text where the typed text goes, and the field has to show it.
                // Guarded on inequality so the echo of what is being typed does
                // not land back in the field mid-word.
                .onChange(of: batch.view.rawText) { _, value in
                    if value != batchPaste { batchPaste = value }
                }
                .onChange(of: batch.view.rateInput) { _, value in
                    if value != batchRate { batchRate = value }
                }
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
                // A quote has a TTL, and somebody reading the confirm page can
                // outlast it. When it expires, ask again — once per expiry,
                // only while the page is open and nothing else is happening.
                //
                // The core keeps the request that was priced, so `requote`
                // re-runs THAT one; the fresh estimate flows back through the
                // handler above. Asking during a submit or a passkey prompt
                // would move the fee under a signature already being made.
                .onChange(of: fees.view?.stale) { _, stale in
                    guard stale == true, fees.view?.busy == false,
                          let view = send.view, view.stage == .confirm,
                          view.txStatus == "idle", !view.sending
                    else { return }
                    fees.requote()
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
                    // The activity LIST polls at its own faster cadence while
                    // it is the screen somebody is looking at — the core has a
                    // separate event for it and this client sent only the
                    // home's slower one.
                    if state == .a1 || state == .a2 {
                        activity.startLiveTicking()
                    } else {
                        activity.stopLiveTicking()
                    }
                    // The viewfinder runs only while it is on screen. A camera
                    // left running behind another screen is a light nobody
                    // asked for and a battery nobody budgeted.
                    if state == .s1 {
                        camera.onScan = { text in scannedCode(text) }
                        send.openScanner()
                        await camera.start()
                    } else {
                        camera.stop()
                    }
                    if state == .t3 { tokens.open() }
                    // The watcher runs while a code is on screen — five
                    // minutes, at the core's own cadence, and it stops itself.
                    if state == .r2 || state == .r3 {
                        deposits.open(address: session.view.address)
                    }
                    // The request machine learns whose address this is when the
                    // receive flow opens, exactly as Android's `openReceive`
                    // does. Every question the code sheet asks it — which
                    // asset, which precision — is answered against this.
                    if state == .r1 {
                        paymentRequest.start(
                            account: session.view.address,
                            recipient: session.view.address,
                            baseUrl: Self.payLinkBase
                        )
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
                .task(id: drawn) {
                    // Keyed on the DRAWN state, because the fee sheet is the
                    // shell's and not the core's: `send`'s stage stays
                    // `enter_details` while it is up.
                    //
                    // Deriving it from `state` was circular — `flowState` only
                    // answers `.sd2f` when this flag is already true, so the
                    // flag could never become true and the sheet never opened.
                    feeSheetOpen = (drawn == .sd2f)
                    // Android's trap, avoided: its contacts machine only opened
                    // on the contacts page, so the picker was empty. `boot` is
                    // idempotent, so opening it from here costs nothing.
                    if drawn == .sd2e { contacts.open(myAddress: session.view.address) }
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
                        onStatusTap: { openRescue() },
                        // The name line's chevron has drawn a disclosure since
                        // spec 015 and led nowhere on this screen until now.
                        onOpenAccounts: {
                            openAccountSwitcher()
                            homeSwitcherOpen = true
                        },
                        onRefresh: RefreshAction {
                            // `pull: true` is carried so the core can tell a
                            // person's own gesture from the 30-second tick and
                            // answer it differently.
                            wallet.refresh(pull: true)
                            activity.focusTick()
                            await wallet.settled()
                        }
                    )
                    // The hero's status line, as a sheet over the wallet —
                    // which is what SR2 and SR3 are drawn as. The settings
                    // route would put the settings list behind a sentence
                    // about the screen somebody was actually on.
                    .sheet(item: $rescue) { overlay in
                        SettingsSheet(
                            model: rescueModel(overlay),
                            overlay: overlay,
                            onDismiss: { rescue = nil },
                            onSignOut: {},
                            rpcDraft: $rpcDraft,
                            onCommitRpc: { commitRescueRpc() },
                            onRetryChain: { _ in wallet.refresh(pull: true) }
                        )
                        .themed(scheme)
                    }
                    // ONE switcher, wherever it is opened from: the same
                    // sheet the settings page shows, over the same live model,
                    // so the two can never drift into showing different
                    // accounts.
                    .sheet(isPresented: $homeSwitcherOpen) {
                        SettingsSheet(
                            model: settingsModel(.st1),
                            overlay: .accounts,
                            onDismiss: { closeHomeSwitcher() },
                            onSignOut: {},
                            onSelectAccount: { address in
                                switchToAccount(address)
                                closeHomeSwitcher()
                            },
                            onAccountCreate: {
                                closeHomeSwitcher()
                                router.path.append(.create)
                            },
                            onAccountSignIn: {
                                closeHomeSwitcher()
                                onboarding.showSignInMethods = true
                            }
                        )
                        .themed(scheme)
                    }
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
                    // Spec 053: 探索 is a browser. Its start page is this
                    // person's own favourites, groups and recents, its tab
                    // strip is the core's, and its pages are real. The
                    // account a site is shown is the REAL one — a connection
                    // panel naming a stranger's account would be the wallet
                    // lying about what it just granted.
                    ExploreScreen(
                        model: ExploreLive.home(
                            explore: browser.explore,
                            history: browser.history,
                            permissions: browser.permissions,
                            engine: browser.current,
                            identity: (name: session.view.activeName,
                                       address: session.view.address),
                            chainId: browser.browserChain,
                            loc: loc
                        ),
                        loc: loc,
                        signing: SigningFixtures.build(.cs12, loc: loc)
                            .withIdentity(name: session.view.activeName,
                                          address: session.view.address),
                        signingLive: signing.map { signingModel(for: $0) },
                        onSigningConfirm: { signing?.approve() },
                        // **Every chip on the editor is a PRESET, 撤销 included.**
                        //
                        // The core has a separate `revoke_chosen`, and it
                        // belongs to the BOOLEAN card — `setApprovalForAll`,
                        // a DAI permit — where there is no amount to cap and
                        // the choice is yes or no. Sending it from the amount
                        // editor's chip is an event the editor does not
                        // answer, and the chip silently does nothing.
                        // Device-found: 撤销 was tapped, the sheet kept saying
                        // 无限额, and the slide stayed shut.
                        onAllowanceChip: { chip in signing?.guardPreset(chip) },
                        onAllowanceAmount: { text in signing?.guardCustomAmount(text) },
                        onSignWith: { id in signing?.signWith(id) },
                        onSigningDismissed: { signing?.swipeDismissed() },
                        controller: browser,
                        onSelectTab: selectTab
                    )
                    .onChange(of: signing?.closed) { _, closed in
                        // The page has its answer and the core cleared the
                        // sheet. Dropping the controller is what makes the
                        // next request start from nothing rather than
                        // inheriting a decoded intent or a half-edited cap.
                        if closed == true { signing = nil }
                    }
                    .onDisappear {
                        // Leaving 探索 settles every request the page left
                        // hanging — as **unknown-pending**, never as a
                        // refusal: nobody declined anything, and a page told
                        // 4001 would show its user "you rejected this" when
                        // they did not.
                        //
                        // On the section change rather than on a view's
                        // `onDisappear` alone would be safer still; this one
                        // fires for the tab switch and nothing else, because
                        // the signing sheet is presented OVER this screen and
                        // leaves the section where it is.
                        browser.close()
                    }
                    .task {
                        browser.ports.onSignRequest = { request in
                            openSigning(request)
                        }
                        browser.start()
                        browser.accountsChanged(
                            addresses: accounts.loadAccounts().compactMap { $0["address"] as? String },
                            active: session.view.address
                        )
                        // The device harness opens its own page. Not a product
                        // affordance: a browser that launched a URL somebody
                        // else chose is a browser nobody should install.
                        if let url = PageOverride.browserURL { browser.open(url) }
                    }
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
    /// A page asked for a signature.
    ///
    /// A **second** request while one is open is refused rather than queued —
    /// the permissions machine's `consent_busy` rule, applied to signing. Two
    /// sheets over one page is a person answering the wrong question.
    private func openSigning(_ request: BrowserController.SignRequest) {
        openSigningRequest(
            SigningController.Incoming(
                id: request.id,
                method: request.method,
                paramsJson: request.paramsJson,
                origin: request.origin,
                transportId: request.transportId,
                chainId: request.chainId
            ),
            respond: { [browser] transportId, id, json in
                browser.answerFromSigning(transportId: transportId, id: id, json: json)
            }
        )
    }

    /// The transport of a request the WALLET made of itself. Nothing is
    /// listening on it: an answer addressed here must never reach a page.
    private static let walletTransport = "wallet"

    /// The wallet's own request to copy its founding keys to Ethereum (spec
    /// 062), through the same sheet a page's request gets — the person reads
    /// what it is, sees the fee, and slides. There is no page to answer, so the
    /// answer goes nowhere; the settings row re-reads the chain when it closes.
    private func openEthereumBackup(_ call: RegistryBackup.Call) {
        let tx: [String: Any] = [
            "from": session.view.address, "to": call.to, "value": "0x0", "data": call.data,
        ]
        guard let params = try? JSONSerialization.data(withJSONObject: [tx]) else { return }
        openSigningRequest(
            SigningController.Incoming(
                id: "backup-\(UUID().uuidString)",
                method: "eth_sendTransaction",
                paramsJson: String(decoding: params, as: UTF8.self),
                origin: "https://getvela.app",
                transportId: Self.walletTransport,
                chainId: call.chainId
            ),
            respond: { _, _, _ in }
        )
    }

    private func openSigningRequest(
        _ incoming: SigningController.Incoming,
        respond: @escaping (String, String, [String: Any]) -> Void
    ) {
        guard signing == nil else {
            respond(
                incoming.transportId, incoming.id,
                BrowserExecutor.errorJson(
                    id: incoming.id, code: -32002, message: "Another request is open"
                )
            )
            return
        }
        let record = accounts.loadAccounts().first {
            ($0["address"] as? String)?.caseInsensitiveCompare(session.view.address) == .orderedSame
        }
        let credentialId = ((record?["keys"] as? [[String: Any]])?.first?["credential_id"] as? String)
            ?? (record?["credential_id"] as? String) ?? ""

        let controller = SigningController(
            wallet: (address: session.view.address, credentialId: credentialId),
            relay: relayClient,
            accounts: sendAccountPort,
            spine: userOpSpine,
            store: shelf,
            pool: pool,
            ports: SigningController.Ports(
                respond: respond,
                trackSubmitted: { [tracker, notifier] hash, ids, chain in
                    notifier.askOnceIfNeeded()
                    tracker.submitted(userOpHash: hash, recordIds: ids, chainId: chain)
                },
                recordsPersisted: { [activity] in activity.reconciled() },
                nativeSymbol: { chainId in ChainCatalog.meta(chainId)?.nativeSymbol ?? "" },
                knownChains: { [settings] in
                    settings.networkAdmin?.networks.map(\.chainId) ?? []
                },
                // The base the settings machine names, with the shipped
                // default behind it — the same resolution `ChainTokens` and
                // `RpcEndpoints` use, so a person's own endpoint reaches the
                // signing sheet's descriptors too.
                dataBase: { [accounts] in
                    (accounts.loadServiceEndpoints()["ethereumDataURL"] as? String)
                        .flatMap { $0.isEmpty ? nil : $0 } ?? NetDefaults.ethereumDataURL
                },
                // The simulated deltas, to the machine that JUDGES them. The
                // same machine the confirmed receipts go to, and it keeps the
                // two apart: a receipt may admit a token, a simulation never
                // may (spec 017, invariant ⑤).
                simDeltas: { [trust] address, chainId, deltas in
                    trust.simDeltasComputed(address: address, chainId: chainId, deltas: deltas)
                }
            )
        )
        // The spine is shared with Send; it reads THIS request's "Sign with"
        // choice, and goes back to `auto` the moment the controller is dropped.
        userOpSpine.signMethod = { [weak controller] in controller?.signMethod ?? "auto" }
        signing = controller
        controller.open(incoming)
    }

    // MARK: - Split (spec 054)

    /// The split's row bindings, or `nil` where the form is a picture.
    ///
    /// Extracted rather than written at the call site: `FlowHost` already has
    /// enough arguments that one more ternary tips the type-checker over
    /// (052's lesson, hit again here).
    private func splitRows(
        for state: FlowStateId
    ) -> ((String) -> (address: Binding<String>, amount: Binding<String>))? {
        guard sendStates.contains(state) else { return nil }
        return splitRowBinding
    }

    /// One row's two live fields, by the core's row id.
    ///
    /// Each keystroke sends the **whole list** back, because that is the
    /// event the core has: `recipients_changed` reconciles ids, names and
    /// identities, and a delta would be the shell deciding which parts of its
    /// own state the core may trust. Untouched rows come back byte-identical,
    /// which is what makes that cheap (`SplitRows`).
    private func splitRowBinding(
        _ rowId: String
    ) -> (address: Binding<String>, amount: Binding<String>) {
        let rows = send.view.map(SplitRows.drafts) ?? []
        let row = rows.first { $0.id == rowId }
        return (
            address: Binding(
                get: { row?.address ?? "" },
                set: { send.recipientsChanged(SplitRows.addressEdited(rows, id: rowId, address: $0)) }
            ),
            amount: Binding(
                get: { row?.amount ?? "" },
                set: { send.recipientsChanged(SplitRows.amountEdited(rows, id: rowId, amount: $0)) }
            )
        )
    }

    /// 「+ 添加收款人」 — the door from a single send into a split.
    ///
    /// **This button shipped doing nothing.** It pushed a state the live
    /// router could never render, so the screen re-drew as the one the person
    /// was already on (survey, 054). From a single send it enters split mode;
    /// from a split it appends a row.
    private func addSplitRow() {
        guard let view = send.view else { return }
        guard view.splitMode else {
            send.enterSplitMode()
            return
        }
        send.recipientsChanged(SplitRows.appended(SplitRows.drafts(from: view)))
    }

    private func removeSplitRow(at index: Int) {
        guard let view = send.view, view.recipients.indices.contains(index) else { return }
        let rows = SplitRows.drafts(from: view)
        send.recipientsChanged(SplitRows.removed(rows, id: rows[index].id))
    }

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
                            model: ContactsLive.detail(
                                contact, view: view,
                                // This device's own record of what passed
                                // between the two of you — the same store the
                                // feed reads, narrowed to one address.
                                records: TxRecords.load(store: shelf), loc: loc,
                                form: contactForm(), groupPick: groupPick
                            ),
                            onBack: { contactsRoute = nil },
                            // The pencil shipped doing nothing (survey, 054).
                            onEdit: {
                                contactName = contact.name ?? ""
                                contactAddress = contact.address
                                contactDraft = ContactsLive.ContactDraft(
                                    editing: contact.address,
                                    name: contactName,
                                    address: contactAddress
                                )
                            },
                            onFavourite: { contacts.toggleFavorite(address: contact.address) },
                            // 删除联系人 at the foot of the page shipped doing
                            // nothing too — the row swipe was the only way out.
                            onDelete: {
                                contacts.delete(address: contact.address)
                                contactsRoute = nil
                            },
                            formName: $contactName,
                            formAddress: $contactAddress,
                            onSaveForm: { saveContactDraft() },
                            onCancelForm: { contactDraft = nil },
                            onOpenGroups: {
                                groupPick = ContactsLive.groupsHolding(
                                    contact.address, in: view
                                )
                            },
                            onToggleGroup: { id in
                                guard var picked = groupPick else { return }
                                if picked.contains(id) { picked.remove(id) } else { picked.insert(id) }
                                groupPick = picked
                            },
                            onSaveGroups: {
                                if let picked = groupPick {
                                    contacts.setContactGroups(
                                        address: contact.address, groupIds: Array(picked).sorted()
                                    )
                                }
                                groupPick = nil
                            },
                            onCancelGroups: { groupPick = nil },
                            // 转账 to this person, with the recipient already
                            // in — the card said so and did nothing until 057.
                            onSendTo: { sendToContact(contact.address) },
                            // 收款 and 二维码 are about the WALLET's own
                            // address, so they go where that lives rather than
                            // pretending to be about the contact.
                            onReceive: {
                                section = .wallet
                                flows.enter(.receive)
                            },
                            onShowQr: {
                                section = .wallet
                                flows.enter(.receive)
                            }
                        )
                        // Opening somebody's page asks the core about their
                        // address: is it a contract, and has this wallet ever
                        // paid it. The answer lands in `view.recipient`.
                        .task(id: contact.address) {
                            contacts.inspect(
                                address: contact.address,
                                // The chain the browser is on, because that is
                                // the chain a page would be paying on. With no
                                // page open it is Gnosis — recorded as a
                                // choice, not a fact about the address.
                                chainId: browser.browserChain
                            )
                        }
                    } else {
                        contactsHome(view)
                    }
                case .group(let id):
                    if let group = contacts.group(id: id) {
                        GroupDetailScreen(
                            model: ContactsLive.group(
                                group, view: view, loc: loc, memberPick: memberPick
                            ),
                            onBack: { contactsRoute = nil },
                            onOpenMember: { member in
                                contactsRoute = .detail(address: member.addressFull)
                            },
                            onDeleteGroup: {
                                contacts.deleteGroup(id: group.id)
                                // Back to the list: the page this was is gone.
                                contactsRoute = nil
                            },
                            onOpenMembers: {
                                memberPick = Set(group.members.map(\.address))
                            },
                            onToggleMember: { address in
                                guard var picked = memberPick else { return }
                                if picked.contains(address) {
                                    picked.remove(address)
                                } else {
                                    picked.insert(address)
                                }
                                memberPick = picked
                            },
                            onSaveMembers: {
                                if let picked = memberPick {
                                    // The WHOLE membership, replaced — which is
                                    // what the sheet asked about. `add_group_`
                                    // `members` is a union and could not remove
                                    // anybody the person just unticked.
                                    contacts.setGroupMembers(
                                        id: group.id, members: Array(picked).sorted()
                                    )
                                }
                                memberPick = nil
                            },
                            onCancelMembers: { memberPick = nil },
                            // 群发转账 — the group's people become a split,
                            // seeded straight into the send machine.
                            onBatchSend: { sendToGroup(group) },
                            onImportIntoGroup: { importContacts(intoGroup: group.id) },
                            onExportGroup: { exportContacts(groupId: group.id) },
                            onRenameGroup: {
                                groupNameDraft = group.name
                                groupNaming = GroupNaming(
                                    id: group.id, current: group.name,
                                    title: loc.t("contacts.groupRename")
                                )
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
            model: ContactsLive.home(
                view, loc: loc,
                query: contactQuery.isEmpty ? nil : contactQuery,
                form: contactForm()
            ),
            onOpenContact: { contactsRoute = .detail(address: $0.addressFull) },
            onOpenGroup: { row in
                // The row model carries a display name, not the core's id — so
                // the id is looked up here rather than smuggled through the
                // drawing.
                if let group = view.groups.first(where: { $0.name == row.name }) {
                    contactsRoute = .group(id: group.id)
                }
            },
            onNewGroup: {
                groupNameDraft = ""
                groupNaming = GroupNaming(id: nil, current: "",
                                          title: loc.t("contacts.groupNew"))
            },
            onSelectTab: selectTab,
            onDelete: { contacts.delete(address: $0) },
            onAdd: {
                contactName = ""
                contactAddress = ""
                contactDraft = ContactsLive.ContactDraft(editing: nil, name: "", address: "")
            },
            onImport: { importContacts() },
            onExport: { exportContacts() },
            searchText: $contactQuery,
            onClearSearch: { contactQuery = "" },
            onAcknowledge: { contacts.importAcknowledged() },
            formName: $contactName,
            formAddress: $contactAddress,
            onSaveForm: { saveContactDraft() },
            onCancelForm: { contactDraft = nil }
        )
    }

    /// 保存 on the contact form.
    ///
    /// The CORE validates. This hands over what was typed and closes the form
    /// only if the book actually gained or changed the contact — so a refusal
    /// leaves the form open with the words still in it, rather than swallowing
    /// somebody's typing and showing them an unchanged list.
    /// The form as it stands: its identity and refusal from the draft, its two
    /// values from the fields being typed into.
    private func contactForm() -> ContactsLive.ContactDraft? {
        guard var draft = contactDraft else { return nil }
        draft.name = contactName
        draft.address = contactAddress
        return draft
    }

    private func saveContactDraft() {
        guard let draft = contactDraft else { return }
        let address = draft.editing ?? contactAddress.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !address.isEmpty else { return }
        contacts.save(address: address, name: contactName)
        // The core is synchronous through the bridge: by the time this returns
        // the view has the contact, or it does not.
        if contacts.contact(at: address.lowercased()) != nil
            || contacts.contact(at: address) != nil {
            contactDraft = nil
        } else {
            contactDraft?.error = loc.t("contacts.invalidAddress")
        }
    }

    /// 从文件导入 — a picked file's TEXT, straight to the core.
    ///
    /// The shell does not parse it. The core sniffs JSON from CSV, refuses a
    /// bad file before any write, and applies existing-wins; a shell that
    /// pre-parsed would be a second, disagreeing reader of the same file.
    private func importContacts(intoGroup: String? = nil) {
        Task {
            guard let picked = await documents.pick(types: DocumentTypes.addressBook) else { return }
            contacts.importFile(
                content: String(decoding: picked.bytes, as: UTF8.self),
                filename: picked.name,
                intoGroup: intoGroup
            )
        }
    }

    /// 转账 from a token's own sheet.
    ///
    /// The token the sheet is ABOUT, preselected — the core's `select_token`,
    /// which also warms the fee quote (028 phase 10's rule). Before 057 this
    /// pushed the form and the live router put the picker back, which reads as
    /// a button that did nothing.
    private func sendSelectedToken() {
        guard let token = wallet.balance?.tokens[safe: assetRow] else { return }
        flows.enter(.send)
        Task {
            await openSend()
            // Matched in the SEND machine's own list, and selected by ITS id.
            //
            // Not a reconstructed one: the core's id is
            // `network_address_symbol` and `network` is the send wire's own
            // spelling of the chain, which the balance wire does not carry.
            // 054 spent an afternoon on exactly that mistake — a hand-built id
            // that matched no token, so the tap did nothing.
            guard let match = send.view?.tokens.first(where: {
                $0.chainId == token.chainId
                    && ($0.tokenAddress ?? "") == (token.tokenAddress ?? "")
                    && $0.symbol == token.symbol
            }) else { return }
            send.selectToken(id: match.id)
        }
    }

    // MARK: - The hero's status line (spec 058, row 12)

    /// What the line opens.
    ///
    /// Android and the web agree on the rule and it is not arbitrary: a chain
    /// that FAILED needs a different endpoint, which is a thing a person can
    /// do; anything else — rate limiting, a partial read — resolves itself,
    /// and the honest answer is the breakdown showing which chains are still
    /// out. Reaching for the RPC sheet there would offer a fix for a problem
    /// that is not the person's to fix (invariant ④).
    private func openRescue() {
        let failed = wallet.balance?.bannerChainIds ?? []
        if let chainId = failed.first {
            rescueChain = chainId
            // By CHAIN ID, not by row id: a row's id is a slug ("gnosis") and
            // what failed is a chain number.
            rpcDraft = settings.networkAdmin?.networks
                .first { $0.chainId == chainId }?.rpcUrl ?? ""
            rescue = .rpcFix
        } else {
            rescueChain = nil
            rescue = .balanceDetail
        }
    }

    /// The rescue sheet's model: the settings page's, with this device's chains
    /// swapped into whichever rescue is up.
    private func rescueModel(_ overlay: SettingsOverlay) -> SettingsScreenModel {
        var model = settingsModel(overlay == .rpcFix ? .sr2 : .sr3)
        if let balance = wallet.balance {
            model = SettingsLive.withBalanceDetail(
                balance,
                display: WalletLive.Display.from(settings.currency),
                on: model, loc: loc
            )
        }
        if let chainId = rescueChain {
            model = SettingsLive.withRpcFix(chainId: chainId, endpoint: rpcDraft,
                                            on: model, loc: loc)
        }
        return model
    }

    /// The typed endpoint, saved for the chain the sheet is about.
    ///
    /// Through `network_admin`'s own per-chain override — the same operation
    /// the networks page uses — so one endpoint store has one writer.
    private func commitRescueRpc() {
        guard let chainId = rescueChain else { return }
        settings.editOverride(chainId: chainId, value: rpcDraft)
        settings.blurOverride(chainId: chainId)
        wallet.refresh(pull: true)
    }

    /// 删除记录 on the open transaction.
    ///
    /// The feed tombstones the record and drops the row at once, so the detail
    /// has nothing left to show and steps back to the list it came from —
    /// exactly what the web's `deleteSelectedTx` does. The CHAIN keeps the
    /// transaction; this is the wallet forgetting it.
    private func deleteOpenTransaction() {
        guard let feed = activity.feed, let item = selectedItem(in: feed) else { return }
        activity.deleteRequested(id: item.id)
        flows.back()
    }

    /// 收款 from a token's own page — R3, the asset-limited code.
    ///
    /// The asset goes through the MACHINE (`asset_picked`), not into a field
    /// beside the sheet: the core re-clamps the request's precision to the new
    /// asset, and a shell that kept the pick to itself would draw a USDC code
    /// over an 18-decimal request.
    private func receiveSelectedToken() {
        guard let token = wallet.balance?.tokens[safe: assetRow] else { return }
        // The catalog's name, or the chain id in words nobody has to invent —
        // the balance wire carries the TOKEN's name, which is not the network's.
        let network = ChainCatalog.meta(token.chainId)?.displayName ?? String(token.chainId)
        paymentRequest.start(
            account: session.view.address,
            recipient: session.view.address,
            baseUrl: Self.payLinkBase
        )
        paymentRequest.pickAsset(
            chainId: token.chainId,
            tokenAddress: token.tokenAddress,
            symbol: token.symbol,
            decimals: token.decimals,
            networkName: network
        )
        // Enter the flow, then the code: Back lands on the network list, which
        // is where a person who wanted a different network needs to be.
        flows.enter(.receive)
        flows.push(.receiveQrAsset)
    }

    /// Where a pay link points. The public host, because a phone has no origin
    /// of its own — the same constant Android keeps in `VelaNavHost`.
    private static let payLinkBase = "https://getvela.app/pay"

    /// 转账 from a contact's own page.
    ///
    /// The recipient rides through the SAME door a scanned code uses, so a
    /// contact, a QR code and a link all land on one screen.
    private func sendToContact(_ address: String) {
        section = .wallet
        flows.enter(.send)
        Task {
            await openSend()
            send.scanned(address)
        }
    }

    /// 群发转账 — every member of a group, as one split.
    ///
    /// The amounts are left empty: the core mints the row ids and the person
    /// fills the figures in. Seeding a number here would be inventing what
    /// somebody means to pay.
    private func sendToGroup(_ group: ContactGroupWire) {
        guard !group.members.isEmpty else { return }
        let rows: [[String: Any]] = group.members.map { member in
            [
                "id": "",
                "address": member.address,
                "amount": "",
                "name": member.name.map { $0 as Any } ?? NSNull(),
            ]
        }
        section = .wallet
        contactsRoute = nil
        flows.enter(.send)
        Task {
            // `Open` resets the machine to the picker, so the seed must follow
            // it — a split seeded first would be thrown away. The screen's own
            // `.task` opens too; whichever gets there first does it, and the
            // second is dropped by the store's idempotence guard.
            await openSend()
            send.seedSplitRecipients(rows)
        }
    }

    /// 导出 — the core writes the file, the share sheet hands it over.
    private func exportContacts(groupId: String? = nil) {
        contacts.exportRequested(groupId: groupId)
        guard let file = contacts.view?.export else { return }
        Task {
            _ = await documents.share(
                name: file.filename,
                type: DocumentTypes.type(forMime: file.mime),
                bytes: Data(file.content.utf8)
            )
            // One-shot: taken or not, the file leaves the view. Leaving it
            // there would re-offer the same backup on the next glance.
            contacts.exportTaken()
        }
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
            // R2 is a NETWORK's code and R3 is one ASSET's. The state is the
            // distinction — the machine always holds an asset, so asking it
            // unconditionally would print a contract line over a code for the
            // chain's own coin.
            let asset = state == .r3 ? paymentRequest.view?.asset : nil
            model.sheet = .receiveQr(FlowsLive.receiveQr(
                address, name: session.view.activeName,
                chain: asset.flatMap { ChainCatalog.meta($0.chainId) }
                    ?? (ChainCatalog.chains.indices.contains(receiveNetwork)
                        ? ChainCatalog.chains[receiveNetwork] : nil),
                asset: asset,
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
                model.base = .sendPick(
                    SendLive.pick(
                        view, on: pick, picking: sweepPicking,
                        classFilter: sendClassFilter, loc: loc
                    )
                )
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
                    display: display, on: confirm, loc: loc, fee: fees.view
                ))
            }
            if case .feeToken(let sheet)? = model.sheet, let fee = fees.view {
                model.sheet = .feeToken(SendLive.feeSheet(fee, on: sheet, loc: loc))
            }
            if case .contactPick(let sheet)? = model.sheet, let book = contacts.view {
                model.sheet = .contactPick(SendLive.contactSheet(book, on: sheet, loc: loc))
            }
            if case .batchImport(let sheet)? = model.sheet {
                model.sheet = .batchImport(
                    SendLive.batchImport(batch.view, view: view, on: sheet, loc: loc)
                )
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

    /// A picker row was tapped. The index travels because the screen knows
    /// positions and the core keys tokens by id.
    private func selectSendToken(_ index: Int) {
        guard let view = send.view, let token = view.tokens[safe: index] else { return }
        guard sweepPicking else {
            send.selectToken(id: token.id)
            return
        }
        // One tap becomes one or two events, in the order the core needs them:
        // the pin before the tick, because the pin is what the tick is judged
        // against.
        send.sweepTap(SweepPick.tap(
            view: view, tokenId: token.id, chainId: token.chainId
        ))
    }

    /// 「全选有价值代币」 — over the rows on screen, and only those.
    private func selectAllValuable(_ visible: [Int]) {
        guard let view = send.view, sweepPicking else { return }
        let ids = visible.compactMap { view.tokens[safe: $0]?.id }
        send.sweepTap(SweepPick.selectAll(
            view: view,
            visibleIds: ids,
            chainOf: { id in view.tokens.first { $0.id == id }?.chainId }
        ))
    }

    /// The picker's CTA.
    ///
    /// **「发送多个代币」 shipped doing nothing** — it pushed a state the live
    /// router overrode back to the picker (survey, 054). From a plain pick it
    /// turns the tick boxes on; from a sweep already under way it confirms the
    /// selection and the core moves to the sweep form.
    private func sendPickCta() {
        guard sweepPicking else {
            sweepPicking = true
            return
        }
        guard send.view?.multiSelectedIds.isEmpty == false else { return }
        send.confirmMultiSelection()
    }

    // MARK: - The payroll importer (spec 054 US3)

    /// 导入表格 — two machines open together.
    ///
    /// The send machine raises its flag (which is what puts SD2c on screen) and
    /// the importer opens on the token being sent. `Open` is a FULL reset by
    /// the core's own rule, so a paste left from a previous file is never
    /// waiting for the next one — which is why the drafts are cleared here too.
    private func openBatch() {
        guard let token = send.view?.selectedToken else { return }
        batchPaste = ""
        batchRate = ""
        send.openBatchImport()
        batch.open(
            symbol: token.symbol,
            decimals: token.decimals,
            balance: token.balance,
            priceUsd: token.priceUsd,
            currencyCode: settings.currency?.code ?? "USD"
        )
    }

    /// 导入 N 位收款人 — the parsed rows become the split's rows.
    ///
    /// Nothing is recomputed on the way across: the addresses the preview
    /// showed as valid are the addresses that get paid, and the core mints the
    /// row ids. `seed_split_recipients` also shuts the sheet, so there is no
    /// second close here.
    private func batchApply() {
        guard let rows = batch.apply() else { return }
        send.seedSplitRecipients(rows)
    }

    /// A fee asset was chosen.
    ///
    /// It is a QUOTE PARAMETER on both machines: the fee session re-quotes the
    /// real leg (a native transfer and an ERC-20 `transfer` are 68 bytes of
    /// calldata and one storage write apart), and the send machine records what
    /// was asked for so `submit_user_op` signs the same shape.
    private func pickFeeToken(_ index: Int) {
        let contract = fees.view?.options[safe: index]?.contract
        fees.selectAsset(contract)
        send.chooseFeeToken(contract)
        feeSheetOpen = false
    }

    /// Somebody was picked from the address book.
    private func pickSendContact(_ index: Int) {
        guard let contact = contacts.view?.contacts[safe: index] else { return }
        recipientDraft = contact.address
        send.pickedAddress(contact.address)
        send.closeContactPicker()
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
        // A journey that starts fresh starts without tick boxes. The flag is
        // the shell's, so nothing else will clear it — and a picker that opened
        // in sweep mode because the last journey ended there would be the
        // wallet remembering a decision nobody made twice.
        sweepPicking = false
        // A journey that starts fresh starts unfiltered too.
        sendClassFilter = "all"
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

    /// The signing sheet, from the four machines behind it.
    ///
    /// Extracted from the host's argument list because that list is one
    /// expression away from a type-checker timeout — 052 hit it, 054 hit it
    /// again, and this is the third.
    private func signingModel(for live: SigningController) -> SigningModel {
        // The REQUEST's chain. The browser's was right for a page's request and
        // wrong for the wallet's own: the key backup is on Ethereum whatever
        // chain the last tab was on.
        let chain = live.request?.chainId ?? browser.browserChain
        let request = live.request ?? SigningController.Incoming(
            id: "", method: "", paramsJson: "[]", origin: "",
            transportId: "", chainId: chain
        )
        let context = SigningLive.Context(
            loc: loc,
            chainName: ChainCatalog.meta(chain)?.displayName ?? String(chain),
            chainDot: SettingsLive.chainColor(chain),
            nativeSymbol: ChainCatalog.meta(chain)?.nativeSymbol ?? "",
            walletName: session.view.activeName,
            walletAddress: session.view.address,
            display: WalletLive.Display.from(settings.currency),
            origin: live.request?.origin,
            // What the chain said this transaction would do, and how far the
            // asking got. The judgment is the CORE's; this only carries it.
            sim: trust.trust?.sim,
            simulation: live.simulation,
            signMethod: live.signMethod,
            signWithOpen: live.signWithOpen
        )
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc),
            request: request,
            sign: live.sign,
            clear: live.clear,
            guard: live.guardView,
            fee: live.fee,
            context: context
        )
    }

    // MARK: - The scanner (spec 055)

    /// Why there is no viewfinder, in the core's words. Four refusals, four
    /// sentences — somebody who denied permission and somebody on a device with
    /// no camera need different things said to them.
    /// The flow screen itself.
    ///
    /// Extracted from `signedInOrWelcome` because this argument list is the
    /// thing that times out Swift's type checker — three times in this program
    /// already, and the scanner was the fourth. A method body is type-checked on
    /// its own, which is the whole trick.
    private func flowScreen(_ state: FlowStateId) -> some View {
                FlowHost(
                    model: flowModel(state),
                    onBack: { flows.back() },
                    onNavigate: { step in
                        // 导入表格 is the CORE's flag, not a push: the live
                        // router derives the send journey's state from the
                        // machine, so a pushed SD2c would be overridden back to
                        // the form — the dead-button shape twice already found
                        // in this flow.
                        if step == .batchImport { openBatch() } else { flows.push(step) }
                    },
                    addTokenInput: addTokenInput(for: state),
                    onAddToken: addTokenAction(for: state),
                    addTokenError: addTokenError(for: state),
                    onReceiveNetwork: { index in
                        receiveNetwork = index
                        // A network row asks to be paid in that chain's OWN
                        // coin. Telling the machine keeps the sheet's mark and
                        // the machine's asset from disagreeing.
                        guard let chain = ChainCatalog.chains[safe: index] else { return }
                        paymentRequest.pickAsset(
                            chainId: chain.chainId,
                            tokenAddress: nil,
                            symbol: chain.nativeSymbol,
                            decimals: 18,
                            networkName: chain.displayName
                        )
                    },
                    onSelectActivity: { activityRow = ($0, $1) },
                    onSelectAsset: { assetRow = $0 },
                    onSendToken: { sendSelectedToken() },
                    onReceiveToken: { receiveSelectedToken() },
                    onDeleteTx: { deleteOpenTransaction() },
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
                    alert: sendRefusal ?? scanAlert ?? saveAlert,
                    onDismissAlert: {
                        if send.alert != nil {
                            send.alert = nil
                        } else if scanNotice != nil {
                            scanNotice = nil
                        } else {
                            saveOutcome = nil
                        }
                    },
                    sendAmount: sendStates.contains(state) ? $amountDraft : nil,
                    sendRecipient: sendStates.contains(state) ? $recipientDraft : nil,
                    sendRow: splitRows(for: state),
                    sendWarning: send.view.flatMap { SendLive.formWarning($0, loc: loc) },
                    sendCtaDisabled: sendCtaDisabled(state),
                    onSelectToken: selectSendToken,
                    onSelectAllTokens: { visible in selectAllValuable(visible) },
                    onSendFilter: { id in sendClassFilter = id },
                    onPickCta: { sendPickCta() },
                    onMax: { send.tapMax() },
                    onRemoveRecipient: { index in removeSplitRow(at: index) },
                    onAddRecipient: { addSplitRow() },
                    onConfirm: { send.slideConfirm() },
                    onReceiptDone: {
                        // One button, two meanings, the core's rule: a prompt
                        // that is up is cancelled at its checkpoint; anything
                        // else is somebody saying they are done looking.
                        if send.view?.txStatus == "signing" {
                            send.cancelSigning()
                        } else {
                            send.done()
                        }
                    },
                    onContinueSend: { send.advance() },
                    onNoticeAction: {
                        // Whatever the notice is about, tried again. The CORE
                        // decides which — a funded relayer re-runs the
                        // pre-check, a refused submit re-signs, and a prompt
                        // that is up is cancelled at its own checkpoint.
                        if send.view?.treasuryBootstrap != nil {
                            send.retryAfterBootstrap()
                        } else if send.view?.txError != nil {
                            send.retryAfterError()
                        } else if send.view?.txStatus == "signing" {
                            send.cancelSigning()
                        }
                    },
                    // 暂不: the pause is dismissed and the facts are kept.
                    onNoticeSecondary: { send.dismissTreasurySheet() },
                    onPickFeeToken: pickFeeToken,
                    onPickContact: pickSendContact,
                    batchPaste: state == .sd2c ? $batchPaste : nil,
                    batchRate: state == .sd2c ? $batchRate : nil,
                    onBatchUnit: { batch.setUnit($0) },
                    onBatchFile: { batch.pickFile() },
                    onBatchTemplate: { batch.saveTemplate() },
                    onBatchResetRate: {
                        batch.resetRate()
                        batchRate = batch.view.rateInput
                    },
                    onBatchApply: { batchApply() },
                    scan: scanInputs()
                )
    }
    /// A picked photo with no code in it. Its own alert rather than a silent
    /// return: somebody who chose a picture is owed an answer about it.
    private var scanAlert: FlowAlertModel? {
        scanNotice.map { FlowAlertModel(title: $0.title, message: $0.body) }
    }

    /// Everything the live scanner needs, as one value.
    ///
    /// The session is handed over only while there is something to SHOW: a
    /// preview layer attached to a session with no frames is a black rectangle
    /// where the drawn placeholder belongs.
    private func scanInputs() -> ScanInputs {
        let live: AVCaptureSession? = (camera.refusal == nil && camera.running)
            ? camera.session : nil
        var inputs = ScanInputs(
            session: live,
            refusal: scanRefusalText(),
            torchOn: camera.torchOn,
            onTool: { tool in scanTool(tool) }
        )
        if camera.refusal == .denied {
            inputs.refusalAction = (
                label: loc.t("componentsUi.scanner.grantPermission"),
                act: openSettings
            )
        }
        return inputs
    }

    private func scanRefusalText() -> String? {
        switch camera.refusal {
        case .denied: loc.t("componentsUi.scanner.permissionText")
        case .restricted: loc.t("componentsUi.scanner.permissionText")
        case .noCamera: loc.t("componentsUi.scanner.noCamera")
        case .unavailable: loc.t("componentsUi.scanner.cameraUnavailable")
        case nil: nil
        }
    }

    private func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }

    private func scanTool(_ tool: ScanTool) {
        switch tool {
        case .torch: camera.toggleTorch()
        case .flip: camera.flip()
        case .gallery: pickCodeFromLibrary()
        }
    }

    /// A code decoded — from the camera or from a photo, through the one
    /// decoder. The CORE decides what it means.
    private func scannedCode(_ text: String) {
        camera.stop()
        send.scanned(text)
        flows.back()
    }

    /// The photo library. `PHPickerViewController` is cross-process and needs
    /// NO photo permission, which is what keeps this app's album key the
    /// narrowest thing in it (051: add-only).
    private func pickCodeFromLibrary() {
        Task {
            guard let image = await PhotoPicker.pick() else { return }
            guard let payload = QrDecoder.decode(image: image) else {
                scanNotice = (
                    loc.t("componentsUi.scanner.noQrFound"),
                    loc.t("componentsUi.scanner.noQrFoundMsg")
                )
                return
            }
            scannedCode(payload)
        }
    }

    /// The CTA's gate is the core's, never a conjunction assembled here.
    private func sendCtaDisabled(_ state: FlowStateId) -> Bool {
        guard let view = send.view else { return false }
        switch state {
        case .sd2, .sd2b, .sd2d: return !view.canContinue
        // Three more reasons the confirm CTA is inert, and each one now has a
        // line on the page saying so: a depleted relayer, a submit the relay
        // refused, and a signature already under way.
        case .sd3, .sd3b, .sd3c:
            return !view.canConfirm || view.sending
                || view.treasuryBootstrap != nil || view.txError != nil
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
            ),
            appearance: SettingsAppearanceActions(
                onTheme: { id in
                    guard let choice = ThemeChoice(rawValue: id) else { return }
                    preferences.setTheme(choice)
                },
                onAvatar: { id in
                    guard let style = AvatarStyle(rawValue: id) else { return }
                    preferences.setAvatarStyle(style)
                    AvatarPreference.apply(preferences)
                },
                onTextScale: { index in
                    guard let level = TextScaleLevel.allCases[safe: index] else { return }
                    preferences.setTextScale(level)
                    UiScale.apply(preferences)
                }
            ),
            onPick: { overlay, id in settingsPicked(overlay, id) },
            onClearCaches: { clearSettingsCaches() },
            onClearStorageItem: { id in
                DeviceStorage.clear(shelf, item: id)
                storageTick += 1
                // What was cleared is what the rest of the app was showing:
                // balances re-read, the feed and the address book re-read
                // themselves from a store that no longer has those keys.
                wallet.refresh(pull: true)
            },
            onErase: { eraseThisDevice() },
            onSelectAccount: { address in switchToAccount(address) },
            // The two ways on from the switcher. Both drew and did nothing
            // until 2026-09-16; the sign-in picker rides the ONE onboarding
            // sheet hosted at the root, so it opens from here as readily as
            // from Welcome.
            onAccountCreate: { router.path.append(.create) },
            onAccountSignIn: { onboarding.showSignInMethods = true },
            endpointActions: SettingsEndpointActions(
                onEditEndpoint: { id, value in settings.editEndpoint(id: id, value: value) },
                onBlurEndpoint: { id in settings.blurEndpoint(id: id) },
                onResetEndpoints: { settings.resetEndpoints() },
                onEditProvider: { id, value in settings.editProviderKey(id: id, value: value) },
                onBlurProvider: { id in settings.blurProviderKey(id: id) },
                onTestProvider: { id in settings.testProvider(id: id) },
                onOpenEndpoints: { settings.openEndpoints() },
                onOpenProviders: { settings.openProviders() }
            ),
            onOpenAccounts: { openAccountSwitcher() },
            // Only "not backed up" carries a call; every other state's row is
            // not a button, and a tap on it sends nothing.
            onEthereumBackup: {
                guard let asked = backupCheck,
                      asked.address.caseInsensitiveCompare(session.view.address) == .orderedSame,
                      let call = asked.check.call
                else { return }
                openEthereumBackup(call)
            }
        )
        // The wallet's own request, over the page that raised it. Settings
        // keeps its pickers on a sheet of its own INSIDE the screen; the row
        // that opens this one opens no picker, so the two never stand at once.
        .sheet(isPresented: Binding(
            get: { signing != nil },
            set: { open in if !open { signing?.swipeDismissed() } }
        )) {
            if let signing {
                SigningSheet(
                    model: signingModel(for: signing),
                    onConfirm: { signing.approve() },
                    onAllowanceChip: { chip in signing.guardPreset(chip) },
                    onAllowanceAmount: { text in signing.guardCustomAmount(text) },
                    onSignWith: { id in signing.signWith(id) }
                )
                    .presentationDragIndicator(.visible)
                    .presentationDetents([.large])
                    .presentationCornerRadius(Tokens.Radius.r20)
                    .themed(scheme)
            }
        }
        .onChange(of: signing?.closed) { _, closed in
            guard closed == true else { return }
            signing = nil
            // Landed, rejected or dismissed — the chain is what knows.
            Task { await checkEthereumBackup() }
        }
        .task(id: session.view.address) { await checkEthereumBackup() }
        .task(id: session.view.address) { await readWalletKeys() }
        .task {
            settings.open()
            // Both pages ask the core to read what is stored when they open.
            // Until 056 nothing sent either event, so two live pages rendered
            // whatever the machine happened to be holding.
            settings.openEndpoints()
            settings.openProviders()
        }
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
        // The switcher's rows are the SESSION's, with the balance core's
        // cached totals — after the currency, because the figures it writes
        // wear that currency's glyph and rate.
        model = SettingsLive.withAccounts(
            session: session.view,
            balances: wallet.balance?.switcher.balances ?? [],
            display: WalletLive.Display.from(settings.currency),
            on: model,
            loc: loc
        )
        // Asked of the chain for THIS wallet, or still being asked (spec 062).
        let backedUp = backupCheck.flatMap {
            $0.address.caseInsensitiveCompare(session.view.address) == .orderedSame ? $0.check.state : nil
        }
        let keys = walletKeys.flatMap {
            $0.address.caseInsensitiveCompare(session.view.address) == .orderedSame ? $0.result : nil
        }
        // No wallet, no block. With no address nothing is ever asked, so "still
        // asking" would never end: on a signed-out iPhone the block sat on its
        // loading skeleton and the backup on 正在检查… for good (device-found
        // 2026-09-19).
        if !session.view.address.isEmpty {
            model = SettingsLive.withWalletKeys(keys, backup: backedUp, on: model, loc: loc)
        }
        // The preferences last: they have no machine to wait for, and every
        // surface they touch is one this page draws.
        model = SettingsLive.withPreferences(preferences, on: model, loc: loc)
        model = SettingsLive.withProviderTests(on: model, loc: loc)
        // Measured, not drawn (058). `storageTick` is what makes a clear show
        // up: the report is read here, so the page has to be asked to build
        // again after keys are removed.
        _ = storageTick
        model = SettingsLive.withStorage(DeviceStorage.measure(shelf), on: model, loc: loc)
        model = SettingsLive.withAbout(
            version: BuildInfo.version,
            commit: BuildInfo.commit,
            networkCount: settings.networkAdmin?.networks.count
                ?? ChainCatalog.chains.count,
            on: model, loc: loc
        )
        return model
    }

    /// Which passkeys control the active wallet (spec 062), from the registry
    /// contract — or, when no chain answers, from this device's own record.
    private func readWalletKeys() async {
        let address = session.view.address
        guard !address.isEmpty else { return }
        let device = await WalletKeys.deviceKeys(
            of: address, walletName: session.view.activeName, in: sendAccountPort
        )
        let reader = WalletKeys(ethCall: { [pool] chainId, to, data in
            let outcome = await pool.call(
                chainId: chainId, method: "eth_call",
                params: [["to": to, "data": data], "latest"]
            )
            guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x")
            else { return nil }
            return hex
        })
        walletKeys = (address, await reader.read(address: address, device: device))
    }

    /// Reads, from the chains themselves, whether this wallet's founding keys
    /// are registered on Ethereum as they are on Gnosis (spec 062).
    ///
    /// A wallet with no stored founding key — the dev seed — has nothing to
    /// look up, and the row is not drawn for it.
    private func checkEthereumBackup() async {
        let address = session.view.address
        guard !address.isEmpty else { return }
        let key = await RegistryBackup.foundingKeyHex(of: address, in: sendAccountPort)
        guard !key.isEmpty else {
            backupCheck = (address, RegistryBackup.Check(state: .unavailable, call: nil))
            return
        }
        let backup = RegistryBackup(ethCall: { [pool] chainId, to, data in
            let outcome = await pool.call(
                chainId: chainId, method: "eth_call",
                params: [["to": to, "data": data], "latest"]
            )
            // The RAW result, a bare `0x` included: a chain without the
            // registry is an answer ("not here"), not a silence.
            guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x")
            else { return nil }
            return hex
        })
        let check = await backup.check(address: address, foundingKeyHex: key)
        backupCheck = (address, check)
    }

    /// 全部清除 — the caches this app can rebuild, and nothing it cannot.
    ///
    /// Balances, prices, rates and the bundler's quotes. NOT the address book,
    /// the custom networks, the transaction history or anything an account is
    /// made of: "clear caches" is an offer to make the app forget what it can
    /// look up again, and a person who taps it is not asking to lose data.
    private func clearSettingsCaches() {
        // Through the SAME mapping the storage page weighs, so what the page
        // said would be freed is exactly what is removed. A hand-kept key list
        // beside a measured one drifts, and the drift shows up as a page that
        // still reports megabytes after clearing them (028's finding).
        DeviceStorage.clearCaches(shelf)
        relayClient.clearCaches()
        storageTick += 1
        wallet.refresh(pull: true)
    }

    /// 抹除此设备 — every key this app owns, and then the door.
    ///
    /// **Never run on the founder's phone.** It is wired because a drawn
    /// destructive action that does nothing is worse than one that works;
    /// verifying it means reading this code, not erasing a device with a real
    /// wallet on it.
    private func eraseThisDevice() {
        // Everything this app owns, by the keys it owns them under. The
        // signed-in wallet goes through `AccountStore`, which is the one writer
        // of those two keys.
        for key in [
            VelaStore.Key.contacts, VelaStore.Key.contactsDismissed,
            VelaStore.Key.contactGroups, VelaStore.Key.customNetworks,
            VelaStore.Key.networkConfig, VelaStore.Key.rpcProviders,
            VelaStore.Key.displayCurrency, VelaStore.Key.balanceCache,
            VelaStore.Key.customTokens, VelaStore.Key.transactionHistory,
            VelaStore.Key.fiatRates, VelaStore.Key.fiatFeedAddrs, VelaStore.Key.fxRates,
            VelaStore.Key.theme, VelaStore.Key.language, VelaStore.Key.localePrefs,
            VelaStore.Key.avatarStyle, VelaStore.Key.textScale,
        ] {
            shelf.writeString(key, nil)
        }
        accounts.clearSignedInWallet()
        session.signOut()
    }

    /// An account row in the switcher.
    private func switchToAccount(_ address: String) {
        Task {
            let records = await accounts.loadAccounts()
            guard let index = records.firstIndex(where: {
                ($0["address"] as? String)?.lowercased() == address.lowercased()
            }) else { return }
            session.switchAccount(index: index)
            // Everything account-scoped starts again: the book, the balances,
            // the feed. A switch that left the previous account's money on
            // screen would be the worst thing this control could do.
            contacts.open(myAddress: address)
            wallet.refresh(pull: true)
        }
    }

    /// The home switcher closed. The balance core is told, so it stops
    /// refreshing per-account totals nobody is looking at (`switcher_closed`).
    private func closeHomeSwitcher() {
        homeSwitcherOpen = false
        wallet.switcherClosed()
    }

    /// The account switcher opened — from settings, or from the wallet header.
    ///
    /// The balance machine is told, so the sheet opens on CACHED totals rather
    /// than on spinners: it has every account's last known figure and refreshes
    /// behind them (the core's invariant ⑩).
    private func openAccountSwitcher() {
        Task {
            let addresses = await accounts.loadAccounts()
                .compactMap { $0["address"] as? String }
            wallet.switcherOpened(addresses: addresses)
        }
    }

    /// A row picked in one of the five select sheets.
    ///
    /// The three FORMATS take effect immediately — `Formats.current` is what
    /// every renderer reads, and a preset that waited for a relaunch would be a
    /// choice a person could not see themselves make.
    private func settingsPicked(_ overlay: SettingsOverlay, _ id: String) {
        switch overlay {
        case .currency:
            // The core owns the display currency: it re-resolves the rate, and
            // a shell that wrote the code itself would show a figure converted
            // at the previous one.
            settings.chooseCurrency(id)
        case .language:
            // `system` is the drawn id; `auto` is what every client STORES.
            let tag = id == "system" ? "auto" : id
            preferences.setLanguage(tag)
            // And the app changes language now, not on the next launch —
            // `Loc.t` observes `resolvedLanguage`, so every translated view
            // redraws. Android has done this since 047.
            loc.apply(tag)
        case .numberFormat:
            guard let key = NumberFormatKey(rawValue: id) else { return }
            preferences.setNumberFormat(key)
            Formats.apply(preferences)
        case .dateFormat:
            guard let key = DateFormatKey(rawValue: id) else { return }
            preferences.setDateFormat(key)
            Formats.apply(preferences)
        case .timeFormat:
            guard let key = TimeFormatKey(rawValue: id) else { return }
            preferences.setTimeFormat(key)
            Formats.apply(preferences)
        default:
            break
        }
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

    /// A page for the browser to open at launch. **Debug only** — a release
    /// build that opened a URL from its environment would be one an attacker
    /// with a launch profile could point anywhere.
    static let browserURL: String? = {
        #if DEBUG
        return ProcessInfo.processInfo.environment["VELA_URL"]
        #else
        return nil
        #endif
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
