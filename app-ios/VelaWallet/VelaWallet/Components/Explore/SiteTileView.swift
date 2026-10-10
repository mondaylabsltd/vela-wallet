//
//  SiteTileView.swift
//  VelaWallet
//
//  Favourites tile (spec 022 vocabulary): 56 avatar over a label one type
//  step below a row's, which is what lets "PancakeSwap" fit a quarter of the
//  frame without an ellipsis — as it does in mock E2.
//

import SwiftUI

struct SiteTileView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let tile: TileModel
    var onOpen: (String) -> Void = { _ in }

    var body: some View {
        Button {
            onOpen(tile.id)
        } label: {
            VStack(spacing: Tokens.Space.s8) {
                switch tile {
                case .site(let site):
                    SiteAvatarView(site: site, size: ExploreGeometry.tileAvatar)
                    Text(verbatim: site.name)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                case .add(let label):
                    LucideIcon(.plus, size: LucideIconSize.tileGlyph)
                        .foregroundStyle(theme.fgSubtle)
                        .frame(width: ExploreGeometry.tileAvatar,
                               height: ExploreGeometry.tileAvatar)
                        .background(theme.bgSunken, in: Circle())
                    Text(verbatim: label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .lineLimit(1)
                }
            }
            .frame(maxWidth: .infinity)
            // The whole tile answers, not only what is drawn on it (final
            // note F9). The shape belongs on the LABEL, as on `SiteRowView`:
            // a plain button hit-tests its label, and set on the button from
            // outside the shape changed the tile's accessibility frame and
            // nothing a finger could feel — the corners beside the round
            // mark took no tap.
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
