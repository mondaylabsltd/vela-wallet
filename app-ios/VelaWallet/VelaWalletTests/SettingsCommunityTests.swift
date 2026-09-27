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
import UIKit
import VelaCore
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

    // MARK: - What the renderer makes of the paths

    /// The brand marks go through vela-core's rasterizer (resvg) — a path
    /// parser of its own, not Android's, which misread simple-icons'
    /// packed arc flags ("0 00-.0785") and drew Discord wrong (2026-09-27).
    /// Each mark must parse to ink inside the 24 box — inside the 2…22 inset
    /// the brand group is scaled into — and fill most of it, so the three
    /// read at one size.
    @Test func everyBrandMarkParsesToInkInsideTheBox() throws {
        let side = 96
        for glyph in [LucideGlyph.brandX, .brandTelegram, .brandDiscord] {
            let ink = try #require(Self.inkBounds(glyph.svg, side: side), "\(glyph) drew nothing")
            let unit = CGFloat(side) / 24
            // One pixel of antialiasing either side of the 2…22 inset.
            #expect(ink.minX >= 2 * unit - 1 && ink.minY >= 2 * unit - 1, "\(glyph) ink \(ink) starts outside the inset")
            #expect(ink.maxX <= 22 * unit + 1 && ink.maxY <= 22 * unit + 1, "\(glyph) ink \(ink) runs past the inset")
            #expect(max(ink.width, ink.height) >= 17 * unit, "\(glyph) ink \(ink) is too small beside the others")
        }
    }

    /// Packed arc flags are HANDLED, not misread: simple-icons' original
    /// Discord path (flags packed, "0 00-.0785") and the normalized one now
    /// shipped render pixel for pixel alike — and so do a packed and a
    /// spaced arc on their own.
    @Test func packedArcFlagsRenderLikeSpacedOnes() throws {
        let packed = Self.brandDocument(Self.simpleIconsDiscord)
        let shipped = LucideGlyph.brandDiscord.svg
        #expect(shipped.contains("0 0 0 -.0785"), "the shipped Discord path is the normalized one")
        let packedPixels = try Self.pixels(packed, side: 96)
        let shippedPixels = try Self.pixels(shipped, side: 96)
        #expect(!shippedPixels.isEmpty && Self.inkBounds(shipped, side: 96) != nil, "nothing to compare")
        #expect(packedPixels == shippedPixels, "resvg reads the packed flags differently from the spaced ones")

        func arc(_ d: String) -> String {
            ##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path fill="#FFFFFF" d="\##(d)"/></svg>"##
        }
        let spaced = arc("M2 12a10 10 0 1 0 20 0a10 10 0 0 0 -20 0z")
        let tight = arc("M2 12a10 10 0 1020 0a10 10 0 00-20 0z")
        #expect(try Self.pixels(spaced, side: 48) == Self.pixels(tight, side: 48))
        #expect(Self.inkBounds(tight, side: 48) != nil, "the packed arc drew nothing")
        // The comparison can tell: the other sweep flag is a different shape.
        let otherSweep = arc("M2 12a10 10 0 1 1 20 0a10 10 0 0 0 -20 0z")
        #expect(try Self.pixels(spaced, side: 48) != Self.pixels(otherSweep, side: 48))
    }

    /// simple-icons' Discord path as it was first shipped — flags packed.
    private static let simpleIconsDiscord = "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z"

    private static func brandDocument(_ d: String) -> String {
        ##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g transform="translate(2 2) scale(0.8333)"><path fill="#FFFFFF" d="\##(d)"/></g></svg>"##
    }

    /// The rasterized document's RGBA bytes.
    private static func pixels(_ svg: String, side: Int) throws -> [UInt8] {
        let png = try rasterizeSvgPng(svg: svg, sizePx: UInt32(side))
        guard let image = UIImage(data: png)?.cgImage else { return [] }
        var bytes = [UInt8](repeating: 0, count: side * side * 4)
        let context = CGContext(data: &bytes, width: side, height: side, bitsPerComponent: 8, bytesPerRow: side * 4,
                                space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
        context?.draw(image, in: CGRect(x: 0, y: 0, width: side, height: side))
        return bytes
    }

    /// The box around every pixel with any ink, in pixels from the top-left.
    private static func inkBounds(_ svg: String, side: Int) -> CGRect? {
        guard let bytes = try? pixels(svg, side: side), !bytes.isEmpty else { return nil }
        var minX = side, minY = side, maxX = -1, maxY = -1
        for y in 0..<side {
            for x in 0..<side where bytes[(y * side + x) * 4 + 3] > 8 {
                minX = min(minX, x); maxX = max(maxX, x)
                minY = min(minY, y); maxY = max(maxY, y)
            }
        }
        guard maxX >= 0 else { return nil }
        return CGRect(x: minX, y: minY, width: maxX - minX + 1, height: maxY - minY + 1)
    }
}
