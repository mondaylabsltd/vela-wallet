package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.CoreDriver
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.Collections

/**
 * The driver's first duty, and the one it was not doing: events reach the core
 * in the order they were dispatched, and effects START in the order the core
 * asked for them.
 *
 * Found 2026-09-19 behind five "flaky" machine tests that had each been
 * explained separately (a wait predicate too narrow, a write not yet landed, a
 * timeout nobody could reproduce). They were one bug. `dispatch` launched a
 * coroutine per event onto a thread pool and let them race for a mutex, so two
 * events sent back to back could arrive reversed — on a fast, idle machine
 * almost never; on a loaded CI runner, often. It is not a test problem: two
 * quick taps on a phone are two events sent back to back.
 *
 * No machine, no uniffi: a bridge that records what it is handed is enough to
 * see the order, and makes this deterministic where the machine tests were not.
 */
class CoreDriverOrderTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    /** Records every event; asks for one effect per event, tagged with the event's number. */
    private class RecordingBridge : CoreBridge {
        val events: MutableList<Int> = Collections.synchronizedList(mutableListOf())
        private var nextEffect = 0L

        @Synchronized
        override fun dispatch(eventJson: String): String {
            val n = JSONObject(eventJson).getInt("n")
            events += n
            val effect = JSONObject().put("id", ++nextEffect).put("operation", JSONObject().put("n", n))
            return JSONObject().put("view", JSONObject()).put("effects", JSONArray().put(effect)).toString()
        }

        @Synchronized
        override fun resolveEffect(effectId: ULong, resultJson: String): String =
            JSONObject().put("view", JSONObject()).toString()

        override fun view(): String = "{}"
    }

    @Test
    fun `events reach the core in the order they were dispatched`() = runBlocking {
        val bridge = RecordingBridge()
        val driver = CoreDriver(bridge, scope, perform = { "{}" }, onView = {}, escapedFailure = { _, _ -> "{}" })
        val count = 2_000
        repeat(count) { driver.dispatch("""{"n":$it}""") }
        withTimeout(20_000) { while (bridge.events.size < count) delay(5) }
        assertEquals((0 until count).toList(), bridge.events.toList())
    }

    /**
     * A write the core asked for second must not land first. Persisting is a
     * synchronous act in every executor that does it, so "effects start in
     * order and run until they first suspend" is what keeps the LAST document
     * the one on disk — the property `ExploreMachineTest`'s restore leans on.
     */
    @Test
    fun `effects that do not suspend complete in the order the core asked for them`() = runBlocking {
        val bridge = RecordingBridge()
        val performed: MutableList<Int> = Collections.synchronizedList(mutableListOf())
        val driver = CoreDriver(
            bridge, scope,
            perform = { operation -> performed += operation.getInt("n"); "{}" },
            onView = {}, escapedFailure = { _, _ -> "{}" },
        )
        val count = 2_000
        repeat(count) { driver.dispatch("""{"n":$it}""") }
        withTimeout(20_000) { while (performed.size < count) delay(5) }
        assertEquals((0 until count).toList(), performed.toList())
    }
}
