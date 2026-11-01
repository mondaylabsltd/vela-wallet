package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.send.core.FeeEvent
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.FeeGasOutcome
import app.getvela.wallet.feature.send.core.FeeOperation
import app.getvela.wallet.feature.send.core.FeeShellResult
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.wallet.core.RpcResult
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.FeePolicyCore

/**
 * r11-8: the same plain send priced ONE way on every shell — the shared
 * vector `tests/vectors/fee/plain-send.json` replayed through THIS shell's
 * own readers (the relay client, the fee executor's arms) and the real fee
 * machine. Android used to read a relay it could not reach as a refusal and
 * the machine priced the static 600,000 gas (0.045 USDC) while the submit
 * signed the relay's 1,600,000 (every other shell: 0.12).
 */
class FeeParityVectorTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val vector = JSONObject(File(repoRoot, "rust/crates/vela-core/tests/vectors/fee/plain-send.json").readText())
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private fun outcomeJson(outcome: FeeGasOutcome): JSONObject =
        JSONObject(Wire.json.encodeToString(FeeGasOutcome.serializer(), outcome))

    /** A JSON object's fields, nulls left out (an absent field and a null are the same answer). */
    private fun sorted(o: JSONObject): Map<String, Any> = o.keys().asSequence().filter { !o.isNull(it) }.associateWith { o.get(it).toString() }.toSortedMap()

    /** The relay of the round-11 send, every arm from the vector's wire; the estimate as [estimate] says. */
    private fun relay(estimate: Any?): FakeRelayPort {
        val port = FakeRelayPort()
        val gas = vector.getJSONObject("gas_price").getJSONObject("wire")
        port.always("eth_gasPrice") { FakeRelayPort.body(gas.getString("eth_gasPrice")) }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", gas.getString("latest_block_baseFeePerGas"))) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body(gas.getString("eth_maxPriorityFeePerGas")) }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "5") }
        port.always("pimlico_getUserOperationGasPrice") { FakeRelayPort.body(vector.getJSONObject("bundler_quote").getJSONObject("wire")) }
        val quotes: JSONArray = vector.getJSONObject("in_band_quotes").getJSONArray("wire")
        port.always("vela_getInBandGasQuote") { FakeRelayPort.body(quotes) }
        if (estimate is JSONObject) port.always("eth_estimateUserOperationGas") { RpcResult.Body(estimate) }
        val request = vector.getJSONObject("request")
        port.rest["https://relay.test/v1/account/${request.getInt("chain_id")}/${request.getString("account").lowercase()}"] = RestAnswer.Ok(
            JSONObject().put("activeDepositAddress", quotes.getJSONObject(0).getString("recipient")).put("status", "ACTIVE"),
        )
        return port
    }

    @Test
    fun the_round_11_send_is_priced_as_every_shell_prices_it() = runBlocking {
        val cases = vector.getJSONArray("cases")
        for (i in 0 until cases.length()) {
            val case = cases.getJSONObject(i)
            val name = case.getString("name")
            val wire = case.opt("estimate_wire").takeIf { it != JSONObject.NULL }
            val port = relay(wire)
            val client = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0)

            // 1. This shell's reader: the wire answer → the outcome the machine hears.
            val read = when (val answer = client.estimateUserOpGas(5042002, "{}")) {
                is RelayClient.EstimateAnswer.Estimated -> FeeGasOutcome.Estimated(answer.verificationGasLimit, answer.callGasLimit, answer.preVerificationGas, answer.settlementGas)
                is RelayClient.EstimateAnswer.Refused -> FeeExecutor.gasOutcomeOf(answer.errorJson)
                RelayClient.EstimateAnswer.Unreachable -> FeeExecutor.gasOutcomeOf(null)
            }
            assertEquals(name, sorted(case.getJSONObject("user_op_gas")), sorted(outcomeJson(read)))

            // 2. The real machine over this shell's arms: the figure every shell shows.
            val host = CoreHost(
                bridge = FeePolicyCore().asBridge(),
                scope = scope,
                initial = FeeView(),
                serializer = FeeView.serializer(),
                perform = JsonShell.perform(FeeOperation.serializer(), FeeShellResult.serializer(), FeeExecutor(client, keyHexes = { emptyList() }, measureCall = { _, _, _, _, _ -> null })::perform),
                escapedFailure = JsonShell.escapedFailure(FeeOperation.serializer(), FeeShellResult.serializer(), FeeShellResult.TtlElapsed, FeeExecutor(client, keyHexes = { emptyList() })::neutralAnswer),
            ).also { it.start() }
            host.dispatch(Wire.json.decodeFromString(FeeEvent.serializer(), vector.getJSONObject("request").toString()), FeeEvent.serializer())
            val settled = withTimeout(20_000) { host.view.first { !it.busy && (it.failed != null || it.fee != null) } }
            val expect = case.getJSONObject("expect")
            if (expect.isNull("failed")) {
                assertNull(name, settled.failed)
                val fee = settled.fee
                assertNotNull(name, fee)
                assertEquals(name, expect.getString("total_gas"), fee!!.total_gas)
                assertEquals(name, expect.getString("total_wei"), fee.total_wei)
                assertEquals(name, expect.getString("in_band_gas_basis"), fee.in_band_gas_basis)
                val want = expect.getJSONObject("prepared")
                val prepared = fee.prepared
                assertNotNull("$name: the limits it was priced on are the ones signed", prepared)
                assertEquals(name, want.getString("verification_gas_limit"), prepared!!.verification_gas_limit)
                assertEquals(name, want.getString("call_gas_limit"), prepared.call_gas_limit)
                assertEquals(name, want.getString("pre_verification_gas"), prepared.pre_verification_gas)
            } else {
                assertEquals(name, expect.getString("failed"), settled.failed?.name)
                assertNull("$name: no fee on a figure nobody measured", settled.fee)
                assertEquals(name, FeeFailure.EstimateFailed, settled.failed)
            }
        }
    }
}
