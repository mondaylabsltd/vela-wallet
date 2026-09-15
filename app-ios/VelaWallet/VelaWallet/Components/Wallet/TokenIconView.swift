//
//  TokenIconView.swift
//  VelaWallet
//
//  TokenIcon (spec 015 vocabulary #10): circular token glyph rendered as a
//  fixture-supplied ticker lettermark (first three characters) with a
//  bottom-trailing chain-dot badge.
//

import SwiftUI

struct TokenIconView: View {
    @Environment(\.theme) private var theme

    let ticker: String
    let badgeColor: Color
    /// The whole mark, where the caller has one (058). Without it this draws
    /// exactly what it always drew — which is what keeps every fixture, board
    /// and preview unchanged.
    var mark: TokenMarkModel?

    init(ticker: String, badgeColor: Color) {
        self.ticker = ticker
        self.badgeColor = badgeColor
        self.mark = nil
    }

    init(mark: TokenMarkModel) {
        self.ticker = mark.ticker
        self.badgeColor = mark.badgeColor
        self.mark = mark
    }

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            RemoteLogoView(urls: mark?.logoURLs ?? [], size: WalletGeometry.rowIcon) {
                // The lettermark IS the fallback — a whole mark, not a hole
                // where an asset's identity should be.
                Circle()
                    .fill(theme.bgRaised)
                    .overlay {
                        Text(verbatim: String(ticker.prefix(3)).uppercased())
                            .typeRole(Typography.tokenGlyph)
                            .foregroundStyle(theme.fgBase)
                    }
            }
            badge
        }
        .accessibilityHidden(true)
    }

    /// The chain badge: its logo where there is one, its colour otherwise, and
    /// **nothing** when it would repeat the token (ETH on Ethereum).
    @ViewBuilder private var badge: some View {
        if mark?.badgeHidden == true {
            EmptyView()
        } else if let url = mark?.badgeLogoURL {
            RemoteLogoView(urls: [url], size: WalletGeometry.badge) {
                Circle().fill(badgeColor)
            }
            .padding(WalletGeometry.badgeRing)
            .background(Circle().fill(theme.bgBase))
        } else {
            ChainBadgeDot(color: badgeColor)
        }
    }
}

#Preview("Token icons") {
    HStack(spacing: Tokens.Space.s16) {
        TokenIconView(ticker: "BNB", badgeColor: ChainPalette.bnb)
        TokenIconView(ticker: "ETH", badgeColor: ChainPalette.arbitrum)
        TokenIconView(ticker: "XDAI", badgeColor: ChainPalette.gnosis)
        TokenIconView(ticker: "USDC", badgeColor: ChainPalette.polygon)
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}
