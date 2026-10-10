package app.getvela.wallet

import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeBalanceChange
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import java.math.BigInteger
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Issue #411 (Android 0.9.6): a Uniswap swap on Polygon in the in-app
 * browser — pUSD −128.51 → USDC +128.4946 — opened the sheet with POL as the
 * fee coin. The account held no POL; pUSD (257.02) and USDC (99.99) could
 * both pay ~0.317. The router's swap path names both stablecoins, so until a
 * simulation says what the swap leaves of them the fee machine cannot count
 * on either (spec 096 F2) and the requested coin — native — stood. The sheet
 * DID simulate the swap, for its balance block, and never told the machine.
 *
 * Through the real `sign_request` and `fee_policy` cores: once the sheet's
 * simulation answers, the fee is in a coin that can pay; a simulation that
 * could not check tells the machine nothing.
 */
class SigningFeeCoinTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()

    @After
    fun stop() = scope.cancel()

    private fun hex(value: String) = "0x" + BigInteger(value).toString(16)

    /** A deployed Safe on Polygon at 2185 gwei: the quote is 2.94975 POL, ≈ $0.317 in a stablecoin. */
    private fun scriptRelay() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body(hex(GAS_PRICE)) }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", "0x0")) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body("0x0") }
        port.always("pimlico_getUserOperationGasPrice") {
            fun row() = JSONObject().put("maxFeePerGas", hex("4370000000000")).put("networkFeePerGas", hex(GAS_PRICE)).put("relayerFeePerGas", hex(GAS_PRICE))
            FakeRelayPort.body(JSONObject().put("fast", row()).put("standard", row()).put("slow", row()))
        }
        port.always("vela_getInBandGasQuote") { FakeRelayPort.body(rows()) }
        port.always("eth_estimateUserOperationGas") {
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        port.rest["https://relay.test/v1/treasury/137"] = RestAnswer.Ok(JSONObject().put("address", "0x1111111111111111111111111111111111111111").put("bootstrapNeeded", false))
        port.rest["https://relay.test/v1/account/137/${SAFE.lowercase()}"] = RestAnswer.Ok(JSONObject().put("activeDepositAddress", RECIPIENT).put("status", "ACTIVE"))
    }

    /** The three rows the sheet listed: POL 0, pUSD 257.02, USDC 99.99. */
    private fun rows(): JSONArray = JSONArray()
        .put(JSONObject().put("recipient", RECIPIENT).put("asset", "native").put("balance", "0x0").put("decimals", 18).put("symbol", "POL").put("usdBalance", "0").put("usdPrice", "0.1075"))
        .put(JSONObject().put("recipient", RECIPIENT).put("asset", "erc20").put("feeToken", PUSD).put("balance", hex("257020000")).put("decimals", 6).put("symbol", "pUSD").put("usdBalance", "257.02").put("usdPrice", "1"))
        .put(JSONObject().put("recipient", RECIPIENT).put("asset", "erc20").put("feeToken", USDC).put("balance", hex("99990000")).put("decimals", 6).put("symbol", "USDC").put("usdBalance", "99.99").put("usdPrice", "1"))

    /** A Universal Router `execute` whose V3 path is pUSD → USDC: both named, no `transfer`. */
    private val swapData = "0x3593564c" + "00".repeat(32 * 9) + bare(PUSD) + "000064" + bare(USDC) + "00".repeat(32 * 2 + 9)

    private fun bare(address: String) = address.removePrefix("0x").lowercase()

    private val transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
    private fun topic(address: String) = "0x" + "0".repeat(24) + bare(address)
    private fun transfer(token: String, from: String, to: String, value: String) = JSONObject()
        .put("address", token)
        .put("topics", JSONArray().put(transferTopic).put(topic(from)).put(topic(to)))
        .put("data", "0x" + BigInteger(value).toString(16).padStart(64, '0'))

    /** The node's answer for the swap: the coins' own `Transfer` logs. */
    private fun swapSimulated(): RpcResult = RpcResult.Body(
        JSONObject().put(
            "result",
            JSONArray().put(
                JSONObject().put(
                    "calls",
                    JSONArray().put(
                        JSONObject().put("status", "0x1").put("returnData", "0x").put(
                            "logs",
                            JSONArray()
                                .put(transfer(PUSD, SAFE, POOL, "128510000"))
                                .put(transfer(USDC, POOL, SAFE, "128494600")),
                        ),
                    ),
                ),
            ),
        ),
    )

    private fun controller(
        simulate: () -> RpcResult?,
        simulated: AtomicInteger = AtomicInteger(),
        /** The trust machine's judged view of the deltas; none by default (this host has no judge). */
        judge: ((List<app.getvela.wallet.feature.wallet.core.TrustAssetDelta>) -> app.getvela.wallet.feature.wallet.core.TrustSimView?)? = null,
    ): SigningController = SigningController(
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
        knownChains = { listOf(1, 137) },
        wallet = SignAccountRef(address = SAFE, credential_id = "00"),
        ports = object : SigningController.Ports {
            override fun respond(transportId: String, id: String, payload: SignResponsePayload) = Unit
            override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) = Unit
            override fun recordsPersisted() = Unit
            override fun recordPersisted(recordId: String) = Unit
            override suspend fun switchAccount(address: String) = true
            override fun nativeSymbol(chainId: Int) = "POL"
            override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
            override fun dataBase() = ""
            override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            override suspend fun simulate(chainId: Int, params: List<Any?>): RpcResult? {
                simulated.incrementAndGet()
                // A node is waited for, never blocked on. These stubs may hold
                // their thread (a latch) to stand for a slow one, so they
                // answer off the caller's: the sheet's controller starts its
                // simulation in the step that opens the request (PR 3), and a
                // stub that blocked there would hold `open` itself. (IO, not
                // the scope's own Default: `withContext` onto the dispatcher
                // a coroutine already has runs in place.)
                return kotlinx.coroutines.withContext(Dispatchers.IO) { simulate() }
            }
            override suspend fun judgeDeltas(chainId: Int, wallet: String, deltas: List<app.getvela.wallet.feature.wallet.core.TrustAssetDelta>) =
                judge?.invoke(deltas)
        },
    )

    /** The real `token_trust` machine's judged view of [deltas], as the app's port reads it (`WalletController.judgeSimDeltas`). */
    private fun trustMachine(deltas: List<app.getvela.wallet.feature.wallet.core.TrustAssetDelta>): app.getvela.wallet.feature.wallet.core.TrustSimView? {
        val script = app.getvela.wallet.core.crux.CoreScript(uniffi.vela_core_uniffi.TokenTrustCore().asBridge()) { null }
        val event: app.getvela.wallet.feature.wallet.core.TrustEvent =
            app.getvela.wallet.feature.wallet.core.TrustEvent.SimDeltasComputed(address = SAFE.lowercase(), chain_id = 137, deltas = deltas)
        script.dispatch(app.getvela.wallet.core.crux.Wire.json.encodeToString(app.getvela.wallet.feature.wallet.core.TrustEvent.serializer(), event))
        return app.getvela.wallet.core.crux.Wire.json.decodeFromString(app.getvela.wallet.feature.wallet.core.TrustView.serializer(), script.viewJson()).sim?.takeIf { it.ready }
    }

    /** A clean run with no logs: checked, and nothing of the account's moves. */
    private fun nothingMoves(): RpcResult = RpcResult.Body(
        JSONObject().put("result", JSONArray().put(JSONObject().put("calls", JSONArray().put(JSONObject().put("status", "0x1").put("returnData", "0x").put("logs", JSONArray()))))),
    )

    /**
     * The device round, item 3: the verdict the sheet draws carries the
     * judged view's own line (`TrustSimView.no_change_key`). The controller
     * used to hand over the judgments alone, and the sheet read "nothing
     * moves" off an empty list by itself. Through the real `sim_outcome` and
     * `token_trust`: a check under which nothing moves lands WITH the core's
     * line; and with no judge to say it, it is "could not check" — never an
     * empty verdict the sheet could take for calm.
     */
    @Test
    fun `a check that moves nothing lands with the trust machine's line, and without a judge it is could-not-check`() = runBlocking<Unit> {
        scriptRelay()
        suspend fun verdict(c: SigningController): SigningController.SimOutcome? {
            c.open(swap())
            return withTimeout(20_000) { c.sim.first { it != null && it != SigningController.SimOutcome.Pending } }
        }
        assertEquals(
            SigningController.SimOutcome.Ready(emptyList(), noChangeKey = "componentsUi.signing.simResultNoChange"),
            verdict(controller({ nothingMoves() }, judge = ::trustMachine)),
        )
        // The line is the view's, whatever it is — and absent, it stays absent.
        val named = app.getvela.wallet.feature.wallet.core.TrustSimView(address = SAFE, chain_id = 137, ready = true, no_change_key = "a.key.of.the.cores")
        assertEquals(SigningController.SimOutcome.Ready(emptyList(), noChangeKey = "a.key.of.the.cores"), verdict(controller({ nothingMoves() }, judge = { named })))
        assertEquals(
            SigningController.SimOutcome.Ready(emptyList(), noChangeKey = null),
            verdict(controller({ nothingMoves() }, judge = { named.copy(no_change_key = null) })),
        )
        // No judge to say it: could not check.
        assertEquals(SimDeltas.couldNotCheck(), verdict(controller({ nothingMoves() })))
    }

    private fun swap() = IncomingRequest(
        id = "r411", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", SAFE).put("to", ROUTER).put("value", "0x0").put("data", swapData)).toString(),
        origin = "https://app.uniswap.org", transportId = "tab-1", chainId = 137,
    )

    private fun FeeView.coin(contract: String?) = options.first { it.contract.equals(contract, ignoreCase = true) }

    @Test
    fun `the swap the sheet simulated pays its fee in a coin that can, never the POL it does not hold`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller({ swapSimulated() })
        c.open(swap())
        val settled = withTimeout(20_000) {
            c.fee.first { !it.busy && it.fee != null && it.fee_token.equals(PUSD, ignoreCase = true) }
        }
        assertTrue("the confirm opens: $settled", settled.confirm_fee_ready)
        val asset = settled.fee!!.fee_asset as FeeAssetView.Erc20
        assertTrue(asset.token.equals(PUSD, ignoreCase = true))
        assertEquals("pUSD", asset.symbol)
        assertFalse("never the POL it does not hold", settled.coin(null).selected)
        assertTrue(settled.coin(null).insufficient)
        assertFalse(settled.coin(PUSD).insufficient || settled.coin(PUSD).spent_by_operation)
        assertFalse(settled.coin(USDC).insufficient || settled.coin(USDC).spent_by_operation)
    }

    @Test
    fun `a simulation that could not check tells the fee machine nothing`() = runBlocking<Unit> {
        scriptRelay()
        val asked = AtomicInteger()
        val c = controller({ RpcResult.Failed(rateLimited = false) }, asked)
        c.open(swap())
        val (fee, _) = withTimeout(20_000) {
            combine(c.fee, c.sim) { fee, sim -> fee to sim }.first { (fee, sim) -> sim != null && !fee.busy && fee.fee != null }
        }
        assertEquals("the sheet asked the node once", 1, asked.get())
        assertNull("no measurement: the fallback stands — POL, short: $fee", fee.fee_token)
        assertTrue(fee.coin(null).selected && fee.coin(null).insufficient)
        assertFalse(fee.confirm_fee_ready)
        assertTrue("both stablecoins stay unmeasured", fee.coin(PUSD).spent_by_operation && fee.coin(USDC).spent_by_operation)
    }

    /**
     * The correctness batch, item 4: the machine's own re-pick after the
     * simulation lands (the dApp sheet's automatic switch) moves the figure to
     * another coin — whose fee leg costs other gas. Until it is measured again
     * with that leg the figure is PROVISIONAL: drawn as it is, with the
     * measuring sign, and the confirm is shut (`fee_measuring`). Then it
     * settles and opens.
     */
    @Test
    fun `a coin switched by the machine is measured again before it can be confirmed`() = runBlocking<Unit> {
        scriptRelay()
        val simulation = java.util.concurrent.CountDownLatch(1)
        val remeasure = java.util.concurrent.CountDownLatch(1)
        val holdEstimates = java.util.concurrent.atomic.AtomicBoolean(false)
        port.always("eth_estimateUserOperationGas") {
            if (holdEstimates.get()) remeasure.await(10, java.util.concurrent.TimeUnit.SECONDS)
            FakeRelayPort.body(JSONObject().put("verificationGasLimit", "0x186a0").put("callGasLimit", "0x30d40").put("preVerificationGas", "0xc350"))
        }
        val c = controller({ simulation.await(10, java.util.concurrent.TimeUnit.SECONDS); swapSimulated() })
        c.open(swap())
        // Priced before the simulation answered: the native fallback, which cannot pay.
        val first = withTimeout(20_000) { c.fee.first { !it.busy && it.fee != null } }
        assertNull(first.fee_token)
        assertFalse(first.provisional)

        // The simulation lands: the machine re-picks pUSD — re-measured with ITS leg.
        holdEstimates.set(true)
        simulation.countDown()
        val provisional = withTimeout(20_000) { c.fee.first { it.provisional } }
        assertTrue("the switched coin: $provisional", provisional.fee_token.equals(PUSD, ignoreCase = true))
        assertTrue("measuring, so the confirm holds", provisional.busy && !provisional.confirm_fee_ready)
        // The sheet draws the switched figure with the measuring sign — never a blank or "estimating".
        val strings = run {
            val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
            app.getvela.wallet.core.i18n.I18nRuntime { tag -> java.io.File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
        }
        val ctx = app.getvela.wallet.feature.signing.SigningLive.Context(strings, "Polygon", androidx.compose.ui.graphics.Color.Red, "POL", "Me", SAFE)
        val row = app.getvela.wallet.feature.signing.SigningLive.feeModel(app.getvela.wallet.feature.signing.core.ClearSigningView(), provisional, ctx)
            as app.getvela.wallet.feature.signing.FeeModel.OnChain
        assertTrue("the switched figure is drawn as it is: $provisional", provisional.fee != null)
        assertTrue("the figure stays on screen: ${row.value}", row.value.startsWith("~") && row.value.contains("pUSD"))
        assertTrue("with the measuring sign", row.measuring && row.refreshing)
        val gate = app.getvela.wallet.feature.signing.SigningLive.confirmState(
            c.signJson.value!!, c.guardJson.value!!, c.clearJson.value!!, c.feeJson.value, null,
        )
        assertFalse(gate.enabled)

        // Measured again: it settles in pUSD and the confirm opens.
        remeasure.countDown()
        val settled = withTimeout(20_000) { c.fee.first { !it.busy && !it.provisional && it.fee_token.equals(PUSD, ignoreCase = true) } }
        assertTrue("settled: $settled", settled.confirm_fee_ready)
    }

    // -- the sessions: told after every question about the measured calls ----------------

    private fun speedControl(): SpeedControl {
        scriptRelay()
        val relay = RelayClient(port, builtinBase = { "https://relay.test" }, retryDelayMs = 0)
        return SpeedControl(scope, relay, FeeExecutor(relay, keyHexes = { emptyList() }), { FeeTier.Fast }, { "comma_dot" }, area = "test").start()
    }

    private val swapCalls = listOf(FeeCall(to = ROUTER, value = "0", data = swapData))
    private val swapChanges = listOf(FeeBalanceChange(PUSD.lowercase(), "-128510000"), FeeBalanceChange(USDC.lowercase(), "128494600"))

    private fun SpeedControl.ask(calls: List<FeeCall> = swapCalls): SpeedControl.Quoted =
        runBlocking { withTimeout(15_000) { quote(137, SAFE, true, calls, null, autoFeeToken = true) } }

    @Test
    fun `a measurement that lands before the question is told right after it, and after every re-ask`() {
        val speed = speedControl()
        speed.balanceChanges(swapCalls, swapChanges)
        val first = speed.ask() as SpeedControl.Quoted.Settled
        assertTrue("pUSD from the start: ${first.view}", first.view.fee_token.equals(PUSD, ignoreCase = true))
        // The same question asked again: the machine forgot it, and was told again.
        val again = speed.ask() as SpeedControl.Quoted.Settled
        assertTrue("still pUSD: ${again.view}", again.view.fee_token.equals(PUSD, ignoreCase = true))
        assertTrue(again.view.confirm_fee_ready)
    }

    @Test
    fun `a measurement of other calls is told to no session`() {
        val speed = speedControl()
        speed.balanceChanges(listOf(FeeCall(to = ROUTER, value = "0", data = swapData + "00")), swapChanges)
        val view = (speed.ask() as SpeedControl.Quoted.Settled).view
        assertNull("not this operation's measurement: $view", view.fee_token)
    }

    @Test
    fun `the other speeds are told too`() = runBlocking<Unit> {
        val speed = speedControl()
        val first = speed.ask() as SpeedControl.Quoted.Settled
        assertNull("unmeasured: today's fallback", first.view.fee_token)
        speed.toggle()
        val tiers = withTimeout(15_000) { speed.speed.first { it.previews.isNotEmpty() } }.previews
        speed.balanceChanges(swapCalls, swapChanges)
        withTimeout(15_000) { speed.fee.first { it.fee_token.equals(PUSD, ignoreCase = true) } }
        for (tier in tiers) {
            val preview = withTimeout(15_000) {
                var view: FeeView? = null
                while (view?.let { !it.busy && it.fee_token.equals(PUSD, ignoreCase = true) } != true) {
                    view = speed.feeViewOf(tier)
                    kotlinx.coroutines.delay(20)
                }
                view
            }
            assertTrue("$tier is priced in pUSD too: $preview", preview!!.confirm_fee_ready)
        }
    }

    @Test
    fun `the simulation's deltas map to the fee machine's balance changes as the desktop's do`() {
        val changes = SimDeltas.feeBalanceChanges(
            listOf(
                TrustAssetDelta(TrustDeltaKind.Native, null, "-1000"),
                TrustAssetDelta(TrustDeltaKind.Erc20, PUSD.lowercase(), "-128510000"),
                // A token move with no contract names no coin: dropped, never native.
                TrustAssetDelta(TrustDeltaKind.Erc20, null, "5"),
            ),
        )
        assertEquals(listOf(FeeBalanceChange(null, "-1000"), FeeBalanceChange(PUSD.lowercase(), "-128510000")), changes)
    }

    private companion object {
        const val SAFE = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RECIPIENT = "0x2222222222222222222222222222222222222222"
        const val ROUTER = "0x1095692A6237d83C6a72F3F5eFEdb9A670C49223"
        const val POOL = "0x3333333333333333333333333333333333333333"
        /** Polygon's native USDC, checksummed as the relay writes it. */
        const val USDC = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
        /** The swap's input coin (synthetic: only its role matters). */
        const val PUSD = "0x7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a"
        const val GAS_PRICE = "2185000000000"
    }
}
