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
//  ## The cadence is the shell's, deliberately
//
//  `activity_feed.rs` keeps the *decisions* — when a read may celebrate, when a
//  sync earns a re-read — and leaves the clock here, because which screen is
//  visible is render-domain state the core never sees. So: a tick when the home
//  appears or the app returns to the foreground, and one every thirty seconds
//  while it is up.
//

import Foundation
import Observation
import VelaCore

extension ActivityFeedCore: CoreBridge {}

@MainActor
@Observable
final class ActivityStore {

    /// The auto-refresh while the home is on screen. Web's is the same thirty
    /// seconds; the ten-second Activity-tab poll waits for a tab that exists.
    private static let tickSeconds: UInt64 = 30
    /// The Activity TAB's own cadence — near-real-time while somebody is
    /// looking at the list, because that is the screen where a new transfer
    /// arriving late is most obviously late.
    private static let liveTickSeconds: UInt64 = 10

    private(set) var feed: FeedViewWire?
    /// Whether the local store has been read at least once.
    ///
    /// The view cannot say: an unread machine and an account with no history
    /// both publish an empty row list, and the two must not draw the same. An
    /// empty feed is a claim ("nothing has happened here"); before the read
    /// lands, the honest surface is the drawn skeleton (FR-008).
    private(set) var hasRead = false

    private let executor: ActivityExecutor
    private var core: CoreStore<FeedViewWire>!
    private var scopedTo: String?
    private var ticker: Task<Void, Never>?
    private var liveTicker: Task<Void, Never>?

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
                if operation["type"] as? String == "read_tx_store" { self?.hasRead = true }
                return answer
            },
            onView: { [weak self] view in self?.feed = view },
            onFault: { print("[vela-wallet] activity_feed fault: \($0)") }
        )
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
        startTicking()
    }

    /// The auto-refresh. Stopped when the home goes away, so a backgrounded app
    /// is not sweeping twelve chains every half minute.
    private func startTicking() {
        guard ticker == nil else { return }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.tickSeconds * 1_000_000_000)
                guard !Task.isCancelled else { return }
                self?.focusTick()
            }
        }
    }

    func stopTicking() {
        ticker?.cancel()
        ticker = nil
    }

    func focusTick() { core.dispatch(CoreJSON.string(["type": "focus_tick"])) }

    /// The 10-second poll while the activity list is on screen.
    ///
    /// A separate cadence from the home's 30 seconds, and a separate EVENT:
    /// the core distinguishes them, and this client sent only the slow one.
    func startLiveTicking() {
        guard liveTicker == nil else { return }
        liveTicker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.liveTickSeconds * 1_000_000_000)
                guard !Task.isCancelled else { return }
                self?.core.dispatch(CoreJSON.string(["type": "live_tick"]))
            }
        }
    }

    func stopLiveTicking() {
        liveTicker?.cancel()
        liveTicker = nil
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

    func chainFilter(_ chainId: Int?) {
        core.dispatch(CoreJSON.string([
            "type": "chain_filter_changed",
            "chain_id": chainId.map { $0 as Any } ?? NSNull(),
        ]))
    }

    /// Swipe to delete one row. Optimistic in the core: the row leaves the feed
    /// at once and comes back on the next read if the storage write failed.
    func deleteRequested(id: String) {
        core.dispatch(CoreJSON.string(["type": "delete_requested", "id": id]))
    }
}
