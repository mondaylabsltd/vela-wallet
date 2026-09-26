//
//  SettingsTruthTests.swift
//  VelaWalletTests
//
//  Two places settings showed the fixture's facts as the device's own
//  (founder, 2026-09-26): 关于 carried two conflicting versions on one row, and
//  the feedback sheet's "what will be sent" was `v1.0.0 (6ab8f) · iOS 26.0 ·
//  zh` whatever the phone was. Neither may reach a live screen again.
//

import Foundation
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct SettingsTruthTests {
    private let loc = Loc(overrideTag: "en")
    private let fixtureVersion = "1.0.0"
    private let fixtureCommit = "6ab8f"

    private func live() -> SettingsScreenModel {
        SettingsLive.withAbout(
            version: BuildInfo.version, commit: BuildInfo.commit, networkCount: 12,
            on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
    }

    /// The home row shows ONE version — BuildInfo's — in its value slot, and
    /// no subtitle beside it.
    @Test func theAboutRowShowsExactlyOneVersionAndItIsTheBuilds() throws {
        let row = try #require(live().sections.flatMap(\.rows).first { $0.id == "about" })
        let expected = loc.t(I18nKeys.SettingsUi.aboutSubtitle, vars: ["version": BuildInfo.version])
        #expect(row.value == expected, "value: \(row.value ?? "nil")")
        #expect(row.subtitle == nil, "a second version line: \(row.subtitle ?? "")")
        if BuildInfo.version != fixtureVersion {
            #expect(!(row.value ?? "").contains(fixtureVersion), "the fixture's version reached the live row")
        }
        // Exactly one version-shaped figure on the row.
        let text = [row.title, row.subtitle, row.value].compactMap { $0 }.joined(separator: " ")
        let versions = text.matches(of: /\d+\.\d+(\.\d+)?/).count
        #expect(versions == 1, "\(versions) versions on one row: \(text)")
    }

    /// The About page's own line is the build's version and commit.
    @Test func theAboutPageNamesTheBuild() {
        let page = live().about
        #expect(page.version.contains(BuildInfo.version))
        #expect(page.version.contains(BuildInfo.commit))
        #expect(!page.version.contains(fixtureCommit))
    }

    private func facts(unreachable: [String] = []) -> SettingsLive.FeedbackFacts {
        SettingsLive.FeedbackFacts(
            version: BuildInfo.version, commit: BuildInfo.commit,
            platform: "iOS \(UIDevice.current.systemVersion)",
            language: loc.resolvedLanguage, unreachable: unreachable
        )
    }

    /// The preview is THIS device: the build, the OS, the language in use, the
    /// networks the pool could not reach — and no line with no real source.
    @Test func theFeedbackPreviewIsThisDevice() {
        let base = SettingsFixtures.build(.st15, loc: loc)
        let lines = SettingsLive.withFeedback(facts(unreachable: ["Gnosis"]), on: base, loc: loc).feedback.previewLines
        let k = I18nKeys.SettingsUi.self
        #expect(lines == [
            "\(loc.t(k.bugPreviewVersion)): v\(BuildInfo.version) (\(BuildInfo.commit))",
            "\(loc.t(k.bugPreviewPlatform)): iOS \(UIDevice.current.systemVersion)",
            "\(loc.t(k.bugPreviewLanguage)): en",
            "\(loc.t(k.bugPreviewRpc)): Gnosis",
        ])
        // The fixture's figures are nowhere, and the failures line — no source
        // on iOS — is not drawn as "none".
        let joined = lines.joined(separator: "\n")
        #expect(!joined.contains(fixtureCommit))
        #expect(!joined.contains(loc.t(k.bugPreviewFailures)))
        // Nothing unreachable says so in the corpus's word.
        let calm = SettingsLive.withFeedback(facts(), on: base, loc: loc).feedback.previewLines
        #expect(calm.last == "\(loc.t(k.bugPreviewRpc)): \(loc.t(k.bugPreviewNone))")
    }

    /// A network somebody named after its URL, or an address, is redacted.
    @Test func thePreviewRedactsURLsAndAddresses() {
        let lines = SettingsLive.withFeedback(
            facts(unreachable: ["https://rpc.example/key123", "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]),
            on: SettingsFixtures.build(.st15, loc: loc), loc: loc
        ).feedback.previewLines
        let rpc = lines.last ?? ""
        #expect(!rpc.contains("key123"))
        #expect(!rpc.contains("88cCA0"))
        #expect(rpc.contains("[url]") && rpc.contains("[address]"), "\(rpc)")
    }
}
