//
//  Prices.swift
//  VelaWallet
//
//  What a coin is worth, and who is allowed to decide.
//
//  Ported from `app-web/vela-wallet/src/lib/services/price-service.ts` (the
//  Ethereum-mainnet Chainlink batch and its alias table) and the per-chain
//  `NATIVE_CHAINLINK_FEEDS` half of `wallet-api.ts`. The **ladder** is ported
//  from nowhere: `chooseNativePrice` is the core's, exported by
//  `vela-core-uniffi/src/lib.rs` (spec 041 promoted it for every native
//  client; this cut's own copy was folded into that one).
//
//  ## Why the ladder is not in this file
//
//  Because it decides whose number wins when two sources disagree, and that is
//  the difference between a balance and a wrong balance. `choose_native_price`
//  has existed in `balance_dashboard.rs` since spec 017 while every platform
//  re-decided around it; web opened the door in 025 and this is the same door.
//  The (0.5, 2.0) sanity band exists because one near-empty X Layer pool quoted
//  WOKB at ~$5 against a real ~$81 — a shell that re-implements that band is a
//  second opinion nobody diffed.
//
//  ## The DEX rung is absent, and says so
//
//  Web's first rung is a DEX quote per chain (a router, a wrapped native, the
//  chain's stables — `fetchChainTokens` master data this client does not have
//  yet). Until it lands, `dex` is passed as `nil` rather than approximated, so
//  the ladder falls to Chainlink honestly and a missing rung never becomes an
//  invented price.
//
//  ## No price is `nil`, and never zero
//
//  `price_usd: nil` renders as the drawn "couldn't be priced" treatment. A zero
//  would render as `$0.00`, which tells somebody their coins are worthless.
//

import Foundation
import VelaCore

@MainActor
final class Prices {

    /// Chainlink feed proxies on **Ethereum mainnet**. Immutable proxies — the
    /// aggregator behind them rotates, the address does not.
    private static let mainnetFeeds: [String: String] = [
        "ETH": "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419",
        "BNB": "0x14e613AC691a42F21B17a6Dc7232f070FF175d25",
        "MATIC": "0x7bAC85A8a13A4BcD8abb3eB7d6b4d632c5a57676",
        "AVAX": "0xFF3EEb22B5E3dE6e705b44749C2559d704923FD7",
        "DAI": "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9",
    ]

    /// The wallet's native symbol → the feed's name for it. Polygon's coin is
    /// `POL` and Chainlink still calls the feed `MATIC`; Gnosis' `xDAI` is
    /// priced by the `DAI` feed.
    nonisolated private static let symbolAliases: [String: String] = [
        "POL": "MATIC",
        "XDAI": "DAI",
    ]

    /// A native/USD feed on the chain **itself** — the ladder's `chainlink_local`
    /// rung, read in the same batch as that chain's balances.
    ///
    /// Polygon is deliberately absent: the MATIC→POL migration left no working
    /// local feed, and a stale one is worse than none.
    nonisolated static let nativeFeeds: [Int: String] = [
        1: "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419",
        56: "0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE",
        42_161: "0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612",
        10: "0x13e3Ee699D1909E989722E753853AE30b17e08c5",
        8_453: "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70",
        43_114: "0x0A77230d17318075983913bC2145DB16C7366156",
        100: "0x678df3415fc31947dA4324eC63212874be5a82f8",
    ]

    /// Feed answers are stale for minutes at a time; re-reading mainnet on
    /// every pull-to-refresh buys nothing and spends somebody's rate limit.
    private static let ttlMs: Double = 3 * 60 * 1000

    private let pool: RpcPool
    private var cached: (prices: [String: Double], atMs: Double)?

    init(pool: RpcPool) {
        self.pool = pool
    }

    // MARK: - The Ethereum-mainnet batch

    /// Every native coin's USD price Chainlink can state, in one batch on
    /// Ethereum mainnet. Cached three minutes.
    ///
    /// A failed batch answers with the **last** map rather than an empty one:
    /// the previous prices are the best thing anybody knows, and dropping them
    /// would turn a mainnet hiccup into twelve unpriced chains.
    func mainnetPrices() async -> [String: Double] {
        let now = Date().timeIntervalSince1970 * 1000
        if let cached, now - cached.atMs < Self.ttlMs { return cached.prices }

        guard let latestRound = Multicall.selector("latestRoundData()") else {
            return cached?.prices ?? [:]
        }
        let symbols = Self.mainnetFeeds.keys.sorted()
        let calls = symbols.compactMap { symbol in
            Self.mainnetFeeds[symbol].map { Multicall.call($0, latestRound) }
        }

        guard case .ok(let results) = await Multicall.aggregate3(
            chainId: 1, calls: calls, pool: pool
        ) else {
            return cached?.prices ?? [:]
        }

        var prices: [String: Double] = [:]
        for (index, symbol) in symbols.enumerated() where index < results.count {
            let result = results[index]
            guard result.success,
                  let usd = Self.chainlinkAnswer(result.returnData, decimals: 8), usd > 0
            else { continue }
            prices[symbol] = usd
        }

        guard !prices.isEmpty else { return cached?.prices ?? [:] }
        cached = (prices, now)
        return prices
    }

    /// A native symbol's price from the mainnet map — the ladder's
    /// `chainlink_eth` rung.
    nonisolated static func mainnetPrice(symbol: String, in prices: [String: Double]) -> Double? {
        let upper = symbol.uppercased()
        // A chain whose gas coin IS a USD stablecoin (Tempo's `USD`) is pegged
        // by definition. It never reaches a balance, because Tempo has no
        // native coin to hold — the peg is here so the ladder answers the same
        // question web answers.
        if upper == "USD" { return 1 }
        if let direct = prices[upper] { return direct }
        guard let alias = symbolAliases[upper] else { return nil }
        return prices[alias]
    }

    // MARK: - The verdict

    /// Which price to publish, decided by the core.
    ///
    /// `dex` is `nil` until the quote path lands — see the module note. The
    /// source string is the core's own rung name, logged verbatim so an
    /// unfamiliar one reads as "the core grew a rung" rather than being
    /// silently mislabelled.
    nonisolated static func choose(local: Double?, mainnet: Double?) -> Double? {
        chooseNativePrice(dex: nil, chainlinkLocal: local, chainlinkEth: mainnet).price
    }

    // MARK: - Decoding a feed

    /// `latestRoundData()` → the USD price, or `nil` when the feed said nothing
    /// usable.
    ///
    /// The answer is the **second** word (the first is `roundId`), an `int256`.
    /// A negative answer is not a price at any scale, so it answers `nil`
    /// rather than being read as a very large unsigned number — which is what
    /// reading the word unsigned would do.
    ///
    /// Scaling goes through `TokenReads.scaled`, in decimal string arithmetic:
    /// a `Double` carries 15–16 significant digits and an 18-decimal feed (PHP
    /// is one) needs more.
    nonisolated static func chainlinkAnswer(_ returnData: Data, decimals: Int) -> Double? {
        guard returnData.count >= 64 else { return nil }
        let word = returnData.subdata(in: 32..<64)
        guard let first = word.first, first < 0x80 else { return nil }
        guard let text = TokenReads.scaled(bytes: word, decimals: decimals),
              let value = Double(text), value.isFinite, value > 0
        else { return nil }
        return value
    }

    /// `decimals()` → the feed's scale. Chainlink's fiat feeds vary (most are
    /// 8, PHP is 18), so it is read rather than assumed; a failed read falls
    /// back to 8 exactly as web does.
    nonisolated static func feedDecimals(_ returnData: Data) -> Int {
        guard returnData.count >= 32,
              let text = TokenReads.scaled(bytes: returnData.subdata(in: 0..<32), decimals: 0),
              let value = Int(text), value > 0, value <= 36
        else { return 8 }
        return value
    }
}
