package app.getvela.wallet

import app.getvela.wallet.core.diagnostics.ScreenshotTray
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.Drag
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.UNZOOMED
import app.getvela.wallet.core.diagnostics.ScreenshotViewerRules.Zoom
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Tap a screenshot to see it large (spec 078 §C) — the rules the viewer and
 * the tiles follow, held without a screen: which tile opens and where, paging,
 * remove from the viewer (middle / last / only) and where focus returns, that a
 * processing tile never opens and nothing opens while sending, the pull-down's
 * numbers, the zoom maths, and the tap guard that keeps a scroll from opening
 * or deleting a screenshot.
 */
class ScreenshotViewerRulesTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)

    @After
    fun tearDown() = scope.cancel()

    /** Three prepared tiles, "jpeg:a" … — the tray the sheet holds, in tile order. */
    private fun trayOf(vararg names: String) =
        ScreenshotTray<String, String>(scope) { "jpeg:$it" }.apply { add(names.toList()) }

    private fun ScreenshotTray<String, String>.ids() = tiles.value.map { it.id }

    /** What the viewer pages through, as the sheet builds it. */
    private fun ScreenshotTray<String, String>.shown() = ScreenshotViewerRules.viewable(tiles.value).map { it.id }

    @Test
    fun `a prepared tile opens the viewer on itself`() {
        val tray = trayOf("a", "b", "c")
        val (a, b, c) = tray.ids()
        assertTrue(ScreenshotViewerRules.opens(tray.tiles.value, b, sending = false))
        // The viewer starts on the tapped image: index 1 of three, "2 / 3".
        val start = tray.shown().indexOf(b)
        assertEquals(1, start)
        assertEquals("2 / 3", ScreenshotViewerRules.counter(start, tray.shown().size))
        assertEquals(listOf(a, b, c), tray.shown())
        // A tile that is not (or no longer) there opens nothing.
        assertFalse(ScreenshotViewerRules.opens(tray.tiles.value, 999L, sending = false))
    }

    @Test
    fun `paging walks the images in tile order and the counter follows`() {
        val tray = trayOf("a", "b", "c")
        val shown = tray.shown()
        assertEquals(listOf("jpeg:a", "jpeg:b", "jpeg:c"), ScreenshotViewerRules.viewable(tray.tiles.value).map { it.ready })
        assertEquals(listOf("1 / 3", "2 / 3", "3 / 3"), shown.indices.map { ScreenshotViewerRules.counter(it, shown.size) })
        // One image: no counter at all.
        assertNull(ScreenshotViewerRules.counter(0, 1))
    }

    @Test
    fun `a sideways drag pages, a drag down dismisses, and zoomed every drag pans`() {
        val slop = 8f
        assertEquals(Drag.Undecided, ScreenshotViewerRules.takesDrag(5f, 5f, zoomed = false, slop = slop))
        assertEquals(Drag.Page, ScreenshotViewerRules.takesDrag(-40f, 10f, zoomed = false, slop = slop))
        assertEquals(Drag.Page, ScreenshotViewerRules.takesDrag(40f, -10f, zoomed = false, slop = slop))
        assertEquals(Drag.Dismiss, ScreenshotViewerRules.takesDrag(4f, 30f, zoomed = false, slop = slop))
        // Up while not zoomed: nothing — and it must not page either.
        assertEquals(Drag.None, ScreenshotViewerRules.takesDrag(4f, -30f, zoomed = false, slop = slop))
        // Zoomed: sideways and down both pan; neither pages nor dismisses.
        assertEquals(Drag.Pan, ScreenshotViewerRules.takesDrag(-40f, 0f, zoomed = true, slop = slop))
        assertEquals(Drag.Pan, ScreenshotViewerRules.takesDrag(0f, 60f, zoomed = true, slop = slop))
    }

    @Test
    fun `removing the middle image shows the next and focus returns to it`() {
        val tray = trayOf("a", "b", "c")
        val (_, b, c) = tray.ids()
        val shown = tray.shown()
        assertEquals(c, ScreenshotViewerRules.shownAfterRemove(shown, b))
        // It was opened on b: focus goes to the tile that took b's place.
        assertEquals(c, ScreenshotViewerRules.returnAfterRemove(tray.ids(), returnTo = b, removed = b))
        tray.remove(b)
        assertEquals(listOf("jpeg:a", "jpeg:c"), tray.ready())
        assertEquals(1, tray.shown().indexOf(c))
        assertEquals("2 / 2", ScreenshotViewerRules.counter(tray.shown().indexOf(c), tray.shown().size))
    }

    @Test
    fun `removing the last image shows the previous one`() {
        val tray = trayOf("a", "b", "c")
        val (a, b, c) = tray.ids()
        assertEquals(b, ScreenshotViewerRules.shownAfterRemove(tray.shown(), c))
        // Opened on a, paged to c, removed c: focus still goes back to a.
        assertEquals(a, ScreenshotViewerRules.returnAfterRemove(tray.ids(), returnTo = a, removed = c))
        // Opened on c and removed it: no tile after it — the add target (null).
        assertNull(ScreenshotViewerRules.returnAfterRemove(tray.ids(), returnTo = c, removed = c))
        tray.remove(c)
        assertEquals(listOf("jpeg:a", "jpeg:b"), tray.ready())
    }

    @Test
    fun `removing the only image closes the viewer and focus goes to the add target`() {
        val tray = trayOf("a")
        val (a) = tray.ids()
        assertNull("nothing left to show: the viewer closes", ScreenshotViewerRules.shownAfterRemove(tray.shown(), a))
        assertNull(ScreenshotViewerRules.returnAfterRemove(tray.ids(), returnTo = a, removed = a))
        tray.remove(a)
        assertTrue(tray.tiles.value.isEmpty())
    }

    @Test
    fun `a tile still being prepared does not open and is not paged to`() {
        val gate = CompletableDeferred<String>()
        val tray = ScreenshotTray<String, String>(scope) { if (it == "slow") gate.await() else "jpeg:$it" }
        tray.add(listOf("a", "slow", "c"))
        val (a, slow, c) = tray.ids()
        assertFalse(ScreenshotViewerRules.opens(tray.tiles.value, slow, sending = false))
        assertTrue(ScreenshotViewerRules.opens(tray.tiles.value, c, sending = false))
        assertEquals(listOf(a, c), tray.shown())
        // Removing c (the last shown) from the viewer shows a — the processing tile is skipped…
        assertEquals(a, ScreenshotViewerRules.shownAfterRemove(tray.shown(), c))
        // …and once prepared it opens, and is paged to in its place.
        gate.complete("jpeg:slow")
        assertTrue(ScreenshotViewerRules.opens(tray.tiles.value, slow, sending = false))
        assertEquals(listOf(a, slow, c), tray.shown())
    }

    @Test
    fun `nothing opens while the report is sending`() {
        val tray = trayOf("a", "b")
        tray.ids().forEach { id -> assertFalse(ScreenshotViewerRules.opens(tray.tiles.value, id, sending = true)) }
    }

    @Test
    fun `a pull down closes past a fifth of the height or on a flick, else springs back`() {
        val height = 2000f
        val flick = 1500f // px/s: 500 dp/s at 3×
        val minDistance = 48f
        assertFalse(ScreenshotViewerRules.shouldDismiss(399f, height, 0f, flick, minDistance))
        assertTrue(ScreenshotViewerRules.shouldDismiss(401f, height, 0f, flick, minDistance))
        // A fast flick closes early, once it has actually moved.
        assertTrue(ScreenshotViewerRules.shouldDismiss(60f, height, 3000f, flick, minDistance))
        assertFalse(ScreenshotViewerRules.shouldDismiss(20f, height, 3000f, flick, minDistance))
        // Pushed back up: never.
        assertFalse(ScreenshotViewerRules.shouldDismiss(-50f, height, 3000f, flick, minDistance))
        // The black gives way as the picture follows the finger, and is gone well before the bottom.
        assertEquals(1f, ScreenshotViewerRules.backdrop(0f, height), 0f)
        assertTrue(ScreenshotViewerRules.backdrop(400f, height) < 1f)
        assertEquals(0f, ScreenshotViewerRules.backdrop(1200f, height), 0f)
        // Up is rubber-banded; down follows 1:1.
        assertEquals(300f, ScreenshotViewerRules.dismissOffset(300f, height), 0f)
        assertTrue(ScreenshotViewerRules.dismissOffset(-300f, height) in -300f..0f)
    }

    @Test
    fun `pinch and double tap zoom about the finger and never leave a gap at a side`() {
        val box = 1080f to 1800f
        val fitted = ScreenshotViewerRules.fitSize(1920f, 1080f, box.first, box.second)
        assertEquals(1080f, fitted.first, 0.01f)
        assertEquals(607.5f, fitted.second, 0.01f)
        // Double tap: 2× about the tap, then back to 1.
        val zoomed = ScreenshotViewerRules.doubleTapZoom(UNZOOMED, 200f, 0f, fitted.first, fitted.second, box.first, box.second)
        assertEquals(2f, zoomed.scale, 0f)
        assertTrue(zoomed.zoomed)
        assertEquals(UNZOOMED, ScreenshotViewerRules.doubleTapZoom(zoomed, 0f, 0f, fitted.first, fitted.second, box.first, box.second))
        // The point under the finger stays put: at 2× about x=200 the picture moves left by 200.
        assertEquals(-200f, ScreenshotViewerRules.zoomAbout(UNZOOMED, 2f, 200f, 0f).x, 0.01f)
        // Panned too far: held at the edge (the picture is 2160 wide in a 1080 box → ±540).
        val clamped = ScreenshotViewerRules.clampPan(Zoom(2f, 5000f, 5000f), fitted.first, fitted.second, box.first, box.second)
        assertEquals(540f, clamped.x, 0.01f)
        assertEquals(0f, clamped.y, 0.01f) // 1215 tall in an 1800 box: no vertical play
        assertEquals(ScreenshotViewerRules.MAX_ZOOM, ScreenshotViewerRules.clampScale(9f), 0f)
        assertEquals(ScreenshotViewerRules.MIN_ZOOM, ScreenshotViewerRules.clampScale(0.3f), 0f)
    }

    @Test
    fun `only a real tap opens or removes - a press that turned into a scroll does not`() {
        val guard = ScreenshotViewerRules.TapSlop(slop = 24f)
        // A still press: a tap.
        guard.down(100f, 100f)
        guard.move(104f, 98f)
        assertTrue(guard.accept())
        guard.end()
        // A scroll that began on the tile (or the badge): not a tap, even if it ends inside.
        guard.down(100f, 100f)
        guard.move(100f, 160f)
        guard.move(100f, 110f)
        assertFalse(guard.accept())
        guard.end()
        // A sideways drag within the tile: not a tap either.
        guard.down(10f, 50f)
        guard.move(60f, 52f)
        assertFalse(guard.accept())
        guard.end()
        // The pointer was taken (a second finger, a cancel): not a tap.
        guard.down(10f, 10f)
        guard.cancel()
        assertFalse(guard.accept())
        guard.end()
        // After a spoiled gesture has ended, a click with no finger behind it
        // (TalkBack's double tap, a keyboard's Enter) is never refused.
        assertTrue(guard.accept())
    }
}
