//
//  BrowserToolbarView.swift
//  VelaWallet
//
//  The browsing bottom bar (mock E4), which REPLACES the four-tab bar while
//  a page is open — two navigation bars on a 390pt screen is where the page
//  would have gone. Back, forward, the account chip whose green dot IS the
//  connection state, the bookmark star, and the tab count.
//

import SwiftUI

struct BrowserToolbarView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let browser: BrowserModel
    let backLabel: String
    let forwardLabel: String
    let accountLabel: String
    let connectedLabel: String
    let bookmarkLabel: String
    let tabsLabel: String
    /// Said by the star when the page is already a favourite — the tap then
    /// removes it.
    var removeBookmarkLabel: String = ""
    var onBack: () -> Void = {}
    var onForward: () -> Void = {}
    var onAccount: () -> Void = {}
    var onBookmark: () -> Void = {}
    var onTabs: () -> Void = {}

    var body: some View {
        HStack {
            iconButton(.arrowLeft, label: backLabel, enabled: browser.canBack, action: onBack)
            Spacer()
            iconButton(.arrowRight, label: forwardLabel, enabled: browser.canForward,
                       action: onForward)
            Spacer()

            Button(action: onAccount) {
                HStack(spacing: Tokens.Space.s8) {
                    IdenticonAvatar(seed: browser.account.seed,
                                    size: ExploreGeometry.accountChipAvatar)
                    if browser.connected {
                        Circle()
                            .fill(theme.successBase)
                            .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                            .accessibilityLabel(connectedLabel)
                    }
                }
                .padding(.horizontal, Tokens.Space.s8)
                .frame(height: Tokens.Space.s32)
                .background(theme.bgRaised, in: Capsule())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(accountLabel)

            Spacer()
            // Filled when the site is a favourite, so the tap's meaning is
            // visible before it is made: add, or take away.
            Button(action: onBookmark) {
                LucideIcon(browser.bookmarked ? .starSolid : .star,
                           size: LucideIconSize.browserBarGlyph)
                    .foregroundStyle(browser.bookmarked ? theme.accentBase : theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(browser.bookmarked && !removeBookmarkLabel.isEmpty
                                ? removeBookmarkLabel : bookmarkLabel)
            Spacer()

            Button(action: onTabs) {
                Text(verbatim: String(browser.tabCount))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .frame(minWidth: ExploreGeometry.tabCount, minHeight: ExploreGeometry.tabCount)
                    .overlay(
                        RoundedRectangle(cornerRadius: Tokens.Radius.r4)
                            .stroke(theme.fgBase, lineWidth: Tokens.BorderWidth.emphasis)
                    )
            }
            .buttonStyle(.plain)
            .accessibilityLabel(tabsLabel)
        }
        .padding(.horizontal, Tokens.Space.s16)
        .frame(height: ExploreGeometry.browserBar)
        .overlay(alignment: .top) {
            Rectangle().fill(theme.borderBase).frame(height: Tokens.BorderWidth.hairline)
        }
    }

    private func iconButton(
        _ glyph: LucideGlyph, label: String, enabled: Bool, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            LucideIcon(glyph, size: LucideIconSize.browserBarGlyph)
                .foregroundStyle(enabled ? theme.fgBase : theme.fgSubtle)
                .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
        }
        .buttonStyle(.plain)
        .disabled(!enabled)
        .accessibilityLabel(label)
    }
}
