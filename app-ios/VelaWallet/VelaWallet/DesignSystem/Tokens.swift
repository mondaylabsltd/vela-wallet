// GENERATED — do not edit. Regenerate: node app-ios/scripts/gen-tokens.mjs
// Source: docs/design-tokens.json (Penpot DTCG export — the design value authority).
// IOS_ADDITIONS entries are the only values the export lacks; each cites its
// docs/design-system.md license inside app-ios/scripts/gen-tokens.mjs.

import SwiftUI

/// A design-token color stored as 0xAARRGGBB.
struct TokenColor {
    let argb: UInt32

    var alpha: Double { Double((argb >> 24) & 0xFF) / 255.0 }
    var red: Double { Double((argb >> 16) & 0xFF) / 255.0 }
    var green: Double { Double((argb >> 8) & 0xFF) / 255.0 }
    var blue: Double { Double(argb & 0xFF) / 255.0 }

    var color: Color { Color(.sRGB, red: red, green: green, blue: blue, opacity: alpha) }

    /// `#rrggbb` — for artwork that is TINTED rather than drawn by SwiftUI: the
    /// passkey fallback marks are SVG documents rasterized by vela-core, and an
    /// SVG wants what CSS wants.
    var hex: String {
        String(format: "#%06X", argb & 0x00FF_FFFF)
    }
}

/// A design-token shadow. Export strings are "x y blur spread color" with
/// spread always 0 (SwiftUI has no spread); blur maps to `radius`.
struct TokenShadow {
    let color: TokenColor
    let radius: CGFloat
    let x: CGFloat
    let y: CGFloat
}

/// One appearance mode's semantic colors (sets color-light / color-dark).
struct TokenPalette {
    let bgBase: TokenColor
    let bgRaised: TokenColor
    let bgSunken: TokenColor
    let fgBase: TokenColor
    let fgMuted: TokenColor
    let fgSubtle: TokenColor
    let fgInverse: TokenColor
    let accentBase: TokenColor
    let accentSoft: TokenColor
    let successBase: TokenColor
    let successSoft: TokenColor
    let warningBase: TokenColor
    let warningSoft: TokenColor
    let errorBase: TokenColor
    let errorSoft: TokenColor
    let infoBase: TokenColor
    let infoSoft: TokenColor
    let borderBase: TokenColor
    let borderStrong: TokenColor
    /// IOS_ADDITIONS: label on accent surfaces, white in BOTH modes.
    let onAccent: TokenColor
}

enum Tokens {
    /// Set color-light.
    static let light = TokenPalette(
        bgBase: TokenColor(argb: 0xFFFAFAF8), // #FAFAF8
        bgRaised: TokenColor(argb: 0xFFFFFFFF), // #FFFFFF
        bgSunken: TokenColor(argb: 0xFFF5F3EF), // #F5F3EF
        fgBase: TokenColor(argb: 0xFF1A1A18), // #1A1A18
        fgMuted: TokenColor(argb: 0xFF6E6B62), // #6E6B62
        fgSubtle: TokenColor(argb: 0xFF8C887E), // #8C887E
        fgInverse: TokenColor(argb: 0xFFFFFFFF), // #FFFFFF
        accentBase: TokenColor(argb: 0xFFE8572A), // #E8572A
        accentSoft: TokenColor(argb: 0xFFFFF0EB), // #FFF0EB
        successBase: TokenColor(argb: 0xFF2D8E5F), // #2D8E5F
        successSoft: TokenColor(argb: 0xFFEDFAF2), // #EDFAF2
        warningBase: TokenColor(argb: 0xFF92600A), // #92600A
        warningSoft: TokenColor(argb: 0xFFFFF8F0), // #FFF8F0
        errorBase: TokenColor(argb: 0xFFC62828), // #C62828
        errorSoft: TokenColor(argb: 0xFFFEF2F2), // #FEF2F2
        infoBase: TokenColor(argb: 0xFF4267F4), // #4267F4
        infoSoft: TokenColor(argb: 0xFFEDF0FF), // #EDF0FF
        borderBase: TokenColor(argb: 0xFFECEBE4), // #ECEBE4
        borderStrong: TokenColor(argb: 0xFFD8D6CE), // #D8D6CE
        onAccent: TokenColor(argb: 0xFFFFFFFF) // IOS_ADDITIONS: #FFFFFF both modes
    )

    /// Set color-dark.
    static let dark = TokenPalette(
        bgBase: TokenColor(argb: 0xFF141412), // #141412
        bgRaised: TokenColor(argb: 0xFF1E1E1B), // #1E1E1B
        bgSunken: TokenColor(argb: 0xFF0F0F0D), // #0F0F0D
        fgBase: TokenColor(argb: 0xFFE8E6E1), // #E8E6E1
        fgMuted: TokenColor(argb: 0xFF9A9790), // #9A9790
        fgSubtle: TokenColor(argb: 0xFF85827A), // #85827A
        fgInverse: TokenColor(argb: 0xFF1A1A18), // #1A1A18
        accentBase: TokenColor(argb: 0xFFE8572A), // #E8572A
        accentSoft: TokenColor(argb: 0xFF2C1A12), // #2C1A12
        successBase: TokenColor(argb: 0xFF3DA872), // #3DA872
        successSoft: TokenColor(argb: 0xFF132A1E), // #132A1E
        warningBase: TokenColor(argb: 0xFFD4A54A), // #D4A54A
        warningSoft: TokenColor(argb: 0xFF2A2010), // #2A2010
        errorBase: TokenColor(argb: 0xFFF87171), // #F87171
        errorSoft: TokenColor(argb: 0xFF2D1515), // #2D1515
        infoBase: TokenColor(argb: 0xFF5A7CF6), // #5A7CF6
        infoSoft: TokenColor(argb: 0xFF131B33), // #131B33
        borderBase: TokenColor(argb: 0xFF2C2C28), // #2C2C28
        borderStrong: TokenColor(argb: 0xFF3E3E38), // #3E3E38
        onAccent: TokenColor(argb: 0xFFFFFFFF) // IOS_ADDITIONS: #FFFFFF both modes
    )

    /// core space (member = point value: space "xl" 16 -> s16).
    enum Space {
        static let s0: CGFloat = 0
        static let s2: CGFloat = 2
        static let s4: CGFloat = 4
        static let s8: CGFloat = 8
        static let s12: CGFloat = 12
        static let s16: CGFloat = 16
        static let s20: CGFloat = 20
        static let s24: CGFloat = 24
        static let s32: CGFloat = 32
        static let s48: CGFloat = 48
    }

    /// core text sizes (member = point value: text "xl" 17 -> t17).
    enum TextSize {
        static let t10: CGFloat = 10
        static let t11: CGFloat = 11
        static let t13: CGFloat = 13
        static let t15: CGFloat = 15
        static let t17: CGFloat = 17
        static let t20: CGFloat = 20
        static let t26: CGFloat = 26
        static let t32: CGFloat = 32
        static let t40: CGFloat = 40
    }

    /// core weight, as raw font-weight numbers.
    enum Weight {
        static let regular: Int = 400
        static let medium: Int = 500
        static let semibold: Int = 600
        static let bold: Int = 700
    }

    /// core radius (member = point value) + full.
    enum Radius {
        static let r0: CGFloat = 0
        static let r4: CGFloat = 4
        static let r8: CGFloat = 8
        static let r12: CGFloat = 12
        static let r16: CGFloat = 16
        static let r20: CGFloat = 20
        static let full: CGFloat = 9999
    }

    /// core leading — line-height multipliers.
    enum Leading {
        static let none: CGFloat = 1
        static let tight: CGFloat = 1.2
        static let normal: CGFloat = 1.4
        static let relaxed: CGFloat = 1.6
        static let amountHero: CGFloat = 1.12
    }

    /// core opacity.
    enum Opacity {
        static let disabled: Double = 0.45
        static let dim: Double = 0.4
        static let backdrop: Double = 0.35
    }

    /// core motion.duration, ms -> seconds ("normal" exposed as `base`).
    enum Motion {
        static let fast: TimeInterval = 0.15 // 150ms
        static let base: TimeInterval = 0.25 // 250ms
        static let slow: TimeInterval = 0.4 // 400ms
    }

    /// core layout + hit sizes.
    enum Layout {
        static let screenPaddingX: CGFloat = 24
        static let maxContentWidth: CGFloat = 800
        static let frameW: CGFloat = 390
        static let frameH: CGFloat = 844
        static let hitTarget: CGFloat = 44 // core size.hitTarget
        static let hitSlop: CGFloat = 8 // core size.hitSlop
    }

    /// core border widths.
    enum BorderWidth {
        static let hairline: CGFloat = 1
        static let emphasis: CGFloat = 1.5
    }

    /// core shadow.
    enum Shadow {
        static let sm = TokenShadow(color: TokenColor(argb: 0x0A1A1A18), radius: 3, x: 0, y: 1)
        static let md = TokenShadow(color: TokenColor(argb: 0x0F1A1A18), radius: 8, x: 0, y: 2)
        static let lg = TokenShadow(color: TokenColor(argb: 0x141A1A18), radius: 16, x: 0, y: 4)
    }

    /// IOS_ADDITIONS: docs/design-system.md sizing.control.sm/md/lg.
    enum Control {
        static let sm: CGFloat = 36
        static let md: CGFloat = 44
        static let lg: CGFloat = 52
    }

    /// core letterSpacing.
    enum LetterSpacing {
        static let sectionLabel: CGFloat = 0.6
    }
}
