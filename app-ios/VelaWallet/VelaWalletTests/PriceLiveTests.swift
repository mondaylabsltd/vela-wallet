//
//  PriceLiveTests.swift
//  VelaWalletTests
//
//  The price path against the real world.
//
//  Behind the same compile flag as `NetworkAdminLiveTests` — these read
//  Ethereum mainnet, Gnosis and the fiat endpoint, so a flaky network must
//  never fail an unrelated change:
//
//      xcodebuild test -project VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'id=<simulator udid>' \
//        -only-testing:VelaWalletTests/PriceLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//
//  Every assertion is a **band**, never a pin. "ETH is between $100 and
//  $100,000" stays true for years and still catches the failures that matter —
//  a feed read off by 10^10, an unsigned decode of a negative answer, a rate
//  inverted the wrong way round. A pinned price would fail every day for the
//  one reason that is not a defect.
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct PriceLiveTests {

    private static let goldenSafe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func freshPool() -> (VelaStore, AccountStore, RpcPool) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        return (store, accounts, pool)
    }

    // MARK: - Chainlink

    /// The Ethereum-mainnet batch: five feeds, one `aggregate3`, through the
    /// pool. ETH's band is wide on purpose.
    @Test func theMainnetFeedsPriceTheNativeCoins() async {
        let (_, _, pool) = freshPool()
        let prices = await Prices(pool: pool).mainnetPrices()

        guard let eth = prices["ETH"] else {
            Issue.record("no ETH price came back: \(prices)")
            return
        }
        #expect(eth > 100 && eth < 100_000, "ETH at $\(eth) — a decode, not a market")
        // DAI is a dollar. If this one drifts, the decimals are wrong rather
        // than the peg being broken.
        if let dai = prices["DAI"] {
            #expect(dai > 0.5 && dai < 2, "DAI at $\(dai)")
        }
        print("[live] mainnet feeds: \(prices)")
    }

    /// The whole per-chain read, on the golden Safe: a balance AND a price, the
    /// price coming from Gnosis' own DAI/USD feed in the same batch.
    ///
    /// This is what phase 2b could not do — the home total read `0.00` under a
    /// warning triangle while real xDAI sat below it.
    @Test func theGoldenSafesXdaiIsPricedFromGnosisOwnFeed() async {
        let (_, _, pool) = freshPool()
        let mainnet = await Prices(pool: pool).mainnetPrices()
        let result = await TokenReads.read(
            address: Self.goldenSafe, chainId: 100, tokens: [], pool: pool,
            chainlinkPrices: mainnet
        )

        #expect(!result.failed, "Gnosis did not answer")
        guard let native = result.tokens.first else {
            Issue.record("no native holding came back")
            return
        }
        guard let price = native["price_usd"] as? Double else {
            Issue.record("xDAI came back unpriced: \(native)")
            return
        }
        #expect(price > 0.5 && price < 2, "xDAI at $\(price) — the peg, or a decode?")
        print("[live] golden Safe xDAI: \(native["balance"] ?? "?") at $\(price)")
    }

    /// A chain with no feed anywhere is unpriced, and that is the honest
    /// answer — not a zero, and not a borrowed number from another coin.
    @Test func aCoinNobodyPricesStaysUnpriced() async {
        let (_, _, pool) = freshPool()
        let mainnet = await Prices(pool: pool).mainnetPrices()
        let result = await TokenReads.read(
            address: Self.goldenSafe, chainId: 143, tokens: [], pool: pool,
            chainlinkPrices: mainnet
        )
        guard let native = result.tokens.first else { return }  // the chain may be down
        #expect(native["price_usd"] is NSNull, "MON was given a price from somewhere")
    }

    // MARK: - The fiat waterfall

    /// ENS resolution reaches the same address the hard-coded fallback names.
    ///
    /// The strongest check on `namehash` there is: the registry is asked for
    /// `eur-usd.data.eth` and hands back an address computed from a hash this
    /// client made itself. One wrong byte and the answer is nothing at all.
    @Test func theEuroFeedResolvesThroughEns() async {
        let (store, _, pool) = freshPool()
        let rates = FiatRates(store: store, pool: pool)
        guard let eur = await rates.rate("EUR") else {
            Issue.record("EUR did not price through Chainlink")
            return
        }
        // A euro has been worth between half and two dollars for its whole
        // existence.
        #expect(eur > 0.5 && eur < 2, "USD→EUR at \(eur) — inverted, or mis-scaled?")
        print("[live] USD→EUR (Chainlink): \(eur)")
    }

    /// The second rung answers for a currency the first cannot: HKD has no
    /// Chainlink feed, and the configurable endpoint quotes it.
    ///
    /// This test asked for **VND** first, on the strength of web's own comment
    /// ("~160 currencies incl. VND"). The deployed endpoint quotes thirty — the
    /// ECB set — and VND is not among them, so the test was wrong about the
    /// world rather than the code being wrong. The gap is recorded in
    /// results.md: the picker offers VND, and today it degrades to USD.
    @Test func theConfigurableEndpointPricesWhatChainlinkCannot() async {
        let (store, accounts, _) = freshPool()
        #expect(!FiatRates.isChainlinkFiat("HKD"))
        guard let hkd = await FiatFx(store: store, accounts: accounts).rate("HKD") else {
            Issue.record("the FX endpoint did not price HKD")
            return
        }
        // The peg has held between 7.75 and 7.85 since 2005; the band is wider.
        #expect(hkd > 5 && hkd < 10, "USD→HKD at \(hkd)")
        print("[live] USD→HKD (endpoint): \(hkd)")
    }

    /// What the endpoint can actually price, printed rather than asserted — the
    /// list is provider data and the picker is supposed to follow it.
    @Test func theEndpointsCurrencyListIsReported() async {
        let (store, accounts, _) = freshPool()
        let codes = await FiatFx(store: store, accounts: accounts).supportedCodes().sorted()
        #expect(codes.count > 1, "the endpoint quoted nothing at all")
        let offered = CurrencyCatalog.entries.map(\.code)
        let unpriceable = offered.filter { code in
            !codes.contains(code) && !FiatRates.isChainlinkFiat(code)
        }
        print("[live] endpoint quotes \(codes.count): \(codes)")
        print("[live] offered but unpriceable today: \(unpriceable)")
    }

    /// The machine's own answer, end to end: `resolve_rate` for a currency with
    /// a feed, through the executor the core talks to.
    @Test func theCurrencyMachineResolvesARealRate() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let executor = DisplayCurrencyExecutor(store: store, accounts: accounts, pool: pool)

        let reply = (try? CoreJSON.object(
            await executor.perform(["type": "resolve_rate", "code": "CNY"])
        )) ?? [:]
        guard let rate = (reply["rate"] as? NSNumber)?.doubleValue else {
            Issue.record("CNY came back unpriced: \(reply)")
            return
        }
        #expect(rate > 3 && rate < 15, "USD→CNY at \(rate)")
        print("[live] USD→CNY (waterfall): \(rate)")
    }
}

#endif
