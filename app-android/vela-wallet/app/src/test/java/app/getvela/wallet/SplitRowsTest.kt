package app.getvela.wallet

import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SplitRows
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Test

class SplitRowsTest {
    private val a = SendRecipientDraft("rcpt_1", "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", "0.001", "Founder")
    private val b = SendRecipientDraft("rcpt_2", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", "0.002")

    @Test
    fun `an amount edit touches one row and keeps the others by identity`() {
        val next = SplitRows.amountEdited(listOf(a, b), "rcpt_2", "0.5")
        assertSame(a, next[0])
        assertEquals("0.5", next[1].amount)
        assertEquals("rcpt_2", next[1].id)
        assertEquals("Founder", next[0].name)
    }

    @Test
    fun `an address edit drops the name that named the old address`() {
        val next = SplitRows.addressEdited(listOf(a, b), "rcpt_1", "0x1111111111111111111111111111111111111111")
        assertNull(next[0].name)
        assertEquals("0.001", next[0].amount)
        assertSame(b, next[1])
    }

    @Test
    fun `a removed row leaves and an appended row has no id for the core to mint`() {
        assertEquals(listOf(b), SplitRows.removed(listOf(a, b), "rcpt_1"))
        val next = SplitRows.appended(listOf(a, b))
        assertEquals(3, next.size)
        assertEquals("", next[2].id)
        assertEquals("", next[2].address)
        assertSame(a, next[0])
    }
}
