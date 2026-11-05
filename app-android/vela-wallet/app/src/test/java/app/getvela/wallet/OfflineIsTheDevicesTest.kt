package app.getvela.wallet

import app.getvela.wallet.core.net.NetHealth
import app.getvela.wallet.feature.wallet.core.NetHealthFeed
import app.getvela.wallet.feature.wallet.core.RpcCallVerdict
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcOperation
import app.getvela.wallet.feature.wallet.core.RpcPayload
import app.getvela.wallet.feature.wallet.core.RpcPoolCallRegistry
import app.getvela.wallet.feature.wallet.core.RpcPoolExecutor
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcTransport
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * r14-3: "Offline" is said only when the device has no network. With every
 * node of Arc Testnet hung behind the device's proxy (each connect timing out
 * at 8 s) and a slow public node of another network doing the same, the home
 * flipped between "Can't reach Arc Testnet right now" and "Offline" every
 * 8–16 s while every other network answered: the transport's connect timeout
 * is `not_connected` for delivery, and the executor fed that verdict to the
 * network's health as a miss. A timeout — of the connect or of the answer —
 * says nothing about the device's network (the core's `net_health` rule).
 */
class OfflineIsTheDevicesTest {
    private var nowMs = 1_000_000.0

    @Before fun fresh() {
        NetHealth.reset()
        NetHealth.clock = { nowMs }
    }

    @After fun clean() = NetHealth.reset()

    private class Scripted(var next: RpcPostResult) : RpcTransport {
        override suspend fun post(url: String, method: String, params: List<Any?>, xRpcUrl: String?, timeoutMs: Int): RpcPostResult = next
    }

    private val transport = Scripted(RpcPostResult(RpcTransportOutcome.NotConnected, failure = "connect_timeout"))

    /** Every call is chain [chainOf]'s, by its id. */
    private val registry = object : RpcPoolCallRegistry {
        override fun payload(callId: String): RpcPayload? = RpcPayload("eth_call", emptyList(), callId.substringBefore(':').toInt())
        override fun keepBody(callId: String, url: String, body: JSONObject) = Unit
        override fun settle(callId: String, verdict: RpcCallVerdict) = Unit
    }

    private val executor = RpcPoolExecutor(
        FakeStore(),
        registry,
        object : RpcEndpointSource { override suspend fun forChain(chainId: Int) = RpcSeeds() },
        transport = transport,
        now = { nowMs },
    )

    /** One POST to [chain], [afterMs] after the last. */
    private fun post(chain: Int, afterMs: Double = 4_000.0) = runBlocking {
        nowMs += afterMs
        executor.perform(RpcOperation.JsonRpcPost(call_id = "$chain:${nowMs.toLong()}", url = "https://node.example/$chain", method = "eth_call", timeout_ms = 8_000))
    }

    @Test
    fun `hung nodes of two networks, their connects timing out, never make the device offline`() {
        // Arc (5042002) hung behind the proxy, a slow Optimism Sepolia node too —
        // for a minute, nothing else read in between.
        repeat(15) {
            post(5042002)
            post(11155420)
        }
        assertTrue("a timed-out connect is not a missing network", NetHealth.online.value)
    }

    @Test
    fun `a device with no network is still offline`() {
        transport.next = RpcPostResult(RpcTransportOutcome.NotConnected, failure = "dns")
        post(5042002)
        post(11155420, afterMs = 6_000.0)
        post(5042002, afterMs = 6_000.0)
        assertFalse("three misses from two networks, nothing answered for twelve seconds", NetHealth.online.value)
        transport.next = RpcPostResult(RpcTransportOutcome.Response(null), body = JSONObject())
        post(1, afterMs = 1_000.0)
        assertTrue("the first answer is back", NetHealth.online.value)
    }

    @Test
    fun `refused connections still count, as before`() {
        transport.next = RpcPostResult(RpcTransportOutcome.NotConnected, failure = "connect_refused")
        post(5042002)
        post(11155420, afterMs = 6_000.0)
        post(5042002, afterMs = 6_000.0)
        assertFalse(NetHealth.online.value)
    }

    @Test
    fun `what each POST tells the network's health`() {
        assertEquals(true, NetHealthFeed.of(RpcTransportOutcome.Response(null), null))
        assertEquals(true, NetHealthFeed.of(RpcTransportOutcome.HttpError(429), null))
        assertEquals(true, NetHealthFeed.of(RpcTransportOutcome.NonJson, null))
        assertNull(NetHealthFeed.of(RpcTransportOutcome.Timeout, "read_timeout"))
        assertNull(NetHealthFeed.of(RpcTransportOutcome.NotConnected, "connect_timeout"))
        assertNull(NetHealthFeed.of(RpcTransportOutcome.Timeout, "timeout"))
        assertEquals(false, NetHealthFeed.of(RpcTransportOutcome.NotConnected, "dns"))
        assertEquals(false, NetHealthFeed.of(RpcTransportOutcome.NotConnected, "connect_refused"))
        assertEquals(false, NetHealthFeed.of(RpcTransportOutcome.NotConnected, "tls"))
        assertEquals(false, NetHealthFeed.of(RpcTransportOutcome.Network, "after_write"))
        // A transport that names no class keeps the verdict's meaning.
        assertEquals(false, NetHealthFeed.of(RpcTransportOutcome.NotConnected, null))
    }
}
