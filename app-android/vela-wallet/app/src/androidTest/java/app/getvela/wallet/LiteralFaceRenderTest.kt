package app.getvela.wallet

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.text.BasicText
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.unit.sp
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaFontFamily
import app.getvela.wallet.core.designsystem.tokens.VelaFontFeatures
import app.getvela.wallet.core.designsystem.tokens.VelaFontFeaturesTabular
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The final round's "0×" check, in pixels. Plus Jakarta Sans — bundled byte
 * for byte on every shell — has one contextual-alternates rule: after a digit
 * `x` becomes `×`. So "0x100" in the no-P-256 hint and every "0x14fB…"
 * address were DRAWN "0×100" and "0×14fB…" wherever the rule was on (found
 * on the iPhone, 2026-10-09).
 *
 * Each sample is drawn as written and with a real `×` typed in its place, in
 * every way this app sets text in the UI face: through the theme (`Text`),
 * with a style built in a screen ([VelaFontFeatures]), and with the tabular
 * spelling ([VelaFontFeaturesTabular]). If the rule were on, the two
 * drawings would be the same pixels — and in the face left to itself (no
 * features said) they ARE, which is what makes the comparison a proof.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.LiteralFaceRenderTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class LiteralFaceRenderTest {

    @get:Rule
    val compose = createComposeRule()

    private val samples = listOf(
        // The no-P-256 hint's own words (`settingsModals.addNetwork.noP256Hint`).
        "EIP-7951 / RIP-7212 at 0x100",
        // An address row, short and whole.
        "0x14fB1f…D1eA5c",
        "0x14fB1f6e2C5A8D3b9E7f4A1c6B2d8E5f3AD1eA5c",
        // A transaction hash in a sentence, and a URL as the browser bar types it.
        "tx 0x9f3c…7a10 on Gnosis",
        "etherscan.io/address/0x14fB1f",
    )

    private fun pixels(tag: String): List<Int> {
        val map = compose.onNodeWithTag(tag).captureToImage().toPixelMap()
        return listOf(map.width, map.height) + map.buffer.toList()
    }

    @Test
    fun aHexPrefixIsDrawnAsWrittenInEveryWayTheAppSetsText() {
        val bare = TextStyle(color = Color.Black, fontFamily = VelaFontFamily, fontSize = 22.sp)
        val literal = bare.copy(fontFeatureSettings = VelaFontFeatures)
        val tabular = bare.copy(fontFeatureSettings = VelaFontFeaturesTabular)
        // One sample on screen at a time: a node pushed off the screen cannot be captured.
        var shown by mutableStateOf(0)
        compose.setContent {
            VelaTheme(darkTheme = false) {
                Column(Modifier.background(Color.White)) {
                    samples.withIndex().filter { it.index == shown }.forEach { (i, written) ->
                        val times = written.replace("0x", "0×")
                        // Through the theme: a Text that names a family and a size, as most of the app does.
                        Text(written, color = Color.Black, fontFamily = VelaFontFamily, fontSize = 22.sp, modifier = Modifier.testTag("theme-x-$i"))
                        Text(times, color = Color.Black, fontFamily = VelaFontFamily, fontSize = 22.sp, modifier = Modifier.testTag("theme-t-$i"))
                        BasicText(written, style = literal, modifier = Modifier.testTag("literal-x-$i"))
                        BasicText(times, style = literal, modifier = Modifier.testTag("literal-t-$i"))
                        BasicText(written, style = tabular, modifier = Modifier.testTag("tabular-x-$i"))
                        BasicText(times, style = tabular, modifier = Modifier.testTag("tabular-t-$i"))
                        // The face left to itself.
                        BasicText(written, style = bare, modifier = Modifier.testTag("bare-x-$i"))
                        BasicText(times, style = bare, modifier = Modifier.testTag("bare-t-$i"))
                    }
                }
            }
        }
        samples.forEachIndexed { i, written ->
            shown = i
            compose.waitForIdle()
            // The proof that this test can see the substitution: with no
            // features said, "0x" IS drawn as "0×".
            assertTrue("the face by itself draws \"$written\" with a ×", pixels("bare-t-$i") == pixels("bare-x-$i"))
            for (way in listOf("theme", "literal", "tabular")) {
                assertFalse("$way: \"$written\" was drawn with a × for its x", pixels("$way-t-$i") == pixels("$way-x-$i"))
            }
        }
    }
}
