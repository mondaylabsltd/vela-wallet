//
//  SplitVerdictTests.swift
//  VelaWalletTests
//
//  The split form says the CORE's verdicts (issues 203, 265) and the amount
//  wears its unit (issue 231). Every judgement asserted here is one the shell
//  used to make for itself, or not make at all.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SplitVerdictTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private static let alice = "0x1111111111111111111111111111111111111111"
    private static let bob = "0x2222222222222222222222222222222222222222"

    /// A split on a real core's view, with the verdict fields overridden.
    private func splitView(
        rows: [[String: Any]],
        duplicates: [[String: Any]] = [],
        issues: [[String: Any]] = [],
        over: Bool = false,
        remaining: String? = nil,
        confirmAmount: String = "3",
        estimating: Bool = false,
        amountWarning: [String: Any]? = nil,
        sameAsset: [String: Any]? = nil
    ) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "enter_details"
        object["selected_token"] = [
            "network": "Gnosis", "chain_id": 100, "symbol": "xDAI", "balance": "5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0,
            "logo_urls": [], "spam": false,
        ]
        object["split_mode"] = true
        object["recipients"] = rows
        object["split_duplicates"] = duplicates
        object["split_row_issues"] = issues
        object["split_over_balance"] = over
        object["split_remaining"] = remaining as Any? ?? NSNull()
        object["confirm_amount"] = confirmAmount
        object["estimating_gas"] = estimating
        object["amount_warning"] = amountWarning as Any? ?? NSNull()
        object["same_asset_fee_issue"] = sameAsset as Any? ?? NSNull()
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func row(_ id: String, _ address: String, _ amount: String) -> [String: Any] {
        ["id": id, "address": address, "amount": amount, "name": NSNull()]
    }

    private func drawnSplit() throws -> SendFormModel {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2b, loc: loc).base else {
            throw DecodeFailure()
        }
        return drawn
    }

    private struct DecodeFailure: Error {}

    /// **The core's own JSON decodes** — the field names are the core's, not
    /// a guess. A fresh core is not in a split, so both lists are empty.
    @Test func theCoresVerdictFieldsDecode() throws {
        let view = try CoreJSON.decode(SendViewWire.self, from: CoreJSON.object(SendCore().view()))
        #expect(view.splitDuplicates == [])
        #expect(view.splitRowIssues == [])
        #expect(view.splitRemaining == nil)
    }

    /// **A repeat is the core's call, and says which row it repeats.**
    ///
    /// The shell used to compare addresses itself and call the repeat
    /// "Duplicate — skipped" — while the batch still paid it twice. The first
    /// occurrence is not the mistake and carries nothing.
    @Test func aRepeatedPayeeIsNamedFromTheCore() throws {
        let view = try splitView(
            rows: [row("rcpt_1", Self.alice, "1"), row("rcpt_2", Self.bob, "1"),
                   row("rcpt_3", Self.alice, "1")],
            duplicates: [["id": "rcpt_3", "first_ordinal": 1]]
        )
        let model = SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(model.recipients[0].problem == nil, "the first occurrence is not the mistake")
        #expect(model.recipients[1].problem == nil)
        #expect(model.recipients[2].problem == "Same address as recipient 1")

        // With no verdict from the core, the shell invents none — even for
        // two rows it could see are the same.
        let silent = try splitView(
            rows: [row("rcpt_1", Self.alice, "1"), row("rcpt_2", Self.alice, "1")]
        )
        let quiet = SendLive.form(silent, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(quiet.recipients.allSatisfy { $0.problem == nil })
    }

    /// **A field is wrong only when the core says `invalid`**; an empty one
    /// is unfinished, and the hint above Continue names the first of those.
    @Test func rowIssuesBecomeRowNotesAndTheHint() throws {
        let view = try splitView(
            rows: [row("rcpt_1", "0x12", "1"), row("rcpt_2", Self.bob, "1,5"),
                   row("rcpt_3", Self.alice, "")],
            issues: [
                ["id": "rcpt_1", "ordinal": 1, "address": "invalid", "amount": "ok"],
                ["id": "rcpt_2", "ordinal": 2, "address": "ok", "amount": "invalid"],
                ["id": "rcpt_3", "ordinal": 3, "address": "ok", "amount": "empty"],
            ]
        )
        let model = SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(model.recipients[0].problem == "Invalid address")
        #expect(model.recipients[0].identiconSeed.isEmpty, "no face for a half-typed address")
        #expect(model.recipients[1].problem == "Not a valid amount")
        #expect(model.recipients[1].identiconSeed == Self.bob)
        #expect(model.recipients[2].problem == nil, "empty is unfinished, not wrong")
        #expect(model.hint == "Recipient 1 needs an address.")

        let amountFirst = try splitView(
            rows: [row("rcpt_1", Self.alice, ""), row("rcpt_2", "", "")],
            issues: [
                ["id": "rcpt_1", "ordinal": 1, "address": "ok", "amount": "empty"],
                ["id": "rcpt_2", "ordinal": 2, "address": "empty", "amount": "empty"],
            ]
        )
        let second = SendLive.form(amountFirst, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(second.hint == "Recipient 1 needs an amount.")

        // While the pre-check is out the button is busy, not unfinished.
        let busy = try splitView(
            rows: [row("rcpt_1", Self.alice, "")],
            issues: [["id": "rcpt_1", "ordinal": 1, "address": "ok", "amount": "empty"]],
            estimating: true
        )
        #expect(SendLive.form(busy, fee: nil, display: .usd, on: try drawnSplit(), loc: loc).hint == nil)
    }

    /// **"Use X for the empty rows" (the web's `fillEmpty`)** is offered only
    /// while a row is empty and another carries a figure the core accepts;
    /// the figure is copied exactly as typed, into the empty rows only.
    @Test func theEmptyRowsAreOfferedTheTypedFigure() throws {
        let view = try splitView(
            rows: [row("rcpt_1", Self.alice, "0.50"), row("rcpt_2", Self.bob, "")],
            issues: [["id": "rcpt_2", "ordinal": 2, "address": "ok", "amount": "empty"]]
        )
        let fill = SendLive.form(view, fee: nil, display: .usd, on: try drawnSplit(), loc: loc).fillEmpty
        #expect(fill == FillEmptyModel(label: "Use 0.5 xDAI for the empty rows", amount: "0.50"))

        // No empty row, nothing to fill.
        let full = try splitView(rows: [row("rcpt_1", Self.alice, "0.50"), row("rcpt_2", Self.bob, "1")])
        #expect(SendLive.form(full, fee: nil, display: .usd, on: try drawnSplit(), loc: loc).fillEmpty == nil)

        // A figure the core rejects is never the one that is copied.
        let rejected = try splitView(
            rows: [row("rcpt_1", Self.alice, "1,5"), row("rcpt_2", Self.bob, "")],
            issues: [
                ["id": "rcpt_1", "ordinal": 1, "address": "ok", "amount": "invalid"],
                ["id": "rcpt_2", "ordinal": 2, "address": "ok", "amount": "empty"],
            ]
        )
        #expect(SendLive.form(rejected, fee: nil, display: .usd, on: try drawnSplit(), loc: loc).fillEmpty == nil)

        // The dispatch: only the empty rows take the figure; ids ride along.
        let rows = SplitRows.drafts(from: view)
        let filled = SplitRows.emptyFilled(rows, amount: "0.50")
        #expect(filled[0] == rows[0])
        #expect(filled[1].amount == "0.50" && filled[1].id == "rcpt_2")
    }

    /// **The total says what is left, and says it is over — live.** The
    /// over-balance verdict is the same predicate Continue refuses on; it used
    /// to arrive only as an alert after the refusal. And a split does not take
    /// the single form's `amount_warning`, which judges a figure no longer on
    /// screen.
    @Test func theTotalCarriesRemainingAndOverBalance() throws {
        let under = try splitView(
            rows: [row("rcpt_1", Self.alice, "1"), row("rcpt_2", Self.bob, "2")],
            remaining: "2", confirmAmount: "3",
            amountWarning: ["type": "not_enough_token", "symbol": "xDAI"]
        )
        let model = SendLive.form(under, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(model.summary?.value == "3 xDAI")
        #expect(model.summary?.remaining == "2 xDAI left")
        #expect(model.summary?.over == false)
        #expect(SendLive.formWarning(under, loc: loc) == nil,
                "the single figure's warning does not judge a split")

        let over = try splitView(
            rows: [row("rcpt_1", Self.alice, "4"), row("rcpt_2", Self.bob, "2")],
            over: true, confirmAmount: "6"
        )
        let heavy = SendLive.form(over, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
        #expect(heavy.summary?.over == true)
        #expect(heavy.summary?.remaining == nil)
        #expect(SendLive.formWarning(over, loc: loc) == "The total exceeds your balance.")

        // A total the core cannot sum yet is a dash, not a bare symbol.
        let blank = try splitView(rows: [row("rcpt_1", Self.alice, "")], confirmAmount: "")
        #expect(SendLive.form(blank, fee: nil, display: .usd, on: try drawnSplit(), loc: loc)
            .summary?.value == "—")
    }

    /// **The same-asset ceiling comes first in a split too.** When the coin
    /// being sent also pays the fee, the core measures the ceiling against the
    /// rows' total; its sentence names the most that can be sent, so it wins
    /// over "the total exceeds your balance" — the order all four shells draw.
    @Test func theSameAssetCeilingOutranksTheOverBalanceSentence() throws {
        let rows = [row("rcpt_1", Self.alice, "4"), row("rcpt_2", Self.bob, "2")]
        let base = try splitView(rows: rows, over: true, confirmAmount: "6")
        #expect(SendLive.formWarning(base, loc: loc) == "The total exceeds your balance.")

        let ceiling = try splitView(
            rows: rows, over: true, confirmAmount: "6",
            sameAsset: [
                "symbol": "xDAI", "transfer_amount": "6000000000000000000",
                "balance": "5000000000000000000", "fee_amount": "100000000000000000",
                "total": "6100000000000000000", "max_transfer_amount": "4900000000000000000",
            ]
        )
        let text = try #require(SendLive.formWarning(ceiling, loc: loc))
        #expect(text.contains("4.9 xDAI"), "the most that can be sent: \(text)")
        #expect(text != "The total exceeds your balance.")
    }

    /// **The confirm page repeats the repeat warning** — the page that signs
    /// is where two identical avatars are easiest to miss.
    @Test func theConfirmSaysTheRepeatAgain() throws {
        let view = try splitView(
            rows: [row("rcpt_1", Self.alice, "1"), row("rcpt_2", Self.bob, "1"),
                   row("rcpt_3", Self.alice, "1")],
            duplicates: [["id": "rcpt_3", "first_ordinal": 1]]
        )
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3b, loc: loc).base else {
            Issue.record("sd3b is not a confirm")
            return
        }
        let model = SendLive.confirm(
            view, from: (Self.bob, nil), display: .usd, on: drawn, loc: loc
        )
        #expect(model.repeatNote == "Recipient 3 · Same address as recipient 1")

        let clean = try splitView(rows: [row("rcpt_1", Self.alice, "1"), row("rcpt_2", Self.bob, "1")])
        #expect(SendLive.confirm(clean, from: (Self.bob, nil), display: .usd, on: drawn, loc: loc)
            .repeatNote == nil)
    }

    /// **The amount wears its own unit** (issue 231), keyed on the figure's
    /// code — never the display currency.
    @Test func theAmountWearsTheFiguresOwnUnit() throws {
        #expect(SendLive.unitAdornment(code: nil, symbol: "BNB") == (nil, "BNB"))
        #expect(SendLive.unitAdornment(code: nil, symbol: "") == (nil, nil))
        #expect(SendLive.unitAdornment(code: "USD", symbol: "BNB") == ("$", nil))
        #expect(SendLive.unitAdornment(code: "EUR", symbol: "BNB") == ("€", nil))
        // A code the catalog has no symbol for follows as itself; nothing
        // defaults to "$".
        #expect(SendLive.unitAdornment(code: "XYZ", symbol: "BNB") == (nil, "XYZ"))

        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("sd2 is not a send form")
            return
        }
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "enter_details"
        object["selected_token"] = [
            "network": "BSC", "chain_id": 56, "symbol": "BNB", "balance": "1",
            "decimals": 18, "token_address": NSNull(), "price_usd": 600.0,
            "logo_urls": [], "spam": false,
        ]
        object["amount"] = "0.00075"
        object["token_amount"] = "0.00075"
        let inToken = try CoreJSON.decode(SendViewWire.self, from: object)
        // Displayed in yen: the figure is still in BNB, and says so.
        let yen = WalletLive.Display(code: "JPY", rate: 150)
        let token = SendLive.form(inToken, fee: nil, display: yen, on: drawn, loc: loc)
        #expect(token.amount?.unitSuffix == "BNB")
        #expect(token.amount?.unitPrefix == nil)
        #expect(token.amount?.denomLabel == "BNB", "not the display currency's code")

        object["amount"] = "4"
        object["amount_fiat_code"] = "USD"
        let inUsd = try CoreJSON.decode(SendViewWire.self, from: object)
        let fiat = SendLive.form(inUsd, fee: nil, display: yen, on: drawn, loc: loc)
        #expect(fiat.amount?.unitPrefix == "$", "the figure's own code, not the display's ¥")
        #expect(fiat.amount?.denomLabel == "USD")
    }
}
