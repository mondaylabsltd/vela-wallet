package app.getvela.wallet.core.designsystem.tokens

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color

/**
 * Semantic color palette mirroring docs/design-tokens.json (Penpot DTCG export).
 * Modes are set activation: core + exactly one of color-light / color-dark.
 * Values are verified byte-equal to the export by DesignTokenDriftTest — edit the
 * Penpot source and re-export, never these literals.
 */
@Immutable
data class VelaColors(
    val fgBase: Color,
    val fgMuted: Color,
    val fgSubtle: Color,
    val fgInverse: Color,
    val bgBase: Color,
    val bgRaised: Color,
    val bgSunken: Color,
    val accentBase: Color,
    val accentSoft: Color,
    val successBase: Color,
    val successSoft: Color,
    val warningBase: Color,
    val warningSoft: Color,
    val warningBorder: Color,
    val errorBase: Color,
    val errorSoft: Color,
    val infoBase: Color,
    val infoSoft: Color,
    val borderBase: Color,
    val borderStrong: Color,
    val fixed: VelaFixedColors,
)

/** color.fixed.* — identical across modes except focusRingInner. */
@Immutable
data class VelaFixedColors(
    val shadowInk: Color,
    val backdrop: Color,
    val focusRingInner: Color,
    val focusRingOuter: Color,
    val splashBg: Color,
    val androidAdaptiveIconBg: Color,
    val desktopCanvas: Color,
)

/**
 * On-accent content color. Not in the DTCG export: proposed semantic token per the
 * design-system brief's handoff rule (white-on-accent CTA in both modes; the
 * ≈3.6:1 contrast is the recorded AA exception shared with specs 006/007).
 */
val VelaOnAccent: Color = Color(0xFFFFFFFF)

/**
 * The value button's fill (web `--color-accent-fill`, r4D-11): accent.base one
 * step darker, so its white 17-point label reads 4.5:1 or more in both themes.
 * Not in the DTCG export — a web addition the shells share; the same value in
 * both modes. Pinned by DesignTokenDriftTest against the web's tokens.css.
 */
val VelaAccentFill: Color = Color(0xFFBF441C)

val VelaColorsLight: VelaColors = VelaColors(
    fgBase = Color(0xFF1A1A18),
    fgMuted = Color(0xFF6E6B62),
    fgSubtle = Color(0xFF8C887E),
    fgInverse = Color(0xFFFFFFFF),
    bgBase = Color(0xFFFAFAF8),
    bgRaised = Color(0xFFFFFFFF),
    bgSunken = Color(0xFFF5F3EF),
    accentBase = Color(0xFFE8572A),
    accentSoft = Color(0xFFFFF0EB),
    successBase = Color(0xFF2D8E5F),
    successSoft = Color(0xFFEDFAF2),
    warningBase = Color(0xFF92600A),
    warningSoft = Color(0xFFFFF8F0),
    warningBorder = Color(0xFFF0DCC8),
    errorBase = Color(0xFFC62828),
    errorSoft = Color(0xFFFEF2F2),
    infoBase = Color(0xFF4267F4),
    infoSoft = Color(0xFFEDF0FF),
    borderBase = Color(0xFFECEBE4),
    borderStrong = Color(0xFFD8D6CE),
    fixed = VelaFixedColors(
        shadowInk = Color(0xFF1A1A18),
        backdrop = Color(0f, 0f, 0f, 0.35f),
        focusRingInner = Color(0xFFFAFAF8),
        focusRingOuter = Color(0xFFE8572A),
        splashBg = Color(0xFF1A1A18),
        androidAdaptiveIconBg = Color(0xFF0A1929),
        desktopCanvas = Color(0xFFE8E8E8),
    ),
)

val VelaColorsDark: VelaColors = VelaColors(
    fgBase = Color(0xFFE8E6E1),
    fgMuted = Color(0xFF9A9790),
    fgSubtle = Color(0xFF85827A),
    fgInverse = Color(0xFF1A1A18),
    bgBase = Color(0xFF141412),
    bgRaised = Color(0xFF1E1E1B),
    bgSunken = Color(0xFF0F0F0D),
    accentBase = Color(0xFFE8572A),
    accentSoft = Color(0xFF2C1A12),
    successBase = Color(0xFF3DA872),
    successSoft = Color(0xFF132A1E),
    warningBase = Color(0xFFD4A54A),
    warningSoft = Color(0xFF2A2010),
    warningBorder = Color(0xFF3D3020),
    errorBase = Color(0xFFF87171),
    errorSoft = Color(0xFF2D1515),
    infoBase = Color(0xFF5A7CF6),
    infoSoft = Color(0xFF131B33),
    borderBase = Color(0xFF2C2C28),
    borderStrong = Color(0xFF3E3E38),
    fixed = VelaFixedColors(
        shadowInk = Color(0xFF1A1A18),
        backdrop = Color(0f, 0f, 0f, 0.35f),
        focusRingInner = Color(0xFF141412),
        focusRingOuter = Color(0xFFE8572A),
        splashBg = Color(0xFF1A1A18),
        androidAdaptiveIconBg = Color(0xFF0A1929),
        desktopCanvas = Color(0xFFE8E8E8),
    ),
)
