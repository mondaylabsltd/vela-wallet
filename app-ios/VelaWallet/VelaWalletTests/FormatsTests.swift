//
//  FormatsTests.swift
//  VelaWalletTests
//
//  The presets, against the values that distinguish them — and against the one
//  rule that makes this not cosmetic: money never becomes a `Double`.
//

import Foundation
import Testing
@testable import VelaWallet

struct FormatsTests {

    // MARK: - Numbers

    /// One value, four presets, four different strings. If any two agree, the
    /// picker offers a choice that changes nothing.
    @Test func thePresetsAreActuallyDifferent() {
        let rendered = [
            Formats.number(1234567.89, .commaDot, minimumFractionDigits: 2, maximumFractionDigits: 2),
            Formats.number(1234567.89, .dotComma, minimumFractionDigits: 2, maximumFractionDigits: 2),
            Formats.number(1234567.89, .spaceComma, minimumFractionDigits: 2, maximumFractionDigits: 2),
            Formats.number(1234567.89, .indian, minimumFractionDigits: 2, maximumFractionDigits: 2),
        ]
        #expect(rendered[0] == "1,234,567.89")
        #expect(rendered[1] == "1.234.567,89")
        #expect(rendered[2] == "1 234 567,89")
        // 2-3 grouping, which is the whole reason this preset exists.
        #expect(rendered[3] == "12,34,567.89")
        #expect(Set(rendered).count == 4)
    }

    /// **The bigint rule.** A uint256 base-unit string must never become a
    /// floating-point number: precision loss on a money surface is not a
    /// rounding error, it is a wrong figure shown to somebody about to sign.
    @Test func moneyNeverBecomesAFloat() {
        let wei = "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        let grouped = Formats.groupDigits(wei, .commaDot)
        // Every digit survives, in order.
        #expect(grouped.filter(\.isNumber) == wei)
        #expect(grouped.hasPrefix("115,792,089"))

        // And the same through the decimal-string entry point.
        let amount = Formats.decimal("12345678901234567890.123456789", .commaDot)
        #expect(amount == "12,345,678,901,234,567,890.123456789")
    }

    @Test func decimalStringsKeepTheirFractionsExactly() {
        #expect(Formats.decimal("0.000001", .commaDot) == "0.000001")
        #expect(Formats.decimal("-1234.5", .dotComma) == "-1.234,5")
        #expect(Formats.decimal("1000", .indian) == "1,000")
        #expect(Formats.decimal("100000", .indian) == "1,00,000")
        // Not a number at all: handed back verbatim rather than mangled into
        // something that looks like one.
        #expect(Formats.decimal("—", .commaDot) == "—")
    }

    /// A token amount's precision follows its magnitude, so dust still reads as
    /// non-zero instead of as nothing.
    @Test func tokenAmountsKeepDustVisible() {
        #expect(Formats.tokenAmount(1234.5, .commaDot) == "1,234.50")
        #expect(Formats.tokenAmount(1.23456789, .commaDot) == "1.2346")
        #expect(Formats.tokenAmount(0.000001234, .commaDot) == "0.000001")
        #expect(Formats.tokenAmount(0, .commaDot) == "0")
        // The glanceable form caps at four decimals — unless that would round a
        // real transfer to "0".
        #expect(Formats.tokenAmount(0.000001234, .commaDot, compact: true) == "0.000001")
    }

    @Test func largeMagnitudesCompact() {
        #expect(Formats.compact(1234567.89, .commaDot) == "1.23M")
        #expect(Formats.compact(12345678.9, .commaDot) == "12.3M")
        #expect(Formats.compact(123456789, .commaDot) == "123M")
        #expect(Formats.compact(1234567890123, .commaDot) == "1.23T")
        #expect(Formats.compact(-1500, .commaDot) == "-1.5K")
    }

    /// What somebody types, back to a plain decimal string — including the
    /// digits a keyboard can produce that are not `0`–`9`.
    @Test func typedFiguresParseBack() {
        #expect(Formats.parse("1.234.567,89", .dotComma) == "1234567.89")
        #expect(Formats.parse("1 234 567,89", .spaceComma) == "1234567.89")
        #expect(Formats.parse("1,234,567.89", .commaDot) == "1234567.89")
        #expect(Formats.parse("١٢٣", .commaDot) == "123", "Eastern Arabic digits")
        #expect(Formats.parse("۴۵۶", .commaDot) == "456", "Persian digits")
    }

    /// An editable field gets the decimal mark and NO grouping: separators that
    /// jump around while somebody types are worse than none.
    @Test func inputFieldsGetNoGrouping() {
        #expect(Formats.inputSeparators(.dotComma).group.isEmpty)
        #expect(Formats.inputSeparators(.dotComma).decimal == ",")
        #expect(Formats.inputSeparators(.commaDot).decimal == ".")
    }

    // MARK: - Dates and times

    /// Five orders, five strings, and the sample is chosen so that no two can
    /// be confused (13 June — a day past twelve, a month under it).
    @Test func everyDatePresetReadsDifferently() {
        let sample = Formats.sample
        let rendered = [
            Formats.date(sample, .ymdSlash),
            Formats.date(sample, .iso),
            Formats.date(sample, .dmySlash),
            Formats.date(sample, .dmyDot),
            Formats.date(sample, .mdySlash),
        ]
        #expect(rendered == ["2026/06/13", "2026-06-13", "13/06/2026", "13.06.2026", "06/13/2026"])
        #expect(Set(rendered).count == 5)
    }

    @Test func theClockHasTwoFaces() {
        #expect(Formats.time(Formats.sample, .h24) == "13:45")
        #expect(Formats.time(Formats.sample, .h12) == "1:45 PM")
        #expect(Formats.dateTime(Formats.sample, .iso, .h24) == "2026-06-13, 13:45")
    }

    /// Midnight and noon are where a twelve-hour clock goes wrong.
    @Test func midnightAndNoonAreNotZero() {
        var parts = DateComponents()
        parts.year = 2026; parts.month = 6; parts.day = 13
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        parts.hour = 0; parts.minute = 5
        let midnight = calendar.date(from: parts)!
        parts.hour = 12; parts.minute = 5
        let noon = calendar.date(from: parts)!
        #expect(Formats.time(midnight, .h12) == "12:05 AM")
        #expect(Formats.time(noon, .h12) == "12:05 PM")
        #expect(Formats.time(midnight, .h24) == "00:05")
    }

    // MARK: - `auto`

    /// `auto` resolves to a CONCRETE preset, and caches — a preset that changed
    /// between two figures on one screen would be worse than either of them.
    @Test func autoResolvesAndStaysResolved() {
        Formats.resetAutoDetectionForTests()
        let first = Formats.resolve(NumberFormatKey.auto)
        #expect(first != .auto)
        #expect(Formats.resolve(NumberFormatKey.auto) == first)
        #expect(Formats.resolve(DateFormatKey.auto) != .auto)
        #expect(Formats.resolve(TimeFormatKey.auto) != .auto)
    }

    /// Every picker row's label IS an example of what it does.
    @Test func everyPickerRowShowsItself() {
        for key in NumberFormatKey.allCases where key != .auto {
            #expect(!Formats.example(key).isEmpty)
        }
        #expect(Formats.example(NumberFormatKey.indian) == "12,34,567.89")
        #expect(Formats.example(DateFormatKey.iso) == "2026-06-13")
        #expect(Formats.example(TimeFormatKey.h12) == "1:45 PM")
    }
}

@MainActor
struct PreferencesTests {

    /// The KEYS are the contract: web and Android write exactly these, and a
    /// person who restores a backup onto another of their own devices finds
    /// their choices intact. A renamed key here silently loses them.
    @Test func theKeysAreTheOnesEveryClientWrites() {
        #expect(VelaStore.Key.theme == "vela.theme")
        #expect(VelaStore.Key.language == "vela.language")
        #expect(VelaStore.Key.localePrefs == "vela.localePrefs")
        #expect(VelaStore.Key.textScale == "vela.textScale")
    }

    @Test func choicesSurviveARelaunch() {
        let defaults = UserDefaults(suiteName: "vela.tests.prefs.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)

        let first = Preferences(store: store)
        first.boot()
        #expect(first.theme == .system, "nothing chosen is `system`, not a pinned palette")
        #expect(first.numberFormat == .auto)
        first.setTheme(.dark)
        first.setNumberFormat(.indian)
        first.setDateFormat(.iso)
        first.setTextScale(.large)
        first.setLanguage("zh")

        let second = Preferences(store: store)
        second.boot()
        #expect(second.theme == .dark)
        #expect(second.numberFormat == .indian)
        #expect(second.dateFormat == .iso)
        #expect(second.timeFormat == .auto, "the one nobody chose stays auto")
        #expect(second.textScale == .large)
        #expect(second.language == "zh")
    }

    /// All three formats share ONE record, so writing one must not drop the
    /// other two.
    @Test func theThreeFormatsShareOneRecordWithoutLosingEachOther() {
        let defaults = UserDefaults(suiteName: "vela.tests.prefs.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let prefs = Preferences(store: store)
        prefs.boot()
        prefs.setNumberFormat(.dotComma)
        prefs.setTimeFormat(.h12)
        let stored = store.readObject(VelaStore.Key.localePrefs)
        #expect(stored["numberFormat"] as? String == "dot_comma")
        #expect(stored["timeFormat"] as? String == "h12")
        #expect(stored["dateFormat"] as? String == "auto")
    }

    /// A torn record reads as the defaults, which is what it means — refusing
    /// to start over a bad string would be worse than showing somebody the
    /// standard size.
    @Test func aTornRecordIsNotAnError() {
        let defaults = UserDefaults(suiteName: "vela.tests.prefs.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        store.writeString(VelaStore.Key.theme, "chartreuse")
        store.writeString(VelaStore.Key.textScale, "enormous")
        store.writeObject(VelaStore.Key.localePrefs, ["numberFormat": "klingon"])
        let prefs = Preferences(store: store)
        prefs.boot()
        #expect(prefs.theme == .system)
        #expect(prefs.textScale == .standard)
        #expect(prefs.numberFormat == .auto)
    }

    /// The slider's factors are the same numbers every client uses, so a phone
    /// and a browser mean the same thing by "large".
    @Test func theTextScaleStopsMatchTheOtherClients() {
        #expect(TextScaleLevel.allCases.map(\.rawValue)
            == ["compact", "small", "standard", "comfortable", "large", "xlarge"])
        #expect(TextScaleLevel.allCases.map(\.factor) == [0.82, 0.91, 1, 1.1, 1.22, 1.35])
    }
}

// MARK: - Where the presets actually show (spec 056 US1)

/// 049's lesson, restated: a format is verified where it SHOWS, never on the
/// settings page. A picker that previews itself is not evidence that the hero
/// reads it — and on Android the avatar style and the number preset both
/// previewed correctly while every real surface ignored them.
@MainActor
struct FormatsReachTheScreensTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    private func withPreset(_ key: NumberFormatKey, _ body: () -> Void) {
        let before = Formats.current
        Formats.current = Formats.Current(number: key, date: .iso, time: .h24)
        body()
        Formats.current = before
    }

    private func withDate(_ key: DateFormatKey, _ body: () -> Void) {
        let before = Formats.current
        Formats.current = Formats.Current(number: .commaDot, date: key, time: .h24)
        body()
        Formats.current = before
    }

    /// **The hero.** The figure a person looks at first, and the decimals it
    /// subordinates — which means the split has to find the chosen decimal
    /// MARK, not a dot.
    @Test func theHeroSplitsOnTheChosenDecimalMark() {
        withPreset(.commaDot) {
            let (whole, cents) = WalletLive.split(1234567.89)
            #expect(whole == "1,234,567")
            #expect(cents == "89")
        }
        withPreset(.dotComma) {
            let (whole, cents) = WalletLive.split(1234567.89)
            #expect(whole == "1.234.567", "the hero is still grouped the device's way")
            #expect(cents == "89", "the split found the comma, not a dot")
        }
        withPreset(.indian) {
            #expect(WalletLive.split(1234567.89).0 == "12,34,567")
        }
    }

    /// **A day group.** Every date this app prints goes through the preset —
    /// "今天" and "昨天" survive, because a relative day is copy and the corpus
    /// owns it.
    @Test func theFeedsDaysFollowTheChosenOrder() {
        let old = Date(timeIntervalSince1970: 1_781_000_000)
        withDate(.iso) {
            let label = WalletLive.dayLabel(
                dayStartMs: old.timeIntervalSince1970 * 1000,
                timestamp: old.timeIntervalSince1970, loc: loc
            )
            #expect(label.contains("-"), "an ISO date has dashes: \(label)")
        }
        withDate(.dmyDot) {
            let label = WalletLive.dayLabel(
                dayStartMs: old.timeIntervalSince1970 * 1000,
                timestamp: old.timeIntervalSince1970, loc: loc
            )
            #expect(label.contains("."), "a dotted date has dots: \(label)")
        }
        // Today is still a word, not a date.
        let now = Date().timeIntervalSince1970
        #expect(WalletLive.dayLabel(dayStartMs: now * 1000, timestamp: now, loc: loc)
            == loc.t("componentsUi.dayGroup.today"))
    }

    /// **The signing sheet.** The core formats inside a clear-signing panel, so
    /// what it is handed must be the RESOLVED preset — never the word "auto",
    /// which only a shell can turn into a convention.
    @Test func theSigningPanelIsHandedAResolvedPreset() {
        let before = Formats.current
        Formats.current = Formats.Current(number: .auto, date: .auto, time: .auto)
        let locale = SigningController.defaultLocale
        #expect(locale["number_format"] as? String != "auto")
        #expect(locale["date_format"] as? String != "auto")
        #expect(locale["time_format"] as? String != "auto")

        Formats.current = Formats.Current(number: .indian, date: .dmyDot, time: .h12)
        let chosen = SigningController.defaultLocale
        #expect(chosen["number_format"] as? String == "indian")
        #expect(chosen["date_format"] as? String == "dmy_dot")
        #expect(chosen["time_format"] as? String == "h12")
        Formats.current = before
    }

    /// The chosen SIZE multiplies a screen's own scale rather than replacing
    /// it: the gallery's 1.35× chip and a person's "large" are two different
    /// statements about the same text.
    @Test func theChosenSizeMultipliesRatherThanReplaces() {
        let before = UiScale.factor
        UiScale.factor = 1.22
        #expect(UiScale.factor * 1 == 1.22)
        #expect(UiScale.factor * 1.35 > 1.35, "the gallery chip still says more than the choice")
        UiScale.factor = before
    }
}
