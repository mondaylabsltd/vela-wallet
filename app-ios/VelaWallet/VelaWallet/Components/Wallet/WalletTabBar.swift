//
//  WalletTabBar.swift
//  VelaWallet
//
//  TabBar (spec 015 vocabulary #13): custom HStack, NOT TabView — 钱包 /
//  通讯录 / 探索 / 设置. Selected: lucide-derived solid glyph + accent tint;
//  unselected: lucide outline + subtle tint (FR-007, research D2 rev).
//
//  **Icons only** (founder, 2026-09-26): the visible labels truncated in
//  es/pt/de/it ("Configuración", "Einstellungen"), so the bar draws a 28pt
//  glyph centred in its 56pt and nothing else — as the web's mobile bar and
//  Android do; the desktop's sidebar keeps its words. Every tab keeps its
//  localized label as its ACCESSIBLE name, with the selected trait, so
//  VoiceOver still says "设置, 已选定" / "Settings, selected".
//

import SwiftUI

/// The four destinations. Only 钱包 (spec 015) and 通讯录 (spec 018) have
/// content; the other two remain inert selections.
enum WalletTab: String, CaseIterable {
    case wallet, contacts, explore, settings
}

struct WalletTabBar: View {
    @Environment(\.theme) private var theme

    let tabs: TabsModel
    /// Which destination reads as selected (solid glyph + accent).
    var selected: WalletTab = .wallet
    var onSelect: (WalletTab) -> Void = { _ in }

    private var items: [(tab: WalletTab, outline: LucideGlyph, fill: LucideGlyph, label: String)] {
        [
            (.wallet, .navWalletOutline, .navWalletSolid, tabs.wallet),
            (.contacts, .navContactsOutline, .navContactsSolid, tabs.contacts),
            (.explore, .navExploreOutline, .navExploreSolid, tabs.explore),
            (.settings, .navSettingsOutline, .navSettingsSolid, tabs.settings),
        ]
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s0) {
            ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                let isSelected = item.tab == selected
                Button {
                    // A tab is a button under the finger (the founder's rule:
                    // press = deformation + haptic; tabs too since
                    // 2026-09-26). Only a tab that CHANGES the destination
                    // buzzes — re-tapping the one in force is not a switch.
                    if !isSelected { VelaHaptic.press.play() }
                    onSelect(item.tab)
                } label: {
                    LucideIcon(isSelected ? item.fill : item.outline, size: LucideIconSize.tabBar)
                        .foregroundStyle(isSelected ? theme.accentBase : theme.fgSubtle)
                        // The whole quarter of the bar is the target, as before.
                        .frame(maxWidth: .infinity, maxHeight: .infinity)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                // The label the eye no longer reads is the name the ear does.
                .accessibilityLabel(item.label)
                .accessibilityAddTraits(isSelected ? .isSelected : [])
            }
        }
        .frame(height: WalletGeometry.tabBarHeight)
        .background(theme.bgBase)
        .overlay(alignment: .top) {
            Rectangle()
                .fill(theme.borderBase)
                .frame(height: Tokens.BorderWidth.hairline)
        }
    }
}

#Preview("Tab bar dark") {
    VStack(spacing: Tokens.Space.s24) {
        WalletTabBar(tabs: TabsModel(wallet: "钱包", contacts: "通讯录", explore: "探索", settings: "设置"))
        WalletTabBar(
            tabs: TabsModel(wallet: "钱包", contacts: "通讯录", explore: "探索", settings: "设置"),
            selected: .contacts
        )
    }
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Tab bar light") {
    WalletTabBar(tabs: TabsModel(wallet: "Wallet", contacts: "Contacts", explore: "Explore", settings: "Settings"))
        .themed(.light)
}
