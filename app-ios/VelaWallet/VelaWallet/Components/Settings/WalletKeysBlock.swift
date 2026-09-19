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
    /// Which rows are open, by position: the list is the founding order.
    @State private var open: Set<Int>
    /// `"row:label"` of the value just copied, for the button's one-second "Copied".
    @State private var copied = ""

    init(model: WalletKeysModel, onTap: @escaping (String) -> Void = { _ in }, initiallyOpen: Set<Int> = []) {
        self.model = model
        self.onTap = onTap
        _open = State(initialValue: initiallyOpen)
    }

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
                // PUBLIC keys: "back up keys" read as handing over the keys themselves.
                Text(model.backupExplain)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.leading, LucideIconSize.action + Tokens.Space.s12)
                    .padding(.bottom, Tokens.Space.s8)
            }
        }
        .padding(.top, Tokens.Space.s16)
    }

    private func keyRow(_ row: WalletKeyRowModel) -> some View {
        let expandable = !row.details.isEmpty
        let isOpen = open.contains(row.id)
        return VStack(alignment: .leading, spacing: 0) {
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
                    // Under the name, not beside it: a phone has no room for a
                    // name AND two pills on one line, and the name is what loses.
                    if !row.pills.isEmpty {
                        HStack(spacing: Tokens.Space.s4) {
                            ForEach(row.pills, id: \.text) { pill in
                                let tint = tint(for: pill.tone)
                                Text(pill.text)
                                    .typeRole(Typography.flowCaption)
                                    .foregroundStyle(tint)
                                    .lineLimit(1)
                                    .padding(.horizontal, Tokens.Space.s8)
                                    .padding(.vertical, Tokens.Space.s2)
                                    .overlay(Capsule().strokeBorder(tint, lineWidth: 1))
                            }
                        }
                        .padding(.top, Tokens.Space.s2)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                if expandable {
                    LucideIcon(.chevronDown, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                        .rotationEffect(.degrees(isOpen ? 180 : 0))
                }
            }
            .frame(minHeight: 52)
            .padding(.vertical, Tokens.Space.s12)
            // The whole row is the target, not only its glyphs.
            .contentShape(Rectangle())
            .onTapGesture {
                guard expandable else { return }
                if isOpen { open.remove(row.id) } else { open.insert(row.id) }
            }
            // One sentence for VoiceOver, not five fragments.
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(([row.name, row.holder] + row.pills.map(\.text)).joined(separator: ", "))
            .accessibilityAddTraits(expandable ? .isButton : [])

            if expandable && isOpen { details(row) }
        }
    }

    /// A long hex value with a zero-width break opportunity every eight
    /// characters. Left as one "word", SwiftUI hyphenates it at the line end —
    /// and a hyphen in the middle of a public key is a character that is not in
    /// the key. DISPLAY only: the copy button hands over the value as it is.
    static func breakable(_ value: String) -> String {
        var out = ""
        for (index, character) in value.enumerated() {
            if index > 0, index % 8 == 0 { out.append("\u{200B}") }
            out.append(character)
        }
        return out
    }

    private func tint(for tone: KeyPillTone) -> Color {
        switch tone {
        case .verified: theme.infoBase
        case .synced: theme.successBase
        case .local: theme.fgMuted
        }
    }

    /// What a key row opens onto — the registry explorer's facts, the two a
    /// person pastes elsewhere copyable.
    private func details(_ row: WalletKeyRowModel) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            ForEach(row.details) { detail in
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(detail.label.uppercased())
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                    Text(detail.mono ? Self.breakable(detail.value) : detail.value)
                        .font(detail.mono ? .system(.footnote, design: .monospaced) : .footnote)
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                        .textSelection(.enabled)
                    if detail.copy {
                        let id = "\(row.id):\(detail.label)"
                        Button {
                            UIPasteboard.general.string = detail.value
                            copied = id
                            Task {
                                try? await Task.sleep(nanoseconds: 1_200_000_000)
                                if copied == id { copied = "" }
                            }
                        } label: {
                            Text(copied == id ? model.copiedLabel : model.copyLabel)
                                .typeRole(Typography.flowCaption)
                                .foregroundStyle(theme.fgMuted)
                                .padding(.horizontal, Tokens.Space.s8)
                                .padding(.vertical, Tokens.Space.s2)
                                .overlay(RoundedRectangle(cornerRadius: Tokens.Radius.r8).strokeBorder(theme.borderStrong, lineWidth: 1))
                                // A stroke is only tappable ON the stroke.
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                    }
                }
            }
        }
        .padding(Tokens.Space.s16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgSunken))
        .overlay(RoundedRectangle(cornerRadius: Tokens.Radius.r12).strokeBorder(theme.borderBase, lineWidth: 1))
        .padding(.bottom, Tokens.Space.s12)
    }
}
