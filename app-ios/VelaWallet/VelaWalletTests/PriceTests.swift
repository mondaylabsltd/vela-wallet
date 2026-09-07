//
//  PriceTests.swift
//  VelaWalletTests
//
//  What a coin is worth — the decode, the alias table, and the one decision
//  this client does not make.
//
//  The ladder itself is `vela-core`'s (`choose_native_price`), reached through
//  the bridge. The tests here pin the two things Swift genuinely owns: turning
//  a Chainlink `latestRoundData()` word into a number, and knowing that
//  Gnosis' xDAI is priced by the DAI feed.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct PriceTests {

    /// A `latestRoundData()` return: `(roundId, answer, startedAt, updatedAt,
    /// answeredInRound)`. Only the first two words matter here, and the answer
    /// is the SECOND — reading the first would price ETH at a round number
    /// nobody can spend.
    private func roundData(answer: String) -> Data {
        let round = String(repeating: "0", count: 63) + "1"
        let padded = String(repeating: "0", count: 64 - answer.count) + answer
        return Data(hexString: round + padded)!
    }

    // MARK: - Decoding a feed

    @Test func theAnswerIsTheSecondWordScaledByTheFeedsDecimals() {
        // 250,012,000,000 at 8 decimals — ETH at $2,500.12.
        let price = Prices.chainlinkAnswer(roundData(answer: "3a35e05f00"), decimals: 8)
        #expect(price == 2_500.12)
    }

    /// PHP's feed carries 18 decimals where most carry 8. Assuming 8 would
    /// report a rate 10^10 out — a number that still formats plausibly, which
    /// is the worst kind of wrong on a money screen.
    @Test func aFeedsOwnDecimalsAreApplied() {
        let eighteen = Prices.chainlinkAnswer(roundData(answer: "0de0b6b3a7640000"), decimals: 18)
        #expect(eighteen == 1)
        let eight = Prices.chainlinkAnswer(roundData(answer: "0de0b6b3a7640000"), decimals: 8)
        #expect(eight == 10_000_000_000)
    }

    /// A negative answer is not a price at any scale. Read unsigned — which is
    /// what taking the word at face value does — `-100` becomes ~1.16e77.
    @Test func aNegativeAnswerIsNoPriceRatherThanAnEnormousOne() {
        let negative = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff9c"
        #expect(Prices.chainlinkAnswer(roundData(answer: negative), decimals: 8) == nil)
    }

    @Test func aZeroAnswerAndAShortReplyAreBothNoPrice() {
        #expect(Prices.chainlinkAnswer(roundData(answer: "0"), decimals: 8) == nil)
        #expect(Prices.chainlinkAnswer(Data(repeating: 0, count: 32), decimals: 8) == nil)
        #expect(Prices.chainlinkAnswer(Data(), decimals: 8) == nil)
    }

    @Test func aFailedDecimalsReadFallsBackToEight() {
        let eighteen = Data(hexString: String(repeating: "0", count: 62) + "12")!
        #expect(Prices.feedDecimals(eighteen) == 18)
        // Nothing usable — 8 is what web assumes, and it is the common case.
        #expect(Prices.feedDecimals(Data()) == 8)
        #expect(Prices.feedDecimals(Data(repeating: 0, count: 32)) == 8)
    }

    // MARK: - The alias table

    /// Polygon's coin is POL and Chainlink still calls the feed MATIC; Gnosis'
    /// xDAI is priced by the DAI feed. Every built-in chain's symbol either
    /// resolves or is honestly absent — an unpriced coin is a drawn state, a
    /// MISPRICED one is somebody's balance.
    @Test func theNativeSymbolsResolveThroughTheAliasTable() {
        let prices = ["ETH": 2_500.0, "BNB": 600.0, "MATIC": 0.4, "AVAX": 25.0, "DAI": 1.0]
        #expect(Prices.mainnetPrice(symbol: "ETH", in: prices) == 2_500)
        #expect(Prices.mainnetPrice(symbol: "POL", in: prices) == 0.4)
        #expect(Prices.mainnetPrice(symbol: "xDAI", in: prices) == 1)
        // Tempo's gas coin IS a dollar.
        #expect(Prices.mainnetPrice(symbol: "USD", in: [:]) == 1)
        // Monad has no feed anywhere. `nil`, never a guess.
        #expect(Prices.mainnetPrice(symbol: "MON", in: prices) == nil)
    }

    /// Every chain with a local feed is a chain the catalog knows — a feed
    /// keyed to a chain id nobody routes is dead configuration.
    @Test func everyLocalFeedBelongsToAKnownChain() {
        for chainId in Prices.nativeFeeds.keys {
            #expect(ChainCatalog.meta(chainId) != nil, "feed for unknown chain \(chainId)")
        }
        // Polygon is deliberately absent: the MATIC→POL migration left no
        // working local feed, and the mainnet MATIC feed covers it instead.
        #expect(Prices.nativeFeeds[137] == nil)
        #expect(Prices.nativeFeeds[100] != nil, "Gnosis lost its DAI/USD feed")
    }

    // MARK: - The ladder, which is not ours

    /// The verdict comes from `vela-core` through the bridge. These assert the
    /// SEAM, not the rules: that the shell forwards what it is given and reads
    /// back what it is told.
    @Test func theLadderIsAskedRatherThanReimplemented() {
        // A local feed outranks the Ethereum-mainnet fallback.
        #expect(Prices.choose(local: 0.999, mainnet: 1.0) == 0.999)
        // A local feed that decoded to nothing falls through.
        #expect(Prices.choose(local: nil, mainnet: 2_500) == 2_500)
        // Nothing anywhere is `nil` — never 0, which would render as "$0.00"
        // over coins somebody holds.
        #expect(Prices.choose(local: nil, mainnet: nil) == nil)
    }

    /// The DEX rung is absent in this cut and passed as `nil` rather than
    /// approximated. If a later cut starts filling it, this test is where the
    /// change announces itself.
    @Test func theDexRungIsAbsentNotApproximated() {
        let chosen = chooseNativePrice(dex: nil, chainlinkLocal: 1.0, chainlinkEth: nil)
        #expect(chosen.source == "chainlinkLocal")
        #expect(Prices.choose(local: 1.0, mainnet: nil) == chosen.price)
    }
}
