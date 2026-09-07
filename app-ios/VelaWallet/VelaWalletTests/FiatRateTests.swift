//
//  FiatRateTests.swift
//  VelaWalletTests
//
//  The two rungs of the display currency's rate waterfall, in the parts that
//  can be tested without a network: where a Chainlink fiat feed lives, and what
//  a swappable FX provider is allowed to send back.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct FiatRateTests {

    // MARK: - Chainlink's feeds

    /// EIP-137's own published vector. The whole feed lookup hangs off this
    /// hash: a namehash that is wrong by one byte resolves to no address, and
    /// the wallet silently falls back to a hard-coded list instead of asking
    /// the registry.
    @Test func namehashMatchesTheEip137Vector() {
        #expect(FiatRates.namehash("").hexString == String(repeating: "0", count: 64))
        #expect(FiatRates.namehash("eth").hexString
                == "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae")
    }

    @Test func aFeedIsNamedAfterItsCurrency() {
        #expect(FiatRates.feedName("GBP") == "gbp-usd.data.eth")
        #expect(FiatRates.isChainlinkFiat("gbp"))
        // VND has no on-chain feed — the configurable endpoint is what prices
        // it, and claiming otherwise would skip the rung that can.
        #expect(!FiatRates.isChainlinkFiat("VND"))
    }

    @Test func anAddressIsTheLowTwentyBytesOfTheWord() {
        let word = String(repeating: "0", count: 24) + "b49f677943bc038e9857d61e7d053caa2c1734c1"
        #expect(FiatRates.addressWord(Data(hexString: word)!)
                == "0xb49f677943bc038e9857d61e7d053caa2c1734c1")
        #expect(FiatRates.addressWord(Data(repeating: 0, count: 20)) == nil)
        #expect(FiatRates.isZeroAddress("0x" + String(repeating: "0", count: 40)))
        #expect(!FiatRates.isZeroAddress("0xb49f677943bc038e9857d61e7d053caa2c1734c1"))
    }

    // MARK: - The configurable endpoint

    /// Frankfurter's array and the `{ rates: {} }` object both, because "the
    /// endpoint is configurable" is only true if a second provider's shape
    /// actually parses.
    @Test func bothProviderShapesNormalise() {
        let array: [[String: Any]] = [
            ["base": "USD", "quote": "EUR", "rate": 0.92],
            ["base": "USD", "quote": "vnd", "rate": 25_000],
        ]
        let fromArray = FiatFx.normalize(array)
        #expect(fromArray?["EUR"] == 0.92)
        #expect(fromArray?["VND"] == 25_000, "a lowercase code must still be found")
        #expect(fromArray?["USD"] == 1)

        let object: [String: Any] = ["rates": ["EUR": 0.92, "JPY": "150.5"]]
        let fromObject = FiatFx.normalize(object)
        #expect(fromObject?["EUR"] == 0.92)
        #expect(fromObject?["JPY"] == 150.5, "a rate sent as a string is still a rate")
    }

    /// A body with no rate in it answers `nil` rather than `{ USD: 1 }`. The
    /// difference matters: the second would be cached for six hours as a
    /// working endpoint that can price exactly nothing.
    @Test func aRatelessBodyIsNoAnswerAtAll() {
        #expect(FiatFx.normalize([]) == nil)
        #expect(FiatFx.normalize(["rates": [:] as [String: Any]]) == nil)
        #expect(FiatFx.normalize(nil) == nil)
        #expect(FiatFx.normalize("not json at all") == nil)
    }

    /// Zero and negative rates are dropped on the way in. One that reached the
    /// core would multiply every figure on the screen to nothing.
    @Test func nonsensicalRatesAreDroppedOnTheWayIn() {
        let body: [String: Any] = ["rates": ["EUR": 0, "GBP": -1, "CHF": 0.88]]
        let rates = FiatFx.normalize(body)
        #expect(rates?["EUR"] == nil)
        #expect(rates?["GBP"] == nil)
        #expect(rates?["CHF"] == 0.88)
    }

    /// A stored map is read element by element. `as? [String: Double]` is
    /// all-or-nothing, and one field somebody wrote as a string would throw
    /// away every rate in the file — which is the map an offline start converts
    /// with.
    @Test func aPartlyMalformedStoredMapStillYieldsItsGoodRates() {
        let stored: [String: Any] = [
            "EUR": 0.92, "JPY": "150.5", "GBP": "not a number", "CHF": 0, "cny": 6.7,
        ]
        let rates = FiatRates.decodeRates(stored)
        #expect(rates["EUR"] == 0.92)
        #expect(rates["JPY"] == 150.5)
        #expect(rates["GBP"] == nil)
        #expect(rates["CHF"] == nil, "zero is not a rate")
        #expect(rates["CNY"] == 6.7, "a lowercase key must still be found")
        #expect(FiatRates.decodeRates(nil).isEmpty)
    }

    /// The three cache keys are the cross-client bytes, spelled once
    /// (FR-005). A second spelling is a wallet that cannot read what it wrote.
    @Test func theCacheKeysAreTheOnesWebWrites() {
        #expect(VelaStore.Key.fiatRates == "vela.fiatRates.v1")
        #expect(VelaStore.Key.fiatFeedAddrs == "vela.fiatFeedAddrs.v1")
        #expect(VelaStore.Key.fxRates == "vela.fxRates.v1")
    }

    /// The default endpoint is `network_admin`'s own constant. Two spellings of
    /// it is how a person's rates come from a service the settings screen never
    /// names.
    @Test func theDefaultEndpointMatchesTheCores() {
        #expect(FiatFx.defaultURL == "https://vela-currency.getvela.app/v2/rates?base=USD")
    }
}
