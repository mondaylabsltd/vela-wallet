//
//  TabCardView.swift
//  VelaWallet
//
//  One card in the tab switcher (mock E5): the page as it was last seen
//  (spec 079 — every card used to be the same drawing), the site's mark and
//  title, and the ✕ that closes it. The selected card carries an accent
//  border — the only accent on that screen — and, for VoiceOver, the
//  selected trait on the button named by its title.
//
//  Spec 082 RE12 (G25): ONE skeleton per cell, so every card in a row is the
//  same height. The preview's size comes from a clear box of the card's
//  aspect, never from its content, and the content sits in an overlay: a
//  snapshot top-cropped, a start page's sail, or — a tab not yet shown since
//  the launch — its avatar and host, never fake page bars that read as a
//  broken page. The "+" tile is the same skeleton (`TabCardPreview`).
//

import SwiftUI

/// What a card's preview shows.
enum TabPreview: Equatable {
    /// The page as it was last seen.
    case snapshot
    /// The start page's own tab: the sail.
    case startPage
    /// A tab not yet photographed: its mark and its host.
    case site(host: String)

    static func of(_ tab: TabModel) -> TabPreview {
        if tab.startPage { return .startPage }
        if tab.snapshot != nil { return .snapshot }
        return .site(host: tab.site?.host ?? tab.title)
    }
}

/// The preview's box: the card's aspect, content in an overlay.
struct TabCardPreview<Content: View>: View {
    var alignment: Alignment = .center
    @ViewBuilder let content: () -> Content

    var body: some View {
        Color.clear
            .aspectRatio(ExploreGeometry.tabCardAspect, contentMode: .fit)
            .frame(maxWidth: .infinity)
            .overlay(alignment: alignment) { content() }
            .clipped()
            .contentShape(Rectangle())
    }
}

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
                preview
            }
            .buttonStyle(.plain)
            // To VoiceOver the card is its title, and "this tab" is said, not
            // only drawn: the accent border was all that marked it, and a
            // snapshot or the start page's sail gave the button no words at
            // all (device pass 2026-10-09). Selected the way the app's other
            // picked rows are (WalletTabBar, the fee speeds).
            .accessibilityLabel(Text(verbatim: tab.title))
            .accessibilityAddTraits(tab.selected ? [.isSelected] : [])

            HStack(spacing: Tokens.Space.s8) {
                if let site = tab.site {
                    SiteAvatarView(site: site, size: Tokens.Space.s20)
                }
                Text(verbatim: tab.title)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                    // Said once, by the card's button above.
                    .accessibilityHidden(true)
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

    @ViewBuilder private var preview: some View {
        switch TabPreview.of(tab) {
        case .snapshot:
            TabCardPreview(alignment: .top) {
                if let snapshot = tab.snapshot {
                    Image(uiImage: snapshot)
                        .resizable()
                        .scaledToFill()
                        .accessibilityHidden(true)
                }
            }
        case .startPage:
            TabCardPreview {
                VelaMark(size: Tokens.Space.s48)
            }
        case .site(let host):
            TabCardPreview {
                VStack(spacing: Tokens.Space.s8) {
                    if let site = tab.site {
                        SiteAvatarView(site: site, size: Tokens.Space.s48)
                    }
                    Text(verbatim: host)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                        .padding(.horizontal, Tokens.Space.s12)
                }
            }
        }
    }
}
