package app.getvela.wallet

import app.getvela.wallet.feature.settings.components.TextScaleTrack
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The text-size slider's one geometry: the dots and the thumb on the same
 * centres, and a touch taken to the nearest of them (the desktop's
 * `step_slider.rs` tests, iOS's `TextScaleTrack`).
 */
class TextScaleTrackTest {
    // Six stops (TextScaleLevel) on a 300px track, 30px in from each end:
    // centres every 48px from 30 to 270.
    private val steps = 6
    private val width = 300f
    private val inset = 30f

    @Test
    fun `the end stops sit one inset in from the track's ends`() {
        assertEquals(30f, TextScaleTrack.centreOf(0f, width, steps, inset), 0.001f)
        assertEquals(270f, TextScaleTrack.centreOf(5f, width, steps, inset), 0.001f)
        assertEquals(126f, TextScaleTrack.centreOf(2f, width, steps, inset), 0.001f)
        // Halfway through a glide from stop 2 to stop 3.
        assertEquals(150f, TextScaleTrack.centreOf(2.5f, width, steps, inset), 0.001f)
    }

    @Test
    fun `a touch goes to the nearest dot, and off the ends to the end stops`() {
        assertEquals(0, TextScaleTrack.stopAt(0f, width, steps, inset))
        assertEquals(0, TextScaleTrack.stopAt(-40f, width, steps, inset))
        // 53 is 23 from stop 0's centre (30) and 25 from stop 1's (78).
        assertEquals(0, TextScaleTrack.stopAt(53f, width, steps, inset))
        assertEquals(1, TextScaleTrack.stopAt(55f, width, steps, inset))
        assertEquals(5, TextScaleTrack.stopAt(300f, width, steps, inset))
        assertEquals(5, TextScaleTrack.stopAt(400f, width, steps, inset))
    }

    @Test
    fun `every dot's own centre is its own stop`() {
        for (stop in 0 until steps) {
            val centre = TextScaleTrack.centreOf(stop.toFloat(), width, steps, inset)
            assertEquals(stop, TextScaleTrack.stopAt(centre, width, steps, inset))
        }
    }

    @Test
    fun `before the first layout, and with one stop, everything is stop 0`() {
        assertEquals(0, TextScaleTrack.stopAt(120f, 0f, steps, inset))
        assertEquals(0, TextScaleTrack.stopAt(120f, width, 1, inset))
    }

    @Test
    fun `the thumb grows through hover to held, and back`() {
        val rest = 20f
        val hover = 24f
        val held = 26f
        assertEquals(20f, TextScaleTrack.lift(rest, hover, held, TextScaleTrack.REST), 0.001f)
        assertEquals(22f, TextScaleTrack.lift(rest, hover, held, 0.5f), 0.001f)
        assertEquals(24f, TextScaleTrack.lift(rest, hover, held, TextScaleTrack.HOVER), 0.001f)
        assertEquals(26f, TextScaleTrack.lift(rest, hover, held, TextScaleTrack.HELD), 0.001f)
        // A spring that strays past either end is held to it.
        assertEquals(26f, TextScaleTrack.lift(rest, hover, held, 2.2f), 0.001f)
        assertEquals(20f, TextScaleTrack.lift(rest, hover, held, -0.1f), 0.001f)
    }
}
