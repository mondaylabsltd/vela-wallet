//
//  MultisigLinkSheet.swift
//  VelaWallet
//
//  A multisig link the app was opened with (native parity N-I3): a proposal
//  to approve, an invite, the connect page, or one of them damaged.
//
//  **Honest about where it can be acted on.** Multisig runs in the web wallet
//  until the native machines land (Phase 2 M1–M6); until then this app does
//  not pretend to approve anything. It shows what the CORE read in the link
//  — the network, the multisig, the transaction's hash, the members the
//  invite names — and offers the web wallet in the system browser, where
//  the same link opens the same transaction, plus the link itself to copy.
//  Nothing is fetched, nothing is signed, nothing is stored. A damaged link
//  is said, in the web's own words.
//
//  Every word is the corpus's; the rows' values are the link's.
//

import SwiftUI

/// What the sheet says, built from the core's reading — apart from the view,
/// so a test can read every branch's words.
struct MultisigLinkModel: Equatable {
    /// A member as the invite names it: its label and kind, and its address
    /// on a line of its own, in the mono face.
    struct Member: Equatable, Hashable {
        let label: String
        var address: String?
    }

    struct Row: Equatable {
        let label: String
        let value: String
        /// An address or a hash: set in the mono face and never cut.
        var mono = false
    }

    let title: String
    /// One sentence under the title, when the link needs one said.
    var body: String?
    /// The words, in the warning ink: the link does not read.
    var broken = false
    var rows: [Row] = []
    /// The members an invite names, by label — "That's you" never: this app
    /// has no multisig list to know who "you" is in one.
    var members: [Member] = []
    var membersLabel: String?
    let openLabel: String
    let copyLabel: String
    let closeLabel: String
    let url: URL

    static func of(_ link: AppLink, loc: Loc, chainName: (Int) -> String?) -> MultisigLinkModel? {
        let k = I18nKeys.AppLinks.self
        func network(_ id: Int) -> String {
            chainName(id) ?? loc.t(k.missingNetwork, vars: ["chainId": String(id)])
        }
        func base(_ title: String, url: URL) -> MultisigLinkModel {
            MultisigLinkModel(
                title: title,
                openLabel: loc.t(k.openInBrowser),
                copyLabel: loc.t(k.copyLink),
                closeLabel: loc.t(k.close),
                url: url
            )
        }
        func txRows(_ tx: MultisigProposalRef) -> [Row] {
            [
                Row(label: loc.t(k.network), value: network(tx.chainId)),
                Row(label: loc.t(k.multisig), value: tx.multisig, mono: true),
                Row(label: loc.t(k.safeTxHash), value: tx.safeTxHash, mono: true),
            ]
        }
        switch link {
        case .proposal(let tx, let url):
            var model = base(loc.t(k.proposalTitle), url: url)
            model.rows = txRows(tx)
            return model
        case .invite(let invite, let url):
            var model = base(loc.t(k.inviteTitle), url: url)
            model.rows = [Row(label: loc.t(k.multisig), value: invite.multisig, mono: true)]
            if !invite.chainIds.isEmpty {
                model.body = loc.t(k.usedOn, vars: ["networks": invite.chainIds.map(network).joined(separator: ", ")])
            }
            let named = (invite.members ?? []).compactMap { member -> Member? in
                let kind = member.kind.flatMap { kind -> String? in
                    switch kind {
                    case "passkey": loc.t(k.kindVela)
                    case "wallet": loc.t(k.kindWallet)
                    case "contract": loc.t(k.kindContract)
                    default: nil
                    }
                }
                let parts = [member.name, kind].compactMap { $0 }.filter { !$0.isEmpty }
                guard !parts.isEmpty || member.address != nil else { return nil }
                return Member(label: parts.joined(separator: " · "), address: member.address)
            }
            if !named.isEmpty {
                model.members = named
                model.membersLabel = loc.t(k.invitedMembers)
            }
            return model
        case .connect(let connect, let url):
            var model = base(loc.t(k.connectTitle), url: url)
            model.body = loc.t(k.connectBody)
            if let tx = connect.tx { model.rows = txRows(tx) }
            return model
        case .broken(let kind, let url):
            let title: String
            let body: String
            switch kind {
            case .proposal:
                title = loc.t(k.proposalTitle)
                body = loc.t(k.badProposal)
            case .invite:
                title = loc.t(k.inviteTitle)
                body = loc.t(k.badInvite)
            case .inviteChecksum:
                title = loc.t(k.inviteTitle)
                body = loc.t(k.badInviteChecksum)
            case .connect:
                title = loc.t(k.connectTitle)
                body = loc.t(k.badConnect)
            }
            var model = base(title, url: url)
            model.body = body
            model.broken = true
            return model
        case .entry, .pay:
            return nil
        }
    }
}

/// A link held for the sheet, keyed for `sheet(item:)`.
struct PendingMultisigLink: Identifiable, Equatable {
    let link: AppLink
    var id: String {
        switch link {
        case .proposal(_, let url), .invite(_, let url), .connect(_, let url), .broken(_, let url):
            url.absoluteString
        case .entry(let entry): entry.rawValue
        case .pay: "pay"
        }
    }
}

struct MultisigLinkSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let model: MultisigLinkModel
    var onOpen: () -> Void
    var onCopy: () -> Void
    var onClose: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            Text(verbatim: model.title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("applink.title")
            if let body = model.body {
                Text(verbatim: body)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(model.broken ? theme.warningBase : theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("applink.body")
            }
            if !model.rows.isEmpty {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    ForEach(Array(model.rows.enumerated()), id: \.offset) { index, row in
                        if index > 0 { SettingsDivider() }
                        rowView(row)
                    }
                }
            }
            if let label = model.membersLabel {
                VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                    Text(verbatim: label)
                        .typeRole(Typography.label.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    ForEach(model.members, id: \.self) { member in
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            if !member.label.isEmpty {
                                Text(verbatim: member.label)
                                    .typeRole(Typography.rowSub.scaled(textScale))
                                    .foregroundStyle(theme.fgBase)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                            if let address = member.address {
                                Text(verbatim: address)
                                    .typeRole(Typography.monoSmall.scaled(textScale))
                                    .foregroundStyle(theme.fgMuted)
                                    .fixedSize(horizontal: false, vertical: true)
                            }
                        }
                    }
                }
                .accessibilityElement(children: .combine)
            }
            VStack(spacing: Tokens.Space.s12) {
                // Opening a page moves nothing: the neutral ink, not the accent.
                VelaButton(title: model.openLabel, kind: .ink, action: onOpen)
                    .accessibilityIdentifier("applink.open")
                VelaButton(title: model.copyLabel, kind: .secondary, action: onCopy)
                    .accessibilityIdentifier("applink.copy")
            }
            .padding(.top, Tokens.Space.s8)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s32)
        .contentSizedSheet(expected: MultisigLinkGeometry.expectedHeight)
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }

    /// A label over its value. An address and a hash WRAP — at the largest
    /// text sizes too — and are never cut: a hash with its middle missing is
    /// not a hash anybody can compare.
    private func rowView(_ row: MultisigLinkModel.Row) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: row.label)
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            Text(verbatim: row.value)
                .typeRole((row.mono ? Typography.monoSmall : Typography.body).scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
        }
        .padding(.vertical, Tokens.Space.s12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

enum MultisigLinkGeometry {
    /// What the sheet comes to at the default size, before it is measured.
    static let expectedHeight: CGFloat = 460
}
