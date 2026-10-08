//
//  GroupManageSheetView.swift
//  VelaWallet
//
//  Manage groups (mock E3): the start page's two sections — 收藏 and
//  最近的 dApp — each with an eye. They can be hidden but never deleted, and
//  there is nothing to add (issue #465: no custom groups), so the sheet has
//  no grip, no trash and no "new group" row: an affordance that does nothing
//  is a lie about what is possible.
//

import SwiftUI

struct GroupManageSheetView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String
    let rows: [GroupManageRow]
    let closeLabel: String
    let hideLabel: String
    let showLabel: String
    var onClose: () -> Void = {}
    var onToggle: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            HStack {
                Text(verbatim: title)
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                Spacer()
                Button(action: onClose) {
                    LucideIcon(.close, size: LucideIconSize.menuRow)
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(closeLabel)
            }
            .padding(.vertical, Tokens.Space.s16)

            ForEach(rows) { row in
                HStack(spacing: Tokens.Space.s12) {
                    Text(verbatim: row.title)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    if let meta = row.meta {
                        Text(verbatim: meta)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer(minLength: Tokens.Space.s8)
                    Button {
                        onToggle(row.id)
                    } label: {
                        LucideIcon(row.hidden ? .eyeOff : .eye,
                                   size: LucideIconSize.menuRow)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(row.hidden ? showLabel : hideLabel)
                }
                // Hidden reads as hidden: the row dims, so the eye is a
                // confirmation rather than the only clue.
                .opacity(row.hidden ? Tokens.Opacity.dim : 1)
                .padding(.vertical, Tokens.Space.s16)
                Rectangle().fill(theme.borderBase).frame(height: Tokens.BorderWidth.hairline)
            }
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
    }
}
