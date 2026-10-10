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
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
    }

    private var base: BalanceModel { WalletFixtures.buildMobileState(.h1, loc: loc).balance }

    // MARK: - The assets section

    /// 087 F03: an account that holds nothing showed 资产 over a blank area —
    /// the drawn home's `.rows` mode survived the live view. The section
    /// draws its empty state in the corpus's words exactly when the core
    /// says the list is empty (`emptyKey`); the skeleton otherwise; with
    /// holdings, the rows.
    @Test func anEmptyWalletSaysSoUnderAssets() throws {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        #expect(drawn.assetsSection.mode == .rows, "the fixture this used to keep")

        // No rows and a known total of 0 is what a cached zero looks like
        // before anything has been read: not this shell's to call empty.
        let unread = WalletLive.apply(view(total: 0), on: drawn, loc: loc)
        #expect(unread.assetRows.isEmpty)
        #expect(unread.assetsSection.mode == .loading, "\"nothing here\" was said without the core's key")

        var found = view(total: 0)
        found.emptyKey = "assets.emptyTitle"
        let empty = WalletLive.apply(found, on: drawn, loc: loc)
        #expect(empty.assetRows.isEmpty)
        #expect(empty.assetsSection.mode == .empty)
        let words = try #require(empty.assetsSection.empty)
        #expect(words.title == loc.t("assets.emptyTitle"))
        #expect(words.caption == loc.t("assets.emptySubtext"))
        #expect(empty.assetsSection.title == drawn.assetsSection.title)

        var loading = view(total: nil)
        loading = BalanceViewWire(
            address: loading.address, displayTotalUsd: nil, balanceUnknown: false,
            balancePartial: false, notice: nil, hidden: false, refreshing: true,
            lastRefreshedAtMs: nil, tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: true,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        #expect(WalletLive.apply(loading, on: drawn, loc: loc).assetsSection.mode == .loading)

        let held = WalletLive.apply(
            view(total: 2, tokens: [token("XDAI", balance: "2", price: 1)]), on: drawn, loc: loc
        )
        #expect(held.assetsSection.mode == .rows)
        #expect(held.assetRows.count == 1)
    }

    // MARK: - The hero

    /// 082 X-DEADPROXY: with chains failing, the line under the figure says
    /// so in words — never a ⚠ › with nothing beside it (the drawn fixture's
    /// status was nil on the live home).
    @Test func aPartialTotalSaysSoInWords() {
        let partial = BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: 1_000, balanceUnknown: false, balancePartial: true,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [], unpricedTokens: [], failedChainIds: [137],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        var silent = base
        silent.status = nil
        let model = WalletLive.balance(partial, fallback: silent, loc: loc)
        #expect(model.status?.kind == .warning)
        #expect(model.status?.text == loc.t("home.balanceStale"))
        #expect(model.status?.text.isEmpty == false)
    }

    /// Spec 092 (F08): unreachable networks come first on the line, in the
    /// core's sentence and without "RPC" — one named, several counted, every
    /// one of them; the list it opens is SR6 (`SettingsLive.withUnreachable`).
    @Test func unreachableNetworksAreSaidPlainlyAndCounted() {
        func down(_ ids: [Int], key: String) -> BalanceViewWire {
            var view = view(total: 1_000)
            view.unreachableNetworks = ids.map {
                UnreachableNetworkWire(chainId: $0, lastKnown: "not_read", lastSeenUsd: nil,
                                       lineKey: I18nKeys.SettingsUi.notReadYet)
            }
            view.unreachableKey = key
            return view
        }
        let one = WalletLive.balance(down([56], key: I18nKeys.SettingsUi.unreachableOne),
                                     fallback: base, loc: loc)
        #expect(one.status?.kind == .warning)
        #expect(one.status?.text == "暂时连不上 BNB Chain")
        let many = WalletLive.balance(down([56, 137, 10], key: I18nKeys.SettingsUi.unreachableMany),
                                      fallback: base, loc: loc)
        #expect(many.status?.text == "3 个网络暂时连不上")
        #expect(many.status?.text.contains("RPC") == false)
    }

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

    /// An uncommitted choice never converts — whatever code and rate the
    /// model is carrying, the digits stay dollars — and it is not SETTLED:
    /// nothing draws a figure from it (PR 3).
    @Test func anUncommittedChoiceStaysInUsd() {
        let display = WalletLive.Display.from(
            CurrencyViewWire(code: "JPY", rate: 150, committed: false)
        )
        #expect(display.code == "USD")
        #expect(display.rate == 1)
        #expect(!display.settled)
    }

    /// PR 3, the 102 device run: an iPhone with CNY stored drew "USD $1,383"
    /// for a few seconds, then jumped to "CNY ¥9,819". While the currency is
    /// not committed NO figure is drawn — the hero waits, named by the stored
    /// choice on its way — and the figure appears once, in the right money.
    @Test func theHeroWaitsForTheCurrencyAndNeverShowsDollarsFirst() {
        let balance = view(total: 1_383.28, tokens: [token("xDAI", balance: "100", price: 1)])

        // The stored choice's rate is on its way (`pending`).
        let pending = WalletLive.Display.from(
            CurrencyViewWire(code: "USD", rate: 1, committed: false, pending: "CNY")
        )
        let waiting = WalletLive.balance(balance, display: pending, fallback: base)
        #expect(waiting.state == .loading)
        #expect(waiting.integer == nil && waiting.decimals == nil, "a figure was drawn before the currency was known")
        #expect(waiting.currency == "CNY", "the label names the stored choice, never USD first")

        // The holdings' worth waits too, at its own line.
        let rows = WalletLive.assetRows(balance, display: pending)
        guard case .pending = rows[0].fiat else {
            Issue.record("the row's worth was drawn in dollars first: \(rows[0].fiat)")
            return
        }
        #expect(rows[0].balance == "100", "the token amount is not money in a currency: it stays")
        #expect(WalletLive.networkHoldings(balance, display: pending).isEmpty)

        // Nothing stored (a first launch): no code to name, and still no figure.
        let first = WalletLive.balance(
            balance, display: .from(CurrencyViewWire(code: "USD", rate: 1, committed: false)), fallback: base
        )
        #expect(first.state == .loading && first.currency.isEmpty)
        // Before the machine has answered at all, the live app waits as well.
        #expect(!WalletLive.Display.live(nil).settled)
        #expect(WalletLive.balance(balance, display: .live(nil), fallback: base).integer == nil)
        // A wallet that holds nothing is not "$0" in the meantime either.
        #expect(WalletLive.balance(view(total: 0), display: pending, fallback: base).state == .loading)

        // Committed: the figure, once, in the person's money.
        let committed = WalletLive.balance(
            balance,
            display: .from(CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)),
            fallback: base
        )
        #expect(committed.state == .normal)
        #expect(committed.currency == "CNY")
        #expect(committed.integer == "¥9,821")

        // Hidden stays hidden: the mask is not a figure, and nothing moves.
        let hidden = view(total: nil, tokens: balance.tokens, hidden: true)
        #expect(WalletLive.balance(hidden, display: pending, fallback: base).state == .hidden)
    }

    /// A surface with no currency machine behind it — a drawing, a test — is
    /// dollars, settled, as before.
    @Test func noCurrencyMachineIsSettledDollars() {
        #expect(WalletLive.Display.from(nil).settled)
        #expect(WalletLive.Display.usd.settled)
        #expect(WalletLive.balance(view(total: 1_000), fallback: base).integer == "$1,000")
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
            timestamp: timestamp, dayStartMs: dayStartMs, txHash: "0xabc", batch: nil,
            // The core's second line for a transfer (spec 093): whom it came
            // from or went to.
            subtitle: counterparty.map { address in
                [direction == .in ? .from(address: address, name: alias) : .to(address: address, name: alias)]
            } ?? [.network(chainId: 1)]
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
        ]).rows, loc: loc, hidden: false)

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
        ]).rows, loc: loc, hidden: false)
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
