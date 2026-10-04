package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.BrowserTabs
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrLayer
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrOutcome
import app.getvela.wallet.feature.browser.core.DbrReadFailure
import app.getvela.wallet.feature.browser.core.DbrReason
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrTabView
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.browser.core.EngineInput
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.onboarding.core.Assertion
import app.getvela.wallet.feature.onboarding.core.FailureKind
import app.getvela.wallet.feature.onboarding.core.KeyMethod
import app.getvela.wallet.feature.onboarding.core.PasskeyFailure
import app.getvela.wallet.feature.send.core.Landing
import app.getvela.wallet.feature.send.core.LandingLine
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.StoreAccountPort
import app.getvela.wallet.feature.send.core.UserOpSigner
import app.getvela.wallet.feature.send.core.UserOpSpine
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.ClearOperation
import app.getvela.wallet.feature.signing.core.ClearShellResult
import app.getvela.wallet.feature.signing.core.ClearSigningEvent
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ConfirmBlock
import app.getvela.wallet.feature.signing.core.GuardEvent
import app.getvela.wallet.feature.signing.core.GuardOperation
import app.getvela.wallet.feature.signing.core.GuardShellResult
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignAccountRef
import app.getvela.wallet.feature.signing.core.SignApproveOpts
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignEvent
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignOperation
import app.getvela.wallet.feature.signing.core.SignShellResult
import app.getvela.wallet.feature.signing.core.SignSponsorship
import app.getvela.wallet.feature.signing.core.SignSubmitOutcome
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import app.getvela.wallet.feature.signing.core.SigningController
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
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
import org.junit.Assert.fail
import org.junit.Test
import uniffi.vela_core_uniffi.ApprovalGuardCore
import uniffi.vela_core_uniffi.ClearSigningCore
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.SignRequestCore

/**
 * Spec 099 on Android, through the REAL core: which tabs keep a live engine,
 * the read deadline and its failure kind, the tab's record and status line,
 * the signing sheet's one gate, the passkey's kind, and the landing's one
 * countdown. The rules are the core's; these prove this shell hands it the
 * right facts and reads its answers back.
 */
class BrowserTabsAndLayersTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val origin = "http://127.0.0.1:8137"
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    @After
    fun stop() = scope.cancel()

    // -- FR-004: the engine plan ------------------------------------------------

    private val tabs = (1..8).map { "t$it" }

    private fun explore(selected: String, recent: List<String>) =
        ExploreView(tabs = tabs.map { ExploreTab(it, url = "https://$it.example") }, selected_tab = selected, ready = true, recent_tabs = recent)

    @Test
    fun `the plan's input is the strip, its recency, the busy tabs and the live engines`() {
        val dapp = DbrView(tabs = listOf(DbrTabView("t2", busy = true), DbrTabView("t3", busy = false)))
        val input = BrowserTabs.input(explore("t1", listOf("t1", "t3", "t2")), dapp, live = listOf("t1", "t2", "t3"), pressure = true)
        assertEquals(EngineInput(tabs = tabs, selected = "t1", recent = listOf("t1", "t3", "t2"), busy = listOf("t2"), live = listOf("t1", "t2", "t3"), pressure = true), input)
    }

    @Test
    fun `past six live engines the least recently used idle tab goes, never the shown or a busy one`() {
        // Eight live engines; t8 the least recently used, t7 busy (a signature open).
        val recent = listOf("t1", "t2", "t3", "t4", "t5", "t6", "t7", "t8")
        val dapp = DbrView(tabs = listOf(DbrTabView("t7", busy = true)))
        val plan = BrowserTabs.plan(BrowserTabs.input(explore("t1", recent), dapp, live = tabs))
        // Kept: t1 (shown), t7 (busy), then t2…t5 by recency — six. Let go: t6 and t8.
        assertEquals(listOf("t6", "t8"), plan.suspend)

        // Under memory pressure: the shown tab and the busy one only.
        val pressed = BrowserTabs.plan(BrowserTabs.input(explore("t1", recent), dapp, live = tabs, pressure = true))
        assertEquals(tabs - listOf("t1", "t7"), pressed.suspend)

        // A tab with no engine has nothing to suspend; the plan never wakes one.
        val few = BrowserTabs.plan(BrowserTabs.input(explore("t8", recent), DbrView(), live = listOf("t8", "t1")))
        assertTrue(few.suspend.isEmpty())
    }

    private fun dbrHost(read: suspend (String) -> BrowserExecutor.ReadAnswer = { BrowserExecutor.ReadAnswer.Body(JSONObject().put("result", "0x10")) }, shrinkDeadlineTo: Double? = null): Pair<CoreHost<DbrView>, java.util.concurrent.CopyOnWriteArrayList<Pair<String, JSONObject>>> {
        val delivered = java.util.concurrent.CopyOnWriteArrayList<Pair<String, JSONObject>>()
        val executor = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) { delivered += tab to JSONObject(messageJson) }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean) = read(method)
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
            },
        )
        val host = CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer()) { operation ->
                // The core's own deadline is 30 s; a test cannot wait that long, so
                // it is shortened on its way in — after the core set it.
                if (operation is DbrOperation.Read) assertEquals("the core sets the deadline", 30_000.0, operation.deadline_ms, 0.0)
                executor.perform(if (operation is DbrOperation.Read && shrinkDeadlineTo != null) operation.copy(deadline_ms = shrinkDeadlineTo) else operation)
            },
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("dapp fault: $error", error) },
        )
        host.dispatch(DbrEvent.Start, DbrEvent.serializer())
        host.dispatch(DbrEvent.NetworksChanged(listOf(1, 100)), DbrEvent.serializer())
        host.dispatch(DbrEvent.AccountsUpdated(listOf(safe)), DbrEvent.serializer())
        host.dispatch(DbrEvent.AccountSwitched(safe, 1.0e12), DbrEvent.serializer())
        return host to delivered
    }

    private fun page(h: CoreHost<DbrView>, tab: String, message: JSONObject, now: Double = 1.0e12) =
        h.dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = origin, is_main_frame = true, message_json = message.toString(), now_ms = now), DbrEvent.serializer())

    private fun hello(h: CoreHost<DbrView>, tab: String, doc: String) = page(h, tab, JSONObject().put("t", "hello").put("doc", doc))

    private fun ask(h: CoreHost<DbrView>, tab: String, doc: String, id: String, method: String) =
        page(h, tab, JSONObject().put("t", "req").put("doc", doc).put("id", id).put("method", method).put("params", JSONArray()))

    private suspend fun answerFor(delivered: List<Pair<String, JSONObject>>, id: String): JSONObject {
        withTimeout(10_000) { while (delivered.none { it.second.optString("id") == id }) delay(10) }
        return delivered.first { it.second.optString("id") == id }.second
    }

    /**
     * A suspended tab's page is gone (the desktop's `page_gone_events`): its
     * old document's messages are never answered, and the page that loads
     * when the tab is shown again is a new document the core serves.
     */
    @Test
    fun `a suspended tab's page is gone to the core, and its next page is served`() = runBlocking<Unit> {
        val (h, delivered) = dbrHost()
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "t1", "d1")
        ask(h, "t1", "d1", "before", "eth_chainId")
        answerFor(delivered, "before")
        BrowserTabs.pageGone("t1", 1.0e12 + 5).forEach { h.dispatch(it, DbrEvent.serializer()) }
        ask(h, "t1", "d1", "stale", "eth_chainId")
        hello(h, "t1", "d2")
        ask(h, "t1", "d2", "woken", "eth_chainId")
        assertEquals("0x1", answerFor(delivered, "woken").getString("result"))
        assertTrue("the gone page's late request is not answered", delivered.none { it.second.optString("id") == "stale" })
        assertFalse("not busy: nothing open", h.view.value.tabs.first { it.tab == "t1" }.busy)
    }

    // -- FR-008/009: the read deadline and its failure kind -------------------

    private fun readOp(deadline: Double) = DbrOperation.Read(tab = "t1", id = "1", chain_id = 1, method = "eth_call", params_json = "[]", bundler = false, deadline_ms = deadline)

    @Test
    fun `a read that never answers is answered timed out at its deadline, and its late body is dropped`() = runBlocking<Unit> {
        val late = java.util.concurrent.atomic.AtomicBoolean(false)
        val hanging = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports by NoPorts {
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer = awaitCancellation()
            },
            now = { 42.0 },
        )
        val started = System.currentTimeMillis()
        assertEquals(DbrShellResult.ReadAnswered(null, now_ms = 42.0, failure = DbrReadFailure.TimedOut), hanging.perform(readOp(150.0)))
        assertTrue("answered at the deadline, not after", System.currentTimeMillis() - started < 5_000)

        // A port that cannot be interrupted at all still costs the page nothing past the deadline.
        val stubborn = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports by NoPorts {
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer {
                    @Suppress("BlockingMethodInNonBlockingContext")
                    Thread.sleep(2_000)
                    late.set(true)
                    return BrowserExecutor.ReadAnswer.Body(JSONObject().put("result", "0xlate"))
                }
            },
        )
        val at = System.currentTimeMillis()
        // On a pool of threads, as the app's executors run: the stuck read holds one, the deadline fires on another.
        val answer = kotlinx.coroutines.withContext(Dispatchers.Default) { stubborn.perform(readOp(150.0)) } as DbrShellResult.ReadAnswered
        assertTrue("did not wait for the stuck I/O: ${System.currentTimeMillis() - at} ms", System.currentTimeMillis() - at < 1_500)
        assertNull(answer.body_json)
        assertEquals(DbrReadFailure.TimedOut, answer.failure)
        assertFalse("the late body arrived after the answer", late.get())
    }

    @Test
    fun `a read the pool gave up on says why — every endpoint rate-limited, or no endpoint`() = runBlocking<Unit> {
        fun executor(answer: BrowserExecutor.ReadAnswer) = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports by NoPorts {
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean) = answer
            },
            now = { 7.0 },
        )
        assertEquals(DbrReadFailure.RateLimited, (executor(BrowserExecutor.ReadAnswer.NoAnswer(rateLimited = true)).perform(readOp(30_000.0)) as DbrShellResult.ReadAnswered).failure)
        assertEquals(DbrReadFailure.NoEndpoint, (executor(BrowserExecutor.ReadAnswer.NoAnswer()).perform(readOp(30_000.0)) as DbrShellResult.ReadAnswered).failure)
        val body = executor(BrowserExecutor.ReadAnswer.Body(JSONObject().put("result", "0x1"))).perform(readOp(30_000.0)) as DbrShellResult.ReadAnswered
        assertEquals(JSONObject().put("result", "0x1").toString(), body.body_json)
        assertNull(body.failure)
        assertEquals(7.0, body.now_ms, 0.0)
    }

    /**
     * Through the REAL machine: a read nobody answers in time ends the page's
     * request as the network's, the tab's status line names it in the core's
     * words, and the status panel lists it.
     */
    @Test
    fun `a timed-out read is the network's, on the status line and in the panel`() = runBlocking<Unit> {
        val (h, delivered) = dbrHost(read = { awaitCancellation() }, shrinkDeadlineTo = 100.0)
        withTimeout(10_000) { h.view.first { it.ready } }
        hello(h, "t1", "d1")
        ask(h, "t1", "d1", "slow", "eth_blockNumber")
        val error = answerFor(delivered, "slow").getJSONObject("error")
        assertTrue("an error the page can read: $error", error.has("code"))
        val tab = withTimeout(10_000) { h.view.first { view -> view.tabs.any { it.tab == "t1" && it.last_failure != null } } }.tabs.first { it.tab == "t1" }
        val note = tab.last_failure!!
        assertEquals(DbrLayer.Network, note.layer)
        assertEquals(DbrReason.TimedOut, note.reason)
        assertEquals("eth_blockNumber", note.method)
        assertEquals("the core's line is the vocabulary's", note.reason.key, note.key)

        val status = ExploreLive.status(tab, reloaded = false, seen = null, strings = strings)!!
        assertEquals("${strings.t(DbrReason.TimedOut.key)} · eth_blockNumber", status.text)
        assertTrue(status.warning)
        assertNull("put away, it stays away until it says something else", ExploreLive.status(tab, reloaded = false, seen = status.seen, strings = strings))
        assertEquals(strings.t(I18nKeys.BrowserStatus.RELOADED), ExploreLive.status(tab, reloaded = true, seen = status.seen, strings = strings)!!.text)

        // The panel: the core carries the tab's whole record only while it is open.
        assertNull(h.view.value.inspector)
        h.dispatch(DbrEvent.InspectorOpened("t1"), DbrEvent.serializer())
        val record = withTimeout(10_000) { h.view.first { it.inspector != null } }.inspector!!
        val row = record.rows.last()
        assertEquals("eth_blockNumber", row.method)
        assertEquals(DbrOutcome.Failed, row.outcome)
        assertEquals(DbrReason.TimedOut, row.reason)
        val panel = ExploreLive.inspector(record, strings)!!
        assertEquals("newest first", "eth_blockNumber", panel.rows.first().method)
        assertEquals(strings.t(DbrReason.TimedOut.key), panel.rows.first().outcome)
        assertTrue("the core's report, to copy: ${panel.report}", panel.report.contains("eth_blockNumber") && panel.report.contains("network/timed_out"))
        h.dispatch(DbrEvent.InspectorClosed, DbrEvent.serializer())
        withTimeout(10_000) { h.view.first { it.inspector == null } }
    }

    @Test
    fun `every reason, page and wallet state the core can name has a line in the corpus`() {
        for (key in DbrReason.entries.map { it.key } + app.getvela.wallet.feature.browser.core.DbrPageState.entries.map { it.key } +
            app.getvela.wallet.feature.browser.core.DbrProviderState.entries.map { it.key }) {
            val text = strings.t(key)
            assertTrue("$key has no line", text.isNotBlank() && text != key)
        }
    }

    // -- R7: the signing sheet's one gate, the core's -------------------------

    private class SignRig(val host: CoreHost<SignView>, val answers: java.util.concurrent.CopyOnWriteArrayList<SignOperation.SendResponse>)

    /** The REAL signing machine; a submit is answered with [submit]. */
    private fun signRig(submit: SignSubmitOutcome): SignRig {
        val answers = java.util.concurrent.CopyOnWriteArrayList<SignOperation.SendResponse>()
        val host = CoreHost(
            bridge = SignRequestCore().asBridge(), scope = scope, initial = SignView(), serializer = SignView.serializer(),
            perform = JsonShell.perform(SignOperation.serializer(), SignShellResult.serializer()) { operation ->
                when (operation) {
                    is SignOperation.SendResponse -> { answers += operation; SignShellResult.Responded }
                    is SignOperation.CheckBundlerFunding -> SignShellResult.PreCheck(null)
                    is SignOperation.AttemptSponsorship -> SignShellResult.Sponsorship(SignSponsorship.Denied(null))
                    is SignOperation.SignAndSubmit -> SignShellResult.Submit(submit, 2.0e12)
                    is SignOperation.PersistRecord -> SignShellResult.RecordPersisted
                    is SignOperation.UpdateRecord -> SignShellResult.RecordUpdated
                    is SignOperation.ClearToPost -> SignShellResult.Responded
                    is SignOperation.DeleteRecord -> SignShellResult.RecordUpdated
                    is SignOperation.SwitchActiveAccount -> SignShellResult.AccountSwitched
                }
            },
            escapedFailure = JsonShell.escapedFailure(SignOperation.serializer(), SignShellResult.serializer(), fallback = SignShellResult.Responded) { SignShellResult.Responded },
            onFault = { error -> throw AssertionError("sign fault: $error", error) },
        )
        return SignRig(host, answers)
    }

    private val message = """["0x68656c6c6f","$safe"]"""

    private suspend fun arrive(rig: SignRig, method: String = "personal_sign", params: String = message) {
        rig.host.dispatch(SignEvent.NetworksChanged(listOf(1, 100)), SignEvent.serializer())
        rig.host.dispatch(SignEvent.AccountsChanged(listOf(SignAccountRef(safe, "cred-1")), 0), SignEvent.serializer())
        rig.host.dispatch(
            SignEvent.RequestArrived(
                id = "s1", method = method, params_json = params, origin = origin, transport_id = "t1",
                dedicated_transport = true, per_request_chain = 100, granted_address = safe, now_ms = 1.0e12,
            ),
            SignEvent.serializer(),
        )
        withTimeout(10_000) { rig.host.view.first { it.surface == SignSurface.Sheet && !it.reconcile_pending } }
    }

    /** The REAL clear-signing machine's view of [method], as raw JSON. */
    private suspend fun clearJson(method: String, params: String): String {
        val host = CoreHost(
            bridge = ClearSigningCore().asBridge(), scope = scope, initial = ClearSigningView(), serializer = ClearSigningView.serializer(),
            perform = JsonShell.perform(ClearOperation.serializer(), ClearShellResult.serializer()) { op ->
                when (op) {
                    is ClearOperation.HttpGet -> ClearShellResult.DescriptorFetched(op.path, null)
                    is ClearOperation.RpcEthCall -> ClearShellResult.RpcAnswer(op.probe, op.chain_id, op.to, null, true)
                    is ClearOperation.SelectorDbLookup -> ClearShellResult.SelectorCandidates()
                    is ClearOperation.Timer -> ClearShellResult.TimedOut(op.token)
                    ClearOperation.Now -> ClearShellResult.Clock(1.0e12)
                }
            },
            escapedFailure = JsonShell.escapedFailure(ClearOperation.serializer(), ClearShellResult.serializer(), fallback = ClearShellResult.Clock(1.0e12)) { ClearShellResult.Clock(1.0e12) },
        )
        host.dispatch(SigningController.clearKickoff(method, params, 100, origin)!!, ClearSigningEvent.serializer())
        // Read: a transaction resolves; a message is presented as it is.
        withTimeout(10_000) {
            host.view.first { !it.resolving && it.surface != app.getvela.wallet.feature.signing.core.ClearSurface.Loading && it.surface != app.getvela.wallet.feature.signing.core.ClearSurface.None }
        }
        return host.viewJson.value!!
    }

    /** The REAL approval guard's view of [method], as raw JSON. */
    private suspend fun guardJson(method: String, params: String): String {
        val host = CoreHost(
            bridge = ApprovalGuardCore().asBridge(), scope = scope, initial = GuardView(), serializer = GuardView.serializer(),
            perform = JsonShell.perform(GuardOperation.serializer(), GuardShellResult.serializer()) { op ->
                when (op) {
                    is GuardOperation.ReadTokenMetadata -> GuardShellResult.MetaResolved(null)
                    is GuardOperation.ReadErc20Allowance -> GuardShellResult.AllowanceRead(null)
                    is GuardOperation.ReadErc20Balance -> GuardShellResult.BalanceRead(null)
                }
            },
            escapedFailure = JsonShell.escapedFailure(GuardOperation.serializer(), GuardShellResult.serializer(), fallback = GuardShellResult.MetaResolved(null)) { GuardShellResult.MetaResolved(null) },
        )
        val before = host.commits.value
        host.dispatch(GuardEvent.ApprovalDetected(method = method, params_json = params, chain_id = 100, wallet_address = safe, now_ms = 1.0e12), GuardEvent.serializer())
        withTimeout(10_000) { host.commits.first { it > before } }
        return host.viewJson.value!!
    }

    @Test
    fun `a message has no fee to wait for, so the core's gate arms with no fee session`() = runBlocking<Unit> {
        val rig = signRig(SignSubmitOutcome.Succeeded("0xsig"))
        arrive(rig)
        val gate = SigningLive.confirmState(rig.host.viewJson.value, guardJson("personal_sign", message), clearJson("personal_sign", message), feeJson = null, speedTier = null)
        assertTrue("armed: $gate", gate.enabled)
        assertNull(gate.block)
        assertNull(gate.key)

        // A view that does not read keeps the slide shut — never a guess.
        assertFalse(SigningLive.confirmState(null, "{}", "{}", null, null).enabled)
        assertFalse(SigningLive.confirmState("{\"nonsense\":1}", "{}", "{}", null, null).enabled)
    }

    @Test
    fun `a transaction waits for its fee, and the sheet says so under the slide`() = runBlocking<Unit> {
        val tx = """[{"to":"$safe","value":"0x1"}]"""
        val rig = signRig(SignSubmitOutcome.Succeeded("0xtx"))
        arrive(rig, "eth_sendTransaction", tx)
        val gate = SigningLive.confirmState(rig.host.viewJson.value, guardJson("eth_sendTransaction", tx), clearJson("eth_sendTransaction", tx), feeJson = null, speedTier = null)
        assertFalse(gate.enabled)
        assertEquals(ConfirmBlock.FeeMeasuring, gate.block)
        assertEquals("componentsUi.signing.confirmBlock.feeMeasuring", gate.key)
    }

    /**
     * R7 + R8: a passkey prompt that failed holds the request's answer; the
     * core's gate names `answered` (try again), the sheet says the passkey
     * failed — not "couldn't submit" — and offers Try again.
     */
    @Test
    fun `a held signer failure shuts the slide as answered, in the signer's words`() = runBlocking<Unit> {
        val rig = signRig(SignExecutor.outcomeOf(UserOpSpine.Failure.Signer(FailureKind.Other, "the prompt broke")))
        arrive(rig)
        rig.host.dispatch(SignEvent.ApproveTapped(SignApproveOpts()), SignEvent.serializer())
        val failed = withTimeout(10_000) { rig.host.view.first { it.error != null } }
        assertEquals(SignErrorKind.SignerFailed, failed.error?.kind)
        assertTrue("a failed prompt may work next time", failed.failure_retryable)
        assertEquals(ConfirmBlock.Answered, failed.confirm_block)

        val gate = SigningLive.confirmState(rig.host.viewJson.value, guardJson("personal_sign", message), clearJson("personal_sign", message), null, null)
        assertFalse(gate.enabled)
        assertEquals(ConfirmBlock.Answered, gate.block)
        assertEquals("componentsUi.signing.confirmBlock.answeredRetry", gate.key)

        val drawn = SigningFixtures.build(SigningScreenState.CS1, strings)
        val ctx = SigningLive.Context(strings, "Gnosis", androidx.compose.ui.graphics.Color.Red, "XDAI", "Me", safe)
        val model = SigningLive.model(drawn, IncomingRequest("s1", "personal_sign", message, origin, "t1", 100), failed, ClearSigningView(), GuardView(), app.getvela.wallet.feature.send.core.FeeView(), ctx, confirm = gate)
        assertFalse(model.confirmEnabled)
        assertEquals(strings.t("componentsUi.signing.confirmBlock.answeredRetry"), model.confirmBlockLine)
        val receipt = model.receipt!!
        assertTrue("the signer is named: ${receipt.captions}", receipt.captions.contains(strings.t(I18nKeys.BrowserStatus.REASON_SIGNER_FAILED)))
        assertEquals(strings.t(I18nKeys.Flows.TX_RETRY), receipt.retry)
    }

    // -- R8: the passkey's kind, from the app's own classifier -----------------

    @Test
    fun `each passkey kind becomes the core's signer kind, and a cancel stays a cancel`() = runBlocking<Unit> {
        for ((kind, expected) in listOf(
            FailureKind.NotSupported to SignErrorKind.SignerUnavailable,
            FailureKind.NotDiscoverable to SignErrorKind.SignerNotDiscoverable,
            FailureKind.Other to SignErrorKind.SignerFailed,
        )) {
            val outcome = SignExecutor.outcomeOf(UserOpSpine.Failure.Signer(kind, "x"))
            assertEquals(SignSubmitOutcome.Failed("x", signer = kind), outcome)
            val rig = signRig(outcome)
            arrive(rig)
            rig.host.dispatch(SignEvent.ApproveTapped(SignApproveOpts()), SignEvent.serializer())
            val view = withTimeout(10_000) { rig.host.view.first { it.error != null } }
            assertEquals(kind.name, expected, view.error?.kind)
        }
        assertEquals(SignSubmitOutcome.PasskeyCancelled, SignExecutor.outcomeOf(UserOpSpine.Failure.PasskeyCancelled))
        // A failure that is not the passkey's stays the submit's.
        assertNull((SignExecutor.outcomeOf(UserOpSpine.Failure.Other("relay")) as SignSubmitOutcome.Failed).signer)
    }

    /** The ceremony's classifier result reaches the outcome: the spine carries the kind, never folds it into "other". */
    @Test
    fun `the spine carries the passkey classifier's kind out of the ceremony`() = runBlocking<Unit> {
        val credential = "aabbcc"
        val publicKey = "04" + "11".repeat(64)
        val store = FakeStore()
        val accounts = AccountStore(store)
        accounts.saveAccount(
            JSONObject().put("id", credential).put("address", safe).put("public_key_hex", publicKey).put(
                "keys",
                JSONArray().put(JSONObject().put("credential_id", credential).put("public_key_hex", publicKey).put("name", "Key 1").put("transports", "internal")),
            ),
        )
        fun spineThrowing(kind: FailureKind) = UserOpSpine(
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            accounts = StoreAccountPort(accounts),
            signer = {
                object : UserOpSigner {
                    override suspend fun sign(challenge: ByteArray, credentialIdHex: String?, transports: String, method: KeyMethod): Assertion =
                        throw PasskeyFailure(kind, "platform said no")
                }
            },
        )
        for (kind in listOf(FailureKind.NotSupported, FailureKind.NotDiscoverable, FailureKind.Other, FailureKind.Cancelled)) {
            try {
                spineThrowing(kind).signMessage(100, safe, ByteArray(32))
                fail("$kind: the ceremony failed, nothing may be signed")
            } catch (refused: UserOpSpine.Refused) {
                val expected = if (kind == FailureKind.Cancelled) UserOpSpine.Failure.PasskeyCancelled else UserOpSpine.Failure.Signer(kind, "platform said no")
                assertEquals(kind.name, expected, refused.failure)
            }
        }
    }

    // -- R6: the landing's one countdown --------------------------------------

    @Test
    fun `the landing pace is the core's ladder, counted from the relay's send`() {
        assertEquals(LandingLine.Waiting, Landing.pace(null, 15, 5_000.0).line)
        assertNull("the ring roams before the send", Landing.pace(null, 15, 5_000.0).progress)
        assertEquals(LandingLine.None, Landing.pace(1_000.0, null, 5_000.0).line)
        val remaining = Landing.pace(1_000.0, 15, 7_000.0)
        assertEquals(LandingLine.Remaining, remaining.line)
        assertEquals(9, remaining.seconds)
        assertNotNull(remaining.progress)
        assertEquals(LandingLine.Elapsed, Landing.pace(1_000.0, 15, 21_000.0).line)
        assertEquals(20, Landing.pace(1_000.0, 15, 21_000.0).seconds)
        assertEquals(LandingLine.Slow, Landing.pace(1_000.0, 15, 61_000.0).line)
    }

    /** Ports that answer nothing — each test overrides the one it means. */
    private object NoPorts : BrowserExecutor.Ports {
        override fun deliver(tab: String, messageJson: String) = Unit
        override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer = BrowserExecutor.ReadAnswer.NoAnswer()
        override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
        override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
        override fun cancelSigning(tab: String, id: String) = Unit
        override fun saveConnectionRecord(row: JSONObject) = Unit
    }
}
