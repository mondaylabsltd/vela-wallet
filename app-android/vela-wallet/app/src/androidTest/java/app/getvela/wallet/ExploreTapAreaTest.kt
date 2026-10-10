package app.getvela.wallet

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.designsystem.tokens.VelaSpacing
import app.getvela.wallet.feature.explore.SiteModel
import app.getvela.wallet.feature.explore.TileModel
import app.getvela.wallet.feature.explore.components.SiteRow
import app.getvela.wallet.feature.explore.components.SiteTile
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * PR 3 final note F9 (check). iOS's favourites tile took taps only on its
 * mark and its words — a tap on the padding or between the two did nothing
 * (`contentShape` outside the plain button style). On Android a tile and a
 * row are each ONE clickable that sits outside their own padding, so the
 * whole of what is drawn — corners, padding, the gap between the mark and
 * the label, the empty stretch between a row's words and its edge — opens
 * the site. Held here by tapping those very places.
 *
 * Laid out as the start page lays them out: a four-column row of tiles, each
 * a quarter of the width, and rows the full width.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ExploreTapAreaTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ExploreTapAreaTest {

    @get:Rule
    val compose = createComposeRule()

    private fun site(id: String, name: String) = SiteModel(id = id, name = name, host = "$id.example", letter = name.take(1), tint = Color(0xFF2E9E7E))

    @Test
    fun aTileAndARowOpenTheirSiteFromAnywhereInsideThem() {
        val opened = mutableListOf<String>()
        compose.setContent {
            VelaTheme(darkTheme = false) {
                Column(Modifier.width(360.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(vertical = VelaSpacing.md),
                        horizontalArrangement = Arrangement.spacedBy(VelaSpacing.md),
                    ) {
                        SiteTile(TileModel.Site(site("uni", "Uniswap")), onOpen = { opened += it }, modifier = Modifier.weight(1f).testTag("tile"))
                        SiteTile(TileModel.Site(site("aave", "Aave")), onOpen = { opened += it }, modifier = Modifier.weight(1f).testTag("tile-2"))
                        SiteTile(TileModel.Add("Add"), onOpen = { opened += it }, modifier = Modifier.weight(1f).testTag("tile-add"))
                        Box(Modifier.weight(1f))
                    }
                    SiteRow(site("poly", "Polymarket"), onOpen = { opened += it }, modifier = Modifier.testTag("row"))
                }
            }
        }
        compose.waitForIdle()

        fun tapAll(tag: String, expected: String) {
            val node = compose.onNodeWithTag(tag)
            val size = node.fetchSemanticsNode().size
            val (w, h) = size.width.toFloat() to size.height.toFloat()
            assertTrue("$tag is laid out: $size", w > 40f && h > 40f)
            val places = mapOf(
                "the top left corner" to Offset(2f, 2f),
                "the top right corner" to Offset(w - 3f, 2f),
                "the bottom left corner" to Offset(2f, h - 3f),
                "the bottom right corner" to Offset(w - 3f, h - 3f),
                "the middle" to Offset(w / 2, h / 2),
                "the left edge, half way down" to Offset(2f, h / 2),
                "the right edge, half way down" to Offset(w - 3f, h / 2),
                "the top edge, in the middle" to Offset(w / 2, 1f),
                "the bottom edge, in the middle" to Offset(w / 2, h - 2f),
            )
            for ((where, at) in places) {
                opened.clear()
                node.performTouchInput { click(at) }
                compose.waitForIdle()
                assertEquals("$tag tapped at $where ($at of ${w}×$h)", listOf(expected), opened)
            }
        }
        tapAll("tile", "uni")
        tapAll("tile-2", "aave")
        tapAll("tile-add", "add")
        tapAll("row", "poly")

        // The gap between a tile's mark and its label, exactly: just above the label's own top.
        val tile = compose.onNodeWithTag("tile").fetchSemanticsNode()
        val label = compose.onNodeWithText("Uniswap", useUnmergedTree = true).fetchSemanticsNode()
        val gapY = label.positionInRoot.y - tile.positionInRoot.y - 4f
        assertTrue("there is a gap above the label ($gapY)", gapY > 0f)
        opened.clear()
        compose.onNodeWithTag("tile").performTouchInput { click(Offset(tile.size.width / 2f, gapY)) }
        compose.waitForIdle()
        assertEquals("the gap between the mark and the label", listOf("uni"), opened)

        // A row: the empty stretch right of its words, and its vertical padding.
        val row = compose.onNodeWithTag("row").fetchSemanticsNode()
        val words = compose.onNodeWithText("Polymarket", useUnmergedTree = true).fetchSemanticsNode()
        val right = words.positionInRoot.x - row.positionInRoot.x + words.size.width + 24f
        assertTrue("there is room right of the words ($right of ${row.size.width})", right < row.size.width - 4f)
        opened.clear()
        compose.onNodeWithTag("row").performTouchInput { click(Offset(right, row.size.height / 2f)) }
        compose.waitForIdle()
        assertEquals("the empty stretch right of the words", listOf("poly"), opened)
        // The whole width is the row's, as on the start page.
        assertEquals(compose.onNodeWithTag("row").fetchSemanticsNode().size.width, with(compose.density) { 360.dp.roundToPx() })
    }
}
