//
//  MethodRowFitTests.swift
//  VelaWalletTests
//
//  PR 3 note 23 — the three places' second lines on a 375 pt phone.
//
//  Issue #475 held each line to one line by letting it tighten (to 80 % at
//  most) before it was cut, and in four languages (de, fr, it, es-MX) "Phone
//  or tablet"'s line only fitted that way: 52–55 characters, drawn smaller
//  than the lines above and below it. The core shortened those four.
//
//  Measured here in the caption's own face against the row's real column on
//  that phone (280 pt: the screen less its gutters, the row's two gaps, the
//  spacer's minimum and the chevron), for every locale, both choosers and
//  every unlock the device can report:
//
//    - the four named lines fit at FULL size (de 279, es-MX 261, fr 259,
//      it 257 pt);
//    - two others do not — ru 309 pt, pt-BR 307 pt — and need the floor
//      (`OneLineSubtitle.minScale`) to stay whole; shortening them is the
//      corpus's (≤ 45 characters did it for the other four);
//    - no line anywhere needs more tightening than the floor allows, so no
//      line is ever cut.
//
//  That is why the floor is still there: a 5 % one was tried and the sweep
//  showed both of those lines cut on an iPhone SE.
//

import Foundation
import Testing
import UIKit
@testable import VelaWallet

@MainActor
struct MethodRowFitTests {

    /// The narrowest phone the app runs on.
    private static let phone: CGFloat = 375

    /// What a row's text column gets there (`KeyMethodRows`, inside
    /// `FlowShell` on the keys screen — the sign-in sheet's gutters are the
    /// same): the screen less its two gutters, the row's two gaps, the
    /// spacer's minimum and the chevron.
    private static var column: CGFloat {
        phone - 2 * Tokens.Layout.screenPaddingX - 2 * Tokens.Space.s12 - Tokens.Space.s8
            - LucideIconSize.rowGlyph
    }

    private func width(_ text: String) -> CGFloat {
        // The caption's own face at its default size — the size it is drawn
        // at when nothing tightens it.
        let font = UIFont(name: Typography.flowCaption.fontName, size: Typography.flowCaption.size)
            ?? .systemFont(ofSize: Typography.flowCaption.size)
        return ceil((text as NSString).size(withAttributes: [.font: font]).width)
    }

    /// No line is ever cut, and none but the two known ones is tightened.
    @Test func everyPlacesLineStaysWholeOnA375ptPhone() {
        #expect(Self.column == 280, "the column is \(Self.column) pt")
        var checked = 0
        var tightened: [String: CGFloat] = [:]
        for tag in Loc.supported {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            for chooser in [KeyChooser.create, .signIn] {
                for unlock in ["face_id", "touch_id", "other"] {
                    for method in KeyMethod.allCases {
                        let line = methodCopy(method, chooser: chooser, loc: loc, unlock: unlock).body
                        guard !line.isEmpty else { continue }
                        checked += 1
                        let measured = width(line)
                        if measured > Self.column { tightened[tag] = max(tightened[tag] ?? 0, measured) }
                        // Within the floor, or the line is cut.
                        #expect(measured * OneLineSubtitle.minScale <= Self.column,
                                "\(tag) \(chooser.rawValue) \(method.rawValue) is cut: \(measured) pt of \(Self.column): \(line)")
                    }
                }
            }
        }
        #expect(checked >= Loc.supported.count * 6, "few lines were measured (\(checked))")
        // Eleven languages besides the four named need no tightening at all.
        #expect(Set(tightened.keys).isSubset(of: ["ru", "pt-BR"]),
                "a line that fitted at full size no longer does: \(tightened)")
        // The two that do, need about 9 % — which a 5 % floor did not give.
        for (tag, measured) in tightened {
            #expect(Self.column / measured < 0.95, "\(tag) now fits within 5 %: the floor can come down")
        }
        print("MEASURE method-rows column=\(Self.column) tightened=\(tightened.sorted { $0.key < $1.key })")
    }

    /// The four languages the note names fit at FULL size.
    @Test func theFourShortenedLinesFitAtFullSize() {
        for tag in ["de", "fr", "it", "es-MX"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let line = loc.t(I18nKeys.Create.methodHybridBody)
            #expect(line.count <= 45, "\(tag): \(line.count) characters — \(line)")
            let measured = width(line)
            #expect(measured <= Self.column, "\(tag) needs \(measured) pt of \(Self.column): \(line)")
            print("MEASURE method-rows \(tag) hybrid=\(measured) of \(Self.column) (\(line.count) chars)")
        }
    }
}
