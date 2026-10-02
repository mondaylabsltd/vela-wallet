package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.CoreHost
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonPrimitive
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `CoreHost.applied`: "the core has applied THIS event", which counting commits
 * from the dispatch is not.
 *
 * The 2026-10-01 CI flake (`SendMachineTest`, PRs #346/#351). The send's
 * pre-check asked the fee session its question while the warm-up's last answer
 * was already queued in that session's inbox. The driver applies its inbox in
 * order, so that answer was committed AFTER the dispatch and BEFORE the
 * question: the pre-check's wait — "a commit after the dispatch, not busy, with
 * a fee" — took the warm-up's view, read the view again, found its own question
 * just begun (`busy`, no fee), and the send said "estimate failed". The person
 * would see the same on a Continue tapped as the warm-up lands.
 *
 * No machine, no uniffi: a bridge that labels each view with the turn that made
 * it, and two latches that park the driver's one consumer exactly between the
 * turn queued ahead and the event — the window a loaded runner opens by chance.
 */
class CoreHostAppliedTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    /**
     * Every view says which turn produced it. `hold` parks the consumer inside
     * the core until let go; `first` asks for one effect.
     */
    private class LabellingBridge : CoreBridge {
        val holding = CountDownLatch(1)
        val letGo = CountDownLatch(1)

        override fun dispatch(eventJson: String): String {
            val name = JSONObject(eventJson).getString("name")
            if (name == "hold") {
                holding.countDown()
                letGo.await(10, TimeUnit.SECONDS)
            }
            val result = JSONObject().put("view", JSONObject().put("label", name))
            if (name == "first") {
                result.put("effects", JSONArray().put(JSONObject().put("id", 1).put("operation", JSONObject().put("park", true))))
            }
            return result.toString()
        }

        override fun resolveEffect(effectId: ULong, resultJson: String): String =
            JSONObject().put("view", JSONObject().put("label", "answer")).toString()

        override fun view(): String = JSONObject().put("label", "initial").toString()
    }

    private fun event(name: String) = JsonObject(mapOf("name" to JsonPrimitive(name)))

    private val CoreHost<JsonObject>.label: String? get() = view.value["label"]?.jsonPrimitive?.content

    @Test
    fun `a view committed for a turn queued ahead of the event is not after it`() = runBlocking {
        val bridge = LabellingBridge()
        val parked = CountDownLatch(1)
        val release = CountDownLatch(1)
        val host = CoreHost(
            bridge = bridge,
            scope = scope,
            initial = JsonObject(emptyMap()),
            serializer = JsonObject.serializer(),
            // An effect starts on the consumer's own thread and runs until it
            // first suspends — this one blocks, so the consumer stops right
            // after committing `first`'s view.
            perform = { _ ->
                parked.countDown()
                release.await(10, TimeUnit.SECONDS)
                "{}"
            },
            escapedFailure = { _, _ -> "{}" },
        )

        host.dispatch(event("hold"), JsonObject.serializer())
        assertTrue("the consumer took `hold`", bridge.holding.await(10, TimeUnit.SECONDS))
        // Queued ahead of ours: the last question's answer, in the flake.
        host.dispatch(event("first"), JsonObject.serializer())
        val before = host.commits.value
        val ours = host.dispatchNumbered(event("second"), JsonObject.serializer())

        bridge.letGo.countDown()
        assertTrue("the consumer parked after `first`", parked.await(10, TimeUnit.SECONDS))

        // The window: a view committed after the dispatch, about something else.
        assertTrue("committed after the dispatch", host.commits.value > before)
        assertEquals("first", host.label)
        assertFalse("but not after the event: `second` is still queued", host.applied(ours))

        release.countDown()
        val view = withTimeout(10_000) {
            host.commits.first { host.applied(ours) }
            host.label
        }
        assertNotEquals("hold", view)
        assertNotEquals("first", view)
    }

    @Test
    fun `an event's view counts as applied the moment it is committed`() = runBlocking {
        val bridge = LabellingBridge()
        bridge.letGo.countDown()
        val host = CoreHost(
            bridge = bridge,
            scope = scope,
            initial = JsonObject(emptyMap()),
            serializer = JsonObject.serializer(),
            perform = { "{}" },
            escapedFailure = { _, _ -> "{}" },
        )
        repeat(50) { round ->
            val number = host.dispatchNumbered(event("e$round"), JsonObject.serializer())
            val label = withTimeout(10_000) {
                host.commits.first { host.applied(number) }
                host.label
            }
            assertEquals("e$round", label)
        }
    }
}
