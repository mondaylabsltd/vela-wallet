//
//  TokenTrustStore.swift
//  VelaWallet
//
//  The resident `token_trust` session, and the one call its users want.
//
//  Ported from `app-web/vela-wallet/src/lib/wallet/core/token-trust-resident.ts`.
//  This machine is **app-lifetime, not screen-lifetime**: the trusted-token set
//  it builds is shared by the receipt scanner, the auto-add admission and (in a
//  later cut) the sign sheet, and the entire point of unifying them is that
//  they see ONE set.
//
//  ## The second facade in the app, for the same reason as the first
//
//  Like `RpcPool`, this machine's caller wants an **answer** — "scan, and tell
//  me what landed" — rather than a view to render. So `pollIncoming` wraps the
//  event/view loop in one `await`. Everything else here is a snapshot push:
//  facts the core cannot fetch for itself.
//
//  ## This file owns no rules
//
//  It pushes what the core cannot fetch (held chains, held tokens, the
//  registry), turns one view field into a promise-shaped call, and serialises
//  callers so the core's single-flight poll is never raced from this side.
//  Every branch below is a wire translation or a wait.
//

import Foundation
import Observation
import VelaCore

extension TokenTrustCore: CoreBridge {}

@MainActor
@Observable
final class TokenTrustStore {

    /// A safety valve for the poll wrapper, not a decision.
    ///
    /// Every operation this machine issues is answered by the executor — the
    /// metadata arm answers every address precisely so a scan cannot wedge — so
    /// this should never fire. It exists because the caller is a scan the home
    /// screen awaits: an unanswered effect must degrade to "this tick found
    /// nothing", never to a spinner that never stops.
    private static let pollDeadline: TimeInterval = 60

    private(set) var trust: TrustViewWire?

    private let executor: TokenTrustExecutor
    private let registry: ChainTokens
    private let held: HeldTokens
    private var core: CoreStore<TrustViewWire>!
    /// Callers waiting for a settled scan.
    private var waiting: [(TrustViewWire) -> Void] = []
    /// One poll in flight, per the core's own single-flight rule.
    private var polling = false

    init(store: VelaStore, pool: RpcPool, accounts: AccountStore, held: HeldTokens) {
        self.executor = TokenTrustExecutor(
            store: store, pool: pool,
            metadata: TokenMetadata(store: store, pool: pool)
        )
        self.registry = ChainTokens(accounts: accounts)
        self.held = held
        self.core = CoreStore(
            bridge: TokenTrustCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { print("[vela-wallet] token_trust fault: \($0)") }
        )
    }

    private func commit(_ view: TrustViewWire) {
        trust = view
        guard !view.scanning else { return }
        let settled = waiting
        waiting = []
        for resume in settled { resume(view) }
    }

    /// Run one scan and answer with the judged incoming feed.
    ///
    /// The answer is the machine's whole de-duped memory for this account, not
    /// just this tick's window — the caller merges by the same stable id, so a
    /// repeat costs nothing and a row that only became renderable on a later
    /// poll still lands.
    func pollIncoming(address: String, chainIds: [Int]) async -> [TrustIncomingWire] {
        guard !address.isEmpty, !polling else { return [] }
        polling = true
        defer { polling = false }

        // The registry's facts first: they are the allowlist the scan is
        // restricted to, and a poll that started without them would scan
        // nothing on a cold launch.
        await primeRegistry(chainIds: chainIds)
        for chainId in chainIds {
            send(["type": "held_tokens_snapshot", "address": address, "chain_id": chainId,
                  "tokens": held.erc20Addresses(address: address, chainId: chainId)])
        }
        send(["type": "held_chains_snapshot", "address": address, "chain_ids": chainIds])
        send(["type": "poll_requested", "address": address])

        let settled = await waitForSettledScan()
        return settled?.incoming ?? []
    }

    /// The token registry's facts per chain — the same read web primes with,
    /// and the same degradation: a chain that cannot be reached contributes
    /// nothing, which leaves its stables out of the allowlist rather than
    /// inventing one.
    private func primeRegistry(chainIds: [Int]) async {
        for chainId in chainIds {
            guard let facts = await registry.facts(chainId: chainId) else { continue }
            send([
                "type": "registry_tokens_snapshot",
                "chain_id": chainId,
                "stables": facts.stables,
                "wrapped_native": facts.wrappedNative.map { $0 as Any } ?? NSNull(),
            ])
        }
    }

    /// Wait for the next view that is not scanning.
    ///
    /// No pre-check: `poll_requested` renders `scanning: true` before this is
    /// reached, so the current view is never the answer. A core that ignored
    /// the request — its single-flight rule — commits nothing further, and the
    /// deadline turns that into "this tick found nothing".
    private func waitForSettledScan() async -> TrustViewWire? {
        await withCheckedContinuation { continuation in
            var resumed = false
            let resume: (TrustViewWire?) -> Void = { view in
                guard !resumed else { return }
                resumed = true
                continuation.resume(returning: view)
            }
            waiting.append { view in resume(view) }
            Task { [weak self] in
                try? await Task.sleep(nanoseconds: UInt64(Self.pollDeadline * 1_000_000_000))
                // A give-up, not a decision: whatever the core has already
                // committed is the answer.
                resume(self?.trust)
            }
        }
    }

    /// `boot` commits the core's first view; everything after is a dispatch.
    /// A confirmation landed, with the logs the tracker just polled.
    ///
    /// **The single constructor of an admission session.** The core's own doc
    /// says so: this path may reach `WriteCustomToken`, and the sign-sheet's
    /// simulated deltas may reach it "through no code path at all". A receipt
    /// is something the chain produced; a simulation is something a site can
    /// author, and a wallet that admitted tokens from the second would let a
    /// scam seed itself.
    func receiptLogsConfirmed(from: String, chainId: Int, logs: [[String: Any]]) {
        guard !logs.isEmpty else { return }
        send([
            "type": "receipt_logs_confirmed",
            "from": from,
            "chain_id": chainId,
            "logs": logs,
        ])
    }

    /// The sign sheet's simulated deltas (spec 055).
    ///
    /// **This path can never write a token.** The core enforces it (invariant
    /// ⑤) and the reason is worth restating at the call site: a `Transfer` log
    /// in a SIMULATION is something the site being signed for controls, and
    /// admitting a token on that basis would let a page seed its own scam coin
    /// into somebody's list by asking them to look at a transaction.
    func simDeltasComputed(address: String, chainId: Int, deltas: [[String: Any]]) {
        send([
            "type": "sim_deltas_computed",
            "address": address,
            "chain_id": chainId,
            "deltas": deltas,
        ])
    }

    private func send(_ event: [String: Any]) {
        let json = CoreJSON.string(event)
        if core.boot(json) { return }
        core.dispatch(json)
    }
}
