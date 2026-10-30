package app.getvela.wallet

import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SpeedControl
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
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The send flow's fee-coin pick (native parity Phase 3): the web's
 * `pickFeeToken` tells the fee session `select_fee_asset` as well as the send
 * machine `choose_fee_token`, and on the confirm step only the former moves
 * the fee — the send machine re-estimates on the form alone. Android told the
 * send machine only, so a coin picked on the confirm page changed nothing.
 *
 * Over the REAL fee core with a scripted relay: Gnosis, an account holding
 * XDAI and USDC, quoted in XDAI first.
 */
class SpeedControlFeeCoinTest {
    private val scopes = mutableListOf<CoroutineScope>()
    private val port = FakeRelayPort()

    @After
    fun stop() = scopes.forEach { it.cancel() }

    private fun wei(gweis: Long) = "0x" + (gweis * 1_000_000_000L).toString(16)

    private fun script() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body(wei(1)) }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", wei(1))) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            fun row() = JSONObject().put("maxFeePerGas", wei(2)).put("networkFeePerGas", wei(1)).put("relayerFeePerGas", wei(1))
            FakeRelayPort.body(JSONObject().put("fast", row()).put("standard", row()).put("slow", row()))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(
                JSONArray()
                    .put(
                        JSONObject().put("recipient", RECIPIENT).put("asset", "native").put("balance", "0x9f3306a949ca000")
                            .put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1"),
                    )
                    .put(
                        JSONObject().put("recipient", RECIPIENT).put("asset", "erc20").put("feeToken", USDC)
                            .put("balance", "0x" + 99_990_000L.toString(16)).put("decimals", 6).put("symbol", "USDC")
                            .put("usdBalance", "99.99").put("usdPrice", "1"),
                    ),
            )
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.rest["https://relay.test/v1/treasury/100"] =
            RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${SAFE.lowercase()}"] =
            RestAnswer.Ok(JSONObject().put("activeDepositAddress", RECIPIENT).put("status", "ACTIVE"))
    }

    private fun control(): SpeedControl {
        script()
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val relay = RelayClient(port, builtinBase = { "https://relay.test" }, retryDelayMs = 0)
        return SpeedControl(scope, relay, FeeExecutor(relay, keyHexes = { emptyList() }), { FeeTier.Fast }, { "comma_dot" }, area = "test").start()
    }

    private val calls = listOf(FeeCall(to = RECIPIENT, value = "1000", data = "0x"))

    @Test
    fun `a fee coin picked after a quote switches the quote in force to that coin`() = runBlocking {
        val speed = control()
        val first = withTimeout(15_000) { speed.quote(100, SAFE, true, calls, null) } as SpeedControl.Quoted.Settled
        assertTrue("quoted in the chain's own coin first: ${first.view.fee?.fee_asset}", first.view.fee?.fee_asset is FeeAssetView.Native)
        assertTrue("USDC is on offer: ${first.view.options.map { it.symbol }}", first.view.options.any { it.symbol == "USDC" })

        speed.selectFeeAsset(USDC)

        val switched = withTimeout(15_000) {
            speed.fee.first { !it.busy && !it.provisional && it.fee?.fee_asset is FeeAssetView.Erc20 }
        }
        val asset = switched.fee!!.fee_asset as FeeAssetView.Erc20
        assertEquals(USDC.lowercase(), asset.token.lowercase())
        assertEquals(6, asset.decimals)
        assertEquals(USDC.lowercase(), switched.fee_token?.lowercase())
        assertTrue("the sheet marks USDC chosen", switched.options.single { it.symbol == "USDC" }.selected)
        assertTrue("and XDAI no longer", !switched.options.single { it.symbol == "XDAI" }.selected)
    }

    @Test
    fun `picking the native coin back returns the quote to it`() = runBlocking {
        val speed = control()
        withTimeout(15_000) { speed.quote(100, SAFE, true, calls, USDC) }
        withTimeout(15_000) { speed.fee.first { !it.busy && it.fee?.fee_asset is FeeAssetView.Erc20 } }

        speed.selectFeeAsset(null)

        val back = withTimeout(15_000) { speed.fee.first { !it.busy && !it.provisional && it.fee?.fee_asset is FeeAssetView.Native } }
        assertNull(back.fee_token)
        assertTrue(back.options.single { it.symbol == "XDAI" }.selected)
    }

    @Test
    fun `a pick before anything was asked tells the core nothing`() = runBlocking {
        val speed = control()
        speed.selectFeeAsset(USDC)
        delay(300)
        // No question, no quote to switch: the view stays the empty one, and
        // the next question (carrying the coin) is what prices it.
        assertNull(speed.fee.value.fee)
        assertTrue(port.calls.none { it.endsWith("vela_getInBandGasQuote") })
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
        const val USDC = "0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0"
    }
}
