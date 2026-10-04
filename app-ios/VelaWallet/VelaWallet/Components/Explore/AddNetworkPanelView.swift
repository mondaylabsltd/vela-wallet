//
//  AddNetworkPanelView.swift
//  VelaWallet
//
//  Spec 100: a page asks Vela to add a network. The connection sheet's
//  header — the site and the question — then what would be added, and
//  Settings' own check: its pill, its list, its callout, and only the buttons
//  the core allows. Every judgement is the core's (`NetView.dapp_add`).
//

import SwiftUI

struct AddNetworkPanelView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: AddNetworkSheetModel
    var onApprove: () -> Void = {}
    var onRetry: () -> Void = {}
    /// Any way out — the ✕, Cancel or Done. The core decides what it answers.
    var onDismiss: () -> Void = {}
    var onSetupTool: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            HStack(alignment: .top, spacing: Tokens.Space.s12) {
                SiteAvatarView(site: model.site)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: model.title)
                        .typeRole(Typography.title.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .accessibilityAddTraits(.isHeader)
                    Text(verbatim: model.lead)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                        .accessibilityIdentifier("explore.addNetwork.lead")
                }
                Spacer(minLength: Tokens.Space.s12)
                Button(action: onDismiss) {
                    LucideIcon(.close, size: LucideIconSize.menuRow)
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.dismiss)
            }

            Divider().overlay(theme.borderBase)

            VStack(spacing: Tokens.Space.s8) {
                ForEach(model.rows) { row in
                    HStack(spacing: Tokens.Space.s12) {
                        Text(verbatim: row.label)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                        Spacer()
                        Text(verbatim: row.value)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
            }

            if let fromSite = model.fromSite {
                SettingsCallout(callout: CalloutModel(tone: .warning, text: fromSite))
            }
            if let pill = model.pill { StatusPill(pill: pill) }
            if let title = model.checksTitle {
                SettingsCheckList(title: title, items: model.checks)
            }
            if let note = model.note {
                SettingsCallout(callout: CalloutModel(tone: .warning, text: note))
            }
            if let add = model.add {
                VelaButton(title: add, kind: .primary, action: onApprove)
                    .accessibilityIdentifier("explore.addNetwork.approve")
            }
            if let retry = model.retry {
                VelaButton(title: retry, kind: .primary, action: onRetry)
            }
            if let setupTool = model.setupTool {
                VelaButton(title: setupTool, kind: .secondary, action: onSetupTool)
            }
            VelaButton(title: model.dismiss, kind: .secondary, action: onDismiss)
                .accessibilityIdentifier("explore.addNetwork.dismiss")
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.vertical, Tokens.Space.s16)
    }
}
