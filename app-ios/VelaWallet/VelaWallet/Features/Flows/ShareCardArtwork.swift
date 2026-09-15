//
//  ShareCardArtwork.swift
//  VelaWallet
//
//  R4 — what "Save image" produces (spec 021).
//
//  Not a screen. It is a render product that ends up in someone's photo
//  library and then in a chat, so its colours are mode-invariant and it
//  carries the wordmark: away from the app, the card has to say what it is
//  on its own.
//
//  The identicon sits in the middle of the code (founder direction): a card
//  whose address was doctored would carry artwork that no longer matches
//  the characters printed under it.
//

import SwiftUI

struct ShareCardArtwork: View {
    @Environment(\.theme) private var theme

    let model: ShareCardModel

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            Text(verbatim: model.headline)
                .typeRole(Typography.display)
                .foregroundStyle(theme.onAccent)
                .multilineTextAlignment(.center)
                .padding(.vertical, Tokens.Space.s24)

            VStack(spacing: Tokens.Space.s8) {
                QrCardView(label: model.headline, modules: model.modules) {
                    // Drawn for a PICTURE — this view is rendered to a PNG, and
                    // an image has nothing to tap.
                    IdenticonAvatar(seed: model.identiconSeed,
                                    size: WalletFlowGeometry.qrCentre, tappable: false)
                }
                Text(verbatim: model.name)
                    .typeRole(Typography.rowTitle)
                    .foregroundStyle(WalletGeometry.qrInk)
                    .padding(.top, Tokens.Space.s8)
                ForEach(Array(model.lines.enumerated()), id: \.offset) { _, line in
                    Text(verbatim: line)
                        .monoRole(Typography.monoAddress)
                        .foregroundStyle(WalletGeometry.qrInk)
                        .opacity(Tokens.Opacity.dim)
                }
                HStack(spacing: Tokens.Space.s4) {
                    Circle()
                        .fill(model.networkMark.badgeColor)
                        .frame(width: LucideIconSize.flowStatus, height: LucideIconSize.flowStatus)
                        .overlay {
                            Text(verbatim: model.networkMark.ticker)
                                .typeRole(Typography.tab)
                                .foregroundStyle(theme.onAccent)
                        }
                    Text(verbatim: model.networkNote)
                        .typeRole(Typography.rowSub)
                        .foregroundStyle(WalletGeometry.qrInk)
                }
                .padding(.leading, Tokens.Space.s2)
                .padding(.trailing, Tokens.Space.s12)
                .padding(.vertical, Tokens.Space.s2)
                .overlay(
                    Capsule().stroke(
                        WalletGeometry.qrInk.opacity(Tokens.Opacity.dim),
                        lineWidth: Tokens.BorderWidth.hairline
                    )
                )
                .padding(.top, Tokens.Space.s4)
            }
            .padding(Tokens.Space.s24)
            .background(
                RoundedRectangle(cornerRadius: Tokens.Radius.r20).fill(WalletGeometry.qrCard)
            )

            HStack(spacing: Tokens.Space.s12) {
                VelaMark(size: WalletFlowGeometry.shareCardMark)
                Text(verbatim: model.wordmark)
                    .typeRole(Typography.display)
                    .foregroundStyle(theme.onAccent)
            }
            .padding(.vertical, Tokens.Space.s24)
        }
        .padding(.horizontal, Tokens.Space.s24)
        .frame(width: WalletFlowGeometry.shareCardWidth)
        // Every colour here is fixed rather than themed. The image is saved
        // once and viewed anywhere — a card that rendered in dark mode and was
        // opened on a white chat background would be a different card.
        .background(theme.accentBase)
    }
}
