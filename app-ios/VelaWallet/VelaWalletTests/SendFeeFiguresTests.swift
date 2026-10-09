//
//  SendFeeFiguresTests.swift
//  VelaWalletTests
//
//  The send confirm's fee row read "~0.000173…B · ≈¥0.85" (iPhone pass
//  2026-10-09) — one line, truncated in the middle of the coin's figure and
//  its unit.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendFeeFiguresTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - The confirm's fee row

    /// The row may take a second line, and the only place it may break is
    /// after the ` · ` between the coin's figure and its price: every space
    /// inside a part is a no-break space.
    @Test func theFeeBreaksOnlyBetweenTheFigureAndItsPrice() {
        let drawn = FactRowView.unbreakable("~0.000173 BNB · ≈¥0.85")
        #expect(drawn == "~0.000173\u{00A0}BNB\u{00A0}· ≈¥0.85")
        #expect(drawn.filter { $0 == " " }.count == 1, "one break opportunity, after the dot")
        // It reads the same.
        #expect(drawn.replacingOccurrences(of: "\u{00A0}", with: " ") == "~0.000173 BNB · ≈¥0.85")
        // A fee with no price is one unbreakable figure and its unit.
        #expect(FactRowView.unbreakable("~0.000173 BNB") == "~0.000173\u{00A0}BNB")
    }

    @Test func theConfirmsFeeRowWrapsAndItsOtherRowsDoNot() throws {
        var object = try CoreJSON.object(SendCore().view())
        object["recipient"] = "0x1111111111111111111111111111111111111111"
        let view = try CoreJSON.decode(SendViewWire.self, from: object)
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            Issue.record("SD3 draws no confirm")
            return
        }
        let model = SendLive.confirm(
            view, from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: "Parallel One"),
            display: .usd, on: drawn, loc: loc
        )
        let fee = try #require(model.facts.first { $0.label == loc.t("send.estFeeLabel") })
        #expect(fee.wraps, "the fee row is cut to one line again")
        #expect(model.facts.filter(\.wraps).count == 1, "only the fee row wraps")
    }
}
