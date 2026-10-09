//
//  LiteralFaceTests.swift
//  VelaWalletTests
//
//  "0x" drawn as "0×" (iPhone pass 2026-10-09). Plus Jakarta Sans has one
//  contextual-alternates rule: after a digit, `x` becomes `multiply` and `-`
//  becomes `minus`. So every address set in the proportional face — a name
//  that falls back to "0x14fB…eA5c", "To 0x…" under an activity row — read
//  "0×14fB…". A `literal` role turns the feature off; these shape the text
//  with CoreText and read the glyphs it chose.
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

    /// The face as designed does it — this is what was on the phone.
    @Test func theFaceTurnsTheXOfAnAddressIntoATimesSign() {
        let names = glyphNames("0x14fB", Typography.rowTitle.uiFont)
        #expect(names.contains("multiply"), "the bundled Jakarta no longer has the rule: \(names)")
        #expect(!names.contains("x"))
    }

    /// A literal role keeps the letter, in every role an address is drawn in.
    @Test func aLiteralRoleDrawsTheAddressAsWritten() {
        for role in [Typography.rowTitle, Typography.rowSub, Typography.body, Typography.title, Typography.label] {
            let names = glyphNames("0x14fB…eA5c 2026-10-09", role.literal.literalUIFont())
            #expect(names.contains("x"), "\(role.fontName): \(names)")
            #expect(!names.contains("multiply"), "\(role.fontName): \(names)")
            #expect(!names.contains("minus"), "a date's hyphen became a minus: \(names)")
        }
    }

    /// Same face, same size: only the feature differs.
    @Test func aLiteralRoleIsTheSameFaceAndSize() {
        let role = Typography.rowTitle
        let literal = role.literal.literalUIFont()
        #expect(literal.fontName == role.uiFont.fontName)
        #expect(abs(literal.pointSize - role.uiFont.pointSize) < 0.01)
        #expect(role.literal.scaled(1.35).contextual == false, "scaling kept the feature off")
        #expect(role.scaled(1.35).contextual)
    }
}
