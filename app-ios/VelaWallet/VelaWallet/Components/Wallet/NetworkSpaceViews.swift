//
//  NetworkSpaceViews.swift
//  VelaWallet
//
//  The space in front, said where a person looks (spec 105): the label beside
//  the account's name, and in the testnet space a bar over every screen —
//  "nothing here has real value" is not a thing to say once and forget.
//

import SwiftUI

/// "Mainnet" / "Testnet" beside the account's name. A tap offers the other
/// space.
struct NetworkSpaceChip: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let label: String
    let testnet: Bool
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            Text(verbatim: label)
                .typeRole(Typography.chip.scaled(textScale))
                .foregroundStyle(testnet ? theme.warningBase : theme.fgMuted)
                .lineLimit(1)
                .padding(.horizontal, Tokens.Space.s8)
                .padding(.vertical, Tokens.Space.s4)
                .background(
                    Capsule().fill(testnet ? theme.warningSoft : theme.bgSunken)
                )
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("network-space")
    }
}

/// The testnet space's bar, over every screen. As a top safe-area inset, like
/// the parallel space's badge: it moves the page down rather than covering
/// the account's name.
struct TestnetBar: View {
    @Environment(\.theme) private var theme

    let text: String
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            Text(verbatim: text)
                .typeRole(Typography.label)
                .foregroundStyle(theme.warningBase)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
                .frame(maxWidth: .infinity)
                .padding(.vertical, Tokens.Space.s4)
                .background(theme.warningSoft)
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("testnet-bar")
    }
}

/// Over the whole app: the parallel space's marker (debug builds), the
/// testnet bar, and the switch the header's label, the bar and Settings →
/// Networks open. A safe-area inset rather than an overlay — it MOVES the
/// content down instead of covering it; an overlay clipped the account name.
struct NetworkSpaceLayer: ViewModifier {
    let space: NetworkSpace
    let loc: Loc
    let parallelSpace: Bool
    /// The person chose the other space: through the intents.
    let onSwitch: (NetworkSpaceKind) -> Void

    func body(content: Content) -> some View {
        content
            .safeAreaInset(edge: .top, spacing: 0) {
                VStack(spacing: 0) {
                    #if DEBUG
                    if parallelSpace { ParallelSpaceBadge() }
                    #endif
                    if space.testnet {
                        TestnetBar(text: loc.t("networkSpace.bar")) { space.switcherOpen = true }
                    }
                }
            }
            .confirmationDialog(
                loc.t("networkSpace.title"),
                isPresented: Binding(get: { space.switcherOpen }, set: { space.switcherOpen = $0 }),
                titleVisibility: .visible
            ) {
                let to = space.current.other
                Button(loc.t(to == .testnet ? "networkSpace.toTestnet" : "networkSpace.toMainnet")) {
                    onSwitch(to)
                }
                Button(loc.t("common.cancel"), role: .cancel) {}
            } message: {
                Text(verbatim: loc.t(space.current.other == .testnet
                    ? "networkSpace.testnetHint" : "networkSpace.mainnetHint"))
            }
    }
}
