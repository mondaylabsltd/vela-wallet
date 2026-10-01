package app.getvela.wallet

import app.getvela.wallet.core.net.NetHealth
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import uniffi.vela_core_uniffi.NetHealthState
import uniffi.vela_core_uniffi.netHealthFresh
import uniffi.vela_core_uniffi.netHealthStep

/**
 * Spec 082 RE3, RJ14: offline is the core's rule (`netHealthStep`) — this
 * object only steps it with what the calls did, the chain each call read, and
 * the clock. The test walks the same calls through the core and through
 * [NetHealth] and checks they agree at every step, plus the edges the screen
 * acts on, and G53's case: one chain faulted while the others answer is never
 * "offline".
 */
class NetHealthTest {
    private var nowMs = 1_000_000.0

    @Before fun fresh() {
        NetHealth.reset()
        NetHealth.clock = { nowMs }
    }

    @After fun clean() = NetHealth.reset()

    /** The core's state as this test steps it. */
    private var state: NetHealthState = netHealthFresh()

    /** One call: [reached] or not, from [source], [afterMs] after the last one. */
    private data class Call(val reached: Boolean, val source: Int?, val afterMs: Double = 0.0)

    /** Every step of [calls] through the core and through [NetHealth], side by side. */
    private fun agreeing(calls: List<Call>): List<String?> = calls.map { call ->
        nowMs += call.afterMs
        val next = netHealthStep(state, call.reached, call.source?.toUInt(), nowMs)
        state = next.state
        if (call.reached) NetHealth.reached(call.source) else NetHealth.unreached(call.source)
        assertEquals("after $call", next.state.online, NetHealth.online.value)
        next.edge
    }

    @Test
    fun `misses from two chains with nothing answering for ten seconds are offline, one answer is back`() {
        val edges = agreeing(
            listOf(
                Call(false, 1),
                Call(false, 100, afterMs = 4_000.0),
                Call(false, 1, afterMs = 7_000.0),
                Call(true, 100, afterMs = 1_000.0),
            ),
        )
        assertEquals(listOf(null, null, "went_offline", "came_back"), edges)
        assertTrue(NetHealth.online.value)
    }

    @Test
    fun `ten misses from Gnosis alone are Gnosis's notice, never offline (G53)`() {
        val edges = agreeing(List(10) { Call(false, 100, afterMs = 2_000.0) })
        assertTrue("one faulted chain is not a missing network", edges.all { it == null })
        assertTrue(NetHealth.online.value)
    }

    @Test
    fun `three quick misses from two chains are a blip until the quiet window has passed`() {
        val edges = agreeing(listOf(Call(false, 1), Call(false, 100, afterMs = 200.0), Call(false, 1, afterMs = 200.0)))
        assertTrue(edges.all { it == null })
        assertTrue("less than ten seconds without an answer", NetHealth.online.value)
    }

    @Test
    fun `an answer in between resets the run`() {
        agreeing(listOf(Call(false, 1), Call(false, 100, afterMs = 5_000.0), Call(true, 10, afterMs = 1_000.0)))
        agreeing(listOf(Call(false, 1, afterMs = 5_000.0), Call(false, 100, afterMs = 5_000.0)))
        assertTrue("two misses after the answer", NetHealth.online.value)
        agreeing(listOf(Call(false, 1, afterMs = 1_000.0)))
        assertFalse("the third miss in a row from two chains, eleven seconds without an answer", NetHealth.online.value)
    }
}
