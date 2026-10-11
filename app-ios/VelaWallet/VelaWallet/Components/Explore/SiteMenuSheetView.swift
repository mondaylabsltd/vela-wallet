//
//  SiteMenuSheetView.swift
//  VelaWallet
//
//  The ⋯ sheet over a page (mock E6): who the site is, then the things you
//  can do to it — forward, refresh, share, copy, favourite, open in Safari,
//  disconnect, close. Forward leads since DESIGN N moved it out of the old
//  bottom toolbar, and is greyed — never hidden — while there is nothing
//  ahead, so the rows under a thumb keep their places.
//

import SwiftUI

struct SiteMenuSheetView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let site: SiteModel
    let statusLine: String
    let items: [SiteMenuItem]
    let closeLabel: String
    /// The mark follows the scheme (spec 070) and says nothing more (spec
    /// 079, owner: https is not "safe") — the core's `OriginLock`: the
    /// padlock for https only, a neutral mark for a local page, the open
    /// lock in the warning colour for public http. `nil`: a row that is only
    /// a host names no scheme, and wears no mark.
    var lock: OriginLock? = .secure
    /// The http lock's screen-reader words (`connect.browser.a11yInsecure`).
    var insecureLabel = ""
    /// A local page's mark, named (`connect.browser.a11yLocal`).
    var localLabel = ""
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
                        if let lock {
                            SiteLockMark(lock: lock, localLabel: localLabel, insecureLabel: insecureLabel,
                                         idPrefix: "explore.menu")
                        }
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
                    .foregroundStyle(item.disabled ? theme.fgSubtle
                                     : item.danger ? theme.errorBase : theme.fgBase)
                    .opacity(item.disabled ? Tokens.Opacity.disabled : 1)
                    .padding(.vertical, Tokens.Space.s16)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .disabled(item.disabled)
                if item.id != items.last?.id {
                    Rectangle().fill(theme.borderBase).frame(height: Tokens.BorderWidth.hairline)
                }
            }
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
    }
}
