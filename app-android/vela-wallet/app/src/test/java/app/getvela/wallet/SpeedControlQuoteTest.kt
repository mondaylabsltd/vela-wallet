package app.getvela.wallet

import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SpeedControl
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * `SpeedControl.quote` — the wait the send machine's `estimate_fee` hangs on —
 * over the REAL fee core with a scripted relay.
 *
 * Two ways that wait has gone wrong: a re-quote at the same price is a view
 * that `equals` the one before, and a StateFlow never re-delivers an equal
 * value (the 2026-09-19 CI flake — hence waiting on `commits`); and the
 * commit counter REPLAYS its current value to a new collector, so a view
 * still carrying the last attempt's failure answered a new question before
 * the core had even seen it (spec 078: a Max after a failed warm-up got the
 * stale failure in milliseconds). These pin both.
 */
class SpeedControlQuoteTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() = scopes.forEach { it.cancel() }

    private val port = FakeRelayPort()

    @Volatile
    private var inBandUp = true

    private fun script() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            FakeRelayPort.body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", "0x77359400").put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")))
        }
        port.always("vela_getInBandGasQuote") {
            if (!inBandUp) {
                FakeRelayPort.error("upstream unavailable")
            } else {
                FakeRelayPort.body(
                    JSONArray().put(
                        JSONObject().put("recipient", RECIPIENT).put("asset", "native").put("balance", "0x9f3306a949ca000")
                            .put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1"),
                    ),
                )
            }
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
    }

    private fun control(): SpeedControl {
        script()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val relay = RelayClient(port, builtinBase = { "https://relay.test" }, retryDelayMs = 0)
        return SpeedControl(scope, relay, FeeExecutor(relay, keyHexes = { emptyList() }), { FeeTier.Fast }, { "comma_dot" }, area = "test").start()
    }

    private val calls = listOf(FeeCall(to = RECIPIENT, value = "1000", data = "0x"))

    private fun SpeedControl.ask(): SpeedControl.Quoted =
        runBlocking { withTimeout(10_000) { quote(100, SAFE, true, calls, null) } }

    @Test
    fun `a re-quote at the same price settles, though its view equals the last`() {
        val speed = control()
        val first = speed.ask() as SpeedControl.Quoted.Settled
        val fee = first.view.fee
        assertNotNull(fee)
        // The same question, the same readings (held 15 s), the same price:
        // every answer is a view `equal` to the one before it.
        repeat(8) {
            val again = speed.ask()
            assertTrue("re-quote $it: $again", again is SpeedControl.Quoted.Settled)
            assertEquals(fee, (again as SpeedControl.Quoted.Settled).view.fee)
        }
    }

    @Test
    fun `a question after a failed one gets its own answer, never the stale failure`() {
        val speed = control()
        inBandUp = false
        val failed = speed.ask() as SpeedControl.Quoted.Settled
        assertNotNull("the relay's rows are down: the quote fails", failed.view.failed)
        assertNull(failed.view.fee)
        // The relay is back; the next question must be answered by ITS quote.
        // The view the wait first sees still says `failed` — the replayed
        // commit — and used to answer it in milliseconds.
        inBandUp = true
        repeat(5) {
            val next = speed.ask() as SpeedControl.Quoted.Settled
            assertNull("attempt $it answered with the stale failure: ${next.view.failed}", next.view.failed)
            assertNotNull(next.view.fee)
        }
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
    }
}
