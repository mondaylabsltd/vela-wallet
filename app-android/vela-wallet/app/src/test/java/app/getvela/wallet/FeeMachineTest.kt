package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEvent
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeOperation
import app.getvela.wallet.feature.send.core.FeeShellResult
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.FeePolicyCore

/**
 * The real `fee_policy` machine through JNA, priced by a scripted relay
 * (spec 043 T026). What the shell does here is fetch six numbers; the estimate
 * the confirm screen renders is the core's, and this pins that the arms hand
 * the core what it asked for and that a re-quote after a fee-token pick
 * replaces the estimate rather than keeping a stale one.
 */
class FeeMachineTest {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val port = FakeRelayPort()
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    @After
    fun stop() = scope.cancel()

    private fun host(): CoreHost<FeeView> {
        val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)
        val executor = FeeExecutor(relay, keyHexes = { emptyList() })
        return CoreHost(
            bridge = FeePolicyCore().asBridge(),
            scope = scope,
            initial = FeeView(),
            serializer = FeeView.serializer(),
            perform = JsonShell.perform(FeeOperation.serializer(), FeeShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(FeeOperation.serializer(), FeeShellResult.serializer(), FeeShellResult.TtlElapsed, executor::neutralAnswer),
        ).also { it.start() }
    }

    /** A Gnosis-shaped relay: native xDAI quotes, a fast tier, an estimate. */
    private fun scriptRelay() {
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") } // 1 gwei
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") } // nonce 7
        port.always("pimlico_getUserOperationGasPrice") {
            FakeRelayPort.body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", "0x77359400").put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(
                JSONArray()
                    .put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1"))
                    .put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "erc20").put("feeToken", "0x3333333333333333333333333333333333333333").put("balance", "0x4c4b40").put("decimals", 6).put("symbol", "USDC").put("usdBalance", "5").put("usdPrice", "1")),
            )
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(
            JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"),
        )
    }

    private fun request(feeToken: String? = null) = FeeEvent.QuoteRequested(
        chain_id = 100,
        account = safe,
        deployed = true,
        public_key_available = true,
        tier = FeeTier.Fast,
        calls = listOf(FeeCall(to = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", value = "1000000000000000", data = "0x")),
        fee_token = feeToken,
    )

    @Test
    fun `a quote settles from the six arms and names the native asset`() = runBlocking {
        scriptRelay()
        val host = host()
        host.dispatch(request(), FeeEvent.serializer())
        val settled = withTimeout(15_000) { host.view.first { !it.busy && (it.fee != null || it.failed != null) } }
        val fee = settled.fee
        assertNotNull("the relay answered every arm, so the core must price", fee)
        assertEquals(100, fee!!.chain_id)
        assertTrue(fee.fee_asset is FeeAssetView.Native)
        assertTrue(fee.total_wei.toBigInteger() > java.math.BigInteger.ZERO)
        assertTrue(fee.quoted)
        // The estimate travelled as decimal strings and the core sized the gas.
        assertTrue(port.calls.any { it.endsWith("eth_estimateUserOperationGas") })
        assertTrue(settled.options.any { it.symbol == "USDC" })
    }

    @Test
    fun `picking a fee token re-quotes in that token`() = runBlocking {
        scriptRelay()
        val host = host()
        host.dispatch(request(), FeeEvent.serializer())
        val trace = scope.launch { host.view.collect { println("fee view: busy=${it.busy} failed=${it.failed} fee=${it.fee?.fee_asset} token=${it.fee_token} options=${it.options.map { o -> o.symbol }}") } }
        val first = withTimeout(15_000) { host.view.first { !it.busy && it.fee != null } }.fee!!
        // The send machine answers a fee-token pick by asking for a NEW quote
        // with the token named (`ChooseFeeToken` → `EstimateFee`); the fee
        // session's own `SelectFeeAsset` records the choice for that request.
        host.dispatch(FeeEvent.SelectFeeAsset("0x3333333333333333333333333333333333333333"), FeeEvent.serializer())
        host.dispatch(request(feeToken = "0x3333333333333333333333333333333333333333"), FeeEvent.serializer())
        val settled = withTimeout(15_000) { host.view.first { !it.busy && (it.fee !== first || it.failed != null) } }
        println("fee re-quote settled: failed=${settled.failed} fee=${settled.fee?.fee_asset} options=${settled.options.map { it.symbol to it.selected }}")
        val second = settled.fee!!
        val asset = second.fee_asset
        assertTrue("the fee is now in the stablecoin", asset is FeeAssetView.Erc20)
        // The estimate names the token by address and decimals; the symbol is
        // the options row's, and the sheet reads it from there.
        assertEquals("0x3333333333333333333333333333333333333333", (asset as FeeAssetView.Erc20).token.lowercase())
        assertEquals(6, asset.decimals)
        assertTrue(settled.options.single { it.symbol == "USDC" }.selected)
        trace.cancel()
    }

    @Test
    fun `a relay that answers nothing is a failure the core names, not a spinner`() = runBlocking {
        val host = host()
        host.dispatch(request(), FeeEvent.serializer())
        val settled = withTimeout(15_000) { host.view.first { !it.busy && (it.fee != null || it.failed != null) } }
        assertNotNull("no quotes, no estimate: the core must say which failure", settled.failed)
    }
}
