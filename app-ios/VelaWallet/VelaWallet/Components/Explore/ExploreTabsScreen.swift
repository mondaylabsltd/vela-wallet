//
//  ExploreTabsScreen.swift
//  VelaWallet
//
//  The tab switcher (mock E5): a two-column grid of cards, a "+" that opens
//  the start page, and the one destructive affordance — 关闭全部标签页 —
//  kept quiet at the bottom rather than beside every card.
//

import SwiftUI

struct ExploreTabsScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let tabs: [TabModel]
    let copy: TabsScreenCopy
    var onDone: () -> Void = {}
    var onOpen: (String) -> Void = { _ in }
    var onClose: (String) -> Void = { _ in }
    var onNew: () -> Void = {}
    var onCloseAll: () -> Void = {}

    /// Top-aligned (spec 082 RE12): a card is never centred against a
    /// taller neighbour.
    static let columns = [GridItem(.flexible(), spacing: Tokens.Space.s16, alignment: .top),
                          GridItem(.flexible(), spacing: Tokens.Space.s16, alignment: .top)]

    var body: some View {
        ScrollView {
            VStack(spacing: Tokens.Space.s16) {
                HStack {
                    Text(verbatim: copy.title)
                        .typeRole(Typography.display.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Spacer()
                    Button(action: onDone) {
                        Text(verbatim: copy.done)
                            .typeRole(Typography.button.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                    }
                    .buttonStyle(.plain)
                }
                .padding(.top, Tokens.Space.s20)

                LazyVGrid(columns: Self.columns, alignment: .center, spacing: Tokens.Space.s16) {
                    ForEach(tabs) { tab in
                        TabCardView(tab: tab, closeLabel: copy.close,
                                    onOpen: onOpen, onClose: onClose)
                    }
                    // The same skeleton as a card: the preview box, then a
                    // caption row — 新建标签页 whole, never squeezed into the
                    // preview beside the "+".
                    Button(action: onNew) {
                        VStack(spacing: Tokens.Space.s0) {
                            TabCardPreview {
                                LucideIcon(.plus, size: LucideIconSize.action)
                                    .foregroundStyle(theme.fgMuted)
                            }
                            HStack {
                                Text(verbatim: copy.newTab)
                                    .typeRole(Typography.rowSub.scaled(textScale))
                                    .foregroundStyle(theme.fgMuted)
                                    .fixedSize(horizontal: false, vertical: true)
                                Spacer(minLength: Tokens.Space.s0)
                            }
                            .padding(Tokens.Space.s12)
                            .background(theme.bgRaised)
                        }
                        .background(theme.bgSunken)
                        .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r16))
                        .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r16))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("explore.tabs.new")
                }

                Button(action: onCloseAll) {
                    Text(verbatim: copy.closeAll)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .padding(Tokens.Space.s20)
                }
                .buttonStyle(.plain)
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
        }
        .background(theme.bgBase)
    }
}
