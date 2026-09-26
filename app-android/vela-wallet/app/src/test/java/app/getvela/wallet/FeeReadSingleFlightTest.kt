package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeEvent
import app.getvela.wallet.feature.send.core.FeeGasOutcome
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RelayPort
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SendOperation
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * "The three speed tiers' network fees appear one after another — it should
 * take ONE request and then compute" (founder, 2026-09-26).
 *
 * The session for the speed in force and a preview per other speed ask the
 * same reads at the same instant. What is pinned here: they share ONE request
 * per read and all get its answer at the same moment — the relay's gas quote
 * (one response answers every tier), the simulation of one exact operation,
 * the gas signals, the in-band rows, the deployment read — and the picker's
 * read-ahead fills the very caches the fee session reads.
 */
class FeeReadSingleFlightTest {

    /** A relay whose every answer takes a while, so askers overlap as they do on a phone. */
    private class SlowPort(private val answers: (String, List<Any?>) -> RpcResult) : RelayPort {
        val calls = CopyOnWriteArrayList<String>()

        override suspend fun call(chainId: Int, method: String, params: List<Any?>, kind: RpcKind): RpcResult {
            calls += "$chainId:$method"
            delay(50)
            return answers(method, params)
        }

        override suspend fun bundlerBase(chainId: Int): String? = "https://relay.test"

        override suspend fun bestRpcUrl(chainId: Int): String? = null

        override suspend fun restGet(url: String, xRpcUrl: String?): RestAnswer {
            calls += "GET:$url"
            delay(50)
            return RestAnswer.Ok(JSONObject().put("activeDepositAddress", RECIPIENT).put("status", "ACTIVE"))
        }
    }

    private fun body(result: Any?): RpcResult = RpcResult.Body(JSONObject().put("result", result))

    private val tiers = JSONObject()
        .put("slow", JSONObject().put("maxFeePerGas", "0x10").put("maxPriorityFeePerGas", "0x1"))
        .put("standard", JSONObject().put("maxFeePerGas", "0x20").put("maxPriorityFeePerGas", "0x2"))
        .put("fast", JSONObject().put("maxFeePerGas", "0x30").put("maxPriorityFeePerGas", "0x3"))

    private fun answers(method: String, @Suppress("UNUSED_PARAMETER") params: List<Any?>): RpcResult = when (method) {
        "pimlico_getUserOperationGasPrice" -> body(tiers)
        "eth_gasPrice" -> body("0x64")
        "eth_getBlockByNumber" -> body(JSONObject().put("baseFeePerGas", "0x32"))
        "eth_maxPriorityFeePerGas" -> body("0x1")
        "eth_getCode" -> body("0x6080")
        "vela_getInBandGasQuote" -> body(
            JSONArray().put(
                JSONObject().put("recipient", RECIPIENT).put("asset", "native").put("decimals", 18)
                    .put("symbol", "XDAI").put("balance", "0x10").put("usdBalance", "1").put("usdPrice", "1"),
            ),
        )
        else -> RpcResult.Failed(rateLimited = false)
    }

    private val port = SlowPort(::answers)
    private val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)

    private fun count(method: String) = port.calls.count { it.endsWith(method) }

    @Test
    fun `three speeds asking at once share one relay gas quote, each reading its own row`() = runBlocking<Unit> {
        val rows = listOf(FeeTier.Fast, FeeTier.Standard, FeeTier.Slow)
            .map { tier -> async { relay.bundlerQuote(100, tier) } }
            .awaitAll()
        assertEquals(1, count("pimlico_getUserOperationGasPrice"))
        assertEquals(listOf("48", "32", "16"), rows.map { it?.max_fee_per_gas })
        // …and the whole response is held: a tier asked later reads from memory.
        relay.bundlerQuote(100, FeeTier.Standard)
        assertEquals(1, count("pimlico_getUserOperationGasPrice"))
    }

    @Test
    fun `one exact operation is simulated once for every session asking, and a refresh simulates again`() = runBlocking<Unit> {
        val simulated = AtomicInteger()
        val calls = listOf(FeeCall(to = RECIPIENT, value = "1000", data = "0x"))
        val estimate: suspend () -> FeeGasOutcome = {
            simulated.incrementAndGet()
            delay(50)
            FeeGasOutcome.Estimated("1", "2", "3")
        }
        val outcomes = (1..3).map { async { relay.simulation(100, SAFE, true, calls, estimate) } }.awaitAll()
        assertEquals(1, simulated.get())
        assertEquals(1, outcomes.distinct().size)
        // Held for the fee-signal window…
        relay.simulation(100, SAFE.lowercase(), true, calls, estimate)
        assertEquals(1, simulated.get())
        // …a different operation is its own question…
        relay.simulation(100, SAFE, false, calls, estimate)
        assertEquals(2, simulated.get())
        // …and the refresh control (or a submit) drops it.
        relay.invalidateFeeSignals(100)
        relay.simulation(100, SAFE, true, calls, estimate)
        assertEquals(3, simulated.get())
    }

    @Test
    fun `a refused simulation is never held`() = runBlocking<Unit> {
        val simulated = AtomicInteger()
        val refuse: suspend () -> FeeGasOutcome = { simulated.incrementAndGet(); FeeGasOutcome.SimulationFailed }
        relay.simulation(100, SAFE, true, emptyList(), refuse)
        relay.simulation(100, SAFE, true, emptyList(), refuse)
        assertEquals(2, simulated.get())
    }

    @Test
    fun `gas signals, in-band rows and the deployment read are one request for simultaneous askers`() = runBlocking<Unit> {
        (1..3).map { async { relay.gasSignals(100, wantTip = true) } }.awaitAll()
        (1..3).map { async { relay.inBandQuotes(100, SAFE) } }.awaitAll()
        (1..3).map { async { relay.isDeployed(100, SAFE) } }.awaitAll()
        assertEquals(1, count("eth_gasPrice"))
        assertEquals(1, count("eth_getBlockByNumber"))
        assertEquals(1, count("eth_maxPriorityFeePerGas"))
        assertEquals(1, count("vela_getInBandGasQuote"))
        assertEquals(1, count("eth_getCode"))
    }

    @Test
    fun `a read asked after a refresh does not join the read that was already out`() = runBlocking<Unit> {
        val first = async { relay.bundlerQuote(100, FeeTier.Fast) }
        yield()
        relay.invalidateFeeSignals(100)
        val second = async { relay.bundlerQuote(100, FeeTier.Fast) }
        first.await()
        second.await()
        assertEquals(2, count("pimlico_getUserOperationGasPrice"))
    }

    @Test
    fun `the picker's read-ahead fills the caches the fee session reads`() = runBlocking<Unit> {
        relay.prewarmFees(SAFE, listOf(100, 4217), FeeTier.Fast)
        val before = port.calls.size
        // What a quote on those chains reads first — now all from memory.
        relay.isDeployed(100, SAFE)
        relay.gasSignals(100, wantTip = true)
        relay.bundlerQuote(100, FeeTier.Fast)
        relay.inBandQuotes(100, SAFE)
        relay.gasSignals(4217, wantTip = false)
        relay.accountInfo(4217, SAFE)
        relay.inBandQuotes(4217, SAFE)
        assertEquals("no read after the prewarm", before, port.calls.size)
        // Tempo reads its fee recipient, never the relay's gas quote.
        assertTrue(port.calls.none { it == "4217:pimlico_getUserOperationGasPrice" })
        assertTrue(port.calls.any { it.startsWith("GET:") && it.contains("/v1/account/4217/") })
    }

    @Test
    fun `a reader that is cancelled leaves the next asker to read for itself`() = runBlocking<Unit> {
        val flight = app.getvela.wallet.feature.send.core.SingleFlight<String, Int>()
        val started = CompletableDeferred<Unit>()
        val leader = launch { flight.run("k") { started.complete(Unit); delay(10_000); 1 } }
        started.await()
        val follower = async { flight.run("k") { 2 } }
        yield()
        leader.cancel()
        assertEquals(2, follower.await())
        assertEquals(0, flight.size)
    }

    @Test
    fun `the auto fee coin flag crosses the wire on the estimate and on the fee session`() {
        val ask = SpeedControl.QuoteAsk(100, SAFE, true, FeeTier.Fast, emptyList(), null, autoFeeToken = true)
        val event = Wire.json.encodeToString(FeeEvent.serializer(), ask.event(deployed = true))
        assertTrue(event, event.contains("\"auto_fee_token\":true"))
        // A chip tap's ask is the person's pick: false, and still on the wire.
        val picked = Wire.json.encodeToString(FeeEvent.serializer(), ask.copy(autoFeeToken = false).event(deployed = true))
        assertTrue(picked, picked.contains("\"auto_fee_token\":false"))
        val op = Wire.json.decodeFromString(
            SendOperation.serializer(),
            """{"type":"estimate_fee","chain_id":100,"account":"$SAFE","tx":null,"batch":null,"gas_fee_token":null,"public_key_hex":null,"auto_fee_token":true}""",
        )
        assertEquals(true, (op as SendOperation.EstimateFee).auto_fee_token)
        val prewarm = Wire.json.decodeFromString(SendOperation.serializer(), """{"type":"prewarm_fees","account":"$SAFE","chain_ids":[100,137]}""")
        assertEquals(listOf(100, 137), (prewarm as SendOperation.PrewarmFees).chain_ids)
        assertEquals("""{"type":"fees_prewarmed"}""", Wire.json.encodeToString(SendShellResult.serializer(), SendShellResult.FeesPrewarmed))
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RECIPIENT = "0x1111111111111111111111111111111111111111"
    }
}
