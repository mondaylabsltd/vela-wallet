//
//  PreferencesCodecTests.swift
//  VelaWalletTests
//
//  One preference, one meaning (spec 072 T034).
//
//  The five display choices are read through the core's codec (`prefsRead`)
//  and an older shell's spellings are rewritten once at launch
//  (`prefsMigrations`).
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct PreferencesCodecTests {

    private func fresh() -> (UserDefaults, VelaStore) {
        let defaults = UserDefaults(suiteName: "vela.tests.prefs.\(UUID().uuidString)")!
        return (defaults, VelaStore(defaults: defaults))
    }

    private func booted(_ store: VelaStore) -> Preferences {
        let prefs = Preferences(store: store)
        prefs.boot()
        return prefs
    }

    // MARK: - Migration

    /// Android wrote `auto` for the theme, `system` for the language and kept
    /// the text size inside `vela.localePrefs`. Once migrated the store holds
    /// the shared spellings, and they read as what Android meant.
    @Test func androidsSpellingsAreMigratedOnceAndReadAsMeant() {
        let (defaults, store) = fresh()
        store.writeString(VelaStore.Key.theme, "auto")
        store.writeString(VelaStore.Key.language, "system")
        store.writeString(
            VelaStore.Key.localePrefs,
            #"{"numberFormat":"space_comma","dateFormat":"ymd_slash","timeFormat":"h24","textScale":4}"#
        )

        Preferences.migrate(store)

        #expect(defaults.string(forKey: VelaStore.Key.theme) == "system")
        #expect(defaults.string(forKey: VelaStore.Key.language) == "auto")
        #expect(defaults.string(forKey: VelaStore.Key.textScale) == "large")
        #expect(store.readObject(VelaStore.Key.localePrefs)["textScale"] == nil,
                "the text size stays inside the formats record")

        let prefs = booted(store)
        #expect(prefs.theme == .system)
        #expect(prefs.language == "auto")
        #expect(prefs.textScale == .large)
        #expect(prefs.numberFormat == .spaceComma)
        #expect(prefs.dateFormat == .ymdSlash)
        #expect(prefs.timeFormat == .h24)

        // Once: a second launch finds nothing to do.
        let before = store.everyKey().filter { $0.hasPrefix("vela.") }.map { ($0, store.rawValue($0) ?? "") }
        Preferences.migrate(store)
        let after = store.everyKey().filter { $0.hasPrefix("vela.") }.map { ($0, store.rawValue($0) ?? "") }
        #expect(Dictionary(uniqueKeysWithValues: before) == Dictionary(uniqueKeysWithValues: after))
    }

    /// The desktop's `vela.formats {number,date,time}` becomes the shared
    /// record, and the old key goes.
    @Test func theDesktopsFormatsRecordBecomesTheSharedOne() {
        let (defaults, store) = fresh()
        store.writeString(VelaStore.Key.legacyFormats, #"{"number":"dot_comma","date":"iso","time":"h12"}"#)

        // Read correctly even before the migration runs…
        let unmigrated = booted(store)
        #expect(unmigrated.numberFormat == .dotComma)

        Preferences.migrate(store)
        #expect(defaults.object(forKey: VelaStore.Key.legacyFormats) == nil)
        let record = store.readObject(VelaStore.Key.localePrefs)
        #expect(record["numberFormat"] as? String == "dot_comma")
        #expect(record["dateFormat"] as? String == "iso")
        #expect(record["timeFormat"] as? String == "h12")
        let prefs = booted(store)
        #expect(prefs.dateFormat == .iso)
        #expect(prefs.timeFormat == .h12)
    }

    /// A value this build does not ship reads as the default and is LEFT —
    /// a newer build's choice survives a trip through this one.
    @Test func aNewerBuildsValueIsLeftAlone() {
        let (defaults, store) = fresh()
        store.writeString(VelaStore.Key.theme, "sepia")
        store.writeString(VelaStore.Key.textScale, "huge")

        Preferences.migrate(store)

        #expect(defaults.string(forKey: VelaStore.Key.theme) == "sepia")
        #expect(defaults.string(forKey: VelaStore.Key.textScale) == "huge")
        let prefs = booted(store)
        #expect(prefs.theme == .system)
        #expect(prefs.textScale == .standard)
    }

    /// The formats record is written as the core writes it, three fields and
    /// no more — so no shell grows it a field the others do not read.
    @Test func theFormatsRecordIsTheCoresShape() {
        let (defaults, store) = fresh()
        let prefs = booted(store)
        prefs.setNumberFormat(.indian)
        prefs.setTimeFormat(.h12)
        #expect(defaults.string(forKey: VelaStore.Key.localePrefs)
                == prefsLocaleJson(numberFormat: "indian", dateFormat: "auto", timeFormat: "h12"))
    }

    /// The slider's six stops are the core's, name for name and factor for
    /// factor — every text style multiplies by them.
    @Test func theTextSizeStopsAreTheCores() {
        let core = prefsTextScaleLevels()
        #expect(core.map(\.name) == VelaWallet.TextScaleLevel.allCases.map(\.rawValue))
        for (level, stop) in zip(VelaWallet.TextScaleLevel.allCases, core) {
            #expect(abs(Double(level.factor) - stop.factor) < 0.0001, "\(level)")
        }
    }
}
