//
//  WalletSectionHeader.swift
//  VelaWallet
//
//  SectionHeader (spec 015 vocabulary #7): title (活动 / 资产) + trailing
//  text action with chevron (全部 ›).
//

import SwiftUI

struct WalletSectionHeader: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String
    let action: String
    /// Trailing chevron. Off for read-only trailing values such as the
    /// contacts 联系人 / 8 位 header (spec 018 mock C1).
    var chevron: Bool = true
    /// Spec 021: the trailing action opens a flow. Absent in the gallery.
    var onAction: (() -> Void)?
    /// The action is a control of its own and takes a full hit target
    /// (`Tokens.Layout.hitTarget`, 44) — the Explore home's resume header,
    /// whose 标签页 › opens the tab switcher (Android's `actionHitTarget`).
    /// Off by default: the other headers keep their targets as they are.
    var actionHitTarget = false

    /// How far the action's target reaches past its words on each side when
    /// `actionHitTarget` is on: any words at least 12 across make a target
    /// of at least 44 (`Tokens.Layout.hitTarget`) each way.
    static let hitOutset = Tokens.Space.s16

    var body: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(verbatim: title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            Spacer(minLength: Tokens.Space.s12)
            if let onAction {
                // The target grows AROUND the words and is laid out at their
                // size: the header keeps the board's height and baseline, and
                // only the area that answers a finger is larger.
                let outset = actionHitTarget ? Self.hitOutset : 0
                Button(action: onAction) {
                    trailing
                        .padding(outset)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .padding(-outset)
            } else {
                trailing
            }
        }
    }

    private var trailing: some View {
        HStack(spacing: Tokens.Space.s4) {
            Text(verbatim: action)
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            if chevron {
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    .foregroundStyle(theme.fgMuted)
            }
        }
    }
}

#Preview("Section header") {
    VStack(spacing: Tokens.Space.s24) {
        WalletSectionHeader(title: "活动", action: "全部")
        WalletSectionHeader(title: "资产", action: "全部")
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}
