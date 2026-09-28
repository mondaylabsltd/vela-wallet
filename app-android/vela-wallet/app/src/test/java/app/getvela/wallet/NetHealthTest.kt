package app.getvela.wallet

import app.getvela.wallet.core.net.NetHealth
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.vela_core_uniffi.netHealthStep

/**
 * Spec 082 RE3: offline is the core's rule (`netHealthStep`) — this object
 * only steps it with what the calls did. The test walks the same calls
 * through the core and through [NetHealth] and checks they agree at every
 * step, plus the two edges the screen acts on.
 */
class NetHealthTest {
    @Before fun fresh() = NetHealth.reset()
    @After fun clean() = NetHealth.reset()

    /** The core's state as this test steps it. */
    private var misses = 0u
    private var online = true

    /** Every step of [calls] (true = answered) through the core and through [NetHealth], side by side. */
    private fun agreeing(calls: List<Boolean>): List<String?> {
        return calls.map { reached ->
            val next = netHealthStep(misses, online, reached)
            misses = next.misses
            online = next.online
            if (reached) NetHealth.reached() else NetHealth.unreached()
            assertEquals("after $calls", next.online, NetHealth.online.value)
            next.edge
        }
    }

    @Test
    fun `two misses are a blip, the third is offline, one answer is back`() {
        val edges = agreeing(listOf(false, false, false, true))
        assertEquals(listOf(null, null, "went_offline", "came_back"), edges)
        assertTrue(NetHealth.online.value)
    }

    @Test
    fun `an answer in between resets the count`() {
        agreeing(listOf(false, false, true, false, false))
        assertTrue(NetHealth.online.value)
        agreeing(listOf(false))
        assertFalse("the third miss in a row after the answer", NetHealth.online.value)
    }
}
