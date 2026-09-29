package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.FeedExecutor
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
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 082 RJ12 / RJ13 (G47, G48): the signing sheet's fee row after a failed
 * quote, through the real `sign_request` and `fee_policy` machines.
 *
 * - A deployment read the chain's nodes did not answer is the chain node's
 *   failure (`ChainRead`), rate-limited or out of reach — never Vela's relay,
 *   never a quote priced as if the Safe did not exist.
 * - A failure a recovering network clears is asked again on the core's
 *   schedule (3 s, 6 s, then every 8 s), each ask bounded by
 *   `feeRequoteTimeoutMs()`, and the fee is back as soon as the relay is.
 */
class SigningFeeRetryTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Me", safe)

    @After
    fun stop() = scope.cancel()

    /** Everything a deployed Safe's quote reads, answering. */
    private fun scriptRelay() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body("0x3b9aca00") }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x3b9aca00")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x5f5e100") }
        port.always("pimlico_getUserOperationGasPrice") {
            fun row(max: String) = JSONObject().put("maxFeePerGas", max).put("networkFeePerGas", "0x3b9aca00").put("relayerFeePerGas", "0x3b9aca00")
            FakeRelayPort.body(JSONObject().put("fast", row("0x77359400")).put("standard", row("0x59682f00")).put("slow", row("0x4a817c80")))
        }
        port.always("vela_getInBandGasQuote") {
            FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
        }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.rest["https://relay.test/v1/treasury/100"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/100/${safe.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", "0x2222222222222222222222222222222222222222").put("status", "ACTIVE"))
    }

    private fun controller(): SigningController = SigningController(
        scope = scope,
        relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
        accounts = StoreAccountPort(AccountStore(store)),
        signer = {
            object : UserOpSigner {
                override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion =
                    error("nothing is signed in a fee test")
            }
        },
        knownChains = { listOf(1, 100) },
        wallet = SignAccountRef(address = safe, credential_id = "00"),
        ports = object : SigningController.Ports {
            override fun respond(transportId: String, id: String, payload: SignResponsePayload) = Unit
            override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) = Unit
            override fun recordsPersisted() = Unit
            override fun recordPersisted(recordId: String) = Unit
            override suspend fun switchAccount(address: String) = true
            override fun nativeSymbol(chainId: Int) = "XDAI"
            override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
            override fun dataBase() = ""
            override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
        },
    )

    private fun transfer() = IncomingRequest(
        id = "r1", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", safe).put("to", "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141").put("value", "0x38d7ea4c68000")).toString(),
        origin = "http://127.0.0.1:8137", transportId = "tab-1", chainId = 100,
    )

    @Test
    fun `a rate-limited deployment read is the chain node's failure, in the core's words`() = runBlocking<Unit> {
        scriptRelay()
        port.always("eth_getCode") { RpcResult.Failed(rateLimited = true) }
        val c = controller()
        c.open(transfer())
        val failed = withTimeout(20_000) { c.fee.first { it.failed != null } }
        assertEquals("never QuoteUnavailable, never a quote for a Safe that was not read", FeeFailure.ChainRead(rate_limited = true), failed.failed)
        assertTrue("nothing was priced", failed.fee == null && !failed.confirm_fee_ready)
        val row = SigningLive.feeModel(ClearSigningView(), failed, ctx) as FeeModel.OnChain
        assertEquals(strings.t("home.balanceDetailStatusRetrying"), row.warning)
        assertTrue("no relay was asked for a price", port.calls.none { it.endsWith("pimlico_getUserOperationGasPrice") })
    }

    @Test
    fun `a chain whose nodes do not answer is named, and the fee is back once they do`() = runBlocking<Unit> {
        scriptRelay()
        val down = java.util.concurrent.atomic.AtomicBoolean(true)
        port.always("eth_getCode") { if (down.get()) RpcResult.Failed(rateLimited = false) else FakeRelayPort.body("0x6080") }
        val c = controller()
        c.open(transfer())
        val failed = withTimeout(20_000) { c.fee.first { it.failed != null } }
        assertEquals(FeeFailure.ChainRead(rate_limited = false), failed.failed)
        val row = SigningLive.feeModel(ClearSigningView(), failed, ctx) as FeeModel.OnChain
        assertEquals(strings.t("explore.chainDown", mapOf("chain" to "Gnosis")), row.warning)

        // The node comes back; the core's schedule asks again (the first after 3 s).
        down.set(false)
        val started = System.currentTimeMillis()
        val back = withTimeout(20_000) { c.fee.first { it.confirm_fee_ready && it.fee != null } }
        val took = System.currentTimeMillis() - started
        assertEquals(null, back.failed)
        assertTrue("back on the schedule's first step (3 s), not a later one: $took ms", took < 3_000 + uniffi.vela_core_uniffi.feeRequoteTimeoutMs().toLong())
    }

    @Test
    fun `a relay that was down is asked again on the schedule and the fee comes back`() = runBlocking<Unit> {
        scriptRelay()
        val relayDown = java.util.concurrent.atomic.AtomicBoolean(true)
        port.always("vela_getInBandGasQuote") {
            if (relayDown.get()) {
                RpcResult.Failed(rateLimited = false)
            } else {
                FakeRelayPort.body(JSONArray().put(JSONObject().put("recipient", "0x2222222222222222222222222222222222222222").put("asset", "native").put("balance", "0x9f3306a949ca000").put("decimals", 18).put("symbol", "XDAI").put("usdBalance", "0.71").put("usdPrice", "1")))
            }
        }
        val c = controller()
        c.open(transfer())
        val failed = withTimeout(30_000) { c.fee.first { it.failed != null && !it.busy } }
        assertTrue("a failure a recovering relay clears: ${failed.failed}", uniffi.vela_core_uniffi.feeRequoteDelayMs(failed.failed!!.wire, 1u) != null)
        relayDown.set(false)
        val back = withTimeout(20_000) { c.fee.first { it.confirm_fee_ready && it.fee != null } }
        assertEquals(null, back.failed)
    }
}
