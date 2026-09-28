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
    /// The http lock's screen-reader words (`connect.browser.a11yInsecure`).
    var insecureLabel = ""
    var onClose: (() -> Void)?
    var onSwitch: () -> Void = {}
    var onDisconnect: () -> Void = {}
    var onApprove: () -> Void = {}
    var onReject: () -> Void = {}
    /// A network picked for this site, by chain id.
    var onPickNetwork: (Int) -> Void = { _ in }

    @State private var pickingNetwork = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            // Spec 079: a site ASKING says what it asks — "连接到 {host}" —
            // the title the model always carried and the panel never drew.
            if connection.consent != nil, !connection.title.isEmpty {
                Text(verbatim: connection.title)
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("explore.connection.title")
            }
            HStack(spacing: Tokens.Space.s12) {
                SiteAvatarView(site: connection.site)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: connection.site.host)
                        .typeRole(Typography.title.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    HStack(spacing: Tokens.Space.s4) {
                        // The lock follows the SCHEME, not the layout, and
                        // says nothing more (spec 079, owner: "你标记的安全站点
                        // 只是https 而已，并不代表这个站点真的安全"): closed and
                        // quiet for https, open in the warning colour for http,
                        // and no word either way. "已连接" stays — a fact about
                        // the connection, not a claim about the site.
                        LucideIcon(connection.secure ? .lock : .lockOpen, size: LucideIconSize.addressLock)
                            .foregroundStyle(connection.secure ? theme.fgMuted : theme.warningBase)
                            .accessibilityLabel(connection.secure ? "" : insecureLabel)
                            .accessibilityHidden(connection.secure)
                            .accessibilityIdentifier(connection.secure ? "explore.connection.lock" : "explore.connection.insecure")
                        if !connection.statusLine.isEmpty {
                            Text(verbatim: connection.statusLine)
                                .typeRole(Typography.rowSub.scaled(textScale))
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
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
                                    size: ExploreGeometry.rowAvatar,
                                    tappable: false)
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

            // The SITE's network (spec 070: per origin, kept across launches).
            // With choices it is a control; a pick moves this site only, and
            // the page hears `chainChanged`.
            Button {
                guard !connection.networks.isEmpty else { return }
                pickingNetwork.toggle()
            } label: {
                HStack {
                    Text(verbatim: connection.networkLabel)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                    Spacer()
                    HStack(spacing: Tokens.Space.s8) {
                        // The chain's logo; the drawn dot until it lands.
                        RemoteLogoView(urls: [connection.networkLogoUrl].compactMap { $0 },
                                       size: Tokens.Space.s20) {
                            Circle().fill(connection.network.dot)
                                .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                                .frame(width: Tokens.Space.s20, height: Tokens.Space.s20)
                        }
                        Text(verbatim: connection.network.name)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                        if !connection.networks.isEmpty {
                            LucideIcon(pickingNetwork ? .chevronDown : .chevronRight,
                                       size: LucideIconSize.smallChevron)
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityIdentifier("explore.connection.network")

            if pickingNetwork {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    ForEach(connection.networks) { network in
                        Button {
                            pickingNetwork = false
                            onPickNetwork(network.id)
                        } label: {
                            HStack(spacing: Tokens.Space.s12) {
                                // Spec 079: each network's logo, as elsewhere
                                // in the wallet, and what the account holds
                                // there — nothing where it is not known.
                                RemoteLogoView(urls: [network.logoUrl].compactMap { $0 },
                                               size: Tokens.Space.s24) {
                                    Circle().fill(network.dot)
                                        .frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                                        .frame(width: Tokens.Space.s24, height: Tokens.Space.s24)
                                }
                                Text(verbatim: network.name)
                                    .typeRole(Typography.body.scaled(textScale))
                                    .foregroundStyle(theme.fgBase)
                                Spacer()
                                if let amount = network.amount {
                                    Text(verbatim: amount)
                                        .typeRole(Typography.rowSub.scaled(textScale))
                                        .foregroundStyle(theme.fgMuted)
                                        .accessibilityIdentifier("explore.connection.network.amount")
                                }
                                if network.id == connection.chainId {
                                    LucideIcon(.check, size: LucideIconSize.smallChevron)
                                        .foregroundStyle(theme.successBase)
                                }
                            }
                            .padding(.vertical, Tokens.Space.s8)
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
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
