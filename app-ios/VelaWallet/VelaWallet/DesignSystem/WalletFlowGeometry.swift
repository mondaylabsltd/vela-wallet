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

    /// The receive code's "include network" switch (spec 090) — the web's
    /// and desktop's 36×20 track with a 16 thumb that stretches to 20 while
    /// pressed.
    static let switchTrack = CGSize(width: 36, height: 20)
    static let switchThumb: CGFloat = 16
    static let switchThumbPressed: CGFloat = 20

    /// The send-receipt status disc, measured 88 in SD4a/SD4c. One size for
    /// all four outcomes so the mark does not resize as the transaction moves
    /// between them.
    static let statusHero: CGFloat = 88
    /// The spinner arc inside it.
    static let statusSpinner: CGFloat = 26
    static let statusSpinnerStroke: CGFloat = 3
    /// The waiting ring OUTSIDE the disc (issue 199, the web's 2.5 in 104).
    static let statusRingStroke: CGFloat = 2.5
    /// Issue #444: a submitted transaction with no time estimate still MOVES —
    /// a quarter arc circling the disc once per period, the disc breathing to
    /// this scale (the web's `roam`/`breathe`, Android's `receiptWait`).
    static let statusRoamPeriod: TimeInterval = 2.4
    static let statusBreathePeriod: TimeInterval = 1.2
    static let statusBreatheScale: CGFloat = 1.04
    /// The arc drawn while circling: a quarter of the ring.
    static let statusRoamArc: Double = 0.25

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
}

/// The share card (R4) — the picture 保存图片 puts in the album, drawn to the
/// geometry every platform copies by hand from the web's `SHARE_CARD`
/// (`app-web/…/flows/share-image.ts`, 2026-09-27: the WeChat Pay collection
/// card the founder holds it against). Points here are the web's CSS px at 1×.
///
/// A render product, not a screen: the geometry is fixed rather than
/// responsive, the text does not follow 设置 → 字号 or Dynamic Type, and the
/// colours do not follow the appearance — the image is saved once and viewed
/// anywhere. Only the HEIGHT moves, with the headline's line count.
enum ShareCardGeometry {
    static let width: CGFloat = 480
    static let top: CGFloat = 52
    /// Headline: one line at 32 when it fits, shrinking to 26, then two lines.
    static let headlineSize: CGFloat = 32
    static let headlineMinSize: CGFloat = 26
    static let headlineLeading: CGFloat = 1.25
    /// Widest a line of text on the orange may run.
    static let textWidth: CGFloat = 400
    static let noteGap: CGFloat = 10
    static let noteSize: CGFloat = 15
    static let noteMinSize: CGFloat = 11
    static let noteLine: CGFloat = 20
    static let sheetGap: CGFloat = 28
    static let sheetWidth: CGFloat = 320
    static let sheetRadius: CGFloat = 20
    static let sheetPad: CGFloat = 48
    static let sheetPadBottom: CGFloat = 40
    static let qr: CGFloat = 224
    /// The white plate the network logo sits on, in the code's centre.
    static let plate: CGFloat = 60
    static let plateRadius: CGFloat = 16
    static let logo: CGFloat = 44
    /// The hairline round a fetched logo, so a white one does not dissolve
    /// into the plate.
    static let logoRing: CGFloat = 1
    static let logoRingOpacity: Double = 0.08
    /// The lettered disc that stands in when no logo could be fetched.
    static let tickerSize: CGFloat = 14
    static let identityGap: CGFloat = 26
    static let identicon: CGFloat = 48
    static let identityTextGap: CGFloat = 12
    static let nameSize: CGFloat = 17
    static let nameLine: CGFloat = 22
    static let addressSize: CGFloat = 12.5
    static let addressLine: CGFloat = 17
    static let addressOpacity: Double = 0.5
    static let nameAddressGap: CGFloat = 3
    /// Sheet bottom to where the curve leaves the card's edges.
    static let curveGap: CGFloat = 52
    /// How far the curve dips at the centre.
    static let curveDepth: CGFloat = 32
    /// The curve's lowest point to the card's bottom.
    static let foot: CGFloat = 112
    static let icon: CGFloat = 52
    static let iconGap: CGFloat = 12
    static let wordmarkSize: CGFloat = 32
    /// Rasterised at 2× whatever the phone's own scale, so every device saves
    /// the same 960-pixel-wide picture the web does.
    static let scale: CGFloat = 2
    /// Where a line of text's baseline sits below the centre of its line, as a
    /// share of its size — the web's `baseline()`, so the two agree on where
    /// every line lands.
    static let baselineDrop: CGFloat = 0.35

    /// The card's fixed colours: paper, ink, and the field — the APP ICON's
    /// own orange (founder, 2026-08-15: #F46D50, not the UI accent).
    static let paper = TokenColor(argb: 0xFFFFFFFF).color
    static let ink = TokenColor(argb: 0xFF1A1A18).color
    static let field = AppIconArt.plate

    /// The canonical app icon, `docs/design/icon/app-icon.svg` — the picture
    /// the home screen shows, NOT the in-app sailboat (`VelaMark`). The card
    /// carries the app's identity away from the app, so it carries the icon
    /// a person would find on the phone.
    enum AppIconArt {
        static let viewBox: CGFloat = 68
        /// The plate: x 1, y 1, 66 across, corners 18.
        static let plateInset: CGFloat = 1
        static let plateSize: CGFloat = 66
        static let plateRadius: CGFloat = 18
        static let plate = TokenColor(argb: 0xFFF46D50).color
        static let mainSail = TokenColor(argb: 0xFFFFF3EC).color
        static let jib = TokenColor(argb: 0xFFFFC6B0).color
        static let hull = TokenColor(argb: 0xFF5A4037).color
    }
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
