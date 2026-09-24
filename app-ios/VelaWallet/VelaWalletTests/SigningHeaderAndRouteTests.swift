//
//  SigningHeaderAndRouteTests.swift
//  VelaWalletTests
//
//  Founder review 2026-09-19, on the native sheets: the wallet's own request
//  wears the wallet's mark and name, a site gets its own icon with its initial
//  as the fallback — and a person can say WHERE their passkey is, which also
//  decides which key the ceremony is pinned to (the core's rule, run for real).
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SigningHeaderAndRouteTests {
    private var loc: Loc { Loc(overrideTag: "en", preferredLanguages: []) }

    @Test func aSiteGetsItsOwnIconOverItsInitialHttpsOnly() {
        #expect(SigningLive.siteIconUrls(origin: "https://app.uniswap.org") == [
            "https://app.uniswap.org/apple-touch-icon.png", "https://app.uniswap.org/favicon.ico",
        ])
        #expect(SigningLive.siteIconUrls(origin: "http://app.uniswap.org").isEmpty)
        #expect(SigningLive.siteIconUrls(origin: "not an origin").isEmpty)
    }

    @Test func signWithUsesTheCreateFlowsWordsAndMarksTheChoice() {
        var context = SigningLive.Context(
            loc: loc, chainName: "Ethereum", chainDot: .red, nativeSymbol: "ETH",
            walletName: "Mine", walletAddress: "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        )
        // Every value the core offers (spec 071 added the Trusted Signer).
        context.signMethods = SignPrefViewWire.initial?.offered ?? []
        let auto = SigningLive.signWith(context: context)
        #expect(auto.label == "Sign with")
        #expect(auto.value == "Automatic")
        #expect(auto.options.map(\.title)
                == ["Automatic", "This device", "Phone or tablet", "USB security key", "Trusted Signer"])
        #expect(auto.options.filter(\.selected).map(\.id) == ["auto"])
        #expect(!auto.open)

        context.signMethod = "security_key"
        context.signWithOpen = true
        let key = SigningLive.signWith(context: context)
        #expect(key.value == "USB security key")
        #expect(key.open)
    }

    @Test func theCorePinsTheKeyOfTheChosenKindAndAutoRoutesNothing() throws {
        let keys = #"[{"credential_id":"apple","transports":"hybrid,internal"},{"credential_id":"yubikey","transports":"nfc,usb"}]"#
        #expect(signRoute(deviceKeysJson: keys, method: "auto") == nil)
        let json = try #require(signRoute(deviceKeysJson: keys, method: "security_key"))
        let route = try #require(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: String])
        // The YubiKey, not the first key: a security key cannot answer for a
        // credential it does not hold.
        #expect(route["credential_id"] == "yubikey")
        #expect(route["transports"] == "usb,nfc,ble")
        #expect(route["method"] == "security_key")
    }

    @Test func feesReadToSixDecimalsRoundedUpNeverAsZero() {
        #expect(SendLive.feeFromBase("410400290875302", decimals: 18) == "0.000411")
        #expect(SendLive.feeFromBase("1270000", decimals: 6) == "1.27")
        #expect(SendLive.feeFromBase("123456789", decimals: 18) == "0.00000000013")
        #expect(SendLive.feeFromBase("0", decimals: 18) == "0")
    }
}
