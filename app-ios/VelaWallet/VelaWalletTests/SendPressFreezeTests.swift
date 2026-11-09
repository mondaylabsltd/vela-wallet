//
//  SendPressFreezeTests.swift
//  VelaWalletTests
//
//  §34 (R20 S1, found on this shell): charged = shown, frozen at the press.
//
//  "R20 iOS ZH" held 0.3 USDC on Arc Testnet and had never deployed. Max on
//  the confirm read 0.0675 with a 0.2325 fee; the chain took 0.0058875 and a
//  0.29411249999 fee leg. A re-quote landed between the press and the
//  signature: the core returned the button without a word and re-derived the
//  Max under it, and the next press signed figures nobody had confirmed.
//
//  Driven through the REAL send core (uniffi `SendCore`), answered by hand,
//  and drawn by this shell's own `SendLive.confirm` — so what is asserted is
//  what the confirm page shows.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendPressFreezeTests {

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

    /// The real core, answered by hand: quotes with `quote`, the press's own
    /// reads (the credential, the relay's float) only while not `holding`.
    @MainActor
    private final class Drive {
        let core = SendCore()
        var latest: [String: Any] = [:]
        var quote: [String: Any]?
        var holding = false
        var parked: [(UInt64, [String: Any])] = []

        func run(_ event: [String: Any]) throws {
            let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
            try absorb(result)
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
                        "network": "arc-testnet", "chain_id": 5_042_002, "symbol": "USDC", "balance": "0.3",
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
        return SendLive.confirm(view, from: (address: Self.account, name: "R20 iOS ZH"), display: .usd, on: drawn, loc: loc)
    }

    private func feeRow(_ model: SendConfirmModel) -> FactRowModel? {
        // From, To, Network, then the fee (a single send's confirm).
        model.facts.count > 3 ? model.facts[3] : nil
    }

    @Test func aQuoteMovedUnderThePressSignsNothingAndSaysSoWithTheNewFigures() throws {
        let drive = Drive()
        try drive.run([
            "type": "open",
            "account": ["id": "cred-0", "address": Self.account, "name": "R20 iOS ZH"] as [String: Any],
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
        try drive.run(["type": "fee_updated", "estimate": fee("267375000000000000")])
        try drive.run(["type": "tap_max"])
        // Continue: the pre-check prices 0.2325 (the deployment in it).
        drive.quote = fee("232500000000000000")
        try drive.run(["type": "continue"])
        var view = try drive.view()
        #expect(view.stage == .confirm)
        #expect(view.confirmAmount == "0.0675")
        #expect(view.fee?.totalWei == "232500000000000000")
        #expect(view.canConfirm)

        // The press. Its reads are out when the fee card's next quote lands.
        drive.holding = true
        try drive.run(["type": "slide_confirm", "review": view.reviewDigest ?? NSNull()])
        try drive.run(["type": "fee_updated", "estimate": fee("294112500000000000")])
        view = try drive.view()
        #expect(view.txStatus == "preparing")
        #expect(view.confirmAmount == "0.0675", "the amount pressed stays drawn")
        #expect(view.fee?.totalWei == "232500000000000000", "the fee pressed stays drawn")
        #expect(view.feeSigningKey == "send.feeSigning")
        let under = try confirm(view)
        #expect(feeRow(under)?.note == loc.t("send.feeSigning"))
        #expect(under.feeMovedNote == nil)

        // The signature's moment: the quote moved — nothing signed, said
        // beside the confirm, the new figures drawn.
        try drive.release()
        #expect(drive.submits.isEmpty, "nothing is signed on a quote that moved")
        view = try drive.view()
        #expect(view.txStatus == "idle")
        #expect(view.feeSigningKey == nil)
        // r21-2: Max sends exactly the figure it shows — the sendable
        // amount cut DOWN to the field's places (`send::max_shown_exact`).
        #expect(view.confirmAmount == "0.005887")
        #expect(view.fee?.totalWei == "294112500000000000")
        #expect(view.feeMoved == SendFeeMovedWire(
            key: "send.feeMovedMax", max: SendMaxMovedWire(from: "0.0675", to: "0.005887")
        ))
        let stopped = try confirm(view)
        #expect(stopped.feeMovedNote == loc.t("send.feeMovedMax", vars: [
            "from": "0.0675", "to": "0.005887", "symbol": "USDC",
        ]))
        #expect(stopped.maxNote == nil, "one sentence for one move")
        #expect(view.canConfirm)

        // The next press signs exactly what is drawn: 0.005887 + 0.2941125.
        try drive.run(["type": "slide_confirm", "review": view.reviewDigest ?? NSNull()])
        let submit = try #require(drive.submits.first)
        let review = try #require(submit["review"] as? [String: Any])
        let transfers = try #require(review["transfers"] as? [[String: Any]])
        #expect(transfers.first?["amount"] as? String == "5887000000000000")
        #expect((review["fee"] as? [String: Any])?["amount"] as? String == "294112500000000000")
    }

    @Test func theStopAndTheFrozenLineAreWordedFromTheCoresKeys() throws {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "confirm"
        object["fee_moved"] = ["key": "send.feeMoved"]
        let plain = try confirm(try CoreJSON.decode(SendViewWire.self, from: object))
        #expect(plain.feeMovedNote == loc.t("send.feeMoved"))
        object["fee_moved"] = NSNull()
        #expect(try confirm(try CoreJSON.decode(SendViewWire.self, from: object)).feeMovedNote == nil)
    }
}
