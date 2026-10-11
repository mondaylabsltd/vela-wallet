package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.times
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaLeading
import app.getvela.wallet.core.designsystem.tokens.VelaTextSize
import app.getvela.wallet.core.format.DateFormatKey
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.TimeFormatKey
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.trustedsigner.SignerPageChecks
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.vela_core_uniffi.SignerIntegrityLine
import uniffi.vela_core_uniffi.SignerIntegrityState

/**
 * The integration's note 14, in this shell's own text stack: a twelve-hour
 * CJK checked time never splits, and the joiner that holds it together draws
 * as nothing.
 *
 * The core writes zh 「下⁠午 2:32」 as `下 U+2060 午 U+00A0 2:32` — U+00A0 alone
 * does not bind two Han characters, so the line used to be able to break
 * between 下 and 午. The integrity line (the string the core fills, drawn in
 * the line's own face and size) is laid out here at EVERY width the moment
 * itself fits in, up to 420 dp, a pixel at a time, and at none of them may a
 * line end inside the moment (narrower than the moment, any text has to
 * break: that is not a width a phone has). And `下⁠午` must be exactly as wide as `下午`: a joiner that drew a
 * box (tofu) or took any room would show.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.CheckedTimeUnbreakableTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class CheckedTimeUnbreakableTest {

    @get:Rule
    val compose = createComposeRule()

    private fun strings(lang: String) = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize(lang) }
    }

    /** The integrity line's own text style ([app.getvela.wallet.feature.settings.components.IntegrityLine]). */
    private val style = TextStyle(fontFamily = VelaFontFamily, fontSize = VelaTextSize.sm, lineHeight = VelaLeading.normal * VelaTextSize.sm)

    private fun line(lang: String): String {
        val zone = java.util.TimeZone.getTimeZone("UTC")
        val h12 = Formats(date = DateFormatKey.MdySlash, time = TimeFormatKey.H12)
        val now = java.time.Instant.parse("2026-10-09T18:00:00Z").toEpochMilli()
        val checked = SignerIntegrityLine(
            SignerIntegrityState.MATCHES, "0ba8ee8c", java.time.Instant.parse("2026-10-09T14:32:00Z").toEpochMilli().toULong(),
            "componentsUi.signing.integrity.matches", true,
        )
        return SignerPageChecks.words(checked, strings(lang), now, h12, zone)
    }

    @Test
    fun theMomentNeverSplitsAtAnyWidthAndTheJoinerDrawsAsNothing() {
        lateinit var measurer: TextMeasurer
        val zh = strings("zh")
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides zh) {
                VelaTheme(darkTheme = false) { measurer = rememberTextMeasurer() }
            }
        }
        compose.waitForIdle()
        val density = compose.density.density

        for ((lang, moment) in listOf("zh" to "下⁠午 2:32", "ja" to "午⁠後 2:32")) {
            val text = line(lang)
            val start = text.indexOf(moment)
            assertTrue("$lang: the core's moment is in the line: $text", start >= 0)
            val end = start + moment.length - 1
            var wrapped = 0
            var widths = 0
            // The narrowest line the moment fits on whole (about 60 dp): from there up.
            val narrowest = measurer.measure(moment, style).size.width
            assertTrue("$lang: the moment is a few characters wide, not a line: ${narrowest / density}dp", narrowest < 100 * density)
            for (px in narrowest..(420 * density).toInt()) {
                val layout = measurer.measure(text, style, constraints = Constraints(maxWidth = px))
                widths += 1
                if (layout.lineCount > 1) wrapped += 1
                assertEquals(
                    "$lang: the moment splits across lines at ${px / density}dp: lines ${(0 until layout.lineCount).map { text.substring(layout.getLineStart(it), layout.getLineEnd(it)) }}",
                    layout.getLineForOffset(start), layout.getLineForOffset(end),
                )
            }
            assertTrue("$lang: the line did wrap somewhere, so the check was not of one-line layouts only", wrapped > widths / 2)
            // The control: the same line WITHOUT the joiner does split there, at
            // some width — so the check above is of the joiner, not of a text
            // stack that would never break between two Han characters anyway.
            val loose = text.replace("\u2060", "")
            val looseStart = loose.indexOf(moment.replace("\u2060", ""))
            val split = (narrowest..(420 * density).toInt()).count { px ->
                val layout = measurer.measure(loose, style, constraints = Constraints(maxWidth = px))
                layout.getLineForOffset(looseStart) != layout.getLineForOffset(looseStart + 1)
            }
            assertTrue("$lang: without the joiner the moment splits at some width (the check has teeth)", split > 0)
            android.util.Log.i("CheckedTime", "$lang: $widths widths, $wrapped of them wrapped, the moment whole in every one; without the joiner it splits at $split of them")
        }

        // The joiner draws as nothing: it takes no room, so it shows no box.
        fun width(text: String) = measurer.measure(text, style).size.width
        assertEquals("zh: U+2060 takes room (a box?)", width("下午"), width("下⁠午"))
        assertEquals("ja: U+2060 takes room (a box?)", width("午後"), width("午⁠後"))
        assertEquals(width("检查于 下午 2:32"), width("检查于 下⁠午 2:32"))
        // …while a character the font really lacks WOULD take room (the check can fail).
        assertTrue(width("下�午") > width("下午"))
    }
}
