package app.getvela.wallet

import app.getvela.wallet.feature.settings.SettingsPage
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Device pass 2026-10-09, Settings' pushed pages: 设置 → 高级 → 网络 opened
 * part-way down — the home and every pushed page shared ONE scroll, so a page
 * opened at the offset the home had been scrolled to — and the system Back on
 * it left Settings for 钱包, because nothing inside Settings answered Back.
 *
 * The rules are [SettingsPage.back] and [SettingsPage.keepsItsPlace]; the
 * last two tests read the source (as BrowserPageClipTest does) to keep
 * SettingsScreen.kt wired to them — there is no JVM Compose harness here.
 */
class SettingsPagesTest {

    private val pushed = SettingsPage.entries - SettingsPage.Home

    @Test
    fun `Back from every pushed page is the Settings home, and the home's Back is not Settings'`() {
        pushed.forEach { assertEquals("Back from $it", SettingsPage.Home, it.back) }
        assertNull(SettingsPage.Home.back)
    }

    @Test
    fun `only the home keeps its place, every pushed page opens at its top`() {
        assertTrue(SettingsPage.Home.keepsItsPlace)
        pushed.forEach { assertFalse("$it opens at its top", it.keepsItsPlace) }
    }

    private val screen: List<String> by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/settings/SettingsScreen.kt")
            .readLines()
            .map { it.substringBefore("//").trim() }
            .filter { it.isNotEmpty() && !it.startsWith("*") && !it.startsWith("/*") }
    }

    /** `SettingsScreen`'s own body: from its signature to the next top-level function. */
    private val screenBody: List<String> by lazy {
        val start = screen.indexOfFirst { it.startsWith("fun SettingsScreen(") }
        assertTrue("SettingsScreen is in SettingsScreen.kt", start >= 0)
        val end = screen.drop(start + 1).indexOfFirst { it.startsWith("private fun ") || it.startsWith("fun ") || it.startsWith("internal fun ") }
        screen.subList(start, if (end < 0) screen.size else start + 1 + end)
    }

    @Test
    fun `the page column scrolls with a state per page, not one shared`() {
        assertTrue("a pushed page's scroll is keyed on the page", screenBody.any { it == "val pageScroll = key(page) { rememberScrollState() }" })
        assertTrue(
            "the column picks the home's or the page's scroll",
            screenBody.any { it == ".verticalScroll(if (page.keepsItsPlace) homeScroll else pageScroll)" },
        )
        assertFalse(
            "one scroll remembered for every page is the bug",
            screenBody.any { it.startsWith(".verticalScroll(rememberScrollState())") },
        )
    }

    @Test
    fun `the system Back and the page's own back both follow SettingsPage_back`() {
        assertTrue("Back is answered inside Settings on a pushed page", screen.any { it == "BackHandler(enabled = page.back != null) { page.back?.let { page = it } }" })
        assertTrue("the ‹ takes the same step", screen.any { it == "onBack = { page.back?.let { page = it } }," })
    }
}
