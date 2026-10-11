package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrReason
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrTabView
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore

/**
 * Device walk 2026-10-11, item 7: the test dApp asked `personal_sign` before
 * it was connected, the page was told 4100 and its tab said 「此网站尚未连接 ·
 * personal_sign」 — and went on saying it after the site connected, for as
 * long as the tab lived.
 *
 * "This site is not connected" is a statement about now. The core's tab view
 * no longer counts that trouble once the tab's site is connected, and this
 * shell draws the banner from the view it has NOW: `ExploreLive.home` is
 * handed the browser machine's current tab on every change and keeps nothing
 * of the last one. Through the REAL `dapp_browser` machine, this shell's
 * executor and the builder the screen uses: sign before connecting → the
 * banner; connect → gone, with nobody putting it away; disconnect → said
 * again; and another trouble is not cleared by connecting.
 */
class BrowserStatusBannerTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val origin = "http://127.0.0.1:8137"
    private val tab = "tab-1"

    private val zh: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("zh") }
    }

    private val delivered = CopyOnWriteArrayList<JSONObject>()
    private val deliveries = MutableStateFlow(0)

    @After
    fun stop() = scope.cancel()

    private fun host(): CoreHost<DbrView> {
        val executor = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) {
                    delivered += JSONObject(messageJson)
                    deliveries.update { it + 1 }
                }
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer =
                    BrowserExecutor.ReadAnswer.Body(JSONObject().put("jsonrpc", "2.0").put("id", 1).put("result", "0x10"))
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
            },
        )
        return CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        ).also { h ->
            h.dispatch(DbrEvent.Start, DbrEvent.serializer())
            h.dispatch(DbrEvent.NetworksChanged(listOf(1, 100)), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountsUpdated(listOf(account)), DbrEvent.serializer())
            h.dispatch(DbrEvent.AccountSwitched(account, 1.0e12), DbrEvent.serializer())
        }
    }

    private var clock = 1.0e12

    private fun page(h: CoreHost<DbrView>, message: JSONObject) {
        clock += 1_000
        h.dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = origin, is_main_frame = true, message_json = message.toString(), now_ms = clock), DbrEvent.serializer())
    }

    private fun ask(h: CoreHost<DbrView>, id: String, method: String, params: JSONArray = JSONArray()) =
        page(h, JSONObject().put("t", "req").put("doc", "d1").put("id", id).put("method", method).put("params", params))

    /** Woken by what it waits for; the limit only turns a machine that never answers into a failure that says so. */
    private suspend fun <T> settled(orElse: String, wait: suspend () -> T): T =
        withTimeoutOrNull(HANG_MS) { wait() } ?: throw AssertionError(orElse)

    private suspend fun answer(id: String): JSONObject = settled("the page was never answered for $id: $delivered") {
        deliveries.first { delivered.any { it.optString("id") == id } }
        delivered.first { it.optString("id") == id }
    }

    /** The tab as the machine states it now, once [test] holds of it. */
    private suspend fun tabView(h: CoreHost<DbrView>, what: String, test: (DbrTabView) -> Boolean): DbrTabView =
        settled("the tab never came to: $what — it says ${h.view.value.tabs}") {
            h.view.first { view -> view.tabs.any { it.tab == tab && test(it) } }.tabs.first { it.tab == tab }
        }

    /** The banner the screen draws for [view]'s tab — the builder `VelaNavHost` calls, with nothing put away. */
    private fun banner(view: DbrTabView?) = ExploreLive.home(
        ExploreFixtures.buildState(ExploreScreenState.E1, zh),
        ExploreView(),
        BhistView(),
        engine = null,
        strings = zh,
        tab = view,
    ).browser.status

    @Test
    fun `not connected is said until the site connects, and again once it is disconnected`() = runBlocking<Unit> {
        val h = host()
        settled("the machine never read its store") { h.view.first { it.ready } }
        page(h, JSONObject().put("t", "hello").put("doc", "d1"))

        // -- the page signs before it connects -----------------------------------------
        ask(h, "s1", "personal_sign", JSONArray().put("0x68656c6c6f").put(account))
        assertEquals(4100, answer("s1").getJSONObject("error").getInt("code"))
        val refused = tabView(h, "a not-connected trouble") { it.last_failure?.reason == DbrReason.NotConnected }
        val said = banner(refused)
        assertNotNull("the tab says the site is not connected", said)
        assertEquals("此网站尚未连接 · personal_sign", said!!.text)

        // -- it connects: the banner is gone, though nobody put it away ----------------
        ask(h, "c1", "eth_requestAccounts")
        settled("no consent sheet") { h.view.first { it.consent != null } }
        clock += 1_000
        h.dispatch(DbrEvent.ConsentApproved(clock), DbrEvent.serializer())
        assertEquals(account.lowercase(), answer("c1").getJSONArray("result").getString(0).lowercase())
        val connected = tabView(h, "connected") { it.connected_address != null }
        assertNull("the view no longer counts it: ${connected.last_failure}", connected.last_failure)
        assertEquals(0, connected.failed_recent)
        assertNull("the banner is drawn from the view as it is now", banner(connected))

        // -- disconnected, the statement is true again ---------------------------------
        h.dispatch(DbrEvent.RevokeRequested(origin), DbrEvent.serializer())
        val disconnected = tabView(h, "disconnected, the old trouble true again") { it.connected_address == null && it.last_failure != null }
        assertEquals("此网站尚未连接 · personal_sign", banner(disconnected)?.text)
    }

    /** Connecting ends "not connected" and nothing else: a different trouble on the tab is still said. */
    @Test
    fun `connecting does not clear another trouble`() = runBlocking<Unit> {
        val h = host()
        settled("the machine never read its store") { h.view.first { it.ready } }
        page(h, JSONObject().put("t", "hello").put("doc", "d1"))
        ask(h, "c1", "eth_requestAccounts")
        settled("no consent sheet") { h.view.first { it.consent != null } }
        clock += 1_000
        h.dispatch(DbrEvent.ConsentApproved(clock), DbrEvent.serializer())
        answer("c1")
        // A network this wallet does not have: the page is told 4902, and the tab says so.
        ask(h, "w1", "wallet_switchEthereumChain", JSONArray().put(JSONObject().put("chainId", "0x539")))
        assertEquals(4902, answer("w1").getJSONObject("error").getInt("code"))
        val troubled = tabView(h, "an unknown-chain trouble") { it.last_failure?.reason == DbrReason.UnknownChain }
        val said = banner(troubled)
        assertNotNull(said)
        assertEquals("${zh.t(DbrReason.UnknownChain.key)} · wallet_switchEthereumChain", said!!.text)
    }

    /** A line the person put away stays away; the builder is told which, and keeps no memory of its own. */
    @Test
    fun `the banner is what the current view says, less what was put away`() {
        val note = app.getvela.wallet.feature.browser.core.DbrFailureNote(
            layer = app.getvela.wallet.feature.browser.core.DbrLayer.Wallet,
            reason = DbrReason.NotConnected,
            method = "personal_sign",
            key = DbrReason.NotConnected.key,
        )
        val troubled = DbrTabView(tab = tab, origin = origin, failed_recent = 1, last_failure = note)
        val line = ExploreLive.status(troubled, reloaded = false, seen = null, strings = zh)!!
        assertEquals("此网站尚未连接 · personal_sign", line.text)
        assertNull("put away", ExploreLive.status(troubled, reloaded = false, seen = line.seen, strings = zh))
        // The same builder, the next view: nothing of the last call is in it.
        assertNull(ExploreLive.status(troubled.copy(failed_recent = 0, last_failure = null), reloaded = false, seen = null, strings = zh))
        assertEquals(line.text, ExploreLive.status(troubled, reloaded = false, seen = null, strings = zh)?.text)
    }

    private companion object {
        /** A hang limit, not a deadline. */
        const val HANG_MS = 120_000L
    }
}
