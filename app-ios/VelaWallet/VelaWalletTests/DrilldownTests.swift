//
//  DrilldownTests.swift
//  VelaWalletTests
//
//  Where a live home leads: the history screen, one transaction, and one token.
//
//  A home that shows real money and hands off to fixture screens is the same
//  lie one level down — tap a real transfer, read somebody else's transaction.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct DrilldownTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private var now: Double { Date().timeIntervalSince1970 }
    private var dayStart: Double { TxRecords.dayStartMs(Date().timeIntervalSince1970) }

    private func item(
        id: String,
        direction: FeedDirectionWire = .in,
        alias: String? = nil,
        counterparty: String? = "0x9F3cA71b8021aE9F3cA71b8021aE9F3cA71b8021",
        value: String? = "120",
        symbol: String = "USDT",
        chainId: Int = 1
    ) -> FeedItemWire {
        FeedItemWire(
            id: id, direction: direction, counterparty: counterparty, alias: alias,
            value: value, symbol: symbol, decimals: 6, usdValue: 120, chainId: chainId,
            timestamp: now, dayStartMs: dayStart, txHash: "0xabcdef0123456789", batch: nil
        )
    }

    private func record(
        id: String, symbol: String = "USDT", usd: String? = "$120.00",
        status: FeedTxStatusWire = .confirmed
    ) -> FeedTxRecordWire {
        FeedTxRecordWire(
            id: id, userOpHash: "", txHash: "0xabcdef0123456789",
            from: "0x9F3cA71b8021aE9F3cA71b8021aE9F3cA71b8021",
            to: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", toName: nil,
            value: "120", symbol: symbol, decimals: 6, logoUrls: nil, chainId: 1,
            timestamp: now, dayStartMs: dayStart, status: status, kind: .receive, usd: usd
        )
    }

    private func feed(_ items: [FeedItemWire], records: [FeedTxRecordWire] = []) -> FeedViewWire {
        var rows: [FeedRowWire] = [.header(id: "day-\(dayStart)", dayStartMs: dayStart,
                                           timestamp: now)]
        rows.append(contentsOf: items.map { .item($0) })
        return FeedViewWire(rows: rows, transactions: records, newItemId: nil, toast: nil)
    }

    private var baseHistory: HistoryModel {
        guard case .history(let model) = WalletFlowFixtures.build(.a1, loc: loc).base
        else { fatalError("the A1 fixture lost its history") }
        return model
    }

    private var baseTxDetail: TxDetailModel {
        guard case .txDetail(let model)? = WalletFlowFixtures.build(.a2, loc: loc).sheet
        else { fatalError("the A2 fixture lost its transaction sheet") }
        return model
    }

    private var baseTokenDetail: TokenDetailModel {
        guard case .tokenDetail(let model)? = WalletFlowFixtures.build(.t2, loc: loc).sheet
        else { fatalError("the T2 fixture lost its token sheet") }
        return model
    }

    // MARK: - A1

    /// The history is the home's own feed, not a second one built from another
    /// source — which is how 全部 comes to disagree with the screen it was
    /// opened from.
    @Test func theHistoryIsTheSameFeedTheHomeShows() {
        let view = feed([item(id: "a"), item(id: "b", direction: .out, value: "2", symbol: "POL")])
        let live = FlowsLive.history(view, on: baseHistory, loc: loc, hidden: false)
        #expect(live.mode == .rows)
        #expect(live.groups.count == 1)
        #expect(live.groups[0].rows.count == 2)
        // The same rows, built by the same function — compared on what a
        // person reads, since the display models carry fresh identities.
        let home = WalletLive.activityGroups(view, loc: loc, hidden: false)
        #expect(live.groups.map { $0.rows.map(\.amount) }
                == home.map { $0.rows.map(\.amount) })
        #expect(live.groups.map(\.label) == home.map(\.label))
    }

    /// An empty feed is the quiet filtered-empty line, not the fixture's rows.
    @Test func anEmptyHistoryDrawsItsOwnEmptyState() {
        let live = FlowsLive.history(feed([]), on: baseHistory, loc: loc, hidden: false)
        #expect(live.mode == .empty)
        #expect(live.groups.isEmpty)
    }

    /// Hiding the balance hides the amounts here too — the mask is not a home
    /// screen decoration, it is the whole session's.
    @Test func hidingTheBalanceHidesTheHistoryAmounts() {
        let live = FlowsLive.history(feed([item(id: "a")]), on: baseHistory,
                                     loc: loc, hidden: true)
        #expect(live.groups[0].rows[0].amount == WalletFixtures.mask)
    }

    // MARK: - A2

    @Test func aTransactionSheetIsBuiltFromTheStoredRecord() {
        let live = FlowsLive.txDetail(
            item(id: "a"), record: record(id: "a"), on: baseTxDetail, loc: loc
        )
        #expect(live.title == loc.t("history.txLabelReceived", vars: ["symbol": "USDT"]))
        #expect(live.amount == "+120 USDT")
        #expect(live.positive)
        // The STORED figure — re-pricing it today would restate history.
        #expect(live.fiat.contains("$120.00"))
        #expect(live.status.text == loc.t("componentsTx.detail.statusSucceeded"))

        let labels = live.facts.map(\.label)
        #expect(labels.contains(loc.t("componentsTx.detail.from")))
        #expect(labels.contains(loc.t("componentsTx.detail.labelChain")))
        #expect(labels.contains(loc.t("componentsTx.detail.labelHash")))
        // The 代币合约 row is omitted, not filled with a plausible address: a
        // stored record does not carry the contract.
        #expect(!labels.contains(loc.t("receive.tokenContract")))
    }

    @Test func aPendingTransactionSaysSoAndAFailedOneDoesToo() {
        let pending = FlowsLive.txDetail(
            item(id: "a"), record: record(id: "a", status: .pending),
            on: baseTxDetail, loc: loc
        )
        #expect(pending.status.text == loc.t("componentsTx.detail.statusPending"))
        let failed = FlowsLive.txDetail(
            item(id: "a"), record: record(id: "a", status: .failed),
            on: baseTxDetail, loc: loc
        )
        #expect(failed.status.text == loc.t("componentsTx.detail.statusFailed"))
    }

    /// A row with no stored record behind it — a folded batch — still opens,
    /// with what the item itself knows and no invented fiat.
    @Test func aRowWithoutARecordStillOpens() {
        let live = FlowsLive.txDetail(
            item(id: "batch", direction: .out), record: nil, on: baseTxDetail, loc: loc
        )
        #expect(live.title == loc.t("history.txLabelSent", vars: ["symbol": "USDT"]))
        #expect(live.fiat.isEmpty, "an unknown value was given a figure")
        #expect(!live.positive)
    }

    /// A resolved name replaces the address, and an unresolved one shows the
    /// address rather than a blank.
    @Test func theCounterpartyIsNamedWhenAnybodyCouldNameIt() {
        let named = FlowsLive.txDetail(
            item(id: "a", alias: "vitalik.eth"), record: record(id: "a"),
            on: baseTxDetail, loc: loc
        )
        #expect(named.facts.first?.value == "vitalik.eth")
        let bare = FlowsLive.txDetail(
            item(id: "a"), record: record(id: "a"), on: baseTxDetail, loc: loc
        )
        #expect(bare.facts.first?.value == AddressText.short(
            "0x9F3cA71b8021aE9F3cA71b8021aE9F3cA71b8021"
        ))
        #expect(bare.facts.first?.mono == true, "an address must be drawn in mono")
    }

    // MARK: - The network filter

    /// The picker offers the chains this account has transfers ON, with their
    /// counts — not the twelve built-ins. A filter offering an empty chain is a
    /// dead end somebody has to back out of.
    @Test func theChainPickerOffersOnlyChainsWithTransfers() {
        let view = feed([
            item(id: "a", chainId: 1),
            item(id: "b", chainId: 1),
            item(id: "c", chainId: 100),
        ])
        let sheet = FlowsLive.chainSheet(view, selected: nil, loc: loc)

        #expect(sheet.rows.count == 3, "expected 所有网络 + two chains")
        #expect(sheet.rows[0].name == loc.t("componentsUi.networkFilter.allNetworks"))
        #expect(sheet.rows[0].chainId == nil)
        #expect(sheet.rows[0].count == 3)
        #expect(sheet.rows[0].selected, "no filter means 所有网络 is the chosen row")
        // Registry order, so the list does not reshuffle as counts change.
        #expect(sheet.rows[1].name == "Ethereum")
        #expect(sheet.rows[1].count == 2)
        #expect(sheet.rows[2].name == "Gnosis")
        #expect(sheet.rows[2].count == 1)
    }

    @Test func thePickedChainIsTheMarkedRowAndTheHeaderPill() {
        let view = feed([item(id: "a", chainId: 1), item(id: "b", chainId: 100)])
        let sheet = FlowsLive.chainSheet(view, selected: 100, loc: loc)
        #expect(sheet.rows.first(where: { $0.chainId == 100 })?.selected == true)
        #expect(sheet.rows[0].selected == false, "所有网络 stayed marked under a filter")

        let live = FlowsLive.history(view, selected: 100, on: baseHistory,
                                     loc: loc, hidden: false)
        #expect(live.header.pill?.label == "Gnosis")
        // And with no filter the drawn 全部网络 pill stands.
        let unfiltered = FlowsLive.history(view, on: baseHistory, loc: loc, hidden: false)
        #expect(unfiltered.header.pill?.label == baseHistory.header.pill?.label)
    }

    /// The shell does not filter. It sends the choice to the core and renders
    /// whatever comes back — the rows above are built from the feed the core
    /// published, filtered or not.
    @Test func theShellDoesNotDoTheFilteringItself() {
        // A feed the core has already filtered to one chain: the screen shows
        // exactly those rows, and the picker still offers what is in it.
        let filtered = feed([item(id: "a", chainId: 100)])
        let live = FlowsLive.history(filtered, selected: 100, on: baseHistory,
                                     loc: loc, hidden: false)
        #expect(live.groups.flatMap(\.rows).count == 1)
        let sheet = FlowsLive.chainSheet(filtered, selected: 100, loc: loc)
        #expect(sheet.rows.count == 2, "the picker invented a chain the feed does not have")
    }

    // MARK: - T2

    private func token(
        symbol: String = "USDT", balance: String = "53.4836",
        price: Double? = 1, chainId: Int = 1, contract: String? = "0xdAC17F958D2ee523a22"
    ) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chainId, symbol: symbol, name: symbol, balance: balance,
                         decimals: 6, tokenAddress: contract, priceUsd: price, spam: false)
    }

    @Test func aTokenSheetShowsThatTokensOwnFactsAndTransfers() {
        let view = feed([
            item(id: "a"),
            // Same ticker, different chain — a different asset, and its row
            // must not appear here.
            item(id: "b", symbol: "USDT", chainId: 137),
            item(id: "c", symbol: "POL", chainId: 1),
        ])
        let live = FlowsLive.tokenDetail(
            token(), feed: view, display: .usd, on: baseTokenDetail, loc: loc
        )
        #expect(live.symbol == "USDT")
        #expect(live.chain == "Ethereum")
        #expect(live.balance == "53.4836 USDT")
        #expect(live.fiat.contains("53.48"))
        #expect(live.rows.count == 1, "another chain's USDT leaked into this token's list")

        let labels = live.facts.map(\.label)
        #expect(labels.contains(loc.t("tokenDetail.labelPrice")))
        #expect(labels.contains(loc.t("tokenDetail.labelContract")))
        #expect(labels.contains(loc.t("tokenDetail.labelDecimals")))
    }

    /// A coin nobody could price has no price row and no fiat line — not a
    /// `$0.00`, which reads as "worthless".
    @Test func anUnpricedTokenShowsNoFigureRatherThanZero() {
        let live = FlowsLive.tokenDetail(
            token(symbol: "MON", balance: "12", price: nil, chainId: 143, contract: nil),
            feed: nil, display: .usd, on: baseTokenDetail, loc: loc
        )
        #expect(live.fiat.isEmpty)
        #expect(!live.facts.map(\.label).contains(loc.t("tokenDetail.labelPrice")))
        // A native coin has no contract row either.
        #expect(!live.facts.map(\.label).contains(loc.t("tokenDetail.labelContract")))
    }

    /// The sheet converts into the chosen currency, like every other money
    /// figure in the app.
    @Test func theTokenSheetSpeaksTheChosenCurrency() {
        let live = FlowsLive.tokenDetail(
            token(balance: "100", price: 1),
            feed: nil,
            display: WalletLive.Display.from(
                CurrencyViewWire(code: "CNY", rate: 7, committed: true)
            ),
            on: baseTokenDetail, loc: loc
        )
        #expect(live.fiat.contains("700"))
        #expect(live.facts.first?.value.contains("7") == true)
    }
}

// MARK: - The explorer links

@MainActor
struct ExplorerLinkTests {

    private func freshStore() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
    }

    @Test func aTransactionLinksToItsOwnChainsExplorer() {
        let store = freshStore()
        #expect(ExplorerLinks.tx(chainId: 100, hash: "0xabc", store: store)?.absoluteString
                == "https://gnosisscan.io/tx/0xabc")
        #expect(ExplorerLinks.address(chainId: 1, "0xdead", store: store)?.absoluteString
                == "https://etherscan.io/address/0xdead")
        #expect(ExplorerLinks.token(chainId: 1, contract: "0xdAC17", store: store)?.absoluteString
                == "https://etherscan.io/token/0xdAC17")
    }

    /// **Web falls back to Etherscan for a chain it has no explorer for; this
    /// does not.** Sending somebody to Ethereum's explorer to look up a Gnosis
    /// transaction is the misleading link web's own comment warns about — they
    /// would find nothing and reasonably conclude their money had vanished.
    @Test func aChainWithoutAnExplorerHasNoLinkRatherThanTheWrongOne() {
        let store = freshStore()
        #expect(ExplorerLinks.base(chainId: 4_294_967_294, store: store) == nil)
        #expect(ExplorerLinks.tx(chainId: 4_294_967_294, hash: "0xabc", store: store) == nil)
        #expect(ExplorerLinks.address(chainId: 4_294_967_294, "0xdead", store: store) == nil)
    }

    /// A native coin has no token page — the caller falls back to the account's
    /// own page rather than linking to the chain's homepage.
    @Test func aNativeCoinHasNoTokenPage() {
        let store = freshStore()
        #expect(ExplorerLinks.token(chainId: 100, contract: nil, store: store) == nil)
        #expect(ExplorerLinks.token(chainId: 100, contract: "", store: store) == nil)
    }

    /// A network the person added carries its own explorer. Reading only the
    /// built-ins would leave every added chain's transactions unlinkable.
    @Test func aCustomNetworkBringsItsOwnExplorer() {
        let store = freshStore()
        store.writeList(VelaStore.Key.customNetworks, [[
            "chainId": 7_777_777, "displayName": "Zora",
            "explorerURL": "https://explorer.zora.energy/",
        ]])
        // The trailing slash is stripped, as web strips it.
        #expect(ExplorerLinks.tx(chainId: 7_777_777, hash: "0xabc", store: store)?.absoluteString
                == "https://explorer.zora.energy/tx/0xabc")
    }

    @Test func nothingToLookUpIsNoLink() {
        let store = freshStore()
        #expect(ExplorerLinks.tx(chainId: 1, hash: "", store: store) == nil)
        #expect(ExplorerLinks.address(chainId: 1, "", store: store) == nil)
    }
}
