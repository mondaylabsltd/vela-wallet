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
    /// The lock follows the scheme (spec 070) and says nothing more (spec
    /// 079, owner: https is not "safe"): closed and quiet for https, open in
    /// the warning colour for plain http, no words either way.
    var secure: Bool = true
    /// The http lock's screen-reader words (`connect.browser.a11yInsecure`).
    var insecureLabel = ""
    var onClose: () -> Void = {}
    var onPick: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            HStack(spacing: Tokens.Space.s12) {
                SiteAvatarView(site: site)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: site.host)
                        .typeRole(Typography.title.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    HStack(spacing: Tokens.Space.s4) {
                        LucideIcon(secure ? .lock : .lockOpen, size: LucideIconSize.addressLock)
                            .foregroundStyle(secure ? theme.fgMuted : theme.warningBase)
                            .accessibilityLabel(secure ? "" : insecureLabel)
                            .accessibilityHidden(secure)
                            .accessibilityIdentifier(secure ? "explore.menu.lock" : "explore.menu.insecure")
                        if !statusLine.isEmpty {
                            Text(verbatim: statusLine)
                                .typeRole(Typography.rowSub.scaled(textScale))
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
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
