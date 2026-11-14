package app.getvela.wallet

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.click
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.captureToImage
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.components.VelaLaunchAnimation
import app.getvela.wallet.core.designsystem.tokens.VelaLaunch
import app.getvela.wallet.core.designsystem.tokens.VelaLaunchScene
import kotlin.math.abs
import kotlin.math.roundToInt
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Spec 012 on Android — the behaviours that need a real composition and a real
 * frame clock, so they cannot live in the JVM unit tests (the timeline itself
 * is `LaunchTimelineTest`'s).
 *
 * Run: `./gradlew :app:connectedDebugAndroidTest` (needs a device or emulator).
 */
@RunWith(AndroidJUnit4::class)
class LaunchAnimationTest {

    @get:Rule
    val compose = createComposeRule()

    /**
     * Time starts when the test says so. The tests drive a VIRTUAL clock in
     * jumps, measured from the overlay's first frame — but `setContent` with
     * the clock on auto-advance waits for idle, and a playing animation is
     * never idle, so it played the whole animation through before the test
     * took the clock. A 2.15 s jump from there landed after the dissolve, and
     * the fade test saw only 0 then 1. So the clock stops before anything is
     * composed.
     */
    @Before
    fun stopTheClock() {
        compose.mainClock.autoAdvance = false
    }

    /** Host harness mirroring `MainActivity`: page underneath, overlay on top. */
    private fun setContent(
        reduceMotion: Boolean = false,
        onFinishedCount: () -> Unit = {},
    ): () -> Float {
        var alpha = 0f
        compose.setContent {
            var launching by remember { mutableStateOf(true) }
            var pageAlpha by remember { mutableFloatStateOf(0f) }
            alpha = pageAlpha
            Box(modifier = Modifier.fillMaxSize()) {
                if (launching) {
                    VelaLaunchAnimation(
                        darkTheme = true,
                        reduceMotion = reduceMotion,
                        backgroundColor = Color.Black,
                        onProgress = { pageAlpha = it; alpha = it },
                        onFinished = {
                            pageAlpha = 1f
                            alpha = 1f
                            launching = false
                            onFinishedCount()
                        },
                    )
                }
            }
        }
        return { alpha }
    }

    /**
     * FR-018 / contract behaviour 1: `onFinished` fires EXACTLY once. A
     * completion racing the hard-ceiling timer is the obvious way to get a
     * double dismiss, and a double dismiss on the launch path is a screen that
     * flickers or a nav stack that loses an entry.
     */
    @Test
    fun onFinished_fires_exactly_once_across_a_whole_playback() {
        var calls = 0
        setContent(onFinishedCount = { calls += 1 })
        compose.mainClock.autoAdvance = false
        // Past the animation, the hold, the dissolve and the hard ceiling.
        compose.mainClock.advanceTimeBy(VelaLaunch.hardCeilingMs + 2_000)
        compose.waitForIdle()
        assertEquals("onFinished must fire exactly once", 1, calls)
    }

    /** FR-016: any tap cuts straight to the dissolve. */
    @Test
    fun a_tap_ends_playback_immediately() {
        var calls = 0
        setContent(onFinishedCount = { calls += 1 })
        compose.mainClock.autoAdvance = false
        compose.mainClock.advanceTimeBy(200) // mid-animation
        compose.onRoot().performTouchInput { click() }
        compose.mainClock.advanceTimeBy(VelaLaunch.exitCrossfadeMs.toLong() + 100)
        compose.waitForIdle()
        assertEquals("a tap must finish the animation", 1, calls)
    }

    /**
     * FR-019/FR-020: reduce-motion shows the finished lockup and does NOT sit
     * through the hold, so it is never slower than the normal path.
     */
    @Test
    fun reduce_motion_skips_the_hold() {
        var calls = 0
        setContent(reduceMotion = true, onFinishedCount = { calls += 1 })
        compose.mainClock.autoAdvance = false
        // Only the dissolve, not the hold — if the hold were honoured this
        // would still be 0.
        compose.mainClock.advanceTimeBy(VelaLaunch.exitCrossfadeMs.toLong() + 100)
        compose.waitForIdle()
        assertEquals("reduce-motion must not wait out the hold", 1, calls)
    }

    /**
     * FR-012: the two layers cross-dissolve. The page alpha must actually pass
     * through the middle rather than jumping 0 → 1, which is what an abrupt cut
     * looks like numerically. (The desktop build shipped exactly that bug.)
     */
    @Test
    fun the_page_fades_in_progressively_rather_than_snapping() {
        val pageAlpha = setContent()
        compose.mainClock.autoAdvance = false
        // Up to the dissolve's first frame, wherever the play ends. With the
        // system's animator scale at 0 (an emulator set up for tests, CI's,
        // or a person who turned animations off) the play ends at once, so
        // the dissolve begins after the hold alone — a fixed
        // 1.7 s + hold jump landed after it, and the test saw only 0 then 1.
        var waited = 0L
        while (pageAlpha() == 0f && waited < VelaLaunch.hardCeilingMs) {
            compose.mainClock.advanceTimeByFrame()
            compose.waitForIdle()
            waited += 16
        }

        var sawPartial = pageAlpha() > 0.05f && pageAlpha() < 0.95f
        repeat(8) {
            compose.mainClock.advanceTimeBy(VelaLaunch.exitCrossfadeMs / 8L)
            compose.waitForIdle()
            val a = pageAlpha()
            if (a > 0.05f && a < 0.95f) sawPartial = true
        }
        assertTrue(
            "the Welcome content must fade in through intermediate values, not snap",
            sawPartial,
        )
    }

    /**
     * The lockup is drawn, and drawn with the NONZERO fill rule: the crossbar
     * of "e" is its own contour wound like the outline, so even-odd would
     * punch it out of the letter. Sampled on the settled lockup, mid-hold,
     * at interior points (no anti-aliased edges): the crossbar and the three
     * pieces of the mark in their colours, the bowl of "e" left empty.
     */
    @Test
    fun the_settled_lockup_is_drawn_with_its_crossbars() {
        setContent()
        compose.mainClock.advanceTimeBy(VelaLaunchScene.settledMs.toLong() + 100)
        compose.waitForIdle()
        val image = compose.onRoot().captureToImage().toPixelMap()

        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val config = context.resources.configuration
        val density = context.resources.displayMetrics.density
        val large = VelaLaunch.isLargeScreen(config.screenWidthDp.dp, config.screenHeightDp.dp)
        val (boxW, boxH) = VelaLaunch.boxSize(config.screenWidthDp.dp, large)
        val boxWpx = boxW.value * density
        val boxHpx = boxH.value * density
        val left = (image.width - boxWpx) / 2f
        val top = (image.height - boxHpx) / 2f
        val canvasW = if (large) VelaLaunch.largeCanvasW else VelaLaunch.phoneCanvasW
        val fit = boxWpx / canvasW
        val frame = if (large) VelaLaunchScene.largeScale else 1f
        val offX = if (large) VelaLaunchScene.largeOffsetX else 0f
        val offY = if (large) VelaLaunchScene.largeOffsetY else 0f
        fun at(sceneX: Float, sceneY: Float): Color = image[
            (left + (offX + sceneX * frame) * fit).roundToInt(),
            (top + (offY + sceneY * frame) * fit).roundToInt(),
        ]
        fun assertColor(what: String, expected: Color, actual: Color) {
            val close = abs(expected.red - actual.red) < 0.02f &&
                abs(expected.green - actual.green) < 0.02f &&
                abs(expected.blue - actual.blue) < 0.02f
            assertTrue("$what: expected $expected, drew $actual", close)
        }
        val colors = VelaLaunchScene.dark
        val markX = VelaLaunchScene.markToX
        val markY = VelaLaunchScene.markToY

        assertColor("the crossbar of the first e", colors.wordmark, at(136f, 63.6f))
        assertColor("the bowl of the first e", Color.Black, at(136f, 58.9f))
        assertColor("the foot of the V", colors.wordmark, at(108.1f, 73.5f))
        assertColor("the notch of the V", Color.Black, at(108.04f, 60f))
        assertColor("the sail", colors.sunset, at(markX - 8f, markY + 8f))
        assertColor("the jib", colors.peach, at(markX + 10f, markY + 8f))
        assertColor("the hull", colors.hull, at(markX, markY + 26f))
    }
}
