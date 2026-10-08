package app.getvela.wallet

import app.getvela.wallet.feature.send.core.FeeTokenWord
import app.getvela.wallet.feature.send.core.SendEvent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * The fee → send bridge's word about the fee card's coin (C10): the send
 * machine is told `fee_token_changed` whenever `FeeView.fee_token` differs
 * from what this send session was last told — and a freshly opened session
 * has been told nothing, so its first word goes even when it is `null`.
 */
class FeeTokenWordTest {

    private val usdt = "0x55d398326f99059fF775485246999027B3197955"

    @Test
    fun `a fresh session hears the first word, null included, and no repeats`() {
        val word = FeeTokenWord()
        assertEquals(SendEvent.FeeTokenChanged(null), word.news(null))
        assertNull("already told", word.news(null))
        assertEquals(SendEvent.FeeTokenChanged(usdt), word.news(usdt))
        assertNull(word.news(usdt))
        assertEquals("back to the chain's own coin", SendEvent.FeeTokenChanged(null), word.news(null))
    }

    @Test
    fun `a new session forgets what the last one was told`() {
        val word = FeeTokenWord()
        word.news(usdt)
        word.forget()
        assertEquals("the same coin, told again to the new session", SendEvent.FeeTokenChanged(usdt), word.news(usdt))
        word.forget()
        assertEquals(SendEvent.FeeTokenChanged(null), word.news(null))
    }
}
