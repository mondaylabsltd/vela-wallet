package app.getvela.wallet

import app.getvela.wallet.core.data.NotificationAsk
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 087 F14: the notification permission was asked on every send's receipt, and
 * again whenever the screen was recreated (seen right after a language switch).
 * It is asked once per install.
 */
class NotificationAskTest {

    @Test
    fun `the first receipt may ask, and nothing after it`() = runBlocking {
        val store = FakeStore()
        assertTrue("the first receipt asks", NotificationAsk.claim(store))
        assertEquals("1", store.values[NotificationAsk.KEY])
        assertFalse("the next send does not", NotificationAsk.claim(store))
        assertFalse("nor does a recreated screen", NotificationAsk.claim(store))
    }

    /** The mark outlives the process: a restart reads it back from the store. */
    @Test
    fun `an install that already asked never asks again`() = runBlocking {
        assertFalse(NotificationAsk.claim(FakeStore(mapOf(NotificationAsk.KEY to "1"))))
    }

    /** A store that cannot keep the mark does not turn into an ask per receipt. */
    @Test
    fun `a store that refuses the mark does not ask`() = runBlocking {
        val store = FakeStore().apply { refuseWrites = true }
        assertFalse(NotificationAsk.claim(store))
        assertFalse(NotificationAsk.claim(store))
    }

    @Test
    fun `the mark is one of the wallet's own keys`() {
        assertTrue(NotificationAsk.KEY.startsWith("vela."))
    }
}
