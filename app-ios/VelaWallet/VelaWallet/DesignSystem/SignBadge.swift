//
//  SignBadge.swift
//  VelaWallet
//
//  A currency's sign on a round badge — the leading mark of the currency
//  picker's rows.
//
//  The sign is the core's (`fiatSymbol(code:language:)`, through `FiatSign`),
//  and it runs to three characters: "HK$", "CA$", "JP¥" for a Chinese reader,
//  "CN¥" for everybody else, and the code itself — "CHF" — for a currency
//  with no glyph anybody would recognise. The badge used to draw whatever it
//  was given at the body size: 「HK$」 was cut to 「H…」 and 「JP¥」 ran rim to
//  rim (owner's iPhone, 2026-10-11).
//
//  The badge is one fixed disc, so the column it stands in stays aligned; the
//  sign is part of the badge and is sized TO it, whole, on one line, never
//  truncated. A lone glyph or a pair ("¥", "R$", "zł") is drawn as large as
//  it always was; every three-letter sign at one smaller size, so a column
//  of them reads as one hand; and nothing larger than fits inside the disc's
//  margin. Being the badge's, the sign does not grow with the reader's text
//  size — the badge does not either, and a sign that outgrew its disc is
//  what was cut.
//

import SwiftUI
import UIKit

struct SignBadge: View {
    @Environment(\.theme) private var theme

    let sign: String

    /// The disc — the size the picker's badge has always been.
    static let side: CGFloat = Tokens.Space.s32
    /// The sign's room across the disc: a margin each side, so no sign
    /// reaches the rim.
    static let room: CGFloat = side - 2 * 3
    /// The sizes a sign can be drawn at, largest first. The first is the
    /// size a lone glyph has always had here (the body's).
    static let sizes: [CGFloat] = [
        Tokens.TextSize.t15, 14, Tokens.TextSize.t13, 12, Tokens.TextSize.t11, 10, 9, 8,
    ]
    /// Where a sign of three characters or more starts on the ladder: the
    /// size at which the widest the core writes ("NOK", "CN¥", "MX$") fits,
    /// so they are all drawn alike.
    static let longSize: CGFloat = Tokens.TextSize.t11

    /// How wide `sign` is, whole, at `size` — rounded up to the point, as it
    /// is laid out.
    static func width(of sign: String, at size: CGFloat) -> CGFloat {
        let font = UIFont(name: FontName.regular, size: size) ?? .systemFont(ofSize: size)
        return (sign as NSString).size(withAttributes: [.font: font]).width.rounded(.up)
    }

    /// The size `sign` is drawn at: its kind's — the body's for one glyph or
    /// two, `longSize` for more — or the next size down the ladder at which
    /// all of it fits the room. A sign wider than any the core writes today
    /// is scaled to fit exactly: smaller, and still whole.
    static func size(for sign: String) -> CGFloat {
        let start = sign.count <= 2 ? sizes[0] : longSize
        if let fits = sizes.first(where: { $0 <= start && width(of: sign, at: $0) <= room }) { return fits }
        let least = sizes[sizes.count - 1]
        return least * room / max(width(of: sign, at: least), 1)
    }

    var body: some View {
        Circle()
            .fill(theme.bgRaised)
            .frame(width: Self.side, height: Self.side)
            .overlay(
                Text(verbatim: sign)
                    .font(.custom(FontName.regular, fixedSize: Self.size(for: sign)))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    // Its own width, always: a sign is never cut to "…".
                    .fixedSize()
            )
    }
}
