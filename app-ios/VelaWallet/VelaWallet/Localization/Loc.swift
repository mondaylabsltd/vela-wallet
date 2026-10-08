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

        self.preferredLanguages = preferredLanguages
        // `VELA_LANG` PINS the language: it is how the screenshot sweep and the
        // acceptance tests ask for a specific one, and a stored preference that
        // overrode it would make every pinned run show the device's language
        // instead. Found the moment `apply` landed — the storage page came up
        // in English under `VELA_LANG=zh`.
        self.pinned = overrideTag != nil
        let candidate = overrideTag ?? Self.systemLanguage(preferredLanguages)
        adopt(candidate)
    }

    /// Whether an explicit tag was supplied at construction.
    private var pinned = false

    /// The device's own order, kept so `auto` can be re-resolved later without
    /// asking `Locale` again mid-session (it does not change while we run, and
    /// a test needs to be able to supply its own).
    private var preferredLanguages: [String] = []

    /// Adopt the stored language choice — `auto`, or one of `supported`.
    ///
    /// **This is what was missing.** `vela.language` was written by the
    /// settings page and read by nobody: not at launch, not on relaunch, not
    /// ever, so picking 日本語 changed one row's subtitle and no other word in
    /// the app. Android has read its preference since 047 and the web since
    /// 028; this is the same call on the third client.
    ///
    /// Safe to call repeatedly, and safe to call with the language already
    /// active — the engine is asked once and the catalog is loaded once.
    func apply(_ stored: String) {
        guard !pinned else { return }
        let wanted = stored == "auto" || stored.isEmpty
            ? Self.systemLanguage(preferredLanguages)
            : (Self.supported.contains(stored) ? stored : Self.mapPreferredLanguage(stored))
        guard wanted != resolvedLanguage else { return }
        adopt(wanted)
    }

    /// Ask the engine for a language and load its catalog, or fall back to
    /// English cleanly. A half-loaded catalog would be a mixed screen, which
    /// FR-005 forbids more strongly than it forbids the wrong language.
    private func adopt(_ candidate: String) {
        guard let engine else { return }
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
        } else {
            _ = try? engine.changeLanguage(lng: "en")
        }
        resolvedLanguage = "en"
    }

    /// Resolve a translation. Missing keys echo the key (FR-005).
    ///
    /// Reading `resolvedLanguage` here is deliberate and is the whole of the
    /// live-change mechanism: `@Observable` records the access, so **every
    /// view that renders a translated string depends on the language**, and
    /// changing it invalidates exactly those views. Without this line the
    /// engine would switch and the screen would keep the old words until
    /// something else happened to redraw it.
    func t(_ key: String, vars: [String: String] = [:]) -> String {
        _ = resolvedLanguage
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

    /// Resolve a PLURAL key: `count` chooses the form — `_one`, `_few`,
    /// `_many` or `_other`, by the language's CLDR rule, in the core — and
    /// fills `{{count}}`.
    ///
    /// `t(_:vars:)` cannot stand in for this: a `"count"` var fills the
    /// number as text and selects no form, so a plural key handed to it
    /// echoes (issue #409). Never pick the suffix here either — `count == 1`
    /// is not Russian's rule (2–4 is `_few`), nor Chinese's (one form).
    func t(_ key: String, count: Int, vars: [String: String] = [:]) -> String {
        _ = resolvedLanguage
        guard let engine else { return key }
        let opts = TOptions(
            count: Double(count),
            context: nil,
            defaultValue: nil,
            lng: nil,
            ordinal: false,
            vars: vars.map { TVar(name: $0.key, value: $0.value) }
        )
        return (try? engine.t(key: key, opts: opts)) ?? key
    }

    // MARK: - How long ago (the core's rule, issue 462)

    /// The core's compact relative time in the active language — "now",
    /// "2m", "3h", a short weekday within the week, else the date in the
    /// person's own format — for the hero's "Updated <ago>".
    ///
    /// `I18n::format_relative_time`, across uniffi: this shell ports none of
    /// it. The moment goes over in WHOLE seconds, the zone is this device's
    /// at that moment, and the date preset is the person's with `auto`
    /// already resolved, as the desktop hands them over.
    func relativeTime(atMs: Double, now: Date = Date(), timeZone: TimeZone = .current) -> String {
        let at = Date(timeIntervalSince1970: atMs / 1000)
        return relativeTime(
            tsSeconds: Int64((atMs / 1000).rounded(.down)),
            nowMs: Int64((now.timeIntervalSince1970 * 1000).rounded(.down)),
            utcOffsetMinutes: Int32(timeZone.secondsFromGMT(for: at) / 60),
            dateFormat: Formats.resolve(Formats.current.date).rawValue
        ) ?? Formats.date(at)
    }

    /// The same, with every input spelled as the core takes it — the route
    /// the conformance vectors replay. `nil` only from a dead engine.
    func relativeTime(
        tsSeconds: Int64, nowMs: Int64, utcOffsetMinutes: Int32, dateFormat: String
    ) -> String? {
        _ = resolvedLanguage
        return try? engine?.formatRelativeTime(
            tsSeconds: tsSeconds, nowMs: nowMs,
            utcOffsetMinutes: utcOffsetMinutes, dateFormat: dateFormat
        )
    }

    // MARK: - Language detection (the core's rule, spec 095)

    /// What "follow the system" means: the first of the person's preferred
    /// languages a shipped locale serves, else English — the core's
    /// `system_language`, shared with the desktop and Android. It walks the
    /// whole list: somebody whose first language Vela does not speak gets
    /// their second, where this used to read only the first.
    static func systemLanguage(_ preferred: [String]) -> String {
        i18nSystemLanguage(preferred: preferred)
    }

    /// One tag through the same rule (zh script/region, es→es-MX, pt→pt-BR,
    /// legacy in→id, language-region, language, otherwise en).
    static func mapPreferredLanguage(_ tag: String) -> String {
        systemLanguage([tag])
    }

    // MARK: - Bundled catalogs

    private static func catalogData(_ lang: String) -> Data? {
        guard let url = Bundle.main.url(forResource: lang, withExtension: "json") else { return nil }
        return try? Data(contentsOf: url)
    }
}
