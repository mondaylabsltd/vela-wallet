//
//  MultisigR12FixTests.swift
//  VelaWalletTests
//
//  Round 12's iOS findings, each on the words or model the screen draws:
//
//  - r12-1: "would fail" said a 0.01 USDC send "needs <0.000001" — the
//    shortfall's token read in 18 decimals when its facts were not on the
//    reason. The figures are the core's facts, else base units, "decimals
//    unknown"; never 18 assumed for a token.
//  - S4: the home's Activity says it is still looking until the account's
//    first incoming scan is back — never "No activity yet" before it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR12FixTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    /// Arc Testnet's USDC, its ERC-20 interface (6 decimals).
    private static let usdc = "0x3600000000000000000000000000000000000000"

    // MARK: - r12-1

    @Test func aShortfallInATokenNobodyReadIsSaidInBaseUnits() {
        let why = MsJSON(any: ["kind": "short", "token": Self.usdc, "have": "0", "need": "10000"])
        let said = words.whyFails(why, chainId: 5_042_002)
        #expect(said.contains("10000 base units · decimals unknown"), "\(said)")
        #expect(!said.contains("<0.000001"), "never a figure in 18 decimals: \(said)")
        #expect(!said.contains("0.00000000000001"), "\(said)")
    }

    @Test func aShortfallSaysTheCoresTokenFacts() {
        // The facts on the reason itself (the core's payload) …
        let object = MsJSON(any: [
            "kind": "short", "have": "0", "need": "10000",
            "token": ["token": Self.usdc, "symbol": "USDC", "decimals": 6],
        ])
        #expect(words.whyFails(object, chainId: 5_042_002) == "the multisig holds 0 USDC, and this needs 0.01 USDC")
        // … or beside the address.
        let flat = MsJSON(any: [
            "kind": "short", "token": Self.usdc, "symbol": "USDC", "decimals": 6,
            "have": "0", "need": "1000000000000",
        ])
        #expect(words.whyFails(flat, chainId: 5_042_002).contains("1,000,000 USDC"))
        // An amount that carries its own decimals (`{raw, decimals}`).
        let amounts = MsJSON(any: [
            "kind": "short", "token": Self.usdc,
            "have": ["raw": "280000", "decimals": 6], "need": ["raw": "500000", "decimals": 6],
        ])
        let said = words.whyFails(amounts, chainId: 5_042_002)
        #expect(said.contains("0.28") && said.contains("0.5"), "\(said)")
    }

    @Test func aShortfallInTheCoinIsInTheCoinsOwnUnit() {
        let why = MsJSON(any: ["kind": "short", "token": NSNull(), "have": "0", "need": "500000000000000000"])
        let said = words.whyFails(why, chainId: 100)
        #expect(said == "the multisig holds 0 xDAI, and this needs 0.5 xDAI", "\(said)")
    }

    @Test func anAmountWithUnknownDecimalsIsNeverAFriendlyFigure() {
        let facts = MsWords.TokenFacts(symbol: "TKN", decimals: nil)
        #expect(words.amount("1500000", facts) == "1500000 base units · decimals unknown TKN")
        #expect(words.amount("1500000", MsWords.TokenFacts(symbol: "TKN", decimals: 6)) == "1.5 TKN")
        #expect(words.facts(chainId: 100, token: Self.usdc).decimals == nil, "no facts read: none assumed")
        #expect(words.facts(chainId: 100, token: nil).decimals == ChainCatalog.coinDecimals)
    }

    @Test func aCustomTokenRecordWithoutDecimalsIsNotCountedIn18() {
        let stored: [String: Any] = ["contractAddress": Self.usdc, "chainId": 100, "symbol": "USDC", "name": "USD Coin"]
        #expect(CustomTokens.toWire(stored) == nil)
        var withDecimals = stored
        withDecimals["decimals"] = 6
        #expect((CustomTokens.toWire(withDecimals)?["decimals"] as? Int) == 6)
        #expect(CustomTokens.fromWire(["chain_id": 100, "contract_address": Self.usdc])["decimals"] == nil)
    }

    // MARK: - S4: Activity before the first scan

    @Test func activityIsLoadingUntilItsFirstScanIsBack() throws {
        let base = WalletFixtures.buildMobileState(.h1, loc: loc)
        let feed = try JSONDecoder().decode(
            FeedViewWire.self, from: Data(#"{"rows":[],"transactions":[],"new_item_id":null,"toast":null}"#.utf8)
        )
        let balance = BalanceViewWire(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            displayTotalUsd: 0, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        let looking = WalletLive.apply(balance, feed: feed, feedRead: false, on: base, loc: loc)
        #expect(looking.activitySection.mode == .loading, "never \"No activity yet\" before anybody looked")
        let looked = WalletLive.apply(balance, feed: feed, feedRead: true, on: base, loc: loc)
        #expect(looked.activitySection.mode == .empty)
        #expect(looked.activitySection.empty?.title == loc.t("home.emptyNoActivity"))
    }
}
