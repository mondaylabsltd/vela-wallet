//
//  SettingsFixturesTests.swift
//  VelaWalletTests
//
//  Spec 023 gates: every settings key resolves through the real engine in all
//  15 locales, the state inventory covers every mock in `design/settings/`,
//  each state builds, and the numbers a reviewer would compare against the
//  PNGs are pinned.
//
//  Same shape as `ContactsFixturesTests`, which is what keeps the four
//  clients' fixture canons from drifting: they are checked the same way.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SettingsFixturesTests {
    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - State inventory

    @Test func stateInventoryCoversEveryMock() {
        let states = SettingsStateId.allCases
        let st = states.filter { $0.rawValue.hasPrefix("st") }.count
        let sr = states.filter { $0.rawValue.hasPrefix("sr") }.count
        #expect(states.count == 29)
        #expect(st == 22)
        #expect(sr == 7)
    }

    @Test func everyStateBuildsAndSaysWhichOneItIs() {
        for state in SettingsStateId.allCases {
            #expect(SettingsFixtures.build(state, loc: loc).state == state)
        }
    }

    @Test func eachPickerMockOpensItsOwnSheet() {
        let pairs: [(SettingsStateId, SettingsOverlay)] = [
            (.st2, .accounts), (.st3, .signOut), (.st4, .language), (.st5, .currency),
            (.st6, .numberFormat), (.st7, .dateFormat), (.st8, .timeFormat),
            (.st13b, .clearCaches), (.st15, .feedback), (.st16, .eraseDevice),
        ]
        for (state, overlay) in pairs {
            #expect(SettingsFixtures.build(state, loc: loc).overlay == overlay)
        }
    }

    @Test func eachSubPageMockOpensItsOwnPage() {
        let pairs: [(SettingsStateId, SettingsPage)] = [
            (.st9, .networks), (.st9b, .networkDetail), (.st10, .addNetwork),
            (.st11, .rpcProviders), (.st12, .endpoints), (.st13, .storage), (.st14, .about),
        ]
        for (state, page) in pairs {
            #expect(SettingsFixtures.build(state, loc: loc).page == page)
        }
    }

    @Test func rescueStatesSitOnTheWalletTab() {
        let rescueStates = SettingsStateId.allCases.filter { $0.rawValue.hasPrefix("sr") }
        for state in rescueStates {
            #expect(SettingsFixtures.build(state, loc: loc).rescue)
        }
    }

    // MARK: - Canon numbers, pinned against the PNGs

    @Test func st9ListsEightNetworksWithTheCustomOneLast() {
        let model = SettingsFixtures.build(.st9, loc: loc)
        #expect(model.networks.count == 8)
        let last = model.networks.last
        #expect(last?.name == "X Layer")
        #expect(last?.removable == true)
        // A custom network has no latency to show.
        #expect(last?.badge == nil)
    }

    @Test func st9bFlagsTheChainIdMismatch() {
        #expect(SettingsFixtures.build(.st9b, loc: loc).networkDetail.callout != nil)
        #expect(SettingsFixtures.build(.st1, loc: loc).networkDetail.callout == nil)
    }

    @Test func compatibilityVerdictsShowAllFourChecks() {
        let ok = SettingsFixtures.build(.st10b, loc: loc).addNetwork
        let bad = SettingsFixtures.build(.st10c, loc: loc).addNetwork
        let okMarks = ok.checks.map(\.ok)
        let badMarks = bad.checks.map(\.ok)
        #expect(okMarks == [true, true, true, true])
        // Missing contracts — the P-256 precompile's row ticks: a chain
        // without it is a different refusal, with no Chain Setup (PR 3).
        #expect(badMarks == [true, false, true, false])
        #expect(bad.secondaryUrl == "https://getvela.app/chain-setup?chain=48900")
        // The failing state offers a way forward, not a greyed-out CTA.
        #expect(ok.primary != nil)
        #expect(bad.primary == nil)
        #expect(bad.secondary != nil)
        #expect(bad.recheck != nil)
    }

    @Test func storageAccountsForTwoPointFourMegabytes() {
        let storage = SettingsFixtures.build(.st13, loc: loc).storage
        #expect(storage.amount == "2.4")
        #expect(storage.unit == "MB")
        #expect(storage.summary.contains("216"))
        let counts = storage.groups.map(\.items.count)
        let hasGroupAction = storage.groups.map { $0.action != nil }
        let userDataDestructive = storage.groups[0].items.allSatisfy(\.destructive)
        let cachesReversible = storage.groups[1].items.allSatisfy { !$0.destructive }
        let fractionSum = storage.segments.reduce(0) { $0 + $1.fraction }
        #expect(counts == [4, 3, 1])
        // Only the cache group offers a clear-them-all action.
        #expect(hasGroupAction == [false, true, false])
        // User data and connections clear destructively; caches do not.
        #expect(userDataDestructive)
        #expect(cachesReversible)
        #expect(abs(fractionSum - 1) < 0.0001)
    }

    @Test func sr2IsOfflineAndSr2bIsRestored() {
        let failing = SettingsFixtures.build(.sr2, loc: loc).rpcFix
        let restored = SettingsFixtures.build(.sr2b, loc: loc).rpcFix
        #expect(failing.badge.tone == .error)
        #expect(failing.callout.tone == .warning)
        #expect(failing.providers.count == 4)
        #expect(restored.badge.tone == .ok)
        #expect(restored.callout.tone == .success)
        // Nothing left to go and get once it works.
        #expect(restored.providers.isEmpty)
        #expect(restored.report == nil)
    }

    @Test func sr3TellsRateLimitingFromADeadRpc() {
        let detail = SettingsFixtures.build(.sr3, loc: loc).balanceDetail
        // Quiet, resolves itself, no button.
        #expect(detail.pending[0].tone == .neutral)
        #expect(detail.pending[0].action == nil)
        // Loud, does not resolve itself, offers 立即重试.
        #expect(detail.pending[1].tone == .error)
        #expect(detail.pending[1].action != nil)
    }

    @Test func st3bAddsThePendingUploadWarning() {
        #expect(SettingsFixtures.build(.st3, loc: loc).signOutSheet.callout == nil)
        #expect(SettingsFixtures.build(.st3b, loc: loc).signOutSheet.callout?.tone == .warning)
    }

    @Test func destructiveConfirmsAreRedAndReversibleOnesAreNot() {
        let model = SettingsFixtures.build(.st1, loc: loc)
        #expect(model.signOutSheet.danger)
        #expect(model.eraseSheet.danger)
        // Clearing a cache is reversible; it rebuilds itself.
        #expect(!model.clearCachesSheet.danger)
    }

    // MARK: - Copy

    @Test func everySettingsKeyResolvesInEveryShippedLocale() {
        // The manifest is read off the fixture surface: if a key stops
        // resolving, the engine echoes it and this catches the echo.
        for tag in Loc.supported {
            let localised = Loc(overrideTag: tag, preferredLanguages: [])
            for state in SettingsStateId.allCases {
                let model = SettingsFixtures.build(state, loc: localised)
                #expect(!model.title.contains("settings."))
                for section in model.sections {
                    for row in section.rows {
                        let echoesKey = row.title.contains(".")
                        let empty = row.title.isEmpty
                        #expect(!echoesKey)
                        #expect(!empty)
                    }
                }
                #expect(!model.storage.summary.isEmpty)
                #expect(!model.about.footer.contains("about."))
            }
        }
    }

    @Test func languageEndonymsAreDataNotCopy() {
        let zh = SettingsFixtures.build(.st4, loc: loc)
        let en = SettingsFixtures.build(.st4, loc: Loc(overrideTag: "en", preferredLanguages: []))
        // Row 0 is 跟随系统 and IS translated; the rest are endonyms and read
        // the same whichever locale the app is in.
        let zhEndonyms = zh.languageSheet.rows.dropFirst().map(\.label)
        let enEndonyms = en.languageSheet.rows.dropFirst().map(\.label)
        #expect(zhEndonyms == enEndonyms)
        #expect(SettingsFixtures.localeEndonyms.count == 15)
    }

    @Test func theSignedInIdentityReplacesOnlyTheActiveAccount() {
        let model = SettingsFixtures.build(.st2, loc: loc)
            .withIdentity(name: "kimik3", address: "0xABC", display: "0xABC…def")
        #expect(model.account.name == "kimik3")
        #expect(model.accountsSheet.rows[0].name == "kimik3")
        // The other two stay fixtures: the core exposes no account list yet,
        // and inventing one would be the screen lying about how many wallets
        // this person has.
        #expect(model.accountsSheet.rows[1].name == "旅行基金")
    }
}
