//
//  TrackerStore.swift
//  VelaWallet
//
//  The resident `tx_tracker` machine, and the clock it needs.
//
//  Booted at launch rather than with a screen, because what it follows outlives
//  every screen: a person confirms a send and immediately locks the phone, and
//  the verdict has to reach the record either way.
//
//  ## What this client can honestly promise
//
//  Three layers, and only the first two are the shell's to schedule:
//
//  1. **Foreground** — a tick while the core follows anything (spec 079: past
//     the wait window too), at the core's cadence.
//  2. **Backgrounding** — `beginBackgroundTask` grace, roughly thirty seconds,
//     read from `backgroundTimeRemaining` rather than assumed.
//  3. **Later** — a `BGAppRefreshTask`, which iOS runs at its own discretion
//     and may never run at all, and a relaunch, which always does.
//
//  So the promise is **not a cadence**. It is that no verdict is lost: the
//  pending set is derived from `vela.transactionHistory`, every launch resumes
//  it, and the core abandons at twenty-four hours. Android's worker gives two
//  minutes; iOS gives half a minute and a maybe, and saying otherwise would be
//  a promise the platform does not keep.
//

import Foundation
import Observation
import BackgroundTasks
import UIKit
import VelaCore

extension TxTrackerCore: CoreBridge {}

/// One op handed to the tracker: what it follows, and — for an op whose
/// submit reply was lost — that it may have been sent and the head read
/// before its first POST (spec 082 RA4, ruling 8). Written with the record
/// and read back on a relaunch (T183), so the handoff and a restart say the
/// same thing.
struct TrackSubmission: Equatable {
    let userOpHash: String
    let recordIds: [String]
    let chainId: Int
    var maybeSent = false
    var submitBlock: Int? = nil
    /// The relay took the op a write-ahead hand-off announced (spec 082 RJ1):
    /// the entry is acknowledged — never "may have been sent" — and a
    /// premature "not sent" reached while the POST was still out yields.
    var admitted = false
    /// The account that signed it (PR 2): what makes the op one this account
    /// must wait for on this chain (`in_flight_ops`). `nil` from a caller
    /// that does not know.
    var sender: String? = nil
}

@MainActor
@Observable
final class TrackerStore {

    private(set) var view: TrackViewWire?
    /// The operations holding their account's nonce (PR 2 §3): the core's
    /// `in_flight_ops` over its OWN view JSON — never a view re-encoded from
    /// `TrackViewWire`, whose entries carry neither `sender` nor `stalled`
    /// (with them gone the list is empty, or the hold never times out). A
    /// JSON array, set before `onView` and the followers hear the view, and
    /// forwarded on every render to the send and signing machines.
    private(set) var inFlightOpsJson = "[]"
    /// Every view, to whoever else watches the same operations — the send
    /// machine's receipt (spec 082: its verdict comes from here, through the
    /// core's `sendReceiptOutcomeOf`).
    var onView: ((TrackViewWire) -> Void)?

    /// Who else follows the view while they live — a dApp request's sheet,
    /// whose answer follows what the tracker knows (spec 082 RJ4). Held
    /// weakly: a request that ended stops listening by going away.
    private var followers: [(owner: WeakOwner, onView: (TrackViewWire) -> Void)] = []

    private final class WeakOwner {
        weak var value: AnyObject?
        init(_ value: AnyObject) { self.value = value }
    }

    /// Follow every view from now on while `owner` lives — the current one
    /// at once, so a follower never waits for the next change to hear it.
    func follow(_ owner: AnyObject, _ onView: @escaping (TrackViewWire) -> Void) {
        followers.removeAll { $0.owner.value == nil }
        followers.append((WeakOwner(owner), onView))
        if let view { onView(view) }
    }

    private var core: CoreStore<TrackViewWire>!
    private let executor: TrackerExecutor

    /// No effect in flight — `CoreDriver.isIdle`. The foreground tick is not
    /// one: between ticks the machine is waiting on nothing.
    var isIdle: Bool { core.isIdle }
    /// The foreground tick. Alive only while the core follows something.
    private var ticker: Task<Void, Never>?
    private var graceTask: UIBackgroundTaskIdentifier = .invalid

    /// The core's own foreground cadence (`tracker-resident.ts:61`, and the
    /// desktop's `tracker.rs:53`).
    private static let tickMs: UInt64 = 3_000

    init(executor: TrackerExecutor) {
        self.executor = executor
        self.core = CoreStore(
            bridge: TxTrackerCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { VelaLog.failure(.tracker, kind: "tx_tracker_fault", VelaLog.error($0)) },
            // The view as the core wrote it, for `in_flight_ops` (PR 2).
            keepsJson: true
        )
    }

    /// Read the pending set and start following it. Called once, at launch.
    func boot() {
        TrackerBackgroundBridge.store = self
        // `AppResumed` is the machine's "look again" — the same event a return
        // from the background sends, which is exactly what a launch is.
        core.boot(CoreJSON.string(["type": "app_resumed"]))
    }

    /// A send was just accepted by the relay — or may have been: `maybeSent`
    /// follows it to its end under the local hash, from `submitBlock`
    /// (spec 082 RA4, ruling 8).
    func submitted(_ submission: TrackSubmission) {
        let event = CoreJSON.string([
            "type": "submitted",
            "user_op_hash": submission.userOpHash,
            "record_ids": submission.recordIds,
            "chain_id": submission.chainId,
            "maybe_sent": submission.maybeSent,
            "submit_block": submission.submitBlock.map { $0 as Any } ?? NSNull(),
            "admitted": submission.admitted,
            // Optional on the wire: an unknown signer is simply not sent.
            "sender": submission.sender.map { $0 as Any } ?? NSNull(),
        ])
        if !core.boot(event) { core.dispatch(event) }
    }

    /// A write-ahead op proven never sent (spec 082 RJ1): the tracker drops
    /// these records from it — an entry left with none goes, with no patch
    /// and no balance read. Idempotent; a later hand-off of the same hash
    /// starts fresh. Before the tracker has booted there is nothing to take
    /// back (the record is already gone from the store it will read), so the
    /// event is not the one that boots it.
    func withdrawn(userOpHash: String, recordIds: [String]) {
        core.dispatch(CoreJSON.string([
            "type": "withdrawn", "user_op_hash": userOpHash, "record_ids": recordIds,
        ]))
    }

    func resumed() { core.dispatch(CoreJSON.string(["type": "app_resumed"])) }
    func homeFocused() { core.dispatch(CoreJSON.string(["type": "home_focused"])) }

    private func commit(_ view: TrackViewWire) {
        self.view = view
        inFlightOpsJson = core.json.map { inFlightOps(trackViewJson: $0) } ?? "[]"
        onView?(view)
        followers.removeAll { $0.owner.value == nil }
        for follower in followers { follower.onView(view) }
        // The tick exists exactly as long as the core follows something —
        // not only inside the wait window (spec 079).
        if view.isFollowing { startTicking() } else { stopTicking() }
    }

    private func startTicking() {
        guard ticker == nil else { return }
        ticker = Task { [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.tickMs * 1_000_000)
                guard let self, !Task.isCancelled else { return }
                self.core.dispatch(CoreJSON.string(["type": "tick"]))
            }
        }
    }

    private func stopTicking() {
        ticker?.cancel()
        ticker = nil
    }

    // MARK: - Leaving and coming back

    /// The app went away with something in flight.
    ///
    /// The grace is the only window this platform guarantees, so it is used:
    /// keep ticking until iOS says the time is nearly up, then end the task
    /// cleanly — an unended background task is terminated by the watchdog, and
    /// a terminated app loses the launch that would have resumed it.
    func backgrounded() {
        guard view?.hasPending == true else { return }
        // Ask for a refresh BEFORE spending the grace: if the app is killed
        // mid-grace the request is already lodged.
        TrackerBackgroundBridge.schedule()
        endGrace()
        graceTask = UIApplication.shared.beginBackgroundTask(withName: "vela.tracker") { [weak self] in
            self?.endGrace()
        }
        Task { [weak self] in
            while let self, self.graceTask != .invalid {
                // Read the remaining time rather than assuming a number: the
                // grace has changed across iOS releases and will again.
                let left = UIApplication.shared.backgroundTimeRemaining
                guard left > 5, self.view?.hasPending == true else { break }
                self.core.dispatch(CoreJSON.string(["type": "tick"]))
                try? await Task.sleep(nanoseconds: Self.tickMs * 1_000_000)
            }
            self?.endGrace()
        }
    }

    func foregrounded() {
        endGrace()
        resumed()
    }

    private func endGrace() {
        guard graceTask != .invalid else { return }
        UIApplication.shared.endBackgroundTask(graceTask)
        graceTask = .invalid
    }

    /// One background-refresh round, for the task iOS may or may not run.
    ///
    /// Deliberately the same code the grace and the foreground tick use: a
    /// second polling path would be a second set of rules about what a receipt
    /// means.
    func refreshRound() async {
        resumed()
        // One cadence's worth of settling, so an in-flight poll can land before
        // the system suspends the app again.
        try? await Task.sleep(nanoseconds: Self.tickMs * 1_000_000)
    }
}

/// The seam between SwiftUI's `backgroundTask` and the resident tracker.
///
/// `backgroundTask` runs outside any view, so it cannot reach a `@State` store
/// directly. The store registers itself here at launch; if the app was
/// relaunched cold for the refresh, `RootView`'s own boot has already put one
/// in place by the time this runs.
@MainActor
enum TrackerBackgroundBridge {
    static weak var store: TrackerStore?

    static func run() async {
        guard let store else { return }
        await store.refreshRound()
    }

    /// Ask iOS for a refresh, once there is something worth waking up for.
    static func schedule() {
        let request = BGAppRefreshTaskRequest(identifier: VelaWalletApp.trackerTask)
        // Fifteen minutes is the floor iOS enforces; asking for sooner does not
        // make it sooner.
        request.earliestBeginDate = Date(timeIntervalSinceNow: 15 * 60)
        try? BGTaskScheduler.shared.submit(request)
    }
}
