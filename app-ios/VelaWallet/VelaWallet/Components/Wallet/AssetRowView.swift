//
//  AssetRowView.swift
//  VelaWallet
//
//  AssetRow (spec 015 vocabulary #9): TokenIcon with chain-dot badge,
//  symbol + chain name, trailing balance + fiat line. Variants: no-price
//  (orange 无价格, H4), masked (both lines dotted, H5), long-value
//  truncation (the value column wins width over the name column but never
//  overlaps it — spec edge case).
//

import SwiftUI

struct AssetRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: AssetRowModel

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            if let mark = model.mark {
                TokenIconView(mark: mark)
            } else {
                TokenIconView(ticker: model.ticker, badgeColor: model.badgeColor)
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: model.ticker)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                Text(verbatim: model.chain)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s12)
            VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
                Text(verbatim: model.balance)
                    .typeRole(Typography.rowValue.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                fiatLine
            }
            .layoutPriority(1)
        }
        .frame(minHeight: WalletGeometry.rowMinHeight)
    }

    @ViewBuilder private var fiatLine: some View {
        switch model.fiat {
        case .value(let text):
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .lineLimit(1)
        case .noPrice(let text):
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.warningBase)
                .lineLimit(1)
        case .masked:
            Text(verbatim: WalletFixtures.mask)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
        case .none:
            EmptyView()
        }
    }
}

#Preview("Asset rows dark") {
    VStack(spacing: Tokens.Space.s0) {
        AssetRowView(model: AssetRowModel(
            ticker: "BNB", chain: "BNB Chain", badgeColor: ChainPalette.bnb,
            balance: "0.8533", fiat: .value("$496.46"), masked: false
        ))
        AssetRowView(model: AssetRowModel(
            ticker: "CAKE", chain: "BNB Chain", badgeColor: ChainPalette.bnb,
            balance: "18.20", fiat: .noPrice("无价格"), masked: false
        ))
        AssetRowView(model: AssetRowModel(
            ticker: "ETH", chain: "Arbitrum", badgeColor: ChainPalette.arbitrum,
            balance: "••••", fiat: .masked, masked: true
        ))
        AssetRowView(model: AssetRowModel(
            ticker: "USDT", chain: "Ethereum", badgeColor: ChainPalette.ethereum,
            balance: "1,234,567.8901", fiat: .value("$1,234,567.89"), masked: false
        ))
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Asset row light") {
    AssetRowView(model: AssetRowModel(
        ticker: "BNB", chain: "BNB Chain", badgeColor: ChainPalette.bnb,
        balance: "0.8533", fiat: .value("$496.46"), masked: false
    ))
    .padding(Tokens.Space.s24)
    .themed(.light)
}
