//
//  R22FollowUpTests.swift
//  VelaWalletTests
//
//  §39 — the Round 22 follow-ups, on this shell, through the real core.
//
//  - r22-1 (S1, found here): Parallel Two held 0.222 USDC; Max read 0.066
//    with a 0.156 fee; a quote of 0.154045 moved it ("Max is now 0.067955
//    (it was 0.066)"); a quote back at 0.156 flipped it back WITHOUT A WORD,
//    and the press signed 0.066 + 0.156. Every move is said from the figure
//    drawn now, and the press signs exactly what is drawn — or, if the quote
//    rose under it, nothing.
//  - A1: the dApp sheet's `quoted_fee` is the fee DRAWN when the confirm
//    began (the hold's start, the assistive path's first press), its fee row
//    held still while the confirm is in progress; `current_fee` the live
//    quote. A rise signs nothing (`fee_rose`), said, the new fee drawn.
//  - r22-6: a re-check of a landed verdict keeps the confirm's door.
//  - The native value the wallet decoded is never erased by a node's
//    "No asset changes" (`sim_outcome`'s `value_wei`).
//  - Multisig words: `signingDisagreed`, `signingUnchecked`, the fee stop's
//    `feeRose`, the review's `oneNode` line and "Add your own node".
//  - r22-8: a sign-in failure with `words_key` is said in those words.
//  - Unlimited approvals: the confirm's posture is the core's.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - r22-1: a Max that moves and moves back

@MainActor
struct R22MaxMovesBackTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private static let account = "0x8275e99B9c20f1dA503492D37f6832ea0104f2eC"
    private static let payee = "0x3e9b950000000000000000000000000000031692"
    private static let collector = "0x5c3D966577b66a12B815119f1af452Ed7adf260c"

    /// A native fee on Arc, for an account its first send deploys.
    private func fee(_ wei: String) -> [String: Any] {
        [
            "chain_id": 5_042_002, "total_wei": wei, "max_fee_per_gas": "0",
            "network_fee_per_gas": "20000000000", "relayer_fee_per_gas": "0",
            "bundler_gas_price": "20000000000", "in_band_gas_basis": "20000000000",
            "effective_gas_price": NSNull(), "max_gas_price": NSNull(), "total_gas": "450000",
            "deployed": false, "tier": "fast", "quoted": true,
            "fee_asset": ["type": "native"], "fee_recipient": Self.collector,
        ]
    }

    private struct Missing: Error {}

    /// The real send core, answered by hand (`SendPressFreezeTests.Drive`'s
    /// shape): Parallel Two's 0.222 USDC on Arc; the press's own reads
    /// answered only while not `holding`.
    @MainActor
    private final class Drive {
        let core = SendCore()
        var latest: [String: Any] = [:]
        var quote: [String: Any]?
        var holding = false
        var parked: [(UInt64, [String: Any])] = []

        func run(_ event: [String: Any]) throws {
            try absorb(try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event))))
        }

        func release() throws {
            holding = false
            let queue = parked
            parked = []
            for (id, op) in queue {
                guard let reply = answer(op) else { parked.append((id, op)); continue }
                try absorb(try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(reply))))
            }
        }

        private func absorb(_ first: [String: Any]) throws {
            var result = first
            latest = result["view"] as? [String: Any] ?? latest
            var queue = result["effects"] as? [[String: Any]] ?? []
            while !queue.isEmpty {
                let effect = queue.removeFirst()
                guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                      let op = effect["operation"] as? [String: Any] else { continue }
                guard let reply = answer(op) else {
                    parked.append((id, op))
                    continue
                }
                result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(reply)))
                latest = result["view"] as? [String: Any] ?? latest
                queue += result["effects"] as? [[String: Any]] ?? []
            }
        }

        func answer(_ op: [String: Any]) -> [String: Any]? {
            switch op["type"] as? String {
            case "fetch_tokens":
                return [
                    "type": "tokens_loaded",
                    "tokens": [[
                        "network": "arc-testnet", "chain_id": 5_042_002, "symbol": "USDC", "balance": "0.222",
                        "decimals": 18, "token_address": NSNull(), "price_usd": 1.0,
                        "logo_urls": [String](), "spam": false,
                    ] as [String: Any]],
                    "chains": [["network": "arc-testnet", "chain_id": 5_042_002, "native_symbol": "USDC"]],
                ]
            case "prewarm_fees": return ["type": "fees_prewarmed"]
            case "estimate_fee":
                guard let quote else {
                    return ["type": "fee_estimated", "outcome": ["type": "failed", "kind": "quote_unavailable"]]
                }
                return ["type": "fee_estimated", "outcome": ["type": "ok", "estimate": quote]]
            case "load_account_credential":
                return holding ? nil : ["type": "account_credential", "public_key_hex": "04deadbeef"]
            case "probe_treasury":
                return holding ? nil : ["type": "treasury_probed", "probe": ["type": "covered"]]
            case "resolve_risk": return ["type": "risk_resolved", "risk": NSNull()]
            case "resolve_identity": return ["type": "identity_resolved", "identity": NSNull()]
            case "simulate_calls": return ["type": "sim_resolved", "sim_json": NSNull()]
            default: return nil
            }
        }

        var submits: [[String: Any]] {
            parked.map(\.1).filter { $0["type"] as? String == "submit_user_op" }
        }

        func view() throws -> SendViewWire { try CoreJSON.decode(SendViewWire.self, from: latest) }
    }

    private func confirm(_ view: SendViewWire) throws -> SendConfirmModel {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            throw Missing()
        }
        return SendLive.confirm(view, from: (address: Self.account, name: "Parallel Two"), display: .usd, on: drawn, loc: loc)
    }

    /// The confirm page as Parallel Two saw it: Max 0.066 + 0.156.
    private func atTheConfirm() throws -> Drive {
        let drive = Drive()
        try drive.run([
            "type": "open",
            "account": ["id": "cred-0", "address": Self.account, "name": "Parallel Two"] as [String: Any],
            "params": [
                "preselected_symbol": NSNull(), "preselected_network": NSNull(),
                "prefilled_recipient": NSNull(), "prefilled_chain_id": NSNull(),
                "prefilled_token_address": NSNull(), "prefilled_amount_base": NSNull(),
                "locked": false, "preselected_multi": NSNull(),
            ] as [String: Any],
            "display": ["code": "USD", "rate": 1.0, "fiat_decimals": 2] as [String: Any],
        ])
        try drive.run(["type": "select_token", "token_id": "arc-testnet_native_USDC"])
        try drive.run(["type": "set_recipient", "recipient": Self.payee])
        try drive.run(["type": "fee_updated", "estimate": fee("156000000000000000")])
        try drive.run(["type": "tap_max"])
        drive.quote = fee("156000000000000000")
        try drive.run(["type": "continue"])
        let view = try drive.view()
        #expect(view.stage == .confirm)
        #expect(view.confirmAmount == "0.066")
        #expect(view.fee?.totalWei == "156000000000000000")
        #expect(view.canConfirm)
        return drive
    }

    private func signed(_ drive: Drive) throws -> (amount: String?, fee: String?) {
        let submit = try #require(drive.submits.first, "nothing was signed")
        let review = try #require(submit["review"] as? [String: Any])
        let transfers = try #require(review["transfers"] as? [[String: Any]])
        return (transfers.first?["amount"] as? String, (review["fee"] as? [String: Any])?["amount"] as? String)
    }

    /// The bug as it happened: the quote falls (said), then comes back
    /// (said too — it was the silent flip), and the press signs exactly the
    /// figures drawn at the press: 0.066 + 0.156.
    @Test func aMaxThatMovesAndMovesBackIsSaidEachTimeAndThePressSignsWhatIsDrawn() throws {
        let drive = try atTheConfirm()

        try drive.run(["type": "fee_updated", "estimate": fee("154045000000000000")])
        var view = try drive.view()
        #expect(view.confirmAmount == "0.067955")
        #expect(view.maxMoved == SendMaxMovedWire(from: "0.066", to: "0.067955"))
        #expect(try confirm(view).maxNote == loc.t("send.maxMoved", vars: [
            "from": "0.066", "to": "0.067955", "symbol": "USDC",
        ]))

        // Back at 0.156: the move back is said, from the figure drawn now.
        // (The old core said it from the figure first shown — equal to the
        // new one — so `max_moved` was nil and the figures flipped back
        // silently: `a_max_that_moves_and_moves_back_is_said_each_time`'s
        // red run in the core, "the move back is said, never a silent flip".)
        try drive.run(["type": "fee_updated", "estimate": fee("156000000000000000")])
        view = try drive.view()
        #expect(view.confirmAmount == "0.066")
        #expect(view.fee?.totalWei == "156000000000000000")
        #expect(view.maxMoved == SendMaxMovedWire(from: "0.067955", to: "0.066"),
                "the move back is said, never a silent flip")
        #expect(try confirm(view).maxNote == loc.t("send.maxMoved", vars: [
            "from": "0.067955", "to": "0.066", "symbol": "USDC",
        ]))

        // The press names the review it was drawn on: signed exactly.
        try drive.run(["type": "slide_confirm", "review": view.reviewDigest ?? NSNull()])
        let (amount, fee) = try signed(drive)
        #expect(amount == "66000000000000000")
        #expect(fee == "156000000000000000")
    }

    /// A press on 0.067955 + 0.154045 whose quote comes back to 0.156 under
    /// it: a rise above the figure pressed — nothing signed, the stop said
    /// with the new Max (`send.feeMovedMax`).
    @Test func aQuoteThatRoseAboveTheFigurePressedStopsThePressAndSignsNothing() throws {
        let drive = try atTheConfirm()
        try drive.run(["type": "fee_updated", "estimate": fee("154045000000000000")])
        var view = try drive.view()
        #expect(view.confirmAmount == "0.067955")

        drive.holding = true
        try drive.run(["type": "slide_confirm", "review": view.reviewDigest ?? NSNull()])
        try drive.run(["type": "fee_updated", "estimate": fee("156000000000000000")])
        view = try drive.view()
        #expect(view.confirmAmount == "0.067955", "the amount pressed stays drawn")
        #expect(view.fee?.totalWei == "154045000000000000", "the fee pressed stays drawn")

        try drive.release()
        #expect(drive.submits.isEmpty, "nothing is signed on a quote that rose")
        view = try drive.view()
        #expect(view.txStatus == "idle")
        #expect(view.confirmAmount == "0.066")
        #expect(view.fee?.totalWei == "156000000000000000")
        #expect(view.feeMoved == SendFeeMovedWire(
            key: "send.feeMovedMax", max: SendMaxMovedWire(from: "0.067955", to: "0.066")
        ))
        #expect(try confirm(view).feeMovedNote == loc.t("send.feeMovedMax", vars: [
            "from": "0.067955", "to": "0.066", "symbol": "USDC",
        ]))
    }

    /// A press on 0.067955 + 0.154045 whose quote falls under it: the
    /// figures pressed are signed exactly — never the newer, smaller fee
    /// with a bigger Max nobody read.
    @Test func aQuoteBelowTheFigurePressedSignsExactlyTheFiguresPressed() throws {
        let drive = try atTheConfirm()
        try drive.run(["type": "fee_updated", "estimate": fee("154045000000000000")])
        let view = try drive.view()
        #expect(view.confirmAmount == "0.067955")

        drive.holding = true
        try drive.run(["type": "slide_confirm", "review": view.reviewDigest ?? NSNull()])
        try drive.run(["type": "fee_updated", "estimate": fee("150000000000000000")])
        try drive.release()
        let (amount, fee) = try signed(drive)
        #expect(amount == "67955000000000000")
        #expect(fee == "154045000000000000")
        #expect(try drive.view().feeMoved == nil)
    }

    /// The shell's own press names the review drawn under the button
    /// (`RootView`: the digest captured at the draw), never the view's
    /// latest: a confirm drawn on 0.067955 whose figures flipped back before
    /// the tap confirms nothing.
    @Test func aPressDrawnOnAnEarlierReviewConfirmsNothing() throws {
        let drive = try atTheConfirm()
        try drive.run(["type": "fee_updated", "estimate": fee("154045000000000000")])
        let drawn = try drive.view().reviewDigest
        try drive.run(["type": "fee_updated", "estimate": fee("156000000000000000")])
        #expect(try drive.view().reviewDigest != drawn)
        try drive.run(["type": "slide_confirm", "review": drawn ?? NSNull()])
        #expect(drive.submits.isEmpty, "a review not drawn now confirms nothing")
        #expect(try drive.view().txStatus == "idle")
    }

    /// The sheet's source keeps taking the digest at the draw.
    @Test func theConfirmCapturesTheDigestAtTheDraw() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/App/RootView.swift"), encoding: .utf8)
        #expect(source.contains("onConfirm: { [drawn = send.view?.reviewDigest] in confirmTransfer(review: drawn) }"))
    }
}

// MARK: - The dApp sheet: A1, r22-6, the native value, the posture

@MainActor
@Suite(.serialized, .hangLimit)
struct R22SheetFollowUpTests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let collector = "0x2222222222222222222222222222222222222222"
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let site = "https://app.example"

    @MainActor
    final class Simulation {
        var immediate: RpcCorroborated?
        private(set) var asked = 0
        private var waiting: [CheckedContinuation<RpcCorroborated, Never>] = []

        func run() async -> RpcCorroborated {
            asked += 1
            if let immediate { return immediate }
            return await withCheckedContinuation { waiting.append($0) }
        }

        func answer(_ outcome: RpcCorroborated) {
            let held = waiting
            waiting = []
            for resume in held { resume.resume(returning: outcome) }
        }
    }

    private struct Rig {
        let controller: SigningController
        let port: ScriptedRelayPort
        let simulation: Simulation
        let signer: CountingSigner
        let answers: Answers
    }

    /// What the site was answered, in order.
    @MainActor
    final class Answers {
        var payloads: [[String: Any]] = []
    }

    /// The per-gas price the relay publishes, in wei (hex).
    private func price(_ port: ScriptedRelayPort, _ hex: String) {
        port.rpc["eth_gasPrice"] = .ok(hex)
        let tier: [String: Any] = [
            "maxFeePerGas": hex, "networkFeePerGas": hex, "relayerFeePerGas": hex,
        ]
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok(["fast": tier, "standard": tier, "slow": tier] as [String: Any])
    }

    /// A deployed Safe on Polygon holding 100 POL (`SimVerdictWaitTests`'
    /// world), the relay's price set by `price`.
    private func rig() -> Rig {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x0"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x0")
        price(port, "0x1fcbc261a00")
        port.rpc["vela_getInBandGasQuote"] = .ok([
            ["recipient": collector, "asset": "native", "feeToken": NSNull(),
             "balance": "0x56bc75e2d63100000", "decimals": 18, "symbol": "POL",
             "usdBalance": "10.75", "usdPrice": "0.1075"] as [String: Any],
        ])
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        port.rest["/v1/account/137/\(safe.lowercased())"] = .ok([
            "activeDepositAddress": collector, "status": "ACTIVE",
        ])
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let defaults = UserDefaults(suiteName: "vela.tests.r22f.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let appAccounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: appAccounts, offline: true)
        let trust = TokenTrustStore(store: store, pool: pool, accounts: appAccounts, held: HeldTokens())
        let accounts = ScriptedAccounts()
        let signer = CountingSigner()
        let simulation = Simulation()
        let answers = Answers()
        let controller = SigningController(
            wallet: (address: safe, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { signer }),
            store: store, pool: pool,
            ports: SigningController.Ports(
                respond: { _, _, payload, _ in answers.payloads.append(payload) },
                nativeSymbol: { _ in "POL" },
                knownChains: { [137] },
                simJudged: { address, chainId, deltas in
                    await trust.simJudged(address: address, chainId: chainId, deltas: deltas)
                },
                simulate: { _, _ in await simulation.run() }
            ),
            timers: .stopped
        )
        return Rig(controller: controller, port: port, simulation: simulation, signer: signer, answers: answers)
    }

    /// 1.5 POL to somebody.
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

    /// Two nodes agree 1.5 POL leaves the wallet.
    private var checked: RpcCorroborated {
        let topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        let log: [String: Any] = [
            "address": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            "topics": [topic, "0x" + String(repeating: "0", count: 24) + bare(safe),
                       "0x" + String(repeating: "0", count: 24) + bare(payee)],
            "data": "0x" + String(repeating: "0", count: 48) + "14d1120d7b160000",
        ]
        return .agreed(.ok([["calls": [["status": "0x1", "returnData": "0x", "logs": [log]] as [String: Any]]] as [String: Any]]))
    }

    private func total(_ controller: SigningController) -> String? { controller.fee?.fee?.totalWei }

    /// A fresh quote at the relay's new `price`, landed.
    private func requote(_ rig: Rig, to hex: String, from before: String?) async {
        price(rig.port, hex)
        rig.controller.refreshFee()
        await Wait.until { rig.controller.feeIdle && total(rig.controller) != before && total(rig.controller) != nil }
    }

    private func ready(_ rig: Rig) async {
        rig.simulation.immediate = checked
        rig.controller.open(transaction)
        await Wait.until { rig.controller.confirmState.enabled && rig.controller.fee?.fee != nil }
    }

    // MARK: A1

    /// The hold began on fee A; a quote B > A landed under the hold. The
    /// fee row stays at A while the hold lasts; at the approve the core
    /// refuses (`fee_rose`): nothing signed, no answer to the site, the new
    /// fee drawn, the reason said beside the confirm. The next confirm pays B.
    @Test func aQuoteThatRoseUnderTheHoldSignsNothingAndSaysWhy() async throws {
        let rig = rig()
        let controller = rig.controller
        await ready(rig)
        let shown = try #require(total(controller))

        controller.confirmEngaged(true)
        await requote(rig, to: "0x2fcbc261a00", from: shown)
        let risen = try #require(total(controller))
        #expect(UInt64(risen)! > UInt64(shown)!, "the quote rose: \(shown) → \(risen)")
        #expect(controller.drawnFee?.fee?.totalWei == shown, "the fee row is held still while the hold lasts")

        controller.approve()
        await Wait.until { controller.sign.error != nil || rig.signer.calls > 0 }
        #expect(controller.sign.error?.kind == .feeRose, "\(String(describing: controller.sign.error))")
        #expect(rig.signer.calls == 0, "nothing signed")
        #expect(!controller.hasAnswered, "no answer is sent to the site")
        #expect(!controller.confirmInProgress, "disarmed")
        #expect(controller.drawnFee?.fee?.totalWei == risen, "the new fee is drawn")
        let said = SigningLive.statusBlocks(sign: controller.shownSign, loc: en)
        #expect(said.contains {
            if case .warning(.caution, let text) = $0 { return text == en.t("componentsUi.signing.feeRose") }
            return false
        }, "\(said.map(\.id))")

        // The next confirm pays the figure now drawn.
        await Wait.until { controller.confirmState.enabled }
        controller.approve()
        await Wait.until { rig.signer.calls > 0 || (controller.sign.error.map { $0.kind != .feeRose } ?? false) }
        #expect(rig.signer.calls == 1, "the next confirm goes to the passkey")
        controller.swipeDismissed()
    }

    /// After a `fee_rose` stop nothing was sent and no answer is held: the
    /// sheet's close (its ✕, `swipe_dismissed`) refuses the request — the
    /// site is answered 4001 at once, never left waiting for an answer that
    /// would not come (core `dfcc25c55`; the core's red run, with the rule
    /// off, routed the swipe to `Dismiss` and sent nothing).
    @Test func aCloseAfterAFeeRoseStopAnswersTheSite4001() async throws {
        let rig = rig()
        let controller = rig.controller
        await ready(rig)
        let shown = try #require(total(controller))
        controller.confirmEngaged(true)
        await requote(rig, to: "0x2fcbc261a00", from: shown)
        controller.approve()
        await Wait.until { controller.sign.error != nil || rig.signer.calls > 0 }
        #expect(controller.sign.error?.kind == .feeRose)
        #expect(rig.answers.payloads.isEmpty, "the stop itself answers nothing")

        controller.swipeDismissed()
        await Wait.until { !rig.answers.payloads.isEmpty }
        #expect(rig.answers.payloads.count == 1)
        let payload = try #require(rig.answers.payloads.first)
        #expect((payload["code"] as? NSNumber)?.intValue == 4001, "\(payload)")
        #expect(payload["kind"] as? String == "user_rejected", "\(payload)")
        #expect(rig.signer.calls == 0, "nothing signed")
    }

    /// A quote that FELL under the hold: the figure drawn is signed — the
    /// approve goes on to the passkey, nothing refused.
    @Test func aQuoteThatFellUnderTheHoldSignsTheFigureShown() async throws {
        let rig = rig()
        let controller = rig.controller
        await ready(rig)
        let shown = try #require(total(controller))

        controller.confirmEngaged(true)
        await requote(rig, to: "0x0fcbc261a00", from: shown)
        let fallen = try #require(total(controller))
        #expect(UInt64(fallen)! < UInt64(shown)!, "the quote fell: \(shown) → \(fallen)")
        #expect(controller.drawnFee?.fee?.totalWei == shown)

        controller.approve()
        await Wait.until { rig.signer.calls > 0 || controller.sign.error != nil }
        #expect(rig.signer.calls == 1, "\(String(describing: controller.sign.error))")
        #expect(controller.sign.error?.kind != .feeRose)
        controller.swipeDismissed()
    }

    /// What the approve carries: `quoted_fee` the figure drawn when the
    /// confirm began, `current_fee` the live quote.
    @Test func theApproveCarriesTheFigureDrawnAndTheLiveQuote() async throws {
        let rig = rig()
        await ready(rig)
        let live = try #require(rig.controller.fee)
        let opts = SigningController.approveOpts(fee: live, clear: .empty, guard: .empty, currentFee: live)
        #expect((opts["quoted_fee"] as? [String: Any])?["amount"] as? String == live.fee?.totalWei)
        #expect((opts["current_fee"] as? [String: Any])?["amount"] as? String == live.fee?.totalWei)
        #expect((opts["current_fee"] as? [String: Any])?["recipient"] as? String == live.fee?.feeRecipient)
        #expect(SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)["current_fee"] is NSNull)
        rig.controller.swipeDismissed()
    }

    /// A confirm that ended without firing lets the fee row follow again.
    @Test func aHoldLiftedEarlyLetsTheFeeRowFollowAgain() async throws {
        let rig = rig()
        let controller = rig.controller
        await ready(rig)
        let shown = try #require(total(controller))
        controller.confirmEngaged(true)
        controller.confirmEngaged(true)
        await requote(rig, to: "0x2fcbc261a00", from: shown)
        #expect(controller.drawnFee?.fee?.totalWei == shown, "a second begin takes no new figure")
        controller.confirmEngaged(false)
        #expect(controller.drawnFee?.fee?.totalWei == total(controller))
        controller.swipeDismissed()
    }

    @Test func theKindDecodes() throws {
        let kinds = try CoreJSON.decoder.decode([SignErrorKind].self, from: Data(#"["fee_rose"]"#.utf8))
        #expect(kinds == [.feeRose])
    }

    // MARK: r22-6

    /// The could-not-check verdict landed (the confirm "… anyway", held);
    /// "Check again" re-checks it: the confirm's door does not flip to the
    /// shut "Checking…" in any step, and a re-check landing the same verdict
    /// leaves it exactly as it was — a hold or an arm on it survives
    /// (`HoldConfirmButton` resets only when `enabled` changes).
    @Test func aRecheckOfALandedVerdictKeepsTheConfirmsDoor() async throws {
        let rig = rig()
        let controller = rig.controller
        rig.simulation.immediate = .unreachable
        controller.open(transaction)
        await Wait.until { controller.confirmState.enabled && controller.simRecheck }
        let door = controller.confirmState
        #expect(door.anyway == true && door.holdMs > 0, "\(door)")

        rig.simulation.immediate = nil
        controller.simulateAgain()
        #expect(controller.simRechecking)
        await Wait.until { rig.simulation.asked == 2 }
        // The old core took the re-check's `sim_started` as a new wait:
        // shut, "Checking…" (`a_recheck_of_a_landed_verdict_keeps_the_door`'s
        // red run in the core).
        #expect(controller.confirmState == door, "\(controller.confirmState)")
        #expect(!controller.sign.checkingSim)

        rig.simulation.answer(.unreachable)
        await Wait.until { !controller.simRechecking }
        #expect(controller.confirmState == door)
        controller.swipeDismissed()
    }

    // MARK: The native value

    /// A node answered "nothing moves" for a request that sends 1.5 POL from
    /// the account: the outflow the wallet decoded is drawn, never erased.
    @Test func aNodesNoChangeNeverErasesTheDecodedNativeValue() async throws {
        let rig = rig()
        let controller = rig.controller
        let nothing: RpcOutcome = .ok([["calls": [["status": "0x1", "returnData": "0x", "logs": [Any]()] as [String: Any]]] as [String: Any]])
        rig.simulation.immediate = .agreed(nothing)
        controller.open(transaction)
        await Wait.until { controller.simVerdict != nil || (controller.simulation != .pending && controller.simulation != .answered) }
        #expect(controller.simRecord?.kind == "deltas")
        #expect(controller.simRecord?.noChangeKey == nil, "never \"No asset changes\"")
        let verdict = try #require(controller.simVerdict)
        #expect(verdict.judgments == [.native(delta: "-1500000000000000000")], "\(verdict.judgments)")
        controller.swipeDismissed()
    }

    /// The value a request sends, as decoded: a batch's is the sum.
    @Test func theNativeValueIsTheSumOfTheCallsDecoded() {
        func call(_ value: String) -> UserOpCall { UserOpCall(to: payee, value: value, data: "0x") }
        #expect(SigningController.nativeValueWei([call("0x14d1120d7b160000")]) == "1500000000000000000")
        #expect(SigningController.nativeValueWei([call("0x1"), call("2"), call("0x0")]) == "3")
        #expect(SigningController.nativeValueWei([call("0x0"), call("")]) == nil)
        #expect(SigningController.nativeValueWei([]) == nil)
    }

    // MARK: Unlimited approvals

    /// The confirm's posture is the core's: an unbounded grant however it
    /// arrives (`GuardView.unlimited_warning`) is a held danger.
    @Test func anUnlimitedWarningHoldsTheConfirmAsADanger() async throws {
        let rig = rig()
        let controller = rig.controller
        controller.open(message)
        await Wait.until { controller.confirmState.enabled }
        #expect(controller.confirmState.holdMs == 0, "a plain message is a plain press")
        var guardView = try CoreJSON.object(try #require(controller.guardJson))
        guardView["unlimited_warning"] = true
        let gate = SignConfirmStateWire.of(
            sign: controller.shownSignJson, guard: CoreJSON.string(guardView), clear: controller.clearJson,
            fee: nil, speedTier: nil
        )
        #expect(gate.enabled)
        #expect(gate.holdMs == Int(holdToConfirmMs()), "\(gate)")
        #expect(gate.dangerStyle, "\(gate)")
        controller.swipeDismissed()
    }
}

// MARK: - The multisig's words (§39)

@MainActor
struct R22MultisigWordsTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private func state(_ object: [String: Any]) -> MsTagged? { MultisigMachine.decode(MsTagged.self, from: object) }

    /// One node's word is never "Can't reach"; two that disagree refuse,
    /// said as that.
    @Test func theGateSaysUncheckedAndDisagreedInTheirOwnWords() {
        let unchecked = words.actionFailure(state(["kind": "signingUnchecked"]), chainId: 100)
        #expect(unchecked == words.t("authority.signUnchecked", ["network": "Gnosis"]))
        #expect(unchecked?.hasPrefix("Can't reach") == false && unchecked?.hasPrefix("Can’t reach") == false)
        let disagreed = words.actionFailure(state(["kind": "signingDisagreed"]), chainId: 100)
        #expect(disagreed == words.t("authority.signDisagreed", ["network": "Gnosis"]))
        #expect(disagreed?.hasPrefix("Two independent nodes on Gnosis") == true, "\(disagreed ?? "")")
        #expect(disagreed?.contains("anyway") == false)
    }

    /// The execute's fee stop says why, through the core's own door
    /// (`review.failure` → `home.multisig.proposal.feeRose`), the new figure
    /// written as the fee line writes it.
    @Test func theFeeStopSaysTheFeeRoseWithTheNewFigure() {
        #expect(MsRules.failure(#"{"kind":"fee_moved","amount":"1","chainId":100,"executor":"0x1"}"#).key
                == "home.multisig.proposal.feeRose")
        let moved = #"{"kind":"fee_moved","amount":"97200000000000000","chainId":100,"executor":"0x00000000000000000000000000000000000000a1"}"#
        let line = words.failureText(moved)
        #expect(line == words.t("proposal.feeRose", ["fee": "0.0972 \(words.symbol(100))"]), "\(line)")
        #expect(line.hasPrefix("The network fee went up to ≈ 0.0972"), "\(line)")
        #expect(!line.contains(words.t("proposal.failed", ["reason": ""]).trimmingCharacters(in: .whitespaces)))
    }

    /// `oneNode`: said as what it is, never "checked", with "Add your own
    /// node" — only for that reason.
    @Test func aStateOnlyOneNodeCouldCheckIsSaidWithTheWayToAddOne() {
        let one = MsJSON(any: ["kind": "unavailable", "reason": "oneNode"])
        #expect(MsReviewBody.unreadStateLine(one, network: "Arc Testnet", words: words)
                == words.t("review.stateOneNode", ["network": "Arc Testnet"]))
        #expect(MsReviewBody.unreadStateLine(one, network: "Arc Testnet", words: words)
                .hasPrefix("Only one node could confirm who can act on Arc Testnet"))
        #expect(MsReviewBody.offersOwnNode(one))
        #expect(!MsReviewBody.offersOwnNode(MsJSON(any: ["kind": "unavailable", "reason": "uncorroborated"])))
        #expect(!MsReviewBody.offersOwnNode(MsJSON(any: ["kind": "unchanged"])))
        #expect(words.t("review.addOwnNode") == "Add your own node")
    }

    /// Drawn: the line and its link, which opens that network's settings.
    @Test func theReviewDrawsTheLinkToTheNetworksSettings() async throws {
        let review = MsJSON(any: [
            "sign": ["allowed": true, "anyway": true],
            "concerns": [[String: Any]](),
            "state": ["source": "none", "verdict": ["kind": "unavailable", "reason": "oneNode"]],
        ] as [String: Any])
        var opened: [Int] = []
        let screen = HostedScreen(
            MsReviewBody(review: review, chainId: 5_042_002, words: words)
                .environment(\.msOpenOwnNode, { opened.append($0) })
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.element("multisig.review.addOwnNode") != nil }
        #expect(seen.element("multisig.review.addOwnNode")?.label == "Add your own node")
        #expect(seen.says("Only one node could confirm"))
        // Without a way there, no link.
        let bare = HostedScreen(MsReviewBody(review: review, chainId: 5_042_002, words: words))
        defer { bare.close() }
        let none = try await bare.read { $0.says("Only one node could confirm") }
        #expect(none.element("multisig.review.addOwnNode") == nil)
        _ = opened
    }
}

// MARK: - r22-8: a sign-in failure in the catalog's words

@MainActor
struct R22SignInWordsTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The core's `SignInFailed` for a key-index group that does not make
    /// its recorded address carries `words_key`: those words are drawn —
    /// never the raw detail, never the biometric advice.
    @Test func aSignInFailureWithWordsIsSaidInThem() {
        let kind = PromptKind(json: [
            "type": "sign_in_failed", "detail": "registry group does not match its address",
            "phone_link": false, "words_key": "onboarding.login.registryUnreachableBody",
        ])
        let copy = promptCopy(kind, loc: loc)
        #expect(copy.message == loc.t("onboarding.login.registryUnreachableBody"))
        #expect(!copy.message.contains("registry group"))
        #expect(copy.message != loc.t(I18nKeys.Login.alertSignInFailedBody, vars: ["message": kind.detail ?? ""]))
    }
}
