//
//  WalletEmptyState.swift
//  VelaWallet
//
//  EmptyState (spec 015 vocabulary #11): outline icon, title, caption —
//  used inside the activity/assets sections (mock H2).
//

import SwiftUI

struct WalletEmptyState: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let icon: LucideGlyph
    let model: SectionEmptyModel

    var body: some View {
        VStack(spacing: Tokens.Space.s8) {
            LucideIcon(icon, size: LucideIconSize.empty)
                .foregroundStyle(theme.fgSubtle)
                .padding(.bottom, Tokens.Space.s4)
            Text(verbatim: model.title)
                .typeRole(Typography.emptyTitle.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            if !model.caption.isEmpty {
                Text(verbatim: model.caption)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
        }
        .multilineTextAlignment(.center)
        .frame(maxWidth: .infinity)
        .padding(.vertical, Tokens.Space.s32)
    }
}

#Preview("Empty states dark") {
    VStack(spacing: Tokens.Space.s24) {
        WalletEmptyState(icon: .inbox, model: SectionEmptyModel(
            title: "暂无交易记录", caption: "收款将实时显示在这里。"
        ))
        WalletEmptyState(icon: .walletUtility, model: SectionEmptyModel(
            title: "存入您的第一笔资产", caption: "点击此处查看地址并接收代币"
        ))
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}
