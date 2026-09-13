package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The pool, end to end, on the real `rpc_pool` machine.
 *
 * Every routing rule under test here belongs to Rust. What is being checked is
 * that the Kotlin half feeds it honestly: that a dead endpoint is reported as
 * dead rather than as a timeout, that a 429 keeps its own identity all the way
 * to the caller, and that the ban the core decides on reaches the disk in the
 * shape the other clients read.
 */
class RpcPoolMachineTest {

    /**
     * Cancelled after every test.
     *
     * A pool keeps a driver alive for the life of its scope, and a suite that
     * leaves six of them running competes with itself for `Dispatchers.Default`
     * — which showed up immediately as a 040 test timing out on a ten-second
     * budget it had never come close to before. A leaked scope is not tidiness;
     * it is a flake in somebody else's test.
     */
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryPool() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun pool(
        store: KeyValueStore,
        urls: List<String>,
        answer: (String, String) -> app.getvela.wallet.feature.wallet.core.RpcPostResult,
    ): Pair<RpcPool, FakeRpcTransport> {
        val transport = FakeRpcTransport(answer)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val pool = RpcPool(
            store = store,
            endpoints = FakeEndpointSource(urls),
            scope = scope,
            transport = transport,
        )
        return pool to transport
    }

    @Test
    fun aHealthyEndpointAnswers() = runBlocking {
        val (pool, _) = pool(FakeStore(), listOf(GOOD)) { _, _ ->
            FakeRpcTransport.body("0x2a")
        }
        pool.start()

        val result = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }
        val body = result as RpcResult.Body
        assertEquals("0x2a", body.json.optString("result"))
    }

    @Test
    fun aDeadEndpointIsRoutedAround() = runBlocking {
        // The first endpoint refuses every connection; the second works. The
        // caller must never learn that happened.
        val (pool, transport) = pool(FakeStore(), listOf(DEAD, GOOD)) { url, _ ->
            if (url == DEAD) FakeRpcTransport.network() else FakeRpcTransport.body("0x7")
        }
        pool.start()

        val result = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }
        assertEquals("0x7", (result as RpcResult.Body).json.optString("result"))
        assertTrue("the dead endpoint was tried", transport.asked.contains(DEAD))
        assertTrue("and the good one answered", transport.asked.contains(GOOD))
    }

    @Test
    fun aRateLimitKeepsItsIdentityAllTheWayToTheCaller() = runBlocking {
        // 429 is not "this chain is broken". A screen that treats them the same
        // tells a person their wallet is down when it is busy — the exact
        // confusion the core's separate verdict exists to prevent.
        val (pool, _) = pool(FakeStore(), listOf(GOOD)) { _, _ ->
            FakeRpcTransport.httpError(429)
        }
        pool.start()

        val result = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }
        val failed = result as RpcResult.Failed
        assertTrue("a 429 must arrive as rate-limited, not as a failure", failed.rateLimited)
    }

    @Test
    fun aRealFailureIsNotReportedAsRateLimited() = runBlocking {
        val (pool, _) = pool(FakeStore(), listOf(DEAD)) { _, _ -> FakeRpcTransport.network() }
        pool.start()

        val result = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }
        assertEquals(false, (result as RpcResult.Failed).rateLimited)
    }

    @Test
    fun aBanReachesDiskInTheSharedShape() = runBlocking {
        // **401, not a network error.** The first version of this test used a
        // refused connection and timed out waiting for a ban that was never
        // coming: the core treats an unreachable endpoint as a COOLDOWN, and
        // bans only what answers 401/403/404 — an endpoint that is up and
        // telling us we may not use it. That distinction is the core's, and
        // asserting the wrong half of it was my assumption, not its behaviour.
        val store = FakeStore()
        val (pool, _) = pool(store, listOf(REFUSING, GOOD)) { url, _ ->
            if (url == REFUSING) FakeRpcTransport.httpError(401) else FakeRpcTransport.body("0x1")
        }
        pool.start()
        withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }

        // The write is an effect the core emits after concluding, so the bytes
        // arrive strictly after the caller does — await the store, never the
        // return value. (The lesson spec 040 learned three times.)
        val raw = withTimeout(TIMEOUT) {
            var value = store.values[KeyValueStore.Keys.RPC_BANNED]
            while (value == null) {
                delay(20)
                value = store.values[KeyValueStore.Keys.RPC_BANNED]
            }
            value
        }

        val bans = JSONArray(raw)
        assertTrue("something was banned", bans.length() > 0)
        val first = bans.getJSONObject(0)
        // `{url, bannedAt, permanent}` under `vela.rpc.banned` — the same key
        // and the same field names the web and desktop clients read.
        assertTrue(first.has("url"))
        assertTrue(first.has("bannedAt"))
        assertTrue(first.has("permanent"))
    }

    @Test
    fun rememberedBansAreHandedBackToTheMachine() = runBlocking {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.RPC_BANNED to JSONArray().put(
                    org.json.JSONObject()
                        .put("url", DEAD)
                        .put("bannedAt", System.currentTimeMillis())
                        .put("permanent", false),
                ).toString(),
            ),
        )
        val (pool, transport) = pool(store, listOf(DEAD, GOOD)) { url, _ ->
            if (url == DEAD) FakeRpcTransport.network() else FakeRpcTransport.body("0x1")
        }
        pool.start()

        val result = withTimeout(TIMEOUT) { pool.call(CHAIN, "eth_blockNumber") }
        assertEquals("0x1", (result as RpcResult.Body).json.optString("result"))
        // The whole point of persisting a ban: a restart must not re-learn it
        // the hard way, at the cost of a request and a person's time.
        assertTrue(
            "a banned endpoint must not be tried again after a restart",
            !transport.asked.contains(DEAD),
        )
    }

    private companion object {
        const val CHAIN = 100
        const val GOOD = "https://good.example/rpc"
        const val DEAD = "https://dead.example/rpc"

        /** Up, reachable, and refusing us — the only thing the core bans. */
        const val REFUSING = "https://refusing.example/rpc"
        const val TIMEOUT = 20_000L
    }
}
