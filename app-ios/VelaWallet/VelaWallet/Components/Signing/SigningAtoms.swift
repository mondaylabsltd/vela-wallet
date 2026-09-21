//
//  SigningAtoms.swift
//  VelaWallet
//
//  The signing sheet's small parts (spec 022 §3): the dApp header, the intent
//  eyebrow, the hero amount and its swap pair, the plain-language sentence,
//  the counterparty row, key/value rows, warnings, reassurances, code blocks,
//  detail cards and the balance-change preview.
//
//  They are grouped in one file because none of them is meaningful alone —
//  they exist to be assembled in the order a scenario dictates.
//

import SwiftUI

// MARK: - Tone

extension SigningTone {
    func color(_ theme: Theme) -> Color {
        switch self {
        case .neutral: theme.fgBase
        case .accent: theme.accentBase
        case .success: theme.successBase
        case .caution: theme.warningBase
        case .danger: theme.errorBase
        }
    }
}

// MARK: - Header

struct SigningHeaderView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let dapp: (name: String, host: String, letter: String, tint: Color)
    let network: (name: String, dot: Color)
    var own = false
    var iconUrls: [String] = []
    var networkLogoUrl: String?

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            if own {
                // The wallet asking itself: its own mark, never a letter on a disc.
                VelaMark(size: ExploreGeometry.signingAvatar * 0.6)
                    .frame(width: ExploreGeometry.signingAvatar, height: ExploreGeometry.signingAvatar)
                    .background(theme.bgSunken, in: Circle())
            } else {
                // The site's own icon; its initial until one lands, and when it has none.
                RemoteLogoView(urls: iconUrls, size: ExploreGeometry.signingAvatar) {
                    LetterAvatarView(letter: dapp.letter, tint: dapp.tint,
                                     size: ExploreGeometry.signingAvatar)
                }
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: dapp.name)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                if !dapp.host.isEmpty {
                    Text(verbatim: dapp.host)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                }
            }
            Spacer(minLength: Tokens.Space.s8)
            HStack(spacing: Tokens.Space.s8) {
                // The chain's logo; the drawn dot until it lands, and when there is none.
                RemoteLogoView(urls: [networkLogoUrl].compactMap { $0 }, size: Tokens.Space.s16) {
                    Circle().fill(network.dot).frame(width: Tokens.Space.s8, height: Tokens.Space.s8)
                        .frame(width: Tokens.Space.s16, height: Tokens.Space.s16)
                }
                Text(verbatim: network.name)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
            .padding(.horizontal, Tokens.Space.s12)
            .frame(height: ExploreGeometry.networkChip)
            .background(theme.bgSunken, in: Capsule())
        }
    }
}

// MARK: - Intent + sentence

struct SigningIntentLabel: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let text: String
    let tone: SigningTone

    var body: some View {
        Text(verbatim: text)
            .typeRole(Typography.label.scaled(textScale))
            .tracking(Tokens.LetterSpacing.sectionLabel)
            .foregroundStyle(tone == .neutral ? theme.fgMuted : tone.color(theme))
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct SigningSentence: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let text: String
    let tone: SigningTone

    var body: some View {
        // The plain-language line is the one thing a hurried person reads, so
        // it carries the tone of the transaction rather than staying neutral.
        Text(verbatim: text)
            .typeRole(Typography.body.scaled(textScale))
            .foregroundStyle(tone.color(theme))
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Hero

struct SigningAmountView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let line: AmountLine
    var card = false
    var note: String?
    var compact = false

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            if let caption = line.caption, !card {
                Text(verbatim: caption)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: "\(line.sign)\(line.value)")
                    .typeRole(heroRole.scaled(textScale))
                    .foregroundStyle(line.tone == .neutral ? theme.fgBase : line.tone.color(theme))
                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                    .lineLimit(1)
                if let token = line.token {
                    LetterAvatarView(letter: token.letter, tint: token.tint,
                                     size: ExploreGeometry.tokenMark)
                }
                Text(verbatim: line.symbol)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(line.tone == .neutral ? theme.fgMuted : line.tone.color(theme))
            }
            if let note {
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            } else if let fiat = line.fiat {
                Text(verbatim: fiat)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .modifier(CardTone(active: card, tone: line.tone))
    }

    private var heroRole: TypeRole {
        card ? Typography.title : (compact ? Typography.amountHeroDecimals : Typography.amountHero)
    }
}

/// Boxes a hero in its own tone — cs28's burn intercept.
private struct CardTone: ViewModifier {
    @Environment(\.theme) private var theme
    let active: Bool
    let tone: SigningTone

    func body(content: Content) -> some View {
        if active {
            content
                .padding(Tokens.Space.s16)
                .background(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                        .fill(tone == .danger ? theme.errorSoft : theme.bgSunken)
                )
                .overlay(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                        .stroke(tone == .danger ? theme.errorBase : .clear,
                                lineWidth: Tokens.BorderWidth.hairline)
                )
        } else {
            content
        }
    }
}

struct SigningSwapPair: View {
    @Environment(\.theme) private var theme

    let pay: AmountLine
    let receive: AmountLine

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            SigningAmountView(line: pay, compact: true)
            LucideIcon(.arrowDown, size: LucideIconSize.rowGlyph)
                .foregroundStyle(theme.fgMuted)
                .frame(width: Tokens.Space.s32, height: Tokens.Space.s32)
                .background(theme.bgSunken, in: Circle())
            SigningAmountView(line: receive, compact: true)
        }
    }
}

struct SigningNftHero: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let id: String
    let collection: String

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: id)
                .typeRole(Typography.amountHero.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            Text(verbatim: collection)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

// MARK: - Rows, parties, warnings

struct SigningRowsView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let rows: [SigningRow]

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            ForEach(rows) { row in
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s16) {
                    Text(verbatim: row.label)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                    Spacer(minLength: Tokens.Space.s8)
                    Text(verbatim: row.value)
                        .typeRole((row.mono ? Typography.monoSmall : Typography.label)
                            .scaled(textScale))
                        .foregroundStyle(row.valueTone.color(theme))
                        .multilineTextAlignment(.trailing)
                }
                .padding(.vertical, Tokens.Space.s12)
            }
        }
    }
}

struct SigningPartyRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let label: String
    let name: String
    var address: String?
    var badge: PartyBadge?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: label)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            HStack(alignment: .top, spacing: Tokens.Space.s12) {
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    if let address {
                        Text(verbatim: address)
                            .typeRole(Typography.monoSmall.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                if let badge {
                    Text(verbatim: badge.text)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(badgeInk(badge.tone))
                        .padding(.horizontal, Tokens.Space.s8)
                        .padding(.vertical, Tokens.Space.s2)
                        .background(badgeFill(badge.tone),
                                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r4))
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func badgeInk(_ tone: SigningTone) -> Color {
        tone == .neutral ? theme.fgMuted : tone.color(theme)
    }

    private func badgeFill(_ tone: SigningTone) -> Color {
        switch tone {
        case .success: theme.successSoft
        case .caution: theme.warningSoft
        case .danger: theme.errorSoft
        default: theme.bgSunken
        }
    }
}

struct SigningWarning: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let tone: SigningTone
    let text: String

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(.triangleAlert, size: LucideIconSize.statusIcon)
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: Tokens.Space.s0)
        }
        .foregroundStyle(tone.color(theme))
        .padding(.horizontal, Tokens.Space.s16)
        .padding(.vertical, Tokens.Space.s12)
        .background(tone == .danger ? theme.errorSoft : theme.warningSoft,
                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .stroke(tone == .danger ? theme.errorBase : theme.warningBase.opacity(Tokens.Opacity.dim),
                        lineWidth: Tokens.BorderWidth.hairline)
        )
        .accessibilityAddTraits(tone == .danger ? .isStaticText : [])
    }
}

struct SigningPositive: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let text: String
    var quiet = false

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            LucideIcon(.check, size: LucideIconSize.checkmark)
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
            Spacer(minLength: Tokens.Space.s0)
        }
        .foregroundStyle(quiet ? theme.successBase : theme.fgBase)
        .padding(.horizontal, quiet ? Tokens.Space.s0 : Tokens.Space.s16)
        .padding(.vertical, quiet ? Tokens.Space.s0 : Tokens.Space.s12)
        .background(
            quiet ? AnyShapeStyle(Color.clear) : AnyShapeStyle(theme.bgSunken),
            in: RoundedRectangle(cornerRadius: Tokens.Radius.r12)
        )
    }
}

struct SigningCodeBlock: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let lines: [String]
    var note: String?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
            ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                Text(verbatim: line)
                    .typeRole(Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
            if let note {
                Text(verbatim: note)
                    .typeRole(Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Space.s16)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
    }
}

struct SigningDetailCard: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String?
    let rows: [SigningRow]
    let tone: SigningTone

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            if let title {
                Text(verbatim: title)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .padding(.vertical, Tokens.Space.s8)
            }
            SigningRowsView(rows: rows)
        }
        .padding(.horizontal, Tokens.Space.s16)
        .padding(.vertical, Tokens.Space.s4)
        .background(tone == .danger ? theme.errorSoft : Color.clear,
                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r16))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(tone == .danger ? theme.errorBase : theme.borderBase,
                        lineWidth: Tokens.BorderWidth.hairline)
        )
    }
}

/// The simulation's own account of what moves. It is the ONE part of a
/// signing sheet a malicious site cannot author, which is why the deeper
/// degradation rungs promote it from footnote to protagonist.
struct SigningBalances: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String
    let rows: [BalanceDeltaRow]
    var note: String?
    var noteTone: SigningTone = .neutral

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            Text(verbatim: title)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .padding(.vertical, Tokens.Space.s8)
            ForEach(rows) { row in
                HStack {
                    Text(verbatim: row.symbol)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Spacer()
                    Text(verbatim: row.delta)
                        .typeRole(Typography.rowValue.scaled(textScale))
                        .foregroundStyle(row.tone == .neutral ? theme.fgBase
                                                              : row.tone.color(theme))
                }
                .padding(.vertical, Tokens.Space.s4)
            }
            if let note {
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(noteTone == .neutral ? theme.fgSubtle
                                                          : noteTone.color(theme))
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, Tokens.Space.s8)
            }
        }
        .padding(.horizontal, Tokens.Space.s16)
        .padding(.vertical, Tokens.Space.s4)
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
        )
    }
}


/// "Sign with · Automatic ›" — where the passkey that signs this request is.
///
/// Creating a wallet and signing in both let a person say whether their key is
/// on this phone, on another device, or on a security key. Signing did not: it
/// took the first key's stored route (founder, 2026-09-19). Per request; the
/// core decides which key the ceremony is pinned to (`signRoute`). Opens in
/// place — a sheet over the signing sheet is a modal under a modal.
struct SignWithRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let model: SignWithModel
    /// `nil` toggles the list; an id picks a method and closes it.
    var onSelect: (String?) -> Void = { _ in }

    var body: some View {
        VStack(spacing: Tokens.Space.s8) {
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: model.label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                Spacer(minLength: Tokens.Space.s8)
                Text(verbatim: model.value)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                LucideIcon(.chevronDown, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgMuted)
                    .rotationEffect(.degrees(model.open ? 180 : 0))
            }
            // The whole row is the target, not only its glyphs.
            .contentShape(Rectangle())
            .onTapGesture { onSelect(nil) }
            .accessibilityElement(children: .combine)
            .accessibilityAddTraits(.isButton)

            if model.open {
                VStack(spacing: 0) {
                    ForEach(model.options) { option in
                        HStack {
                            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                                Text(verbatim: option.title)
                                    .typeRole(Typography.rowSub.scaled(textScale))
                                    .foregroundStyle(option.selected ? theme.fgBase : theme.fgMuted)
                                // What the option does, under its name — never
                                // squeezed beside it (the speed sheet's rule).
                                if let detail = option.detail {
                                    Text(verbatim: detail)
                                        .typeRole(Typography.flowCaption.scaled(textScale))
                                        .foregroundStyle(theme.fgSubtle)
                                        .fixedSize(horizontal: false, vertical: true)
                                }
                            }
                            Spacer()
                            if option.selected {
                                LucideIcon(.check, size: LucideIconSize.rowGlyph)
                                    .foregroundStyle(theme.accentBase)
                            }
                        }
                        .padding(.horizontal, Tokens.Space.s16)
                        .padding(.vertical, Tokens.Space.s12)
                        .contentShape(Rectangle())
                        .onTapGesture { onSelect(option.id) }
                        .accessibilityElement(children: .combine)
                        .accessibilityAddTraits(option.selected ? [.isButton, .isSelected] : .isButton)
                    }
                }
                .padding(Tokens.Space.s4)
                .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            }
        }
    }
}
