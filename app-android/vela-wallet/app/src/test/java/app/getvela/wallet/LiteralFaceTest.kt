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

    @Test
    fun `the theme hands that scale to every screen, and the text fields carry it`() {
        assertTrue("the theme's MaterialTheme takes the scale", source("core/designsystem/theme/VelaTheme.kt").contains("typography = VelaTypography,"))
        for (field in listOf("core/designsystem/components/VelaTextField.kt", "feature/contacts/components/ContactsSearchField.kt")) {
            assertTrue("$field types in the face with its alternates on", source(field).contains("fontFeatureSettings = VelaFontFeatures"))
        }
    }
}
