//
//  TokenReadsTests.swift
//  VelaWalletTests
//
//  The unit conversion that cost desktop a defect, and the ABI encoder that
//  replaces a fourth hand-roll.
//
//  `balance_dashboard.rs:298` requires a **human decimal string, never a JSON
//  number**. Desktop's 031 wrote raw units there and it was invisible for as
//  long as prices were `None` — the first real price would have multiplied a
//  total by 10^18. That is what the first group of tests is for.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct TokenReadsTests {

    // MARK: - Raw units → human decimal

    /// The golden Safe's actual Gnosis balance, as the pool returned it.
    ///
    /// Not a made-up number: `0xa8867319d2da000` is what
    /// `theGoldenSafesGnosisBalanceComesBackThroughThePool` printed, and
    /// 0.75897 xDAI is what desktop's 031 measured independently.
    @Test func theGoldenSafesBalanceScalesToTheFigureTwoClientsAgreeOn() {
        #expect(TokenReads.scaled(hex: "0xa8867319d2da000", decimals: 18) == "0.75897")
    }

    /// Whole units keep no trailing point, and sub-unit values keep their
    /// leading zero — both are how the figure is read aloud.
    @Test func theDecimalPointLandsWhereItShould() {
        #expect(TokenReads.scaled(hex: "0xde0b6b3a7640000", decimals: 18) == "1")
        #expect(TokenReads.scaled(hex: "0x1", decimals: 18) == "0.000000000000000001")
        #expect(TokenReads.scaled(hex: "0x0", decimals: 18) == "0")
        // USDC is 6 decimals, and its whole units are the common case.
        #expect(TokenReads.scaled(hex: "0xf4240", decimals: 6) == "1")
        #expect(TokenReads.scaled(hex: "0x1e8480", decimals: 6) == "2")
        // A token with no decimals at all is a real thing.
        #expect(TokenReads.scaled(hex: "0x2a", decimals: 0) == "42")
    }

    /// **The reason this is string arithmetic and not `Double`.**
    ///
    /// A `Double` carries 15–16 significant digits. This balance needs 21, and
    /// going through a `Double` would round somebody's money — quietly, in the
    /// direction the hardware happens to prefer.
    @Test func aBalanceTooBigForADoubleSurvivesExactly() {
        // 123456789012345678901 wei = 123.456789012345678901 ether.
        let huge = "0x6B14E9F812F366C35"
        #expect(TokenReads.scaled(hex: huge, decimals: 18) == "123.456789012345678901")

        // What the naive implementation would have produced.
        let viaDouble = Double(UInt64("6B14E9F812F366C35".prefix(15), radix: 16) ?? 0)
        #expect(String(viaDouble) != "123.456789012345678901")
    }

    @Test func junkScalesToNothingRatherThanZero() {
        #expect(TokenReads.scaled(hex: "0x", decimals: 18) == nil)
        #expect(TokenReads.scaled(hex: "0xzz", decimals: 18) == nil)
        #expect(TokenReads.scaled(bytes: Data(), decimals: 18) == nil)
    }

    /// A 32-byte `balanceOf` return, as it actually arrives from a multicall.
    @Test func a32ByteReturnScalesLikeAQuantity() {
        var padded = Data(repeating: 0, count: 24)
        padded.append(contentsOf: [0x0a, 0x88, 0x67, 0x31, 0x9d, 0x2d, 0xa0, 0x00])
        #expect(TokenReads.scaled(bytes: padded, decimals: 18) == "0.75897")
    }

    // MARK: - The ABI encoder, through the bridge

    /// `balanceOf(address)` is `0x70a08231` with a left-padded argument. The
    /// selector is derived from the signature in Rust, so a typo would be a
    /// parse error rather than a call that reverts on-chain.
    @Test func balanceOfEncodesThroughTheBridge() throws {
        let data = try erc20EncodeBalanceOf(
            ownerHex: "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        )
        #expect(data.count == 36)
        #expect(data.hexString.hasPrefix("70a08231"))
        #expect(data.hexString.hasSuffix("88cca0eedbf2c4426110bbfc998f048689266894"))
    }

    /// `aggregate3((address,bool,bytes)[])` is a dynamic array of tuples with a
    /// dynamic member — the shape the bridge's `abi_encode_*` primitives cannot
    /// express, and the reason there is a Rust encoder at all.
    @Test func aggregate3EncodesADynamicArrayOfTuples() throws {
        let owner = try erc20EncodeBalanceOf(
            ownerHex: "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        )
        let calldata = try multicall3EncodeAggregate3(calls: [
            Multicall3Call(target: "0xcA11bde05977b3631167028862bE2a173976CA11",
                           allowFailure: true, callData: owner),
            Multicall3Call(target: "0xdAC17F958D2ee523a2206206994597C13D831ec7",
                           allowFailure: true, callData: owner),
        ])
        #expect(calldata.hexString.hasPrefix("82ad56cb"), "not the aggregate3 selector")
        // Two 36-byte calls, each padded to 64, plus heads and offsets: the
        // exact length is the encoder's business, but it must be well past what
        // a single call would occupy.
        #expect(calldata.count > 4 + 32 * 8)
    }

    /// Garbage is **refused**, not decoded into a plausible empty list.
    ///
    /// The property Swift owns here. The Rust side already proves that a
    /// reverted entry keeps its slot (`multicall::tests::
    /// a_failed_entry_keeps_its_slot`, encoded by alloy rather than by hand —
    /// an earlier version of this test hand-wrote the ABI offsets and only
    /// proved that I cannot).
    ///
    /// What matters on this side is the failure direction: `readTokenBalances`
    /// treats an undecodable answer as **failed**, never as "no tokens", because
    /// "this address holds nothing" is a claim and a malformed response is not
    /// evidence for it.
    @Test func anUndecodableAnswerIsRefusedRatherThanReadAsEmpty() {
        #expect((try? multicall3DecodeAggregate3(data: Data([0x01, 0x02, 0x03]))) == nil)
        #expect((try? multicall3DecodeAggregate3(data: Data())) == nil)
        // A well-formed 32-byte word that is not an array either.
        #expect((try? multicall3DecodeAggregate3(
            data: Data(hexString: String(repeating: "ff", count: 32))!
        )) == nil)
    }

    // MARK: - Tempo

    /// Tempo has no native coin, so no native balance is read for it — and the
    /// guard is the chain's declared gas model, never a "that number looks too
    /// big" threshold, which would also reject a genuine whale.
    @Test func tempoIsMarkedSoItsNativeBalanceIsNeverRead() {
        #expect(ChainCatalog.meta(4_217)?.gasModel == .tempo)
        #expect(ChainCatalog.chains.filter { $0.gasModel == .tempo }.count == 1)
    }
}
