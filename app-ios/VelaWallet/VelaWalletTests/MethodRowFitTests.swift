//
//  MethodRowFitTests.swift
//  VelaWalletTests
//
//  The one-line second lines on a 375 pt phone, at FULL size (PR 3 note 23,
//  final note F24).
//
//  Issue #475 held each line to one line by letting it tighten (to 80 % at
//  most) before it was cut. In four languages (de, fr, it, es-MX) "Phone or
//  tablet"'s line only fitted that way; the core shortened those four, and
//  then two others still needed the floor (ru 309 pt, pt-BR 307 pt of 280).
//  The core shortened those two as well, and the floor is gone: no line is
//  drawn smaller than the ones above and below it.
//
//  Measured here in the caption's own face against each line's real column
//  on that phone, for every locale:
//
//    - the three places' lines (both choosers, every unlock the device can
//      report) against 280 pt — the screen less its gutters, the row's two
//      gaps, the spacer's minimum and the chevron;
//    - the signing page's line against 253 pt (the same row, with its globe);
//    - the security-key sheet's hint against 327 pt (the sheet less its
//      gutters).
//
//  Every one fits. With no floor behind it, a line that stops fitting is a
//  failure HERE — a corpus line to shorten — never a line quietly tightened
//  or cut on somebody's phone.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
struct MethodRowFitTests {

    /// The narrowest phone the app runs on.
    private static let phone: CGFloat = 375

    /// What a place's text column gets there (`KeyMethodRows`, inside
    /// `FlowShell` on the keys screen — the sign-in sheet's gutters are the
    /// same): the screen less its two gutters, the row's two gaps, the
    /// spacer's minimum and the chevron.
    private static var column: CGFloat {
        phone - 2 * Tokens.Layout.screenPaddingX - 2 * Tokens.Space.s12 - Tokens.Space.s8
            - LucideIconSize.rowGlyph
    }

    /// The signing page's row (`SigningPageEntry`): the same row with a globe
    /// and one more gap before its words.
    private static var pageColumn: CGFloat {
        column - LucideIconSize.rowGlyph - Tokens.Space.s12
    }

    /// The security-key sheet's hint (`UsbInsertKeySheet`): the sheet less
    /// its gutters.
    private static var hintColumn: CGFloat {
        phone - 2 * Tokens.Layout.screenPaddingX
    }

    private func width(_ text: String) -> CGFloat {
        // The caption's own face at its default size — the size it is drawn
        // at, now that nothing tightens it.
        let font = UIFont(name: Typography.flowCaption.fontName, size: Typography.flowCaption.size)
            ?? .systemFont(ofSize: Typography.flowCaption.size)
        return ceil((text as NSString).size(withAttributes: [.font: font]).width)
    }

    /// The same line as SwiftUI itself lays it out — the caption's role on a
    /// `Text`, at its ideal width. With no floor behind the line, a point's
    /// disagreement between two text engines would be an ellipsis on
    /// somebody's phone, so the row's own engine is asked too.
    private func drawnWidth(_ text: String) -> CGFloat {
        let host = UIHostingController(
            rootView: Text(text).typeRole(Typography.flowCaption).lineLimit(1).fixedSize()
        )
        return host.sizeThatFits(in: CGSize(width: CGFloat.greatestFiniteMagnitude,
                                            height: CGFloat.greatestFiniteMagnitude)).width
    }

    /// The widest of the three places' lines in `loc`, over both choosers
    /// and every unlock.
    private func widestPlace(_ loc: Loc) -> (width: CGFloat, line: String, count: Int) {
        var widest: (CGFloat, String) = (0, "")
        var count = 0
        for chooser in [KeyChooser.create, .signIn] {
            for unlock in ["face_id", "touch_id", "other"] {
                for method in KeyMethod.allCases {
                    let line = methodCopy(method, chooser: chooser, loc: loc, unlock: unlock).body
                    guard !line.isEmpty else { continue }
                    count += 1
                    if width(line) > widest.0 { widest = (width(line), line) }
                }
            }
        }
        return (widest.0, widest.1, count)
    }

    /// Every one-line second line, in every language, fits its column at
    /// FULL size: nothing needs tightening, so nothing is tightened.
    @Test func everyOneLineSubtitleFitsA375ptPhoneAtFullSize() throws {
        #expect(Self.column == 280, "the places' column is \(Self.column) pt")
        #expect(Self.pageColumn == 253, "the signing page's column is \(Self.pageColumn) pt")
        #expect(Self.hintColumn == 327, "the hint's column is \(Self.hintColumn) pt")
        var checked = 0
        for tag in Loc.supported {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let place = widestPlace(loc)
            checked += place.count
            let page = try #require(venueWords(row: "signing_page")?.lineKey.map { loc.t($0) },
                                    "the core names the signing page's line")
            let hint = loc.t(I18nKeys.Flow.insertKeyAppleSheetHint)
            #expect(place.width <= Self.column,
                    "\(tag): a place's line needs \(place.width) pt of \(Self.column): \(place.line)")
            #expect(width(page) <= Self.pageColumn,
                    "\(tag): the signing page's line needs \(width(page)) pt of \(Self.pageColumn): \(page)")
            #expect(width(hint) <= Self.hintColumn,
                    "\(tag): the hint needs \(width(hint)) pt of \(Self.hintColumn): \(hint)")
            // …and as SwiftUI draws them.
            let drawn = (place: drawnWidth(place.line), page: drawnWidth(page), hint: drawnWidth(hint))
            #expect(drawn.place <= Self.column, "\(tag): SwiftUI draws the place's line \(drawn.place) pt wide")
            #expect(drawn.page <= Self.pageColumn, "\(tag): SwiftUI draws the page's line \(drawn.page) pt wide")
            #expect(drawn.hint <= Self.hintColumn, "\(tag): SwiftUI draws the hint \(drawn.hint) pt wide")
            print("MEASURE one-line \(tag) place=\(place.width)/\(Self.column) (drawn \(drawn.place))"
                + " page=\(width(page))/\(Self.pageColumn) (drawn \(drawn.page))"
                + " hint=\(width(hint))/\(Self.hintColumn) (drawn \(drawn.hint))")
        }
        #expect(checked >= Loc.supported.count * 6, "few lines were measured (\(checked))")
    }

    /// The two the final round shortened (F24): 309 and 307 pt they were,
    /// drawn 9 % tighter. And the four from the round before stay short.
    @Test func theShortenedLinesFitAtFullSize() {
        for tag in ["ru", "pt-BR", "de", "fr", "it", "es-MX"] {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            let line = loc.t(I18nKeys.Create.methodHybridBody)
            #expect(line.count <= 45, "\(tag): \(line.count) characters — \(line)")
            let measured = width(line)
            #expect(measured <= Self.column, "\(tag) needs \(measured) pt of \(Self.column): \(line)")
            print("MEASURE method-rows \(tag) hybrid=\(measured) of \(Self.column) (\(line.count) chars)")
        }
    }

    /// One line at the default text size and below; a second line only for
    /// somebody who asked for bigger text — the system's, or the app's own.
    @Test func theLineWrapsOnlyForTextLargerThanTheDefault() {
        #expect(!OneLineSubtitle.wraps(typeSize: .large, scale: 1))
        #expect(!OneLineSubtitle.wraps(typeSize: .small, scale: TextScaleLevel.compact.factor))
        #expect(OneLineSubtitle.wraps(typeSize: .xLarge, scale: 1))
        #expect(OneLineSubtitle.wraps(typeSize: .accessibility1, scale: 1))
        for level in TextScaleLevel.allCases {
            #expect(OneLineSubtitle.wraps(typeSize: .large, scale: level.factor) == (level.factor > 1),
                    "\(level.rawValue)")
        }
    }
}
