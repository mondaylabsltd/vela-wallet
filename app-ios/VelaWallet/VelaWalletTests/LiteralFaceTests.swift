//
//  LiteralFaceTests.swift
//  VelaWalletTests
//
//  "0x" drawn as "0×" (iPhone pass 2026-10-09). The face the app bundled then
//  had one contextual-alternates rule: after a digit, `x` became `multiply`
//  and `-` became `minus`. So every address set in the proportional face — a
//  name that falls back to "0x14fB…eA5c", "To 0x…" under an activity row —
//  read "0×14fB…". The app now draws the system face, and a `literal` role
//  still turns the feature off wherever an address can appear, whatever the
//  face (docs/FONT-POLICY.md §4). These shape the text with CoreText and read
//  the glyphs it chose.
//

import CoreText
import Foundation
import Testing
import UIKit
@testable import VelaWallet

struct LiteralFaceTests {

    /// The names of the glyphs `font` draws `text` with, in order.
    private func glyphNames(_ text: String, _ font: UIFont) -> [String] {
        let line = CTLineCreateWithAttributedString(
            NSAttributedString(string: text, attributes: [.font: font]))
        var names: [String] = []
        for run in CTLineGetGlyphRuns(line) as? [CTRun] ?? [] {
            let attributes = CTRunGetAttributes(run) as NSDictionary
            let face = attributes[kCTFontAttributeName] as! CTFont
            var glyphs = [CGGlyph](repeating: 0, count: CTRunGetGlyphCount(run))
            CTRunGetGlyphs(run, CFRange(location: 0, length: 0), &glyphs)
            names += glyphs.map { (CTFontCopyNameForGlyph(face, $0) as String?) ?? "?" }
        }
        return names
    }

    /// The face as designed has contextual alternates — the system face sets
    /// a colon between digits as `colon.uc` — so the switch is not moot.
    @Test func theFaceHasContextualAlternates() {
        let names = glyphNames("10:30", Typography.rowTitle.uiFont)
        #expect(names.contains("colon.uc"), "the face no longer shapes in context: \(names)")
    }

    /// A literal role keeps every character as written, in every role an
    /// address is drawn in.
    @Test func aLiteralRoleDrawsTheAddressAsWritten() {
        for role in [Typography.rowTitle, Typography.rowSub, Typography.body, Typography.title, Typography.label] {
            let names = glyphNames("0x14fB…eA5c 2026-10-09 10:30", role.literal.uiFont)
            #expect(names.contains("x"), "\(role.face): \(names)")
            #expect(!names.contains("multiply"), "\(role.face): \(names)")
            #expect(!names.contains("minus"), "a date's hyphen became a minus: \(names)")
            #expect(!names.contains("colon.uc"), "contextual alternates still on: \(names)")
        }
    }

    /// Same face, same size: only the feature differs.
    @Test func aLiteralRoleIsTheSameFaceAndSize() {
        let role = Typography.rowTitle
        let literal = role.literal.uiFont
        #expect(literal.fontName == role.uiFont.fontName)
        #expect(abs(literal.pointSize - role.uiFont.pointSize) < 0.01)
        #expect(role.literal.scaled(1.35).contextual == false, "scaling kept the feature off")
        #expect(role.scaled(1.35).contextual)
    }

    /// Nothing is bundled: every role is the system face (docs/FONT-POLICY.md).
    @Test func everyRoleIsTheSystemFace() {
        let system = UIFont.systemFont(ofSize: 17).familyName
        let mono = UIFont.monospacedSystemFont(ofSize: 17, weight: .regular).familyName
        for role in [Typography.display, Typography.body, Typography.title, Typography.button,
                     Typography.rowTitle, Typography.amountHero, Typography.caption] {
            #expect(role.uiFont.familyName == system, "\(role.face): \(role.uiFont.familyName)")
        }
        for role in [Typography.mono, Typography.monoSmall] {
            #expect(role.uiFont.familyName == mono, "\(role.face): \(role.uiFont.familyName)")
        }
        #expect((Bundle.main.object(forInfoDictionaryKey: "UIAppFonts") as? [String] ?? []).isEmpty)
    }
}
