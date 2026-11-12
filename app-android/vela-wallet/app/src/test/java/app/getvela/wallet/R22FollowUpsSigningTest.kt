package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PromptKind
import app.getvela.wallet.feature.onboarding.flow.promptCopy
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.core.ConfirmState
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcCallVerdict
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustDeltaKind
import app.getvela.wallet.feature.wallet.core.TrustEvent
import app.getvela.wallet.feature.wallet.core.TrustSimView
import app.getvela.wallet.feature.wallet.core.TrustView
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.TokenTrustCore

/**
 * §39 (Round 22 follow-ups) on Android's dApp signing sheet, through the
 * REAL `sign_request`, `clear_signing`, `approval_guard` and `fee_policy`
 * machines and the real confirm gate:
 *
 * - A1: the fee the sheet DREW when the confirm began (the hold's start, the
 *   assistive path's first press) is the figure signed (`quoted_fee`); the
 *   fee machine's quote at the approve rides beside it (`current_fee`). A
 *   quote that ROSE signs nothing (`fee_rose`): the sheet stays, draws the
 *   new figure and says why; the next confirm pays it. (Android suspect A1:
 *   armed at 0.12, charged 0.138.)
 * - r22-6: a re-check of a landed verdict keeps the confirm's door.
 * - The native value the wallet decoded is never erased by a node's "No
 *   asset changes" (`sim_outcome`'s `value_wei`).
 * - r22-2: a one-provider read carries `sole_provider` into the envelope.
 * - r22-8: a sign-in failure the core words (`words_key`) is said in them.
 */
class R22FollowUpsSigningTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()
    private val port = FakeRelayPort()
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val other = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Me", safe)
    private val drawn = SigningFixtures.build(SigningScreenState.CS1, strings)

    @After
    fun stop() = scope.cancel()

    /**
     * The chain's and the relay's per-gas price (every speed, the base fee,
     * the network's share): 50 gwei, so the fee stands above Gnosis's floor
     * (0.01 XDAI) and moves when the price does.
     */
    @Volatile private var gasPrice = 50_000_000_000L

    private fun hex(v: Long) = "0x" + v.toString(16)

    private fun scriptRelay() {
        port.always("eth_getCode") { FakeRelayPort.body("0x6080") }
        port.always("eth_call") { FakeRelayPort.body("0x" + "0".repeat(63) + "7") }
        port.always("eth_gasPrice") { FakeRelayPort.body(hex(gasPrice)) }
        port.always("eth_getBlockByNumber") { FakeRelayPort.body(JSONObject().put("baseFeePerGas", hex(gasPrice))) }
        port.always("eth_maxPriorityFeePerGas") { FakeRelayPort.body(hex(gasPrice / 10)) }
        port.always("pimlico_getUserOperationGasPrice") {
            fun row(max: Long) = JSONObject().put("maxFeePerGas", hex(max)).put("networkFeePerGas", hex(gasPrice)).put("relayerFeePerGas", hex(gasPrice))
            FakeRelayPort.body(JSONObject().put("fast", row(gasPrice * 4 / 3)).put("standard", row(gasPrice)).put("slow", row(gasPrice * 5 / 6)))
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

    private class StoppedClock {
        val asked = CopyOnWriteArrayList<Long>()
        private val passed = CompletableDeferred<Unit>()
        val timer: suspend (Long) -> Unit = { ms ->
            asked += ms
            passed.await()
        }
    }

    /** Every answer the site was sent: (transport, request id, payload). */
    private val responses = CopyOnWriteArrayList<Triple<String, String, SignResponsePayload>>()

    /** How many times a passkey was asked to sign — the one place a signature is made. */
    private val signed = AtomicInteger()
    private val simulations = AtomicInteger()

    private fun controller(clock: StoppedClock = StoppedClock(), simulate: (suspend () -> RpcResult?)? = null): SigningController = SigningController(
        scope = scope,
        relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
        accounts = StoreAccountPort(AccountStore(store)),
        signer = {
            object : UserOpSigner {
                override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion {
                    signed.incrementAndGet()
                    error("the passkey is not asked in this test")
                }
            }
        },
        knownChains = { listOf(1, 100) },
        wallet = SignAccountRef(address = safe, credential_id = "00"),
        ports = object : SigningController.Ports {
            override fun respond(transportId: String, id: String, payload: SignResponsePayload) {
                responses += Triple(transportId, id, payload)
            }
            override fun opSubmitted(id: String, submitted: app.getvela.wallet.feature.send.core.UserOpSpine.Submitted) = Unit
            override fun recordsPersisted() = Unit
            override fun recordPersisted(recordId: String) = Unit
            override suspend fun switchAccount(address: String) = true
            override fun nativeSymbol(chainId: Int) = "XDAI"
            override fun trackSubmitted(handoff: app.getvela.wallet.feature.send.core.TrackHandoff) = Unit
            override fun dataBase() = ""
            override suspend fun ethCall(chainId: Int, to: String, data: String): Pair<String?, Boolean> = null to false
            override suspend fun simulate(chainId: Int, params: List<Any?>): RpcResult? {
                val node = simulate ?: return null
                simulations.incrementAndGet()
                return node()
            }
            override suspend fun judgeDeltas(chainId: Int, wallet: String, deltas: List<TrustAssetDelta>) = trustMachine(deltas)
        },
        simVerdictTimer = clock.timer,
    )

    private fun trustMachine(deltas: List<TrustAssetDelta>): TrustSimView? {
        val script = CoreScript(TokenTrustCore().asBridge()) { null }
        val event: TrustEvent = TrustEvent.SimDeltasComputed(address = safe.lowercase(), chain_id = 100, deltas = deltas)
        script.dispatch(Wire.json.encodeToString(TrustEvent.serializer(), event))
        return Wire.json.decodeFromString(TrustView.serializer(), script.viewJson()).sim?.takeIf { it.ready }
    }

    /** 0.001 XDAI to another address. */
    private fun transfer() = IncomingRequest(
        id = "r1", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", safe).put("to", other).put("value", "0x38d7ea4c68000")).toString(),
        origin = "http://127.0.0.1:8137", transportId = "tab-1", chainId = 100,
    )

    /** A clean run under which no log moves anything — a node's "No asset changes". */
    private fun silentRun(witness: Boolean = true): RpcResult {
        val call = JSONObject().put("status", "0x1").put("returnData", "0x").put("logs", JSONArray())
        val body = JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", JSONArray().put(JSONObject().put("calls", JSONArray().put(call))))
        return RpcResult.Body(body, witness = if (witness) JSONObject(body.toString()) else null)
    }

    private fun gate(c: SigningController): ConfirmState =
        SigningLive.confirmState(c.signJson.value, c.guardJson.value, c.clearJson.value, c.feeJson.value, null, simJson = c.sim.value?.verdictJson, refuseBlind = false)

    private suspend fun until(c: SigningController, what: (ConfirmState) -> Boolean): ConfirmState = withTimeout(30_000) {
        var state = gate(c)
        while (!what(state)) {
            delay(20)
            state = gate(c)
        }
        state
    }

    private fun total(fee: FeeView): java.math.BigInteger = fee.fee!!.total_wei.toBigInteger()

    /**
     * The approve went past the fee's check: the core took it on (preparing,
     * the passkey) or ended it for a reason that is not the fee — never a
     * `fee_rose`, never still idle on the form.
     */
    private suspend fun taken(c: SigningController) {
        val view = kotlinx.coroutines.withTimeoutOrNull(30_000) {
            var v = c.sign.value
            while (signed.get() == 0 && v.phase == app.getvela.wallet.feature.signing.core.SignPhase.Idle && !v.is_signing && !v.is_submitting && (v.error == null || v.error?.kind == SignErrorKind.FeeRose)) {
                delay(20)
                v = c.sign.value
            }
            v
        } ?: error("the approve was never taken: ${c.sign.value}")
        assertTrue("taken on, not stopped by the fee: $view", view.error?.kind != SignErrorKind.FeeRose)
    }

    // -- A1: the figure drawn when the confirm began is the figure signed -----------------

    @Test
    fun `A1 - a quote that rose after the confirm began signs nothing, draws the new figure and says why, and the next confirm pays it`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller()
        c.open(transfer())
        until(c) { it.enabled }
        val shown = c.fee.value
        assertNotNull(shown.fee)

        // The hold begins (or the assistive path's first press): the figure drawn now is held still.
        c.confirmBegan()
        assertEquals("the row is held at the figure drawn", shown, c.heldFee.value)

        // The relay's price rises while the hold runs; the fee machine re-quotes.
        gasPrice *= 2
        c.refreshFee()
        withTimeout(30_000) { while (c.fee.value.fee == null || total(c.fee.value) <= total(shown)) delay(20) }
        val risen = c.fee.value
        assertEquals("the row did not move under the hold", shown, c.heldFee.value)

        // The hold completes: the figure shown and the quote now both ride the approve.
        val opts = SigningController.approveOpts(shown, c.clear.value, c.guard.value, c.sim.value, current = risen)
        assertEquals(total(shown).toString(), opts.quoted_fee!!.amount)
        assertEquals(total(risen).toString(), opts.current_fee!!.amount)
        c.approve(anywayConfirmed = gate(c).anyway)
        withTimeout(20_000) { while (c.sign.value.error?.kind != SignErrorKind.FeeRose) delay(20) }
        delay(300)
        assertEquals("nothing was signed at a fee nobody was shown", 0, signed.get())
        // The sheet stays, the hold is let go, the new figure is drawn, and why is said.
        assertNull(c.heldFee.value)
        val sheet = SigningLive.model(drawn, transfer(), c.sign.value, c.clear.value, c.guard.value, c.heldFee.value ?: c.fee.value, ctx, c.sim.value, confirm = gate(c))
        assertNull("no receipt: the form stays", sheet.receipt)
        assertTrue(
            "said why: ${SigningLive.statusBlocks(c.sign.value, strings)}",
            SigningLive.statusBlocks(c.sign.value, strings).contains(SigningBlock.Warning(SigningTone.Caution, strings.t("componentsUi.signing.feeRose"))),
        )
        assertTrue("the confirm is not shut by it", gate(c).enabled)

        // The next confirm pays the new figure: it goes on to the passkey.
        c.confirmBegan()
        assertEquals(total(risen), total(c.heldFee.value!!))
        c.approve(anywayConfirmed = gate(c).anyway)
        taken(c)
        assertTrue(c.sign.value.error?.kind != SignErrorKind.FeeRose)
    }

    @Test
    fun `A1 - a quote that fell signs the figure shown, never stopping`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller()
        c.open(transfer())
        until(c) { it.enabled }
        val shown = c.fee.value
        c.confirmBegan()
        gasPrice /= 2
        c.refreshFee()
        withTimeout(30_000) { while (c.fee.value.fee == null || total(c.fee.value) >= total(shown)) delay(20) }
        val fell = c.fee.value
        val opts = SigningController.approveOpts(shown, c.clear.value, c.guard.value, c.sim.value, current = fell)
        assertEquals("the figure shown is what is signed", total(shown).toString(), opts.quoted_fee!!.amount)
        c.approve(anywayConfirmed = gate(c).anyway)
        taken(c)
        assertTrue(c.sign.value.error?.kind != SignErrorKind.FeeRose)
    }

    /**
     * §39 (core dfcc25c55): a `fee_rose` stop sends the site nothing and holds
     * no answer — the sheet stays for the person to confirm again. Closing it
     * then is their "no": the site gets 4001, never left waiting for ever
     * (it used to be a `Dismiss` that answered nobody).
     */
    @Test
    fun `A1 - closing the sheet after a fee-rose stop answers the site 4001, once`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller()
        c.open(transfer())
        until(c) { it.enabled }
        val shown = c.fee.value
        c.confirmBegan()
        gasPrice = gasPrice * 115 / 100
        c.refreshFee()
        withTimeout(30_000) { while (c.fee.value.fee == null || total(c.fee.value) <= total(shown)) delay(20) }
        c.approve(anywayConfirmed = gate(c).anyway)
        withTimeout(20_000) { while (c.sign.value.error?.kind != SignErrorKind.FeeRose) delay(20) }
        delay(300)
        assertEquals(0, signed.get())
        assertTrue("nothing told the site yet: $responses", responses.isEmpty())

        c.swipeDismissed()
        val answer = kotlinx.coroutines.withTimeoutOrNull(10_000) {
            while (responses.isEmpty()) delay(20)
            responses.single()
        } ?: error("the site was left waiting: ${c.sign.value}")
        assertEquals("tab-1", answer.first)
        assertEquals("r1", answer.second)
        val err = answer.third as SignResponsePayload.Err
        assertEquals(4001, err.code)
        assertEquals(SignErrorKind.UserRejected, err.kind)
        delay(500)
        assertEquals("answered once", 1, responses.size)
        assertEquals(0, signed.get())
    }

    @Test
    fun `A1 - a hold let go early releases the row`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller()
        c.open(transfer())
        until(c) { it.enabled }
        c.confirmBegan()
        assertNotNull(c.heldFee.value)
        c.confirmLetGo()
        assertNull(c.heldFee.value)
    }

    @Test
    fun `fee_rose is a kind this shell reads`() {
        assertEquals(SignErrorKind.FeeRose, Wire.json.decodeFromString(SignErrorKind.serializer(), "\"fee_rose\""))
    }

    // -- r22-6: a re-check of a landed verdict keeps the door -----------------------------

    @Test
    fun `r22-6 - a re-check of a landed verdict keeps the confirm's door, and lands the same`() = runBlocking<Unit> {
        scriptRelay()
        val clock = StoppedClock()
        val answers = CopyOnWriteArrayList<CompletableDeferred<RpcResult?>>()
        val c = controller(clock) {
            val next = CompletableDeferred<RpcResult?>()
            answers += next
            next.await()
        }
        c.open(transfer())
        withTimeout(10_000) { while (answers.isEmpty()) delay(10) }
        answers[0].complete(silentRun(witness = false))
        val landed = until(c) { it.enabled && c.sim.value is SigningController.SimOutcome.Notice }
        assertTrue("one node's word: … anyway", landed.anyway)
        assertTrue(landed.hold_ms > 0)
        assertEquals("the first check's wait", 1, clock.asked.size)

        // Every frame from here: the door never shuts.
        val frames = CopyOnWriteArrayList<ConfirmState>()
        val watching = scope.launch(start = CoroutineStart.UNDISPATCHED) {
            while (true) {
                frames += gate(c)
                delay(5)
            }
        }
        assertTrue("check again is offered", c.simAgainOffered.value)
        c.simAgain()
        withTimeout(10_000) { while (answers.size < 2) delay(10) }
        assertTrue(c.simRechecking.value)
        delay(300)
        assertFalse("the core holds no new wait", c.sign.value.sim_checking)
        answers[1].complete(silentRun(witness = false))
        withTimeout(10_000) { while (c.simRechecking.value) delay(10) }
        delay(200)
        watching.cancel()
        assertTrue(frames.isNotEmpty())
        assertTrue("a frame shut the door: ${frames.filter { !it.enabled }}", frames.all { it.enabled && it.anyway && it.hold_ms == landed.hold_ms })
        assertEquals("a re-check asks for no wait of its own", 1, clock.asked.size)
    }

    // -- the native value the wallet decoded ----------------------------------------------

    @Test
    fun `a node's no-asset-changes never erases the native value the request sends`() = runBlocking<Unit> {
        scriptRelay()
        val c = controller { silentRun() }
        c.open(transfer())
        val ready = withTimeout(30_000) {
            var sim = c.sim.value
            while (sim !is SigningController.SimOutcome.Ready && sim !is SigningController.SimOutcome.Notice) {
                delay(20)
                sim = c.sim.value
            }
            sim
        }
        ready as SigningController.SimOutcome.Ready
        assertNull("not \"No asset changes\"", ready.noChangeKey)
        val balances = SigningLive.simBlocks(ready, ctx).filterIsInstance<SigningBlock.Balances>().single()
        assertEquals(listOf("XDAI" to "−0.001"), balances.rows.map { it.symbol to it.delta })
    }

    @Test
    fun `the decoded native value - summed for a batch, decimal or hex, none when zero or unreadable`() {
        fun call(value: String?) = SimDeltas.Call(other, value, null)
        assertEquals("1000", SimDeltas.nativeValue(listOf(call("1000"))))
        assertEquals("1255", SimDeltas.nativeValue(listOf(call("0x3e8"), call("255"), call(null))))
        assertNull(SimDeltas.nativeValue(listOf(call("0x0"), call(""))))
        assertNull(SimDeltas.nativeValue(listOf(call("lots"))))
        val record = SimDeltas.outcome(safe, silentRun(), "1000")
        val deltas = SimDeltas.deltas(record)!!
        assertEquals(listOf(TrustDeltaKind.Native to "-1000"), deltas.map { it.kind to it.delta })
    }

    // -- r22-2: one provider, said into the envelope ----------------------------------------

    @Test
    fun `r22-2 - the pool's sole-provider verdict rides into the corroborated envelope`() {
        val verdict = Wire.json.decodeFromString(RpcCallVerdict.serializer(), """{"type":"respond","url":"https://a","sole_provider":true}""")
        assertTrue((verdict as RpcCallVerdict.Respond).sole_provider)
        val body = JSONObject().put("result", "0x1")
        val sole = JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Body(body, soleProvider = true)))
        assertTrue(sole.optBoolean("soleProvider"))
        assertFalse(sole.has("witness"))
        val plain = JSONObject(RpcResult.corroboratedEnvelope(RpcResult.Body(body)))
        assertFalse(plain.optBoolean("soleProvider"))
    }

    // -- r22-8: a sign-in failure the core words ------------------------------------------

    @Test
    fun `r22-8 - a registry group that does not match is said in the catalog's words, without the biometric advice`() {
        val kind = PromptKind.from(
            JSONObject().put("type", "sign_in_failed").put("detail", "registry group does not match its address")
                .put("phone_link", false).put("words_key", "onboarding.login.registryUnreachableBody"),
        )
        val message = promptCopy(kind, strings).message
        assertEquals(strings.t("onboarding.login.registryUnreachableBody"), message)
        assertFalse(message.contains("registry group"))
        assertFalse(message.contains("screen lock"))
    }
}
