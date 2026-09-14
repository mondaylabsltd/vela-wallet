//
//  BatchImportTests.swift
//  VelaWalletTests
//
//  The payroll importer, driven through the REAL `batch_import` core.
//
//  Three things this file is for, and only the first is about decoding:
//
//  1. **Drift** — `BatchViewWire` is hand-written, `ts-rs` has no Swift
//     backend, and nothing else notices a renamed field in `vela-core`.
//  2. **The seam to the send machine** — `apply()` hands over the CORE's rows.
//     A shell that re-parsed, re-priced or re-filtered on the way across would
//     pay somebody the preview never showed.
//  3. **The one gate** — `canApply`. Every screen and every intent reads that
//     single flag rather than assembling its own conjunction, because a second
//     opinion about whether a payroll may be sent eventually disagrees with
//     the first.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BatchImportTests {

    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    private func effects(from dispatchResult: String) throws -> [[String: Any]] {
        try CoreJSON.object(dispatchResult)["effects"] as? [[String: Any]] ?? []
    }

    private func tags(_ effects: [[String: Any]]) -> [String] {
        effects.compactMap { ($0["operation"] as? [String: Any])?["type"] as? String }
    }

    /// A send view holding one USDC. The importer reads exactly one field of
    /// it — the selected token's symbol — so the rest is the core's own
    /// starting view rather than a second invented one.
    private func sendView() -> SendViewWire {
        let core = SendCore()
        var object = try! CoreJSON.object(core.view())
        object["selected_token"] = [
            "network": "gnosis", "chain_id": 100, "symbol": "USDC", "balance": "5000",
            "decimals": 6, "token_address": "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83",
            "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func openEvent(currency: String = "USD", priceUsd: Double? = 1) -> [String: Any] {
        [
            "type": "open",
            "token": [
                "symbol": "USDC", "decimals": 6, "balance": "5000",
                "price_usd": priceUsd.map { $0 as Any } ?? NSNull(),
            ],
            "currency_code": currency,
            "max_recipients": BatchStore.maxRecipients,
        ]
    }

    /// Open the sheet AND answer the rate it asks for.
    ///
    /// The unit starts at `fiat` — the payroll case — so every amount is a
    /// fiat figure awaiting a multiplier, and until one lands the machine
    /// prices nothing and applies nothing. A test that skipped this would be
    /// reading a machine that is still waiting.
    @discardableResult
    private func priced(_ core: BatchImportCore, currency: String = "USD", rate: Double? = 1)
        throws -> BatchViewWire
    {
        let opened = try core.dispatch(eventJson: CoreJSON.string(openEvent(currency: currency)))
        guard let ask = try effects(from: opened).first(where: {
            ($0["operation"] as? [String: Any])?["type"] as? String == "fetch_usd_fiat_rate"
        }), let id = (ask["id"] as? NSNumber)?.uint64Value else {
            return try CoreJSON.decode(BatchViewWire.self, from: try view(from: opened))
        }
        let answered = try core.resolveEffect(effectId: id, resultJson: CoreJSON.string([
            "type": "rate_resolved", "code": currency,
            "rate": rate.map { $0 as Any } ?? NSNull(),
        ]))
        return try CoreJSON.decode(BatchViewWire.self, from: try view(from: answered))
    }

    // MARK: - Drift

    @Test func viewDecodes() throws {
        let core = BatchImportCore()
        let initial = try CoreJSON.decode(BatchViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.opened)
        #expect(!initial.canApply, "nothing is applicable before the sheet is open")

        let opened = try core.dispatch(eventJson: CoreJSON.string(openEvent()))
        let after = try CoreJSON.decode(BatchViewWire.self, from: try view(from: opened))
        #expect(after.opened)
        #expect(after.fiatCode == "USD")
        #expect(after.preview.isEmpty)
    }

    /// Every operation this machine can ask for is one the executor handles.
    @Test func asksOnlyWhatTheExecutorAnswers() throws {
        let core = BatchImportCore()
        var asked = tags(try effects(from: core.dispatch(eventJson: CoreJSON.string(
            openEvent(currency: "CNY")
        ))))
        for event in [["type": "pick_file_requested"], ["type": "save_template_requested"]] {
            asked += tags(try effects(from: core.dispatch(eventJson: CoreJSON.string(event))))
        }
        for tag in Set(asked) {
            #expect(BatchExecutor.operations.contains(tag), "unhandled operation \(tag)")
        }
        #expect(asked.contains("fetch_usd_fiat_rate"), "a non-USD currency has to be priced")
    }

    // MARK: - The seam

    /// Pasting two lines and applying hands the send machine exactly the rows
    /// the preview showed — same addresses, the core's own converted amounts,
    /// and ids left empty for the core to mint.
    @Test func applyHandsOverTheCoresOwnRows() throws {
        let core = BatchImportCore()
        try priced(core)
        let pasted = try core.dispatch(eventJson: CoreJSON.string([
            "type": "set_raw_text",
            "text": "0x1111111111111111111111111111111111111111,10\n0x2222222222222222222222222222222222222222,20",
        ]))
        let parsed = try CoreJSON.decode(BatchViewWire.self, from: try view(from: pasted))
        #expect(parsed.preview.count == 2)
        #expect(parsed.preview.allSatisfy { $0.ok })
        #expect(parsed.recipientCount == 2)
        #expect(parsed.canApply)
        #expect(parsed.recipients.map(\.address) == parsed.preview.map(\.address))
        // The amounts crossing the seam are the CONVERTED ones, not the raw
        // column: at unit `fiat` and a USD price of 1 those coincide, and the
        // point is which field is read.
        #expect(parsed.recipients.map(\.amount) == parsed.preview.map(\.tokenAmount))
    }

    /// A duplicate is skipped and counted, and the rejected count is what the
    /// screen reports — the shell never counts rows itself.
    @Test func duplicatesAreTheCoresVerdict() throws {
        let core = BatchImportCore()
        try priced(core)
        let pasted = try core.dispatch(eventJson: CoreJSON.string([
            "type": "set_raw_text",
            "text": """
            0x1111111111111111111111111111111111111111,10
            0x1111111111111111111111111111111111111111,20
            not-an-address,30
            """,
        ]))
        let parsed = try CoreJSON.decode(BatchViewWire.self, from: try view(from: pasted))
        // A line that does not parse into an address and an amount at all is
        // COUNTED but not previewed — there is nothing to show a verdict
        // beside. Only lines that made it into a pair get a row.
        #expect(parsed.preview.count == 2)
        #expect(parsed.rejected == 2, "one duplicate and one unparseable line")
        #expect(parsed.recipientCount == 1)
        #expect(parsed.preview.contains { $0.dup })
    }

    /// An unpriceable currency: the rate fetch answered `nil`, and the machine
    /// refuses to apply rather than quoting a rate of 1.
    ///
    /// This is the defect the executor's rate rule exists to prevent — 5,000 of
    /// a currency worth a fraction of a dollar sent as 5,000 tokens.
    @Test func anUnpriceableCurrencyCannotBeApplied() throws {
        let core = BatchImportCore()
        try priced(core, currency: "VND", rate: nil)
        let pasted = try core.dispatch(eventJson: CoreJSON.string([
            "type": "set_raw_text",
            "text": "0x1111111111111111111111111111111111111111,10",
        ]))
        let parsed = try CoreJSON.decode(BatchViewWire.self, from: try view(from: pasted))
        #expect(parsed.rateStatus == .failed)
        #expect(!parsed.canApply, "no rate, no payroll")
    }

    // MARK: - The screen

    /// The built model says what the core decided, and says it in the corpus's
    /// words. Checked on the three that are easy to get wrong: the CTA counts
    /// people, the note colour separates a refusal from a remark, and the rate
    /// line shows the typed string rather than a reformatted one.
    @Test func theScreenRepeatsTheCore() throws {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let model = sheetModel(loc: loc)

        let core = BatchImportCore()
        try priced(core)
        // A duplicate rather than a malformed line, because a malformed one is
        // counted without being previewed and this test is about the rows.
        let pasted = try core.dispatch(eventJson: CoreJSON.string([
            "type": "set_raw_text",
            "text": "0x1111111111111111111111111111111111111111,10\n0x1111111111111111111111111111111111111111,30",
        ]))
        let wire = try CoreJSON.decode(BatchViewWire.self, from: try view(from: pasted))

        let built = SendLive.batchImport(wire, view: sendView(), on: model, loc: loc)
        #expect(built.rows.count == 2)
        #expect(built.rows.map { $0.ok } == [true, false])
        #expect(built.cta == loc.t("send.batchApply_one", vars: ["count": "1"]),
                "one row survived, so the button offers one — never two")
        #expect(!built.ctaDisabled)
        #expect(built.rejectedText != nil)
        #expect(built.note == nil, "a rejected row is not a note")
        // Untouched by the builder: the sheet's own furniture stays drawn.
        #expect(built.title == model.title)
        #expect(built.pastePlaceholder == model.pastePlaceholder)
    }

    /// Over the balance is an ERROR, over the cap is not. The desktop colours
    /// the same two facts the same way: one cannot be paid, the other will
    /// simply be trimmed.
    @Test func theNoteSeparatesRefusalFromRemark() throws {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let base = BatchViewWire.empty

        func built(_ mutate: (inout BatchViewWire) -> Void) -> BatchImportModel {
            var wire = base
            mutate(&wire)
            return SendLive.batchImport(
                wire, view: sendView(), on: sheetModel(loc: loc), loc: loc
            )
        }
        #expect(built { $0 = overBalance(base) }.noteIsError)
        #expect(built { $0 = overCap(base) }.noteIsError == false)
        #expect(built { $0 = overCap(base) }.note == loc.t(
            "send.batchOverCap", vars: ["n": String(BatchStore.maxRecipients)]
        ))
        // A file that could not be read outranks everything below it: the rows
        // on screen are from the previous paste, and saying "over the cap"
        // about them would answer a question nobody asked.
        var both = overBalance(base)
        both = BatchViewWire(
            opened: both.opened, unit: both.unit, fiatCode: both.fiatCode, rawText: both.rawText,
            fileName: both.fileName, busy: both.busy, fileError: true,
            templateSaved: both.templateSaved, priced: both.priced, rateStatus: both.rateStatus,
            rateInput: both.rateInput, rateEdited: both.rateEdited, preview: both.preview,
            overCap: both.overCap, rejected: both.rejected, recipientCount: both.recipientCount,
            totalToken: both.totalToken, totalFiat: both.totalFiat, overBalance: both.overBalance,
            canApply: both.canApply, recipients: both.recipients, applied: both.applied
        )
        let model = SendLive.batchImport(
            both, view: sendView(), on: sheetModel(loc: loc), loc: loc
        )
        #expect(model.note?.contains(loc.t("send.batchImportFailedBody")) == true)
    }

    private func sheetModel(loc: Loc) -> BatchImportModel {
        guard case .batchImport(let model)? = WalletFlowFixtures.build(.sd2c, loc: loc).sheet
        else { fatalError("SD2c does not draw the importer") }
        return model
    }

    private func overBalance(_ base: BatchViewWire) -> BatchViewWire {
        BatchViewWire(
            opened: true, unit: base.unit, fiatCode: base.fiatCode, rawText: base.rawText,
            fileName: nil, busy: false, fileError: false, templateSaved: false, priced: false,
            rateStatus: .ok, rateInput: base.rateInput, rateEdited: false, preview: [],
            overCap: false, rejected: 0, recipientCount: 0, totalToken: "0", totalFiat: nil,
            overBalance: true, canApply: false, recipients: [], applied: false
        )
    }

    private func overCap(_ base: BatchViewWire) -> BatchViewWire {
        BatchViewWire(
            opened: true, unit: base.unit, fiatCode: base.fiatCode, rawText: base.rawText,
            fileName: nil, busy: false, fileError: false, templateSaved: false, priced: false,
            rateStatus: .ok, rateInput: base.rateInput, rateEdited: false, preview: [],
            overCap: true, rejected: 0, recipientCount: 0, totalToken: "0", totalFiat: nil,
            overBalance: false, canApply: false, recipients: [], applied: false
        )
    }
}

// MARK: - The confirm page's notice (spec 054 US4)

/// What the confirm page says when it cannot go on, and which way out it
/// offers. Three facts stop that page and each used to stop it silently: the
/// CTA simply went inert and nothing on screen said why.
@MainActor
struct ConfirmNoticeTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func model() -> SendConfirmModel {
        guard case .sendConfirm(let confirm) = WalletFlowFixtures.build(.sd3, loc: loc).base
        else { fatalError("SD3 does not draw the confirm page") }
        return confirm
    }

    /// A send view with the treasury pause raised, or an error, or a signature
    /// under way — built from the core's own starting view so no field is
    /// invented.
    private func view(_ patch: [String: Any]) -> SendViewWire {
        var object = try! CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private func built(_ view: SendViewWire) -> SendConfirmModel {
        SendLive.confirm(
            view,
            from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd,
            on: model(), loc: loc
        )
    }

    /// The relayer is empty. Two exits: fund and retry, or 暂不 — and 暂不 is
    /// the one this page never had. Without it the only way past a depleted
    /// relayer was to leave the send entirely and start again.
    @Test func theTreasuryPauseOffersBothExits() {
        let confirm = built(view([
            "treasury_bootstrap": [
                "chain_id": 100,
                "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                "asset": "native", "balance": "0", "floor": "1000000000000000000",
                "bootstrap_needed": true,
            ],
        ]))
        #expect(confirm.notice?.contains(loc.t("componentsUi.treasuryBootstrap.title")) == true)
        #expect(confirm.noticeAction == loc.t("componentsUi.treasuryBootstrap.retryBtn"))
        #expect(confirm.noticeSecondary == loc.t("componentsUi.funding.cancel"))
    }

    /// A submit the relay refused has ONE exit — retrying it. Offering 暂不
    /// here would suggest the attempt is still alive, and it is not.
    @Test func aRefusedSubmitOffersOnlyRetry() {
        let confirm = built(view(["tx_error": "generic"]))
        #expect(confirm.notice == loc.t("send.txErrorGeneric"))
        #expect(confirm.noticeAction == loc.t("send.txRetryBtn"))
        #expect(confirm.noticeSecondary == nil)
    }

    /// While the passkey prompt is up the page stays on confirm, and the one
    /// honest button under it is cancel — the core's own checkpoint, not a
    /// second signature.
    @Test func aPromptThatIsUpOffersCancel() {
        let confirm = built(view(["tx_status": "signing"]))
        #expect(confirm.notice == loc.t("send.txPreparingBiometric"))
        #expect(confirm.noticeAction == loc.t("componentsUi.funding.cancel"))
        #expect(confirm.noticeSecondary == nil)
    }

    /// Nothing wrong, nothing said. A page that always carries a banner
    /// teaches people to ignore it.
    @Test func aQuietPageSaysNothing() {
        let confirm = built(view([:]))
        #expect(confirm.notice == nil)
        #expect(confirm.noticeAction == nil)
        #expect(confirm.noticeSecondary == nil)
    }
}
