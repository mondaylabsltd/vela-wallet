package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.SessionController
import com.sun.net.httpserver.HttpServer
import java.net.InetSocketAddress
import java.util.Collections
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Issue #409, on the REAL session machine through the host dylib, the real
 * executor, and a real local registry that answers `/api/task/{id}` as the test
 * tells it.
 *
 * A one-key wallet is entered at the registry's 202 and its pending record is
 * left behind with the task id. The session confirms the landing — a read of
 * that task, never a passkey — and only then removes the record. A landing
 * that fails leaves the record, and with it the sign-out warning.
 */
class LandingWatchTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private var server: HttpServer? = null
    private val asked: MutableList<String> = Collections.synchronizedList(mutableListOf())

    @After
    fun tearDown() {
        server?.stop(0)
        scope.cancel()
    }

    private val account = """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        "public_key_hex":"04ab","created_at_iso":"2026-10-04T10:00:00.000Z",
        "keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}"""

    private fun record(task: String) = JSONObject()
        .put("id", "cred-1")
        .put("name", "Ann")
        .put("public_key_hex", "04ab")
        .put("attestation_object_hex", "a0")
        .put("created_at_iso", "2026-10-04T10:00:00.000Z")
        .put("members", JSONArray())
        .put("task_id", task)

    /** A registry whose task endpoint answers `body` — and records who asked. */
    private fun registry(body: String): String {
        val http = HttpServer.create(InetSocketAddress("127.0.0.1", 0), 0)
        http.createContext("/api/task/") { exchange ->
            asked += exchange.requestURI.path
            val bytes = body.toByteArray()
            exchange.sendResponseHeaders(200, bytes.size.toLong())
            exchange.responseBody.use { it.write(bytes) }
        }
        http.createContext("/") { exchange ->
            asked += exchange.requestURI.path
            exchange.sendResponseHeaders(404, -1)
            exchange.close()
        }
        http.start()
        server = http
        return "http://127.0.0.1:${http.address.port}"
    }

    private suspend fun seeded(registryUrl: String, task: String): FakeStore {
        val store = FakeStore(
            mapOf(
                "vela.accounts" to "[$account]",
                "vela.activeAccountIndex" to "0",
                // Something an older build or a test left behind: it must cost
                // only itself.
                "vela.pendingUploads" to JSONArray().put(JSONObject().put("unconfirmed", true)).put(record(task)).toString(),
            ),
        )
        AccountStore(store).saveRegistryUrl(registryUrl)
        return store
    }

    private fun outbox(store: FakeStore): String = store.values["vela.pendingUploads"].orEmpty()

    @Test
    fun `a landing confirmed at launch removes the record with nothing but a read`() = runBlocking {
        val url = registry("""{"id":"t1","status":"done","txHash":"0xabc"}""")
        val store = seeded(url, "t1")
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading && it.hasWallet } }

        withTimeout(10_000) {
            while (outbox(store).contains("cred-1")) delay(50)
        }
        assertEquals(listOf("/api/task/t1"), asked.toList())
        assertTrue("the unreadable record was left alone: ${outbox(store)}", outbox(store).contains("unconfirmed"))
    }

    @Test
    fun `a failed landing keeps the record and the sign-out warning`() = runBlocking {
        val url = registry("""{"id":"t2","status":"failed","error":"execution reverted"}""")
        val store = seeded(url, "t2")
        val session = SessionController(AccountStore(store), scope)
        session.boot()
        withTimeout(10_000) { session.view.first { !it.loading && it.hasWallet } }
        withTimeout(10_000) {
            while (asked.isEmpty()) delay(50)
        }
        // The watch has its answer; give the (ignored) removal a chance to be wrong.
        delay(300)
        assertTrue("the record stays: ${outbox(store)}", outbox(store).contains("cred-1"))

        session.signOut()
        val sheet = withTimeout(10_000) { session.view.first { it.signOut != null } }.signOut!!
        assertTrue(sheet.pendingUploadWarning)
    }
}
