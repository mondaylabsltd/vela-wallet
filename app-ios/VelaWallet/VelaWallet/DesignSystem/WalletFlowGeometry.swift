//
//  WalletFlowGeometry.swift
//  VelaWallet
//
//  Wallet-flow geometry the token set does not name (spec 021), measured
//  from design/wallet-2 at the 390×844 design frame (@2x pixels ÷ 2).
//  Licensed by docs/design-system.md ("if a needed token doesn't exist…
//  propose a semantic name") — kept here, never inline in views. Same
//  arrangement as WalletGeometry next door.
//

import SwiftUI

enum WalletFlowGeometry {
    /// The receive QR card, measured 344×344 in R2. Fixed, NOT fluid: the
    /// SPEC sheet pins it at 1.35× text scale too, because a code that
    /// shrinks to make room for its caption stops scanning.
    static let qrCard: CGFloat = 344
    /// The QR card's inner padding — the code's quiet zone.
    static let qrCardPadding: CGFloat = Tokens.Space.s24
    /// The mark drawn over the centre of the code.
    static let qrCentre: CGFloat = 36

    /// The send-receipt status disc, measured 88 in SD4a/SD4c. One size for
    /// all four outcomes so the mark does not resize as the transaction moves
    /// between them.
    static let statusHero: CGFloat = 88
    /// The spinner arc inside it.
    static let statusSpinner: CGFloat = 26
    static let statusSpinnerStroke: CGFloat = 3

    /// The receive network-row chain badge, measured 40 in R1. Larger than
    /// the 32 token icon because this row IS the network, not a token that
    /// happens to be on one.
    static let chainBadge: CGFloat = 40

    /// The inline token mark — inside a line of text (fee row, fact row,
    /// notice banner) rather than leading a row of its own.
    static let inlineMark: CGFloat = 26

    /// The scanner's viewfinder, as a fraction of the screen, and the length
    /// of each corner bracket arm.
    static let scanFrameFraction: CGFloat = 0.68
    static let scanBracketArm: CGFloat = 28
    static let scanBracketStroke: CGFloat = 3
    static let scanToolDisc: CGFloat = Tokens.Control.md

    /// The share card (R4) — a render product saved to the photo library, so
    /// its geometry is fixed rather than responsive.
    static let shareCardWidth: CGFloat = 480
    static let shareCardMark: CGFloat = 60
}
