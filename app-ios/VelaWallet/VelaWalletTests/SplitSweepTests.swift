//
//  SplitSweepTests.swift
//  VelaWalletTests
//
//  The two pure pieces the split and sweep screens rest on, and the batch
//  executor's three rules.
//

import Foundation
import Testing
import UniformTypeIdentifiers
import VelaCore
@testable import VelaWallet

// MARK: - Editing a list of recipients

struct SplitRowsTests {

    private func rows() -> [SplitRows.Draft] {
        [
            SplitRows.Draft(id: "rcpt_0", address: "0xaaa", amount: "1", name: "Alice"),
            SplitRows.Draft(id: "rcpt_1", address: "0xbbb", amount: "2", name: "Bob"),
        ]
    }

    /// **An untouched row comes back unchanged**, name and all.
    ///
    /// The core resolved that name against that address; rewriting the row
    /// throws it away and makes the core resolve it again — which on a slow
    /// network means a name that blinks out while somebody is reading it.
    @Test func editingOneRowLeavesTheOthersExactlyAsTheyWere() {
        let edited = SplitRows.amountEdited(rows(), id: "rcpt_0", amount: "5")
        #expect(edited[0].amount == "5")
        #expect(edited[0].name == "Alice", "an amount edit must not disturb the name")
        #expect(edited[1] == rows()[1], "the other row is untouched")

        // The same value is not an edit.
        #expect(SplitRows.amountEdited(rows(), id: "rcpt_0", amount: "1") == rows())
        // An id nobody has changes nothing.
        #expect(SplitRows.amountEdited(rows(), id: "rcpt_9", amount: "5") == rows())
    }

    /// **A share is cleaned by the core's amount rule** (spec 073).
    ///
    /// A decimal-comma pad's comma reached the core raw, and "1,5" was
    /// refused only when the call was built. The cases below read the same
    /// under every number preset, so the global preset is left alone.
    @Test func aSharesEditRunsTheCoresAmountRule() {
        // One typed comma into a figure with no point is the decimal mark.
        #expect(SplitRows.amountEdited(rows(), id: "rcpt_0", amount: "1,")[0].amount == "1.")
        // A paste with no reading as one figure changes nothing — 1.57 is
        // not what "1.5e-7" meant.
        #expect(SplitRows.amountEdited(rows(), id: "rcpt_0", amount: "1.5e-7") == rows())
        // A pasted grouped figure is read for what it is.
        #expect(SplitRows.amountEdited(rows(), id: "rcpt_1", amount: "1.234,56")[1].amount == "1234.56")
    }

    /// **A new address means the name is gone.**
    ///
    /// The name belonged to the old address. Keeping it puts somebody's name
    /// above a stranger's address, which is the most expensive mislabelling
    /// this screen can produce.
    @Test func changingAnAddressClearsThatRowsName() {
        let edited = SplitRows.addressEdited(rows(), id: "rcpt_0", address: "0xccc")
        #expect(edited[0].address == "0xccc")
        #expect(edited[0].name == nil)
        #expect(edited[1].name == "Bob", "and only that row's")

        // Re-typing the same address is not a change, so the name stays.
        #expect(SplitRows.addressEdited(rows(), id: "rcpt_0", address: "0xaaa")[0].name == "Alice")
    }

    /// **A new row carries no id.** The core mints `rcpt_{n}`; a shell that
    /// invented one would collide with it.
    @Test func anAppendedRowHasNoIdOfItsOwn() {
        let grown = SplitRows.appended(rows())
        #expect(grown.count == 3)
        #expect(grown.last?.id.isEmpty == true)
        #expect(grown.last?.address.isEmpty == true)
        #expect(grown.last?.json["name"] is NSNull)
    }

    @Test func removingTakesExactlyOneRow() {
        #expect(SplitRows.removed(rows(), id: "rcpt_0").map(\.id) == ["rcpt_1"])
        #expect(SplitRows.removed(rows(), id: "rcpt_9") == rows())
    }
}

// MARK: - The sweep picker

@MainActor
struct SweepPickTests {

    private func view(
        selected: [String] = [], valuable: [String] = [], chain: Int? = nil
    ) -> SendViewWire {
        SendPickFixture.view(selected: selected, valuable: valuable, chain: chain)
    }

    private func tags(_ events: [[String: Any]]) -> [String] {
        events.compactMap { $0["type"] as? String }
    }

    /// The first tick pins the chain, and pins it **before** ticking — the
    /// core reads the pin when it decides whether the tick is allowed.
    @Test func theFirstTickPinsTheChain() {
        let events = SweepPick.tap(view: view(), tokenId: "t1", chainId: 100)
        #expect(tags(events) == ["set_multi_network", "toggle_multi_token"])
        #expect((events.first?["chain_id"] as? NSNumber)?.intValue == 100)
    }

    /// **Unticking the last ticked token releases the chain.**
    ///
    /// Without this, somebody who ticks the wrong token first is pinned to that
    /// chain until they leave the screen — the wallet having quietly decided
    /// something they did not mean to.
    @Test func untickingTheLastOneReleasesTheChain() {
        let events = SweepPick.tap(
            view: view(selected: ["t1"], chain: 100), tokenId: "t1", chainId: 100
        )
        #expect(tags(events) == ["toggle_multi_token", "set_multi_network"])
        #expect(events.last?["chain_id"] is NSNull)

        // With two ticked, unticking one is just a toggle.
        let two = SweepPick.tap(
            view: view(selected: ["t1", "t2"], chain: 100), tokenId: "t1", chainId: 100
        )
        #expect(tags(two) == ["toggle_multi_token"])
    }

    /// A row on another chain answers **nothing**, and is drawn dimmed — a
    /// tappable row is an invitation the wallet will not honour.
    @Test func aRowOnAnotherChainIsNotAnAnswer() {
        let events = SweepPick.tap(view: view(chain: 100), tokenId: "t9", chainId: 1)
        #expect(events.isEmpty)
        #expect(SweepPick.dimmed(view: view(chain: 100), chainId: 1))
        #expect(!SweepPick.dimmed(view: view(chain: 100), chainId: 100))
        #expect(!SweepPick.dimmed(view: view(), chainId: 1), "nothing pinned dims nothing")
    }

    /// "Select all valuable" is scoped to what is **visible**, and when nothing
    /// is pinned the first VALUABLE visible row decides the chain — not merely
    /// the first visible one.
    @Test func selectAllIsScopedToTheVisibleRowsAndPinsFromAValuableOne() {
        let unpinned = SweepPick.selectAll(
            view: view(valuable: ["t2"]), visibleIds: ["t1", "t2"],
            chainOf: { $0 == "t2" ? 100 : 1 }
        )
        #expect(tags(unpinned) == ["set_multi_network", "toggle_all_multi_tokens"])
        #expect((unpinned.first?["chain_id"] as? NSNumber)?.intValue == 100,
                "t1 is visible but worthless; the chain comes from t2")
        #expect(unpinned.last?["visible_ids"] as? [String] == ["t1", "t2"])

        let pinned = SweepPick.selectAll(
            view: view(chain: 100), visibleIds: ["t1"], chainOf: { _ in 100 }
        )
        #expect(tags(pinned) == ["toggle_all_multi_tokens"])

        // Nothing visible is worth sweeping: no events, rather than a pin on
        // a chain nobody chose.
        #expect(SweepPick.selectAll(
            view: view(), visibleIds: ["t1"], chainOf: { _ in 1 }
        ).isEmpty)
    }
}

// MARK: - The batch executor

@MainActor
struct BatchExecutorTests {

    /// A document layer that answers whatever it was built with.
    final class ScriptedDocuments: DocumentPorts {
        var picked: PickedDocument?
        var created = true
        private(set) var createdNames: [String] = []

        func pick(types: [UTType]) async -> PickedDocument? { picked }
        func create(name: String, type: UTType, bytes: Data) async -> Bool {
            createdNames.append(name)
            return created
        }
        func share(name: String, type: UTType, bytes: Data) async -> Bool { true }
    }

    private func executor(
        rate: @escaping (String) async -> Double?, documents: DocumentPorts?
    ) -> BatchExecutor {
        BatchExecutor(fiatRate: rate, documents: { documents })
    }

    /// **The rate is never a defaulted 1.**
    ///
    /// The display currency's own resolver falls back to 1 so a screen always
    /// has something to print. That fallback arriving here would read as "the
    /// rate really is 1", and somebody's 5,000 of a currency worth a fraction
    /// of a dollar would be sent as 5,000 tokens.
    @Test func anUnpriceableCurrencyAnswersUnknownRatherThanOne() async {
        for answer in [nil, Double.nan, Double.infinity, 0, -1] as [Double?] {
            let json = try! CoreJSON.object(
                await executor(rate: { _ in answer }, documents: nil)
                    .perform(["type": "fetch_usd_fiat_rate", "code": "VND"])
            )
            #expect(json["rate"] is NSNull, "\(String(describing: answer)) must not become a rate")
        }
        let good = try! CoreJSON.object(
            await executor(rate: { _ in 24_500 }, documents: nil)
                .perform(["type": "fetch_usd_fiat_rate", "code": "VND"])
        )
        #expect((good["rate"] as? NSNumber)?.doubleValue == 24_500)
    }

    /// **The extension decides**, not the type the system guessed: a CSV
    /// exported from Excel is routinely typed `application/vnd.ms-excel`.
    @Test func theFilesExtensionDecidesTextOrMatrix() async {
        let documents = ScriptedDocuments()

        documents.picked = PickedDocument(name: "people.csv", bytes: Data("a,b\n1,2".utf8))
        let text = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: documents).perform(["type": "pick_file"])
        )
        #expect(text["type"] as? String == "file_picked")
        #expect((text["content"] as? [String: Any])?["type"] as? String == "text")

        let workbook = try! Data(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("app-desktop/vela-wallet/tests/fixtures/payroll-sample.xlsx"))
        documents.picked = PickedDocument(name: "payroll.XLSX", bytes: workbook)
        let matrix = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: documents).perform(["type": "pick_file"])
        )
        let content = matrix["content"] as? [String: Any]
        #expect(content?["type"] as? String == "matrix", "an uppercase extension is the same extension")
        #expect((content?["rows"] as? [[String]])?.count == 4)
    }

    /// A workbook this build cannot read is a **file** error, not an empty
    /// list of people.
    @Test func anUnreadableWorkbookIsAFileErrorNotAnEmptyList() async {
        let documents = ScriptedDocuments()
        documents.picked = PickedDocument(name: "broken.xlsx", bytes: Data("not a zip".utf8))
        let json = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: documents).perform(["type": "pick_file"])
        )
        #expect(json["type"] as? String == "file_pick_failed")
    }

    /// **A missing document layer is a failure, not a cancel.** A cancel means
    /// somebody changed their mind; here nobody was asked anything.
    @Test func noDocumentLayerIsAFailureNotACancel() async {
        let json = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: nil).perform(["type": "pick_file"])
        )
        #expect(json["type"] as? String == "file_pick_failed")

        let cancelled = ScriptedDocuments()
        cancelled.picked = nil
        let backedOut = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: cancelled).perform(["type": "pick_file"])
        )
        #expect(backedOut["type"] as? String == "file_pick_cancelled")
    }

    @Test func theTemplateIsSavedUnderTheCoresName() async {
        let documents = ScriptedDocuments()
        let saved = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: documents).perform([
                "type": "save_template_file", "name": "vela-template.csv",
                "contents": "address,amount\n", "mime": "text/csv",
            ])
        )
        #expect(saved["type"] as? String == "template_saved")
        #expect(documents.createdNames == ["vela-template.csv"])

        documents.created = false
        let failed = try! CoreJSON.object(
            await executor(rate: { _ in 1 }, documents: documents).perform([
                "type": "save_template_file", "name": "t.csv", "contents": "", "mime": "text/csv",
            ])
        )
        #expect(failed["type"] as? String == "template_save_failed")
    }

    /// Every arm has a neutral twin, so a shell fault never leaves the machine
    /// waiting.
    @Test func everyArmHasANeutralAnswer() {
        for tag in BatchExecutor.operations {
            let json = try! CoreJSON.object(BatchExecutor.neutralAnswer(["type": tag, "code": "USD"]))
            #expect(!(json["type"] as? String ?? "").isEmpty, "\(tag) has no neutral answer")
        }
    }
}

/// A `SendViewWire` with only the sweep fields set — the rest is whatever a
/// fresh machine publishes, decoded from the real core so the fixture cannot
/// drift from the wire.
@MainActor
enum SendPickFixture {
    static func view(selected: [String], valuable: [String], chain: Int?) -> SendViewWire {
        let core = SendCore()
        var object = try! CoreJSON.object(core.view())
        object["multi_selected_ids"] = selected
        object["multi_valuable_ids"] = valuable
        object["multi_chain_id"] = chain as Any? ?? NSNull()
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }
}

// MARK: - What the sweep form promises

@MainActor
struct SweepFormTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// **A sweep row shows what MOVES, never the balance.**
    ///
    /// A sweep is not "the whole balance": the core reserves what the fee needs
    /// on the asset that pays it, so a row that showed 0.5 in the picker sends
    /// slightly less. And when the core has not worked a row out yet the row
    /// shows **nothing** rather than the balance — a figure the operation does
    /// not carry is worse than no figure.
    @Test func aSweepRowReadsTheCoresSpecAndNeverTheBalance() throws {
        let core = SendCore()
        var object = try CoreJSON.object(core.view())
        object["multi_select_mode"] = true
        // The core's own spelling: `network_address_symbol`. Inventing a second
        // one is how a tick never matches its own token.
        object["multi_selected_ids"] = ["Gnosis_native_xDAI", "Gnosis_0xtoken_USDC"]
        object["multi_chain_id"] = 100
        object["tokens"] = [
            [
                "symbol": "xDAI", "network": "Gnosis", "chain_id": 100,
                "token_address": NSNull(), "decimals": 18, "balance": "0.50067",
                "price_usd": 1.0, "logo_urls": [], "spam": false,
            ],
            [
                "symbol": "USDC", "network": "Gnosis", "chain_id": 100,
                "token_address": "0xtoken", "decimals": 6, "balance": "12.5",
                "price_usd": 1.0, "logo_urls": [], "spam": false,
            ],
        ]
        // Only the native one has been worked out.
        object["multi_specs"] = [
            ["token_address": NSNull(), "decimals": 18, "amount": "0.49067"],
        ]
        let view = try CoreJSON.decode(SendViewWire.self, from: object)

        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2d, loc: loc).base else {
            Issue.record("sd2d is not a send form")
            return
        }
        let model = SendLive.form(
            view,
            fee: nil,
            display: .usd,
            on: drawn,
            loc: loc
        )

        #expect(model.sweepRows.count == 2)
        #expect(model.sweepRows[0].amount == "0.49067 xDAI",
                "the row must show what the core says leaves, not the 0.50067 held")
        #expect(model.sweepRows[0].balanceLabel == "0.50067 xDAI",
                "the balance is still shown — beside the figure, not as it")
        #expect(model.sweepRows[1].amount.isEmpty,
                "no spec yet is no figure, never the balance")
        #expect(model.sweepSummary?.contains("Gnosis") == true)
        #expect(model.token == nil, "a sweep has no single token to name in a header")
        #expect(model.amount == nil, "and no single amount — the rows are the amount")
    }
}
