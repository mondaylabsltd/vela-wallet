//
//  VelaLogTests.swift
//  VelaWalletTests
//
//  The system log's promises (spec 082 RE11, FR-018, FR-019): the report's
//  ring holds eight, a Release build never writes a host, and no line ever
//  carries an address.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct VelaLogTests {

    @Test func theRingKeepsTheLastEightFailuresOldestFirst() {
        VelaLog.resetRecentFailures()
        defer { VelaLog.resetRecentFailures() }
        for index in 1...11 {
            VelaLog.failure(.browser, kind: "k\(index)")
        }
        let recent = VelaLog.recentFailures
        #expect(recent.count == VelaLog.ringCap)
        #expect(recent.first == "browser: k4")
        #expect(recent.last == "browser: k11")
    }

    @Test func aRingEntryIsScopeAndKindOnly() {
        VelaLog.resetRecentFailures()
        defer { VelaLog.resetRecentFailures() }
        VelaLog.failure(.relay, kind: "maybe_sent", "hash=\(VelaLog.short("0x" + String(repeating: "ab", count: 32)))")
        #expect(VelaLog.recentFailures == ["relay: maybe_sent"])
    }

    @Test func aReleaseBuildWritesAHostAsAToken() {
        VelaLog.releaseOverride = true
        defer { VelaLog.releaseOverride = nil }
        let token = VelaLog.host("App.Uniswap.org")
        #expect(token.hasPrefix("h-"))
        #expect(!token.contains("uniswap"))
        // One site, one token — whatever its case.
        #expect(token == VelaLog.host("app.uniswap.org"))
        #expect(token == "h-" + VelaLog.fnv1a("app.uniswap.org"))
        #expect(VelaLog.hostOf(url: "https://app.uniswap.org/swap?x=1").hasPrefix("h-"))
    }

    @Test func aDebugBuildWritesTheHostAndNeverThePath() {
        VelaLog.releaseOverride = false
        defer { VelaLog.releaseOverride = nil }
        #expect(VelaLog.hostOf(url: "http://192.168.50.9:8137/path?secret=1#frag") == "192.168.50.9:8137")
        #expect(VelaLog.hostOf(url: nil) == "-")
    }

    @Test func noAddressSurvivesAFormattedLine() {
        let address = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33"
        let hash = "0x" + String(repeating: "c6f3544f", count: 8)
        let line = VelaLog.format("from=\(address) op=\(hash) url=https://rpc.example/v3/SECRET\(address)")
        #expect(!line.contains(address))
        #expect(!line.lowercased().contains(address.lowercased().dropFirst(2)))
        #expect(!line.contains("SECRET"))
        #expect(line.contains("[address]"))
        #expect(line.contains("[url]"))
        // A long hash keeps ten digits, enough to find it in the relay's log.
        #expect(line.contains("0xc6f3544fc6…"))
        #expect(!line.contains(hash))
    }

    @Test func aShortHashIsTenDigits() {
        #expect(VelaLog.short("0xABCDEF0123456789") == "0xABCDEF0123")
        #expect(VelaLog.short(nil) == "-")
    }

    @Test func theReportShowsTheRingThroughTheRedaction() {
        let loc = Loc(overrideTag: "en")
        let lines = SettingsLive.withFeedback(
            SettingsLive.FeedbackFacts(
                version: "1", commit: "c", platform: "iOS", language: "en", unreachable: [],
                failures: ["browser: timeout", "relay: maybe_sent"]
            ),
            on: SettingsFixtures.build(.st15, loc: loc), loc: loc
        ).feedback.previewLines
        #expect(lines.last == "Recent failures: browser: timeout; relay: maybe_sent")

        let none = SettingsLive.withFeedback(
            SettingsLive.FeedbackFacts(version: "1", commit: "c", platform: "iOS", language: "en", unreachable: []),
            on: SettingsFixtures.build(.st15, loc: loc), loc: loc
        ).feedback.previewLines
        #expect(none.last?.hasPrefix("Recent failures: ") == true)
        #expect(none.last != "Recent failures: ")
    }
}
