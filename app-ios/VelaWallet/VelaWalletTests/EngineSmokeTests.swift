//
//  EngineSmokeTests.swift
//  VelaWalletTests
//
//  vela-core engine through Loc: catalog load, fallback ladder, key echo
//  as the visible failure signal (FR-005/FR-007).
//

import Testing
@testable import VelaWallet

@MainActor
struct EngineSmokeTests {
    @Test func zhOverrideResolvesAndTranslates() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(loc.resolvedLanguage == "zh")
        #expect(loc.t("onboarding.welcome.createWallet") == "创建钱包")
        #expect(loc.t("onboarding.welcome.desktopTagline") == "您的密钥，您的资产")
    }

    @Test func unsupportedDeviceLanguageFallsBackToEnglish() {
        let loc = Loc(overrideTag: nil, preferredLanguages: ["ar-SA", "hi-IN"])
        #expect(loc.resolvedLanguage == "en")
        #expect(loc.t("onboarding.welcome.createWallet") == "Create Wallet")
    }

    @Test func germanCatalogLoads() {
        let loc = Loc(overrideTag: nil, preferredLanguages: ["de-DE"])
        #expect(loc.resolvedLanguage == "de")
        // Non-echo, non-English: the de catalog actually resolved.
        let value = loc.t("onboarding.welcome.createWallet")
        #expect(value != "onboarding.welcome.createWallet")
        #expect(!value.isEmpty)
        #expect(value != "Create Wallet")
    }

    /// Issue #409: the done screen's line is plural, and the count reaches
    /// the core as the PLURAL count. As a text var it fills the number but
    /// picks no form — so the bare key, which has no value now, echoes.
    @Test func theDoneLineAgreesWithItsCount() {
        let key = I18nKeys.Create.successMessage
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(en.t(key, count: 1)
            == "Your key can sign in on its own. The contract deploys with your first transaction.")
        #expect(en.t(key, count: 2)
            == "Any of your 2 keys can sign in on its own. The contract deploys with your first transaction.")
        #expect(en.t(key, vars: ["count": "1"]) == key)

        // Russian's 2–4 is `few`, which `count == 1 ? one : other` would miss.
        let ru = Loc(overrideTag: "ru", preferredLanguages: [])
        #expect(ru.t(key, count: 1).hasPrefix("Ваш ключ"))
        #expect(ru.t(key, count: 3).contains("из 3 ключей"))

        // Chinese has one form for every count, so it must not say how many.
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(zh.t(key, count: 1) == zh.t(key, count: 3))
        #expect(zh.t(key, count: 1) != key)
    }

    @Test func missingKeyEchoesItself() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(loc.t("onboarding.welcome.noSuchKey") == "onboarding.welcome.noSuchKey")
    }

    @Test func welcomeContentFullyResolvedInZhAndEn() {
        for (tag, expectedLang) in [("zh", "zh"), ("en", "en")] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            #expect(loc.resolvedLanguage == expectedLang)
            let content = WelcomeContentBuilder.build(loc: loc)
            // The v2 screen (spec 019) is a headline, a supporting line and the
            // two ways in; the six feature cards and the tagline this used to
            // count went with the carousel.
            #expect(!content.heroTitle.isEmpty && !content.heroTitle.hasPrefix("onboarding."))
            #expect(!content.heroSubtitle.isEmpty && !content.heroSubtitle.hasPrefix("onboarding."))
            #expect(!content.createWallet.hasPrefix("onboarding."))
            #expect(!content.alreadyHaveWallet.hasPrefix("onboarding."))
            // The headline's type tier is a corpus ENUM, so a key echo or a
            // translated value would silently fall back to `.regular` at the
            // call site. Assert the corpus really carries one of the two.
            #expect(HeroFit(rawValue: loc.t("onboarding.welcome.heroTitleFit")) != nil)
        }
    }
}
