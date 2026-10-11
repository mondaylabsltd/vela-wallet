//
//  SignBadgeTests.swift
//  VelaWalletTests
//
//  The currency picker's round badge shows every sign whole (owner's iPhone,
//  2026-10-11: 「HK$」 was cut to 「H…」 and 「JP¥」 ran rim to rim — the core's
//  signs run to three characters now, and the badge drew whatever it was
//  given at the body size).
//
//  Every currency the core's rule names — the thirty with a sign of their own
//  and the four it names as having none, drawn as their code — in each of the
//  app's fifteen languages: by the badge's own rule, and then DRAWN, in a
//  hosted window, at the default and the largest text size, where the sign's
//  laid-out width is read back and held against the disc it stands in.
//
//  Settings' currency row ("CNY · CN¥8,262.68") is held to the same: its
//  value is never cut, beside the title or under it.
//

import Foundation
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized, .hangLimit)
struct SignBadgeTests {

    /// The currencies the core's rule names (`l10n::currency::fiat_symbol`):
    /// thirty with a sign, and the four it gives as its examples of none.
    static let codes = [
        "USD", "EUR", "GBP", "CNY", "JPY", "KRW", "VND", "INR", "TRY", "PHP", "ILS", "THB", "UAH", "NGN", "RUB",
        "PLN", "CZK", "HUF", "IDR", "MYR", "ZAR", "BRL", "HKD", "CAD", "AUD", "NZD", "SGD", "TWD", "MXN", "ARS",
        "CHF", "SEK", "NOK", "DKK",
    ]

    /// Every sign a badge can be handed, once each, with one place it comes from.
    private static let signs: [(sign: String, from: String)] = {
        var seen: [String: String] = [:]
        var order: [String] = []
        for language in Loc.supported {
            for code in codes {
                let sign = FiatSign.symbol(code, language: language)
                if seen[sign] == nil {
                    seen[sign] = "\(code) in \(language)"
                    order.append(sign)
                }
            }
        }
        return order.map { ($0, seen[$0] ?? "") }
    }()

    /// The rule, for all 34 × 15: each sign has a size on the ladder at which
    /// the whole of it fits the disc's margin, and a lone glyph is as large
    /// as it always was.
    @Test func everySignHasASizeAtWhichItFitsWhole() {
        #expect(Self.codes.count == 34 && Loc.supported.count == 15)
        var widest: (sign: String, width: CGFloat) = ("", 0)
        for language in Loc.supported {
            for code in Self.codes {
                let sign = FiatSign.symbol(code, language: language)
                let size = SignBadge.size(for: sign)
                #expect(SignBadge.sizes.contains(size), "\(code) in \(language): 「\(sign)」 fell off the ladder, to \(size) pt")
                #expect(size >= Tokens.TextSize.t11, "\(code) in \(language): 「\(sign)」 is drawn at \(size) pt")
                let width = SignBadge.width(of: sign, at: size)
                #expect(width <= SignBadge.room, "\(code) in \(language): 「\(sign)」 is \(width) pt in a room of \(SignBadge.room)")
                // One glyph or two: as large as it always was. Three letters:
                // all alike, so a column of them reads as one hand.
                #expect(size == (sign.count <= 2 ? SignBadge.sizes[0] : SignBadge.longSize),
                        "\(code) in \(language): 「\(sign)」 is drawn at \(size) pt, unlike the others of its length")
                let natural = SignBadge.width(of: sign, at: SignBadge.sizes[0])
                if natural > widest.width { widest = (sign, natural) }
            }
        }
        // What was wrong: at the body's size these do not fit the disc at all.
        #expect(SignBadge.width(of: "HK$", at: SignBadge.sizes[0]) > SignBadge.room)
        #expect(SignBadge.size(for: "HK$") < SignBadge.sizes[0])
        // The reader's own of the two that share ¥ is the lone glyph.
        #expect(FiatSign.symbol("CNY", language: "zh") == "¥" && FiatSign.symbol("JPY", language: "zh") == "JP¥")
        #expect(FiatSign.symbol("JPY", language: "en") == "¥" && FiatSign.symbol("CNY", language: "en") == "CN¥")
        // A sign longer than any the core writes is still whole, just smaller.
        let long = "WWWWW$"
        let size = SignBadge.size(for: long)
        #expect(size < SignBadge.sizes[SignBadge.sizes.count - 1])
        #expect(SignBadge.width(of: long, at: size) <= SignBadge.room + 1)
        print("MEASURE sign-badge: room \(SignBadge.room) of \(SignBadge.side); widest at \(SignBadge.sizes[0]) pt is 「\(widest.sign)」 \(widest.width) pt; sizes: "
            + Self.signs.map { "\($0.sign)=\(SignBadge.size(for: $0.sign))" }.joined(separator: " "))
    }

    /// One badge per sign, each with a marker the size of its disc, so the
    /// sign's drawn frame can be held against the disc it stands in.
    private struct Board: View {
        let signs: [String]
        var body: some View {
            let rows = stride(from: 0, to: signs.count, by: 6).map { Array(signs[$0..<min($0 + 6, signs.count)]) }
            VStack(spacing: Tokens.Space.s12) {
                ForEach(rows, id: \.self) { row in
                    HStack(spacing: Tokens.Space.s16) {
                        ForEach(row, id: \.self) { sign in
                            SignBadge(sign: sign)
                                .background(Color.clear.accessibilityElement().accessibilityLabel("disc " + sign))
                        }
                    }
                }
            }
        }
    }

    /// Drawn. The sign's laid-out width is the whole sign's — not a cut one's
    /// — and it stands inside its disc's margin, on its middle; every disc is
    /// the one size. At the default text size and the largest.
    @Test(arguments: [CGFloat(1), 1.35])
    func everySignIsDrawnWholeInsideItsDisc(scale: CGFloat) async throws {
        let signs = Self.signs.map(\.sign)
        let hosted = HostedScreen(
            Board(signs: signs).environment(\.walletTextScale, scale).themed(.light),
            size: HostedScreen.se
        )
        defer { hosted.close() }
        let seen = try await hosted.read { $0.labelled("disc " + signs[signs.count - 1]) != nil }
        try await hosted.save("sign-badges-\(scale)")
        for (sign, from) in Self.signs {
            let disc = try #require(seen.labelled("disc " + sign), "\(from): no badge for 「\(sign)」")
            let text = try #require(seen.labelled(sign), "\(from): 「\(sign)」 is not on its badge")
            #expect(abs(disc.frame.width - SignBadge.side) <= 0.5 && abs(disc.frame.height - SignBadge.side) <= 0.5,
                    "\(from): the disc is \(disc.frame.size)")
            // Whole: as wide as all of the sign is at the size it is drawn
            // at. A cut sign ("H…") is narrower than the whole one.
            let whole = SignBadge.width(of: sign, at: SignBadge.size(for: sign))
            #expect(abs(text.frame.width - whole) <= 1, "\(from): 「\(sign)」 is drawn \(text.frame.width) pt wide; whole it is \(whole)")
            #expect(text.frame.width <= SignBadge.room + 0.5, "\(from): 「\(sign)」 is \(text.frame.width) pt in a room of \(SignBadge.room)")
            #expect(abs(text.frame.midX - disc.frame.midX) <= 0.5 && abs(text.frame.midY - disc.frame.midY) <= 0.5,
                    "\(from): 「\(sign)」 is off its disc's middle")
            #expect(text.frame.minX >= disc.frame.minX && text.frame.maxX <= disc.frame.maxX
                        && text.frame.minY >= disc.frame.minY - 0.5 && text.frame.maxY <= disc.frame.maxY + 0.5,
                    "\(from): 「\(sign)」 \(text.frame) leaves its disc \(disc.frame)")
        }
    }

    /// The picker's own row: the badge is one size whatever its sign, so the
    /// codes beside it stand in one column — at the largest text size too.
    @Test func thePickersRowsStandInOneColumn() async throws {
        let picked = ["USD", "CNY", "JPY", "HKD", "KRW", "CHF", "MXN", "BRL"]
        for language in ["en", "zh"] {
            let rows = picked.map { code in
                SelectRowModel(id: code, label: code, glyph: FiatSign.symbol(code, language: language), caption: "caption \(code)")
            }
            let hosted = HostedScreen(
                VStack(spacing: Tokens.Space.s0) { ForEach(rows) { SelectRow(row: $0) } }
                    .padding(.horizontal, Tokens.Layout.screenPaddingX)
                    .environment(\.walletTextScale, 1.35)
                    .themed(.light),
                size: HostedScreen.se
            )
            defer { hosted.close() }
            let seen = try await hosted.read { $0.labelled("caption \(picked[picked.count - 1])") != nil }
            try await hosted.save("sign-rows-\(language)")
            var column: Set<CGFloat> = [], middles: Set<CGFloat> = []
            for row in rows {
                let sign = try #require(row.glyph)
                // "CHF" is written twice on its row: the sign's is the first.
                let text = try #require(seen.tree.first { $0.label == sign }, "\(language): no sign on \(row.id)'s row")
                let code = try #require(seen.tree.last { $0.label == row.id }, "\(language): no code on \(row.id)'s row")
                let whole = SignBadge.width(of: sign, at: SignBadge.size(for: sign))
                #expect(abs(text.frame.width - whole) <= 1 && text.frame.width <= SignBadge.room + 0.5,
                        "\(language) \(row.id): 「\(sign)」 is drawn \(text.frame.width) pt wide; whole it is \(whole), in a room of \(SignBadge.room)")
                column.insert(code.frame.minX.rounded())
                middles.insert(text.frame.midX.rounded())
            }
            #expect(column.count == 1, "\(language): the codes start at \(column.sorted()) — the badges are not one size")
            #expect(middles.count == 1, "\(language): the signs' middles are at \(middles.sorted())")
        }
    }

    /// Settings' currency row. Its value — "HKD · HK$9,629.57" — has the
    /// title's line while both fit and its own line under the title when
    /// they do not, and is whole either way: for every currency, at the
    /// worst rate a figure has (tens of millions), under the shortest title
    /// and the longest, at the default text size and the largest, on the SE.
    @Test(arguments: [CGFloat(1), 1.35])
    func theCurrencyRowsValueIsNeverCut(scale: CGFloat) async throws {
        // The row as the live screen builds it, per language; the titles are
        // the corpus's.
        func row(_ code: String, _ language: String) throws -> SettingsRowModel {
            let loc = Loc(overrideTag: language, preferredLanguages: [])
            let base = SettingsFixtures.build(.st1, loc: loc)
            let live = SettingsLive.withCurrency(CurrencyViewWire(code: code, rate: 30_000, committed: true), on: base, loc: loc)
            return try #require(live.sections.flatMap(\.rows).first { $0.id == "currency" }, "no currency row in \(language)")
        }
        let titled = try Loc.supported.map { ($0, try row("USD", $0).title) }.sorted { $0.1.count < $1.1.count }
        let languages = Array(Set([try #require(titled.first).0, try #require(titled.last).0, "en", "zh"])).sorted()
        var rows: [(at: String, row: SettingsRowModel)] = []
        for language in languages {
            for code in Self.codes {
                let model = try row(code, language)
                let value = try #require(model.value)
                #expect(value.contains(FiatSign.symbol(code, language: language)), "\(code) in \(language): \(value)")
                rows.append(("\(code) in \(language)", model))
            }
        }
        // Two columns of rows, each the SE's width, so every row is laid out
        // at the width it has on the narrowest phone.
        let width = HostedScreen.se.width
        let board = ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                ForEach(Array(rows.enumerated()), id: \.offset) { index, item in
                    SettingsRow(row: item.row)
                    // The value whole, as a measure: the same words in the
                    // same face, at their own width.
                    Text(item.row.value ?? "")
                        .typeRole(Typography.flowCaption)
                        .lineLimit(1)
                        .fixedSize()
                        .accessibilityLabel("whole \(index)")
                }
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
        }
        .frame(width: width)
        .environment(\.walletTextScale, scale)
        .themed(.light)

        let hosted = HostedScreen(board, size: HostedScreen.se)
        defer { hosted.close() }
        var checked = 0
        // A scroll view keeps only what is near the screen in the tree: read
        // it a screenful at a time.
        var offset: CGFloat = 0
        var seenRows: Set<Int> = []
        for _ in 0..<400 where seenRows.count < rows.count {
            try await hosted.scroll(by: offset)
            let seen = try await hosted.read()
            for (index, item) in rows.enumerated() where !seenRows.contains(index) {
                guard let whole = seen.labelled("whole \(index)"),
                      let value = seen.tree.first(where: { $0.label == item.row.value && $0.frame.maxY <= whole.frame.minY + 1 && $0.frame.maxY > whole.frame.minY - 200 })
                else { continue }
                seenRows.insert(index)
                checked += 1
                #expect(value.frame.width >= whole.frame.width - 0.5,
                        "\(item.at) ×\(scale): 「\(item.row.value ?? "")」 has \(value.frame.width) pt of the \(whole.frame.width) it needs")
                #expect(value.frame.maxX <= width - Tokens.Layout.screenPaddingX + 0.5, "\(item.at) ×\(scale): the value runs off the row")
            }
            offset += HostedScreen.se.height / 2
        }
        #expect(checked == rows.count, "×\(scale): \(checked) of \(rows.count) rows were read")
        print("MEASURE currency-row ×\(scale): \(checked) rows whole at \(Int(width)) wide, languages \(languages)")
    }
}
