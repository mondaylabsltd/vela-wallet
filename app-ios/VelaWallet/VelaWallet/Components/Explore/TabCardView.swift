//
//  TabCardView.swift
//  VelaWallet
//
//  One card in the tab switcher (mock E5): a stand-in preview, the site's
//  mark and title, and the ✕ that closes it. The selected card carries an
//  accent border — the only accent on that screen.
//

import SwiftUI

struct TabCardView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let tab: TabModel
    let closeLabel: String
    var onOpen: (String) -> Void = { _ in }
    var onClose: (String) -> Void = { _ in }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            Button {
                onOpen(tab.id)
            } label: {
                VStack(spacing: Tokens.Space.s8) {
                    if tab.startPage {
                        VelaMark(size: Tokens.Space.s48)
                        Capsule().fill(theme.bgRaised).frame(height: Tokens.Space.s12)
                            .padding(.horizontal, Tokens.Space.s16)
                    } else {
                        RoundedRectangle(cornerRadius: Tokens.Radius.r8).fill(theme.bgRaised)
                            .frame(height: Tokens.Space.s24)
                        RoundedRectangle(cornerRadius: Tokens.Radius.r8).fill(theme.bgRaised)
                            .frame(height: Tokens.Space.s24)
                        Capsule().fill(tab.site?.tint ?? theme.bgRaised)
                            .frame(height: Tokens.Space.s20)
                    }
                }
                .padding(Tokens.Space.s16)
                .frame(maxWidth: .infinity)
                .aspectRatio(ExploreGeometry.tabCardAspect, contentMode: .fit)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            HStack(spacing: Tokens.Space.s8) {
                if let site = tab.site {
                    SiteAvatarView(site: site, size: Tokens.Space.s20)
                }
                Text(verbatim: tab.title)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                Spacer(minLength: Tokens.Space.s4)
                Button {
                    onClose(tab.id)
                } label: {
                    LucideIcon(.close, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(closeLabel)
            }
            .padding(Tokens.Space.s12)
            .background(theme.bgRaised)
        }
        .background(theme.bgSunken)
        .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r16))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(tab.selected ? theme.accentBase : .clear,
                        lineWidth: Tokens.BorderWidth.emphasis)
        )
    }
}
