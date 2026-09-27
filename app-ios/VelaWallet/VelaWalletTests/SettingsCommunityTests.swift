//
//  SettingsCommunityTests.swift
//  VelaWalletTests
//
//  Settings → Community (founder, 2026-09-27: "设置里面再加一下，我们的官方社交
//  账号链接：X/twitter, discord, telegram"). The three official accounts,
//  exactly as getvela.app's footer publishes them — pinned here so the four
//  shells cannot drift — directly above 关于 / 反馈, each row leaving the app.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SettingsCommunityTests {

    @Test func theLinksAreTheOfficialOnesExactly() {
        let links = SettingsFixtures.communityLinks
        #expect(links.map(\.title) == ["X (Twitter)", "Telegram", "Discord"])
        #expect(links.map(\.handle) == ["@realvelawallet", "@velawallet", "discord.gg/23gWrtaYSa"])
        #expect(links.map(\.url) == [
            "https://x.com/realvelawallet",
            "https://t.me/velawallet",
            "https://discord.gg/23gWrtaYSa",
        ])
        #expect(links.map(\.glyph) == [.brandX, .brandTelegram, .brandDiscord])
    }

    @Test func theGroupSitsDirectlyAboveAboutAndFeedback() throws {
        for tag in ["en", "zh", "de", "es-MX"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let sections = SettingsFixtures.build(.st1, loc: loc).sections
            #expect(sections.count >= 2)
            let community = sections[sections.count - 2]
            #expect(community.rows.map(\.id) == ["community-x", "community-telegram", "community-discord"], "\(tag)")
            #expect(community.label == loc.t("settings.sections.community"), "\(tag)")
            #expect(community.label?.contains("settings.") == false, "untranslated in \(tag)")
            // Brand names are never translated; the handles are literal.
            #expect(community.rows.map(\.title) == ["X (Twitter)", "Telegram", "Discord"], "\(tag)")
            #expect(community.rows.allSatisfy { $0.trailing == .external }, "a row that leaves the app wears a chevron")
            #expect(sections.last?.rows.map(\.id) == ["about", SettingsFixtures.feedbackRow], "\(tag)")
        }
    }

    @Test func eachRowOpensExactlyItsURL() {
        #expect(SettingsScreen.externalLink(forRow: "community-x") == "https://x.com/realvelawallet")
        #expect(SettingsScreen.externalLink(forRow: "community-telegram") == "https://t.me/velawallet")
        #expect(SettingsScreen.externalLink(forRow: "community-discord") == "https://discord.gg/23gWrtaYSa")
        #expect(SettingsScreen.externalLink(forRow: "about") == nil)
        #expect(SettingsScreen.overlay(forRow: "community-x", hasSignerPage: true) == nil)
    }

    /// The brand marks are real SVG documents the core can rasterize.
    @Test func theBrandGlyphsAreFilledSVG() {
        for glyph in [LucideGlyph.brandX, .brandTelegram, .brandDiscord] {
            #expect(glyph.svg.hasPrefix("<svg"))
            #expect(glyph.svg.contains("fill=\"#FFFFFF\""))
            #expect(!glyph.svg.contains("stroke-width"), "\(glyph) would be drawn as an outline")
        }
    }
}
