//
//  FeeStore.swift
//  VelaWallet
//
//  The resident `fee_policy` machine, and the one thing its caller wants.
//
//  Two shapes in one file, because they are two halves of the same seam:
//
//  - a `CoreStore<FeeViewWire>` like every other machine's, whose view drives
//    the fee row and the fee-token sheet;
//  - a small `async` facade, because `send`'s `estimate_fee` operation must be
//    ANSWERED, and answering it means waiting for a quote.
//
//  `RpcPool` is the only other machine in the client with a facade, and it has
//  one for the same reason. This one is narrower: at most one quote is in
//  flight per surface, and a generation token retires a late answer rather
//  than letting it land on a newer attempt.
//
//  ## Why the two machines are bridged HERE and not in the core
//
//  `send` and `fee_policy` are kept apart on purpose — the signing sheet uses
//  the fee machine without the send machine existing at all. Every client
//  bridges them in the shell (web `send-executor.ts:237`, desktop
//  `money.rs:458`, Android's `FeeQuoter`), and this is that bridge.
//

import Foundation
import Observation
import VelaCore

extension FeePolicyCore: CoreBridge {}

@MainActor
@Observable
final class FeeStore {

    private(set) var view: FeeViewWire?

    private var core: CoreStore<FeeViewWire>!
    /// Who is waiting for the quote in flight, and for which attempt.
    private var waiting: [(generation: Int, resume: (FeeViewWire?) -> Void)] = []
    private var generation = 0
    /// The last view's busy flag, so a settle can be recognised as an EDGE
    /// rather than as a level — a quote that was never busy still settles.
    private var requested = false

    init(relay: RelayClient, accounts: UserOpSpine.AccountPort) {
        let executor = FeeExecutor(relay: relay, accounts: accounts)
        self.core = CoreStore(
            bridge: FeePolicyCore(),
            perform: { operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { print("[vela-wallet] fee_policy fault: \($0)") }
        )
    }

    /// The machine has no boot event of its own: its first event IS a quote
    /// request. `CoreStore` drops events sent before boot, so the first request
    /// boots it and every later one dispatches.
    private func send(_ json: String) {
        if !core.boot(json) { core.dispatch(json) }
    }

    private func commit(_ view: FeeViewWire) {
        self.view = view
        guard requested, !view.busy else { return }
        requested = false
        let settled = waiting.filter { $0.generation == generation }
        waiting.removeAll()
        for entry in settled { entry.resume(view) }
    }

    /// Ask for a quote and wait for it to settle.
    ///
    /// Answers the view as it stands once the machine stops being busy —
    /// including a failed one, because a failure is an answer and `send` has a
    /// variant for it. A caller superseded by a newer request is resumed with
    /// `nil` rather than left hanging.
    func quote(
        chainId: Int,
        account: String,
        deployed: Bool,
        publicKeyAvailable: Bool,
        calls: [[String: Any]],
        feeToken: String?
    ) async -> FeeViewWire? {
        generation += 1
        let mine = generation
        // Everybody older is superseded. Resuming them with `nil` is what keeps
        // `estimate_fee` answered exactly once per operation.
        let stale = waiting
        waiting.removeAll()
        for entry in stale { entry.resume(nil) }

        requested = true
        return await withCheckedContinuation { continuation in
            var resumed = false
            waiting.append((mine, { view in
                guard !resumed else { return }
                resumed = true
                continuation.resume(returning: view)
            }))
            send(CoreJSON.string([
                "type": "quote_requested",
                "chain_id": chainId,
                "account": account,
                "deployed": deployed,
                "public_key_available": publicKeyAvailable,
                // `fast` is the send flow's tier on every client.
                "tier": "fast",
                "calls": calls,
                "fee_token": feeToken.map { $0 as Any } ?? NSNull(),
            ]))
        }
    }

    /// A fee-asset chip tap. `nil` = the native coin.
    ///
    /// The token is a QUOTE PARAMETER, not a post-quote adjustment: the fee leg
    /// batched into the simulated operation is a native transfer for `nil` and
    /// an ERC-20 `transfer` for a contract — 68 bytes of calldata and one real
    /// storage write apart. Re-denominating a native quote would price a
    /// different operation than the one that gets submitted.
    func selectAsset(_ token: String?) {
        send(CoreJSON.string([
            "type": "select_fee_asset",
            "token": token.map { $0 as Any } ?? NSNull(),
        ]))
    }

    /// The refresh affordance on a stale quote.
    func requote() { send(CoreJSON.string(["type": "requote"])) }

    /// Leaving the confirm step.
    func leaveConfirm() { send(CoreJSON.string(["type": "leave_confirm"])) }

    /// The form now targets a different chain, so every earlier quote is
    /// invalid for it (the core's invariant ①).
    func chainChanged(_ chainId: Int) {
        send(CoreJSON.string(["type": "chain_changed", "chain_id": chainId]))
    }
}
