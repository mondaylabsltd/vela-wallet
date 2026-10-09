//
//  AppGraph.swift
//  VelaWallet
//
//  The app's object graph — every resident machine, the one request pool and
//  the one money path — built ONCE per app lifetime and owned by the App
//  struct, never by a view.
//
//  ## Why not in `RootView.init` (issue #483)
//
//  It was. `RootView.init` built all of this, keeping most of it in `@State`
//  and the money path (relay client, account port, submit spine) in plain
//  `let`s. SwiftUI runs a view's init again whenever the content that builds
//  it is re-evaluated, and that init read `@Observable` state (the stored
//  language, the text size, the formats, debug mode): so a Settings change
//  re-ran it. `@State` kept its FIRST values and threw the new ones away; the
//  plain `let`s took the NEW ones — a relay over a second request pool that
//  nothing ever booted. From then on every dApp fee read died inside the app
//  (`rpc: before_boot`), the row said "Can't reach <chain>" and Tap to retry
//  re-ran the same dead objects until the app was killed. Measured on a
//  simulator: a text-size change, then a dApp request
//  (`Issue483DeviceTests`). The trusted page and the signing-pages trust hook
//  were orphaned the same way.
//
//  Here nothing a view does can build a second copy: the App holds one graph
//  in `@State`, an erase (spec 072) replaces it — the only time the store is
//  emptied under a running app — and `RootView` only reads it. Its init does
//  no work and reads no observable state, so its re-runs are free.
//
//  ## One scene
//
//  The app is iPhone-only (`TARGETED_DEVICE_FAMILY = 1`) and declares one
//  scene (`UIApplicationSupportsMultipleScenes = false`). Even so, a second
//  scene would read this same graph — the App's — never build its own over
//  the same storage; and `launch()` runs once per graph whichever root asks.
//

import Foundation
import SwiftUI

@MainActor
final class AppGraph {

    let loc: Loc
    /// `false` for the graph an erase rebuilt: no launch lockup, and no DEBUG
    /// account seed writing a wallet back into the store just emptied.
    let firstLaunch: Bool
    /// The `vela.*` shelf, for the reads that are not a machine's — the
    /// explorer bases of custom networks.
    let shelf: VelaStore
    /// The account list. The parallel space's door needs it at launch.
    let accounts: AccountStore
    let router: Router
    let flows: FlowNav
    let session: SessionController
    let onboarding: OnboardingModel
    /// The routing authority (spec 051). One session, app-wide: a ban is a fact
    /// about the network, and two callers with their own endpoint lists is how
    /// the Expo client got a ban map that disagreed with itself.
    let pool: RpcPool
    /// ONE name resolver: the address book and the feed ask the same question.
    let identity: RecipientIdentity
    /// The money path, shared with 053's dApp transactions: one relay client,
    /// one account port, one submit spine — all over `pool`. A second spine
    /// would be a second set of rules about the same Safe.
    let relay: RelayClient
    let accountPort: SendAccountPort
    let spine: UserOpSpine
    /// The send form's fee sessions (the signing sheet runs its own, over the
    /// same relay).
    let fees: FeeStore
    let contacts: ContactsStore
    let settings: SettingsStore
    /// ONE trusted page for the whole app (specs 075, 102).
    let trustedSigner: TrustedSigner
    let trust: TokenTrustStore
    let activity: ActivityStore
    let wallet: WalletStore
    let tokens: ManageTokensStore
    let notifier: TrackerNotifier
    let tracker: TrackerStore
    let browser: BrowserController
    let netWatch: NetWatch
    let send: SendStore
    let documents: UIKitDocumentPorts
    let paymentRequest: PaymentRequestStore
    let preferences: Preferences
    let batch: BatchStore
    let deposits: ReceiveWatchStore
    let welcome: WelcomeModel
    /// The camera behind the scanner (spec 055): its capture session survives
    /// the surface's rebuilds.
    let camera: CameraScanner

    /// The launch sequence has started (`launch`).
    private(set) var launched = false

    /// - Parameter defaults: the storage; `nil` is the app's own. Tests pass a
    ///   suite of their own.
    init(loc: Loc, firstLaunch: Bool, defaults: UserDefaults? = nil) {
        self.loc = loc
        self.firstLaunch = firstLaunch
        let router = Router()
        self.router = router
        let flowNav = FlowNav()
        self.flows = flowNav
        let store = defaults.map { AccountStore(defaults: $0) } ?? AccountStore()
        self.accounts = store
        let session = SessionController(store: store)
        self.session = session
        let shelf = defaults.map { VelaStore(defaults: $0) } ?? VelaStore()
        self.shelf = shelf
        // Before the session machine boots: it reads `vela.accounts` on its
        // first event, and a record written after that is not seen until a
        // relaunch. DEBUG-only, env-gated, and key-less (spec 051 D3).
        if firstLaunch { DevAccountSeed.applyIfRequested(store: shelf) }
        // An older shell's spellings of the five preferences, brought to the
        // shared record before anything reads them (spec 072). Nothing to do
        // for a store that already agrees, so this is safe every launch.
        Preferences.migrate(shelf)
        // One pool, built before anything that reads a chain — the settings
        // machines included, since spec 051 put the fiat feeds behind it.
        let pool = RpcPool(store: shelf, accounts: store)
        self.pool = pool
        // Booted with the graph, before anything can ask it: the ban map is
        // a synchronous read, and a pool that answers from its first moment
        // leaves `late_boot` meaning one thing — a pool built outside here.
        pool.boot()
        // Through the three layers (067): the index's answers are PROVED against
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
        self.onboarding = onboarding
        // ONE name resolver for the whole app: the address book and the
        // activity feed ask the same question about the same addresses, and two
        // resolvers would mean two caches and two names for one person.
        let identity = RecipientIdentity(store: shelf, pool: pool, accounts: store)
        self.identity = identity
        // The money path. One relay client, one spine, one fee session — the
        // spine is shared with 053's dApp transactions, which is why it is
        // built here rather than inside the send store.
        let relay = RelayClient(
            port: PoolRelayPort(pool: pool),
            builtinBase: { await RpcEndpoints.builtinBundlerBase(accounts: store) }
        )
        self.relay = relay
        let port = SendAccountPort(accounts: store)
        self.accountPort = port
        let spine = UserOpSpine(
            relay: relay,
            accounts: port,
            signer: { ParallelSpaceHook.signer(passkey: PasskeyExecutor()) },
            measureCall: FeeExecutor.measuring(with: pool)
        )
        self.spine = spine
        // The quote measures the inner calls the way the spine does at submit,
        // so the fee prices the `callGasLimit` the op will carry.
        let feeStore = FeeStore(relay: relay, accounts: port, measureCall: FeeExecutor.measuring(with: pool))
        self.fees = feeStore
        self.contacts = ContactsStore(
            store: shelf, identity: identity, pool: pool
        )
        // `vela.serviceEndpoints` has two writers; the executor reaches it
        // through this same `AccountStore` so onboarding's endpoint override
        // survives a settings write (data-model §5).
        let settingsStore = SettingsStore(store: shelf, accounts: store, pool: pool)
        // The logos come from the person's chain-data endpoint, and follow it
        // the moment Settings saves a new one — it used to take a relaunch.
        settingsStore.onEndpointsWritten = { endpoints in Marks.adopt(endpoints) }
        self.settings = settingsStore
        // ONE trusted page for the whole app (specs 075, 102): an account's
        // signatures whose venue is a page, and the ceremonies of a wallet on
        // its own domain, go through the same object — which is what keeps
        // "one page, one session, one sheet" true; two instances would be two
        // sheets racing to present over each other. It opens nothing this
        // phone did not check first (`SignerPageChecks`, R6).
        let trustedSigner = TrustedSigner(loc: loc, checks: .shared)
        self.trustedSigner = trustedSigner
        // "Trust this version" — on Settings → Signing pages, the hand-off
        // card or the page's own sheet — is stored on that page by the one
        // signing pages machine (D-15), never in a list of the checker's own.
        SignerPageChecks.shared.recordTrust = { [settingsStore] url, version in
            await settingsStore.trustSigningPage(url: url, version: version)
        }
        spine.trustedSigner = trustedSigner
        onboarding.trustedSigner = trustedSigner
        onboarding.words = { [loc] key in loc.t(key) }
        // Spec 102: the choosers' "Use a trusted signing page" lists the pages
        // Settings keeps — read by the same machine, so the two never differ.
        onboarding.signingPages = { [settingsStore] in settingsStore.signingPages }
        onboarding.openSigningPages = { [settingsStore] in settingsStore.openSigningPages() }
        onboarding.addSigningPage = { [settingsStore] url in settingsStore.addSigningPage(url: url) }
        // The balance read publishes what it found here, and the receipt scan
        // reads it: which chains this account uses, which tokens it holds, and
        // what they were worth. Web gets the same three facts from its
        // `fetchTokens` cache; there is no such cache here, so it is explicit.
        let held = HeldTokens()
        let trust = TokenTrustStore(store: shelf, pool: pool, accounts: store, held: held)
        self.trust = trust
        let activityStore = ActivityStore(
            store: shelf, accounts: store, held: held, trust: trust, identity: identity
        )
        self.activity = activityStore
        // A saved token has to reach the balances, so the core's
        // "invalidate the token cache" becomes a re-read here — there is no
        // cache on this client, only a fetch.
        // The registry's stablecoins and wrapped coin join every balance
        // read (spec 082 RE9, G24): USDC on Base is counted, as on the other
        // clients.
        let wallet = WalletStore(store: shelf, pool: pool, held: held, registry: ChainTokens(accounts: store))
        self.wallet = wallet
        // An incoming transfer the feed just found: the hero follows (#188).
        activityStore.onNewItem = { [weak wallet] in wallet?.refresh(pull: false) }
        self.tokens = ManageTokensStore(
            store: shelf, pool: pool,
            onInvalidate: { [weak wallet] in wallet?.refresh(pull: false) }
        )
        // The tracker, before the send machine that hands off to it.
        let notify = TrackerNotifier(loc: loc)
        self.notifier = notify
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
                recordsPatched: { [weak activityStore] in activityStore?.reconciled() },
                // An op of ours landed — a send or a page's — so the figure on
                // the home is read again, without a pull (spec 082 RE8, G26).
                holdingsMoved: { [weak wallet] _ in wallet?.refresh(pull: false) }
            )
        )
        let trackerStore = TrackerStore(executor: trackerExecutor)
        self.tracker = trackerStore
        // The browser. Its reads go through the SAME pool the wallet uses, so
        // a page asking a chain something gets this person's endpoints, their
        // bans and their cooldowns — never an endpoint the page named.
        let browserController = BrowserController(store: shelf)
        browserController.ports = BrowserController.Ports(
            poolCall: { [weak pool] chainId, method, params, bundler in
                guard let pool else { return .unanswered(rateLimited: false) }
                switch await pool.call(
                    chainId: chainId, method: method, params: params,
                    kind: bundler ? "bundler" : "rpc"
                ) {
                case .ok(let body):
                    return .answered(["result": body ?? NSNull()])
                case .rpcError(let code, let message):
                    // The node's own sentence, verbatim. A page that shows its
                    // user "execution reverted: insufficient allowance" is a
                    // page that can be debugged; one that shows "-32603" is not.
                    return .answered(["error": ["code": code ?? -32603, "message": message]])
                // Spec 099 FR-009: the pool's word on why nobody answered —
                // every endpoint throttling is not the network being down.
                case .failed(let rateLimited):
                    return .unanswered(rateLimited: rateLimited)
                case .rangeCap:
                    return .unanswered(rateLimited: false)
                }
            },
            // A page answered with a user-operation hash polls for its receipt
            // by that hash; the core asks here which transaction carried it —
            // the relay's own `eth_getUserOperationReceipt`, the lookup the
            // tracker and Android's `RelayClient.userOpReceipt` use.
            resolveUserOp: { [relay] chainId, userOpHash in
                guard case .resolved(_, let txHash, _, _) = await relay.userOpReceipt(
                    chainId: chainId, userOpHash: userOpHash
                ), !txHash.isEmpty else { return nil }
                return txHash
            },
            writeRecords: { [weak activityStore] rows in
                TxRecords.writeRecords(rows, store: shelf)
                activityStore?.reconciled()
            }
        )
        self.browser = browserController
        // The network came back (spec 082 RE3): every failed page starts its
        // count again and the one in front is asked for again, the logos a
        // dead network kept away are asked for again, and the balance is
        // read — nobody has to tap anything.
        let netWatch = NetWatch()
        // Health per source (spec 082 RJ14): each call names its chain.
        pool.onOutcome = { [weak netWatch] outcome, chainId in
            netWatch?.observe(outcome, chainId: chainId)
        }
        netWatch.onCameBack = { [weak browserController, weak wallet] in
            browserController?.networkCameBack()
            LogoStore.networkCameBack()
            wallet?.refresh(pull: false)
        }
        netWatch.watchPath()
        self.netWatch = netWatch
        // The send machine, last: it reads the holdings the balance machine
        // found and asks the fee session for a quote, so both must exist.
        let metadata = TokenMetadata(store: shelf, pool: pool)
        let sendExecutor = SendExecutor(
            store: shelf, relay: relay, pool: pool, spine: spine, accounts: port,
            fees: feeStore, identity: identity, metadata: metadata, accountStore: store,
            balances: { [weak wallet] in wallet?.balance },
            networks: { [weak settingsStore] in settingsStore?.networkAdmin },
            // The asset list's own rounds (spec 078): a flow opened before the
            // dashboard settled for this account waits for its first round —
            // booting it if nothing has — instead of answering "could not load";
            // a round that reached nothing is read once more first.
            holdingsRound: { [weak wallet] address in wallet?.settledRound(for: address) },
            openHoldings: { [weak wallet] address in wallet?.open(address: address) },
            ports: SendExecutor.Ports(
                // A send the relay accepted is the tracker's from that moment.
                // The permission is asked HERE — at the first submit, never at
                // launch — because this is the first time there is anything to
                // notify about.
                trackSubmitted: { [weak trackerStore, weak notify] submission in
                    notify?.askOnceIfNeeded()
                    trackerStore?.submitted(submission)
                },
                // A write-ahead op proven never sent (spec 082 RJ1): the
                // tracker drops it, as the store already has.
                trackWithdrawn: { [weak trackerStore] hash, ids in
                    trackerStore?.withdrawn(userOpHash: hash, recordIds: ids)
                },
                // The core's `haptic { kind }`: money left, or a refusal the
                // person should feel. Unwired until 074, so an iPhone sent in
                // silence where Android buzzed.
                haptic: { kind in VelaHaptic(sendKind: kind).play() },
                // The core's own exit. `Done` on a receipt, and `close` on any
                // refusal that ends the attempt, both land here.
                closed: { [weak flowNav] in flowNav?.close() },
                // Leaving is what re-arms `Open`. Without it a second visit to
                // 转账 would render the machine's last state instead of a
                // fresh picker.
                refreshBalances: { [weak wallet] in wallet?.refresh(pull: false) },
                // The rows changed on disk — written ahead, admitted or taken
                // back (spec 082 RJ1): the feed reads the store again.
                recordsPersisted: { [weak activityStore] in activityStore?.reconciled() }
            )
        )
        let sendStore = SendStore(executor: sendExecutor)
        self.send = sendStore
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
        trackerExecutor.ports.notifyConfirmed = { [weak notify] hash, chain, tx in
            notify?.confirmed(userOpHash: hash, chainId: chain, txHash: tx)
        }
        // Every verdict — confirmed, failed, not sent, held, acknowledged —
        // reaches the receipt through the core's one mapping (spec 082), once.
        trackerStore.onView = { [weak sendStore, weak trackerStore] view in
            sendStore?.trackerChanged(view)
            // The ops holding a nonce (PR 2 §3), from the core's own view.
            sendStore?.inFlightChanged(trackerStore?.inFlightOpsJson ?? "[]")
        }
        // The payroll importer. Its fiat column is priced through the DISPLAY
        // machine's own waterfall — chain feed, then endpoint, then nothing —
        // so a currency the wallet cannot price stays unpriced here too. A
        // fallback of 1 would pay out the fiat figure in tokens.
        let documentPorts = UIKitDocumentPorts()
        self.documents = documentPorts
        // Read before the first frame: a theme or a text size adopted one
        // render late is a visible flash of the wrong one.
        self.paymentRequest = PaymentRequestStore(
            executor: PaymentRequestExecutor(store: shelf)
        )
        let prefs = Preferences(store: shelf)
        prefs.boot()
        // Before the first frame, and before the welcome model is built from
        // it: the stored language decides which words this launch uses. Until
        // 058 nothing read `vela.language` at all.
        loc.apply(prefs.language)
        // Which chain-data endpoint the logos come from (058). The person's
        // own endpoint wins; an empty one is the built-in endpoint, as the
        // core reads it. Settings' saves re-adopt it (`onEndpointsWritten`).
        // The same `vela.serviceEndpoints` record `AccountStore` keeps, read on
        // the shelf: an actor cannot be read from a synchronous init.
        Marks.adopt(shelf.readObject(VelaStore.Key.serviceEndpoints))
        Formats.apply(prefs)
        UiScale.apply(prefs)
        // Settings' debug mode (spec 091), before the browser boots: its core
        // hears it right behind `start`, and every tab's script follows it.
        // An erase builds a new graph over the emptied store, so this reads
        // hidden — off — again.
        browserController.setDebugMode(prefs.debugMode.isOn)
        self.preferences = prefs
        self.batch = BatchStore(executor: BatchExecutor(
            fiatRate: { [weak settingsStore] code in await settingsStore?.usdRate(code) },
            documents: { documentPorts }
        ))
        // The receive screen's watcher. A detected deposit buzzes and re-reads
        // the balances, so the figure behind the code is the new one.
        self.deposits = ReceiveWatchStore(
            store: shelf, pool: pool, held: held,
            onDeposit: { [weak wallet] in wallet?.refresh(pull: false) }
        )
        self.camera = CameraScanner()
        self.welcome = WelcomeModel(content: WelcomeContentBuilder.build(loc: loc)) { intent in
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
        }
        Self.built += 1
    }

    /// How many graphs this process has built — the regression seam for
    /// "a view's init builds nothing" (issue #483).
    private(set) static var built = 0

    /// What runs once the first root is on screen, once per graph: the
    /// parallel space's door (before the session machine reads `vela.accounts`),
    /// then the session and the tracker. Every later call — another scene's
    /// root, a root SwiftUI re-identified — returns at once. (The request pool
    /// is booted with the graph: a launch that opens on 探索 and booted it
    /// with the wallet screen answered every page read "No endpoint
    /// answered", device-found in spec 070.)
    func launch() async {
        guard !launched else { return }
        launched = true
        // Before the session machine's first event, which is when it reads
        // `vela.accounts`: a record written after that is not seen until a
        // relaunch. The space upserts ONE record and removes exactly that one
        // on the way out (FR-003) — this door is opened on a phone that holds
        // the founder's real wallet.
        await ParallelSpaceHook.applyIfRequested(store: shelf, accounts: accounts)
        session.boot()
        // At LAUNCH, not with a screen: what the tracker follows outlives every
        // screen. The pending set is derived from the transaction store, so a
        // force-quit mid-send loses nothing — the next launch picks it up here.
        tracker.boot()
    }
}
