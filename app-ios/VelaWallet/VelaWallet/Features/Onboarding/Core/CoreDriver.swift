//
//  CoreDriver.swift
//  VelaWallet
//
//  Platform-shell plumbing for a Crux core, blind to product semantics.
//
//  It knows how to dispatch an event, perform the effects that come back, and
//  hand the answers to the core. It knows nothing about wallets — which is why
//  the create machine, the login machine and the session machine all share it
//  instead of each screen re-deriving the same failure and cancellation rules.
//
//      View                    CoreDriver               executor
//       │ dispatch(event) ───────►│
//       │                         │ perform(operation) ──────►│  (a Task each)
//       │◄──── onView(view) ──────│◄─────────── result json ──│
//       │                         │ resolveEffect(id, result) …until it drains
//
//  Three properties are the whole contract, and each is a bug that is easy to
//  write and hard to see:
//
//  1. **Bridge calls are serialized.** `@MainActor` supplies that here: every
//     mutation of the core and every `onView` happens on one actor, so the
//     order the screen renders is the core's, not the scheduler's. Two effects
//     resolving in the same instant would otherwise interleave their views, and
//     the LAST to arrive — not the latest state — would be what is drawn.
//  2. **Nothing thrown by an executor reaches the loop.** `perform` owes a
//     result variant for every failure; if one still escapes, it is a shell bug
//     and the loop reports it rather than dying and leaving the core waiting
//     forever on an effect nobody will answer.
//  3. **A cancelled effect is not answered.** The core asked for it to be
//     abandoned, so it is not waiting — resolving it would push a stale answer
//     into a machine that moved on.
//

import Foundation
import VelaCore

/// The bridge methods, without caring which machine is behind them.
///
/// uniffi generates one class per exported object with no shared supertype, so
/// this protocol is what lets `CoreDriver` be written once.
protocol CoreBridge {
    func dispatch(eventJson: String) throws -> String
    func resolveEffect(effectId: UInt64, resultJson: String) throws -> String
    func view() throws -> String
    /// Opt in to lighter answers (core-perf-1): from the next one on, each
    /// carries `view_version`, and `view` only when it differs from the last
    /// one this object sent. `includeVolatile`: also send the machine's
    /// volatile fields, apart, in `volatile` (for a shell that reads them).
    func skipUnchangedViews(includeVolatile: Bool) throws
}

extension CreateWalletCore: CoreBridge {}
extension LoginCore: CoreBridge {}
extension SessionCore: CoreBridge {}

@MainActor
final class CoreDriver {
    private let bridge: CoreBridge
    private let perform: @MainActor ([String: Any]) async -> String
    private let onView: ([String: Any]) -> Void
    private let onFault: (Error) -> Void
    /// The machine's own failure variant for an effect the core refused to
    /// resolve. `nil` (or a machine with no such variant) leaves the loop
    /// reporting the fault and nothing else — which is the honest thing when
    /// there is no failure to express.
    var toFailure: ((UInt64, Error) -> String?)?
    /// The same, keyed by the OPERATION rather than the effect id: the
    /// machine's neutral answer for what was asked (spec 070 — every browser
    /// operation has one, so an answer the core refuses still settles a page
    /// instead of leaving it waiting). Consulted when `toFailure` has none.
    var neutralAnswer: (([String: Any]) -> String?)?

    /// An answer that carries no view — the core's "keep the one you hold"
    /// (core-perf-1) — is not committed again. Off by default: `onView` then
    /// hears the view it already holds on every answer, as it always did.
    /// `CoreStore` turns it on and tells its own listener, from the view it
    /// has already decoded, through `onUnchanged`.
    var skipsUnchangedView = false
    /// Called for an answer whose view did not move, when `skipsUnchangedView`.
    var onUnchanged: (() -> Void)?
    /// The last view the core sent — what an answer without one means.
    private var heldView: [String: Any]?
    /// The machine's volatile fields as last sent (only to a driver that
    /// asked for them), folded back into every view committed.
    private var heldVolatile: [String: Any] = [:]

    private var running: [UInt64: Task<Void, Never>] = [:]
    private var disposed = false

    /// No effect is in flight: the machine is waiting on nothing but its next
    /// event, so its view cannot change until somebody sends one.
    ///
    /// Exact, not a guess: an effect's task leaves `running` and the effects
    /// its answer raises join it in the same main-actor turn (`run` →
    /// `resolve` → `apply`), so no turn ever sees an empty map while an
    /// answer is still owed. It is what lets a test wait for the machine to
    /// FINISH rather than for a clock — a clock measures how busy the
    /// scheduler was, which on a shared CI runner is not the machine.
    var isIdle: Bool { running.isEmpty }
    /// How many effects are in flight, by the same count.
    var inFlight: Int { running.count }

    init(
        bridge: CoreBridge,
        /// Perform one operation and return the result JSON. Must not throw.
        perform: @escaping @MainActor ([String: Any]) async -> String,
        /// Called on every committed view, in the order the core produced them.
        onView: @escaping ([String: Any]) -> Void,
        /// A shell fault: a malformed event, an escaped error. Never a user error.
        onFault: @escaping (Error) -> Void = { _ in },
        /// Ask for the machine's volatile fields too
        /// (`CoreBridge.skipUnchangedViews`). No machine this shell runs reads
        /// them: the RPC pool's `pending_urls` are a hedging shell's.
        includeVolatile: Bool = false
    ) {
        self.bridge = bridge
        self.perform = perform
        self.onView = onView
        self.onFault = onFault
        // Every machine opts in: an unchanged view is neither serialized into
        // the answer, nor crossed, nor parsed here (core-perf-1). A bridge
        // that refused keeps answering with the whole view, which reads the
        // same below. Said in the log, not to `onFault`: an owner may still
        // be wiring that closure while this runs (`CoreStore.init`).
        do {
            try bridge.skipUnchangedViews(includeVolatile: includeVolatile)
        } catch {
            print("[vela-wallet] core: skip_unchanged_views refused, whole views it is — \(error)")
        }
    }

    /// Emit the core's current view without sending anything.
    func start() {
        do {
            let view = try CoreJSON.object(bridge.view())
            heldView = view
            commit(view)
        } catch {
            onFault(error)
        }
    }

    /// Send one event, as the JSON the core's `Event` deserializes from.
    func dispatch(_ eventJson: String) {
        do {
            try applyText(bridge.dispatch(eventJson: eventJson))
        } catch {
            onFault(error)
        }
    }

    /// Stop driving. In-flight effects are cancelled and their answers dropped:
    /// a view produced after the screen has gone has nowhere to render.
    func dispose() {
        disposed = true
        running.values.forEach { $0.cancel() }
        running.removeAll()
    }

    // MARK: - The loop

    /// One answer from the core, as text.
    private func applyText(_ text: String) throws {
        // Each answer's Foundation objects go when it has been applied. The
        // main queue drains its pool once per run-loop pass, and a launch's
        // hundreds of answers in one pass held every parsed view at once
        // (native parity Phase 4).
        try autoreleasepool { try apply(CoreJSON.object(text)) }
    }

    /// The core sends a view only when it moved (core-perf-1): an answer
    /// without `view` means "keep the one you hold", and `volatile`, when it
    /// comes, is folded back into the view it belongs to. The answers of a
    /// launch are mostly the first kind — a pool call settled, a name was not
    /// found — and parsing a 200-row feed again to learn that was most of
    /// what a launch spent on the main thread (native parity Phase 4).
    private func apply(_ result: [String: Any]) {
        let volatile = result["volatile"] as? [String: Any]
        if let volatile { heldVolatile = volatile }
        if let view = result["view"] as? [String: Any] {
            heldView = view
            commit(folded(view))
        } else if let heldView {
            if volatile == nil, skipsUnchangedView {
                if !disposed { onUnchanged?() }
            } else {
                commit(folded(heldView))
            }
        }

        for raw in result["cancelled_effect_ids"] as? [Any] ?? [] {
            guard let id = (raw as? NSNumber)?.uint64Value else { continue }
            running.removeValue(forKey: id)?.cancel()
        }

        for effect in result["effects"] as? [[String: Any]] ?? [] {
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let operation = effect["operation"] as? [String: Any]
            else { continue }
            run(id: id, operation: operation)
        }
    }

    /// The view with the volatile fields last sent put back in.
    private func folded(_ view: [String: Any]) -> [String: Any] {
        guard !heldVolatile.isEmpty else { return view }
        return view.merging(heldVolatile) { _, volatile in volatile }
    }

    private func commit(_ view: [String: Any]) {
        // A view produced before disposal can still arrive after it (an effect
        // resolving while the screen goes away). Dropping it keeps the UI from
        // being asked to render into a torn-down tree.
        guard !disposed else { return }
        onView(view)
    }

    private func run(id: UInt64, operation: [String: Any]) {
        guard !disposed else { return }
        let task = Task { @MainActor [weak self] in
            guard let self else { return }
            let resultJson = await self.perform(operation)
            // Property 3: the core abandoned this operation, so it is not
            // waiting for an answer and giving it one would be the bug.
            guard !Task.isCancelled else {
                self.running.removeValue(forKey: id)
                return
            }
            self.running.removeValue(forKey: id)
            self.resolve(id: id, operation: operation, resultJson: resultJson)
        }
        running[id] = task
    }

    private func resolve(id: UInt64, operation: [String: Any], resultJson: String) {
        guard !disposed else { return }
        do {
            try applyText(bridge.resolveEffect(effectId: id, resultJson: resultJson))
        } catch {
            // The core REFUSED this answer — a result it cannot deserialise,
            // usually because the shell and the machine disagree about a shape.
            //
            // Reporting the fault is not enough. The machine asked a question
            // and is still waiting for it: without an answer the screen sits on
            // its loading state forever, which is exactly what somebody sees as
            // a wallet that will not open (contract 048 §2).
            //
            // So the machine gets its OWN failure variant, exactly once, and
            // then the fault is reported. If a second answer is refused too,
            // the loop stops there rather than recursing.
            onFault(error)
            guard let failure = toFailure?(id, error) ?? neutralAnswer?(operation) else { return }
            do {
                try applyText(bridge.resolveEffect(effectId: id, resultJson: failure))
            } catch {
                print("[vela-wallet] core: the failure answer was refused too — \(error)")
                onFault(error)
            }
        }
    }
}
