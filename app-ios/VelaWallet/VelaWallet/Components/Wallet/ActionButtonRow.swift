//
//  ActionButtonRow.swift
//  VelaWallet
//
//  ActionButtonRow (spec 015 vocabulary #6): 收款 / 转账 / 扫码 as three
//  equal raised cards, icon above label. Taps are inert in this feature
//  (no destination mocks on mobile — spec assumption).
//

import SwiftUI

/// One card of the row: glyph above label. Spec 018 reuses the component
/// with its own items (转账 / 收款 / 二维码) instead of a second row type.
struct ActionCardItem: Identifiable {
    /// The card's glyph: one per card in every row that draws them, and the
    /// same on every render. It was a fresh `UUID()` per init, and the home
    /// re-inits this row on every balance that lands — so each landing gave
    /// the three buttons new identities, SwiftUI replaced them, and a tap in
    /// flight on the old 转账 ended on a view that never saw it go down: the
    /// send flow never opened (2026-09-27/28, the amount device test's "the
    /// amount field never appeared", the tap made while €6.67 became €8.25).
    var id: LucideGlyph { icon }
    let icon: LucideGlyph
    let label: String
    /// Spec 021: the card is an entry into a flow. Absent in the gallery,
    /// where the dock is a picture.
    var action: (() -> Void)?
}

struct ActionButtonRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let items: [ActionCardItem]

    init(items: [ActionCardItem]) {
        self.items = items
    }

    /// Wallet home (spec 015): 收款 / 转账 / 扫码.
    init(
        model: ActionsModel,
        onReceive: (() -> Void)? = nil,
        onSend: (() -> Void)? = nil,
        onScan: (() -> Void)? = nil
    ) {
        self.items = [
            ActionCardItem(icon: .arrowDownLeft, label: model.receive, action: onReceive),
            ActionCardItem(icon: .arrowUpRight, label: model.send, action: onSend),
            ActionCardItem(icon: .scanLine, label: model.scan, action: onScan),
        ]
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            ForEach(items) { item in
                if let action = item.action {
                    Button(action: action) {
                        card(icon: item.icon, label: item.label).contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                } else {
                    card(icon: item.icon, label: item.label)
                }
            }
        }
    }

    private func card(icon: LucideGlyph, label: String) -> some View {
        VStack(spacing: Tokens.Space.s8) {
            LucideIcon(icon, size: LucideIconSize.action)
                .foregroundStyle(theme.fgBase)
            Text(verbatim: label)
                .typeRole(Typography.actionLabel.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, Tokens.Space.s12)
        .frame(minHeight: WalletGeometry.actionCardHeight)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r16).fill(theme.bgRaised))
    }
}

#Preview("Actions dark") {
    ActionButtonRow(model: ActionsModel(receive: "收款", send: "转账", scan: "扫码"))
        .padding(Tokens.Space.s24)
        .background(Tokens.dark.bgBase.color)
        .themed(.dark)
}

#Preview("Actions light") {
    ActionButtonRow(model: ActionsModel(receive: "Receive", send: "Send", scan: "Scan"))
        .padding(Tokens.Space.s24)
        .themed(.light)
}
