package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.CoreDriver
import app.getvela.wallet.core.crux.CoreHost
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.Serializable
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Collections

/**
 * core-perf-1: every machine skips unchanged views. The driver opts the core
 * in before anything else, keeps the view it holds when an answer carries
 * none, folds the volatile fields back in, and draws nothing for an answer
 * that moved nothing — while an event that moved nothing still counts as
 * applied (`CoreHost.applied`, the send's quote wait).
 */
class CoreDriverLeanAnswersTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    /** Answers from a script: event `n` gets `answers[n]`, every effect answer `{effects:[]}` plus [resolved]. */
    private class ScriptedBridge(private val answers: List<JSONObject>, private val resolved: JSONObject = JSONObject()) : CoreBridge {
        val calls: MutableList<String> = Collections.synchronizedList(mutableListOf())
        @Volatile var optedIn: Boolean? = null

        override fun dispatch(eventJson: String): String {
            calls += "dispatch"
            return answers[JSONObject(eventJson).getInt("n")].toString()
        }

        override fun resolveEffect(effectId: ULong, resultJson: String): String {
            calls += "resolve"
            return JSONObject(resolved.toString()).put("effects", JSONArray()).toString()
        }

        override fun view(): String = """{"a":0}"""

        override fun skipUnchangedViews(includeVolatile: Boolean) {
            calls += "skip($includeVolatile)"
            optedIn = includeVolatile
        }
    }

    private fun lean(version: Int, view: JSONObject? = null, volatile: JSONObject? = null, effects: JSONArray = JSONArray()) =
        JSONObject().put("view_version", version).put("effects", effects).apply {
            view?.let { put("view", it) }
            volatile?.let { put("volatile", it) }
        }

    @Test
    fun `the driver opts in before the first answer, and keeps the view it holds`() = runBlocking {
        val bridge = ScriptedBridge(
            listOf(
                lean(1, view = JSONObject().put("a", 1)),
                lean(1),
                lean(2, view = JSONObject().put("a", 2)),
            ),
        )
        val views: MutableList<String> = Collections.synchronizedList(mutableListOf())
        var unchanged = 0
        val driver = CoreDriver(
            bridge, scope, perform = { "{}" }, onView = { views += it.toString() },
            escapedFailure = { _, _ -> "{}" }, onUnchanged = { unchanged++ },
        )
        repeat(3) { driver.dispatch("""{"n":$it}""") }
        withTimeout(10_000) { while (bridge.calls.count { it == "dispatch" } < 3 || views.size < 2) delay(5) }
        assertEquals("skip(false)", bridge.calls.first())
        assertEquals(false, bridge.optedIn)
        assertEquals(listOf("""{"a":1}""", """{"a":2}"""), views.toList())
        assertEquals("the second event moved nothing, and is still applied", 1, unchanged)
    }

    @Test
    fun `volatile fields are folded back into the view, whichever half moved`() = runBlocking {
        val bridge = ScriptedBridge(
            listOf(
                lean(1, view = JSONObject().put("a", 1), volatile = JSONObject().put("pending", JSONObject())),
                lean(1, volatile = JSONObject().put("pending", JSONObject().put("c1", JSONArray().put("x")))),
                lean(2, view = JSONObject().put("a", 2)),
            ),
        )
        val views: MutableList<JSONObject> = Collections.synchronizedList(mutableListOf())
        val driver = CoreDriver(
            bridge, scope, perform = { "{}" }, onView = { views += it },
            escapedFailure = { _, _ -> "{}" }, includeVolatile = true,
        )
        repeat(3) { driver.dispatch("""{"n":$it}""") }
        withTimeout(10_000) { while (views.size < 3) delay(5) }
        assertEquals(true, bridge.optedIn)
        assertEquals("""{"a":1,"pending":{}}""", views[0].toString())
        assertEquals("""{"a":1,"pending":{"c1":["x"]}}""", views[1].toString())
        // The view moved alone: the volatile fields held are still there.
        assertEquals("""{"a":2,"pending":{"c1":["x"]}}""", views[2].toString())
    }

    @Test
    fun `an effect answer that moved nothing draws nothing and counts nothing`() = runBlocking {
        val effect = JSONObject().put("id", 1).put("operation", JSONObject().put("type", "x"))
        val bridge = ScriptedBridge(
            listOf(lean(1, view = JSONObject().put("a", 1), effects = JSONArray().put(effect))),
            resolved = JSONObject().put("view_version", 1),
        )
        val views: MutableList<String> = Collections.synchronizedList(mutableListOf())
        var unchanged = 0
        val driver = CoreDriver(
            bridge, scope, perform = { "{}" }, onView = { views += it.toString() },
            escapedFailure = { _, _ -> "{}" }, onUnchanged = { unchanged++ },
        )
        driver.dispatch("""{"n":0}""")
        withTimeout(10_000) { while ("resolve" !in bridge.calls) delay(5) }
        delay(50)
        assertEquals(listOf("""{"a":1}"""), views.toList())
        assertEquals(0, unchanged)
    }

    @Test
    fun `a classic answer is read as it always was`() = runBlocking {
        val bridge = object : CoreBridge {
            override fun dispatch(eventJson: String) = """{"view":{"a":7},"effects":[]}"""
            override fun resolveEffect(effectId: ULong, resultJson: String) = """{"view":{"a":8},"effects":[]}"""
            override fun view() = "{}"
        }
        val views: MutableList<String> = Collections.synchronizedList(mutableListOf())
        val driver = CoreDriver(bridge, scope, perform = { "{}" }, onView = { views += it.toString() }, escapedFailure = { _, _ -> "{}" })
        driver.dispatch("{}")
        driver.dispatch("{}")
        withTimeout(10_000) { while (views.size < 2) delay(5) }
        assertEquals(listOf("""{"a":7}""", """{"a":7}"""), views.toList())
    }

    @Serializable
    data class AView(val a: Int = 0)

    /** The send's quote wait: `applied(n)` must come true for an event whose view did not move. */
    @Test
    fun `an event that moved nothing is applied all the same`() = runBlocking {
        val bridge = ScriptedBridge(listOf(lean(1, view = JSONObject().put("a", 1)), lean(1)))
        val host = CoreHost(
            bridge = bridge, scope = scope, initial = AView(), serializer = AView.serializer(),
            perform = { "{}" }, escapedFailure = { _, _ -> "{}" },
        )
        host.start()
        host.dispatch("""{"n":0}""")
        val second = host.dispatchNumbered(
            mapOf("n" to 1),
            kotlinx.serialization.builtins.MapSerializer(kotlinx.serialization.serializer<String>(), kotlinx.serialization.serializer<Int>()),
        )
        withTimeout(10_000) { while (!host.applied(second)) delay(5) }
        assertTrue(host.applied(second))
        assertEquals(AView(1), host.view.value)
        assertFalse(host.viewJson.value.isNullOrEmpty())
    }

    /** The real bridge: an opted-in machine answers with `view_version`, and a repeated answer without `view`. */
    @Test
    fun `the core's own machines answer the lean shape once opted in`() {
        val core = uniffi.vela_core_uniffi.RpcPoolCore()
        core.skipUnchangedViews(false)
        val first = JSONObject(core.dispatch("""{"type":"invalidate_all"}"""))
        assertTrue(first.has("view_version"))
        assertTrue("the first answer after opting in carries the view", first.has("view"))
        assertFalse("pending_urls is volatile: not sent to a shell that did not ask", first.getJSONObject("view").has("pending_urls"))
        val again = JSONObject(core.dispatch("""{"type":"invalidate_all"}"""))
        assertFalse("nothing moved: no view", again.has("view"))
        assertEquals(first.getLong("view_version"), again.getLong("view_version"))
    }
}
