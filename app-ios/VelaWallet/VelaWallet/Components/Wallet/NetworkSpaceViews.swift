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
                // The amber runs up behind the status bar; the words stay
                // under it.
                .background(theme.warningSoft.ignoresSafeArea(edges: .top))
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("testnet-bar")
    }
}

/// Over the whole app: the parallel space's marker (debug builds), the
/// testnet bar, and the switch the header's label, the bar and Settings →
/// Networks open.
///
/// The bar is stacked ABOVE the app, not set as a safe-area inset: the pages
/// lay their tops out against the window, and an inset slid the home's header
/// — the account's name and the very label that says "Testnet" — under it
/// (iPhone 15 Pro, 2026-10-27). Stacked, the app below starts where the bar
/// ends.
struct NetworkSpaceLayer: ViewModifier {
    let space: NetworkSpace
    let loc: Loc
    let parallelSpace: Bool
    /// The person chose the other space: through the intents.
    let onSwitch: (NetworkSpaceKind) -> Void

    func body(content: Content) -> some View {
        VStack(spacing: 0) {
            if space.testnet {
                TestnetBar(text: loc.t("networkSpace.bar")) { space.switcherOpen = true }
            }
            #if DEBUG
            // Stacked for the same reason, and under the bar: the amber
            // owns the status bar's band.
            if parallelSpace { ParallelSpaceBadge() }
            #endif
            content
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
