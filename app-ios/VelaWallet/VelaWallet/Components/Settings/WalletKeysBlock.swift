//
//  WalletKeysBlock.swift
//  VelaWallet
//
//  The keys that control this wallet, and their Ethereum backup (spec 062).
//
//  A key row says what a person can act on: what it is called, WHO is holding
//  it (the vault's own mark and name when the core's catalog knows), a short
//  fingerprint that tells two unnamed keys apart, and whether it is synced. The
//  backup is the block's last row and its only button.
//
//  De-containered and hairline-divided like the rest of settings: facts to
//  read, not cards to tap. Metrics are `SettingsRow`'s, so names line up with
//  the page below.
//

import SwiftUI

struct WalletKeysBlock: View {
    @Environment(\.theme) private var theme
    let model: WalletKeysModel
    var onTap: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s8) {
                Text(model.title)
                    .typeRole(Typography.fieldLabel)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                if !model.count.isEmpty {
                    Text(model.count)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isHeader)
            Text(model.subtitle)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.top, Tokens.Space.s2)
                .padding(.bottom, Tokens.Space.s8)

            if model.loading {
                // The shape of one row, so the block does not jump when the answer lands.
                HStack(spacing: Tokens.Space.s12) {
                    Circle().fill(theme.borderBase).frame(width: LucideIconSize.action, height: LucideIconSize.action)
                    Capsule().fill(theme.borderBase).frame(width: 140, height: 8)
                    Spacer()
                }
                .frame(minHeight: 52)
                .padding(.vertical, Tokens.Space.s12)
                .accessibilityHidden(true)
                SettingsDivider()
            } else {
                ForEach(model.rows) { row in
                    keyRow(row)
                    SettingsDivider()
                }
                if let note = model.note {
                    Text(note)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.vertical, Tokens.Space.s8)
                }
            }

            if let backup = model.backup {
                SettingsRow(row: backup, divider: false, onTap: onTap)
            }
        }
        .padding(.top, Tokens.Space.s16)
    }

    private func keyRow(_ row: WalletKeyRowModel) -> some View {
        HStack(spacing: Tokens.Space.s12) {
            PasskeyProviderMark(key: row.key, label: "", size: LucideIconSize.action, glyphFallback: true)
                .foregroundStyle(theme.fgMuted)
                .frame(width: LucideIconSize.action, height: LucideIconSize.action)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(row.name)
                    .typeRole(Typography.fieldLabel)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                (Text(row.holder)
                    + Text(row.fingerprint.isEmpty ? "" : "  ·  ")
                    + Text(row.fingerprint).font(.system(.caption, design: .monospaced)))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s8)
            if let badge = row.badge {
                Text(badge)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(row.badgeSynced ? theme.successBase : theme.fgSubtle)
                    .lineLimit(1)
            }
        }
        .frame(minHeight: 52)
        .padding(.vertical, Tokens.Space.s12)
        // One sentence for VoiceOver, not four fragments.
        .accessibilityElement(children: .ignore)
        .accessibilityLabel([row.name, row.holder, row.badge].compactMap { $0 }.joined(separator: ", "))
    }
}
