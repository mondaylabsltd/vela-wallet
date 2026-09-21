//
//  SendAssetsParityTests.swift
//  VelaWalletTests
//
//  Seven places the iOS shell said something the web does not — each one a
//  drawing standing in for the core, or a core verdict dropped on the way to
//  the screen. The web builders (`live-send.ts`, `live-batch.ts`,
//  `live-contact-pick.ts`, `live.ts`) are the spec these are checked against.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SendAssetsParityTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private static let alice = "0x1111111111111111111111111111111111111111"
    private static let bob = "0x2222222222222222222222222222222222222222"

    /// The core's own starting view with a few fields patched — no field is
    /// invented, so the fixture cannot drift from the wire.
    private func sendView(_ patch: [String: Any]) -> SendViewWire {
        var object = try! CoreJSON.object(SendCore().view())
        for (key, value) in patch { object[key] = value }
        return try! CoreJSON.decode(SendViewWire.self, from: object)
    }

    private let xdai: [String: Any] = [
        "symbol": "xDAI", "network": "Gnosis", "chain_id": 100,
        "token_address": NSNull(), "decimals": 18, "balance": "10",
        "price_usd": 1.0, "logo_urls": [], "spam": false,
    ]

    private func drawnConfirm(_ state: FlowStateId) -> SendConfirmModel {
        guard case .sendConfirm(let model) = WalletFlowFixtures.build(state, loc: loc).base
        else { fatalError("\(state) does not draw the confirm page") }
        return model
    }

    private func confirm(_ view: SendViewWire, on state: FlowStateId) -> SendConfirmModel {
        SendLive.confirm(
            view,
            from: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", name: nil),
            display: .usd, on: drawnConfirm(state), loc: loc
        )
    }

    // MARK: - 1. The split / sweep confirm lists the real parts

    @Test func aSplitConfirmListsTheTypedPayeesAndNeverTheDrawing() {
        let view = sendView([
            "stage": "confirm", "split_mode": true, "selected_token": xdai,
            "confirm_amount": "3.5",
            "recipients": [
                ["id": "r1", "address": Self.alice, "amount": "1.5", "name": "Alice"],
                ["id": "r2", "address": Self.bob, "amount": "2", "name": NSNull()],
            ],
        ])
        let model = confirm(view, on: .sd3b)

        #expect(model.breakdown.count == 2)
        #expect(model.breakdown.map(\.value) == ["1.5 xDAI", "2 xDAI"])
        // A name never stands in for the address on the page that signs.
        #expect(model.breakdown[0].label == "Alice · \(AddressText.short(Self.alice))")
        #expect(model.breakdown[1].label == AddressText.short(Self.bob))
        #expect(model.breakdown[0].identiconSeed == Self.alice)
        // Nothing from the drawing survives.
        let drawn = drawnConfirm(.sd3b).breakdown.map(\.label)
        #expect(model.breakdown.allSatisfy { !drawn.contains($0.label) })
        // "2 recipients", and no blank single "To" row.
        #expect(model.subline.hasPrefix(loc.t("send.recipientCount_other", vars: ["count": "2"])))
        #expect(!model.facts.contains { $0.label == loc.t("send.toLabel") })
        #expect(model.amount == "3.5 xDAI")
    }

    @Test func aSweepConfirmListsWhatMovesPricedFromTheCoresSpecs() {
        let usdc: [String: Any] = [
            "symbol": "USDC", "network": "Gnosis", "chain_id": 100,
            "token_address": "0xtoken", "decimals": 6, "balance": "12.5",
            "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        let view = sendView([
            "stage": "confirm", "multi_select_mode": true, "recipient": Self.alice,
            "multi_selected_ids": ["Gnosis_native_xDAI", "Gnosis_0xtoken_USDC"],
            "multi_chain_id": 100,
            "tokens": [xdai, usdc],
            "multi_specs": [["token_address": NSNull(), "decimals": 18, "amount": "9.5"]],
        ])
        let model = confirm(view, on: .sd3c)

        #expect(model.breakdown.map(\.label) == ["xDAI", "USDC"])
        #expect(model.breakdown[0].value == "9.5 xDAI · ≈$9.50",
                "the reserved spec, not the 10 held")
        #expect(model.breakdown[1].value == "12.5 USDC · ≈$12.50")
        #expect(model.amount == loc.t("componentsTx.receipt.assetsCount", vars: ["n": "2"]))
        #expect(model.subline == loc.t("send.confirmTotalLine", vars: [
            "fiat": "$22.00", "network": "Gnosis",
        ]))
        #expect(model.mark == nil)
    }

    // MARK: - 2. The picker's groups are the book's, and a pick adds members

    @Test func thePickersGroupsAreTheBooksOwn() {
        let member = ContactWire(
            address: Self.alice, name: nil, resolvedName: "alice.eth", resolvedSource: "ENS",
            kind: .unknown, favorite: false, note: nil, txCount: 0,
            lastUsedMs: 0, firstSeenMs: 0, source: .manual
        )
        let book = ContactsViewWire(
            loaded: true, contacts: [member], sections: [],
            groups: [ContactGroupWire(id: "g1", name: "Payroll", color: nil, members: [member])],
            lastImport: nil, importFailure: nil, export: nil, recipient: nil
        )
        guard case .contactPick(let drawn)? = WalletFlowFixtures.build(.sd2e, loc: loc).sheet
        else { fatalError("SD2e does not draw the picker") }
        let model = SendLive.contactSheet(book, on: drawn, loc: loc)

        #expect(model.groups.map(\.name) == ["Payroll"])
        #expect(model.groups[0].count == loc.t("contacts.groupMembers", vars: ["count": "1"]))
        #expect(model.contacts[0].group == "Payroll")

        let rows = SendLive.groupRecipients(book.groups[0])
        #expect(rows?.count == 1)
        #expect(rows?[0]["address"] as? String == Self.alice)
        #expect(rows?[0]["amount"] as? String == "")
        #expect(rows?[0]["name"] as? String == "alice.eth")
        #expect(SendLive.groupRecipients(
            ContactGroupWire(id: "g2", name: "Empty", color: nil, members: [])
        ) == nil, "an empty group adds nobody")
    }

    // MARK: - 3 + 7. The importer adds by default, and says what it comes to

    private func batchWire(count: Int, canApply: Bool, over: Bool = false) -> BatchViewWire {
        let base = BatchViewWire.empty
        return BatchViewWire(
            opened: true, unit: base.unit, fiatCode: "USD", rawText: "",
            fileName: nil, busy: false, fileError: false, templateSaved: false, priced: true,
            rateStatus: .ok, rateInput: "1", rateEdited: false, preview: [],
            overCap: false, rejected: 0, recipientCount: count, totalToken: "4.50",
            totalFiat: "4.50", overBalance: over, canApply: canApply,
            recipients: [], applied: false
        )
    }

    private func drawnBatch() -> BatchImportModel {
        guard case .batchImport(let model)? = WalletFlowFixtures.build(.sd2c, loc: loc).sheet
        else { fatalError("SD2c does not draw the importer") }
        return model
    }

    @Test func anEmptyFormOffersNoMergeChoiceAndShowsTheBalance() {
        let view = sendView(["selected_token": xdai])
        #expect(view.splitImportRoom == BatchStore.maxRecipients, "the core's field decodes")
        let model = SendLive.batchImport(
            batchWire(count: 2, canApply: true), view: view, on: drawnBatch(), loc: loc
        )
        #expect(model.merge == nil, "nobody on the form, nothing to add to or replace")
        #expect(model.total?.label == "\(loc.t("send.splitTotalLabel")) · "
                + loc.t("send.recipientCount_other", vars: ["count": "2"]))
        #expect(model.total?.value == "4.5 xDAI")
        #expect(model.total?.detail == "4.50 USD")
        #expect(model.total?.balance == loc.t("send.balanceLabel", vars: ["amount": "10 xDAI"]))
    }

    @Test func aFormWithRowsAddsByDefaultAndCanBeToldToReplace() {
        let view = sendView([
            "selected_token": xdai, "split_mode": true,
            "split_import_room": 58, "split_remaining": "6.5",
        ])
        let adding = SendLive.batchImport(
            batchWire(count: 1, canApply: true), view: view, on: drawnBatch(), loc: loc
        )
        #expect(adding.merge?.note == loc.t("send.batchAddsToRows"))
        #expect(adding.merge?.action == loc.t("send.batchReplaceInstead"))
        #expect(adding.total?.balance == loc.t("send.splitRemaining", vars: ["amount": "6.5 xDAI"]))

        let replacing = SendLive.batchImport(
            batchWire(count: 1, canApply: true), view: view, on: drawnBatch(), loc: loc,
            replaces: true
        )
        #expect(replacing.merge?.note == loc.t("send.batchReplacesRows"))
        #expect(replacing.merge?.action == loc.t("send.batchAddInstead"))
        #expect(replacing.total?.balance == loc.t("send.balanceLabel", vars: ["amount": "10 xDAI"]))

        let refused = SendLive.batchImport(
            batchWire(count: 1, canApply: false, over: true), view: view, on: drawnBatch(), loc: loc
        )
        #expect(refused.merge == nil, "said only once the import can happen")
        #expect(refused.total?.over == true)

        let nothing = SendLive.batchImport(
            batchWire(count: 0, canApply: false), view: view, on: drawnBatch(), loc: loc
        )
        #expect(nothing.total == nil, "no rows, no total")
    }

    // MARK: - 4. A coin that cannot pay is drawn as that

    @Test func anInsufficientFeeCoinIsMarkedWithTheCoresVerdict() {
        func option(_ symbol: String, insufficient: Bool, selected: Bool) -> FeeOptionWire {
            FeeOptionWire(
                symbol: symbol, contract: symbol == "XDAI" ? nil : "0xusdc", decimals: 18,
                balance: "0", recipient: "0x1", usdBalance: "0", usdPrice: "1",
                amount: "10000000000000000", insufficient: insufficient, selected: selected
            )
        }
        let fee = FeeViewWire(
            busy: false, failed: nil, fee: nil, stale: false, feeToken: nil,
            options: [
                option("XDAI", insufficient: false, selected: true),
                option("USDC", insufficient: true, selected: false),
            ],
            confirmFeeReady: true
        )
        guard case .feeToken(let drawn)? = WalletFlowFixtures.build(.sd2f, loc: loc).sheet
        else { fatalError("SD2f does not draw the fee sheet") }
        let model = SendLive.feeSheet(fee, on: drawn, loc: loc)
        #expect(model.rows.map(\.insufficient) == [false, true])
        #expect(model.rows[1].insufficientNote
                == loc.t("send.warnInsufficientGas", vars: ["sym": "USDC"]))
    }

    // MARK: - 5. The pill narrows the assets list

    private func balance(_ tokens: [BalanceTokenWire], unknown: Bool = false, loading: Bool = false)
        -> BalanceViewWire
    {
        BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: 1, balanceUnknown: unknown, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: tokens, unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], bannerChainIds: [], holdingsLoading: loading,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    private func holding(_ symbol: String, chainId: Int) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chainId, symbol: symbol, name: symbol, balance: "1",
                         decimals: 18, tokenAddress: nil, priceUsd: 1, spam: false)
    }

    private func drawnAssets() -> AssetsModel {
        guard case .assets(let model) = WalletFlowFixtures.build(.t1, loc: loc).base
        else { fatalError("T1 does not draw the assets list") }
        return model
    }

    @Test func theChosenChainNarrowsTheListAndATapNamesTheRightToken() {
        let held = balance([holding("ETH", chainId: 1), holding("xDAI", chainId: 100)])
        let all = FlowsLive.assets(held, currency: nil, on: drawnAssets(), loc: loc)
        #expect(all.rows.map(\.ticker) == ["ETH", "xDAI"])

        let gnosis = FlowsLive.assets(held, currency: nil, selected: 100, on: drawnAssets(), loc: loc)
        #expect(gnosis.rows.map(\.ticker) == ["xDAI"])
        #expect(gnosis.header.pill?.label == ChainCatalog.meta(100)?.displayName)
        #expect(FlowsLive.visibleAssetIndices(held, selected: 100) == [1],
                "row 0 on the narrowed list is holding 1")

        let sheet = FlowsLive.assetChainSheet(held, selected: 100, loc: loc)
        #expect(sheet.rows.map(\.chainId) == [nil, 1, 100])
        #expect(sheet.rows.first { $0.selected }?.chainId == 100)
    }

    @Test func aChainWithNothingOnItSaysSoAndLoadingSaysNothing() {
        let held = balance([holding("ETH", chainId: 1)])
        let empty = FlowsLive.assets(held, currency: nil, selected: 100, on: drawnAssets(), loc: loc)
        #expect(empty.rows.isEmpty)
        #expect(empty.empty?.title == loc.t("assets.emptyTitle"))

        let settled = FlowsLive.assets(balance([]), currency: nil, on: drawnAssets(), loc: loc)
        #expect(settled.empty != nil, "a wallet that holds nothing, once the core has looked")

        let loading = FlowsLive.assets(balance([], loading: true), currency: nil,
                                       on: drawnAssets(), loc: loc)
        #expect(loading.empty == nil, "never while the holdings are still out")
        let unknown = FlowsLive.assets(balance([], unknown: true), currency: nil,
                                       on: drawnAssets(), loc: loc)
        #expect(unknown.empty == nil)
    }

    // MARK: - 6. The first-time tag, on the page that signs

    /// Device-found: the core resolves `first_time` only while the confirm
    /// page is up (`confirm_probes`), so the form never had it to show.
    @Test func theConfirmSaysItIsTheFirstTimeSendingHere() {
        let base: [String: Any] = [
            "stage": "confirm", "selected_token": xdai, "recipient": Self.alice,
            "confirm_amount": "1",
        ]
        #expect(confirm(sendView(base), on: .sd3).recipientTag == nil)
        var first = base
        first["recipient_risk"] = ["is_contract": false, "first_time": true]
        #expect(confirm(sendView(first), on: .sd3).recipientTag
            == loc.t("componentsUi.signing.firstTimeTag"))
        var known = base
        known["recipient_risk"] = ["is_contract": false, "first_time": false]
        #expect(confirm(sendView(known), on: .sd3).recipientTag == nil)
        // A split has no one recipient for the verdict to be about.
        var split = first
        split["split_mode"] = true
        split["recipients"] = [["id": "r1", "address": Self.alice, "amount": "1", "name": NSNull()]]
        #expect(confirm(sendView(split), on: .sd3b).recipientTag == nil)
    }

    /// The web's `hasPriorInteraction`: only a send, a dApp transaction or a
    /// legacy row with no type is a prior send; case does not matter.
    /// An EIP-7702-delegated EOA (`0xef0100 ++ impl`) is a person's wallet, not
    /// a contract — the web's `isContractAddress`, as Android and desktop answer.
    @Test func aDelegatedWalletIsNotAContract() {
        #expect(!RelayClient.codeIsContract("0xef0100" + String(repeating: "ab", count: 20)))
        #expect(!RelayClient.codeIsContract("0xEF0100" + String(repeating: "AB", count: 20)))
        #expect(RelayClient.codeIsContract("0x6080604052"))
        #expect(!RelayClient.codeIsContract("0x"))
        // 24 bytes is not the designator: a contract.
        #expect(RelayClient.codeIsContract("0xef0100" + String(repeating: "ab", count: 21)))
    }

    @Test func firstTimeIsAnsweredFromTheWalletsOwnSends() {
        let to = "0xAbCdEf0000000000000000000000000000000001"
        #expect(SendExecutor.firstTime(to, records: []))
        #expect(SendExecutor.firstTime(to, records: [
            ["to": to, "type": "receive"], ["to": to, "type": "sign_message"],
        ]), "a receive or a signature is not a send")
        #expect(!SendExecutor.firstTime(to, records: [["to": to.lowercased(), "type": "send"]]))
        #expect(!SendExecutor.firstTime(to, records: [["to": to, "type": "dapp_tx"]]))
        #expect(!SendExecutor.firstTime(to, records: [["to": to]]), "a legacy row with no type")
        #expect(SendExecutor.firstTime(to, records: [["to": Self.bob, "type": "send"]]))
        #expect(!SendExecutor.firstTime("alice.eth", records: []), "a non-address is never first")
    }
}
