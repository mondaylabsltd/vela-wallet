//
//  SwitcherCountTests.swift
//  VelaWalletTests
//
//  The account switcher's count (PR 3 final note F15).
//
//  `home.switcherAccountCount` is a plural family now
//  (`_one` / `_few` / `_many` / `_other`), and the number is the PLURAL
//  count: the core's engine chooses the form by the language's own rule.
//  Handed over as a plain variable, a wallet with one account read
//  "1 accounts · Total".
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SwitcherCountTests {
    private static let first = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let second = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"

    private func session(accounts count: Int) throws -> SessionView {
        let all: [[String: Any]] = [
            ["index": 0, "account": ["name": "Mine", "address": Self.first]],
            ["index": 1, "account": ["name": "Savings", "address": Self.second]],
        ]
        return try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.first,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": Array(all.prefix(count)),
        ])
    }

    /// The summary over `count` accounts. The currency is still on its way,
    /// so the line is the count alone (the total is a withheld surface).
    private func summary(_ count: Int, lang: String) throws -> String {
        let loc = Loc(overrideTag: lang, preferredLanguages: [])
        return SettingsLive.withAccounts(
            session: try session(accounts: count), balances: [], display: .waiting(for: nil),
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).accountsSheet.summary
    }

    @Test func oneAccountIsSingularAndTwoArePlural() throws {
        #expect(try summary(1, lang: "en") == "1 account · ")
        #expect(try summary(2, lang: "en") == "2 accounts · ")
        // es-MX has the same two forms; zh has one.
        #expect(try summary(1, lang: "es-MX") == "1 cuenta · ")
        #expect(try summary(2, lang: "es-MX") == "2 cuentas · ")
        #expect(try summary(1, lang: "zh") == "1 个账户 · ")
        #expect(try summary(2, lang: "zh") == "2 个账户 · ")
    }

    /// With the currency committed the total joins the count on the line.
    @Test func theTotalJoinsTheCount() throws {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let line = SettingsLive.withAccounts(
            session: try session(accounts: 1),
            balances: [BalanceCacheEntryWire(address: Self.first, usd: 12.5)], display: .usd,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).accountsSheet.summary
        #expect(line == "1 account · Total $12.50")
    }

    /// No locale echoes the retired bare key, at 1, 2 or 5 — and the drawn
    /// fixture resolves the family too.
    @Test func everyLocaleResolvesTheFamily() throws {
        for lang in ["en", "zh", "zh-TW", "zh-HK", "ja", "ko", "de", "fr", "es-MX", "it", "pt-BR", "ru", "tr", "vi", "id"] {
            let loc = Loc(overrideTag: lang, preferredLanguages: [])
            for count in [1, 2, 5] {
                let text = loc.t(I18nKeys.SettingsUi.accountsCount, count: count)
                #expect(!text.contains("switcherAccountCount"), "\(lang) echoes the key at \(count)")
                #expect(text.contains(String(count)), "\(lang) lost the number at \(count): \(text)")
            }
            let drawn = SettingsFixtures.build(.st1, loc: loc).accountsSheet.summary
            #expect(!drawn.contains("switcherAccountCount"), "\(lang): the drawn sheet echoes the key")
        }
    }
}
