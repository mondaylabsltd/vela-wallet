//
//  SheetHeaderTests.swift
//  VelaWalletTests
//
//  A settings sheet's ✕ belongs to a header the body scrolls UNDER (iPhone
//  walk 2026-10-10: "Report a problem", scrolled, ran 「可选 · 最多 5 张」
//  straight through the floating ✕).
//
//  The real `SettingsSheet`, hosted short enough to scroll, read two ways:
//  the tree for where the ✕, the title and the line stand, and the window's
//  own pixels for what is SEEN on the ✕'s line — the tree still lists a line
//  that has scrolled under the header, so only the picture can say it is not
//  drawn over it.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SheetHeaderTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    private func sheet(_ overlay: SettingsOverlay, _ model: SettingsScreenModel) -> some View {
        SettingsSheet(model: model, overlay: overlay, loc: zh, onDismiss: {}, onSignOut: {})
            .themed(.light)
    }

    /// The report sheet, scrolled until the screenshots' "Optional · up to 5"
    /// is on the ✕'s line: the ✕ and the title have not moved, and nothing
    /// of the page is drawn on their line.
    @Test func theReportsPageScrollsUnderItsHeader() async throws {
        let model = SettingsFixtures.build(.st1, loc: zh)
        let hint = model.feedback.screenshotsHint
        #expect(hint == "可选 · 最多 5 张")
        let hosted = HostedScreen(sheet(.feedback, model), size: CGSize(width: 390, height: 420))
        defer { hosted.close() }

        let rest = try await hosted.read { $0.element("sheet.close") != nil && $0.labelled(hint) != nil }
        let close = try #require(rest.element("sheet.close")).frame
        let title = try #require(rest.element("sheet.title")).frame
        #expect(rest.element("sheet.title")?.label == model.feedback.title)
        let line = try #require(rest.labelled(hint)).frame
        try await hosted.save("sheet-header-1-rest")
        #expect(abs(title.midY - close.midY) < 8, "the ✕ is on the title's line: \(title) vs \(close)")
        #expect(title.maxX < close.minX, "the title leaves the ✕ its corner")
        #expect(line.maxX > close.minX, "the line reaches under the ✕'s column — it is what ran through it")

        // Bring that line up to the ✕'s.
        try await hosted.scroll(by: line.midY - close.midY)
        let scrolled = try await hosted.read()
        try await hosted.save("sheet-header-2-scrolled")
        let moved = try #require(scrolled.labelled(hint)).frame
        #expect(moved.minY < close.maxY && moved.maxY > close.minY,
                "the page did not scroll the line to the ✕'s: \(moved) vs \(close)")
        #expect(scrolled.element("sheet.close")?.frame == close, "the ✕ moved with the page")
        #expect(scrolled.element("sheet.title")?.frame == title, "the title left its place")

        // What is SEEN on the ✕'s line, between the title and the ✕: the
        // sheet's own ground, and nothing of the page.
        let picture = try await hosted.picture()
        let ground = picture.pixel(3, Int(close.midY))
        let beside = CGRect(
            x: title.maxX + 8, y: close.minY, width: close.minX - 4 - (title.maxX + 8), height: close.height
        )
        #expect(beside.width > 40)
        #expect(picture.strangers(in: beside, to: ground) == 0,
                "the page is drawn on the header's line, beside the ✕")
        // …and at rest that same strip was the same ground: nothing changed
        // for a sheet nobody scrolled.
        try await hosted.scroll(by: 0)
        #expect(try await hosted.picture().strangers(in: beside, to: ground) == 0)
    }

    /// Every sheet of this host opens the same way: its title where it was,
    /// and the ✕ on that line.
    @Test func everySheetKeepsItsTitleOnTheCloseLine() async throws {
        let model = SettingsFixtures.build(.st1, loc: zh)
        let cases: [(SettingsOverlay, String)] = [
            (.language, model.languageSheet.title),
            (.currency, model.currencySheet.title),
            (.accounts, model.accountsSheet.title),
            (.signOut, model.signOutSheet.title),
            (.clearCaches, model.clearCachesSheet.title),
        ]
        for (overlay, title) in cases {
            let hosted = HostedScreen(sheet(overlay, model), size: CGSize(width: 390, height: 520))
            let seen = try await hosted.read { $0.element("sheet.close") != nil }
            hosted.close()
            let close = try #require(seen.element("sheet.close")).frame
            // The header's own words, read once: the body's copy only keeps the place.
            let titles = seen.elements("sheet.title")
            #expect(titles.map(\.label) == [title], "\(overlay): the header says \(titles.map(\.label))")
            let frame = try #require(titles.first).frame
            #expect(abs(frame.midY - close.midY) < 8, "\(overlay): the title \(frame) is not on the ✕'s line \(close)")
            #expect(frame.maxX < close.minX, "\(overlay): the title \(frame) runs under the ✕ \(close)")
        }
    }
}
