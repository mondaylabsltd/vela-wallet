//
//  ConnectionPanelView.swift
//  VelaWallet
//
//  What a connected site can and cannot do (mock E7), in that order: who it
//  is, which account it sees, which network, then the sentence that says a
//  connection is not a permission to move money.
//

import SwiftUI

struct ConnectionPanelView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let connection: ConnectionModel
    let closeLabel: String
    var onClose: (() -> Void)?
    var onSwitch: () -> Void = {}
    var onDisconnect: () -> Void = {}
    var onApprove: () -> Void = {}
    var onReject: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            HStack(spacing: Tokens.Space.s12) {
                LetterAvatarView(letter: connection.site.letter, tint: connection.site.tint)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: connection.site.host)
                        .typeRole(Typography.title.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    HStack(spacing: Tokens.Space.s4) {
                        // The padlock follows the SCHEME, not the layout. A
                        // consent sheet that drew a lock over an http origin
                        // would be the most dangerous pixel in this cut.
                        if connection.secure {
                            LucideIcon(.lock, size: LucideIconSize.addressLock)
                        }
                        Text(verbatim: connection.statusLine)
                            .typeRole(Typography.rowSub.scaled(textScale))
                    }
                    .foregroundStyle(connection.secure ? theme.successBase : theme.fgMuted)
                }
                Spacer(minLength: Tokens.Space.s12)
                if let onClose {
                    Button(action: onClose) {
                        LucideIcon(.close, size: LucideIconSize.menuRow)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(closeLabel)
                }
            }

            Divider().overlay(theme.borderBase)

            Button(action: onSwitch) {
                HStack(spacing: Tokens.Space.s12) {
                    IdenticonAvatar(seed: connection.account.seed,
                                    size: ExploreGeometry.rowAvatar)
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(verbatim: connection.account.name)
                            .typeRole(Typography.rowTitle.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                        Text(verbatim: connection.account.address)
                            .typeRole(Typography.monoSmall.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                    }
                    Spacer(minLength: Tokens.Space.s12)
                    HStack(spacing: Tokens.Space.s4) {
                        Text(verbatim: connection.switchLabel)
                            .typeRole(Typography.rowSub.scaled(textScale))
                        LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    }
                    .foregroundStyle(theme.fgMuted)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)

            Divider().overlay(theme.borderBase)

            HStack {
                Text(verbatim: connection.networkLabel)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                Spacer()
                HStack(spacing: Tokens.Space.s8) {
                    Circle().fill(connection.network.dot)
                        .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                    Text(verbatim: connection.network.name)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                }
            }

            Text(verbatim: connection.explainer)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)

            if let consent = connection.consent {
                // Asking, not connected. The refusal is the outline and the
                // approval is the filled control, so the deliberate act is
                // the one that grants.
                HStack(spacing: Tokens.Space.s12) {
                    Button(action: onReject) {
                        Text(verbatim: consent.reject)
                            .typeRole(Typography.button.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .frame(maxWidth: .infinity)
                            .frame(height: Tokens.Control.lg)
                            .overlay(
                                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                                    .stroke(theme.borderStrong, lineWidth: Tokens.BorderWidth.hairline)
                            )
                            .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                    }
                    .buttonStyle(.plain)
                    Button(action: onApprove) {
                        Text(verbatim: consent.approve)
                            .typeRole(Typography.button.scaled(textScale))
                            .foregroundStyle(theme.onAccent)
                            .frame(maxWidth: .infinity)
                            .frame(height: Tokens.Control.lg)
                            .background(theme.accentBase, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                            .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                    }
                    .buttonStyle(.plain)
                }
            } else {
                Button(action: onDisconnect) {
                    Text(verbatim: connection.disconnect)
                        .typeRole(Typography.button.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .frame(maxWidth: .infinity)
                        .frame(height: Tokens.Control.lg)
                        .overlay(
                            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                                .stroke(theme.borderStrong, lineWidth: Tokens.BorderWidth.hairline)
                        )
                        .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                }
                .buttonStyle(.plain)
            }

            Text(verbatim: connection.footnote)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .frame(maxWidth: .infinity)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s16)
    }
}
