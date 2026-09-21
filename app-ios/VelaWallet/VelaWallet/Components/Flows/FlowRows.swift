//
//  FlowRows.swift
//  VelaWallet
//
//  The rows of the wallet flows (spec 021 components 9, 12–15, 23).
//

import SwiftUI

/// R1's network row (component 9): the chain, the address on it, and the two
/// things a person does with an address — copy it, or show it.
///
/// Both actions sit on the row rather than behind it. The point of R1 is that
/// ONE address serves every network, so the fastest path is to copy it from
/// whichever line you happened to look at, without opening anything.
struct NetworkRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let row: NetworkRowModel
    var copied = false
    var onCopy: () -> Void = {}
    var onQr: () -> Void = {}

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            RemoteLogoView(urls: row.logoURLs, size: WalletFlowGeometry.chainBadge) {
                Circle()
                    .fill(row.badgeColor)
                    .overlay {
                        Text(verbatim: row.code)
                            .typeRole(Typography.tokenGlyph)
                            // The chain colours are brand fills, dark enough for
                            // white in both appearances — so the mode-invariant
                            // white, not fgInverse.
                            .foregroundStyle(theme.onAccent)
                    }
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: row.name)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                Text(verbatim: row.addressDisplay)
                    .monoRole(Typography.monoAddress.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s8)
            FlowIconButton(
                glyph: copied ? .check : .copy,
                label: row.copyLabel,
                tint: copied ? theme.successBase : theme.fgMuted,
                action: onCopy
            )
            FlowIconButton(glyph: .qrCode, label: row.qrLabel, action: onQr)
        }
        .frame(minHeight: WalletGeometry.rowMinHeight)
    }
}

/// A tap target for a single glyph, at the platform hit size.
struct FlowIconButton: View {
    @Environment(\.theme) private var theme

    let glyph: LucideGlyph
    let label: String
    var tint: Color?
    var action: () -> Void = {}

    var body: some View {
        Button(action: action) {
            LucideIcon(glyph, size: LucideIconSize.flowRowAction)
                .foregroundStyle(tint ?? theme.fgMuted)
                .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(label)
    }
}

/// The label-value row (component 15).
///
/// One component for A2's transaction facts, SD3's confirmation summary, T2's
/// token facts and T3b's chain facts. They differ only in what art the value
/// carries — a chain dot, a token mark, an identicon, or nothing — and in
/// whether the value is copyable, so those are parameters rather than four
/// near-identical rows.
struct FactRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let fact: FactRowModel
    var copied = false
    var onCopy: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            row
            // The reason under its row, in the row's own quiet voice (issue
            // 686): a fact about the value, not a warning about it.
            if let note = fact.note {
                Text(verbatim: note)
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.vertical, Tokens.Space.s12)
    }

    private var row: some View {
        HStack(spacing: Tokens.Space.s8) {
            Text(verbatim: fact.label)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
                .lineLimit(1)
            Spacer(minLength: Tokens.Space.s8)
            lead
            value
            if let copy = fact.copy {
                Button {
                    // It copies. Until 058 every fact row on this client — the
                    // counterparty, the transaction hash, a token's contract —
                    // showed a checkmark and left the clipboard untouched.
                    velaCopy(fact.copyValue ?? fact.value)
                    onCopy()
                } label: {
                    LucideIcon(copied ? .check : .copy, size: LucideIconSize.checkmark)
                        .foregroundStyle(copied ? theme.successBase : theme.fgSubtle)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(copy)
            }
        }
    }

    /// The mono and prose faces are different role TYPES, so the choice is a
    /// branch on the view rather than on the role.
    @ViewBuilder private var value: some View {
        let text = Text(verbatim: fact.value)
        if fact.mono {
            text
                .monoRole(Typography.monoAddressDetail.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
                .truncationMode(.middle)
        } else {
            text
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }

    @ViewBuilder private var lead: some View {
        switch fact.lead {
        case .dot(let color):
            Circle().fill(color).frame(width: WalletGeometry.badge, height: WalletGeometry.badge)
        case .token(let mark):
            InlineTokenMark(mark: mark)
        case .identicon(let seed):
            IdenticonAvatar(seed: seed, size: LucideIconSize.flowRowAction)
        case nil:
            EmptyView()
        }
    }
}

/// The token mark inside a line of text — the fee row's fee token, a fact
/// row's network, a notice banner's chain.
///
/// A component and not a scaled `TokenIconView`: the glyph has to shrink with
/// the circle, and scaling only the box clips a three-letter ticker out of it.
/// It carries no chain dot either — at this diameter the dot is a few pixels
/// of colour on an already-crowded glyph, and the row it sits in has said
/// which chain this is.
struct InlineTokenMark: View {
    @Environment(\.theme) private var theme

    let mark: TokenMarkModel

    var body: some View {
        Circle()
            .fill(theme.bgSunken)
            .frame(width: WalletFlowGeometry.inlineMark, height: WalletFlowGeometry.inlineMark)
            .overlay {
                Text(verbatim: String(mark.ticker.prefix(3)).uppercased())
                    .typeRole(Typography.tab)
                    .foregroundStyle(theme.fgMuted)
                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                    .lineLimit(1)
                    .padding(.horizontal, Tokens.Space.s2)
            }
            .accessibilityHidden(true)
    }
}

/// The small status pill (component 23): A2's 已确认, T3's 已添加, T3b's
/// 兼容 / 不兼容.
///
/// Four tones off the semantic colour pairs, so a chip never invents a colour —
/// and never uses the accent, which in this product means "this moves money",
/// not "this is fine".
struct StatusChipView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let chip: StatusChipModel

    var body: some View {
        Text(verbatim: chip.text)
            .typeRole(Typography.chip.scaled(textScale))
            .foregroundStyle(foreground)
            .padding(.horizontal, Tokens.Space.s8)
            .padding(.vertical, Tokens.Space.s2)
            .background(Capsule().fill(background))
    }

    private var background: Color {
        switch chip.tone {
        case .success: theme.successSoft
        case .warning: theme.warningSoft
        case .error: theme.errorSoft
        case .info: theme.infoSoft
        }
    }

    private var foreground: Color {
        switch chip.tone {
        case .success: theme.successBase
        case .warning: theme.warningBase
        case .error: theme.errorBase
        case .info: theme.infoBase
        }
    }
}

/// SD2b's split row (component 13): one of N people, what they get, and the
/// way to drop them.
///
/// The ordinal ("Recipient 2") is a label above the name rather than a number
/// beside it, because in a split the ROW is the person and the number is only
/// there to keep three otherwise-similar cards apart.
struct RecipientCardView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let recipient: RecipientCardModel
    var onRemove: () -> Void = {}
    /// The live fields. `nil` renders exactly as drawn — the gallery and the
    /// screenshot sweep stay pixel-identical (the mode-not-a-type shape
    /// `AmountInputView` and `RecipientFieldView` already use).
    var address: Binding<String>?
    var amount: Binding<String>?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: recipient.identiconSeed, size: WalletGeometry.rowIcon)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: recipient.ordinal)
                        .typeRole(Typography.caption.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    if let address {
                        TextField(recipient.name, text: address)
                            .font(Typography.monoAddressDetail.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .lineLimit(1)
                    } else {
                        Text(verbatim: recipient.name)
                            .monoRole(Typography.monoAddressDetail.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .lineLimit(1)
                            .truncationMode(.middle)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                if let amount {
                    TextField("0", text: amount)
                        .font(Typography.rowValue.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                        .multilineTextAlignment(.trailing)
                        .keyboardType(.decimalPad)
                        .frame(maxWidth: WalletGeometry.splitAmountWidth)
                } else {
                    Text(verbatim: recipient.amount)
                        .typeRole(Typography.rowValue.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                }
                Button(action: onRemove) {
                    LucideIcon(.close, size: LucideIconSize.flowRowAction)
                        .foregroundStyle(theme.fgSubtle)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(recipient.removeLabel)
            }
            // The core's verdict on THIS row. A list of six with one sentence
            // underneath makes somebody count rows to find the bad one.
            if let problem = recipient.problem {
                Text(verbatim: problem)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
            }
        }
        .padding(Tokens.Space.s12)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
    }
}

/// SD2f's fee-token row (component 14): a coin that could pay this transfer's
/// fee, what you hold of it, and what the fee would come to.
///
/// The estimate is per row and not per screen because that is the whole
/// decision: the same transfer costs a different number in each coin, and one
/// figure with a token switcher would hide the comparison.
struct FeeTokenRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let row: FeeTokenRowModel
    let estimateLabel: String
    var onSelect: () -> Void = {}

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: Tokens.Space.s12) {
                TokenIconView(mark: row.mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: row.symbol)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Text(verbatim: row.insufficient
                        ? (row.insufficientNote ?? row.balanceLabel)
                        : row.balanceLabel)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
                Spacer(minLength: Tokens.Space.s8)
                VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
                    Text(verbatim: row.fee)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Text(verbatim: estimateLabel)
                        .typeRole(Typography.caption.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                }
                // Always laid out, so choosing a row does not shift the ones
                // under it.
                LucideIcon(.check, size: LucideIconSize.checkmark)
                    .foregroundStyle(theme.accentBase)
                    .opacity(row.selected ? 1 : 0)
            }
            .padding(Tokens.Space.s12)
            .background(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .fill(row.selected ? theme.bgRaised : .clear)
            )
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        // Drawn for context, answering to nothing — and `.disabled` rather
        // than only a dimmer colour, so VoiceOver does not call it a button
        // that works.
        .disabled(row.insufficient)
        .opacity(row.insufficient ? Tokens.Opacity.disabled : 1)
    }
}

/// SD2e's contact row (component 12).
///
/// Close cousin of spec 018's `ContactRow`, and deliberately not it: that row
/// MANAGES a contact (swipe to reveal edit and delete, a favourite star, a
/// send count). This one PICKS one, so it carries a chevron and nothing else —
/// every affordance it does not have is one that cannot fire by accident while
/// someone is halfway through a transfer.
struct ContactPickRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let contact: ContactEntryModel
    var onSelect: () -> Void = {}

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: contact.identiconSeed, size: WalletGeometry.rowIcon)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    HStack(spacing: Tokens.Space.s4) {
                        Text(verbatim: contact.name)
                            .typeRole(Typography.rowTitle.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .lineLimit(1)
                        if let group = contact.group {
                            Text(verbatim: group)
                                .typeRole(Typography.chip.scaled(textScale))
                                .foregroundStyle(theme.fgMuted)
                                .padding(.horizontal, Tokens.Space.s4)
                                .background(
                                    RoundedRectangle(cornerRadius: Tokens.Radius.r4)
                                        .fill(theme.bgRaised)
                                )
                        }
                    }
                    Text(verbatim: contact.addressDisplay)
                        .monoRole(Typography.monoAddress.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .lineLimit(1)
                }
                Spacer(minLength: Tokens.Space.s8)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    .foregroundStyle(theme.fgSubtle)
            }
            .frame(minHeight: WalletGeometry.rowMinHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
