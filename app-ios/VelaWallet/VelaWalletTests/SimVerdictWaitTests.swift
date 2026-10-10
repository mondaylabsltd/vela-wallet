//
//  SimVerdictWaitTests.swift
//  VelaWalletTests
//
//  The confirm waits for the simulation's verdict (PR 3, fix C).
//
//  The confirm's gate looked at the request, the reading, the approval guard
//  and the fee — not at the simulation. A person could confirm before the
//  balance changes were on the sheet, and those are the one part of it the
//  site being signed for cannot write.
//
//  The wait, its line and its deadline are the core's. What this shell owes
//  it is tested here, on its own wiring: `SigningController` says when the
//  simulation of the request on the sheet is SENT and when what stands in
//  the verdict's place is no longer "Checking…"; `SignExecutor` runs the
//  core's one timer; `TokenTrustStore` says when a checked answer is judged;
//  and the sheet draws the gate's line under a held confirm, and the
//  could-not-check caution in the verdict's place past the deadline, without
//  the confirm moving.
//
//  Through the real `sign_request`, `clear_signing`, `approval_guard`,
//  `fee_policy`, `sim_outcome` and `token_trust` cores, a scripted relay, a
//  simulation the test answers when it chooses, and a stopped clock: no
//  time passes here but the time a test moves.
//

import Foundation
import Observation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SimVerdictWaitTests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let recipient = "0x2222222222222222222222222222222222222222"
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let site = "https://app.example"
    private let transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    private static let checkingKey = "componentsUi.signing.confirmBlock.simChecking"
    private static let couldNotCheckKey = "componentsUi.signing.simUnavailableWarning"

    // MARK: - The rig

    /// A simulation the test answers when it chooses — or at once.
    @MainActor
    final class Simulation {
        /// Answered with this the moment it is asked; `nil` waits for `answer`.
        var immediate: RpcOutcome?
        private(set) var asked = 0
        private var waiting: [CheckedContinuation<RpcOutcome, Never>] = []

        func run() async -> RpcOutcome {
            asked += 1
            if let immediate { return immediate }
            return await withCheckedContinuation { waiting.append($0) }
        }

        func answer(_ outcome: RpcOutcome) {
            let held = waiting
            waiting = []
            for resume in held { resume.resume(returning: outcome) }
        }
    }

    private struct Rig {
        let controller: SigningController
        let simulation: Simulation
        let signer: CountingSigner
        let trust: TokenTrustStore
    }

    /// A deployed Safe on Polygon holding 100 POL, quoted 2.94975 POL: the
    /// fee settles and its own coin pays, so only the verdict can be missing.
    private func scriptedRelay() -> RelayClient {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        port.rpc["eth_gasPrice"] = .ok("0x1fcbc261a00")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x0"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x0")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x3f9784c3400", "networkFeePerGas": "0x1fcbc261a00",
            "relayerFeePerGas": "0x1fcbc261a00",
        ]
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "fast": tier, "standard": tier, "slow": tier,
        ] as [String: Any])
        port.rpc["vela_getInBandGasQuote"] = .ok([
            ["recipient": recipient, "asset": "native", "feeToken": NSNull(),
             "balance": "0x56bc75e2d63100000", "decimals": 18, "symbol": "POL",
             "usdBalance": "10.75", "usdPrice": "0.1075"] as [String: Any],
        ])
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        port.rest["/v1/account/137/\(safe.lowercased())"] = .ok([
            "activeDepositAddress": recipient, "status": "ACTIVE",
        ])
        return RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
    }

    /// The controller as the app builds it, but for the chain: the relay and
    /// the simulation are scripted, the clock is stopped, and the judge is
    /// the real `TokenTrustStore` over a pool that reaches nothing.
    private func rig() -> Rig {
        let defaults = UserDefaults(suiteName: "vela.tests.sim-wait.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let appAccounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: appAccounts, offline: true)
        let trust = TokenTrustStore(store: store, pool: pool, accounts: appAccounts, held: HeldTokens())
        let relay = scriptedRelay()
        let accounts = ScriptedAccounts()
        let signer = CountingSigner()
        let simulation = Simulation()
        let controller = SigningController(
            wallet: (address: safe, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { signer }),
            store: store, pool: pool,
            ports: SigningController.Ports(
                nativeSymbol: { _ in "POL" },
                knownChains: { [137] },
                simJudged: { address, chainId, deltas in
                    await trust.simJudged(address: address, chainId: chainId, deltas: deltas)
                },
                simulate: { _, _ in await simulation.run() }
            ),
            timers: .stopped
        )
        return Rig(controller: controller, simulation: simulation, signer: signer, trust: trust)
    }

    /// 1.5 POL to somebody: a plain send, which the core reads at once.
    private var transaction: SigningController.Incoming {
        SigningController.Incoming(
            id: "tx-1", method: "eth_sendTransaction",
            paramsJson: #"[{"from":"\#(safe)","to":"\#(payee)","value":"0x14d1120d7b160000"}]"#,
            origin: site, transportId: "tab-1", chainId: 137, grantedAddress: safe
        )
    }

    private var message: SigningController.Incoming {
        SigningController.Incoming(
            id: "msg-1", method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(safe)"]"#,
            origin: site, transportId: "tab-1", chainId: 137, grantedAddress: safe
        )
    }

    private func bare(_ address: String) -> String { String(address.dropFirst(2)).lowercased() }

    /// The node's checked answer: 1.5 POL leaves the wallet (the chain's own
    /// coin, as `traceTransfers` logs it) — and, `unknownToken`, a token
    /// nobody vouches for arrives, which the judge has to ask about first.
    private func checked(unknownToken: Bool = false) -> RpcOutcome {
        func log(_ address: String, from: String, to: String, hex: String) -> [String: Any] {
            [
                "address": address,
                "topics": [transferTopic, "0x" + String(repeating: "0", count: 24) + bare(from),
                           "0x" + String(repeating: "0", count: 24) + bare(to)],
                "data": "0x" + String(repeating: "0", count: 64 - hex.count) + hex,
            ]
        }
        var logs = [log("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", from: safe, to: payee, hex: "14d1120d7b160000")]
        if unknownToken {
            logs.append(log(TrustCoreScene.unknown, from: payee, to: safe, hex: "10f0cf064dd59200000"))
        }
        return .ok([["calls": [["status": "0x1", "returnData": "0x", "logs": logs] as [String: Any]]] as [String: Any]])
    }

    /// The sheet's model for `live`, as `RootView.signingModel` builds it.
    private func drawn(_ live: SigningController, loc: Loc? = nil) throws -> SigningModel {
        let loc = loc ?? en
        var context = SigningLive.Context(
            loc: loc, chainName: "Polygon", chainDot: .purple, nativeSymbol: "POL",
            walletName: "Me", walletAddress: safe
        )
        context.chainId = 137
        context.origin = live.request?.origin
        context.sim = live.simVerdict
        context.simulation = live.simulation
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: try #require(live.request),
            sign: live.shownSign, clear: live.clear, guard: live.guardView, fee: live.fee, context: context,
            gate: live.confirmState
        )
    }

    /// What stands in the verdict's place on `model`.
    private func place(_ model: SigningModel) throws -> [SigningBlock] {
        try #require(model.formItems.compactMap { item -> [SigningBlock]? in
            if case .verdict(let inner) = item { return inner }
            return nil
        }.first, "the sheet keeps no place for the verdict")
    }

    private func isChecking(_ blocks: [SigningBlock], loc: Loc) -> Bool {
        guard blocks.count == 1, case .balances(_, let rows, let note, _)? = blocks.first else { return false }
        return rows.isEmpty && note == loc.t("componentsUi.funding.checking")
    }

    /// The gate has come to its last word: everything but the verdict is in.
    private func untilOnlyTheVerdictIsMissing(_ controller: SigningController) async {
        await Wait.until { controller.confirmState.block == "sim_checking" }
    }

    // MARK: - (a) Held until the verdict is on the sheet

    /// A transaction with its reading, guard and fee ready and its
    /// simulation out: the confirm is DISABLED and the line under it is the
    /// core's "Checking what this transaction does…"; the verdict's place
    /// says "Checking…"; a tap is not taken. When the node answers and the
    /// judged card is drawn, the confirm is ENABLED and the line is gone.
    @Test func aTransactionsConfirmWaitsForTheVerdictAndOpensWhenItIsDrawn() async throws {
        let rig = rig()
        let controller = rig.controller
        controller.open(transaction)
        #expect(controller.sign.checkingSim, "told in the step that opened the request, before any frame")
        await untilOnlyTheVerdictIsMissing(controller)

        let gate = controller.confirmState
        #expect(gate == SignConfirmStateWire(enabled: false, block: "sim_checking", key: Self.checkingKey))
        #expect(rig.simulation.asked == 1, "one simulation, sent")
        #expect(controller.heldSimVerdictTimers == 1, "the core's one deadline, held by the stopped clock")
        #expect(controller.simulation == .pending && controller.simVerdict == nil)
        #expect(controller.fee?.confirmFeeReady == true, "the fee is not what holds it")

        let held = try drawn(controller)
        #expect(held.confirm?.enabled == false)
        #expect(held.confirmBlockLine == "Checking what this transaction does…")
        let heldPlace = try place(held)
        #expect(isChecking(heldPlace, loc: en), "\(heldPlace.map(\.id))")

        // A tap from a stale frame is not taken: nothing is signed.
        controller.approve()
        try await Task.sleep(for: .milliseconds(100))
        #expect(!controller.sign.isSigning && !controller.sign.isSubmitting && controller.sign.phase == .idle)
        #expect(rig.signer.calls == 0)

        // The node answers; the judge needs no name for the chain's coin.
        rig.simulation.answer(checked())
        await Wait.until { controller.confirmState.enabled }
        #expect(controller.confirmState == SignConfirmStateWire(enabled: true, block: nil, key: nil))
        #expect(!controller.sign.checkingSim && controller.sign.simWaitedOutKey == nil)
        #expect(controller.simulation == .answered)
        let verdict = try #require(controller.simVerdict, "the settle and the verdict land together")
        #expect(verdict.ready && verdict.judgments == [.native(delta: "-1500000000000000000")])

        let landed = try drawn(controller)
        #expect(landed.confirm?.enabled == true)
        #expect(landed.confirmBlockLine == nil, "the line is gone")
        let landedPlace = try place(landed)
        guard case .balances(_, let rows, _, _)? = landedPlace.first else {
            Issue.record("no balance card in the verdict's place: \(landedPlace.map(\.id))")
            return
        }
        #expect(rows.map(\.symbol) == ["POL"] && rows.first?.delta == "\u{2212}1.5")

        // The deadline of a wait that is over opens and says nothing.
        controller.elapseSimVerdictTimer()
        try await Task.sleep(for: .milliseconds(100))
        #expect(controller.confirmState.enabled && controller.sign.simWaitedOutKey == nil)
        #expect(controller.heldSimVerdictTimers == 0)
        controller.swipeDismissed()
    }

    /// "The verdict is on the sheet" is when its tokens are JUDGED, not when
    /// the node replied: a checked answer that moves a token nobody has
    /// named keeps the confirm held while the judge asks, and the settle
    /// arrives with the judged card — never before it.
    @Test func aCheckedAnswerSettlesWhenItIsJudgedNotWhenTheNodeReplied() async throws {
        let rig = rig()
        let controller = rig.controller
        let frames = Frames(controller)
        controller.open(transaction)
        await untilOnlyTheVerdictIsMissing(controller)

        rig.simulation.answer(checked(unknownToken: true))
        await Wait.until { controller.confirmState.enabled }
        let verdict = try #require(controller.simVerdict)
        #expect(verdict.judgments.count == 2)
        #expect(verdict.judgments.last == .erc20Unverified(token: TrustCoreScene.unknown, direction: .in))

        // The node's reply came first, in a frame of its own: still held,
        // still "Checking…" — and no frame ever had one without the other.
        frames.sample()
        #expect(frames.samples.contains { $0.simulation == .answered && !$0.judged && $0.checking },
                "the reply alone did not open the confirm: \(frames.samples)")
        for frame in frames.samples {
            #expect(frame.judged == !frame.checking, "the card and the settle are one step: \(frame)")
            if frame.judged { #expect(frame.block != "sim_checking", "\(frame)") }
        }
        controller.swipeDismissed()
    }

    // MARK: - (b) The deadline

    /// The simulation never answers: the core's timer firing — and nothing
    /// else — opens the confirm, and the verdict's place says the
    /// could-not-check sentence as a caution, the very block a node that
    /// cannot simulate draws. A verdict that still lands replaces it.
    @Test func aVerdictThatNeverComesIsWaitedOutByTheCoresTimerAndALateOneReplacesIt() async throws {
        let rig = rig()
        let controller = rig.controller
        controller.open(transaction)
        await untilOnlyTheVerdictIsMissing(controller)

        // No clock of this shell's opens it: with the core's timer held, the
        // confirm stays shut.
        try await Task.sleep(for: .milliseconds(300))
        #expect(controller.confirmState.block == "sim_checking")
        #expect(controller.sign.simWaitedOutKey == nil)

        controller.elapseSimVerdictTimer()
        await Wait.until { controller.confirmState.enabled }
        #expect(controller.confirmState == SignConfirmStateWire(enabled: true, block: nil, key: nil))
        #expect(controller.sign.simWaitedOutKey == Self.couldNotCheckKey)
        #expect(!controller.sign.checkingSim)
        #expect(controller.simulation == .pending, "nothing answered: the shell's own state did not move")

        let waitedOut = try drawn(controller)
        #expect(waitedOut.confirm?.enabled == true && waitedOut.confirmBlockLine == nil)
        let caution = try place(waitedOut)
        guard caution.count == 1, case .warning(let tone, let text)? = caution.first else {
            Issue.record("the verdict's place does not hold one caution: \(caution.map(\.id))")
            return
        }
        #expect(tone == .caution)
        #expect(text == en.t(Self.couldNotCheckKey))
        // Exactly as the could-not-check notice is drawn today.
        var noticed = SigningLive.Context(
            loc: en, chainName: "Polygon", chainDot: .purple, nativeSymbol: "POL", walletName: "Me", walletAddress: safe
        )
        noticed.simulation = .notice(risk: "caution", key: Self.couldNotCheckKey, reason: nil)
        #expect(caution.map(\.id) == SigningLive.balanceBlocks(isTransaction: true, context: noticed).map(\.id))

        // The verdict lands late: it takes the line's place, and the confirm
        // is not shut again.
        rig.simulation.answer(checked())
        await Wait.until { controller.sign.simWaitedOutKey == nil }
        #expect(controller.confirmState.enabled)
        #expect(controller.simVerdict?.ready == true)
        let late = try drawn(controller)
        let latePlace = try place(late)
        guard case .balances(_, let rows, _, _)? = latePlace.first else {
            Issue.record("the late verdict did not replace the caution: \(latePlace.map(\.id))")
            return
        }
        #expect(rows.map(\.symbol) == ["POL"])
        #expect(late.confirm?.enabled == true && late.confirmBlockLine == nil)
        controller.swipeDismissed()
    }

    /// A late "expected to fail" replaces the waited-out caution too — with
    /// the core's own danger sentence, in the same step the key clears.
    @Test func aLateNoticeReplacesTheWaitedOutCaution() async throws {
        let rig = rig()
        let controller = rig.controller
        controller.open(transaction)
        await untilOnlyTheVerdictIsMissing(controller)
        controller.elapseSimVerdictTimer()
        await Wait.until { controller.sign.simWaitedOutKey != nil }

        rig.simulation.answer(.rpcError(code: 3, message: "execution reverted: ERC20: transfer amount exceeds balance"))
        await Wait.until { controller.sign.simWaitedOutKey == nil }
        guard case .notice(let risk, _, _) = controller.simulation else {
            Issue.record("a revert is a notice: \(controller.simulation)")
            return
        }
        let afterNotice = try drawn(controller)
        let blocks = try place(afterNotice)
        guard blocks.count == 1, case .warning(let tone, _)? = blocks.first else {
            Issue.record("\(blocks.map(\.id))")
            return
        }
        #expect(tone == (risk == "danger" ? .danger : .caution))
        #expect(controller.confirmState.enabled, "a verdict of \"expected to fail\" does not shut the confirm")
        controller.swipeDismissed()
    }

    // MARK: - (c) A message is never held

    /// A message starts no simulation, tells the core of none, starts no
    /// timer, and its confirm is never held for one.
    @Test func aMessageIsNeverHeldAndStartsNoTimer() async throws {
        let rig = rig()
        let controller = rig.controller
        var blocks: Set<String> = []
        controller.open(message)
        #expect(!controller.sign.checkingSim)
        await Wait.until {
            blocks.insert(controller.confirmState.block ?? "open")
            return controller.confirmState.enabled
        }
        #expect(controller.confirmState.enabled)
        #expect(!blocks.contains("sim_checking"), "\(blocks)")
        #expect(rig.simulation.asked == 0, "nothing is simulated for a signature")
        #expect(controller.heldSimVerdictTimers == 0, "and no deadline is asked for")
        #expect(!controller.sign.checkingSim && controller.sign.simWaitedOutKey == nil)
        let model = try drawn(controller)
        #expect(model.verdictPlace == nil && model.confirmBlockLine == nil)
        controller.swipeDismissed()
    }

    // MARK: - (d) "Not offered", at once

    /// A node that answers "not offered" at once: the could-not-check notice
    /// and the settle land in ONE step, so no frame that draws the notice is
    /// a held frame — and the deadline the core had started changes nothing
    /// when it later passes.
    @Test func aSimulationNotOfferedAtOnceNeverShowsAHeldFrameAfterIt() async throws {
        let rig = rig()
        let controller = rig.controller
        rig.simulation.immediate = .rpcError(code: -32601, message: "the method eth_simulateV1 does not exist")
        let frames = Frames(controller)
        controller.open(transaction)
        await Wait.until { controller.confirmState.enabled }
        frames.sample()

        guard case .notice(let risk, let key, _) = controller.simulation else {
            Issue.record("\"not offered\" is a notice: \(controller.simulation)")
            return
        }
        #expect(risk == "caution" && key == Self.couldNotCheckKey)
        #expect(!controller.sign.checkingSim && controller.sign.simWaitedOutKey == nil)
        let model = try drawn(controller)
        #expect(model.confirm?.enabled == true && model.confirmBlockLine == nil)
        let noticePlace = try place(model)
        guard noticePlace.count == 1, case .warning(let tone, let text)? = noticePlace.first else {
            Issue.record("\(noticePlace.map(\.id))")
            return
        }
        #expect(tone == .caution && text == en.t(Self.couldNotCheckKey))

        let noticed = frames.samples.filter { $0.simulation != .pending }
        #expect(!noticed.isEmpty)
        for frame in noticed {
            #expect(!frame.checking && frame.block != "sim_checking" && frame.line != Self.checkingKey,
                    "a frame drew the notice under a held confirm: \(frame)")
        }

        // The stale deadline: answered, dropped.
        #expect(controller.heldSimVerdictTimers == 1)
        controller.elapseSimVerdictTimer()
        try await Task.sleep(for: .milliseconds(100))
        #expect(controller.confirmState.enabled && controller.sign.simWaitedOutKey == nil)
        controller.swipeDismissed()
    }

    /// One frame's worth of the controller, sampled the turn after anything
    /// the sheet reads has changed — when SwiftUI would draw it.
    @MainActor
    final class Frames {
        struct Frame: CustomStringConvertible {
            let simulation: SigningController.Simulation
            /// The judged verdict is in.
            let judged: Bool
            /// `SignView.sim_checking`.
            let checking: Bool
            let block: String?
            let line: String?
            var description: String {
                "(\(simulation), judged \(judged), checking \(checking), block \(block ?? "-"))"
            }
        }

        private(set) var samples: [Frame] = []
        private let controller: SigningController

        init(_ controller: SigningController) {
            self.controller = controller
            arm()
        }

        func sample() {
            let gate = controller.confirmState
            samples.append(Frame(
                simulation: controller.simulation, judged: controller.simVerdict != nil,
                checking: controller.sign.checkingSim, block: gate.block, line: gate.key
            ))
        }

        private func arm() {
            withObservationTracking {
                _ = controller.simulation
                _ = controller.simVerdict
                _ = controller.sign
                _ = controller.signJson
            } onChange: { [weak self] in
                Task { @MainActor in
                    self?.sample()
                    self?.arm()
                }
            }
        }
    }

    // MARK: - The timer is a timer and nothing else

    /// The real core asks for its deadline when told the simulation is out:
    /// an operation this build's executor handles, carrying its request, its
    /// round and four seconds. The executor's answer names the same request
    /// and round, after the whole of the wait it was given.
    @Test func theCoresDeadlineIsAnOperationThisExecutorRunsWholeAndAnswersByIdAndRound() async throws {
        let scene = try #require(SimWaitScene.views(waitedOut: false))
        let timer = scene.timer
        #expect(SignExecutor.operations.contains("sim_verdict_timer"))
        #expect(timer["id"] as? String == scene.request.id)
        #expect((timer["ms"] as? NSNumber)?.intValue == 4_000)
        let round = try #require((timer["round"] as? NSNumber)?.intValue)

        let relay = scriptedRelay()
        let accounts = ScriptedAccounts()
        let store = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.sim-timer.\(UUID().uuidString)")!)
        func executor(_ timers: FeeStore.Timers) -> SignExecutor {
            SignExecutor(
                spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
                relay: relay, store: store, timers: timers
            )
        }

        // The wall clock: not answered before its `ms` has passed.
        var short = timer
        short["ms"] = 250
        let started = ContinuousClock.now
        let fired = try CoreJSON.object(await executor(.wallClock).perform(short))
        #expect(ContinuousClock.now - started >= .milliseconds(250), "the wait was shortened")
        #expect(fired["type"] as? String == "sim_verdict_timer_fired")
        #expect(fired["id"] as? String == scene.request.id)
        #expect((fired["round"] as? NSNumber)?.intValue == round)
        #expect(Set(fired.keys) == ["type", "id", "round"])

        // A stopped clock: held until the test moves it, whatever its `ms`.
        let stopped = executor(.stopped)
        let done = Flag()
        let answer = Kept<String>()
        Task {
            answer.value = await stopped.perform(timer)
            done.set()
        }
        await Wait.until { stopped.heldSimVerdictTimers == 1 }
        try await Task.sleep(for: .milliseconds(150))
        #expect(!done.isSet, "a stopped clock answered by itself")
        stopped.elapseSimVerdictTimer()
        await Wait.until { done.isSet }
        let held = try CoreJSON.object(try #require(answer.value))
        #expect(held["id"] as? String == scene.request.id)
        #expect((held["round"] as? NSNumber)?.intValue == round)

        // The gate takes the block's name and the corpus has its sentence.
        #expect(scene.gate == SignConfirmStateWire(enabled: false, block: "sim_checking", key: Self.checkingKey))
        #expect(en.t(Self.checkingKey) == "Checking what this transaction does…")
        #expect(Loc(overrideTag: "zh", preferredLanguages: []).t(Self.checkingKey) == "正在检查这笔交易的结果…")
        for tag in Loc.supported {
            let said = Loc(overrideTag: tag, preferredLanguages: []).t(Self.checkingKey)
            #expect(said != Self.checkingKey && !said.isEmpty, "\(tag) has no sentence for the hold")
        }
    }

    /// What a task hands back to the test that started it.
    @MainActor
    final class Kept<Value> {
        var value: Value?
        nonisolated deinit {}
    }

    // MARK: - The judge answers for the deltas it was sent

    /// `TokenTrustStore.simJudged` returns the judgment of THE deltas it was
    /// handed: at once when nothing needs a name, once the names are in when
    /// something does — and `nil` to whoever was still waiting when a later
    /// simulation took the judge's one session.
    @Test func theJudgeAnswersForTheDeltasItWasSentAndNilWhenTheyWereReplaced() async throws {
        let rig = rig()
        let native: [[String: Any]] = [["kind": "native", "token": NSNull(), "delta": "-1"]]
        let unknown: [[String: Any]] = [["kind": "erc20", "token": TrustCoreScene.unknown, "delta": "7"]]

        let first = try #require(await rig.trust.simJudged(address: safe, chainId: 137, deltas: native))
        #expect(first.ready && first.judgments == [.native(delta: "-1")])

        // A token to name: the answer waits for the judge's metadata read,
        // and is that session's.
        let second = try #require(await rig.trust.simJudged(address: safe, chainId: 137, deltas: unknown))
        #expect(second.ready)
        #expect(second.judgments == [.erc20Unverified(token: TrustCoreScene.unknown, direction: .in)])

        // Replaced while waiting: the earlier caller is told there will be
        // no judgment of its deltas — never handed the later one's.
        var replacedOnce = false
        for attempt in 0..<5 where !replacedOnce {
            let other = "0x6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6\(attempt)"
            let earlierAnswer = Kept<TrustSimViewWire?>()
            let trust = rig.trust
            let me = safe
            let earlier = Task {
                earlierAnswer.value = .some(await trust.simJudged(
                    address: me, chainId: 137, deltas: [["kind": "erc20", "token": other, "delta": "9"]]
                ))
            }
            // The earlier call has sent its deltas and is waiting for a name.
            await Task.yield()
            let later = await rig.trust.simJudged(address: safe, chainId: 137, deltas: native)
            await earlier.value
            #expect(later?.judgments == [.native(delta: "-1")], "the later call has its own judgment")
            switch earlierAnswer.value {
            case .some(.none):
                replacedOnce = true
            case .some(.some(let own)):
                // Judged before the later one was sent: then it is its OWN.
                #expect(own.judgments == [.erc20Unverified(token: other, direction: .in)])
            case .none:
                Issue.record("the earlier call never returned")
            }
        }
        #expect(replacedOnce, "a call whose session was replaced was never told so")
    }

    // MARK: - The confirm stays at one y

    /// The board's scene (`SimWaitScene`) as the sheet draws it in `loc`.
    private func board(_ scene: SimWaitScene.Views, _ loc: Loc) -> SigningModel {
        var context = SigningLive.Context(
            loc: loc, chainName: "Base", chainDot: .blue, nativeSymbol: "ETH",
            walletName: "Me", walletAddress: safe
        )
        context.chainId = 8_453
        return SigningLive.model(
            fallback: SigningFixtures.build(.cs1, loc: loc), request: scene.request, sign: scene.sign,
            clear: scene.clear, guard: scene.guardView, fee: HandoffFeeFixtures.feeView, context: context,
            gate: scene.gate
        )
    }

    /// The real sheet, hosted, through a request's wait — one live sheet,
    /// the controller's own models: held with the line under the confirm;
    /// past the deadline, the line gone and the caution in the verdict's
    /// place; then the late verdict's card there. The confirm stands at ONE
    /// y through all of it — and at that y on a sheet that never said a
    /// line at all (the waited-out board, fresh).
    ///
    /// Four reads, on purpose: a hosted sheet is heavy on the main actor
    /// every suite shares (`SigningSheetProbe`). Chinese, both boards on the
    /// presented sheet, light and dark, are the simulator walk's
    /// (`FinalRoundScreenshotTests.testTheConfirmWaitsForTheVerdict`).
    @Test func theConfirmStaysAtOneYThroughTheWaitTheDeadlineAndTheVerdict() async throws {
        let rig = rig()
        let controller = rig.controller
        controller.open(transaction)
        await untilOnlyTheVerdictIsMissing(controller)
        let checking = en.t(Self.checkingKey)
        let couldNotCheck = en.t(Self.couldNotCheckKey)

        let probe = SigningSheetProbe(try drawn(controller))
        let held = try await probe.read { $0.confirmLine?.label == checking }
        controller.elapseSimVerdictTimer()
        await Wait.until { controller.confirmState.enabled }
        let waitedOut = try await probe.show(try drawn(controller)) { $0.says(couldNotCheck) }
        rig.simulation.answer(checked())
        await Wait.until { controller.sign.simWaitedOutKey == nil }
        let late = try await probe.show(try drawn(controller)) { !$0.says(couldNotCheck) }
        probe.close()

        // A sheet of its own that never held: no line was ever said on it.
        let scene = try #require(SimWaitScene.views(waitedOut: true))
        let fresh = SigningSheetProbe(board(scene, en))
        let neverHeld = try await fresh.read { $0.says(couldNotCheck) }
        fresh.close()

        let steps = [("held", held), ("waited-out", waitedOut), ("late-verdict", late), ("never-held", neverHeld)]
        var ys: [CGFloat] = []
        for (name, seen) in steps {
            let confirm = try #require(seen.confirm, "\(name): no confirm in the tree")
            ys.append(confirm.frame.minY)
            let line = seen.confirmLine
            print("MEASURE sim-wait \(name) confirm.y=\(confirm.frame.minY) h=\(confirm.frame.height)"
                  + " enabled=\(confirm.enabled) line=\(line.map { "\"\($0.label)\" y=\($0.frame.minY) h=\($0.frame.height)" } ?? "none")")
        }
        #expect(Set(ys).count == 1, "the confirm moved: \(ys)")

        // Held: shut, the core's line under it, "Checking…" in the place.
        #expect(held.confirm?.enabled == false)
        #expect(held.confirmLine?.label == checking)
        // The line the sheet drew is as tall as `noteHeight` says it is —
        // what lets the fifteen languages be measured without a sheet each.
        let drawnLine = try #require(held.confirmLine).frame
        let footerWidth = try #require(held.confirm).frame.width
        #expect(abs(drawnLine.height - noteHeight(checking, width: footerWidth)) < 0.5,
                "the sheet's line (\(drawnLine.height)) is not the line `noteHeight` measures")
        #expect(held.says(en.t("componentsUi.funding.checking")))
        // Open: no line is said, and the verdict's place holds what the
        // core made of the simulation.
        for (name, seen) in steps.dropFirst() {
            #expect(seen.confirm?.enabled == true, "\(name)")
            #expect(seen.confirmLine == nil, "\(name): \(seen.confirmLine?.label ?? "")")
            #expect(!seen.says(checking), "\(name)")
        }
        #expect(waitedOut.says(couldNotCheck), "\(waitedOut.words)")
        #expect(!late.says(couldNotCheck) && late.says("POL"), "the late verdict replaced it: \(late.words)")
        #expect(neverHeld.says(couldNotCheck))
        controller.swipeDismissed()
    }

    /// The footer's line as `SigningSheet.confirmNote` draws it — its type
    /// role, its alignment, its wrap — asked for its height at `width` with
    /// no sheet hosted. `theConfirmStaysAtOneY…` holds this mirror to the
    /// real thing: the hosted sheet's line is exactly as tall as it says.
    private func noteHeight(_ text: String, width: CGFloat) -> CGFloat {
        let note = Text(verbatim: text)
            .typeRole(Typography.rowSub)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity)
        return UIHostingController(rootView: note)
            .sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude)).height
    }

    /// The hold line is ONE line in every language, on a narrow phone: the
    /// room the footer keeps under the confirm is a line's — a blank one
    /// until a line is said — so a sentence that wrapped, or stood taller
    /// than that blank, would lift the confirm when it appeared.
    @Test func theHoldLineIsOneLineInEveryLanguage() {
        // The footer's width on a 375-point phone, inside its padding.
        let width = 375 - 2 * Tokens.Layout.screenPaddingX
        let blank = noteHeight(" ", width: width)
        #expect(blank > 0)
        var heights: [String: CGFloat] = [:]
        for tag in Loc.supported {
            let sentence = Loc(overrideTag: tag, preferredLanguages: []).t(Self.checkingKey)
            let height = noteHeight(sentence, width: width)
            heights[tag] = height
            #expect(abs(height - blank) < 0.5, "\(tag): \"\(sentence)\" is \(height) tall in a room of \(blank)")
        }
        // The measure can tell: a sentence twice as long does wrap.
        let long = en.t(Self.checkingKey) + " " + en.t(Self.checkingKey)
        #expect(noteHeight(long, width: width) > blank * 1.5, "the measure did not see a wrap")
        print("MEASURE hold-line width=\(width) blank=\(blank) heights=\(heights.sorted { $0.key < $1.key })")
    }

    // MARK: - The boards

    /// `VELA_PAGE=pr3c` `VELA_STATE=sheet-held` / `sheet-waited-out`: the
    /// scene the boards draw is the real `sign_request` core's, under the
    /// core's own gate — held with the line, and open with the caution.
    @Test func theBoardsScenesAreTheCoresOwn() throws {
        let held = try #require(SimWaitScene.views(waitedOut: false))
        #expect(held.sign.checkingSim && held.sign.simWaitedOutKey == nil)
        #expect(held.sign.request?.id == held.request.id)
        #expect(held.gate == SignConfirmStateWire(enabled: false, block: "sim_checking", key: Self.checkingKey))

        let waitedOut = try #require(SimWaitScene.views(waitedOut: true))
        #expect(!waitedOut.sign.checkingSim)
        #expect(waitedOut.sign.simWaitedOutKey == Self.couldNotCheckKey)
        #expect(waitedOut.gate == SignConfirmStateWire(enabled: true, block: nil, key: nil))
    }
}
