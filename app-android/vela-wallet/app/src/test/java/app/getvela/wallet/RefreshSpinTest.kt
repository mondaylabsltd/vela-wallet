package app.getvela.wallet

import app.getvela.wallet.feature.wallet.RefreshSpin
import app.getvela.wallet.feature.wallet.RefreshSpin.Companion.MIN_SPIN_MS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The hero refresh control's spin (issue 462): it starts on the tap, holds
 * while the core says the person's refresh is out, and never ends sooner
 * than 650 ms after it started — so a read answered in a frame still says,
 * on screen, that it happened. While it turns, a second tap does nothing.
 */
class RefreshSpinTest {

    @Test
    fun `a tap spins at once, before the core answers`() {
        val spin = RefreshSpin().tapped(now = 1_000)!!
        assertTrue(spin.spinning)
        assertEquals(1_000 + MIN_SPIN_MS, spin.releaseAt)
    }

    @Test
    fun `a fast read still turns for the minimum`() {
        var spin = RefreshSpin().tapped(now = 1_000)!!
        spin = spin.core(busy = true, now = 1_016)
        assertNull("no release while the core is busy", spin.releaseAt)
        spin = spin.core(busy = false, now = 1_100)
        // Done after 100 ms — but the spin holds to 650.
        assertTrue(spin.settle(now = 1_100).spinning)
        assertTrue(spin.settle(now = 1_000 + MIN_SPIN_MS - 1).spinning)
        assertFalse(spin.settle(now = 1_000 + MIN_SPIN_MS).spinning)
    }

    @Test
    fun `a slow read turns until it settles`() {
        var spin = RefreshSpin().tapped(now = 0)!!
        spin = spin.core(busy = true, now = 10)
        assertTrue("held past the minimum while the core is busy", spin.settle(now = 5_000).spinning)
        spin = spin.core(busy = false, now = 5_000)
        assertFalse(spin.settle(now = 5_000).spinning)
    }

    @Test
    fun `while it turns a second tap is refused`() {
        val spin = RefreshSpin().tapped(now = 0)!!
        assertNull(spin.tapped(now = 100))
        assertNull(spin.core(busy = true, now = 50).tapped(now = 2_000))
    }

    @Test
    fun `a pull the core reports spins the control too, held the same`() {
        var spin = RefreshSpin().core(busy = true, now = 3_000)
        assertTrue(spin.spinning)
        assertNull(spin.tapped(now = 3_010))
        spin = spin.core(busy = false, now = 3_200)
        assertTrue(spin.settle(now = 3_200).spinning)
        assertFalse(spin.settle(now = 3_000 + MIN_SPIN_MS).spinning)
    }

    @Test
    fun `a core that never says busy ends the spin at the minimum`() {
        // The dispatch was refused or answered before the view came back:
        // the tap still turns, once, for the minimum — never forever.
        val spin = RefreshSpin().tapped(now = 0)!!.core(busy = false, now = 5)
        assertFalse(spin.settle(now = MIN_SPIN_MS).spinning)
        assertTrue(spin.settle(now = MIN_SPIN_MS).tapped(now = MIN_SPIN_MS + 1)!!.spinning)
    }

    @Test
    fun `idle is not spinning and has nothing to release`() {
        val idle = RefreshSpin().core(busy = false, now = 0)
        assertFalse(idle.spinning)
        assertNull(idle.releaseAt)
        assertEquals(idle, idle.settle(now = 10_000))
    }
}
