package app.getvela.wallet

import app.getvela.wallet.core.platform.LaunchIntent
import app.getvela.wallet.feature.multisig.LookAsk
import app.getvela.wallet.feature.multisig.MultisigReach
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Round 14's Android findings, at the shell's pure seams (docs/multisig-machines.md §20). */
class R14AndroidFixesTest {

    // -- r14-6: the fee line recovers by itself, and offers Check again ------------

    @Test
    fun `a look that could not price it is asked again slowly after the first few, never a priced one`() {
        for (kind in listOf(null, "unpriced", "unchecked")) {
            assertTrue(LookAsk.timerAsks(kind, 0, 0))
            assertFalse("not again at once past the first few", LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, LookAsk.ASK_EVERY_MS))
            assertTrue("again once the slow pace is up", LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, LookAsk.SLOW_EVERY_MS))
        }
        for (kind in listOf("priced", "fails", "refused", "spent")) {
            assertFalse(kind, LookAsk.timerAsks(kind, LookAsk.AUTO_LIMIT, 10 * LookAsk.SLOW_EVERY_MS))
        }
        // 3.5 minutes of "Couldn't check whether it would run" (r14-6) is asked
        // again many times — the line comes back on its own once the network does.
        var asks = 0
        var last = 0L
        var clock = 0L
        while (clock < 210_000) {
            clock += LookAsk.ASK_EVERY_MS
            if (LookAsk.timerAsks("unchecked", asks, clock - last)) { asks += 1; last = clock }
        }
        assertTrue("asked $asks times", asks >= LookAsk.AUTO_LIMIT + 8)
    }

    @Test
    fun `Check again is offered under a fee that was not worked out or checked`() {
        assertTrue(LookAsk.offersCheckAgain("unchecked"))
        assertTrue(LookAsk.offersCheckAgain("unpriced"))
        assertFalse(LookAsk.offersCheckAgain("priced"))
        assertFalse(LookAsk.offersCheckAgain(null))
    }

    // -- r14-9: the fee signed is the fee shown ------------------------------------
    // The core's rule since §19.11 (`fee_moved`); the shell's copy is gone —
    // R14LeftoversTest pins that, and the words of the core's answer.

    // -- r14-2: the launch intent, acted on once ----------------------------------

    @Test
    fun `an activity's starting intent is acted on only on its first creation`() {
        assertTrue(LaunchIntent.firstDelivery(noSavedState = true, flags = 0))
        assertFalse("recreated: a restored task or a configuration change", LaunchIntent.firstDelivery(noSavedState = false, flags = 0))
        assertFalse("reopened from Recents", LaunchIntent.firstDelivery(noSavedState = true, flags = LaunchIntent.LAUNCHED_FROM_HISTORY))
        assertEquals(android.content.Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY, LaunchIntent.LAUNCHED_FROM_HISTORY)
    }

    // -- r14-2 / r14-6: asked again when a network answers again ------------------

    @Test
    fun `a network the pool lets go of, or the device back online, is a reason to read again`() {
        assertEquals(setOf(5042002), MultisigReach.back(setOf(5042002, 1), setOf(1), wasOnline = true, online = true))
        assertNull("nothing came back", MultisigReach.back(setOf(5042002), setOf(5042002), wasOnline = true, online = true))
        assertNull("a network newly out of reach is no reason", MultisigReach.back(emptySet(), setOf(5042002), wasOnline = true, online = true))
        assertEquals("the device back: every network", emptySet<Int>(), MultisigReach.back(setOf(5042002), setOf(5042002), wasOnline = false, online = true))
        assertNull("offline: nothing to ask", MultisigReach.back(setOf(5042002), emptySet(), wasOnline = true, online = false))
    }
}
