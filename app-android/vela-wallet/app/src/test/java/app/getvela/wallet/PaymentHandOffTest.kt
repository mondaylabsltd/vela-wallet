package app.getvela.wallet

import app.getvela.wallet.feature.send.core.PaymentHandOff
import app.getvela.wallet.feature.send.core.SendOpenParams
import kotlinx.coroutines.flow.MutableStateFlow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/**
 * Spec 078 round 3: a payment request from outside Send reaches Send once.
 * Open → applied now, nothing parked; closed → parked, taken once by the open.
 * The bug this pins: with Send already open the request was parked, never
 * taken, and applied the NEXT time Send opened — over a recipient the person
 * had typed.
 */
class PaymentHandOffTest {
    private val parkedRequest = MutableStateFlow<SendOpenParams?>(null)
    private val parkedScan = MutableStateFlow<String?>(null)
    private val handOff = PaymentHandOff(parkedRequest, parkedScan)
    private val link = SendOpenParams(prefilled_recipient = PAYEE, prefilled_chain_id = "100", prefilled_amount_base = "1000", locked = true)

    @Test
    fun `a link while send is open is applied now, once, and never again`() {
        val opened = mutableListOf<SendOpenParams>()
        var entered = 0
        handOff.request(link, sendOpen = true, openNow = { opened += it }, enterSend = { entered++ })
        assertEquals(listOf(link), opened)
        assertEquals("the send is already there: nothing to enter", 0, entered)
        // Nothing is left for the next open of Send to re-apply.
        assertNull(parkedRequest.value)
        assertNull(handOff.takeRequest())
    }

    @Test
    fun `a link while send is closed opens send, and the open takes it exactly once`() {
        var entered = 0
        handOff.request(link, sendOpen = false, openNow = { error("not open") }, enterSend = { entered++ })
        assertEquals(1, entered)
        assertEquals(link, handOff.takeRequest())
        assertNull("taken once — a second open finds nothing", handOff.takeRequest())
    }

    @Test
    fun `a code scanned while send is open is read now and not parked`() {
        val read = mutableListOf<String>()
        handOff.scan("ethereum:$PAYEE@100", sendOpen = true, scanNow = { read += it }, enterScanner = { error("already open") })
        assertEquals(listOf("ethereum:$PAYEE@100"), read)
        assertNull(handOff.takeScan())

        var entered = 0
        handOff.scan("ethereum:$PAYEE@100", sendOpen = false, scanNow = { error("not open") }, enterScanner = { entered++ })
        assertEquals(1, entered)
        assertEquals("ethereum:$PAYEE@100", handOff.takeScan())
        assertNull(handOff.takeScan())
    }

    private companion object {
        const val PAYEE = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    }
}
