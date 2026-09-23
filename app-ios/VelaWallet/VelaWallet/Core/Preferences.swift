//
//  Preferences.swift
//  VelaWallet
//
//  What a person chose about how this app looks and counts.
//
//  **No machine owns these, and this cut does not give them one.** They are
//  storage keys with a reader — and the SPELLINGS are the contract: web writes
//  `vela.theme`, `vela.language`, `vela.localePrefs` and `vela.textScale`,
//  Android writes the same, and a person who restores a backup onto another of
//  their own devices finds their choices intact. (`vela.avatarStyle` is
//  retired — spec 074: every avatar is the identicon — and the core's
//  migrations remove it.)
//
//  How a stored record READS is the core's (`vela_core::prefs`, spec 072):
//  four shells had written the same five choices four ways — Android's
//  `system` language and `auto` theme, its text size inside `localePrefs`, the
//  desktop's `vela.formats` — and each read only its own. `prefsRead` accepts
//  every spelling, and `prefsMigrations` rewrites the known older ones once,
//  at launch. An unrecognised value reads as the DEFAULT rather than as an
//  error: a torn record is a record, and refusing to start over a bad string
//  would be worse than showing somebody the standard size.
//

import Foundation
import VelaCore

enum ThemeChoice: String, CaseIterable { case system, light, dark }

/// Grouping + decimal marks. `auto` reads the device's conventions once.
enum NumberFormatKey: String, CaseIterable {
    case auto, commaDot = "comma_dot", dotComma = "dot_comma"
    case spaceComma = "space_comma", indian
}

/// Field order + separator.
enum DateFormatKey: String, CaseIterable {
    case auto, ymdSlash = "ymd_slash", mdySlash = "mdy_slash"
    case dmySlash = "dmy_slash", dmyDot = "dmy_dot", iso
}

enum TimeFormatKey: String, CaseIterable { case auto, h24, h12 }

/// The six stops of the A ——●—— A slider, named as every client names them.
///
/// The factors are the core's `prefs::TEXT_SCALE_LEVELS` (`prefsTextScaleLevels`),
/// so a phone and a browser mean the same thing by "large" — held as an enum
/// because every text style reads it, and `PreferencesCodecTests` fails the
/// moment the two drift.
enum TextScaleLevel: String, CaseIterable {
    case compact, small, standard, comfortable, large, xlarge

    var factor: CGFloat {
        switch self {
        case .compact: 0.82
        case .small: 0.91
        case .standard: 1
        case .comfortable: 1.1
        case .large: 1.22
        case .xlarge: 1.35
        }
    }
}

@MainActor
@Observable
final class Preferences {

    private(set) var theme: ThemeChoice = .system
    /// `auto` follows the device. A bare string, as the other clients store it.
    private(set) var language = "auto"
    private(set) var textScale: TextScaleLevel = .standard
    private(set) var numberFormat: NumberFormatKey = .auto
    private(set) var dateFormat: DateFormatKey = .auto
    private(set) var timeFormat: TimeFormatKey = .auto

    private let store: VelaStore
    private var booted = false

    init(store: VelaStore) {
        self.store = store
    }

    /// Read what is stored. Idempotent and synchronous — safe to call from
    /// every route's `.task`, because the second call is a no-op rather than a
    /// second read that could land after somebody has already chosen something.
    ///
    /// Through the core: every vocabulary check, default and older spelling is
    /// `prefsRead`'s, so a record Android or the desktop wrote reads here as it
    /// reads there.
    func boot() {
        guard !booted else { return }
        booted = true
        let read = prefsRead(entries: Self.entries(store))
        theme = ThemeChoice(rawValue: read.theme) ?? .system
        language = read.language
        textScale = TextScaleLevel(rawValue: read.textScale) ?? .standard
        numberFormat = NumberFormatKey(rawValue: read.numberFormat) ?? .auto
        dateFormat = DateFormatKey(rawValue: read.dateFormat) ?? .auto
        timeFormat = TimeFormatKey(rawValue: read.timeFormat) ?? .auto
    }

    /// Bring an older shell's spellings to the shared record — once per
    /// launch, before anything reads them.
    ///
    /// Safe to repeat: the core answers nothing to do for a store that
    /// already agrees, and leaves alone a value it does not know (a newer
    /// build's choice survives a trip through this one).
    static func migrate(_ store: VelaStore) {
        for write in prefsMigrations(entries: entries(store)) {
            store.writeString(write.key, write.value)
        }
    }

    /// The raw text under every key the codec reads — the desktop's old
    /// `vela.formats` and the retired `vela.avatarStyle` included, so their
    /// migrations can find them.
    private static func entries(_ store: VelaStore) -> [String: String] {
        let keys = [
            VelaStore.Key.theme, VelaStore.Key.language, VelaStore.Key.localePrefs,
            VelaStore.Key.retiredAvatarStyle, VelaStore.Key.textScale, VelaStore.Key.legacyFormats,
            // Spec 075's pairing service, retired 2026-09-23: the core
            // REMOVES both spellings, so the migration has to see them.
            VelaStore.Key.retiredClearSignerTunnel,
            VelaStore.Key.retiredClearSignerRelay,
        ]
        return keys.reduce(into: [:]) { entries, key in
            if let raw = store.rawValue(key) { entries[key] = raw }
        }
    }

    // MARK: - What the settings page sets

    func setTheme(_ value: ThemeChoice) {
        theme = value
        store.writeString(VelaStore.Key.theme, value.rawValue)
    }

    func setLanguage(_ tag: String) {
        language = tag
        store.writeString(VelaStore.Key.language, tag)
    }

    func setTextScale(_ value: TextScaleLevel) {
        textScale = value
        store.writeString(VelaStore.Key.textScale, value.rawValue)
    }

    func setNumberFormat(_ value: NumberFormatKey) {
        numberFormat = value
        writeLocalePrefs()
    }

    func setDateFormat(_ value: DateFormatKey) {
        dateFormat = value
        writeLocalePrefs()
    }

    func setTimeFormat(_ value: TimeFormatKey) {
        timeFormat = value
        writeLocalePrefs()
    }

    /// All three together, because they share ONE record — writing a partial
    /// object would drop the two a person did not just change. The record is
    /// the core's (`prefsLocaleJson`), so no shell can grow it a field the
    /// others do not read — which is how Android's text size came to live in
    /// here.
    private func writeLocalePrefs() {
        store.writeString(VelaStore.Key.localePrefs, prefsLocaleJson(
            numberFormat: numberFormat.rawValue,
            dateFormat: dateFormat.rawValue,
            timeFormat: timeFormat.rawValue
        ))
    }
}
