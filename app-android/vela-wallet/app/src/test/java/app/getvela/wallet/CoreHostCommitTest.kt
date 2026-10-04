package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreBridge
import app.getvela.wallet.core.crux.CoreHost
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.yield
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.concurrent.Executors

/**
 * A view field that is an instruction rather than a state — the sign
 * machine's tracker hand-off — has to be read from EVERY commit, which
 * `CoreHost.onCommit` is and `CoreHost.view` is not.
 *
 * `DappSignMachineTest`'s transfer failed now and then (`handed.first()`
 * named the relay's record): the write-ahead's hand-off rode one view and the
 * relay's replaced it within the same POST, and the controller read hand-offs
 * by collecting the `StateFlow` — which hands a collector scheduled late only
 * the newest view. Here the late collector is made certain: the driver's loop
 * and the collector share one thread, and the loop commits five views before
 * it yields it.
 */
class CoreHostCommitTest {
    @Test
    fun `onCommit sees every view in order where a late collector of the view sees only the last`() = runBlocking {
        val thread = Executors.newSingleThreadExecutor()
        val dispatcher = thread.asCoroutineDispatcher()
        val scope = CoroutineScope(SupervisorJob() + dispatcher)
        try {
            var n = 0
            val bridge = object : CoreBridge {
                override fun dispatch(eventJson: String): String = """{"view":{"n":${++n}},"effects":[]}"""
                override fun resolveEffect(effectId: ULong, resultJson: String): String = """{"view":{"n":$n},"effects":[]}"""
                override fun view(): String = """{"n":$n}"""
            }
            fun JsonElement.n() = jsonObject["n"]?.jsonPrimitive?.int
            val committed = ArrayList<Int?>()
            val collected = ArrayList<Int?>()
            withContext(dispatcher) {
                // Built on the loop's own thread, so nothing below runs until
                // this block yields it: the loop, then the collector, in the
                // order they were launched.
                val host = CoreHost<JsonElement>(
                    bridge = bridge, scope = scope, initial = JsonObject(emptyMap()),
                    serializer = JsonElement.serializer(),
                    perform = { "{}" }, escapedFailure = { _, _ -> "{}" },
                    onCommit = { committed += it.n() },
                )
                scope.launch { host.view.collect { collected += it.n() } }
                repeat(5) { host.dispatch("{}") }
                yield()
                assertEquals("every commit, in the core's order", listOf(1, 2, 3, 4, 5), committed)
                assertEquals("the view, collected late: only the last", listOf<Int?>(5), collected)
            }
        } finally {
            scope.cancel()
            thread.shutdown()
        }
    }
}
