//
//  SiteMenuSheetView.swift
//  VelaWallet
//
//  The ⋯ sheet over a page (mock E6): who the site is, then the seven things
//  you can do to it — refresh, share, copy, favourite, open in Safari,
//  disconnect, close.
//

import SwiftUI

struct SiteMenuSheetView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let site: SiteModel
    let statusLine: String
    let items: [SiteMenuItem]
    let closeLabel: String
    /// The padlock and the green are for a secure origin only (spec 070):
    /// an http page's menu names its host in plain text.
    var secure: Bool = true
    var onClose: () -> Void = {}
    var onPick: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            HStack(spacing: Tokens.Space.s12) {
                LetterAvatarView(letter: site.letter, tint: site.tint)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: site.host)
                        .typeRole(Typography.title.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    HStack(spacing: Tokens.Space.s4) {
                        if secure {
                            LucideIcon(.lock, size: LucideIconSize.addressLock)
                        }
                        Text(verbatim: statusLine)
                            .typeRole(Typography.rowSub.scaled(textScale))
                    }
                    .foregroundStyle(secure ? theme.successBase : theme.fgMuted)
                }
                Spacer(minLength: Tokens.Space.s12)
                Button(action: onClose) {
                    LucideIcon(.close, size: LucideIconSize.menuRow)
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(closeLabel)
            }
            .padding(.vertical, Tokens.Space.s16)

            ForEach(items) { item in
                Button {
                    onPick(item.id)
                } label: {
                    HStack(spacing: Tokens.Space.s16) {
                        LucideIcon(LucideGlyph(rawValue: item.icon) ?? .link2,
                                   size: LucideIconSize.menuRow)
                        Text(verbatim: item.label)
                            .typeRole(Typography.body.scaled(textScale))
                        Spacer()
                    }
                    .foregroundStyle(item.danger ? theme.errorBase : theme.fgBase)
                    .padding(.vertical, Tokens.Space.s16)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                if item.id != items.last?.id {
                    Rectangle().fill(theme.borderBase).frame(height: Tokens.BorderWidth.hairline)
                }
            }
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
    }
}
