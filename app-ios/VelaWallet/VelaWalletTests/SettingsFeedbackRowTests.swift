//
//  SettingsFeedbackRowTests.swift
//  VelaWalletTests
//
//  The report sheet has a door (2026-09-27). SettingsScreen routed
//  `"feedback"` to `.feedback` for two specs while no section drew a row with
//  that id — so on the founder's iPhone there was nothing to tap
//  ("设置的关于下面，没有看到反馈按钮"). And a report always ends in something
//  the person can see: a sheet closed mid-send must not take the outcome
//  with it. The network is ALWAYS stubbed here.
//

import Foundation
import Testing
@testable import VelaWallet

/// `timeLimit`: some waits here are for a task the test itself started to
/// reach a point (`Waits.swift`); one that never does is a hang, reported here.
@MainActor
@Suite(.hangLimit)
struct SettingsFeedbackRowTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The LIVE home, built the way RootView builds it.
    private func liveHome() -> SettingsScreenModel {
        let defaults = UserDefaults(suiteName: "vela.tests.feedback-row.\(UUID().uuidString)")!
        let prefs = Preferences(store: VelaStore(defaults: defaults))
        prefs.boot()
        var model = SettingsLive.withPreferences(prefs, on: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        model = SettingsLive.withAbout(version: "0.9.4", commit: "abc1234", networkCount: 24, on: model, loc: loc)
        model = SettingsLive.withFeedback(
            SettingsLive.FeedbackFacts(version: "0.9.4", commit: "abc1234", platform: "iOS 26.5",
                                       language: "en", unreachable: []),
            on: model, loc: loc
        )
        return model
    }

    @Test func theLiveHomeHasAFeedbackRowRightAfterAbout() throws {
        let last = try #require(liveHome().sections.last)
        #expect(last.rows.map(\.id) == ["about", SettingsFixtures.feedbackRow])
        let row = try #require(last.rows.last)
        #expect(row.title == "Send feedback")
        #expect(row.subtitle == "Report a bug or share an idea")
        #expect(row.icon == .messageSquareText)
        #expect(row.trailing == .chevron)
    }

    @Test func theRowIsTranslated() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let row = try #require(SettingsFixtures.build(.st1, loc: zh).sections.flatMap(\.rows)
            .first { $0.id == SettingsFixtures.feedbackRow })
        #expect(!row.title.contains("settings."), "untranslated key: \(row.title)")
        #expect(row.title != "Send feedback")
    }

    @Test func selectingTheRowOpensTheReportSheet() {
        #expect(SettingsScreen.overlay(forRow: SettingsFixtures.feedbackRow) == .feedback)
    }

    /// Every row on the live home goes SOMEWHERE — a sheet, a page, or a
    /// callback — so the next row with a route and no door, or a door and no
    /// route, fails here.
    @Test func everyHomeRowIsRouted() {
        let pages: Set<String> = [
            "networks", "rpc-providers", "add-network", "endpoints", "storage", "about",
            SigningPagesPageModel.rowId,
        ]
        for row in liveHome().sections.flatMap(\.rows) {
            let routed = SettingsScreen.overlay(forRow: row.id) != nil
                || SettingsScreen.externalLink(forRow: row.id) != nil
                || pages.contains(row.id)
            #expect(routed, "row \(row.id) opens nothing")
        }
    }

    // MARK: - Every outcome is visible

    /// A report whose sheet was closed while sending still lands on the
    /// page-owned sender, and a reset (the next sheet opening) does not drop
    /// it; the toast then has something to say.
    @Test func aSendOutlivesItsSheetAndResetNeverDropsIt() async throws {
        let endpoint = StubEndpoint()
        endpoint.answer = .status(200, #"{"number":42,"url":"https://github.com/o/r/issues/42","deduped":false}"#)
        // Held open until the test lets it go. It was 150 ms of sleep, and on
        // a busy machine the report could land before the reset below ran —
        // the reset then met a finished report, not one on its way.
        var release: CheckedContinuation<Void, Never>?
        let held: BugReport.Transport = { request in
            await withCheckedContinuation { release = $0 }
            return try await endpoint.transport(request)
        }
        let sender = FeedbackSender(endpoint: "https://example.test/api/bug-report", transport: held)
        let sending = Task { await sender.send(what: "Send froze", steps: "", previewLines: ["Platform: iOS"], version: "0.9.4") }
        await Wait.until { release != nil }
        sender.reset()
        #expect(sender.sending, "reset dropped a report on its way")
        release?.resume()
        await sending.value
        #expect(sender.state == .filed(number: 42, url: "https://github.com/o/r/issues/42", deduped: false, screenshotsDropped: 0))

        let words = SettingsFixtures.build(.st15, loc: loc).feedback
        let toast = try #require(FeedbackOutcomeToast.Model.from(sender.state, words: words))
        #expect(toast.success)
        #expect(toast.title == words.successTitle)
        #expect(toast.actionTitle == words.viewIssue)
        #expect(toast.url == "https://github.com/o/r/issues/42")

        // The next sheet opens fresh, not on the old success.
        sender.reset()
        #expect(sender.state == .idle)
        #expect(sender.shots.isEmpty)
    }

    @Test func aFallbackToastOffersTheForm() throws {
        let words = SettingsFixtures.build(.st15, loc: loc).feedback
        let toast = try #require(FeedbackOutcomeToast.Model.from(
            .fallback(url: "https://github.com/x/new?template=bug.yml", screenshots: 2), words: words))
        #expect(!toast.success)
        #expect(toast.title == words.fallbackTitle)
        #expect(toast.actionTitle == words.openGithub)
        #expect(toast.url.hasPrefix("https://github.com/"))
        #expect(FeedbackOutcomeToast.Model.from(.idle, words: words) == nil)
        #expect(FeedbackOutcomeToast.Model.from(.sending, words: words) == nil)
    }
}
