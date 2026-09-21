//
//  WalletLiveTests.swift
//  VelaWalletTests
//
//  The home screen's money, in the currency the person chose — or honestly in
//  USD when nobody could price theirs.
//
//  `rate: null` is not `1` (FR-009), and this is the surface where that stops
//  being a settings-row detail: the hero is the biggest number in the app.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct WalletLiveTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func token(
        _ symbol: String, chainId: Int = 100, balance: String, price: Double?
    ) -> BalanceTokenWire {
        BalanceTokenWire(chainId: chainId, symbol: symbol, name: symbol, balance: balance,
                         decimals: 18, tokenAddress: nil, priceUsd: price, spam: false)
    }

    private func view(
        total: Double?, tokens: [BalanceTokenWire] = [], hidden: Bool = false
    ) -> BalanceViewWire {
        BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: total, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: hidden, refreshing: false, lastRefreshedAtMs: nil,
            tokens: tokens, unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], bannerChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    private var base: BalanceModel { WalletFixtures.buildMobileState(.h1, loc: loc).balance }

    // MARK: - The hero

    @Test func aPricedCurrencyConvertsTheHeroAndWearsItsCode() {
        let model = WalletLive.balance(
            view(total: 1_000),
            display: WalletLive.Display.from(CurrencyViewWire(code: "CNY", rate: 7.1,
                                                             committed: true)),
            fallback: base
        )
        #expect(model.currency == "CNY")
        #expect(model.integer == "¥7,100", "the hero lost its currency badge")
    }

    /// **The defect this rule exists to prevent.** With no rate, the hero shows
    /// the USD figure labelled USD — not the same digits under a CNY label,
    /// which would tell somebody 1,000 dollars is 1,000 yuan.
    @Test func anUnpriceableCurrencyDegradesRatherThanRelabelling() {
        let model = WalletLive.balance(
            view(total: 1_000),
            display: WalletLive.Display.from(CurrencyViewWire(code: "CNY", rate: nil,
                                                             committed: true)),
            fallback: base
        )
        #expect(model.currency == "USD")
        #expect(model.integer == "$1,000")
    }

    /// An uncommitted choice is the USD placeholder — the person has not chosen
    /// yet, whatever code the model is carrying.
    @Test func anUncommittedChoiceStaysInUsd() {
        let display = WalletLive.Display.from(
            CurrencyViewWire(code: "JPY", rate: 150, committed: false)
        )
        #expect(display.code == "USD")
        #expect(display.rate == 1)
    }

    /// A zero or non-finite rate is not a rate. Converting through one would
    /// erase the balance entirely.
    @Test func aNonsensicalRateDegradesToo() {
        for rate in [0, -1, Double.infinity, Double.nan] {
            let display = WalletLive.Display.from(
                CurrencyViewWire(code: "EUR", rate: rate, committed: true)
            )
            #expect(display.code == "USD", "rate \(rate) was taken as a conversion")
        }
    }

    // MARK: - The asset rows

    @Test func anAssetRowIsPricedInTheChosenCurrency() {
        let rows = WalletLive.assetRows(
            view(total: 1, tokens: [token("xDAI", balance: "100", price: 1)]),
            display: WalletLive.Display.from(CurrencyViewWire(code: "JPY", rate: 150,
                                                             committed: true))
        )
        #expect(rows.count == 1)
        guard case .value(let text) = rows[0].fiat else {
            Issue.record("a priced holding rendered as unpriced")
            return
        }
        // 100 × $1 × 150.
        #expect(text.contains("15,000"))
    }

    /// A held token nobody could price shows its amount and no fiat figure.
    /// `$0.00` would read as "worthless", which is a different claim.
    @Test func anUnpriceableHoldingShowsNoFiatFigure() {
        let rows = WalletLive.assetRows(
            view(total: nil, tokens: [token("MON", chainId: 143, balance: "12", price: nil)])
        )
        guard case .noPrice = rows[0].fiat else {
            Issue.record("an unpriced holding was given a figure")
            return
        }
    }

    /// A balance reads at six places at most, as on the web and Android — the
    /// core's full precision pushed the ticker and chain out of the row on the
    /// iPhone 11 ("…" / "…" beside 0.00067035411363817).
    @Test func anAssetRowsBalanceIsTrimmedToSixPlaces() {
        let before = Formats.current
        defer { Formats.current = before }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        let rows = WalletLive.assetRows(view(total: 1, tokens: [
            token("ETH", chainId: 8453, balance: "0.00067035411363817", price: 2700),
            token("xDAI", balance: "0.48967", price: 1),
            token("USDC", balance: "12.500000", price: 1),
            token("DOGE", balance: "5", price: nil),
        ]))
        #expect(rows.map(\.balance) == ["0.00067", "0.48967", "12.5", "5"])

        // The person's decimal mark, never grouping.
        Formats.current = Formats.Current(number: .dotComma, date: .iso, time: .h24)
        #expect(WalletLive.trimBalance("12345.678901234") == "12345,678901")
        #expect(WalletLive.trimBalance("0.000000123") == "0")
    }

    /// Hiding is by construction, not by masking downstream: a hidden balance
    /// has no digits to leak into an accessibility label.
    @Test func ahiddenBalanceCarriesNoFigureAtAll() {
        let model = WalletLive.balance(view(total: 1_000, hidden: true), fallback: base)
        #expect(model.state == .hidden)
        #expect(model.integer == nil)
        #expect(model.decimals == nil)
    }
}

// MARK: - The activity feed

@MainActor
struct ActivityRowTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func item(
        id: String = "1",
        direction: FeedDirectionWire = .in,
        alias: String? = nil,
        counterparty: String? = "0x9F3cA71b8021aE9F3cA71b8021aE9F3cA71b8021",
        value: String? = "120",
        symbol: String = "USDT",
        dayStartMs: Double,
        timestamp: Double
    ) -> FeedItemWire {
        FeedItemWire(
            id: id, direction: direction, counterparty: counterparty, alias: alias,
            value: value, symbol: symbol, decimals: 6, usdValue: 120, chainId: 1,
            timestamp: timestamp, dayStartMs: dayStartMs, txHash: "0xabc", batch: nil
        )
    }

    private func feed(_ rows: [FeedRowWire]) -> FeedViewWire {
        FeedViewWire(rows: rows, transactions: [], newItemId: nil, toast: nil)
    }

    /// The core emits headers already interleaved with items, in render order.
    /// This walks that list — it must never re-group or re-sort, because the
    /// interleaving is what makes a header unable to drift away from its rows.
    @Test func theCoresOwnGroupingIsWalkedRatherThanRebuilt() {
        let today = Date().timeIntervalSince1970
        let dayStart = TxRecords.dayStartMs(today)
        let yesterday = dayStart - 86_400_000
        let groups = WalletLive.activityGroups(feed([
            .header(id: "day-\(dayStart)", dayStartMs: dayStart, timestamp: today),
            .item(item(id: "a", dayStartMs: dayStart, timestamp: today)),
            .item(item(id: "b", direction: .out, dayStartMs: dayStart, timestamp: today)),
            .header(id: "day-\(yesterday)", dayStartMs: yesterday, timestamp: today - 86_400),
            .item(item(id: "c", dayStartMs: yesterday, timestamp: today - 86_400)),
        ]), loc: loc, hidden: false)

        #expect(groups.count == 2)
        #expect(groups[0].label == loc.t("componentsUi.dayGroup.today"))
        #expect(groups[0].rows.count == 2)
        #expect(groups[1].label == loc.t("componentsUi.dayGroup.yesterday"))
        #expect(groups[1].rows.count == 1)
    }

    /// A header with nothing under it is not a day — the chain filter can empty
    /// one, and an orphan date over blank space reads as a loading failure.
    @Test func anEmptyDayIsNotDrawn() {
        let dayStart = TxRecords.dayStartMs(Date().timeIntervalSince1970)
        let groups = WalletLive.activityGroups(feed([
            .header(id: "day-\(dayStart)", dayStartMs: dayStart, timestamp: 0),
        ]), loc: loc, hidden: false)
        #expect(groups.isEmpty)
    }

    @Test func aReceiptReadsAsMoneyInAndASendAsMoneyOut() {
        let now = Date().timeIntervalSince1970
        let dayStart = TxRecords.dayStartMs(now)
        let received = WalletLive.activityRow(
            item(alias: "vitalik.eth", dayStartMs: dayStart, timestamp: now),
            loc: loc, hidden: false
        )
        #expect(received.kind == .received)
        #expect(received.positive)
        #expect(received.amount == "+120")
        #expect(received.unit == "USDT")
        #expect(received.subtitle.contains("vitalik.eth"))

        let sent = WalletLive.activityRow(
            item(direction: .out, value: "2", symbol: "POL",
                 dayStartMs: dayStart, timestamp: now),
            loc: loc, hidden: false
        )
        #expect(sent.kind == .sent)
        #expect(!sent.positive)
        #expect(sent.amount == "\u{2212}2")
        // Nobody named them, so the address is shown — a fact, where a made-up
        // label would not be.
        #expect(sent.subtitle.contains("0x9F3c\u{2026}8021"))
    }

    /// Privacy hides the FIGURE, not the fact that something moved: the row
    /// stays, its unit stays, and the amount is dots.
    @Test func ahiddenBalanceMasksTheAmountAndKeepsTheRow() {
        let now = Date().timeIntervalSince1970
        let row = WalletLive.activityRow(
            item(dayStartMs: TxRecords.dayStartMs(now), timestamp: now),
            loc: loc, hidden: true
        )
        #expect(row.masked)
        #expect(row.amount == WalletFixtures.mask)
        #expect(row.unit == "USDT")
    }

    /// Amounts are grouped in string space and TRUNCATED, never rounded up: a
    /// glance view that rounds is how a history stops matching the chain.
    @Test func amountsAreGroupedAndNeverRoundedUp() {
        #expect(WalletLive.compactAmount("1234567.8901") == "1,234,567.8901")
        #expect(WalletLive.compactAmount("0.123456789") == "0.123456")
        #expect(WalletLive.compactAmount("120") == "120")
        #expect(WalletLive.compactAmount("0.7589700") == "0.75897")
        // A mixed-token batch has no single amount; the row states how many
        // assets moved instead.
        #expect(WalletLive.compactAmount(nil) == "0")
    }
}
