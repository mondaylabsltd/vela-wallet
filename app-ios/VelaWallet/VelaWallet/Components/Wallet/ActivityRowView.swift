//
//  ActivityRowView.swift
//  VelaWallet
//
//  ActivityRow (spec 015 vocabulary #8): leading direction glyph in a
//  raised circle with a chain-dot badge, title + subtitle, trailing signed
//  amount (+success / −foreground) with unit. `masked` renders the amount
//  as dots while units stay visible (H5); received rows keep the success
//  color. The trailing text concatenation lets long amounts wrap the unit
//  onto a second line (H7 −0.0000001 BNB) instead of clipping.
//

import SwiftUI

struct ActivityRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ActivityRowModel

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            iconCircle
            // Amounts always render fully (spec: numbers never clip); the
            // subtitle is the yielding element — it middle-truncates when the
            // row runs out of width. The title keeps its natural width.
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: model.title)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                    .fixedSize(horizontal: true, vertical: false)
                Text(verbatim: model.subtitle)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: Tokens.Space.s12)
            // Inline when it fits (H1 −2 POL); otherwise the unit drops to a
            // second line (mock H7 −0.0000001 / BNB).
            ViewThatFits(in: .horizontal) {
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s4) {
                    amountText.lineLimit(1)
                    unitText.lineLimit(1)
                }
                VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
                    amountText
                        .lineLimit(1)
                        .minimumScaleFactor(WalletGeometry.heroMinScale)
                    unitText.lineLimit(1)
                }
            }
            .layoutPriority(1)
        }
        .frame(minHeight: WalletGeometry.rowMinHeight)
    }

    private var amountColor: Color {
        model.positive ? theme.successBase : theme.fgBase
    }

    private var amountText: Text {
        Text(verbatim: model.amount)
            .font(Typography.rowValue.scaled(textScale).font)
            .foregroundStyle(amountColor)
    }

    private var unitText: Text {
        Text(verbatim: model.unit)
            .font(Typography.rowSub.scaled(textScale).font)
            .foregroundStyle(theme.fgMuted)
    }

    private var glyph: LucideGlyph {
        switch model.kind {
        case .sent: .arrowUpRight
        case .received: .arrowDownLeft
        case .dapp: .link2
        }
    }

    private var iconCircle: some View {
        ZStack(alignment: .bottomTrailing) {
            Circle()
                .fill(theme.bgRaised)
                .frame(width: WalletGeometry.rowIcon, height: WalletGeometry.rowIcon)
                .overlay {
                    LucideIcon(glyph, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(model.kind == .received ? theme.successBase : theme.fgBase)
                }
            if let url = model.badgeLogoURL {
                RemoteLogoView(urls: [url], size: WalletGeometry.badge) {
                    Circle().fill(model.badgeColor)
                }
                .padding(WalletGeometry.badgeRing)
                .background(Circle().fill(theme.bgBase))
            } else {
                ChainBadgeDot(color: model.badgeColor)
            }
        }
    }
}

/// Chain-dot badge shared by activity rows and token icons: fixture color
/// dot ringed by the base background so it reads on any circle.
struct ChainBadgeDot: View {
    @Environment(\.theme) private var theme
    let color: Color

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: WalletGeometry.badge, height: WalletGeometry.badge)
            .padding(WalletGeometry.badgeRing)
            .background(Circle().fill(theme.bgBase))
    }
}

#Preview("Activity rows dark") {
    VStack(spacing: Tokens.Space.s0) {
        ActivityRowView(model: ActivityRowModel(
            kind: .sent, title: "已发送", subtitle: "至 hold on",
            amount: "−2", unit: "POL", positive: false, masked: false,
            badgeColor: ChainPalette.polygon
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .received, title: "已收到", subtitle: "来自 0x9F3c…21aE",
            amount: "+120", unit: "USDT", positive: true, masked: false,
            badgeColor: ChainPalette.ethereum
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .dapp, title: "dApp 交易", subtitle: "PancakeSwap · BNB Chain",
            amount: "−0.05", unit: "BNB", positive: false, masked: false,
            badgeColor: ChainPalette.bnb
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .received, title: "已收到", subtitle: "来自 Alice",
            amount: "••••", unit: "USDC", positive: true, masked: true,
            badgeColor: ChainPalette.base
        ))
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Activity row light") {
    ActivityRowView(model: ActivityRowModel(
        kind: .sent, title: "Sent", subtitle: "To Alexandra",
        amount: "−1234.5678", unit: "POL", positive: false, masked: false,
        badgeColor: ChainPalette.polygon
    ))
    .padding(Tokens.Space.s24)
    .themed(.light)
}
