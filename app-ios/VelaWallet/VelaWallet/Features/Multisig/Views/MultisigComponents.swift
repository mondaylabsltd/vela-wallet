//
//  MultisigComponents.swift
//  VelaWallet
//
//  The multisig screens' building blocks, in the app's design language
//  (docs/DESIGN-LANGUAGE.md: quiet, typographic, de-containered): a sheet
//  with detents and its ✕, a section, a label over its value, a note in a
//  tone, an address that wraps and is never cut. Tokens only — no literal
//  colour, size or font — and every block scales with Dynamic Type and the
//  app's own text size, and reads as one element to VoiceOver.
//

import SwiftUI
import UIKit

/// A multisig sheet: a title, its ✕, a scrolling body; medium and large
/// detents (`large` only for a sheet that holds a long review).
struct MsSheet<Content: View>: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let title: String
    var closeLabel: String
    var tall = false
    var onClose: () -> Void
    @ViewBuilder var content: () -> Content

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                    content()
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.bottom, Tokens.Space.s32)
            }
            .scrollDismissesKeyboard(.interactively)
            .background(theme.bgRaised)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .topBarTrailing) {
                    Button(action: onClose) {
                        LucideIcon(.close, size: LucideIconSize.action)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .accessibilityLabel(closeLabel)
                    .accessibilityIdentifier("multisig.sheet.close")
                }
            }
        }
        .presentationDetents(tall ? [.large] : [.medium, .large])
        .presentationDragIndicator(.visible)
        .presentationBackground(theme.bgRaised)
    }
}

/// A heading over a group of rows.
struct MsSection<Content: View>: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let title: String?
    @ViewBuilder var content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            if let title {
                Text(verbatim: title)
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .accessibilityAddTraits(.isHeader)
            }
            content()
        }
        .padding(.top, Tokens.Space.s8)
    }
}

/// A label over its value; an address or hash in the mono face, wrapped,
/// never cut — a hash with its middle missing is not one anybody can compare.
struct MsRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let label: String
    let value: String
    var mono = false
    var copyable = false

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s8) {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(verbatim: label)
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                Text(verbatim: value)
                    .typeRole((mono ? Typography.monoSmall : Typography.body).scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .textSelection(.enabled)
            }
            Spacer(minLength: 0)
            if copyable {
                Button {
                    UIPasteboard.general.string = value
                    VelaHaptic.select.play()
                } label: {
                    LucideIcon(.copy, size: LucideIconSize.action)
                        .foregroundStyle(theme.fgMuted)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(label)
            }
        }
        .padding(.vertical, Tokens.Space.s8)
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
    }
}

/// One sentence in a tone: what is so, said where it applies.
struct MsNote: View {
    enum Tone { case neutral, warning, danger, positive }
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let text: String
    var tone: Tone = .neutral
    var identifier: String?

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s8) {
            if tone == .warning || tone == .danger {
                LucideIcon(.triangleAlert, size: LucideIconSize.statusIcon)
                    .foregroundStyle(color)
                    .padding(.top, Tokens.Space.s2)
                    .accessibilityHidden(true)
            } else if tone == .positive {
                LucideIcon(.check, size: LucideIconSize.statusIcon)
                    .foregroundStyle(color)
                    .padding(.top, Tokens.Space.s2)
                    .accessibilityHidden(true)
            }
            Text(verbatim: text)
                .typeRole(Typography.flowCaption.scaled(textScale))
                .foregroundStyle(tone == .neutral ? theme.fgMuted : color)
                .fixedSize(horizontal: false, vertical: true)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .accessibilityElement(children: .combine)
        .accessibilityIdentifier(identifier ?? "multisig.note")
    }

    private var color: Color {
        switch tone {
        case .neutral: theme.fgMuted
        case .warning: theme.warningBase
        case .danger: theme.errorBase
        case .positive: theme.successBase
        }
    }
}

/// A short tag: the rule, "Not shared yet", "High risk".
struct MsChip: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let text: String
    var tone: MsNote.Tone = .neutral

    var body: some View {
        Text(verbatim: text)
            .typeRole(Typography.chip.scaled(textScale))
            .foregroundStyle(tone == .neutral ? theme.fgMuted : (tone == .danger ? theme.errorBase : theme.warningBase))
            .padding(.horizontal, Tokens.Space.s8)
            .padding(.vertical, Tokens.Space.s4)
            // The danger tag on the error tokens (soft fill, base ink) — a
            // danger chip on the warning's fill read as a caution.
            .background(Capsule().fill(tone == .neutral ? theme.bgSunken : (tone == .danger ? theme.errorSoft : theme.warningSoft)))
    }
}

/// A divider between rows.
struct MsDivider: View {
    @Environment(\.theme) private var theme
    var body: some View {
        Rectangle().fill(theme.borderBase).frame(height: 1).accessibilityHidden(true)
    }
}

/// A plain row that can be tapped: a title, an optional line under it, a chevron.
struct MsNavRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let title: String
    var subtitle: String?
    var icon: LucideGlyph?
    var identifier: String?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            HStack(spacing: Tokens.Space.s12) {
                if let icon {
                    LucideIcon(icon, size: LucideIconSize.action)
                        .foregroundStyle(theme.fgBase)
                        .accessibilityHidden(true)
                }
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: title)
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                    if let subtitle {
                        Text(verbatim: subtitle)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                            .fixedSize(horizontal: false, vertical: true)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    .foregroundStyle(theme.fgSubtle)
                    .accessibilityHidden(true)
            }
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(.isButton)
        .accessibilityIdentifier(identifier ?? title)
    }
}

/// A text field in the app's style.
struct MsField: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let label: String
    let placeholder: String
    @Binding var text: String
    var mono = false
    var error: String?
    var identifier: String

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: label)
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            TextField(placeholder, text: $text)
                .font((mono ? Typography.monoSmall : Typography.body).scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                .textInputAutocapitalization(.never)
                .autocorrectionDisabled()
                .padding(.vertical, Tokens.Space.s12)
                .padding(.horizontal, Tokens.Space.s12)
                .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgSunken))
                .overlay(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                        .stroke(error == nil ? theme.borderBase : theme.errorBase, lineWidth: 1)
                )
                .accessibilityIdentifier(identifier)
            if let error {
                Text(verbatim: error)
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("\(identifier).error")
            }
        }
    }
}
