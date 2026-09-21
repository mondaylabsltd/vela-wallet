//
//  SigningFooter.swift
//  VelaWallet
//
//  The fee row, its expanded fee-token selector, and the signer row — the
//  three things that sit between the last block and the slide, in that order
//  on every scenario.
//

import SwiftUI

struct SigningFeeView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let fee: FeeModel
    /// The row's tap: retry a failed quote, or open / close the coin list.
    var onToggle: () -> Void = {}
    /// A coin from the list, by id (`SigningLive.nativeFeeId` for the chain's own).
    var onPick: (String) -> Void = { _ in }
    /// The speed control under the fee (spec 069), inside the same card.
    var speed: FeeSpeedModel?
    /// `nil` folds or unfolds it; an id picks that speed.
    var onSpeed: (String?) -> Void = { _ in }

    var body: some View {
        switch fee {
        case .hidden:
            EmptyView()
        case .offchain(let note):
            SigningPositive(text: note, quiet: true)
        case .onchain(let label, let value, let selector, let warning):
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                onchainBody(label: label, value: value, selector: selector)
                // Issue #262: the reason the slide below is shut, said where the fix is.
                if let warning {
                    Text(verbatim: warning)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.errorBase)
                        .padding(.horizontal, Tokens.Space.s16)
                }
            }
        }
    }

    @ViewBuilder
    private func onchainBody(
        label: String, value: String, selector: (title: String, options: [FeeTokenOption])?
    ) -> some View {
        if let selector {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Button(action: onToggle) {
                    HStack {
                        Text(verbatim: selector.title)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                        Spacer()
                        LucideIcon(.chevronDown, size: LucideIconSize.disclosure)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .padding(.vertical, Tokens.Space.s8)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                ForEach(selector.options) { option in
                    // A coin that cannot pay is DRAWN and not pickable: hiding
                    // it would be a second filter beside the core's own.
                    Button { onPick(option.id) } label: {
                        HStack(spacing: Tokens.Space.s12) {
                            LetterAvatarView(letter: option.mark.letter, tint: option.mark.tint,
                                             size: Tokens.Space.s32)
                            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                                Text(verbatim: option.name)
                                    .typeRole(Typography.rowTitle.scaled(textScale))
                                    .foregroundStyle(theme.fgBase)
                                Text(verbatim: option.balance)
                                    .typeRole(Typography.rowSub.scaled(textScale))
                                    .foregroundStyle(theme.fgMuted)
                            }
                            Spacer(minLength: Tokens.Space.s8)
                            Text(verbatim: option.fee)
                                .typeRole(Typography.label.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                            if option.selected {
                                LucideIcon(.check, size: LucideIconSize.checkmark)
                                    .foregroundStyle(theme.accentBase)
                            }
                        }
                        .padding(Tokens.Space.s8)
                        .background(option.selected ? theme.bgRaised : Color.clear,
                                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                        .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                    }
                    .buttonStyle(.plain)
                    .disabled(option.disabled)
                    .opacity(option.disabled ? 0.45 : 1)
                }
            }
            .padding(.horizontal, Tokens.Space.s16)
            .padding(.vertical, Tokens.Space.s8)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        } else if let speed {
            VStack(alignment: .leading, spacing: 0) {
                feeRow(label: label, value: value)
                // The control pads its rows by 12; the row above by 16.
                FeeSpeedControlView(speed: speed, onToggle: { onSpeed(nil) },
                                    onPick: { onSpeed($0) })
                    .padding(.horizontal, Tokens.Space.s4)
                    .padding(.bottom, Tokens.Space.s4)
            }
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        } else {
            feeRow(label: label, value: value)
                .background(theme.bgSunken,
                            in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        }
    }

    private func feeRow(label: String, value: String) -> some View {
        Button(action: onToggle) {
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                Spacer()
                Text(verbatim: value)
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    .foregroundStyle(theme.fgMuted)
            }
            .padding(.horizontal, Tokens.Space.s16)
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        }
        .buttonStyle(.plain)
    }
}

struct SigningSignerRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let label: String
    let name: String
    let seed: String

    var body: some View {
        HStack {
            Text(verbatim: label)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            Spacer()
            HStack(spacing: Tokens.Space.s8) {
                IdenticonAvatar(seed: seed, size: ExploreGeometry.signerAvatar)
                Text(verbatim: name)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
        }
    }
}
