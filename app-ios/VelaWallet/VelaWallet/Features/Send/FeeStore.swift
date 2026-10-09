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
    /// The same view as the core wrote it, for `signConfirmState` (spec 099
    /// R7); `nil` while `view` is.
    private(set) var viewJson: String?
    /// Told every view of the session in force — for an owner that reacts to
    /// one (the signing sheet asks a failed quote again on the core's
    /// schedule). Promotions included.
    @ObservationIgnored var onInForce: ((FeeViewWire) -> Void)?
    /// The speed control, as the `fee_speed` core decided it (spec 069).
    private(set) var speed: FeeSpeedViewWire?
    /// The same view as the core wrote it, for `handoffFeeRow` (spec 102);
    /// `nil` while `speed` is.
    private(set) var speedJson: String?
    /// The resolved number preset `configureSpeed` was last given: every fee
    /// request carries it, and the core writes a coin's shortfall in it
    /// (issue #408).
    @ObservationIgnored private var number = "comma_dot"

    /// What a session was asked to price; compared minus the tier.
    private struct Ask {
        let chainId: Int
        let account: String
        let deployed: Bool
        let publicKeyAvailable: Bool
        let tier: String
        let calls: [[String: Any]]
        let feeToken: String?
        /// Nobody has chosen the fee coin (spec 078): the fee machine pays in
        /// one that can, and `feeToken` is only where it falls back to. Part
        /// of the operation, so every preview replays it.
        let autoFeeToken: Bool

        /// The operation, tier aside — `calls` compared as the JSON the core
        /// reads, which is exact.
        func sameOperation(_ other: Ask) -> Bool {
            chainId == other.chainId && account == other.account && deployed == other.deployed
                && publicKeyAvailable == other.publicKeyAvailable && feeToken == other.feeToken
                && autoFeeToken == other.autoFeeToken
                && CoreJSON.string(["c": calls]) == CoreJSON.string(["c": other.calls])
        }

        func at(_ tier: String) -> Ask {
            Ask(chainId: chainId, account: account, deployed: deployed,
                publicKeyAvailable: publicKeyAvailable, tier: tier, calls: calls,
                feeToken: feeToken, autoFeeToken: autoFeeToken)
        }

        /// The same operation in the coin the person tapped: from here on it
        /// is theirs and is priced exactly as asked.
        func paying(_ token: String?) -> Ask {
            Ask(chainId: chainId, account: account, deployed: deployed,
                publicKeyAvailable: publicKeyAvailable, tier: tier, calls: calls,
                feeToken: token, autoFeeToken: false)
        }

        /// `number`: the resolved preset the core writes a coin's shortfall
        /// in (issue #408) — the one `configureSpeed` was last given.
        func event(number: String) -> String {
            CoreJSON.string([
                "type": "quote_requested",
                "chain_id": chainId,
                "account": account,
                "deployed": deployed,
                "public_key_available": publicKeyAvailable,
                "tier": tier,
                "calls": calls,
                "fee_token": feeToken.map { $0 as Any } ?? NSNull(),
                "auto_fee_token": autoFeeToken,
                "number": number,
            ])
        }
    }

    /// How the timers the fee machine asks for run out: `start_ttl` (the
    /// quote on screen priced again a block later, `requote_interval_ms`) and
    /// `start_deadline` (the bound on a whole run, spec 094 S9 — 15 s).
    enum Timers {
        /// The app's: each runs out on the wall clock, after its `ms`.
        case wallClock
        /// A test's, under a scripted relay: none runs out by itself — no time
        /// passes until the test moves the clock (`elapse`). A starved CI
        /// runner took longer than 15 s to hand a scripted quote its answers,
        /// and the deadline failed it (`chain_read`, `quote_unavailable`): a
        /// failure the code never had, measured on a clock (`Waits.swift`). A
        /// timer held here is not in flight for `isIdle` — nothing but time
        /// could answer it.
        case stopped
    }

    /// A session's timers under `.stopped`: held until the machine abandons
    /// them or a test runs them out (`elapse`), and counted so `isIdle` tells
    /// them from a read.
    @MainActor
    private final class HeldTimers {
        private var nextId = 0
        /// The timers held now, by the operation each stands for.
        private var held: [Int: (timer: String, wake: CheckedContinuation<Void, Never>)] = [:]
        /// How many are held. One run out stops counting at once, before its
        /// answer is in, so `isIdle` waits for whatever that answer sets going.
        var count: Int { held.count }

        /// Until the machine abandons `timer` (the effect's task is cancelled)
        /// or the test runs it out.
        func hold(_ timer: String) async {
            let id = nextId
            nextId += 1
            await withTaskCancellationHandler {
                await withCheckedContinuation { wake in
                    if Task.isCancelled { wake.resume() } else { held[id] = (timer, wake) }
                }
            } onCancel: {
                Task { @MainActor [weak self] in self?.held.removeValue(forKey: id)?.wake.resume() }
            }
        }

        /// Every held `timer` runs out now.
        func elapse(_ timer: String) {
            for (id, entry) in held where entry.timer == timer {
                held.removeValue(forKey: id)
                entry.wake.resume()
            }
        }

        nonisolated deinit {}
    }

    /// One `fee_policy` session: in force, or a preview of another tier.
    @MainActor
    private final class Session {
        let key: Int
        let core: CoreStore<FeeViewWire>
        let held: HeldTimers
        var view: FeeViewWire?
        var ask: Ask?
        var generation = 0

        init(key: Int, core: CoreStore<FeeViewWire>, held: HeldTimers) {
            self.key = key
            self.core = core
            self.held = held
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
    /// Spec 083 fee, issue #411: what the operation moves, per asset, as the
    /// signing sheet's own simulation measured it — with the calls it
    /// measured (as the JSON the core reads), so it is told only to a session
    /// pricing those very calls. The fee machine forgets it on every
    /// `quote_requested`, so each session is told again right after each
    /// question (the desktop's `speed_control::balance_changes`).
    private var measured: (calls: String, changes: [[String: Any]])?

    /// No session has an effect in flight — the one in force, every speed
    /// preview, the speed core (`CoreDriver.isIdle`) — but timers a stopped
    /// clock holds. What a test waits on instead of a clock.
    var isIdle: Bool {
        ([inForce!] + previews).allSatisfy { $0.core.inFlight == $0.held.count } && speedCore.isIdle
    }

    /// A test's clock moving, under `.stopped`: every `timer` the stopped
    /// clock holds (`start_ttl`, `start_deadline`) runs out now, in every
    /// session, and the machine hears it as it would from the wall clock.
    func elapse(_ timer: String) {
        for session in [inForce!] + previews { session.held.elapse(timer) }
    }

    /// Who is waiting for the quote in flight, and for which attempt.
    private var waiting: [(generation: Int, resume: (FeeViewWire?) -> Void)] = []
    private var generation = 0
    /// A caller is waiting on the session in force to settle.
    private var requested = false
    /// How long a quote may take before the shell calls it a failure. Generous:
    /// the core's own TTL is 30 s and a cold pool sweeps three passes.
    nonisolated static let settleDeadline: Duration = .seconds(45)
    /// The deadline in force. `nil` — a test's, whose relay is scripted and
    /// answers or does not by design — sets no clock under the quote: on a
    /// starved CI runner a scripted quote could otherwise outlast 45 s and be
    /// reported as a failure the code never had (`Waits.swift`). The core's
    /// own timers are the other clock under it (`timers`).
    private let deadline: Duration?
    private let timers: Timers

    init(
        relay: RelayClient,
        accounts: UserOpSpine.AccountPort,
        measureCall: @escaping FeeExecutor.MeasureCall = { _, _, _, _, _ in nil },
        settleDeadline: Duration? = FeeStore.settleDeadline,
        timers: Timers = .wallClock
    ) {
        self.executor = FeeExecutor(relay: relay, accounts: accounts, measureCall: measureCall)
        self.relay = relay
        self.deadline = settleDeadline
        self.timers = timers
        self.inForce = newSession()
        self.speedCore = CoreStore(
            bridge: FeeSpeedCore(),
            // The speed core asks the shell for nothing.
            perform: { operation in
                print("[vela-wallet] fee_speed asked for an operation: \(operation)")
                return "{}"
            },
            onView: { [weak self] view in self?.speedChanged(view) },
            onFault: { print("[vela-wallet] fee_speed fault: \($0)") },
            // The hand-off card's fee row reads this view whole (spec 102).
            keepsJson: true
        )
        _ = speedCore.boot(CoreJSON.string(["type": "reset"]))
    }

    private func newSession() -> Session {
        let key = nextKey
        nextKey += 1
        let executor = self.executor
        let held = HeldTimers()
        let stopped = timers == .stopped
        let core = CoreStore<FeeViewWire>(
            bridge: FeePolicyCore(),
            perform: { operation in
                if stopped, let timer = operation["type"] as? String, FeeExecutor.timers.contains(timer) {
                    // Abandoned, the answer is dropped (`CoreDriver`); run out
                    // by the test, it is the timer's own: `ttl_elapsed`,
                    // `deadline_elapsed`.
                    await held.hold(timer)
                    return FeeExecutor.neutralAnswer(operation)
                }
                return await executor.perform(operation)
            },
            onView: { [weak self] view in self?.commit(key: key, view) },
            onFault: { print("[vela-wallet] fee_policy fault: \($0)") },
            keepsJson: true
        )
        return Session(key: key, core: core, held: held)
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
        viewJson = inForce.view == nil ? nil : inForce.core.json
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

    /// The chain the session in force is pricing — its last question's. What
    /// `view.feeToken` is a contract on; `nil` before any question.
    var pricingChainId: Int? { inForce.ask?.chainId }

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
        feeToken: String?,
        autoFeeToken: Bool = false
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
        if let deadline { Task { [weak self] in
            try? await Task.sleep(for: deadline)
            guard let self, self.generation == mine, self.requested else { return }
            print("[vela-wallet] fee_policy: quote did not settle in \(deadline)")
            self.requested = false
            let waiting = self.waiting
            self.waiting.removeAll()
            for entry in waiting { entry.resume(nil) }
        } }
        let ask = Ask(
            chainId: chainId, account: account, deployed: deployed,
            publicKeyAvailable: publicKeyAvailable, tier: speed?.tier ?? "standard",
            calls: calls, feeToken: feeToken, autoFeeToken: autoFeeToken
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
        feeToken: String?,
        autoFeeToken: Bool = false
    ) {
        askInForce(Ask(
            chainId: chainId, account: account, deployed: deployed,
            publicKeyAvailable: publicKeyAvailable, tier: speed?.tier ?? "standard",
            calls: calls, feeToken: feeToken, autoFeeToken: autoFeeToken
        ))
    }

    /// Price `ask` on the session in force. Recorded before the dispatch, so a
    /// preview of the previous operation can never be promoted over it.
    private func askInForce(_ ask: Ask) {
        askGeneration += 1
        inForce.ask = ask
        inForce.generation = askGeneration
        inForce.send(ask.event(number: number))
        tellMeasured(inForce)
        settleSpeed()
    }

    // MARK: - What the operation moves (spec 083 fee, issue #411)

    /// The signing sheet's simulation of the operation answered: what it
    /// moves, per asset — the coins' own `Transfer` logs and the node's trace
    /// of native value (`SimDeltas.feeBalanceChanges`). Which fee coin can pay
    /// is what the operation LEAVES of it; a coin the calls only name (a
    /// router's swap path) is one the machine may pay in once this says
    /// enough of it is left. A Uniswap swap on Polygon whose path named both
    /// stablecoins was priced in POL, held at 0, because nobody said so.
    ///
    /// Kept for every question about `calls`, and told now to each session
    /// already asked one — the session in force and every speed preview
    /// pricing those very calls. Never call this for a simulation that could
    /// not check or that reverted: that is no measurement.
    func balanceChanges(calls: [[String: Any]], changes: [[String: Any]]) {
        // Calls that cannot be written down cannot be matched: no measurement.
        measured = Self.callsKey(calls).map { ($0, changes) }
        let told = ([inForce!] + previews).filter { tellMeasured($0) }.count
        VelaLog.notice(.fee, "balance changes measured changes=\(changes.count) told=\(told)")
    }

    /// Tell `session` what its operation moves, when it prices the calls that
    /// were measured — straight after its `quote_requested`, which clears it
    /// in the machine.
    @discardableResult
    private func tellMeasured(_ session: Session) -> Bool {
        guard let measured, let ask = session.ask, let calls = Self.callsKey(ask.calls),
              calls == measured.calls
        else { return false }
        session.send(CoreJSON.string([
            "type": "balance_changes_measured",
            "changes": measured.changes,
        ]))
        return true
    }

    /// The calls as one comparable text — keys sorted, so two dictionaries
    /// holding the same call always read the same (`CoreJSON.string` keeps
    /// whatever order the dictionary enumerates in).
    private static func callsKey(_ calls: [[String: Any]]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: calls, options: [.sortedKeys])
        else { return nil }
        return String(data: data, encoding: .utf8)
    }

    /// A fee coin picked for the operation in force (`nil` = the native coin),
    /// by a surface nobody's machine is waiting on (the dApp sheet).
    ///
    /// The coin is part of the operation every preview replays, so the whole
    /// question is asked again with it and the other speeds follow in that
    /// coin. Telling the session in force alone would leave the previews
    /// pricing the old coin — and a speed tapped next would promote one,
    /// switching the payment back to a coin the person just walked away from.
    ///
    /// A tap is the person's choice (spec 078): asked with `auto_fee_token`
    /// off, so the coin tapped is the coin priced — even when it is the one
    /// the machine had picked for them, which is why an unchanged coin still
    /// re-asks while the pick was automatic.
    func chooseFeeToken(_ token: String?) {
        guard let ask = inForce.ask, ask.feeToken != token || ask.autoFeeToken else { return }
        askInForce(ask.paying(token).at(speed?.tier ?? ask.tier))
    }

    /// A fee-asset chip tap. `nil` = the native coin.
    ///
    /// The token is a QUOTE PARAMETER, not a post-quote adjustment: the fee leg
    /// batched into the simulated operation is a native transfer for `nil` and
    /// an ERC-20 `transfer` for a contract. Re-denominating a native quote
    /// would price a different operation than the one that gets submitted.
    ///
    /// The session in force recomputes locally (`select_fee_asset`, which
    /// also ends the machine's own pick). The operation on record follows
    /// the tap too, so the previews re-price in the coin chosen and a speed
    /// re-asked later carries it with `auto_fee_token` off — replaying the
    /// old automatic request would hand the choice back to the machine.
    func selectAsset(_ token: String?) {
        inForce.send(CoreJSON.string([
            "type": "select_fee_asset",
            "token": token.map { $0 as Any } ?? NSNull(),
        ]))
        guard let ask = inForce.ask, ask.feeToken != token || ask.autoFeeToken else { return }
        askGeneration += 1
        inForce.ask = ask.paying(token)
        inForce.generation = askGeneration
        settleSpeed()
    }

    /// The core re-runs the request it priced — `refresh`'s, once the held
    /// readings are gone. Nothing else asks: a figure on screen is priced
    /// again by the core itself once a block (`requote_interval_ms`), and
    /// `stale` is only ever up while it does.
    private func requote() { inForce.send(CoreJSON.string(["type": "requote"])) }

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

    /// Ask the operation in force again FROM THE START (spec 082 RJ12): an
    /// automatic re-quote that did not answer in time. `requote` is ignored
    /// while a run is out; a fresh request makes the core abandon whatever the
    /// hung run was still waiting on, and measure again.
    func reask() {
        guard let ask = inForce.ask else { return }
        relay.invalidateFeeSignals(chainId: ask.chainId)
        askInForce(ask)
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
        self.number = number
        speedCore.dispatch(CoreJSON.string(["type": "configure", "preferred": preferred, "number": number]))
    }

    /// A send starts or ends: a new one starts at the stored default, and
    /// what the last operation moved is no measurement of the next.
    func resetSpeed() {
        measured = nil
        speedCore.dispatch(CoreJSON.string(["type": "reset"]))
    }

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
        speedJson = speedCore.json
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
            session.send(ask.event(number: number))
            tellMeasured(session)
        }
    }
}
