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
//  1. **Foreground** — a tick while anything is pending, at the core's cadence.
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

@MainActor
@Observable
final class TrackerStore {

    private(set) var view: TrackViewWire?

    private var core: CoreStore<TrackViewWire>!
    private let executor: TrackerExecutor
    /// The foreground tick. Alive only while something is pending.
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
            onFault: { print("[vela-wallet] tx_tracker fault: \($0)") }
        )
    }

    /// Read the pending set and start following it. Called once, at launch.
    func boot() {
        TrackerBackgroundBridge.store = self
        // `AppResumed` is the machine's "look again" — the same event a return
        // from the background sends, which is exactly what a launch is.
        core.boot(CoreJSON.string(["type": "app_resumed"]))
    }

    /// A send was just accepted by the relay.
    func submitted(userOpHash: String, recordIds: [String], chainId: Int) {
        let event = CoreJSON.string([
            "type": "submitted",
            "user_op_hash": userOpHash,
            "record_ids": recordIds,
            "chain_id": chainId,
        ])
        if !core.boot(event) { core.dispatch(event) }
    }

    func resumed() { core.dispatch(CoreJSON.string(["type": "app_resumed"])) }
    func homeFocused() { core.dispatch(CoreJSON.string(["type": "home_focused"])) }

    private func commit(_ view: TrackViewWire) {
        self.view = view
        // The tick exists exactly as long as there is something to follow.
        if view.hasPending { startTicking() } else { stopTicking() }
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
