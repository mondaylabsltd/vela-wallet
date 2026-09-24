//
//  AddressStrip.swift
//  VelaWallet
//
//  The single authoritative copyable address strip (spec 014 / FR-003):
//  sunken rounded row, mono tail-truncated display of the FULL address,
//  trailing copy affordance. Copying puts the untruncated value on the
//  pasteboard, shows transient confirmation, and emits `copy_address`.
//

import SwiftUI

struct AddressStrip: View {
    @Environment(\.theme) private var theme
    /// Full 42-char address; display truncates, copy never does.
    let address: String
    /// Resolved a11y/action label (复制地址).
    let copyLabel: String
    /// Resolved transient confirmation (已复制).
    let copiedLabel: String
    var onCopy: () -> Void = {}

    @State private var copied = false

    var body: some View {
        Button(action: copy) {
            HStack(spacing: Tokens.Space.s12) {
                Text(address)
                    .typeRole(Typography.mono)
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                    .truncationMode(.tail)
                if copied {
                    Text(copiedLabel)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.successBase)
                        .fixedSize()
                        .transition(.opacity)
                }
                Image(systemName: copied ? "checkmark" : "square.on.square")
                    .font(GlyphFont.control)
                    .foregroundStyle(copied ? theme.successBase : theme.fgSubtle)
            }
            .padding(.horizontal, Tokens.Space.s16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(minHeight: Tokens.Control.lg)
            .background {
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .fill(theme.bgSunken)
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .animation(.easeOut(duration: Tokens.Motion.fast), value: copied)
        .accessibilityLabel(copyLabel)
        .accessibilityValue(Text(verbatim: address))
    }

    private func copy() {
        velaCopy(address)
        onCopy()
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(Interaction.copiedFeedbackSeconds))
            copied = false
        }
    }
}

#Preview("Address strip") {
    AddressStrip(
        address: FlowFixtures.fixtureAddress,
        copyLabel: "复制地址",
        copiedLabel: "已复制"
    )
    .padding(Tokens.Space.s24)
    .themed(.light)
}

#Preview("Address strip dark") {
    AddressStrip(
        address: FlowFixtures.fixtureAddress,
        copyLabel: "复制地址",
        copiedLabel: "已复制"
    )
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgRaised.color)
    .themed(.dark)
}
