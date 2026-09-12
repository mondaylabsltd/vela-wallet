//
//  Loc.swift
//  VelaWallet
//
//  The ONLY i18n touchpoint (FR-009): wraps vela-core's engine (uniffi
//  bindings), resolves the device language with the same semantics the RN
//  and web apps use (src/i18n/shared.ts — D6), and loads the bundled
//  runtime catalogs (Localization/Catalogs, synced from assets/i18n).
//
//  Failure model: a missing key or dead engine returns the key itself —
//  the visible failure signal mandated by FR-005. A catalog that fails to
//  load leaves the engine on the English fallback, never a mixed screen.
//

import Foundation
import Observation
import VelaCore

@Observable
final class Loc {
    /// The 15 supported locales — mirrors `vela_core::i18n::resolve::SUPPORTED`
    /// and `src/i18n/shared.ts#SUPPORTED_LANGUAGES`.
    static let supported: [String] = [
        "en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id", "tr",
        "es-MX", "pt-BR", "fr", "de", "ru", "it",
    ]

    private let engine: I18n?
    /// The engine-resolved active language (single source of truth).
    private(set) var resolvedLanguage: String = "en"

    init(
        overrideTag: String? = ProcessInfo.processInfo.environment["VELA_LANG"],
        preferredLanguages: [String] = Locale.preferredLanguages
    ) {
        guard let en = Loc.catalogData("en"), let engine = try? I18n(fallbackJson: en) else {
            // Dead engine: every t() echoes its key (visible failure, FR-005).
            self.engine = nil
            return
        }
        self.engine = engine

        let candidate = overrideTag ?? Self.mapPreferredLanguage(preferredLanguages.first ?? "en")
        if candidate != "en" {
            let state = try? engine.changeLanguage(lng: candidate)
            let active = state?.resolvedLanguage ?? "en"
            if active != "en", let data = Loc.catalogData(active) {
                if (try? engine.loadCatalog(lang: active, json: data)) != nil {
                    resolvedLanguage = active
                    return
                }
            }
            // Catalog unavailable → fall back to English cleanly.
            _ = try? engine.changeLanguage(lng: "en")
        }
        resolvedLanguage = "en"
    }

    /// Resolve a translation. Missing keys echo the key (FR-005).
    func t(_ key: String, vars: [String: String] = [:]) -> String {
        guard let engine else { return key }
        let opts = TOptions(
            count: nil,
            context: nil,
            defaultValue: nil,
            lng: nil,
            ordinal: false,
            vars: vars.map { TVar(name: $0.key, value: $0.value) }
        )
        return (try? engine.t(key: key, opts: opts)) ?? key
    }

    // MARK: - Language detection (D6 — shared.ts semantics)

    /// Maps a BCP-47 preferred-language tag onto the supported set, with the
    /// exact base-language semantics of `shared.ts#detectSystemLanguage`:
    /// zh script/region handling, es→es-MX, pt→pt-BR, legacy in→id, exact
    /// match, base match, otherwise en.
    static func mapPreferredLanguage(_ tag: String) -> String {
        let language = Locale.Language(identifier: tag)
        var code = language.languageCode?.identifier.lowercased() ?? "en"
        let script = language.script?.identifier
        let region = language.region?.identifier.uppercased()

        if code == "in" { code = "id" } // legacy Indonesian tag

        if code == "zh" {
            let traditional = script == "Hant"
                || (script == nil && ["TW", "HK", "MO"].contains(region ?? ""))
            if !traditional { return "zh" }
            return (region == "HK" || region == "MO") ? "zh-HK" : "zh-TW"
        }
        if code == "es" { return "es-MX" } // only Spanish variant shipped
        if code == "pt" { return "pt-BR" } // only Portuguese variant shipped

        if let region, supported.contains("\(code)-\(region)") {
            return "\(code)-\(region)"
        }
        if supported.contains(code) { return code }
        return "en"
    }

    // MARK: - Bundled catalogs

    private static func catalogData(_ lang: String) -> Data? {
        guard let url = Bundle.main.url(forResource: lang, withExtension: "json") else { return nil }
        return try? Data(contentsOf: url)
    }
}
