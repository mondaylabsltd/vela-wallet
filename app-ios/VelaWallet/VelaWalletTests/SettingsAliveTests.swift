//
//  SettingsAliveTests.swift
//  VelaWalletTests
//
//  The settings page, as a page that does what it draws (spec 056 US2).
//
//  Every assertion here is about a control REACHING something. What each
//  control then changes is tested where it shows — `FormatsReachTheScreensTests`
//  — because a picker that previews itself is not evidence that the hero reads
//  it (049's lesson).
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SettingsAliveTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func preferences() -> Preferences {
        let defaults = UserDefaults(suiteName: "vela.tests.settings.\(UUID().uuidString)")!
        let prefs = Preferences(store: VelaStore(defaults: defaults))
        prefs.boot()
        return prefs
    }

    private func model(_ prefs: Preferences) -> SettingsScreenModel {
        SettingsLive.withPreferences(
            prefs, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        )
    }

    private func rows(_ model: SettingsScreenModel) -> [SettingsRowModel] {
        model.sections.flatMap(\.rows)
    }

    /// **The founder's ruling** (2026-09-12, Android 047): the address book has
    /// its own tab and the feedback sheet has its own doors. Neither is a row
    /// on the preferences page.
    @Test func theSettingsHomeHasNoContactsOrFeedbackRow() {
        let ids = rows(model(preferences())).map(\.id)
        #expect(!ids.contains("contacts"))
        #expect(!ids.contains("feedback"))
        // And the rows that DO belong are still there.
        #expect(ids.contains("language"))
        #expect(ids.contains("currency"))
        #expect(ids.contains("networks"))
    }

    /// Every row's right-aligned value is what is actually in force — a page
    /// that showed one thing in the row and another in the sheet would be two
    /// answers to one question.
    @Test func theRowsShowWhatIsInForce() {
        let prefs = preferences()
        prefs.setNumberFormat(.indian)
        prefs.setDateFormat(.dmyDot)
        prefs.setTimeFormat(.h12)
        let built = rows(model(prefs))

        #expect(built.first { $0.id == "number-format" }?.value == "12,34,567.89")
        #expect(built.first { $0.id == "date-format" }?.value == "13.06.2026")
        #expect(built.first { $0.id == "time-format" }?.value == "1:45 PM")
    }

    /// Exactly one row is selected in each sheet, and it is the one in force.
    @Test func eachSheetHasExactlyOneSelection() {
        let prefs = preferences()
        prefs.setNumberFormat(.spaceComma)
        prefs.setDateFormat(.iso)
        let built = model(prefs)

        for sheet in [built.numberSheet, built.dateSheet, built.timeSheet, built.languageSheet] {
            #expect(sheet.rows.filter(\.selected).count == 1, "\(sheet.title)")
        }
        #expect(built.numberSheet.rows.first { $0.selected }?.id == "space_comma")
        #expect(built.dateSheet.rows.first { $0.selected }?.id == "iso")
    }

    /// Each option's LABEL is a live example of itself. A picker whose rows all
    /// read the same is a picker nobody can choose from — and the drawn sheets
    /// keyed their rows by POSITION over hardcoded strings, which is fine for a
    /// picture and useless for a choice.
    @Test func everyOptionShowsWhatItWouldDo() {
        let built = model(preferences())
        let labels = built.numberSheet.rows.map(\.label)
        #expect(labels.count == NumberFormatKey.allCases.count)
        #expect(labels.contains("12,34,567.89"), "the Indian preset's own 2-3 grouping")
        #expect(labels.contains("1 234 567,89"))
        // `auto` resolves to one of the others, so its LABEL legitimately
        // repeats one — its note is what tells them apart.
        #expect(built.numberSheet.rows.first { $0.id == "auto" }?.note?.isEmpty == false)
        #expect(Set(labels.dropFirst()).count == labels.count - 1,
                "two named presets rendered the same sample")
        // And every row can be picked by NAME rather than by position.
        #expect(built.dateSheet.rows.map(\.id).contains("dmy_dot"))
        #expect(built.timeSheet.rows.map(\.id) == ["auto", "h24", "h12"])
    }

    /// `system` is what the drawn sheet calls "follow the device"; `auto` is
    /// what every client STORES. One mapping, in one place — and it survives a
    /// round trip.
    @Test func theLanguageIdsMapBothWays() {
        let prefs = preferences()
        #expect(prefs.language == "auto")
        #expect(model(prefs).languageSheet.rows.first { $0.selected }?.id == "system")

        prefs.setLanguage("ja")
        #expect(model(prefs).languageSheet.rows.first { $0.selected }?.id == "ja")
    }

    /// The three appearance controls read what is in force, including the
    /// slider's stop.
    @Test func theAppearanceControlsShowTheChoice() {
        let prefs = preferences()
        prefs.setTheme(.dark)
        prefs.setAvatarStyle(.initials)
        prefs.setTextScale(.xlarge)
        let built = model(prefs)
        #expect(built.theme.selected == "dark")
        #expect(built.avatar.selected == "initials")
        #expect(built.textScale.index == TextScaleLevel.allCases.count - 1)
        #expect(built.textScale.steps == TextScaleLevel.allCases.count)
    }
}
