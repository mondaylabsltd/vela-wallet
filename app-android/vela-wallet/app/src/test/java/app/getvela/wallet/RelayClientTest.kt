package app.getvela.wallet

import app.getvela.wallet.feature.send.core.FeeAssetKind
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RelayPort
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.feature.send.core.TrackLifecycle
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The relay as the send path talks to it: transport only, with the pool's
 * routing questions answered by a script (spec 043 T011).
 *
 * What is worth pinning here is the shape of each answer — a 404 treasury is
 * "uncovered" and never "down", a busy relay is retried and a refusing one is
 * not, a receipt with no transaction hash is "pending", and the two caches
 * answer from memory for exactly as long as the web's do.
 */
class RelayClientTest {

    private val port = FakeRelayPort()
    private var clock = 1_000L
    private val relay = RelayClient(port, builtinBase = { "https://builtin.test" }, now = { clock }, retryDelayMs = 0)

    private fun body(result: Any?): RpcResult = RpcResult.Body(JSONObject().put("result", result))
    private fun error(message: String): RpcResult =
        RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32000).put("message", message)))

    @Test
    fun `a 404 treasury is uncovered, an unreachable one unknown, a low float is the status`() = runBlocking {
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Status(404)
        assertEquals(SendTreasuryProbe.Uncovered, relay.probeTreasury(100))

        port.rest["https://relay.test/v1/treasury/137"] = RestAnswer.Failed
        assertEquals(SendTreasuryProbe.Unknown, relay.probeTreasury(137))

        port.rest["https://relay.test/v1/treasury/4217"] = RestAnswer.Ok(
            JSONObject()
                .put("address", "0x1111111111111111111111111111111111111111")
                .put("asset", "pathUSD")
                .put("balance", "0x10")
                .put("floor", "0x20")
                .put("bootstrapNeeded", true),
        )
        val probe = relay.probeTreasury(4217) as SendTreasuryProbe.LowFloat
        assertEquals(SendTreasuryAsset.PathUsd, probe.status.asset)
        assertEquals("16", probe.status.balance)
        assertEquals("32", probe.status.floor)
        assertTrue(probe.status.bootstrap_needed)
    }

    @Test
    fun `the built-in base serves a chain the pool names no relay for`() = runBlocking {
        port.base = null
        port.rest["https://builtin.test/v1/treasury/100"] = RestAnswer.Ok(
            JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false),
        )
        assertEquals(SendTreasuryProbe.Covered, relay.probeTreasury(100))
    }

    @Test
    fun `in-band quotes are cached for eight seconds and cleared on demand`() = runBlocking {
        val row = JSONObject()
            .put("recipient", "0x2222222222222222222222222222222222222222")
            .put("asset", "native").put("balance", "0x3e8").put("decimals", 18)
            .put("symbol", "XDAI").put("usdBalance", "1000").put("usdPrice", "1")
        port.answer("vela_getInBandGasQuote", body(JSONArray().put(row)), body(JSONArray().put(row)))

        val first = relay.inBandQuotes(100, "0xabc")!!
        assertEquals(FeeAssetKind.Native, first.single().asset)
        clock += 7_999
        relay.inBandQuotes(100, "0xabc")
        assertEquals(1, port.calls.count { it.endsWith("vela_getInBandGasQuote") })

        relay.clearCaches()
        relay.inBandQuotes(100, "0xabc")
        assertEquals(2, port.calls.count { it.endsWith("vela_getInBandGasQuote") })
    }

    @Test
    fun `without a native price only the native row survives`() = runBlocking {
        val native = JSONObject().put("recipient", "0x2222222222222222222222222222222222222222")
            .put("asset", "native").put("balance", "0x1").put("decimals", 18).put("symbol", "ETH").put("usdBalance", "0")
        val usdc = JSONObject().put("recipient", "0x2222222222222222222222222222222222222222")
            .put("asset", "erc20").put("feeToken", "0x3333333333333333333333333333333333333333")
            .put("balance", "0x1").put("decimals", 6).put("symbol", "USDC").put("usdBalance", "1").put("usdPrice", "1")
        port.answer("vela_getInBandGasQuote", body(JSONArray().put(native).put(usdc)))
        assertEquals(listOf(FeeAssetKind.Native), relay.inBandQuotes(1, "0xabc")!!.map { it.asset })
    }

    @Test
    fun `the bundler quote reads one tier as decimal strings`() = runBlocking {
        port.answer(
            "pimlico_getUserOperationGasPrice",
            body(JSONObject().put("fast", JSONObject().put("maxFeePerGas", "0x10").put("networkFeePerGas", "0x8"))),
        )
        val quote = relay.bundlerQuote(100, FeeTier.Fast)!!
        assertEquals("16", quote.max_fee_per_gas)
        assertEquals("8", quote.network_fee_per_gas)
        assertNull(quote.relayer_fee_per_gas)
    }

    /** Spec 069: the tip this tier is signed with — one half of the gas bid the core publishes. */
    @Test
    fun `the bundler quote carries the tier's signed tip`() = runBlocking {
        port.answer(
            "pimlico_getUserOperationGasPrice",
            body(JSONObject().put("slow", JSONObject().put("maxFeePerGas", "0x20").put("maxPriorityFeePerGas", "0x3"))),
        )
        val quote = relay.bundlerQuote(100, FeeTier.Slow)!!
        assertEquals("3", quote.max_priority_fee_per_gas)
    }

    /**
     * Spec 069: the speed the displayed fee was priced at is the third
     * parameter, by name; no speed is the pre-068 two-element wire; the dead
     * `rapid` is never sent.
     */
    @Test
    fun `a submission names its speed as the third parameter`() = runBlocking {
        val sent = java.util.concurrent.CopyOnWriteArrayList<List<Any?>>()
        port.always("eth_sendUserOperation") { params -> sent += params; body("0xhash") }
        relay.sendUserOp(100, "{\"sender\":\"0x1\"}", FeeTier.Slow)
        relay.sendUserOp(100, "{\"sender\":\"0x1\"}")
        relay.sendUserOp(100, "{\"sender\":\"0x1\"}", FeeTier.Rapid)
        assertEquals(listOf(3, 2, 2), sent.map { it.size })
        assertEquals("slow", sent[0][2])
    }

    /**
     * Issue 212 on Android: a complete gas reading is held 15 s per chain, so
     * a recipient edit does not re-roll the price; a refresh drops it first.
     */
    @Test
    fun `gas signals hold still for fifteen seconds and a refresh measures again`() = runBlocking {
        port.always("eth_gasPrice") { body("0x64") }
        port.always("eth_getBlockByNumber") { body(JSONObject().put("baseFeePerGas", "0x32")) }
        port.always("eth_maxPriorityFeePerGas") { body("0x1") }
        val first = relay.gasSignals(100, wantTip = true)
        assertEquals("100", first.ethGasPrice)
        relay.gasSignals(100, wantTip = true)
        assertEquals(1, port.calls.count { it.endsWith("eth_gasPrice") })
        relay.invalidateFeeSignals(100)
        relay.gasSignals(100, wantTip = true)
        assertEquals(2, port.calls.count { it.endsWith("eth_gasPrice") })
        clock += 15_001L
        relay.gasSignals(100, wantTip = true)
        assertEquals(3, port.calls.count { it.endsWith("eth_gasPrice") })
    }

    @Test
    fun `a busy relay is retried and a refusing one is not`() = runBlocking {
        port.answer("eth_sendUserOperation", error("Bundler is currently processing; Retry later"), body("0xhash"))
        val accepted = relay.sendUserOp(100, """{"sender":"0x1","nonce":"0x0"}""")
        assertEquals(RelayClient.SubmitAnswer.Accepted("0xhash"), accepted)
        assertEquals(2, port.calls.count { it.endsWith("eth_sendUserOperation") })

        port.answer("eth_sendUserOperation", error("AA25 invalid account nonce"))
        val rejected = relay.sendUserOp(100, """{"sender":"0x1","nonce":"0x0"}""") as RelayClient.SubmitAnswer.Rejected
        assertTrue(rejected.errorJson.contains("AA25"))
        assertEquals(3, port.calls.count { it.endsWith("eth_sendUserOperation") })

        assertEquals(RelayClient.SubmitAnswer.Unreachable, relay.sendUserOp(100, """{"sender":"0x1"}"""))
    }

    @Test
    fun `an estimate crosses as decimal strings and a refusal keeps the relay's words`() = runBlocking {
        port.answer(
            "eth_estimateUserOperationGas",
            body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350")),
            error("AA23 reverted"),
        )
        assertEquals(
            RelayClient.EstimateAnswer.Estimated("100000", "200000", "50000"),
            relay.estimateUserOpGas(100, "{}"),
        )
        assertEquals(RelayClient.EstimateAnswer.Refused("AA23 reverted"), relay.estimateUserOpGas(100, "{}"))
        assertEquals(RelayClient.EstimateAnswer.Unreachable, relay.estimateUserOpGas(100, "{}"))
    }

    @Test
    fun `a receipt without a transaction hash is pending, with one it is resolved with its logs`() = runBlocking {
        port.answer(
            "eth_getUserOperationReceipt",
            body(JSONObject.NULL),
            body(
                JSONObject().put("success", true).put("sender", "0xabc").put(
                    "receipt",
                    JSONObject().put("transactionHash", "0xtx").put(
                        "logs",
                        JSONArray().put(JSONObject().put("address", "0xtoken").put("topics", JSONArray().put("0xt0")).put("data", "0x01")),
                    ),
                ),
            ),
        )
        assertEquals(RelayClient.ReceiptAnswer.Pending, relay.userOpReceipt(100, "0xop"))
        val resolved = relay.userOpReceipt(100, "0xop") as RelayClient.ReceiptAnswer.Resolved
        assertTrue(resolved.confirmed)
        assertEquals("0xtx", resolved.txHash)
        assertEquals("0xtoken", resolved.logs.single().address)
        assertEquals(RelayClient.ReceiptAnswer.Unreachable, relay.userOpReceipt(100, ""))
    }

    @Test
    fun `the status is the relay's word and null for an older relay`() = runBlocking {
        port.answer("eth_getUserOperationStatus", body(JSONObject().put("status", "included").put("last_executor_stage", "mined")))
        assertEquals(TrackLifecycle.Included to "mined", relay.userOpStatus(100, "0xop"))
        assertNull(relay.userOpStatus(100, "0xop"))
    }

    @Test
    fun `the nonce and the deployment state are chain reads through the pool`() = runBlocking {
        val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        port.answer("eth_call", body("0x" + "0".repeat(63) + "7"))
        port.answer("eth_getCode", body("0x"), body("0x6080"))
        assertEquals("0x7", relay.nonce(100, safe))
        // Not deployed is never held: the first send deploys it.
        assertEquals(false, relay.isDeployed(100, safe))
        assertEquals(true, relay.isDeployed(100, safe))
        // Deployed is held for good (spec 078): code does not go away, so the
        // next asker is answered without a read — the queue here is empty and
        // an unanswered read would be `null`.
        assertEquals(true, relay.isDeployed(100, safe.lowercase()))
        assertEquals(2, port.calls.count { it.endsWith("eth_getCode") })
        // Unknown is never held either.
        assertNull(relay.isDeployed(137, safe))
        assertTrue(port.calls.first { it.contains("eth_call") }.startsWith("Rpc:"))
    }

    /**
     * The recipient-risk `is_contract` is the web's rule: an EIP-7702
     * delegated EOA (`0xef0100 ++ impl`) is a wallet, not a contract.
     * `isDeployed` (the sender's Safe) keeps counting any code.
     */
    @Test
    fun `a 7702-delegated account is a wallet, not a contract`() = runBlocking {
        val to = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val delegated = "0xef0100" + "63c0c19a282a1B52b07dD5a65b58948A07DAE32B"
        port.answer(
            "eth_getCode",
            body(delegated), body(delegated.uppercase().replace("0X", "0x")), body("0x6080"), body("0x"),
            body("0xef0100" + "ab".repeat(21)),
        )
        assertEquals(false, relay.isContract(100, to))
        assertEquals(false, relay.isContract(100, to))
        assertEquals(true, relay.isContract(100, to))
        assertEquals(false, relay.isContract(100, to))
        assertEquals("not exactly 23 bytes: a contract", true, relay.isContract(100, to))
        assertNull(relay.isContract(100, to))
        port.answer("eth_getCode", body(delegated))
        assertEquals("the sender's deployment reads any code", true, relay.isDeployed(100, to))
    }
}
