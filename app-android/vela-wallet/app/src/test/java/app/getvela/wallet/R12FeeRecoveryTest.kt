package app.getvela.wallet

import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * r12-6 (round 12): after a relay outage the multisig's Execute said
 * "Couldn't work out the network fee right now" for about two minutes,
 * through Retry and Check again, while the fee machine logged quotes; only a
 * force-stop priced it. The multisig's look asks its own fee session
 * (`MultisigLive.quoteCalls` → [SpeedControl.quote]) once per look.
 *
 * Pinned here over the REAL fee core: a session that failed through a long
 * outage — every relay read refused, the core's own re-asks out and failed
 * too — answers the first question after the relay is back with THAT
 * question's quote, at once: no cached failure, no wait on the core's
 * back-off. (On the device the two minutes were the transport: every relay
 * call met a dead pooled connection first — see R12ShellFixesTest and
 * `VelaHttp`.)
 */
class R12FeeRecoveryTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() = scopes.forEach { it.cancel() }

    private val port = FakeRelayPort()

    @Volatile
    private var relayUp = true

    private fun relay(answer: () -> RpcResult): (List<Any?>) -> RpcResult = { if (relayUp) answer() else RpcResult.Failed(rateLimited = false) }

    private fun script() {
        val wei = "0x" + 1_000_000_000L.toString(16)
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body(wei) }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", wei)) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice", relay {
            FakeRelayPort.body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", wei).put("networkFeePerGas", wei).put("relayerFeePerGas", wei)))
        })
        port.always("vela_getInBandGasQuote", relay {
            FakeRelayPort.body(
                JSONArray().put(
                    JSONObject().put("recipient", RECIPIENT).put("asset", "native").put("balance", "0x9f3306a949ca000")
                        .put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1"),
                ),
            )
        })
        port.always("eth_estimateUserOperationGas", relay {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        })
    }

    private fun control(): SpeedControl {
        script()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val relay = RelayClient(port, builtinBase = { "https://relay.test" }, retryDelayMs = 0)
        return SpeedControl(scope, relay, FeeExecutor(relay, keyHexes = { emptyList() }), { FeeTier.Fast }, { "comma_dot" }, area = "multisig").start()
    }

    private val calls = listOf(FeeCall(to = RECIPIENT, value = "1000", data = "0x"))

    private fun SpeedControl.look(): SpeedControl.Quoted = runBlocking { withTimeout(35_000) { quote(100, SAFE, true, calls, null) } }

    @Test
    fun the_first_look_after_a_relay_outage_is_priced_by_its_own_quote() {
        val fees = control()
        assertNotNull("priced before the outage", (fees.look() as SpeedControl.Quoted.Settled).view.fee)

        // The outage: look after look fails (the sheet's Check again, the
        // machine's 5 s re-look), and the core's own re-asks fail beside them.
        // (Readings the core holds for a while may still price the first
        // looks; the outage is on once a look fails.)
        relayUp = false
        var failures = 0
        val outage = System.currentTimeMillis()
        while (failures < 5) {
            assertTrue("no look failed in 60 s of outage", System.currentTimeMillis() - outage < 60_000)
            val look = fees.look()
            assertTrue("a look during the outage: $look", look is SpeedControl.Quoted.Settled)
            if ((look as SpeedControl.Quoted.Settled).view.failed != null) failures++ else assertTrue(failures == 0)
            Thread.sleep(300)
        }

        // Back: the very next look is priced, and promptly — never the outage's failure.
        relayUp = true
        val started = System.currentTimeMillis()
        val back = fees.look()
        val took = System.currentTimeMillis() - started
        assertTrue("$back", back is SpeedControl.Quoted.Settled)
        back as SpeedControl.Quoted.Settled
        assertNull("the first look after the outage answered with its failure: ${back.view.failed}", back.view.failed)
        assertNotNull(back.view.fee)
        assertTrue("priced in ${took} ms — never after the core's back-off", took < 5_000)
        // And it stays priced, look after look.
        repeat(3) { assertNotNull((fees.look() as SpeedControl.Quoted.Settled).view.fee) }
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
    }
}
