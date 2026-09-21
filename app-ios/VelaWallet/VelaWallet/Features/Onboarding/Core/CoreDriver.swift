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

/// The three bridge methods, without caring which machine is behind them.
///
/// uniffi generates one class per exported object with no shared supertype, so
/// this protocol is what lets `CoreDriver` be written once.
protocol CoreBridge {
    func dispatch(eventJson: String) throws -> String
    func resolveEffect(effectId: UInt64, resultJson: String) throws -> String
    func view() throws -> String
}

extension CreateWalletCore: CoreBridge {}
extension LoginCore: CoreBridge {}
extension SessionCore: CoreBridge {}

@MainActor
final class CoreDriver {
    private let bridge: CoreBridge
    private let perform: ([String: Any]) async -> String
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

    private var running: [UInt64: Task<Void, Never>] = [:]
    private var disposed = false

    init(
        bridge: CoreBridge,
        /// Perform one operation and return the result JSON. Must not throw.
        perform: @escaping ([String: Any]) async -> String,
        /// Called on every committed view, in the order the core produced them.
        onView: @escaping ([String: Any]) -> Void,
        /// A shell fault: a malformed event, an escaped error. Never a user error.
        onFault: @escaping (Error) -> Void = { _ in }
    ) {
        self.bridge = bridge
        self.perform = perform
        self.onView = onView
        self.onFault = onFault
    }

    /// Emit the core's current view without sending anything.
    func start() {
        do {
            commit(try CoreJSON.object(bridge.view()))
        } catch {
            onFault(error)
        }
    }

    /// Send one event, as the JSON the core's `Event` deserializes from.
    func dispatch(_ eventJson: String) {
        do {
            apply(try CoreJSON.object(bridge.dispatch(eventJson: eventJson)))
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

    private func apply(_ result: [String: Any]) {
        commit(result["view"] as? [String: Any] ?? [:])

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
            apply(try CoreJSON.object(bridge.resolveEffect(effectId: id, resultJson: resultJson)))
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
                apply(try CoreJSON.object(
                    bridge.resolveEffect(effectId: id, resultJson: failure)
                ))
            } catch {
                print("[vela-wallet] core: the failure answer was refused too — \(error)")
                onFault(error)
            }
        }
    }
}
