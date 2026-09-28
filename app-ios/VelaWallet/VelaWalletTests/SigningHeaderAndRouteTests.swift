//
//  SigningHeaderAndRouteTests.swift
//  VelaWalletTests
//
//  Founder review 2026-09-19, on the native sheets: the wallet's own request
//  wears the wallet's mark and name, and a site gets its own icon with its
//  initial as the fallback.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SigningHeaderAndRouteTests {
    @Test func aSiteGetsItsOwnIconOverItsInitialHttpsOnly() {
        #expect(SigningLive.siteIconUrls(origin: "https://app.uniswap.org") == [
            "https://app.uniswap.org/apple-touch-icon.png", "https://app.uniswap.org/favicon.ico",
        ])
        #expect(SigningLive.siteIconUrls(origin: "http://app.uniswap.org").isEmpty)
        #expect(SigningLive.siteIconUrls(origin: "not an origin").isEmpty)
    }

    @Test func feesReadToSixDecimalsRoundedUpNeverAsZero() {
        #expect(SendLive.feeFromBase("410400290875302", decimals: 18) == "0.000411")
        #expect(SendLive.feeFromBase("1270000", decimals: 6) == "1.27")
        #expect(SendLive.feeFromBase("123456789", decimals: 18) == "0.00000000013")
        #expect(SendLive.feeFromBase("0", decimals: 18) == "0")
    }

    // MARK: - The header at 375 pt (spec 082 T117, RE13, RE7)

    /// A site whose name IS its host is said once — the core's
    /// `browserSiteLabel`, ignoring case; a real name keeps its host line.
    @Test func theHeaderSaysTheHostOnce() {
        let host = SigningHeaderView.label(name: "192.168.50.9:8137", host: "192.168.50.9:8137")
        #expect(host.name == "192.168.50.9:8137")
        #expect(host.hostLine == nil)
        #expect(SigningHeaderView.label(name: "APP.UNISWAP.ORG", host: "app.uniswap.org").hostLine == nil)
        let named = SigningHeaderView.label(name: "Uniswap", host: "app.uniswap.org")
        #expect(named.name == "Uniswap")
        #expect(named.hostLine == "app.uniswap.org")
    }

    /// The header never cuts a name, a host or the chain's name: the column
    /// outranks the chip, name and host wrap to two lines, the chip keeps
    /// its size — the part a spoofer controls is never the part hidden.
    @Test func theHeaderCutsNothing() throws {
        let source = try String(
            contentsOf: URL(fileURLWithPath: #filePath)
                .deletingLastPathComponent().deletingLastPathComponent()
                .appendingPathComponent("VelaWallet/Components/Signing/SigningAtoms.swift"),
            encoding: .utf8
        )
        let start = try #require(source.range(of: "struct SigningHeaderView"))
        let end = try #require(source.range(of: "// MARK: - Intent + sentence"))
        let header = String(source[start.lowerBound..<end.lowerBound])
        #expect(!header.contains("truncationMode"))
        #expect(!header.contains(".lineLimit(1)"), "no single-line cut of the name or host")
        #expect(header.contains(".lineLimit(2)"))
        #expect(header.contains(".layoutPriority(1)"))
        #expect(header.contains(".fixedSize()"), "the chain chip keeps its whole name")
        #expect(!header.contains("dapp.host != dapp.name"), "079's F14 copy is gone")
    }
}
