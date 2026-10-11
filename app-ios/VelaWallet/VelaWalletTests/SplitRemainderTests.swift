//
//  SplitRemainderTests.swift
//  VelaWalletTests
//
//  "How much is left to give out", as a person reads it (iPhone walk
//  2026-10-10: 「还剩 0.000300194967818323 BNB」 beside 「余额 0.0003」).
//
//  The exact remainder is the core's subtraction and stays exact for whoever
//  counts; what is DRAWN is the core's `split_remaining_display` — the same
//  figure cut to the token ladder, never rounded up — with the person's
//  decimal mark, as the balance beside it. The view here is the REAL send
//  core's, driven to a split over the balance the phone held.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SplitRemainderTests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    private static let alice = "0x1111111111111111111111111111111111111111"
    private static let bob = "0x2222222222222222222222222222222222222222"
    /// What the owner's wallet held on BNB Chain.
    private static let held = "0.000300194967818323"

    /// The real core, opened on a wallet holding `balance` BNB, in a split
    /// with `amounts` typed on its rows.
    private func split(balance: String, amounts: [String]) throws -> SendViewWire {
        let core = SendCore()
        var latest: [String: Any] = [:]
        func run(_ event: [String: Any]) throws {
            var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
            var queue = result["effects"] as? [[String: Any]] ?? []
            latest = result["view"] as? [String: Any] ?? latest
            while !queue.isEmpty {
                let effect = queue.removeFirst()
                guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                      let op = effect["operation"] as? [String: Any],
                      op["type"] as? String == "fetch_tokens" else { continue }
                let reply: [String: Any] = [
                    "type": "tokens_loaded",
                    "tokens": [[
                        "network": "chain-56", "chain_id": 56, "symbol": "BNB", "balance": balance,
                        "decimals": 18, "token_address": NSNull(), "price_usd": 600.0,
                        "logo_urls": [String](), "spam": false,
                    ] as [String: Any]],
                    "chains": [["network": "chain-56", "chain_id": 56, "native_symbol": "BNB"]],
                ]
                result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(reply)))
                latest = result["view"] as? [String: Any] ?? latest
                queue += result["effects"] as? [[String: Any]] ?? []
            }
        }
        try run([
            "type": "open",
            "account": ["id": "cred-0", "address": "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", "name": NSNull()] as [String: Any],
            "params": [
                "preselected_symbol": NSNull(), "preselected_network": NSNull(),
                "prefilled_recipient": NSNull(), "prefilled_chain_id": NSNull(),
                "prefilled_token_address": NSNull(), "prefilled_amount_base": NSNull(),
                "locked": false, "preselected_multi": NSNull(),
            ] as [String: Any],
            "display": ["code": "CNY", "rate": 7.1, "fiat_decimals": 2] as [String: Any],
        ])
        try run(["type": "select_token", "token_id": "chain-56_native_BNB"])
        try run(["type": "enter_split_mode"])
        let addresses = [Self.alice, Self.bob]
        try run(["type": "recipients_changed", "recipients": amounts.enumerated().map { index, amount in
            ["id": "rcpt_\(index + 1)", "address": addresses[index % 2], "amount": amount, "name": NSNull()] as [String: Any]
        }])
        return try CoreJSON.decode(SendViewWire.self, from: latest)
    }

    private func drawnSplit(_ loc: Loc) throws -> SendFormModel {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2b, loc: loc).base else {
            throw Unbuilt()
        }
        return drawn
    }

    private func drawnBatch(_ loc: Loc) throws -> BatchImportModel {
        guard case .batchImport(let model)? = WalletFlowFixtures.build(.sd2c, loc: loc).sheet else {
            throw Unbuilt()
        }
        return model
    }

    private struct Unbuilt: Error {}

    private func batchWire() -> BatchViewWire {
        let base = BatchViewWire.empty
        return BatchViewWire(
            opened: true, unit: base.unit, fiatCode: "CNY", rawText: "",
            fileName: nil, busy: false, fileError: false, templateSaved: false, priced: true,
            rateStatus: .ok, rateInput: "1", rateEdited: false, preview: [],
            overCap: false, rejected: 0, recipientCount: 1, totalToken: "0.0001",
            totalFiat: nil, overBalance: false, canApply: true,
            recipients: [], applied: false
        )
    }

    /// The owner's screen: nothing typed on either row yet, so all of the
    /// balance is left — and it reads as the balance beside it does.
    @Test func theRemainderIsTheCoresCutFigureNotTheExactOne() throws {
        let view = try split(balance: Self.held, amounts: ["", ""])
        #expect(view.splitRemaining == Self.held, "the exact figure is still the core's, for whoever counts")
        #expect(view.splitRemainingDisplay == "0.0003", "and the one a person reads is cut on the ladder")

        let form = SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(zh), loc: zh)
        #expect(form.summary?.remaining == "还剩 0.0003 BNB")
        #expect(form.summary?.remaining?.contains("0.000300194967818323") == false)
        #expect(SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(en), loc: en)
            .summary?.remaining == "0.0003 BNB left")

        // The balance on the same form is the same figure.
        #expect(form.token?.detail.hasSuffix("余额 0.0003") == true, "\(form.token?.detail ?? "no token card")")

        // The importer's sheet says the same remainder.
        let batch = SendLive.batchImport(batchWire(), view: view, on: try drawnBatch(zh), loc: zh)
        #expect(batch.total?.balance == "还剩 0.0003 BNB")
    }

    /// Cut, never rounded up: a remainder a hair under 1.2 must not read as
    /// 1.2 — that would say more is left than is.
    @Test func theRemainderIsNeverRoundedUp() throws {
        let view = try split(balance: "2", amounts: ["0.5", "0.300000000000000001"])
        #expect(view.splitRemaining == "1.199999999999999999")
        #expect(SendLive.remainderText(view) == "1.1999")
        let form = SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(en), loc: en)
        #expect(form.summary?.remaining == "1.1999 BNB left")
    }

    /// The person's decimal mark, as on the balance beside it.
    @Test func theRemainderWearsThePersonsDecimalMark() throws {
        let view = try split(balance: Self.held, amounts: ["", ""])
        let saved = Formats.current
        defer { Formats.current = saved }
        Formats.current = Formats.Current(number: .dotComma, date: .iso, time: .h24)
        #expect(SendLive.remainderText(view) == "0,0003")
        #expect(WalletLive.tokenAmountText(Self.held) == "0,0003", "the balance beside it")
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        #expect(SendLive.remainderText(view) == "0.0003")
    }

    /// Nothing left to say: over the balance, or a row that cannot be summed.
    @Test func noRemainderIsSaidWhenThereIsNone() throws {
        let over = try split(balance: "2", amounts: ["1.5", "0.6"])
        #expect(over.splitRemaining == nil && over.splitRemainingDisplay == nil)
        #expect(SendLive.remainderText(over) == nil)
        #expect(SendLive.form(over, fee: nil, display: .usd, on: try drawnSplit(en), loc: en)
            .summary?.remaining == nil)
    }

    /// A view from a core built before the field: the exact figure is cut by
    /// the core's own rule, not drawn as it came.
    @Test func aViewWithoutTheDisplayFieldIsCutByTheCoresRule() throws {
        let view = try split(balance: Self.held, amounts: ["", ""])
        var object = try CoreJSON.object(SendCore().view())
        object["split_mode"] = true
        object["split_remaining"] = Self.held
        object.removeValue(forKey: "split_remaining_display")
        let old = try CoreJSON.decode(SendViewWire.self, from: object)
        #expect(old.splitRemainingDisplay == nil)
        #expect(SendLive.remainderText(old) == "0.0003")
        #expect(SendLive.remainderText(old) == SendLive.remainderText(view))
        #expect(cutTokenAmount(plain: Self.held) == "0.0003")
    }
}
