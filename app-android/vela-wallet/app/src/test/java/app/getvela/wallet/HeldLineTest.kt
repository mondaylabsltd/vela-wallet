package app.getvela.wallet

import app.getvela.wallet.feature.signing.HeldLine
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The signing sheet is anchored at the bottom and wraps its content; a line
 * that came and went with every re-quote moved the whole form (device-found,
 * ~33 px on the Xiaomi). [HeldLine] is what keeps those lines still.
 */
class HeldLineTest {

    private val noCoin = "No token can pay this fee"

    /** The sheet draws a held line invisible and silent (its room only) — the web's rule. */
    @Test
    fun `the line under the fee keeps its room while the fee is measured again`() {
        val line = HeldLine()
        assertEquals(noCoin, line.next(noCoin, measuring = false))
        // A re-quote, a speed pick, a refresh: the builder has no line to give.
        assertEquals("held while measuring", noCoin, line.next(null, measuring = true))
        assertEquals(noCoin, line.next(null, measuring = true))
        // Landed, and the coin can pay now: the line goes.
        assertNull(line.next(null, measuring = false))
        assertNull("nothing to hold any more", line.next(null, measuring = true))
        // A new word replaces the old at once.
        assertEquals("Insufficient ETH", line.next("Insufficient ETH", measuring = true))
    }

    /**
     * The first figure, on an account with nothing to pay from (iPhone pass
     * 2026-10-09, the backup sheet): the core says so while the figure is
     * measured, and its line's room is held from then — the landing moves
     * nothing. Without that word, nothing is held before a line was said.
     */
    @Test
    fun `before any line was said, the core's reserve is held while measuring`() {
        val line = HeldLine()
        assertEquals("held from the first measurement", noCoin, line.next(null, measuring = true, reserve = noCoin))
        assertEquals("the figure lands as the line held for it", noCoin, line.next(noCoin, measuring = false))
        assertEquals(noCoin, line.next(null, measuring = true))
        assertNull("not measuring: a reserve is no line", HeldLine().next(null, measuring = false, reserve = noCoin))
        assertNull("no word from the core: nothing held", HeldLine().next(null, measuring = true))
    }

    @Test
    fun `the confirm note's room is the last note's, said now or not`() {
        val note = HeldLine()
        assertNull("no note said yet: no line at all (a board gains no blank line)", note.room(null))
        assertEquals("Calculating the network fee…", note.room("Calculating the network fee…"))
        assertEquals("kept (drawn invisible) once the confirm opens", "Calculating the network fee…", note.room(null))
        assertEquals("Reading…", note.room("Reading…"))
    }
}
