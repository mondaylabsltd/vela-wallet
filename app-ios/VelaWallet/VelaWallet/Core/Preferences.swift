//
//  Preferences.swift
//  VelaWallet
//
//  What a person chose about how this app looks and counts.
//
//  **No machine owns these, and this cut does not give them one.** They are
//  storage keys with a reader — and the SPELLINGS are the contract: web writes
//  `vela.theme`, `vela.language`, `vela.localePrefs`, `vela.avatarStyle` and
//  `vela.textScale`, Android writes the same five, and a person who restores a
//  backup onto another of their own devices finds their choices intact.
//
//  Ported from `app-web/.../services/preferences.svelte.ts` and Android's
//  `core/data/Preferences.kt`. An unrecognised stored value reads as the
//  DEFAULT rather than as an error: a torn record is a record, and refusing to
//  start over a bad string would be worse than showing somebody the standard
//  size.
//

import Foundation

enum ThemeChoice: String, CaseIterable { case system, light, dark }
enum AvatarStyle: String, CaseIterable { case initials, identicon }

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
/// The factors are `src/constants/text-scale.ts` verbatim, so a phone and a
/// browser mean the same thing by "large".
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
    private(set) var avatarStyle: AvatarStyle = .identicon
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
    func boot() {
        guard !booted else { return }
        booted = true
        theme = ThemeChoice(rawValue: store.readString(VelaStore.Key.theme) ?? "") ?? .system
        avatarStyle = AvatarStyle(rawValue: store.readString(VelaStore.Key.avatarStyle) ?? "")
            ?? .identicon
        language = store.readString(VelaStore.Key.language) ?? "auto"
        textScale = TextScaleLevel(rawValue: store.readString(VelaStore.Key.textScale) ?? "")
            ?? .standard
        let locale = store.readObject(VelaStore.Key.localePrefs)
        numberFormat = NumberFormatKey(rawValue: locale["numberFormat"] as? String ?? "") ?? .auto
        dateFormat = DateFormatKey(rawValue: locale["dateFormat"] as? String ?? "") ?? .auto
        timeFormat = TimeFormatKey(rawValue: locale["timeFormat"] as? String ?? "") ?? .auto
    }

    // MARK: - What the settings page sets

    func setTheme(_ value: ThemeChoice) {
        theme = value
        store.writeString(VelaStore.Key.theme, value.rawValue)
    }

    func setAvatarStyle(_ value: AvatarStyle) {
        avatarStyle = value
        store.writeString(VelaStore.Key.avatarStyle, value.rawValue)
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
    /// object would drop the two a person did not just change.
    private func writeLocalePrefs() {
        store.writeObject(VelaStore.Key.localePrefs, [
            "numberFormat": numberFormat.rawValue,
            "dateFormat": dateFormat.rawValue,
            "timeFormat": timeFormat.rawValue,
        ])
    }
}
