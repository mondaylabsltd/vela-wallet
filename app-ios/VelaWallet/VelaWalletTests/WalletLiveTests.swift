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

    /// Hiding is by construction, not by masking downstream: a hidden balance
    /// has no digits to leak into an accessibility label.
    @Test func ahiddenBalanceCarriesNoFigureAtAll() {
        let model = WalletLive.balance(view(total: 1_000, hidden: true), fallback: base)
        #expect(model.state == .hidden)
        #expect(model.integer == nil)
        #expect(model.decimals == nil)
    }
}
