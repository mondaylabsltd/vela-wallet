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
import SwiftUI
import Testing
import UIKit
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
        // The factory default is `standard` since the Ethereum fee fix.
        #expect(initial.tier == "standard")
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

    // MARK: - The per-payment picker draws no descriptions, and never jumps

    private func speedModel(bids: [String?], values: [String]) -> FeeSpeedModel {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let tiers = ["fast", "standard", "slow"]
        return FeeSpeedModel(
            label: loc.t("send.feeSpeedLabel"), value: SendLive.tierName("fast", loc: loc), open: true,
            onceNote: loc.t("send.feeSpeedOnce"), freeNote: nil, singleNote: nil,
            gasPriceLabel: loc.t("send.gasPriceLabel"), gasPriceLine: true,
            options: tiers.indices.map { index in
                FeeSpeedOptionModel(
                    id: tiers[index], label: SendLive.tierName(tiers[index], loc: loc),
                    value: values[index], gasPrice: bids[index], selected: index == 0
                )
            }
        )
    }

    private func height(_ model: FeeSpeedModel, width: CGFloat = 343) -> CGFloat {
        let host = UIHostingController(rootView: FeeSpeedControlView(speed: model).themed(.light))
        return host.sizeThatFits(in: CGSize(width: width, height: .greatestFiniteMagnitude)).height
    }

    /// The speeds are a name, a price and — on a chain with an honest bid —
    /// the Gas Bid under the price. A tier still measuring has no bid yet, and
    /// its second line keeps its height: picking a speed or refreshing re-prices
    /// every option, and a row that shrank and grew on each one jumped under
    /// the finger (the tier description used to hold that line).
    @Test func aMeasuringSpeedKeepsItsBidLinesHeight() {
        let priced = speedModel(bids: ["0.012 – 0.020 Gwei", "0.010 Gwei", "0.008 Gwei"],
                                values: ["~0.0021 ETH ≈ $5.40", "~0.0019 ETH ≈ $4.90", "~0.0017 ETH ≈ $4.40"])
        let measuring = speedModel(bids: ["0.012 – 0.020 Gwei", nil, nil],
                                   values: ["~0.0021 ETH ≈ $5.40", "…", "…"])
        let pricedHeight = height(priced)
        #expect(pricedHeight > 0)
        #expect(height(measuring) == pricedHeight, "the picker changed height while a tier measured")

        // A chain with no honest bid draws no second line at all — one line
        // per speed, also steady.
        let bare = FeeSpeedModel(
            label: priced.label, value: priced.value, open: true, onceNote: priced.onceNote,
            freeNote: nil, singleNote: nil, gasPriceLabel: priced.gasPriceLabel, gasPriceLine: false,
            options: priced.options
        )
        #expect(height(bare) < pricedHeight)
    }
}
