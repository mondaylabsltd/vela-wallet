//
//  FeeStore.swift
//  VelaWallet
//
//  The send flow's `fee_policy` sessions, the speed control that decides how
//  fast they price, and the one thing `send`'s caller wants.
//
//  Three shapes in one file, because they are halves of the same seam:
//
//  - the fee SESSIONS: one in force — the quote the send machine pre-checks
//    against, the fee row shows and the submit signs — and, since spec 069,
//    one preview per other tier the speed core wants priced;
//  - the `fee_speed` core (spec 069), which decides the tier in force, the free
//    upgrade, the one-speed statement and which previews to keep — every rule
//    of the speed control, the same machine every other client drives;
//  - a small `async` facade, because `send`'s `estimate_fee` operation must be
//    ANSWERED, and answering it means waiting for a quote.
//
//  ## Why the machines are bridged HERE and not in the core
//
//  `send` and `fee_policy` are kept apart on purpose — the signing sheet uses
//  the fee machine without the send machine existing at all — and a machine
//  cannot hold another machine. Every client bridges them in the shell (web
//  `send-executor.ts`, desktop `money.rs`, Android's `SendController`), and
//  this is that bridge. What the shell owns of the speed control is the
//  reconcile step (`fee_speed.rs`): when the session in force is not pricing
//  the tier in force, PROMOTE the preview that already priced this operation at
//  that tier — the price tapped is the price paid (#681) — else re-price once
//  nothing is measuring.
//

import Foundation
import Observation
import VelaCore

extension FeePolicyCore: CoreBridge {}
extension FeeSpeedCore: CoreBridge {}

@MainActor
@Observable
final class FeeStore {

    /// The fee session in force — what the fee row and the fee-token sheet read.
    private(set) var view: FeeViewWire?
    /// Told every view of the session in force — for an owner that reacts to
    /// one (the signing sheet re-asks a stale quote). Promotions included.
    @ObservationIgnored var onInForce: ((FeeViewWire) -> Void)?
    /// The speed control, as the `fee_speed` core decided it (spec 069).
    private(set) var speed: FeeSpeedViewWire?

    /// What a session was asked to price; compared minus the tier.
    private struct Ask {
        let chainId: Int
        let account: String
        let deployed: Bool
        let publicKeyAvailable: Bool
        let tier: String
        let calls: [[String: Any]]
        let feeToken: String?

        /// The operation, tier aside — `calls` compared as the JSON the core
        /// reads, which is exact.
        func sameOperation(_ other: Ask) -> Bool {
            chainId == other.chainId && account == other.account && deployed == other.deployed
                && publicKeyAvailable == other.publicKeyAvailable && feeToken == other.feeToken
                && CoreJSON.string(["c": calls]) == CoreJSON.string(["c": other.calls])
        }

        func at(_ tier: String) -> Ask {
            Ask(chainId: chainId, account: account, deployed: deployed,
                publicKeyAvailable: publicKeyAvailable, tier: tier, calls: calls, feeToken: feeToken)
        }

        var event: String {
            CoreJSON.string([
                "type": "quote_requested",
                "chain_id": chainId,
                "account": account,
                "deployed": deployed,
                "public_key_available": publicKeyAvailable,
                "tier": tier,
                "calls": calls,
                "fee_token": feeToken.map { $0 as Any } ?? NSNull(),
            ])
        }
    }

    /// One `fee_policy` session: in force, or a preview of another tier.
    @MainActor
    private final class Session {
        let key: Int
        let core: CoreStore<FeeViewWire>
        var view: FeeViewWire?
        var ask: Ask?
        var generation = 0

        init(key: Int, core: CoreStore<FeeViewWire>) {
            self.key = key
            self.core = core
        }

        /// The machine has no boot event of its own: its first event IS a
        /// quote request. `CoreStore` drops events sent before boot, so the
        /// first request boots it and every later one dispatches.
        func send(_ json: String) {
            if !core.boot(json) { core.dispatch(json) }
        }

        nonisolated deinit {}
    }

    private let executor: FeeExecutor
    private let relay: RelayClient
    private var nextKey = 0
    private var inForce: Session!
    private var previews: [Session] = []
    private var speedCore: CoreStore<FeeSpeedViewWire>!
    /// Bumped whenever the session in force is asked to price again, so the
    /// previews follow it.
    private var askGeneration = 0

    /// Who is waiting for the quote in flight, and for which attempt.
    private var waiting: [(generation: Int, resume: (FeeViewWire?) -> Void)] = []
    private var generation = 0
    /// A caller is waiting on the session in force to settle.
    private var requested = false
    /// How long a quote may take before the shell calls it a failure. Generous:
    /// the core's own TTL is 30 s and a cold pool sweeps three passes.
    private static let settleDeadlineMs = 45_000

    init(relay: RelayClient, accounts: UserOpSpine.AccountPort) {
        self.executor = FeeExecutor(relay: relay, accounts: accounts)
        self.relay = relay
        self.inForce = newSession()
        self.speedCore = CoreStore(
            bridge: FeeSpeedCore(),
            // The speed core asks the shell for nothing.
            perform: { operation in
                print("[vela-wallet] fee_speed asked for an operation: \(operation)")
                return "{}"
            },
            onView: { [weak self] view in self?.speedChanged(view) },
            onFault: { print("[vela-wallet] fee_speed fault: \($0)") }
        )
        _ = speedCore.boot(CoreJSON.string(["type": "reset"]))
    }

    private func newSession() -> Session {
        let key = nextKey
        nextKey += 1
        let executor = self.executor
        let core = CoreStore<FeeViewWire>(
            bridge: FeePolicyCore(),
            perform: { operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(key: key, view) },
            onFault: { print("[vela-wallet] fee_policy fault: \($0)") }
        )
        return Session(key: key, core: core)
    }

    private func session(_ key: Int) -> Session? {
        if inForce.key == key { return inForce }
        return previews.first { $0.key == key }
    }

    private func commit(key: Int, _ view: FeeViewWire) {
        guard let session = session(key) else { return }
        session.view = view
        if session === inForce { inForceChanged() }
        settleSpeed()
    }

    /// The session in force moved: publish it, answer a caller waiting on it
    /// once it settles, and — when it settled as a failure — drop the held
    /// readings behind it, so a retry measures again (issue 212).
    private func inForceChanged() {
        view = inForce.view
        if let view { onInForce?(view) }
        guard let view, !view.busy else { return }
        if view.failed != nil, let ask = inForce.ask {
            relay.invalidateFeeSignals(chainId: ask.chainId)
        }
        guard requested else { return }
        requested = false
        let settled = waiting.filter { $0.generation == generation }
        waiting.removeAll()
        for entry in settled { entry.resume(view) }
    }

    // MARK: - The facade `send` awaits

    /// Ask for a quote and wait for it to settle.
    ///
    /// Answers the view as it stands once the session in force stops being
    /// busy — including a failed one, because a failure is an answer and `send`
    /// has a variant for it. A caller superseded by a newer request is resumed
    /// with `nil` rather than left hanging. HOW FAST is the speed core's to say
    /// (spec 068): the stored default, a one-shot pick, or a free upgrade.
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
        // A quote that never settles is a spinner nobody can explain, and the
        // `await` behind it holds `estimate_fee` open forever — which holds the
        // confirm gate shut forever. This is the deadline that makes a hung
        // quote a FAILURE the core can act on.
        Task { [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(Self.settleDeadlineMs) * 1_000_000)
            guard let self, self.generation == mine, self.requested else { return }
            print("[vela-wallet] fee_policy: quote did not settle in \(Self.settleDeadlineMs)ms")
            self.requested = false
            let waiting = self.waiting
            self.waiting.removeAll()
            for entry in waiting { entry.resume(nil) }
        }
        let ask = Ask(
            chainId: chainId, account: account, deployed: deployed,
            publicKeyAvailable: publicKeyAvailable, tier: speed?.tier ?? "fast",
            calls: calls, feeToken: feeToken
        )
        return await withCheckedContinuation { continuation in
            var resumed = false
            waiting.append((mine, { view in
                guard !resumed else { return }
                resumed = true
                continuation.resume(returning: view)
            }))
            askInForce(ask)
        }
    }

    /// Price the operation at the tier in force and let the fee row show it
    /// when it lands — for a surface nobody's machine is waiting on (the dApp
    /// signing sheet, spec 069). Same sessions, same speed rules as `quote`.
    func ask(
        chainId: Int,
        account: String,
        deployed: Bool,
        publicKeyAvailable: Bool,
        calls: [[String: Any]],
        feeToken: String?
    ) {
        askInForce(Ask(
            chainId: chainId, account: account, deployed: deployed,
            publicKeyAvailable: publicKeyAvailable, tier: speed?.tier ?? "fast",
            calls: calls, feeToken: feeToken
        ))
    }

    /// Price `ask` on the session in force. Recorded before the dispatch, so a
    /// preview of the previous operation can never be promoted over it.
    private func askInForce(_ ask: Ask) {
        askGeneration += 1
        inForce.ask = ask
        inForce.generation = askGeneration
        inForce.send(ask.event)
        settleSpeed()
    }

    /// A fee coin picked for the operation in force (`nil` = the native coin),
    /// by a surface nobody's machine is waiting on (the dApp sheet).
    ///
    /// The coin is part of the operation every preview replays, so the whole
    /// question is asked again with it and the other speeds follow in that
    /// coin. Telling the session in force alone would leave the previews
    /// pricing the old coin — and a speed tapped next would promote one,
    /// switching the payment back to a coin the person just walked away from.
    func chooseFeeToken(_ token: String?) {
        guard let ask = inForce.ask, ask.feeToken != token else { return }
        askInForce(Ask(
            chainId: ask.chainId, account: ask.account, deployed: ask.deployed,
            publicKeyAvailable: ask.publicKeyAvailable, tier: speed?.tier ?? ask.tier,
            calls: ask.calls, feeToken: token
        ))
    }

    /// A fee-asset chip tap. `nil` = the native coin.
    ///
    /// The token is a QUOTE PARAMETER, not a post-quote adjustment: the fee leg
    /// batched into the simulated operation is a native transfer for `nil` and
    /// an ERC-20 `transfer` for a contract. Re-denominating a native quote
    /// would price a different operation than the one that gets submitted.
    func selectAsset(_ token: String?) {
        inForce.send(CoreJSON.string([
            "type": "select_fee_asset",
            "token": token.map { $0 as Any } ?? NSNull(),
        ]))
    }

    /// The core re-runs the request it priced (a stale quote on the confirm).
    func requote() { inForce.send(CoreJSON.string(["type": "requote"])) }

    /// The refresh control (spec 068): measure AGAIN. The held readings go
    /// first (issue 212), so this is a new measurement rather than the number
    /// on screen, and the other tiers are re-priced with it.
    func refresh() {
        guard let ask = inForce.ask else { return }
        relay.invalidateFeeSignals(chainId: ask.chainId)
        askGeneration += 1
        inForce.generation = askGeneration
        requote()
        settleSpeed()
    }

    /// Leaving the confirm step.
    func leaveConfirm() { inForce.send(CoreJSON.string(["type": "leave_confirm"])) }

    /// The form now targets a different chain, so every earlier quote is
    /// invalid for it (the core's invariant ①).
    func chainChanged(_ chainId: Int) {
        inForce.send(CoreJSON.string(["type": "chain_changed", "chain_id": chainId]))
    }

    // MARK: - The speed control (spec 069)

    /// The stored default and the resolved number preset — repeat freely.
    func configureSpeed(preferred: String, number: String) {
        speedCore.dispatch(CoreJSON.string(["type": "configure", "preferred": preferred, "number": number]))
    }

    /// A send starts or ends: a new one starts at the stored default.
    func resetSpeed() { speedCore.dispatch(CoreJSON.string(["type": "reset"])) }

    /// A free upgrade is only decided while the person is still choosing.
    func speedStage(onForm: Bool) {
        speedCore.dispatch(CoreJSON.string(["type": "stage_changed", "on_form": onForm]))
    }

    /// Fold or unfold the control.
    func toggleSpeed() { speedCore.dispatch(CoreJSON.string(["type": "toggle"])) }

    /// A tap on an option — one-shot, never the stored preference.
    func pickSpeed(_ tier: String) {
        speedCore.dispatch(CoreJSON.string(["type": "pick", "tier": tier]))
    }

    /// The fee view of the session pricing `tier` — for formatting that
    /// option's fee with its own fee-coin options.
    func view(of tier: String) -> FeeViewWire? {
        if inForce.ask?.tier == tier { return inForce.view }
        return previews.first { $0.ask?.tier == tier }?.view
    }

    private var passing = false
    private var dirty = false

    private func speedChanged(_ view: FeeSpeedViewWire) {
        guard view != speed else { return }
        speed = view
        settleSpeed()
    }

    /// Report every session, then bring the sessions in line with what the
    /// speed core decided — until nothing moves. Re-entrant calls (a view that
    /// arrives inline) leave the rest of the work to the pass already running.
    private func settleSpeed() {
        if passing {
            dirty = true
            return
        }
        passing = true
        defer { passing = false }
        for _ in 0..<4 {
            dirty = false
            reportQuotes()
            applySpeed()
            if !dirty { break }
        }
    }

    private func reportQuotes() {
        let previews: [[String: Any]] = self.previews.compactMap { session in
            guard let tier = session.ask?.tier else { return nil }
            return [
                "tier": tier,
                "busy": session.view?.busy ?? true,
                "fee": session.view?.fee?.coreJSON ?? NSNull(),
            ]
        }
        speedCore.dispatch(CoreJSON.string([
            "type": "quotes_changed",
            "chain_id": inForce.ask.map { $0.chainId as Any } ?? NSNull(),
            "in_force": [
                "busy": inForce.view?.busy ?? (inForce.ask != nil),
                "fee": inForce.view?.fee?.coreJSON ?? NSNull(),
            ] as [String: Any],
            "previews": previews,
        ]))
    }

    /// Rule 1, then rule 2 (`fee_speed.rs`).
    private func applySpeed() {
        guard let speed else { return }
        if let ask = inForce.ask, ask.tier != speed.tier {
            if promote(speed.tier) {
                dirty = true
            } else if !(inForce.view?.busy ?? true), !requested {
                // Never over a measurement that is out — `send` may be waiting
                // on it. What is left is a tier with nothing priced for it.
                askInForce(ask.at(speed.tier))
            }
        }
        syncPreviews(speed.previews)
    }

    /// THE PRICE YOU TAP IS THE PRICE YOU GET (issue 681): the preview that
    /// already priced THIS operation at `tier` becomes the session in force —
    /// no second question to the relay — and the one it replaces stays on as
    /// its own tier's preview. Refused when that preview has no settled quote
    /// of its own, or priced another operation than the one last asked.
    private func promote(_ tier: String) -> Bool {
        guard let mine = inForce.ask,
              let index = previews.firstIndex(where: { session in
                  guard let ask = session.ask, ask.tier == tier, ask.sameOperation(mine),
                        let view = session.view, !view.busy, view.fee?.tier == tier
                  else { return false }
                  return true
              })
        else { return false }
        let promoted = previews.remove(at: index)
        let demoted = inForce!
        inForce = promoted
        if demoted.ask != nil { previews.append(demoted) }
        inForceChanged()
        return true
    }

    /// Rule 2: a preview for every tier the speed core names, pricing the same
    /// operation as the session in force at the same generation; every other
    /// preview dropped.
    private func syncPreviews(_ wanted: [String]) {
        guard let base = inForce.ask else {
            previews.removeAll()
            return
        }
        previews.removeAll { session in
            guard let ask = session.ask else { return true }
            return !(wanted.contains(ask.tier) && ask.sameOperation(base) && session.generation == askGeneration)
        }
        for tier in wanted where !previews.contains(where: { $0.ask?.tier == tier }) {
            let session = newSession()
            let ask = base.at(tier)
            session.ask = ask
            session.generation = askGeneration
            previews.append(session)
            session.send(ask.event)
        }
    }
}
