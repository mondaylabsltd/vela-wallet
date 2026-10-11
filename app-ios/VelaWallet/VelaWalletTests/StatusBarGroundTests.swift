//
//  StatusBarGroundTests.swift
//  VelaWalletTests
//
//  A scrolled page is not drawn behind the status bar (iPhone walk
//  2026-10-10, first seen in Settings: a paragraph and a row's value showed
//  through behind the clock).
//
//  Every page of the app is a scroll view under no bar of its own, and a
//  SwiftUI scroll view draws its content on into the safe area above it. The
//  pages now stand on one ground (`pageGround`) that is also the status
//  bar's: the strip above the safe area is the page's own colour, over
//  whatever scrolls up.
//
//  Each real page, hosted full-screen in a window of the app's scene (which
//  has the screen's own safe area), scrolled to several places, and the
//  WINDOW'S PIXELS read: the system's clock is not the app's and is not in
//  the picture, so the whole strip must be the ground's one colour. The tree
//  cannot say this — a row that has scrolled up there is still in it.
//

import Foundation
import SwiftUI
import Testing
import UIKit
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct StatusBarGroundTests {
    private static let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    /// The pages: every one a person can scroll under the clock.
    private static func page(_ name: String) -> AnyView {
        let loc = zh
        switch name {
        case "wallet":
            return AnyView(WalletScreen(model: WalletFixtures.buildMobileState(.h1, loc: loc), loc: loc))
        case "settings":
            return AnyView(SettingsScreen(model: SettingsFixtures.build(.st1, loc: loc), loc: loc))
        case "explore":
            return AnyView(ExploreScreen(
                model: ExploreFixtures.buildMobileState(.e2, loc: loc), loc: loc,
                signing: SigningFixtures.build(.cs12, loc: loc)
            ))
        case let name where name.hasPrefix("contacts-"):
            let state = ContactsStateId(rawValue: String(name.dropFirst("contacts-".count))) ?? .c1
            return AnyView(ContactsStateHost(state: state, loc: loc))
        default:
            let state = FlowStateId(rawValue: String(name.dropFirst("flow-".count))) ?? .a1
            return AnyView(FlowHost(model: WalletFlowFixtures.build(state, loc: loc)))
        }
    }

    /// The four tabs, and one page of each kind the flows have (a list, a
    /// form, a confirm) — every flow page stands on `FlowScaffold`'s ground.
    static let pages = [
        "wallet", "settings", "explore", "contacts-c1", "contacts-c5",
        "flow-a1", "flow-r1", "flow-sd2b", "flow-sd3",
    ]

    @Test(arguments: pages)
    func aScrolledPageIsNotDrawnBehindTheStatusBar(_ name: String) async throws {
        let hosted = HostedScreen(Self.page(name).themed(.light))
        defer { hosted.close() }
        _ = try await hosted.read()
        let top = hosted.safeTop
        try #require(top > 20, "the hosted window has the screen's status bar: \(top)")
        // The status bar's strip, less its last point: the picture is one
        // pixel to the point, and the row on the seam takes a little of the
        // row under it (measured: only y = top − 1, a shade off the ground).
        // What scrolls up crosses the rows above it on its way.
        let strip = CGRect(x: 0, y: 0, width: hosted.size.width, height: top - 1)

        let rest = try await hosted.picture(settled: true)
        let ground = rest.pixel(2, Int(top / 2))
        #expect(rest.strangers(in: strip, to: ground) == 0, "\(name): something is in the status bar at rest")

        // A page with no scroll view of its own has nothing to put up there.
        guard hosted.pageScroll != nil else { return }
        var worst = 0
        var found = "none"
        for offset: CGFloat in [23, 81, 110, 197, 226, 371] {
            try await hosted.scroll(by: offset)
            let picture = try await hosted.picture(settled: true)
            let strangers = picture.strangers(in: strip, to: ground)
            if strangers > worst {
                worst = strangers
                found = "at \(Int(offset)) pt: " + HostedScreen.where_(picture, in: strip, to: ground)
                try await hosted.save("statusbar-\(name)")
            }
        }
        #expect(worst == 0, "\(name): scrolled, the page is drawn behind the status bar (\(worst) px at worst, \(found))")
        print("MEASURE statusbar \(name): safeTop \(top), longer than its room \(hosted.scrolls), strangers at worst \(worst) \(found)")
    }
}
