//
//  ActivityStore.swift
//  VelaWallet
//
//  The `activity_feed` machine, app-resident.
//
//  Residency matters as much here as it does for the balance: the machine holds
//  the folded feed, the alias memo and the celebration state. One that died
//  with the home screen would re-scan every chain each time somebody came back
//  from the address book, and would celebrate the same receipt twice.
//
//  ## The cadence is the core's (r9-17)
//
//  When to scan for incoming funds is `incoming_scan::next_scan_ms`, driven by
//  `activity_feed` (r10 follow-up): a back-off while nothing arrives — 15 s,
//  30 s, 60 s, 2 min; 2 min hidden — and at once when the app is shown again,
//  back online, pulled to refresh or after a send — the same rule on every
//  platform. This store only reports the app's presence (`presence_changed`:
//  shown, hidden, online, refreshed, sent), says when a list is being
//  scrolled (`interacting {on}`: a scan is held, never in a gesture's way)
//  and runs the one timer the core asks for (`next_scan`, answered with
//  `scan_due`).
//

import Foundation
import Observation
import VelaCore

extension ActivityFeedCore: CoreBridge {}

@MainActor
@Observable
final class ActivityStore {

    /// A touch is reported at most this often (the web's
    /// `INTERACTION_REPORT_MS`): the core only needs to hear that someone is
    /// at the app. Transport, not a decision.
    static let interactionReportMs: Double = 1_000

    private(set) var feed: FeedViewWire?
    /// A transfer the scan just found moved the balances too (issue #188):
    /// the host refetches them. Fired once per arrival — the core celebrates
    /// only a genuinely-new incoming record, never the first pass.
    var onNewItem: () -> Void = {}
    /// Whether the local store has been read at least once.
    ///
    /// The view cannot say: an unread machine and an account with no history
    /// both publish an empty row list, and the two must not draw the same. An
    /// empty feed is a claim ("nothing has happened here"); before the read
    /// lands, the honest surface is the drawn skeleton (FR-008).
    private(set) var hasRead = false

    /// The accounts (lowercased) whose first whole incoming scan has come
    /// back this session (R12 S4) — one that ran with the account's
    /// holdings read (`ActivityExecutor.looked`). Before it, an empty feed
    /// is not yet "No activity yet": the chains have not been asked what
    /// came in — the home says it is still looking. Kept per account: a
    /// switch to one never scanned is a first look again.
    private(set) var scanned: Set<String> = []

    /// What the home's Activity may claim for `address`: read from the store
    /// AND its first scan back — "nothing has happened here" needs both.
    func settled(for address: String) -> Bool {
        hasRead && scanned.contains(address.lowercased())
    }

    private let executor: ActivityExecutor
    private var core: CoreStore<FeedViewWire>!

    /// The view as the core wrote it — the AI agent reads the wallet whole
    /// (`AgentShellBridge.views`, spec 104).
    var viewJson: String? { core.json }
    private var scopedTo: String?
    /// r9-17: the cadence — following it, the timer armed for `next_scan`
    /// and its `seq`, the app's presence.
    private var following = false
    private var armedSeq: Int?
    private var dueTimer: Task<Void, Never>?
    private var visible = true
    private var lastInteractionMs = Date().timeIntervalSince1970 * 1000
    private var lastReportMs = -Double.infinity
    /// r10 follow-up: the lists that are moving under a finger right now
    /// (the home, the history). The core hears `on` when the first starts
    /// and `off` when the last has settled.
    private var movingLists: Set<String> = []

    init(
        store: VelaStore,
        accounts: AccountStore,
        held: HeldTokens,
        trust: TokenTrustStore,
        identity: RecipientIdentity? = nil
    ) {
        self.executor = ActivityExecutor(
            store: store, accounts: accounts, held: held, trust: trust, identity: identity
        )
        self.core = CoreStore(
            bridge: ActivityFeedCore(),
            perform: { [weak self, executor] operation in
                let answer = await executor.perform(operation)
                switch operation["type"] as? String {
                case "read_tx_store": self?.hasRead = true
                case "scan_incoming_transfers":
                    if let address = operation["address"] as? String,
                       executor.looked.contains(address.lowercased()) {
                        self?.scanned.insert(address.lowercased())
                    }
                default: break
                }
                return answer
            },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { print("[vela-wallet] activity_feed fault: \($0)") },
            // The view as the core wrote it, for the AI agent's world (spec 104).
            keepsJson: true,
            // Every name looked up for a row comes back as a whole view, and
            // most change nothing (native parity Phase 4). `commit` only
            // mirrors it, and an arrival is a CHANGED `new_item_id`.
            skipsUnchanged: true
        )
    }

    /// The multisig accounts machine's names and networks, for the executor
    /// (r9-12: one name per address; r9-17: a multisig's networks scanned).
    func names(
        labels: @escaping @MainActor () -> [String: String],
        accountNetworks: @escaping @MainActor (String) -> [Int]
    ) {
        executor.labels = labels
        executor.accountNetworks = accountNetworks
    }

    /// The web's `feed.svelte.ts`: a NEW non-null `new_item_id` is an arrival.
    func commit(_ view: FeedViewWire) {
        let arrived = view.newItemId != nil && view.newItemId != feed?.newItemId
        feed = view
        if arrived { onNewItem() }
        follow(view.nextScan)
    }

    // MARK: - r9-17: the core's scan cadence

    /// The app as the cadence sees it: shown or not, when last touched.
    private func presence(nowMs: Double = Date().timeIntervalSince1970 * 1000) -> [String: Any] {
        ["visible": visible, "since_interaction_ms": max(0, nowMs - lastInteractionMs), "now_ms": nowMs]
    }

    private func report(_ change: String) {
        guard following else { return }
        core.dispatch(CoreJSON.string(["type": "presence_changed", "presence": presence(), "change": change]))
    }

    /// One timer for the core's `next_scan`, re-armed whenever its `seq`
    /// changes, answered with `scan_due` and the app as it stands then.
    private func follow(_ next: FeedNextScanWire?) {
        guard next?.seq != armedSeq else { return }
        dueTimer?.cancel()
        dueTimer = nil
        armedSeq = next?.seq
        guard let next else { return }
        dueTimer = Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(max(0, next.afterMs)) * 1_000_000)
            guard !Task.isCancelled, let self else { return }
            self.dueTimer = nil
            self.core.dispatch(CoreJSON.string(["type": "scan_due", "seq": next.seq, "presence": self.presence()]))
        }
    }

    /// The app came to the front, or left it.
    func sceneActive(_ active: Bool) {
        guard active != visible else { return }
        visible = active
        if active { lastInteractionMs = Date().timeIntervalSince1970 * 1000 }
        report(active ? "shown" : "hidden")
    }

    /// The network came back: what failed offline can be read now.
    func online() { report("online") }

    /// The person is at the app (a screen opened, a tab chosen).
    func interacted() {
        let now = Date().timeIntervalSince1970 * 1000
        lastInteractionMs = now
        guard now - lastReportMs >= Self.interactionReportMs else { return }
        lastReportMs = now
        report("interacted")
    }

    /// Called from the home screen's `.task`, with the signed-in address.
    ///
    /// Re-entering the tab must not re-announce the same account: the switch
    /// discards the feed and every in-flight answer, so doing it needlessly is
    /// a screen that empties itself for no reason.
    func open(address: String, hidden: Bool = false) {
        guard !address.isEmpty else { return }
        let event = CoreJSON.string(["type": "account_switched", "address": address])
        if core.boot(event) {
            scopedTo = address
        } else if scopedTo != address {
            scopedTo = address
            core.dispatch(event)
        } else {
            // Same account, screen re-appeared: a tick, not a switch.
            core.dispatch(CoreJSON.string(["type": "focus_tick"]))
        }
        // The balance machine hydrates its hide state from storage at boot, so
        // a person who concealed their balance is concealing it again before
        // any gesture happens. Telling this core at open is what stops a
        // receipt toast announcing an amount over a hidden hero.
        privacyChanged(hidden: hidden)
        // A core booted just now has not heard the space: say it again.
        if let spaceChainIds { spaceChains(spaceChainIds) }
        // r9-17: follow the core's cadence from now on — the account hand-off
        // scanned already; this arms the next one.
        if !following {
            following = true
            report("started")
        }
    }

    /// A pull to refresh: the core re-reads and re-scans by its own rules.
    func focusTick() { core.dispatch(CoreJSON.string(["type": "focus_tick"])) }

    /// r10 follow-up: the person pulled to refresh (or pressed the hero's
    /// refresh). Following the cadence, the core reads the store, scans at
    /// once and starts the back-off again (`presence_changed {refreshed}`);
    /// before that, the plain re-read.
    func refreshed() {
        guard following else { return focusTick() }
        lastInteractionMs = Date().timeIntervalSince1970 * 1000
        report("refreshed")
    }

    /// r10 follow-up: the person sent from this account — the operation was
    /// handed to the tracker. The core scans at once (the change, a refund,
    /// the other side's answer) and starts the back-off again; the send's own
    /// record re-reads the store through `reconciled`. One op is handed off
    /// twice when its write-ahead is admitted (spec 082 RJ1): said once.
    func sent(userOpHash: String) {
        let op = userOpHash.lowercased()
        guard !op.isEmpty, !sentOps.contains(op) else { return }
        sentOps.append(op)
        if sentOps.count > 16 { sentOps.removeFirst() }
        report("sent")
    }

    private var sentOps: [String] = []

    /// r10 follow-up: a list (`list`: the home, the history) started moving
    /// under a finger — a drag, a fling — or settled. A scan that comes due
    /// meanwhile is held and runs when the last list settles; nothing of the
    /// cadence redraws while one moves (`interacting {on, presence}`).
    /// Repeats are dropped here; the core would take them too.
    func scrolling(_ list: String, _ on: Bool) {
        let was = !movingLists.isEmpty
        if on { movingLists.insert(list) } else { movingLists.remove(list) }
        let now = !movingLists.isEmpty
        guard now != was, following else { return }
        if now { lastInteractionMs = Date().timeIntervalSince1970 * 1000 }
        core.dispatch(CoreJSON.string(["type": "interacting", "on": now, "presence": presence()]))
    }

    /// The balance's hide state. The core suppresses the receipt toast while it
    /// is on — the row still glows and the haptic still fires, which is exactly
    /// what a hidden balance should do: say something arrived without saying
    /// how much.
    func privacyChanged(hidden: Bool) {
        core.dispatch(CoreJSON.string(["type": "privacy_changed", "hidden": hidden]))
    }

    /// The network chip. `nil` is all chains.
    /// The tracker patched records on disk, so the feed re-reads.
    ///
    /// The count matters to the core: zero resolved verdicts is not a reason to
    /// re-read, and a feed that re-read on every poll would rebuild itself
    /// three times a second while a send is in flight.
    func reconciled(count: Int = 1) {
        core.dispatch(CoreJSON.string([
            "type": "reconcile_completed", "resolved_count": max(0, count),
        ]))
    }

    /// The space in front's chains (spec 105): only their records are drawn.
    /// The others stay stored, and switching back draws them again.
    func spaceChains(_ chainIds: [Int]) {
        spaceChainIds = chainIds
        core.dispatch(CoreJSON.string(["type": "space_chains_changed", "chain_ids": chainIds]))
    }

    private var spaceChainIds: [Int]?

    func chainFilter(_ chainId: Int?) {
        core.dispatch(CoreJSON.string([
            "type": "chain_filter_changed",
            "chain_id": chainId.map { $0 as Any } ?? NSNull(),
        ]))
    }

    /// A contact's page opened (`address`) or closed (`nil`): the view then
    /// carries that contact's rows (`contactRows`, spec 093) — the feed's own
    /// items, never rows a page builds from raw records.
    func contactFilter(_ address: String?) {
        core.dispatch(CoreJSON.string([
            "type": "contact_filter_changed",
            "address": address.map { $0 as Any } ?? NSNull(),
        ]))
    }

    /// Swipe to delete one row. Optimistic in the core: the row leaves the feed
    /// at once and comes back on the next read if the storage write failed.
    func deleteRequested(id: String) {
        core.dispatch(CoreJSON.string(["type": "delete_requested", "id": id]))
    }
}
