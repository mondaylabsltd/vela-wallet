package app.getvela.wallet

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.doubleClick
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pinch
import androidx.compose.ui.test.swipeDown
import androidx.compose.ui.test.swipeLeft
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.unit.dp
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.settings.FeedbackModel
import app.getvela.wallet.feature.settings.components.FeedbackScreenshotsSection
import app.getvela.wallet.feature.settings.components.ScreenshotTileView
import app.getvela.wallet.feature.settings.components.ScreenshotViewer
import app.getvela.wallet.feature.settings.components.ViewerImage
import java.io.ByteArrayOutputStream
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Tap a screenshot to see it large (spec 078 §C), on a real device's input
 * pipeline: a TAP opens a tile, a scroll or a sideways drag that starts on a
 * tile (or on its ✕) opens and deletes nothing, a processing tile and a
 * sending form open nothing; in the viewer a pinch zooms (and a zoomed drag
 * does not page), a double tap resets, a swipe pages, remove shows the next
 * image and closes on the last, a pull down and Back close it.
 *
 * ```bash
 * adb shell am instrument -w -e class app.getvela.wallet.ScreenshotViewerDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ScreenshotViewerDeviceTest {

    @get:Rule
    val compose = createComposeRule()

    private val model = FeedbackModel(
        title = "Send feedback",
        subtitle = "",
        placeholder = "",
        addSteps = "",
        previewToggle = "",
        previewLines = emptyList(),
        consent = "",
        send = "Send",
        githubLink = "",
        screenshotsLabel = "Screenshots",
        addScreenshots = "Add screenshots",
        screenshotsHint = "Optional · up to 5",
        screenshotsPublic = "Screenshots are public on GitHub.",
        removeScreenshot = "Remove screenshot {{index}}",
        viewScreenshot = "View screenshot {{index}}",
        closeViewer = "Close",
        removeFromViewer = "Remove",
    )

    private fun picture(width: Int, height: Int, hue: Int): Bitmap {
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(Color.rgb(240, 240 - hue, 230))
        val paint = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = Color.rgb(hue, 80, 200 - hue / 2) }
        for (i in 0 until 12) canvas.drawCircle((i * 97 % width).toFloat(), (i * 61 % height).toFloat(), 30f + i * 5, paint)
        return bitmap
    }

    private fun jpeg(bitmap: Bitmap): ByteArray = ByteArrayOutputStream().use { bitmap.compress(Bitmap.CompressFormat.JPEG, 85, it); it.toByteArray() }

    // --- the tiles ---------------------------------------------------------

    private class TileHarness {
        var opened: Long? = null
        val removed = mutableListOf<Long>()
        var scrolled = 0
    }

    /** The section inside a column that scrolls like the sheet does, with room below to scroll into. */
    private fun tiles(vararg ready: Boolean, enabled: Boolean = true): TileHarness {
        val harness = TileHarness()
        val thumbs = ready.mapIndexed { i, r -> ScreenshotTileView(i.toLong(), if (r) picture(144, 144, i * 60) else null, r) }
        compose.setContent {
            VelaTheme(darkTheme = false) {
                val scroll = rememberScrollState()
                harness.scrolled = scroll.value
                Column(modifier = Modifier.fillMaxSize().verticalScroll(scroll).padding(16.dp)) {
                    Spacer(modifier = Modifier.height(200.dp))
                    FeedbackScreenshotsSection(
                        model = model,
                        tiles = thumbs.filterNot { it.id in harness.removed },
                        notice = null,
                        onAdd = {},
                        onRemove = { harness.removed += it },
                        enabled = enabled,
                        onOpen = { harness.opened = it },
                    )
                    Spacer(modifier = Modifier.height(2000.dp))
                }
            }
        }
        compose.waitForIdle()
        return harness
    }

    @Test
    fun aTapOnAPreparedTileOpensIt() {
        val harness = tiles(true, true, true)
        // TalkBack reaches it as a button with a click action, under its label.
        compose.onNodeWithContentDescription("View screenshot 2").assertHasClickAction()
        compose.onNodeWithContentDescription("Remove screenshot 2").assertHasClickAction()
        compose.onNodeWithContentDescription("View screenshot 2").performClick()
        compose.waitForIdle()
        assertEquals(1L, harness.opened)
    }

    @Test
    fun aScrollThatStartsOnATileDoesNotOpenIt() {
        val harness = tiles(true, true)
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput {
            swipeUp(startY = centerY, endY = centerY - 500f, durationMillis = 300)
        }
        compose.waitForIdle()
        assertTrue("the column scrolled — it really was a scroll", harness.scrolled > 0)
        assertNull("a scroll never opens a screenshot", harness.opened)
    }

    @Test
    fun aSidewaysDragInsideATileDoesNotOpenIt() {
        val harness = tiles(true, true)
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput {
            // Starts and ends inside the tile — Compose's clickable alone would click.
            down(Offset(width * 0.2f, centerY))
            moveBy(Offset(width * 0.3f, 0f))
            moveBy(Offset(width * 0.3f, 0f))
            up()
        }
        compose.waitForIdle()
        assertNull(harness.opened)
    }

    @Test
    fun aScrollThatStartsOnTheRemoveBadgeDeletesNothing() {
        val harness = tiles(true, true)
        compose.onNodeWithContentDescription("Remove screenshot 1").performTouchInput {
            swipeUp(startY = centerY, endY = centerY - 500f, durationMillis = 300)
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        assertTrue(harness.removed.isEmpty())
        compose.onNodeWithContentDescription("Remove screenshot 1").performTouchInput {
            down(Offset(width * 0.3f, height * 0.5f))
            moveBy(Offset(width * 0.5f, 0f))
            up()
        }
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        assertTrue("a sideways drag on the ✕ deletes nothing", harness.removed.isEmpty())
        // A real tap still removes (after the tile's exit fade).
        compose.onNodeWithContentDescription("Remove screenshot 1").performClick()
        compose.mainClock.advanceTimeBy(500)
        compose.waitForIdle()
        assertEquals(listOf(0L), harness.removed)
    }

    @Test
    fun aTileStillBeingPreparedDoesNotOpen() {
        val harness = tiles(true, false)
        compose.onNodeWithContentDescription("View screenshot 2").performClick()
        compose.waitForIdle()
        assertNull(harness.opened)
    }

    @Test
    fun nothingOpensWhileSending() {
        val harness = tiles(true, true, enabled = false)
        compose.onNodeWithContentDescription("View screenshot 1").performClick()
        compose.waitForIdle()
        assertNull(harness.opened)
    }

    // --- the viewer --------------------------------------------------------

    private class ViewerHarness {
        var closed = 0
        val images = mutableStateListOf<ViewerImage>()
    }

    private fun viewer(count: Int, start: Int = 0): ViewerHarness {
        val harness = ViewerHarness()
        repeat(count) { i ->
            val w = if (i % 2 == 0) 1080 else 1920
            val h = if (i % 2 == 0) 1920 else 1080
            harness.images += ViewerImage(i.toLong(), jpeg(picture(w, h, i * 60)), w, h, "View screenshot ${i + 1}")
        }
        compose.setContent {
            VelaTheme(darkTheme = false) {
                var open by remember { mutableStateOf(true) }
                if (open) {
                    ScreenshotViewer(
                        images = harness.images.toList(),
                        startId = start.toLong(),
                        closeLabel = "Close",
                        removeLabel = "Remove",
                        onRemove = { id -> harness.images.removeAll { it.id == id } },
                        onClosed = {
                            harness.closed++
                            open = false
                        },
                    )
                }
            }
        }
        compose.waitForIdle()
        return harness
    }

    @Test
    fun theViewerOpensOnTheTappedImageAndSwipesPage() {
        viewer(3, start = 1)
        compose.onNodeWithText("2 / 3").assertIsDisplayed()
        compose.onNodeWithContentDescription("View screenshot 2").performTouchInput { swipeLeft() }
        compose.waitForIdle()
        compose.onNodeWithText("3 / 3").assertIsDisplayed()
    }

    @Test
    fun aPinchZoomsAndAZoomedDragPansInsteadOfPaging() {
        viewer(3, start = 0)
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput {
            pinch(
                start0 = center - Offset(60f, 0f),
                end0 = center - Offset(360f, 0f),
                start1 = center + Offset(60f, 0f),
                end1 = center + Offset(360f, 0f),
                durationMillis = 400,
            )
        }
        compose.waitForIdle()
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput { swipeLeft() }
        compose.waitForIdle()
        compose.onNodeWithText("1 / 3").assertIsDisplayed()
        // Double tap resets the zoom; now a swipe pages.
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput { doubleClick(center) }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput { swipeLeft() }
        compose.waitForIdle()
        compose.onNodeWithText("2 / 3").assertIsDisplayed()
    }

    @Test
    fun removeShowsTheNextThenThePreviousAndClosesOnTheOnlyOne() {
        val harness = viewer(3, start = 1)
        // Middle: the next one takes its place.
        compose.onNodeWithText("Remove").performClick()
        compose.waitForIdle()
        assertEquals(listOf(0L, 2L), harness.images.map { it.id })
        compose.onNodeWithText("2 / 2").assertIsDisplayed()
        compose.onNodeWithContentDescription("View screenshot 3").assertIsDisplayed()
        // Last: the previous one.
        compose.onNodeWithText("Remove").performClick()
        compose.waitForIdle()
        assertEquals(listOf(0L), harness.images.map { it.id })
        compose.onNodeWithContentDescription("View screenshot 1").assertIsDisplayed()
        // The only one: removed, and the viewer closes.
        compose.onNodeWithText("Remove").performClick()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertTrue(harness.images.isEmpty())
        assertEquals(1, harness.closed)
    }

    @Test
    fun aPullDownCloses() {
        val harness = viewer(2)
        compose.onNodeWithContentDescription("View screenshot 1").performTouchInput {
            swipeDown(startY = centerY - 200f, endY = centerY + 700f, durationMillis = 250)
        }
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertEquals(1, harness.closed)
    }

    @Test
    fun systemBackCloses() {
        val harness = viewer(2)
        Espresso.pressBack()
        compose.mainClock.advanceTimeBy(600)
        compose.waitForIdle()
        assertEquals(1, harness.closed)
    }
}
