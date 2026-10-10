package app.getvela.wallet

import app.getvela.wallet.core.designsystem.theme.VelaTypography
import app.getvela.wallet.core.designsystem.tokens.VelaFontFeatures
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * "0x14fB…" was drawn "0×14fB…" (iPhone pass 2026-10-09): Plus Jakarta Sans,
 * bundled byte for byte on every shell, turns an `x` after a digit into `×`
 * through its one contextual-alternates rule. Android turns `calt` off in the
 * theme's type scale — the style every Text takes from the theme — and in the
 * text fields that build their own style. (No JVM Compose harness shapes text
 * here; the theme's wiring is read from its source, as SettingsPagesTest does.)
 */
class LiteralFaceTest {

    @Test
    fun `contextual alternates are off in every style of the theme's type scale`() {
        assertEquals("calt 0", VelaFontFeatures)
        val styles = with(VelaTypography) {
            listOf(
                displayLarge, displayMedium, displaySmall, headlineLarge, headlineMedium, headlineSmall,
                titleLarge, titleMedium, titleSmall, bodyLarge, bodyMedium, bodySmall,
                labelLarge, labelMedium, labelSmall,
            )
        }
        styles.forEach { assertEquals(VelaFontFeatures, it.fontFeatureSettings) }
    }

    private fun source(path: String): String {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        return File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/$path").readText()
    }

    /**
     * The final round's "0×" check. The theme's scale reaches a `Text` only
     * through `LocalTextStyle` — and a style BUILT in a screen never passes
     * through it: a `TextStyle(…)` handed to `Text(style = …)`, to a
     * `BasicTextField` or to a text measurer replaces the theme's, and the
     * face's contextual alternates are back on. Thirteen such styles drew in
     * the UI face without the rule — the browser's URL bar and Explore's
     * address box ("…/address/0×14fB…"), the token search, a contact's name,
     * the activity row that names an ADDRESS, the share card — and four more
     * asked for `"tnum"` alone, which replaces the features too.
     *
     * So: every `TextStyle(` constructed in the app says its features, unless
     * it is merged INTO the theme's style or is set in the platform mono face
     * (which has no such rule); and no style spells features as a bare
     * string — the two constants are the only spellings.
     */
    @Test
    fun `every text style built outside the theme says that 0x stays 0x`() {
        val root = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"), "app-android/vela-wallet/app/src/main/java")
        val offenders = mutableListOf<String>()
        var seen = 0
        root.walkTopDown().filter { it.isFile && it.extension == "kt" }.forEach { file ->
            val text = file.readText()
            Regex("(?<![A-Za-z_])TextStyle\\(").findAll(text).forEach { match ->
                var depth = 1
                var end = match.range.last + 1
                while (depth > 0 && end < text.length) {
                    when (text[end]) {
                        '(' -> depth += 1
                        ')' -> depth -= 1
                    }
                    end += 1
                }
                val body = text.substring(match.range.last + 1, end - 1)
                val merged = text.substring(0, match.range.first).trimEnd().endsWith(".merge(")
                val monoOnly = "VelaMonoFontFamily" in body && "VelaFontFamily" !in body.replace("VelaMonoFontFamily", "")
                seen += 1
                if ("fontFeatureSettings" !in body && !merged && !monoOnly) {
                    offenders += "${file.relativeTo(root)}:${text.substring(0, match.range.first).count { it == '\n' } + 1}"
                }
            }
            if (file.name != "VelaType.kt") {
                Regex("fontFeatureSettings\\s*=\\s*\"").findAll(text).forEach { match ->
                    offenders += "${file.relativeTo(root)}:${text.substring(0, match.range.first).count { it == '\n' } + 1} spells its features as a bare string"
                }
            }
        }
        assertTrue("the scan found the styles it is about ($seen)", seen >= 20)
        assertEquals("text styles that draw the UI face with its contextual alternates ON", emptyList<String>(), offenders)
        // The tabular spelling keeps the rule off as well.
        assertTrue(app.getvela.wallet.core.designsystem.tokens.VelaFontFeaturesTabular.split(",").map { it.trim() }.containsAll(listOf("tnum", VelaFontFeatures)))
    }

    @Test
    fun `the theme hands that scale to every screen, and the text fields carry it`() {
        assertTrue("the theme's MaterialTheme takes the scale", source("core/designsystem/theme/VelaTheme.kt").contains("typography = VelaTypography,"))
        for (field in listOf("core/designsystem/components/VelaTextField.kt", "feature/contacts/components/ContactsSearchField.kt")) {
            assertTrue("$field types in the face with its alternates on", source(field).contains("fontFeatureSettings = VelaFontFeatures"))
        }
    }
}
