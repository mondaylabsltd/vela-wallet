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
}
