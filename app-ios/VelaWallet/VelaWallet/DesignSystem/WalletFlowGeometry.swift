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
    /// The fee card's refresh (round 3): a fixed round button inside the card
    /// at its trailing edge — 40pt drawn, a 44pt target — that never grows
    /// with the card at large text sizes.
    static let feeRefreshButton: CGFloat = 40
    static let feeRefreshTarget: CGFloat = 44
    /// The confirm page's unit beside its 32pt figure (round 2): the Send
    /// form's hero proportion, figure 46 : unit 26, carried to 32 → 18.
    static let confirmUnitSize: CGFloat = (Tokens.TextSize.t32 * Tokens.TextSize.t26 / amountHero).rounded()
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

/// The feedback sheet's screenshots and its success state (2026-09-26, v2
/// after the design review), from the shared spec every shell draws
/// (`ui-spec-feedback.md` §2 + v2 amendments).
enum FeedbackGeometry {
    /// The empty section's one add target: full width, this tall.
    static let addTargetHeight: CGFloat = 56
    /// Tiles: five equal square columns in ONE row — images and the add tile
    /// never exceed five — each at most this big, this far apart.
    static let tileMax: CGFloat = 72
    static let tileGap: CGFloat = Tokens.Space.s12
    static let columns = 5
    /// The remove badge: a fixed look in both themes — a near-black disc with
    /// a white ✕ and a ring in the page colour, so it separates from any
    /// screenshot — overlapping the tile's top-trailing corner, with a target
    /// of at least 44.
    static let badge: CGFloat = 22
    static let badgeGlyph: CGFloat = 12
    static let badgeRing: CGFloat = 2
    static let badgeOverlap: CGFloat = 4
    static let badgeTarget: CGFloat = 44
    /// How far the remove area reaches past the tile's corner, outward — and,
    /// equally, into the picture: half the target, the badge's own size.
    static let removeReach: CGFloat = badgeTarget / 2
    /// VoiceOver focus moves this long after the outcome, once it is laid out.
    static let focusDelay: TimeInterval = 0.35
    /// The dashed outline of an add target.
    static let dash: [CGFloat] = [4, 3]
    /// The success state's disc, its check, and the ring that shows it on a
    /// light page.
    static let successDisc: CGFloat = 56
    static let successCheck: CGFloat = 28
    static let successRing: CGFloat = 1.5
    static let successRingOpacity: Double = 0.4
    /// The success body's measure, so a sentence wraps into a readable shape.
    static let successBodyWidth: CGFloat = 300
    /// Where the success block sits: its top gap is this share of the space
    /// the block leaves free (≈ 2 : 3 above : below — the optical centre).
    static let opticalTop: CGFloat = 0.4
    /// Adding and removing a tile: a short fade and scale.
    static let tileAnimation: TimeInterval = 0.15
}
