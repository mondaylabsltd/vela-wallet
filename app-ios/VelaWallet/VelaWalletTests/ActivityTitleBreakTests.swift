//
//  ActivityTitleBreakTests.swift
//  VelaWalletTests
//
//  An activity row's title breaks only BETWEEN its parts — what was done and
//  where (device walk 2026-10-11, the other phone: 「在 arcorbs.game3.app 合 /
//  约交互」, the action split across two lines).
//
//  The home and History draw one row view, and its title is laid out by
//  `PlaceTitleView`: what was done is always one line (set smaller before it
//  would break), where it was done is cut in its middle, and the title is
//  one line or two with the break between them. Hosted at the SE's width
//  and narrower, at the usual and the largest text size, in Chinese and
//  English, with a long host, with and without a figure beside it: the three
//  parts' own frames say which line each stands on and that each is one
//  line tall.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct ActivityTitleBreakTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    static let hosts = ["arcorbs.game3.app", "very-long-subdomain.staging.arcorbs.game3.app"]

    /// The row the core's words make for a contract interaction at `host`.
    private func parts(_ host: String, _ loc: Loc) throws -> (title: String, parts: TitlePlace) {
        let verb = loc.t("componentsUi.signing.intentContractCall")
        #expect(verb != "componentsUi.signing.intentContractCall", "the corpus has the verb")
        let dapp = try CoreJSON.decode(FeedDappWire.self, from: [
            "action": "call", "intent": verb, "intent_term": "intentContractCall",
            "place": host, "estimated": false, "off_chain": false,
            "facts": [[String: Any]](), "technical": [[String: Any]](),
        ] as [String: Any])
        let title = WalletLive.dappTitle(dapp, loc: loc)
        return (title, try #require(WalletLive.dappTitleParts(dapp, loc: loc), "\(loc.resolvedLanguage): no parts"))
    }

    private struct Laid {
        let lead: CGRect?
        let joiner: CGRect?
        let place: CGRect
        let trail: CGRect?
        let line: CGFloat
        /// The action's own width on one line at its smallest size, and that
        /// size's line.
        let smallest: CGSize
    }

    /// The title's parts laid out `width` wide at `scale`.
    private func lay(_ parts: TitlePlace, width: CGFloat, scale: CGFloat) async throws -> Laid {
        let role = Typography.rowTitle.scaled(scale)
        let view = VStack(alignment: .leading, spacing: 40) {
            PlaceTitleView(parts: parts, role: role)
                .frame(width: width, alignment: .leading)
            // One line of the same face, to measure a line by.
            Text(verbatim: "Ag合").typeRole(role).lineLimit(1).accessibilityIdentifier("title.ruler")
            // The action alone, on one line at its smallest size.
            Text(verbatim: parts.lead.isEmpty || !parts.trail.isEmpty ? parts.trail : parts.lead)
                .typeRole(role.scaled(ActionWords.floor)).lineLimit(1).fixedSize()
                .accessibilityIdentifier("title.smallest")
        }
        .padding(8)
        // At the window's leading edge: x is the column's own, plus the 8.
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .themed(.light)
        let hosted = HostedScreen(view, size: CGSize(width: max(width + 16, 320), height: 400))
        defer { hosted.close() }
        let seen = try await hosted.read { $0.element("title.place") != nil && $0.element("title.ruler") != nil }
        return Laid(
            lead: parts.lead.isEmpty ? nil : seen.element("title.lead")?.frame,
            joiner: parts.joiner.isEmpty ? nil : seen.element("title.joiner")?.frame,
            place: try #require(seen.element("title.place")).frame,
            trail: parts.trail.isEmpty ? nil : seen.element("title.trail")?.frame,
            line: try #require(seen.element("title.ruler")).frame.height,
            smallest: try #require(seen.element("title.smallest")).frame.size
        )
    }

    /// The title column's width in a row `screen` wide: the page's margins,
    /// the icon and its gap, and — with a figure — the gap and the figure.
    private static func column(screen: CGFloat, figure: CGFloat) -> CGFloat {
        screen - 2 * Tokens.Layout.screenPaddingX - WalletGeometry.rowIcon - Tokens.Space.s12
            - (figure > 0 ? Tokens.Space.s12 + figure : 0)
    }

    @Test(arguments: [CGFloat(1), 1.35])
    func theTitleBreaksOnlyBetweenWhatWasDoneAndWhere(scale: CGFloat) async throws {
        for loc in [zh, en] {
            for host in Self.hosts {
                let (title, parts) = try parts(host, loc)
                // The SE (375), the smallest phone (320), with no figure and
                // beside one ("−0.0001 ETH", about 96 pt at the largest size).
                for (screen, figure) in [(375, 0), (375, 96), (320, 0), (320, 96)] as [(CGFloat, CGFloat)] {
                    let width = Self.column(screen: screen, figure: figure)
                    let laid = try await lay(parts, width: width, scale: scale)
                    let what = "\(loc.resolvedLanguage) ×\(scale) \(Int(screen))\(figure > 0 ? "+figure" : "") 「\(title)」"

                    // What was done: ONE line, whole — set smaller before it
                    // would break. Only an action wider than the column at
                    // its smallest size takes a second line, between words.
                    let fitsSmall = laid.smallest.width <= width + 0.5
                    for words in [laid.lead, laid.trail].compactMap({ $0 }) {
                        if fitsSmall {
                            #expect(words.height <= laid.line * 1.15,
                                    "\(what): the action is \(words.height) tall — \(laid.line) a line — and fits at \(laid.smallest.width)")
                        } else {
                            #expect(words.height <= laid.smallest.height * 2.15, "\(what): the action takes three lines")
                            #expect(loc.resolvedLanguage != "zh", "\(what): a Chinese action never needs a second line")
                        }
                        #expect(words.maxX <= 8 + width + 0.5, "\(what): the action runs out of the column")
                    }
                    // Where: one line, inside the column (cut in its middle).
                    #expect(laid.place.height <= laid.line * 1.15, "\(what): the site wrapped")
                    #expect(laid.place.maxX <= 8 + width + 0.5, "\(what): the site runs out of the column")
                    #expect(laid.place.width >= min(width, 60), "\(what): the site is cut to nothing: \(laid.place.width)")

                    // One line, or two with the break BETWEEN the parts.
                    let tops = Set([laid.lead, laid.place, laid.trail].compactMap { $0?.midY.rounded() })
                    #expect(tops.count <= 2, "\(what): the parts stand on three lines")
                    if let lead = laid.lead, let trail = laid.trail {
                        // 「在 ⟨site⟩ 合约交互」: the site beside 在, the action under.
                        #expect(abs(lead.midY - laid.place.midY) < 2)
                        #expect(trail.midY >= laid.place.midY - 0.5)
                    } else if let lead = laid.lead {
                        // "Contract interaction on ⟨site⟩": the site beside or
                        // under — and "on" with the site, never left at the
                        // end of the action's line.
                        #expect(laid.place.minY >= lead.minY - 0.5)
                        if laid.place.minY.rounded() == lead.minY.rounded() {
                            #expect(laid.place.minX >= lead.maxX, "\(what): the site is drawn over the action")
                        }
                        if let joiner = laid.joiner {
                            #expect(abs(joiner.midY - laid.place.midY) < 2, "\(what): the joiner is not on the site's line")
                            #expect(joiner.maxX <= laid.place.minX + 0.5)
                        }
                    }
                    print("MEASURE title \(what) column \(width): lead \(laid.lead.map { "\($0)" } ?? "-") place \(laid.place) trail \(laid.trail.map { "\($0)" } ?? "-") line \(laid.line)")
                }
            }
        }
    }

    /// The whole row, on the home's own view (History draws the same one),
    /// at the SE's width and the largest text size: the title is two lines
    /// at most and the figure beside it is whole. The one exception is the
    /// one `ActionWords` names — an English action beside a figure at that
    /// size is wider than its column even set small, and takes a second
    /// line between its words: three short lines, nothing cut.
    @Test func theRowsTitleIsTwoLinesAtMostAndItsFigureIsWhole() async throws {
        for loc in [zh, en] {
            let (title, parts) = try parts(Self.hosts[1], loc)
            for withFigure in [false, true] {
                let row = ActivityRowModel(
                    kind: .dapp, title: title, subtitle: "Arc · 12:30",
                    amount: withFigure ? "\u{2212}0.0001" : "", unit: withFigure ? "ETH" : "",
                    positive: false, masked: false, badgeColor: .clear, titlePlace: parts
                )
                let view = VStack(alignment: .leading, spacing: 30) {
                    ActivityRowView(model: row)
                    Text(verbatim: "Ag合").typeRole(Typography.rowTitle.scaled(1.35)).lineLimit(1)
                        .accessibilityIdentifier("title.ruler")
                }
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .environment(\.walletTextScale, 1.35)
                .themed(.light)
                let hosted = HostedScreen(view, size: CGSize(width: 375, height: 400))
                let seen = try await hosted.read { $0.labelled(title) != nil }
                try await hosted.save("activity-title-\(loc.resolvedLanguage)-\(withFigure ? "figure" : "plain")")
                hosted.close()
                let drawn = try #require(seen.labelled(title), "the title is said whole, once").frame
                let line = try #require(seen.element("title.ruler")).frame.height
                let lines = drawn.height / line
                print("MEASURE title-row \(loc.resolvedLanguage) \(withFigure ? "figure" : "plain") ×1.35 375: title \(drawn) = \(lines) lines")
                if loc.resolvedLanguage == "en", withFigure {
                    // One full line (the site) and two at the action's small size.
                    #expect(lines <= 1 + 2 * ActionWords.floor * 1.15, "en beside a figure: \(lines) lines")
                } else {
                    #expect(lines <= 2.2, "\(loc.resolvedLanguage): the title is \(lines) lines")
                }
                if withFigure {
                    let figure = try #require(seen.tree.first { $0.label.contains("0.0001") }, "the figure").frame
                    #expect(figure.maxX <= 375 - Tokens.Layout.screenPaddingX + 0.5)
                    #expect(drawn.maxX <= figure.minX, "the title runs under the figure")
                }
            }
        }
    }

    /// Every language's title names its place once, so every language's row
    /// has parts to break between — none falls back to one run of text.
    @Test func everyLanguagesTitleHasItsParts() throws {
        for language in Loc.supported {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let (title, parts) = try parts(Self.hosts[0], loc)
            #expect(parts.place == Self.hosts[0])
            #expect(!(parts.lead.isEmpty && parts.trail.isEmpty), "\(language): 「\(title)」 has no action")
            #expect(title.contains(Self.hosts[0]))
        }
    }
}
