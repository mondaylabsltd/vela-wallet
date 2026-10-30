package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.TextUnit
import app.getvela.wallet.core.designsystem.tokens.VelaBorder
import app.getvela.wallet.core.designsystem.tokens.VelaColors
import app.getvela.wallet.core.designsystem.tokens.VelaColorsDark
import app.getvela.wallet.core.designsystem.tokens.VelaColorsLight
import app.getvela.wallet.core.designsystem.tokens.VelaFontWeight
import app.getvela.wallet.core.designsystem.tokens.VelaIconSize
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaLetterSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.core.designsystem.tokens.VelaOnAccent
import app.getvela.wallet.core.designsystem.tokens.VelaOpacity
import app.getvela.wallet.core.designsystem.tokens.VelaRadius
import app.getvela.wallet.core.designsystem.tokens.VelaSizing
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * FR-005 drift gate (SC-002a): every mirrored Kotlin token value must equal
 * docs/design-tokens.json (the Penpot DTCG export). The JSON is the model of
 * record; this test makes the Kotlin projection unable to rot silently.
 */
class DesignTokenDriftTest {

    private val export: JSONObject by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root system property not set (see app/build.gradle.kts testOptions)")
        val file = File(root, "docs/design-tokens.json")
        assertTrue("design-tokens.json missing at ${file.absolutePath}", file.isFile)
        JSONObject(file.readText())
    }

    private fun token(setName: String, vararg path: String): String {
        var node = export.getJSONObject(setName)
        for (segment in path.dropLast(1)) node = node.getJSONObject(segment)
        return node.getJSONObject(path.last()).getString("\$value")
    }

    private fun hexColor(setName: String, vararg path: String): Color {
        val value = token(setName, *path)
        return if (value.startsWith("rgba")) {
            val parts = value.removePrefix("rgba(").removeSuffix(")").split(",")
            Color(
                red = parts[0].trim().toFloat() / 255f,
                green = parts[1].trim().toFloat() / 255f,
                blue = parts[2].trim().toFloat() / 255f,
                alpha = parts[3].trim().toFloat(),
            )
        } else {
            Color(("FF" + value.removePrefix("#")).toLong(16))
        }
    }

    private fun assertDp(expected: String, actual: Dp) =
        assertEquals(expected.toFloat(), actual.value, 0.0001f)

    private fun assertSp(expected: String, actual: TextUnit) =
        assertEquals(expected.toFloat(), actual.value, 0.0001f)

    private fun assertPalette(setName: String, palette: VelaColors) {
        assertEquals(hexColor(setName, "color", "fg", "base"), palette.fgBase)
        assertEquals(hexColor(setName, "color", "fg", "muted"), palette.fgMuted)
        assertEquals(hexColor(setName, "color", "fg", "subtle"), palette.fgSubtle)
        assertEquals(hexColor(setName, "color", "fg", "inverse"), palette.fgInverse)
        assertEquals(hexColor(setName, "color", "bg", "base"), palette.bgBase)
        assertEquals(hexColor(setName, "color", "bg", "raised"), palette.bgRaised)
        assertEquals(hexColor(setName, "color", "bg", "sunken"), palette.bgSunken)
        assertEquals(hexColor(setName, "color", "accent", "base"), palette.accentBase)
        assertEquals(hexColor(setName, "color", "accent", "soft"), palette.accentSoft)
        assertEquals(hexColor(setName, "color", "success", "base"), palette.successBase)
        assertEquals(hexColor(setName, "color", "success", "soft"), palette.successSoft)
        assertEquals(hexColor(setName, "color", "warning", "base"), palette.warningBase)
        assertEquals(hexColor(setName, "color", "warning", "soft"), palette.warningSoft)
        assertEquals(hexColor(setName, "color", "warning", "border"), palette.warningBorder)
        assertEquals(hexColor(setName, "color", "error", "base"), palette.errorBase)
        assertEquals(hexColor(setName, "color", "error", "soft"), palette.errorSoft)
        assertEquals(hexColor(setName, "color", "info", "base"), palette.infoBase)
        assertEquals(hexColor(setName, "color", "info", "soft"), palette.infoSoft)
        assertEquals(hexColor(setName, "color", "border", "base"), palette.borderBase)
        assertEquals(hexColor(setName, "color", "border", "strong"), palette.borderStrong)
        assertEquals(hexColor(setName, "color", "fixed", "shadowInk"), palette.fixed.shadowInk)
        assertEquals(hexColor(setName, "color", "fixed", "backdrop"), palette.fixed.backdrop)
        assertEquals(hexColor(setName, "color", "fixed", "focusRingInner"), palette.fixed.focusRingInner)
        assertEquals(hexColor(setName, "color", "fixed", "focusRingOuter"), palette.fixed.focusRingOuter)
        assertEquals(hexColor(setName, "color", "fixed", "splashBg"), palette.fixed.splashBg)
        assertEquals(
            hexColor(setName, "color", "fixed", "androidAdaptiveIconBg"),
            palette.fixed.androidAdaptiveIconBg,
        )
        assertEquals(hexColor(setName, "color", "fixed", "desktopCanvas"), palette.fixed.desktopCanvas)
    }

    @Test
    fun lightPaletteMatchesExport() = assertPalette("color-light", VelaColorsLight)

    @Test
    fun darkPaletteMatchesExport() = assertPalette("color-dark", VelaColorsDark)

    @Test
    fun onAccentIsWhiteInBothModes() {
        // Proposed semantic token (not in the export): white-on-accent CTA content,
        // the recorded ≈3.6:1 AA exception shared with specs 006/007.
        assertEquals(Color(0xFFFFFFFF), VelaOnAccent)
    }

    /**
     * The value button's fill (Phase 3: accent only on value-moving actions):
     * the web's `--color-accent-fill` (r4D-11, a web addition outside the
     * export), the same in every mode it is declared for.
     */
    @Test
    fun accentFillIsTheWebValueButtons() {
        val css = File(System.getProperty("vela.repo.root"), "app-web/vela-wallet/src/lib/tokens/tokens.css").readText()
        val fills = Regex("--color-accent-fill:\\s*#([0-9A-Fa-f]{6})").findAll(css).map { it.groupValues[1].uppercase() }.toSet()
        assertEquals("one accent fill in every mode: $fills", 1, fills.size)
        assertEquals(Color(0xFF000000 or fills.single().toLong(16)), app.getvela.wallet.core.designsystem.tokens.VelaAccentFill)
    }

    @Test
    fun spacingMatchesExport() {
        assertDp(token("core", "space", "0"), VelaSpacing.none)
        assertDp(token("core", "space", "xs"), VelaSpacing.xs)
        assertDp(token("core", "space", "sm"), VelaSpacing.sm)
        assertDp(token("core", "space", "md"), VelaSpacing.md)
        assertDp(token("core", "space", "lg"), VelaSpacing.lg)
        assertDp(token("core", "space", "xl"), VelaSpacing.xl)
        assertDp(token("core", "space", "2xl"), VelaSpacing.xl2)
        assertDp(token("core", "space", "3xl"), VelaSpacing.xl3)
        assertDp(token("core", "space", "4xl"), VelaSpacing.xl4)
        assertDp(token("core", "space", "5xl"), VelaSpacing.xl5)
        assertDp(token("core", "layout", "screenPaddingX"), VelaSizing.screenPaddingX)
        assertDp(token("core", "layout", "scanFabSize"), VelaSizing.scanFabSize)
    }

    @Test
    fun radiusAndBordersMatchExport() {
        assertDp(token("core", "radius", "none"), VelaRadius.none)
        assertDp(token("core", "radius", "sm"), VelaRadius.sm)
        assertDp(token("core", "radius", "md"), VelaRadius.md)
        assertDp(token("core", "radius", "lg"), VelaRadius.lg)
        assertDp(token("core", "radius", "xl"), VelaRadius.xl)
        assertDp(token("core", "radius", "2xl"), VelaRadius.xl2)
        assertDp(token("core", "radius", "full"), VelaRadius.full)
        assertDp(token("core", "border", "hairline"), VelaBorder.hairline)
        assertDp(token("core", "border", "emphasis"), VelaBorder.emphasis)
    }

    @Test
    fun typographyMatchesExport() {
        assertSp(token("core", "text", "xs"), VelaTextSize.xs)
        assertSp(token("core", "text", "sm"), VelaTextSize.sm)
        assertSp(token("core", "text", "base"), VelaTextSize.base)
        assertSp(token("core", "text", "lg"), VelaTextSize.lg)
        assertSp(token("core", "text", "xl"), VelaTextSize.xl)
        assertSp(token("core", "text", "2xl"), VelaTextSize.xl2)
        assertSp(token("core", "text", "3xl"), VelaTextSize.xl3)
        assertSp(token("core", "text", "4xl"), VelaTextSize.xl4)
        assertSp(token("core", "text", "5xl"), VelaTextSize.xl5)

        assertEquals(token("core", "weight", "regular").toInt(), VelaFontWeight.regular.weight)
        assertEquals(token("core", "weight", "medium").toInt(), VelaFontWeight.medium.weight)
        assertEquals(token("core", "weight", "semibold").toInt(), VelaFontWeight.semibold.weight)
        assertEquals(token("core", "weight", "bold").toInt(), VelaFontWeight.bold.weight)

        assertEquals(token("core", "leading", "none").toFloat(), VelaLeading.none, 0.0001f)
        assertEquals(token("core", "leading", "tight").toFloat(), VelaLeading.tight, 0.0001f)
        assertEquals(token("core", "leading", "normal").toFloat(), VelaLeading.normal, 0.0001f)
        assertEquals(token("core", "leading", "relaxed").toFloat(), VelaLeading.relaxed, 0.0001f)
        assertEquals(token("core", "leading", "amountHero").toFloat(), VelaLeading.amountHero, 0.0001f)

        assertSp(token("core", "letterSpacing", "sectionLabel"), VelaLetterSpacing.sectionLabel)
    }

    @Test
    fun motionMatchesExport() {
        assertEquals(token("core", "motion", "duration", "fast").toInt(), VelaMotion.durationFast)
        assertEquals(token("core", "motion", "duration", "normal").toInt(), VelaMotion.durationNormal)
        assertEquals(token("core", "motion", "duration", "slow").toInt(), VelaMotion.durationSlow)
        assertEquals(token("core", "motion", "sheet", "in").toInt(), VelaMotion.sheetIn)
        assertEquals(token("core", "motion", "sheet", "out").toInt(), VelaMotion.sheetOut)
        assertEquals(token("core", "motion", "sheet", "drag").toInt(), VelaMotion.sheetDrag)
        assertEquals(token("core", "motion", "entrance", "fade").toInt(), VelaMotion.entranceFade)
        assertEquals(token("core", "motion", "entrance", "fadeUp").toInt(), VelaMotion.entranceFadeUp)
        assertEquals(token("core", "motion", "entrance", "stagger").toInt(), VelaMotion.entranceStagger)
        assertEquals(token("core", "motion", "press", "button").toFloat(), VelaMotion.pressScaleButton, 0.0001f)
        assertEquals(token("core", "motion", "press", "row").toFloat(), VelaMotion.pressScaleRow, 0.0001f)
        assertEquals(token("core", "motion", "press", "fab").toFloat(), VelaMotion.pressScaleFab, 0.0001f)
        assertEquals(token("core", "motion", "spring", "damping").toFloat(), VelaMotion.springDamping, 0.0001f)
        assertEquals(token("core", "motion", "spring", "stiffness").toFloat(), VelaMotion.springStiffness, 0.0001f)
        assertEquals(token("core", "motion", "spring", "mass").toFloat(), VelaMotion.springMass, 0.0001f)
        assertEquals(token("core", "motion", "springGentle", "damping").toFloat(), VelaMotion.springGentleDamping, 0.0001f)
        assertEquals(token("core", "motion", "springGentle", "stiffness").toFloat(), VelaMotion.springGentleStiffness, 0.0001f)
        assertEquals(token("core", "motion", "springGentle", "mass").toFloat(), VelaMotion.springGentleMass, 0.0001f)
    }

    @Test
    fun opacityAndSizesMatchExport() {
        assertEquals(token("core", "opacity", "disabled").toFloat(), VelaOpacity.disabled, 0.0001f)
        assertEquals(token("core", "opacity", "dim").toFloat(), VelaOpacity.dim, 0.0001f)
        assertEquals(token("core", "opacity", "backdrop").toFloat(), VelaOpacity.backdrop, 0.0001f)

        assertDp(token("core", "size", "hitTarget"), VelaSizing.hitTarget)
        assertDp(token("core", "size", "hitSlop"), VelaSizing.hitSlop)
        assertDp(token("core", "size", "emptyStateCircle"), VelaSizing.emptyStateCircle)

        assertDp(token("core", "icon", "xs"), VelaIconSize.xs)
        assertDp(token("core", "icon", "sm"), VelaIconSize.sm)
        assertDp(token("core", "icon", "base"), VelaIconSize.base)
        assertDp(token("core", "icon", "md"), VelaIconSize.md)
        assertDp(token("core", "icon", "lg"), VelaIconSize.lg)
        assertDp(token("core", "icon", "2xl"), VelaIconSize.xl2)
        assertDp(token("core", "icon", "3xl"), VelaIconSize.xl3)
        assertDp(token("core", "icon", "xl"), VelaIconSize.xl)
    }

    @Test
    fun windowXmlColorsMatchExport() {
        val root = File(System.getProperty("vela.repo.root")!!)
        val appRes = File(root, "app-android/vela-wallet/app/src/main/res")
        val values = File(appRes, "values/colors.xml").readText()
        val night = File(appRes, "values-night/colors.xml").readText()

        fun hexOf(setName: String, vararg path: String) = token(setName, *path).uppercase()

        assertTrue(values.contains(hexOf("color-light", "color", "fixed", "splashBg")))
        assertTrue(values.contains(hexOf("color-light", "color", "bg", "base")))
        assertTrue(night.contains(hexOf("color-dark", "color", "bg", "base")))
    }
}
