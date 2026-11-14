package app.getvela.wallet

import androidx.compose.ui.unit.dp
import app.getvela.wallet.core.designsystem.tokens.VelaLaunch
import app.getvela.wallet.core.designsystem.tokens.VelaLaunchScene
import kotlin.math.abs
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 012 FR-011 and the shared time budget.
 *
 * The same tables are asserted in Rust, Swift and TypeScript. Repeating them in
 * four languages is the point: a transcription slip in any one shows up here
 * rather than on a user's screen, and nothing else would catch it — the four
 * apps never run the same code.
 */
class LaunchGeometryTest {

    /** Lockup width ÷ core canvas width, a property of the scene. */
    private val phoneLockupRatio = 314.85f / VelaLaunch.phoneCanvasW
    private val largeLockupRatio = 566.73f / VelaLaunch.largeCanvasW

    @Test
    fun `box ratios are derived from the scene, not chosen`() {
        // core canvas ÷ full-bleed canvas.
        assertEquals(350f / 390f, VelaLaunch.phoneBoxWidthRatio, 1e-6f)
        assertEquals(680f / 1920f, VelaLaunch.largeBoxWidthRatio, 1e-6f)
    }

    @Test
    fun `at the authored width the box is the core canvas one to one`() {
        val (phoneW, phoneH) = VelaLaunch.boxSize(390.dp, largeScreen = false)
        assertEquals(VelaLaunch.phoneCanvasW, phoneW.value, 0.01f)
        assertEquals(VelaLaunch.phoneCanvasH, phoneH.value, 0.01f)

        val (largeW, largeH) = VelaLaunch.boxSize(1920.dp, largeScreen = true)
        assertEquals(VelaLaunch.largeCanvasW, largeW.value, 0.01f)
        assertEquals(VelaLaunch.largeCanvasH, largeH.value, 0.01f)
    }

    @Test
    fun `the lockup holds the authored share of the viewport at every size`() {
        // research D1's table. Phone: 80.7 % of width. Large screen: 29.5 %.
        for (width in listOf(320f, 360f, 390f, 412f, 430f)) {
            val (boxW, _) = VelaLaunch.boxSize(width.dp, largeScreen = false)
            val share = boxW.value * phoneLockupRatio / width
            assertTrue(
                "phone @${width}dp: lockup is $share of the width, authored is 0.8073",
                abs(share - 0.8073f) < 0.001f,
            )
        }
        for (width in listOf(768f, 1280f, 1440f, 1920f, 3440f)) {
            val (boxW, _) = VelaLaunch.boxSize(width.dp, largeScreen = true)
            val share = boxW.value * largeLockupRatio / width
            assertTrue(
                "large @${width}dp: lockup is $share of the width, authored is 0.2952",
                abs(share - 0.2952f) < 0.001f,
            )
        }
    }

    @Test
    fun `the form-factor predicate is the one shared by all four apps`() {
        // Android is portrait-locked and phones are under 768 dp, so in practice
        // this always resolves to the phone canvas — but a tablet picks up
        // the large-screen one with no special-casing, which is the whole reason
        // the predicate is shared rather than hardcoded per platform.
        assertTrue(!VelaLaunch.isLargeScreen(390.dp, 844.dp))
        assertTrue(!VelaLaunch.isLargeScreen(430.dp, 932.dp))
        assertTrue(VelaLaunch.isLargeScreen(768.dp, 1024.dp))
        assertTrue(VelaLaunch.isLargeScreen(1280.dp, 800.dp))
    }

    @Test
    fun `the large canvas frames the same scene, centred`() {
        // One scene, two framings: the phone canvas drawn at largeScale, offset
        // into the large canvas with equal margins either side.
        val w = VelaLaunchScene.canvasW * VelaLaunchScene.largeScale + 2 * VelaLaunchScene.largeOffsetX
        val h = VelaLaunchScene.canvasH * VelaLaunchScene.largeScale + 2 * VelaLaunchScene.largeOffsetY
        assertEquals(VelaLaunch.largeCanvasW, w, 0.01f)
        assertEquals(VelaLaunch.largeCanvasH, h, 0.01f)
        assertEquals("the large lockup is the phone one, scaled", 566.73f, 314.85f * VelaLaunchScene.largeScale, 0.01f)
        assertEquals(VelaLaunchScene.canvasW, VelaLaunch.phoneCanvasW, 0f)
        assertEquals(VelaLaunchScene.canvasH, VelaLaunch.phoneCanvasH, 0f)
    }

    /**
     * The transition timeline, pinned. These are numbers the founder set by feel
     * on a running build; a silent change to any of them changes the product.
     */
    @Test
    fun `transition timeline matches the agreed shape`() {
        assertEquals(1700, VelaLaunch.durationMs)
        assertEquals(VelaLaunchScene.durationMs, VelaLaunch.durationMs.toFloat(), 0f)
        assertEquals(400L, VelaLaunch.holdMs)
        assertEquals(400, VelaLaunch.exitCrossfadeMs)

        val nominal = VelaLaunch.durationMs + VelaLaunch.holdMs + VelaLaunch.exitCrossfadeMs
        assertEquals(2500L, nominal)
        assertTrue(
            "the ceiling must leave room for the nominal sequence, or a healthy launch is cut short",
            VelaLaunch.hardCeilingMs > nominal,
        )
        assertTrue(
            "too little slack for a slow device",
            VelaLaunch.hardCeilingMs - nominal >= 400L,
        )
    }
}
