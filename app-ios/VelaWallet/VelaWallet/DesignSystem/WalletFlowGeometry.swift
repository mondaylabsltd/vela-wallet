//
//  WalletFlowGeometry.swift
//  VelaWallet
//
//  Wallet-flow geometry the token set does not name (spec 021), measured
//  from docs/design/wallet-2 at the 390×844 design frame (@2x pixels ÷ 2).
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
    /// The waiting ring OUTSIDE the disc (issue 199, the web's 2.5 in 104).
    static let statusRingStroke: CGFloat = 2.5

    /// The receive network-row chain badge, measured 40 in R1. Larger than
    /// the 32 token icon because this row IS the network, not a token that
    /// happens to be on one.
    static let chainBadge: CGFloat = 40

    /// The inline token mark — inside a line of text (fee row, fact row,
    /// notice banner) rather than leading a row of its own.
    static let inlineMark: CGFloat = 26

    /// The send form's figure, on the hero ladder the web (`AmountInput`) and
    /// the desktop (`theme::amount_hero_rung`) draw: 46 / 38 / 31 — the same
    /// three rungs the Welcome headline steps down, so the 38 and the 31 ARE
    /// its two. Chosen by DRAWN length (`AmountRung`), so an 18-decimal figure
    /// steps down instead of running off the column.
    static let amountHero: CGFloat = 46
    static let amountHeroCompact: CGFloat = WelcomeGeometry.heroSize
    static let amountHeroTight: CGFloat = WelcomeGeometry.heroSizeLong
    /// The last drawn length each rung takes (figure + prefix + half the
    /// quieter suffix): up to 8 on the hero rung, up to 11 on the compact one.
    static let amountHeroMaxDrawn: Double = 8
    static let amountCompactMaxDrawn: Double = 11
    /// The line the figure sits on — the HERO rung's on every rung, so the
    /// block is one height whatever is typed and the recipient, the fee row
    /// and Continue never jump under a finger reaching for them.
    static let amountHeroLine: CGFloat = amountHero * Tokens.Leading.tight
    /// Room after the last digit for the caret, so it is never the thing that
    /// overflows; taken off the unit's gap so the figure and its unit sit the
    /// same distance apart typed or drawn.
    static let amountCaretSlack: CGFloat = Tokens.Space.s4

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
