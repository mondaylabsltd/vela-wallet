package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.RestAnswer
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.core.ConfirmBlock
import app.getvela.wallet.feature.signing.core.ConfirmState
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignPhase
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.signing.core.SimDeltas
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.TrustAssetDelta
import app.getvela.wallet.feature.wallet.core.TrustEvent
import app.getvela.wallet.feature.wallet.core.TrustSimView
import app.getvela.wallet.feature.wallet.core.TrustView
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
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
import uniffi.vela_core_uniffi.TokenTrustCore

/**
 * PR 3, fix C on Android — **the confirm waits for the simulation's verdict.**
 *
 * The confirm gate looked at the request, the reading, the approval guard and
 * the fee, and not at the simulation: a person could confirm before the
 * balance changes were on screen — the one part of the sheet a site cannot
 * write. The wait is the core's (`sign_request`): this shell says when its
 * simulation is out (`sim_started`) and when its verdict is on the sheet
 * (`sim_settled`), and runs the one timer the core asks for.
 *
 * Through the REAL `sign_request`, `clear_signing`, `approval_guard` and
 * `fee_policy` machines and the real confirm gate, with the simulation
 * stubbed (the node answers when the test says) and the sign executor's timer
 * stopped (the deadline passes when the test says): what is pinned is the
 * controller's and the sheet's wiring, not the core's rule.
 */
class SigningSimWaitTest {
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

    /** Everything a deployed Safe's quote reads, answering: the fee is priced, and its coin can pay. */
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

    /**
     * The sign executor's timer, stopped: it remembers every wait it was
     * asked for and lets none pass until the test fires it.
     */
    private class StoppedClock {
        val asked = CopyOnWriteArrayList<Long>()
        private val passed = CompletableDeferred<Unit>()
        val timer: suspend (Long) -> Unit = { ms ->
            asked += ms
            passed.await()
        }

        fun fire() {
            passed.complete(Unit)
        }
    }

    /** How many times the node was asked to simulate. */
    private val simulations = java.util.concurrent.atomic.AtomicInteger()

    private fun controller(
        clock: StoppedClock,
        /** The node's answer to `eth_simulateV1`; `null` = this host has no simulator (the port's default). */
        simulate: (suspend () -> RpcResult?)? = null,
    ): SigningController = SigningController(
        scope = scope,
        relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
        feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
        accounts = StoreAccountPort(AccountStore(store)),
        signer = {
            object : UserOpSigner {
                override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion =
                    error("nothing is signed in these tests")
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
            override suspend fun simulate(chainId: Int, params: List<Any?>): RpcResult? {
                val node = simulate ?: return null
                simulations.incrementAndGet()
                return node()
            }
            override suspend fun judgeDeltas(chainId: Int, wallet: String, deltas: List<TrustAssetDelta>) = trustMachine(deltas)
        },
        simVerdictTimer = clock.timer,
    )

    /** The real `token_trust` machine's judged view of [deltas], as the app's port reads it. */
    private fun trustMachine(deltas: List<TrustAssetDelta>): TrustSimView? {
        val script = CoreScript(TokenTrustCore().asBridge()) { null }
        val event: TrustEvent = TrustEvent.SimDeltasComputed(address = safe.lowercase(), chain_id = 100, deltas = deltas)
        script.dispatch(Wire.json.encodeToString(TrustEvent.serializer(), event))
        return Wire.json.decodeFromString(TrustView.serializer(), script.viewJson()).sim?.takeIf { it.ready }
    }

    private fun transfer() = IncomingRequest(
        id = "r1", method = "eth_sendTransaction",
        paramsJson = JSONArray().put(JSONObject().put("from", safe).put("to", other).put("value", "0x38d7ea4c68000")).toString(),
        origin = "http://127.0.0.1:8137", transportId = "tab-1", chainId = 100,
    )

    private fun message() = IncomingRequest(
        id = "m1", method = "personal_sign",
        paramsJson = JSONArray().put("0x68656c6c6f").put(safe).toString(),
        origin = "http://127.0.0.1:8137", transportId = "tab-1", chainId = 100,
    )

    /** A clean run in which 0.001 of the chain's coin leaves the account. */
    private fun moves(): RpcResult {
        fun topic(address: String) = "0x" + "0".repeat(24) + address.removePrefix("0x").lowercase()
        val log = JSONObject()
            .put("address", "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")
            .put("topics", JSONArray().put("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef").put(topic(safe)).put(topic(other)))
            .put("data", "0x" + 1_000_000_000_000_000L.toString(16).padStart(64, '0'))
        val call = JSONObject().put("status", "0x1").put("returnData", "0x").put("logs", JSONArray().put(log))
        return RpcResult.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", JSONArray().put(JSONObject().put("calls", JSONArray().put(call)))))
    }

    /** A node that does not offer `eth_simulateV1` (Gnosis): it says so at once. */
    private fun notOffered(): RpcResult =
        RpcResult.Body(JSONObject().put("error", JSONObject().put("code", -32601).put("message", "the method eth_simulateV1 does not exist/is not available")))

    /** The core's one gate, over the four views exactly as the machines last wrote them. */
    private fun gate(c: SigningController): ConfirmState =
        SigningLive.confirmState(c.signJson.value, c.guardJson.value, c.clearJson.value, c.feeJson.value, null)

    private suspend fun until(c: SigningController, what: (ConfirmState) -> Boolean): ConfirmState = withTimeout(30_000) {
        var state = gate(c)
        while (!what(state)) {
            delay(20)
            state = gate(c)
        }
        state
    }

    /** The sheet as the live builder draws it now, under [confirm]. */
    private fun sheet(c: SigningController, confirm: ConfirmState, request: IncomingRequest = transfer()): SigningScreenModel =
        SigningLive.model(drawn, request, c.sign.value, c.clear.value, c.guard.value, c.fee.value, ctx, c.sim.value, confirm = confirm)

    private fun place(model: SigningScreenModel) = model.blocks.filterIsInstance<SigningBlock.Held>().single()

    private val checkingLine = "Checking what this transaction does…"
    private val couldNotCheck get() = SigningBlock.Warning(SigningTone.Caution, strings.t("componentsUi.signing.simUnavailableWarning"))

    // -- (a) held until the verdict is on the sheet --------------------------------------

    @Test
    fun `a transaction's confirm waits for the simulation's verdict and opens when it is drawn`() = runBlocking<Unit> {
        scriptRelay()
        val node = CompletableDeferred<RpcResult?>()
        val clock = StoppedClock()
        val c = controller(clock) { node.await() }
        c.open(transfer())

        // The reading is in, there is nothing to choose and the fee is priced:
        // the verdict is all that is missing, and the gate says so.
        val held = until(c) { it.block == ConfirmBlock.SimChecking }
        assertFalse(held.enabled)
        assertEquals(I18nKeys.Flows.SIM_CHECKING, held.key)
        assertTrue("the fee is ready: the confirm is held for the verdict alone", c.fee.value.confirm_fee_ready && c.fee.value.fee != null)
        assertTrue(c.sign.value.sim_checking)
        assertEquals(1, simulations.get())
        val waiting = sheet(c, held)
        assertFalse(waiting.confirmEnabled)
        assertEquals(checkingLine, strings.t(I18nKeys.Flows.SIM_CHECKING))
        assertEquals(checkingLine, waiting.confirmBlockLine)
        assertNull("the verdict's place says checking: its skeleton", place(waiting).shown)
        // The one timer in the flow is the core's, run by the sign executor
        // for the core's four seconds — and it has not passed.
        assertEquals(listOf(4_000L), clock.asked.toList())

        // A tap from a frame that had not caught up is not taken: it is
        // dispatched AHEAD of the verdict, and nothing starts signing.
        c.approve()

        // The node answers, and the tokens are judged: the verdict is drawn.
        node.complete(moves())
        val open = until(c) { it.enabled }
        assertNull(open.block)
        assertNull(open.key)
        assertEquals("the approve ahead of the verdict was not taken", SignPhase.Idle, c.sign.value.phase)
        assertFalse(c.sign.value.is_signing || c.sign.value.is_submitting)
        assertFalse(c.sign.value.sim_checking)
        assertNull(c.sign.value.sim_waited_out_key)
        val landed = sheet(c, open)
        assertTrue(landed.confirmEnabled)
        assertNull("the line is gone", landed.confirmBlockLine)
        val verdict = place(landed).shown as SigningBlock.Balances
        assertEquals(listOf("XDAI" to "−0.001"), verdict.rows.map { it.symbol to it.delta })
        // Same place, same sheet: only what is in the place changed.
        assertEquals(waiting.blocks.indexOfFirst { it is SigningBlock.Held }, landed.blocks.indexOfFirst { it is SigningBlock.Held })
        assertEquals(waiting.blocks.size, landed.blocks.size)
        assertEquals(place(waiting).rooms, place(landed).rooms)

        // The deadline passing AFTER the verdict changes nothing: the core
        // drops it, and no second timer was ever asked for.
        clock.fire()
        delay(300)
        assertTrue(gate(c).enabled)
        assertNull(c.sign.value.sim_waited_out_key)
        assertTrue(place(sheet(c, gate(c))).shown is SigningBlock.Balances)
        assertEquals(listOf(4_000L), clock.asked.toList())
        assertEquals(1, simulations.get())
    }

    // -- (b) the simulation never answers ------------------------------------------------

    @Test
    fun `when the simulation never answers, the deadline opens the confirm under a could-not-check caution, and a late answer replaces it`() = runBlocking<Unit> {
        scriptRelay()
        val node = CompletableDeferred<RpcResult?>()
        val clock = StoppedClock()
        val c = controller(clock) { node.await() }
        c.open(transfer())
        val held = until(c) { it.block == ConfirmBlock.SimChecking }
        val waiting = sheet(c, held)
        assertNull(place(waiting).shown)
        // Held for as long as the clock stands: nothing in this shell lets go by itself.
        delay(400)
        assertEquals(ConfirmBlock.SimChecking, until(c) { it.block != ConfirmBlock.FeeMeasuring }.block)

        // The core's deadline passes (its timer, answered by the sign executor).
        clock.fire()
        val open = until(c) { it.enabled }
        assertFalse(c.sign.value.sim_checking)
        assertEquals("componentsUi.signing.simUnavailableWarning", c.sign.value.sim_waited_out_key)
        assertEquals("the simulation is still out: nothing was made up for it", SigningController.SimOutcome.Pending, c.sim.value)
        val waitedOut = sheet(c, open)
        assertTrue(waitedOut.confirmEnabled)
        assertNull(waitedOut.confirmBlockLine)
        // The verdict's place: the could-not-check sentence, a caution —
        // the very block a node that cannot simulate is drawn as.
        assertEquals(couldNotCheck, place(waitedOut).shown)
        assertEquals(SigningLive.simBlocks(SimDeltas.couldNotCheck(), ctx).single(), place(waitedOut).shown)
        assertEquals(waiting.blocks.indexOfFirst { it is SigningBlock.Held }, waitedOut.blocks.indexOfFirst { it is SigningBlock.Held })
        assertEquals(waiting.blocks.size, waitedOut.blocks.size)
        assertEquals(place(waiting).rooms, place(waitedOut).rooms)
        // Signed now, the record would keep no balance changes: none was checked.
        assertNull(SigningController.approvedChanges(c.sim.value))

        // The answer lands after all: it takes the caution's place, the
        // core takes its line back, and the confirm is not shut again.
        node.complete(moves())
        withTimeout(20_000) { c.sim.first { it is SigningController.SimOutcome.Ready } }
        withTimeout(20_000) { c.sign.first { it.sim_waited_out_key == null } }
        val late = sheet(c, until(c) { it.enabled })
        assertTrue(late.confirmEnabled)
        assertNull(late.confirmBlockLine)
        assertEquals(listOf("−0.001"), (place(late).shown as SigningBlock.Balances).rows.map { it.delta })
        assertFalse(c.sign.value.sim_checking)
        assertEquals("one deadline, started by the core, and no clock of this shell's", listOf(4_000L), clock.asked.toList())
    }

    // -- (c) a message ---------------------------------------------------------------------

    @Test
    fun `a message is never held, keeps no place for a verdict and starts no timer`() = runBlocking<Unit> {
        val clock = StoppedClock()
        val c = controller(clock) { error("a message is never simulated") }
        c.open(message())
        assertNull("no simulation: no place", c.sim.value)

        val open = until(c) { it.enabled }
        assertNull(open.block)
        assertFalse(c.sign.value.sim_checking)
        assertNull(c.sign.value.sim_waited_out_key)
        val model = sheet(c, open, message())
        assertTrue(model.confirmEnabled)
        assertNull(model.confirmBlockLine)
        assertTrue(model.blocks.none { it is SigningBlock.Held })
        // The core starts its deadline on `sim_started` alone: none was asked
        // for, so none was said. (The timer's effect starts in the core's own
        // turn, before the view that follows it — so by now it would be here.)
        delay(200)
        assertTrue("a timer was started for a message: ${clock.asked}", clock.asked.isEmpty())
        assertEquals(0, simulations.get())
    }

    // -- (d) "not offered", at once --------------------------------------------------------

    @Test
    fun `a node that answers not-offered at once never leaves a held frame behind it`() = runBlocking<Unit> {
        scriptRelay()
        val clock = StoppedClock()
        val c = controller(clock) { notOffered() }
        // Every pair of (the request's view, the simulation's outcome) a frame could be drawn from.
        val frames = CopyOnWriteArrayList<Pair<SignView, SigningController.SimOutcome?>>()
        val watching = scope.launch(start = CoroutineStart.UNDISPATCHED) {
            combine(c.sign, c.sim) { sign, sim -> sign to sim }.collect { frames += it }
        }
        c.open(transfer())
        // Answered in the very step that sent it: the notice is the sheet's
        // before `open` returns, and the core was told both in that step.
        assertEquals(SimDeltas.couldNotCheck(), c.sim.value)
        assertEquals(1, simulations.get())

        val open = until(c) { it.enabled }
        assertNull(open.block)
        assertFalse(c.sign.value.sim_checking)
        assertNull("it settled: nothing was waited out", c.sign.value.sim_waited_out_key)
        val model = sheet(c, open)
        assertTrue(model.confirmEnabled)
        assertNull(model.confirmBlockLine)
        assertEquals(couldNotCheck, place(model).shown)

        // No frame draws the notice over a confirm held for it: with
        // everything else ready, wherever the gate says "checking" the
        // verdict's place says so too — whatever pair of views a frame was
        // drawn from.
        watching.cancel()
        val ready = FeeView(confirm_fee_ready = true, fee = c.fee.value.fee)
        assertTrue(frames.isNotEmpty())
        for ((sign, sim) in frames.filter { it.first.request != null }) {
            val held = SigningLive.confirmState(sign, c.guard.value, c.clear.value, ready, null).block == ConfirmBlock.SimChecking
            if (held) {
                assertEquals(
                    "a verdict stands over a confirm that still says checking",
                    SigningController.SimOutcome.Pending,
                    SigningLive.verdictShown(sim, sign),
                )
            }
        }
        // …and the same for the notice itself against every view the core
        // wrote for this request: where the gate holds the confirm for the
        // verdict the place still says "checking", and once the core's wait
        // is over the notice is what stands there.
        for (sign in frames.map { it.first }.filter { it.request != null }.distinct()) {
            val held = SigningLive.confirmState(sign, c.guard.value, c.clear.value, ready, null).block == ConfirmBlock.SimChecking
            val shown = SigningLive.verdictShown(SimDeltas.couldNotCheck(), sign)
            if (held) assertEquals(SigningController.SimOutcome.Pending, shown)
            if (!sign.sim_checking) assertEquals(SimDeltas.couldNotCheck(), shown)
        }
        // The last frame, and every one after the wait ended, is not held.
        assertFalse(frames.last().first.sim_checking)
        val ended = frames.indexOfLast { it.first.sim_checking }
        assertTrue("held again after the answer", frames.drop(ended + 1).none { it.first.sim_checking })

        // The deadline the core started with `sim_started` passes later: nothing moves.
        assertEquals(listOf(4_000L), clock.asked.toList())
        clock.fire()
        delay(300)
        assertTrue(gate(c).enabled)
        assertNull(c.sign.value.sim_waited_out_key)
        assertEquals(couldNotCheck, place(sheet(c, gate(c))).shown)
    }

    /**
     * A host with no simulator at all (the port's default answer, which
     * needs no wait): `sim_started` and `sim_settled` are said in one step,
     * no place is kept, and the confirm opens as soon as the fee is priced.
     */
    @Test
    fun `a host with no simulator holds nothing and keeps no place`() = runBlocking<Unit> {
        scriptRelay()
        val clock = StoppedClock()
        val c = controller(clock, simulate = null)
        c.open(transfer())
        assertNull("no simulator: the place is given back in the step that opened the request", c.sim.value)

        val open = until(c) { it.enabled }
        assertNull(open.block)
        assertFalse(c.sign.value.sim_checking)
        assertNull(c.sign.value.sim_waited_out_key)
        val model = sheet(c, open)
        assertTrue(model.confirmEnabled)
        assertTrue(model.blocks.none { it is SigningBlock.Held })
        // The deadline passing later reopens nothing and says nothing.
        clock.fire()
        delay(300)
        assertTrue(gate(c).enabled)
        assertNull(c.sign.value.sim_waited_out_key)
    }
}
