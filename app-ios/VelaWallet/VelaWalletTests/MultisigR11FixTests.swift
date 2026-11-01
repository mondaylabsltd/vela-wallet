//
//  MultisigR11FixTests.swift
//  VelaWalletTests
//
//  Round 11's iOS findings, each on the model the screen draws from:
//
//  - r11-3: a multisig whose networks are all in the other space is said to
//    live there — no false zero, stamp or empty state on its home.
//  - r11-1 / r11-10: a notification is keyed by the transactions the core's
//    presence names, so one approved elsewhere and one new (the count the
//    same) is said; a list kept from an earlier page is never news.
//  - S4: the sheet's balance changes with the core's minus (U+2212); the
//    queued note says every number it waits behind; a transaction refused
//    for what it is asks no member; one short form for an address.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR11FixTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords {
        MsWords(loc: loc, hub: nil, localNames: [MultisigViewsTests.one.lowercased(): "Parallel One"])
    }

    private static let safe = MultisigViewsTests.safe
    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    // MARK: - r11-3

    @Test func aMultisigOfTheOtherSpaceLivesElsewhere() {
        let mainnet = [1, 10, 100, 8453]
        let known = mainnet + [11_155_111, 5_042_002]
        #expect(WalletLive.livesElsewhere(chainIds: [5_042_002], here: mainnet, known: known) == [5_042_002])
        #expect(WalletLive.livesElsewhere(chainIds: [5_042_002, 100], here: mainnet, known: known).isEmpty,
                "one network here: it lives here")
        #expect(WalletLive.livesElsewhere(chainIds: [], here: mainnet, known: known).isEmpty)
    }

    @Test func itsHomeSaysNoFalseZeroStampOrEmptyState() {
        let base = WalletFixtures.buildMobileState(.h1, loc: loc)
        let model = WalletLive.livingElsewhere(base, heading: loc.t("networkSpace.headingTestnet"))
        #expect(model.balance.state == .noValue)
        #expect(model.balance.noValueText == "Testnet space")
        #expect(model.balance.integer == nil)
        #expect(model.balance.refresh == nil, "no \"Updated now\" for networks it never lived on")
        #expect(model.activitySection.empty == nil, "no \"No activity yet\"")
        #expect(model.assetsSection.empty == nil, "no \"Deposit your first asset\"")
        #expect(model.activityGroups.isEmpty && model.assetRows.isEmpty)
    }

    // MARK: - r11-1 / r11-10: notifications keyed by transaction

    private func proposals(needs keys: [String], readAt: String = "1") throws -> MsProposalsView {
        let first = keys.first.map { k -> String in
            let hash = k.split(separator: ":")[1]
            return #"{"chain_id":100,"safe_tx_hash":"\#(hash)"}"#
        } ?? "null"
        let presence = #""presence":{"\#(Self.safe.lowercased())":{"needs_you":\#(keys.count),"ready":0,"waiting_payer":0,"first":\#(first),"needs_you_keys":[\#(keys.map { "\"\($0)\"" }.joined(separator: ","))],"ready_keys":[],"read_at":\#(readAt)}}"#
        let json = MultisigViewsTests.proposalsJson(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]), standing: #"{"kind":"yourTurn"}"#
        ).replacingOccurrences(
            of: #""presence":{"\#(Self.safe.lowercased())":{"needs_you":0,"ready":1,"waiting_payer":0,"first":{"chain_id":100,"safe_tx_hash":"\#(Self.hash)"}}}"#,
            with: presence)
        return try #require(decode(MsProposalsView.self, json))
    }

    @Test func oneApprovedElsewhereAndOneNewIsSaid() throws {
        let notifier = MultisigNotifier(loc: loc, defaults: UserDefaults(suiteName: "ms.r11.\(UUID().uuidString)")!)
        var posted: [MultisigNotifier.Note] = []
        notifier.post = { posted.append($0) }
        let accounts = try #require(decode(MsAccountsView.self, MultisigViewsTests.accountsJson))
        notifier.evaluate(accounts: accounts, proposals: try proposals(needs: ["100:0xaa", "100:0xbb"]), words: words, post: true)
        let first = posted.count
        // 0xbb approved elsewhere, 0xcc proposed: still two — and said.
        notifier.evaluate(accounts: accounts, proposals: try proposals(needs: ["100:0xaa", "100:0xcc"]), words: words, post: true)
        #expect(posted.count == first + 1)
        #expect(posted.last?.safeTxHash == "0xcc", "it opens the new one")
        // The same again: nothing.
        notifier.evaluate(accounts: accounts, proposals: try proposals(needs: ["100:0xaa", "100:0xcc"]), words: words, post: true)
        #expect(posted.count == first + 1)
        // A list kept from an earlier page: neither said nor recorded.
        notifier.evaluate(accounts: accounts, proposals: try proposals(needs: ["100:0xaa", "100:0xcc", "100:0xdd"], readAt: "null"),
                          words: words, post: true)
        #expect(posted.count == first + 1)
        notifier.evaluate(accounts: accounts, proposals: try proposals(needs: ["100:0xaa", "100:0xcc", "100:0xdd"]), words: words, post: true)
        #expect(posted.count == first + 2, "read on this page now: 0xdd is news")
    }

    // MARK: - S4

    @Test func aBalanceChangeIsSignedWithTheCoresMinus() throws {
        let out = try #require(MsReviewBody.signedDelta("-10000000000000000", decimals: 18, symbol: "xDAI"))
        #expect(out.text.hasPrefix("\u{2212}"), "\(out.text)")
        #expect(!out.text.hasPrefix("-"))
        #expect(out.negative)
        let into = try #require(MsReviewBody.signedDelta("600000000000000000", decimals: 18, symbol: "USDC"))
        #expect(into.text.hasPrefix("+"))
        #expect(MsReviewBody.signedDelta("0", decimals: 18, symbol: "USDC") == nil, "a zero change is not drawn")
    }

    @Test func oneShortFormForAnAddress() {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        #expect(MsWords.short(address) == "0x88cCA0…266894")
        #expect(AddressText.short(address) == "0x88cCA0…266894")
        #expect(AddressText.short("not-an-address-at-all") == "not-an…-all")
    }

    @Test func theQueuedNoteSaysEveryNumberItWaitsBehind() throws {
        func model(_ waits: String) throws -> MsProposalModel {
            let item = MultisigViewsTests.item(approved: 2, status: "queued")
                .replacingOccurrences(of: #""renameUndeployed":false}"#, with: #""renameUndeployed":false,"waitsFor":\#(waits)}"#)
            let json = MultisigViewsTests.proposalsJson(item: item, standing: #"{"kind":"queued"}"#)
            let view = try #require(decode(MsProposalsView.self, json))
            return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        }
        let one = try model(#"["3"]"#).notes.first { $0.id == "queued" }
        #expect(one?.text == loc.t("home.multisig.proposal.queuedNote", vars: ["nonce": "3"]))
        let many = try model(#"["1","2","3"]"#).notes.first { $0.id == "queued" }
        #expect(many?.text == loc.t("home.multisig.proposal.queuedNoteMany", vars: ["nonces": loc.list(["#1", "#2", "#3"])]))
    }
}
