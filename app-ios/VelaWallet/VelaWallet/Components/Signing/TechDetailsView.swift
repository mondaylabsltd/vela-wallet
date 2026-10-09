//
//  TechDetailsView.swift
//  VelaWallet
//
//  The universal fallback renderer (SPEC 签名 · 技术细节): function →
//  parameters → address identities → simulation → raw data. Five fixed
//  layers that can render ANY request, folded away by default and never
//  removed — whatever the descriptor could not explain is still in here.
//

import SwiftUI

struct TechDetailsView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let tech: TechModel
    @Binding var open: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Button {
                open.toggle()
            } label: {
                HStack(spacing: Tokens.Space.s8) {
                    LucideIcon(open ? .chevronDown : .chevronRight,
                               size: LucideIconSize.disclosure)
                    Text(verbatim: tech.summary.map { "\(tech.title) · \($0)" } ?? tech.title)
                        .typeRole(Typography.rowSub.scaled(textScale))
                    Spacer()
                }
                .foregroundStyle(theme.fgMuted)
                .padding(.vertical, Tokens.Space.s12)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(.isButton)

            if open {
                VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                    if let fn = tech.fn {
                        layerLabel(fn.label)
                        Text(verbatim: fn.signature)
                            .typeRole(Typography.monoSmall.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                    }
                    if !tech.params.isEmpty {
                        SigningRowsView(rows: tech.params)
                    }
                    ForEach(tech.identities) { identity in
                        HStack(spacing: Tokens.Space.s8) {
                            switch identity.lead {
                            case .token(let mark)?:
                                // On the grey card: ringed, or the drawn disc
                                // is the card's own colour and its letters
                                // float.
                                InlineTokenMark(mark: mark, ring: true)
                            case .identicon(let seed)?:
                                IdenticonAvatar(seed: seed, size: WalletFlowGeometry.inlineMark,
                                                tappable: false)
                            case .dot?, nil:
                                EmptyView()
                            }
                            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                                Text(verbatim: "\(identity.role) · \(identity.name)")
                                    .typeRole(Typography.rowSub.scaled(textScale))
                                    .foregroundStyle(theme.fgSubtle)
                                Text(verbatim: identity.address)
                                    .typeRole(Typography.monoSmall.scaled(textScale))
                                    .foregroundStyle(theme.fgBase)
                                    .lineLimit(2)
                            }
                            Spacer(minLength: Tokens.Space.s8)
                            LucideIcon(.copy, size: LucideIconSize.rowGlyph)
                                .foregroundStyle(theme.fgMuted)
                                .accessibilityLabel(tech.copyLabel)
                            LucideIcon(.externalLink, size: LucideIconSize.rowGlyph)
                                .foregroundStyle(theme.fgMuted)
                                .accessibilityLabel(tech.explorerLabel)
                        }
                        .padding(.vertical, Tokens.Space.s4)
                    }
                    if let sim = tech.simResult {
                        SigningRowsView(rows: [sim])
                    }
                    if let raw = tech.raw {
                        layerLabel(raw.label)
                        Text(verbatim: raw.hex)
                            .typeRole(Typography.monoSmall.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                    }
                }
                .padding(.horizontal, Tokens.Space.s16)
                .padding(.bottom, Tokens.Space.s12)
            }
        }
        .background(open ? theme.bgSunken : Color.clear,
                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r16))
    }

    private func layerLabel(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub.scaled(textScale))
            .foregroundStyle(theme.fgSubtle)
    }
}
