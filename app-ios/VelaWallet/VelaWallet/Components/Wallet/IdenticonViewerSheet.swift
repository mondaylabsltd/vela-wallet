//
//  IdenticonViewerSheet.swift
//  VelaWallet
//
//  The identicon, big, above the address that drew it (founder call,
//  2026-08-26).
//
//  The artwork is a fingerprint of the address: the same address always draws
//  the same pattern, which is only useful once somebody has seen the two
//  together often enough to recognise one from the other. A 40pt avatar in a
//  header never teaches that. This does — and it opens from the artwork itself,
//  wherever the artwork is drawn, rather than from a settings page nobody
//  visits.
//

import SwiftUI

struct IdenticonViewerSheet: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    /// The seed, verbatim: what the artwork was drawn from.
    let address: String
    /// Whose it is, where the caller knows — the initials style draws from the
    /// NAME, so a viewer without one would show a different face than the row
    /// it was opened from.
    var name: String?
    let onClose: () -> Void

    @State private var copied = false

    var body: some View {
        VStack(spacing: Tokens.Space.s16) {
            // Not tappable: it is already the viewer.
            IdenticonAvatar(seed: address, size: WalletGeometry.identiconViewer,
                            name: name, tappable: false)
                .padding(.bottom, Tokens.Space.s8)

            Text(loc.t("componentsUi.identiconViewer.title"))
                .typeRole(Typography.title)
                .foregroundStyle(theme.fgBase)

            Text(loc.t("componentsUi.identiconViewer.caption"))
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgMuted)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)

            // The WHOLE address, wrapped rather than middle-truncated: a
            // fingerprint you can only see half of teaches half a habit.
            Text(verbatim: address)
                .monoRole(Typography.monoAddress)
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
                .frame(maxWidth: .infinity)
                .padding(Tokens.Space.s16)
                .background(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                        .fill(theme.bgSunken)
                )

            VStack(spacing: Tokens.Space.s12) {
                VelaButton(
                    title: loc.t(copied
                        ? "componentsUi.identiconViewer.copied"
                        : "componentsUi.identiconViewer.copyAddress"),
                    kind: .primary,
                    action: copy
                )
                VelaButton(
                    title: loc.t("componentsUi.identiconViewer.close"),
                    kind: .secondary,
                    action: onClose
                )
            }
            .padding(.top, Tokens.Space.s8)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }

    private func copy() {
        #if canImport(UIKit)
        UIPasteboard.general.string = address
        #endif
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(Interaction.copiedFeedbackSeconds))
            copied = false
        }
    }
}

#Preview("Identicon viewer") {
    IdenticonViewerSheet(
        loc: Loc(overrideTag: "zh"),
        address: "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c",
        onClose: {}
    )
    .themed(.light)
    .environment(\.identiconProvider, .previewSafe)
}
