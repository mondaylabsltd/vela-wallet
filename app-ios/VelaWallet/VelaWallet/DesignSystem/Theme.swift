//
//  Theme.swift
//  VelaWallet
//
//  Semantic palette resolution + welcome-screen geometry. The only
//  hand-written file allowed to compose token values (Tokens.swift is
//  generated; nothing outside DesignSystem/ may name a visual value).
//

import SwiftUI

/// The active design-system theme: exactly one semantic color set
/// (`color-light` or `color-dark`) plus the mode-invariant core scales.
struct Theme {
    let palette: TokenPalette
    let scheme: ColorScheme

    init(scheme: ColorScheme) {
        self.scheme = scheme
        self.palette = scheme == .dark ? Tokens.dark : Tokens.light
    }

    // MARK: Semantic colors (SwiftUI)

    var bgBase: Color { palette.bgBase.color }
    var bgRaised: Color { palette.bgRaised.color }
    var bgSunken: Color { palette.bgSunken.color }
    var fgBase: Color { palette.fgBase.color }
    var fgMuted: Color { palette.fgMuted.color }

    /// The three colour slots vela-core's fallback artwork wears. Read off the
    /// active palette so the mark belongs to this app in either theme, rather
    /// than carrying the greys it shipped with.
    var markPalette: MarkPalette {
        MarkPalette(
            strong: palette.fgMuted.hex,
            soft: palette.borderStrong.hex,
            hole: palette.bgBase.hex
        )
    }
    var fgSubtle: Color { palette.fgSubtle.color }
    var accentBase: Color { palette.accentBase.color }
    var accentSoft: Color { palette.accentSoft.color }
    var borderBase: Color { palette.borderBase.color }
    var borderStrong: Color { palette.borderStrong.color }
    var onAccent: Color { palette.onAccent.color }

    /// Brand mark hull — themed per docs/design-system.md brand rules
    /// (Warm Graphite in light UI, Dusk Ivory in dark UI); sails constant.
    var markHull: Color { scheme == .dark ? Brand.hullDark.color : Brand.hullLight.color }

    // MARK: Status colors (spec 014 outcome badges / diagnostics)

    var successBase: Color { palette.successBase.color }
    var successSoft: Color { palette.successSoft.color }
    var warningBase: Color { palette.warningBase.color }
    var warningSoft: Color { palette.warningSoft.color }
    var errorBase: Color { palette.errorBase.color }
    var errorSoft: Color { palette.errorSoft.color }
    var infoBase: Color { palette.infoBase.color }
    var infoSoft: Color { palette.infoSoft.color }
}

/// UIKit colors, for the few places a `UIView` has to be handed one.
///
/// The ack row's legal line is drawn by TextKit, because SwiftUI's `Text`
/// cannot say which character a tap landed on and the row needs to know
/// (`AckRow`). This is the one bridge that needs; nothing else should.
extension TokenColor {
    var uiColor: UIColor {
        UIColor(red: red, green: green, blue: blue, alpha: alpha)
    }
}

/// Brand constants from docs/design-system.md's brand section (mode-dependent by
/// rule, not part of the Penpot color sets — see spec 007 logo tokens).
enum Brand {
    static let sailMain = TokenColor(argb: 0xFF_FF6A45)
    static let sailSecondary = TokenColor(argb: 0xFF_FFA98E)
    static let hullLight = TokenColor(argb: 0xFF_554B46)
    static let hullDark = TokenColor(argb: 0xFF_DED5CE)
}

/// Welcome-screen geometry the token set does not name, measured from
/// `docs/design/onboarding/W1 Welcome _ default.png` at the 390×844 design frame
/// (@2x pixels ÷ 2). Licensed by docs/design-system.md ("if a needed token doesn't
/// exist… propose a semantic name") — kept here, never inline in views.
enum WelcomeGeometry {
    /// The v2 mark: a 60pt glyph beside a small tracked wordmark, not v1's
    /// 37pt mark under a 42pt display title (docs/design/onboarding-new; the web
    /// and the desktop draw the same pair at the same size).
    static let markSize: CGFloat = 60
    /// Gap between mark and wordmark.
    static let markWordmarkGap: CGFloat = Tokens.Space.s12
    /// The wordmark is a LABEL beside the mark — heavy and widely tracked, but
    /// not so small that a 60pt mark dwarfs it: at t17 the pair read as a big
    /// boat with a caption (founder-found 2026-08-25).
    static let wordmarkSize: CGFloat = Tokens.TextSize.t20
    static let wordmarkTracking: CGFloat = 0.11
    /// Brand row → hero, and hero → its supporting line.
    static let brandHeroGap: CGFloat = Tokens.Space.s24
    static let heroSubGap: CGFloat = Tokens.Space.s12
    /// The hero headline. The DTCG scale tops out at 40 and the design asks
    /// for 46/38 — this is the compact one, which is what a phone gets (web
    /// precedent: WEB_ADDITIONS `text-heroCompact`).
    static let heroSize: CGFloat = 38
    /// One rung down, for a locale whose headline is too wide for `heroSize`
    /// (the corpus says which, in `heroTitleFit`). The ladder is 46/38/31,
    /// stepping ~0.82 each; measured at the shipped font the widest authored
    /// line runs 6.9em (zh) to 10.9em (ru), and 31 is what fits the widest of
    /// them in the 342pt the 390pt design frame leaves between its gutters.
    /// 390 is the contract, not the floor: a 375pt phone has ~15pt less than
    /// the widest headline needs and is allowed to wrap (founder direction
    /// 2026-08-26). Do not shrink a locale a rung to serve it — that shrinks
    /// the headline on every phone.
    static let heroSizeLong: CGFloat = 31
    static let heroLeading: CGFloat = 1.25
    /// Optical tightening on a headline this large, as the web sets it.
    static let heroTracking: CGFloat = -0.02
    /// Between the two ways in.
    static let ctaGap: CGFloat = Tokens.Space.s16
    /// The least air between the hero block and the CTAs; on a tall phone the
    /// gap is whatever is left over.
    static let heroCtaMinGap: CGFloat = Tokens.Space.s32
}

/// Onboarding-flow (create/login sheet) geometry the token set does not
/// name, measured from `docs/design/onboarding/create|login/*.png` at the
/// 390×844 design frame (spec 014). Same docs/design-system.md license as
/// `WelcomeGeometry` — values live here, never inline in views.
enum FlowGeometry {
    /// Outcome status-badge circle (mock ≈ 112 px @2x).
    static let badgeSize: CGFloat = 56
    /// SF Symbol point size inside the badge circle.
    static let badgeGlyphSize: CGFloat = 24
    /// Elapsed-seconds ring outer size (mock ≈ 80 px @2x).
    static let ringSize: CGFloat = 40
    /// Ring stroke width.
    static let ringLineWidth: CGFloat = 3
    /// Frozen sweep fraction of the ring arc (mock shows an open arc;
    /// no elapsed-time measurement is wired in this feature — FR-011).
    static let ringSweep: CGFloat = 0.72
    /// Arc start angle: 12 o'clock.
    static let ringStartDegrees: CGFloat = -90
    /// Progress bar height (stepped segments and the login single track).
    static let barHeight: CGFloat = 4
    /// Gap between step segments (space.s8).
    static let barGap: CGFloat = Tokens.Space.s8
    /// Login waiting bar fill fraction (mock ≈ 40%).
    static let loginBarFill: CGFloat = 0.4
    /// Acknowledgment checkbox square (mock ≈ 40 px @2x).
    static let checkboxSize: CGFloat = 20
    /// Check glyph point size inside the checkbox.
    static let checkboxGlyphSize: CGFloat = 12
    /// Small control glyphs: close ×, disclosure chevron, copy icon.
    static let controlGlyphSize: CGFloat = 16
    /// Account-name field height (sizing control.lg).
    static let fieldHeight: CGFloat = Tokens.Control.lg
    /// Secondary action-row height (matches the primary CTA control.lg).
    static let actionRowHeight: CGFloat = Tokens.Control.lg
    /// Vertical gap between stacked actions (mock ≈ 24 px @2x).
    static let actionGap: CGFloat = Tokens.Space.s12
}

/// SF Symbols are sized through a font; this is the sanctioned wrapper for
/// the flow glyphs so no view outside DesignSystem/ calls `Font.system`
/// (audit-literals). Text NEVER uses these — text goes through Typography.
enum GlyphFont {
    /// Badge glyph (✓ / ! / × / clock) inside the 56 pt circle.
    static let badge: Font = .system(size: FlowGeometry.badgeGlyphSize, weight: .semibold)
    /// Close ×, disclosure chevron, address-copy icon.
    static let control: Font = .system(size: FlowGeometry.controlGlyphSize, weight: .medium)
    /// Ack checkbox check mark.
    static let checkbox: Font = .system(size: FlowGeometry.checkboxGlyphSize, weight: .bold)
}

// MARK: - Environment plumbing

private struct ThemeKey: EnvironmentKey {
    static let defaultValue = Theme(scheme: .light)
}

extension EnvironmentValues {
    var theme: Theme {
        get { self[ThemeKey.self] }
        set { self[ThemeKey.self] = newValue }
    }
}

extension View {
    /// Injects the theme resolved from the current color scheme.
    func themed(_ scheme: ColorScheme) -> some View {
        environment(\.theme, Theme(scheme: scheme))
    }
}
