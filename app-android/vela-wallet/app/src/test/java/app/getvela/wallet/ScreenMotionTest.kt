package app.getvela.wallet

import app.getvela.wallet.core.designsystem.motion.BackLayer
import app.getvela.wallet.core.designsystem.motion.reducedMotion
import app.getvela.wallet.core.designsystem.tokens.VelaMotion
import app.getvela.wallet.feature.signing.SigningHaptics
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.GuardView
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Phase 3 motion and haptics, the parts that are rules rather than drawing:
 * the predictive-back layer's shape, the reduce-motion switch, and when the
 * signing sheet buzzes a warning.
 */
class ScreenMotionTest {
    @Test
    fun `at rest the screen is untouched`() {
        assertEquals(BackLayer.REST, BackLayer.of(0f, reduced = false))
    }

    @Test
    fun `a fully held gesture is the M3 in-app shape`() {
        val held = BackLayer.of(1f, reduced = false)
        assertEquals(VelaMotion.predictiveBackScale, held.scale, 1e-4f)
        assertEquals(1f, held.shiftFraction, 1e-4f)
        assertEquals(1f, held.cornerFraction, 1e-4f)
        // Beyond the gesture's range is the range's end, never past it.
        assertEquals(held, BackLayer.of(1.7f, reduced = false))
        assertEquals(BackLayer.REST, BackLayer.of(-0.2f, reduced = false))
    }

    @Test
    fun `the layer follows the gesture monotonically`() {
        var last = BackLayer.REST
        for (step in 1..20) {
            val next = BackLayer.of(step / 20f, reduced = false)
            assertTrue("shrinks as the gesture goes on", next.scale <= last.scale)
            assertTrue("moves toward the edge", next.shiftFraction >= last.shiftFraction)
            last = next
        }
    }

    @Test
    fun `less motion asked for - the screen does not move with the gesture`() {
        assertEquals(BackLayer.REST, BackLayer.of(0.5f, reduced = true))
        assertEquals(BackLayer.REST, BackLayer.of(1f, reduced = true))
        assertTrue(reducedMotion(0f))
        assertFalse(reducedMotion(1f))
        assertFalse(reducedMotion(0.5f))
    }

    @Test
    fun `the signing sheet warns once for what the core judged dangerous, and only that`() {
        assertFalse(SigningHaptics.warnOnOpen(ClearSigningView(), GuardView()))
        assertTrue("eth_sign / a SIWE domain that does not bind", SigningHaptics.warnOnOpen(ClearSigningView(danger_haptic = true), GuardView()))
        assertTrue("an unbounded approval", SigningHaptics.warnOnOpen(ClearSigningView(), GuardView(unlimited_warning = true)))
        assertTrue(SigningHaptics.warnOnOpen(ClearSigningView(danger_haptic = true), GuardView(unlimited_warning = true)))
    }
}
