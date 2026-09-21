//
//  FeeSpeedTests.swift
//  VelaWalletTests
//
//  Spec 069 on iOS: the two machines behind the speed control, driven through
//  the real core — the stored default (`fee_tier_pref`) and the control's
//  decisions (`fee_speed`). The rules themselves are pinned in
//  `app_fee_speed.rs`; what these pin is that iOS's wire carries them whole,
//  that its executor stores the preference where every client does, and that
//  a submission names its speed on the wire.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct FeeSpeedTests {

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// Optimism-shaped: every tier the same fee, each its own gas bid.
    private func quote(_ tier: String, _ low: String, _ high: String) -> [String: Any] {
        FeeEstimateWire(
            chainId: 10, totalWei: "10000", maxFeePerGas: high, totalGas: "0", deployed: true,
            quoted: true, feeAsset: .native, feeRecipient: "0xfee", tier: tier,
            effectiveGasPrice: low, maxGasPrice: high
        ).coreJSON
    }

    @Test func aSlowerDefaultGoesFastWhereFastCostsNoMore() throws {
        let core = FeeSpeedCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "configure", "preferred": "slow", "number": "comma_dot"]))
        let asked = try CoreJSON.decode(
            FeeSpeedViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string(["type": "stage_changed", "on_form": true])))
        )
        // The partner the core wants priced beside the slower default.
        #expect(asked.previews == ["fast"])
        let upgraded = try CoreJSON.decode(
            FeeSpeedViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string([
                "type": "quotes_changed",
                "chain_id": 10,
                "in_force": ["busy": false, "fee": quote("slow", "1937", "4500")] as [String: Any],
                "previews": [["tier": "fast", "busy": false, "fee": quote("fast", "3244", "9000")] as [String: Any]],
            ])))
        )
        #expect(upgraded.tier == "fast")
        #expect(upgraded.free)
        #expect(upgraded.freeNote)
        #expect(!upgraded.picked)
    }

    @Test func openEveryOptionCarriesItsOwnFeeAndGasBid() throws {
        let core = FeeSpeedCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "configure", "preferred": "fast", "number": "space_comma"]))
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "toggle"]))
        let open = try CoreJSON.decode(
            FeeSpeedViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string([
                "type": "quotes_changed",
                "chain_id": 10,
                "in_force": ["busy": false, "fee": quote("fast", "3244", "9000")] as [String: Any],
                "previews": [
                    ["tier": "standard", "busy": false, "fee": quote("standard", "2377", "6000")] as [String: Any],
                    ["tier": "slow", "busy": false, "fee": quote("slow", "1937", "4500")] as [String: Any],
                ],
            ])))
        )
        #expect(open.open)
        #expect(open.options.map(\.tier) == ["fast", "standard", "slow"])
        #expect(open.options.map(\.gasPrice) == ["3 244 ~ 9 000 wei", "2 377 ~ 6 000 wei", "1 937 ~ 4 500 wei"])
        #expect(open.options[1].fee?.tier == "standard")
        #expect(!open.single)
    }

    /// The estimate goes back to the core whole — a subset would silently drop
    /// the gas bid, and the speed control would have nothing to draw.
    @Test func anEstimateRoundTripsWhole() throws {
        let json = quote("standard", "2377", "6000")
        let decoded = try CoreJSON.decode(FeeEstimateWire.self, from: json)
        #expect(decoded.tier == "standard")
        #expect(decoded.effectiveGasPrice == "2377")
        #expect(decoded.maxGasPrice == "6000")
        // Compared as objects: key order in the encoded text is not stable.
        #expect(NSDictionary(dictionary: decoded.coreJSON).isEqual(to: json))
    }

    @Test func theDefaultSpeedPersistsUnderVelaFeeTier() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = FeeTierExecutor(store: store)
        for operation in FeeTierExecutor.operations {
            let answer = executor.perform(["type": operation, "tier": "slow"])
            #expect(!answer.isEmpty)
        }
        #expect(store.readString(VelaStore.Key.feeTier) == "slow")
        let read = try CoreJSON.object(executor.perform(["type": "read_stored_tier"]))
        #expect(read["raw"] as? String == "slow")
        let fresh = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let absent = try CoreJSON.object(FeeTierExecutor(store: fresh).perform(["type": "read_stored_tier"]))
        #expect(absent["raw"] is NSNull, "absent means never chose")
    }

    @Test func theDefaultSpeedMachineDecodesAndReadsBack() throws {
        let core = FeeTierPrefCore()
        let initial = try CoreJSON.decode(FeeTierPrefViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.tier == "fast")
        #expect(!initial.committed)
        #expect(initial.offered == ["fast", "standard", "slow"])
    }

    /// The speed the displayed fee was priced at is the third parameter, by
    /// name; no speed is the two-element wire; `rapid` is never sent.
    @Test func aSubmissionNamesItsSpeedAsTheThirdParameter() {
        let op: [String: Any] = ["sender": "0x1"]
        #expect(RelayClient.submitParams(op, tier: "slow").count == 3)
        #expect(RelayClient.submitParams(op, tier: "slow").last as? String == "slow")
        #expect(RelayClient.submitParams(op, tier: nil).count == 2)
        #expect(RelayClient.submitParams(op, tier: "rapid").count == 2)
    }
}
