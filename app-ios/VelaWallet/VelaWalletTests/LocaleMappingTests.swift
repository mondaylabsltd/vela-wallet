//
//  LocaleMappingTests.swift
//  VelaWalletTests
//
//  D6 fixtures — semantics of src/i18n/shared.ts#detectSystemLanguage.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct LocaleMappingTests {
    @Test(arguments: [
        // Simplified Chinese in all its spellings
        ("zh", "zh"), ("zh-CN", "zh"), ("zh-Hans-CN", "zh"), ("zh-Hans", "zh"),
        // Traditional Chinese: script/region routing
        ("zh-Hant", "zh-TW"), ("zh-TW", "zh-TW"), ("zh-Hant-TW", "zh-TW"),
        ("zh-HK", "zh-HK"), ("zh-Hant-HK", "zh-HK"), ("zh-Hant-MO", "zh-HK"), ("zh-MO", "zh-HK"),
        // Only one Spanish / Portuguese variant ships
        ("es", "es-MX"), ("es-AR", "es-MX"), ("es-MX", "es-MX"),
        ("pt", "pt-BR"), ("pt-PT", "pt-BR"), ("pt-BR", "pt-BR"),
        // Legacy Indonesian tag
        ("in", "id"), ("id", "id"), ("id-ID", "id"),
        // Base-language match
        ("fr-CA", "fr"), ("de-DE", "de"), ("en-GB", "en"), ("ru-RU", "ru"),
        // Unsupported → en
        ("ar", "en"), ("hi-IN", "en"), ("th", "en"),
    ])
    func mapsPreferredLanguage(fixture: (String, String)) {
        #expect(Loc.mapPreferredLanguage(fixture.0) == fixture.1,
                "\(fixture.0) should map to \(fixture.1)")
    }

    /// Spec 095: the app declares exactly the corpus's locales, as the core
    /// names them for Apple (`CFBundleLocalizations`) — and each one, chosen
    /// as the app's language in iOS Settings, resolves back to that corpus
    /// language, so the per-app Language row can never pick English by
    /// accident.
    @Test func theBundleDeclaresTheCorpusLocales() {
        let declared = Bundle.main.object(forInfoDictionaryKey: "CFBundleLocalizations") as? [String]
        #expect(declared == i18nAppleLocalizations())
        #expect(Bundle.main.object(forInfoDictionaryKey: "CFBundleDevelopmentRegion") as? String == "en")
        for (apple, corpus) in zip(i18nAppleLocalizations(), SettingsFixtures.localeEndonyms.map(\.id)) {
            #expect(Loc.mapPreferredLanguage(apple) == corpus, "\(apple) should resolve to \(corpus)")
        }
    }
}
