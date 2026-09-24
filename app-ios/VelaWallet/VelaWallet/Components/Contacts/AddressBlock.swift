//
//  AddressBlock.swift
//  VelaWallet
//
//  AddressBlock (spec 018 vocabulary #12, mock C2): 地址 label over the FULL
//  address in mono — pre-split by the fixture layer into exactly the mock's
//  two lines — with a trailing copy button kept vertically centered against
//  the block. Copy semantics follow the spec-014 AddressStrip pattern: the
//  untruncated value goes to the pasteboard and a transient confirmation
//  replaces the glyph.
//

import SwiftUI

struct AddressBlock: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    /// 地址.
    let label: String
    /// Display lines (mobile: two; desktop parity: one).
    let lines: [String]
    let copyLabel: String
    let copiedLabel: String
    var onCopy: () -> Void = {}

    @State private var copied = false

    /// The value that reaches the pasteboard — never truncated.
    private var fullAddress: String { lines.joined() }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: label)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            HStack(alignment: .center, spacing: Tokens.Space.s12) {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    ForEach(lines, id: \.self) { line in
                        Text(verbatim: line)
                            .monoRole(Typography.monoAddressBlock.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .lineLimit(1)
                            .minimumScaleFactor(WalletGeometry.heroMinScale)
                    }
                }
                Spacer(minLength: Tokens.Space.s12)
                Button(action: copy) {
                    Group {
                        if copied {
                            LucideIcon(.check, size: LucideIconSize.addressCopy)
                                .foregroundStyle(theme.successBase)
                        } else {
                            LucideIcon(.copy, size: LucideIconSize.addressCopy)
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(copied ? copiedLabel : copyLabel)
            }
            .frame(minHeight: ContactsGeometry.addressBlockMinHeight)
        }
        .animation(.easeOut(duration: ContactsMotion.crossfade), value: copied)
        .accessibilityElement(children: .contain)
    }

    private func copy() {
        velaCopy(fullAddress)
        onCopy()
        copied = true
        Task {
            try? await Task.sleep(for: .seconds(Interaction.copiedFeedbackSeconds))
            copied = false
        }
    }
}

#Preview("Address block dark") {
    AddressBlock(
        label: "地址",
        lines: ContactsFixtures.aliceAddressLines,
        copyLabel: "复制地址",
        copiedLabel: "已复制"
    )
    .padding(Tokens.Layout.screenPaddingX)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
    .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("Address block light · one line") {
    AddressBlock(
        label: "Address",
        lines: [ContactsFixtures.roster[0].addressFull],
        copyLabel: "Copy address",
        copiedLabel: "Copied"
    )
    .padding(Tokens.Layout.screenPaddingX)
    .themed(.light)
    .environment(\.lucideIconProvider, .previewSafe)
}
