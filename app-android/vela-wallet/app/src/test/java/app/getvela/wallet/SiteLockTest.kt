package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrConsentView
import app.getvela.wallet.feature.browser.core.DbrEvent
import app.getvela.wallet.feature.browser.core.DbrOperation
import app.getvela.wallet.feature.browser.core.DbrShellResult
import app.getvela.wallet.feature.browser.core.DbrTabView
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.browser.core.EngineState
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.browser.core.OriginLock
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.SiteLock
import app.getvela.wallet.feature.explore.components.siteLockWords
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.DappBrowserCore

/**
 * Device walk 2026-10-11, item 11: a padlock was drawn for
 * `http://127.0.0.1:8137`, in the address bar and on the connect sheet. Every
 * surface drew its lock from the core's `secure` — "safe to use the wallet
 * from", true for http on this device — and a padlock says "encrypted".
 *
 * The mark is the core's `lock` now (`OriginLock`): the padlock for `https`
 * alone; a neutral mark for a local page, never a padlock and never the
 * warning; the open lock for public `http`. On every surface that draws one:
 * the address bar, the site menu, the connection panel, the connect sheet.
 */
class SiteLockTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("zh") }
    }
    private val fallback = ExploreFixtures.buildState(ExploreScreenState.E2, strings)

    private val local = "http://127.0.0.1:8137"
    private val lan = "http://192.168.1.5:3000"
    private val https = "https://arcorbs.game3.app"
    private val public = "http://dapp.example"

    private fun home(url: String, tab: DbrTabView?) = ExploreLive.home(
        fallback,
        ExploreView(tabs = listOf(ExploreTab("t1", url, "", url.substringAfter("://").substringBefore('/'))), selected_tab = "t1", ready = true),
        BhistView(),
        EngineState(url = url, origin = url.trimEnd('/'), host = url.substringAfter("://").substringBefore('/'), shown = url),
        strings,
        tab,
    )

    /** The core's table, and this shell's reading of it: three words, three marks, nothing in between. */
    @Test
    fun `a padlock is the core's word for https, and for nothing else`() {
        assertEquals(OriginLock.Secure, OriginLock.of(https))
        assertEquals(OriginLock.Local, OriginLock.of(local))
        assertEquals(OriginLock.Local, OriginLock.of("http://localhost:3000"))
        assertEquals(OriginLock.Local, OriginLock.of(lan))
        assertEquals(OriginLock.Insecure, OriginLock.of(public))
        assertEquals(OriginLock.Insecure, OriginLock.of(null))
        assertEquals(SiteLock.Secure, ExploreLive.siteLock(OriginLock.Secure))
        assertEquals(SiteLock.Local, ExploreLive.siteLock(OriginLock.Local))
        assertEquals(SiteLock.Insecure, ExploreLive.siteLock(OriginLock.Insecure))
        assertEquals("no word from the core is no padlock", SiteLock.Insecure, ExploreLive.siteLock(null))
    }

    /** The walk's own page: `secure` is true for it — the wallet may be used there — and no surface draws a padlock. */
    @Test
    fun `the test dApp on loopback is a local page on every surface`() {
        val tab = DbrTabView(tab = "t1", origin = local, secure = true, lock = OriginLock.Local)
        val model = home("$local/", tab)
        assertTrue("the bar has a mark for a page that is there", model.browser.lockShown)
        assertEquals("the address bar", SiteLock.Local, model.browser.lock)
        assertEquals("the site menu", SiteLock.Local, model.siteMenuSheet.lock)
        assertEquals("the connection panel", SiteLock.Local, model.connection.lock)
        val consent = ExploreLive.consent(fallback.connection, DbrConsentView(tab = "t1", origin = local, lock = OriginLock.Local), strings, ExploreLive.Identity())
        assertEquals("the connect sheet", SiteLock.Local, consent.lock)
        // On the sheets a local page says what its mark is, ahead of what the line already says.
        assertEquals("本地页面", siteLockWords(consent.lock, consent.statusLine, strings.t("explore.localSite")))
        assertEquals("本地页面 · ${model.connection.statusLine}", siteLockWords(model.connection.lock, model.connection.statusLine, strings.t("explore.localSite")))
        assertEquals("a padlock and an open lock say themselves", "", siteLockWords(SiteLock.Secure, "", strings.t("explore.localSite")))
        assertEquals("", siteLockWords(SiteLock.Insecure, "", strings.t("explore.localSite")))
    }

    @Test
    fun `https keeps its padlock and public http its warning`() {
        val safe = home("$https/play", DbrTabView(tab = "t1", origin = https, secure = true, lock = OriginLock.Secure))
        assertEquals(listOf(SiteLock.Secure, SiteLock.Secure, SiteLock.Secure), listOf(safe.browser.lock, safe.siteMenuSheet.lock, safe.connection.lock))
        val open = home("$public/", DbrTabView(tab = "t1", origin = public, secure = false, lock = OriginLock.Insecure))
        assertEquals(listOf(SiteLock.Insecure, SiteLock.Insecure, SiteLock.Insecure), listOf(open.browser.lock, open.siteMenuSheet.lock, open.connection.lock))
        assertEquals(SiteLock.Secure, ExploreLive.consent(fallback.connection, DbrConsentView(tab = "t1", origin = https, lock = OriginLock.Secure), strings, ExploreLive.Identity()).lock)
    }

    /**
     * The bar's mark is its OWN address's (spec 082 RE1): while a load heads
     * for another site the tab's origin — and its `lock` — are already the
     * next page's. A local page on its way to an https site keeps its local
     * mark under its own host, and the reverse keeps its padlock.
     */
    @Test
    fun `the bar's mark is the committed document's, not the page a load is heading for`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", "$local/", "", "127.0.0.1:8137")), selected_tab = "t1", ready = true)
        val leaving = EngineState(url = "$local/", origin = local, host = "127.0.0.1:8137", loading = true, shown = "$local/", pending = "$https/")
        val heading = DbrTabView(tab = "t1", origin = https, secure = true, lock = OriginLock.Secure)
        val shown = ExploreLive.home(fallback, view, BhistView(), leaving, strings, heading)
        assertEquals("127.0.0.1:8137", shown.browser.host)
        assertEquals("the host is the local page's, and so is its mark", SiteLock.Local, shown.browser.lock)

        val back = EngineState(url = "$https/", origin = https, host = "arcorbs.game3.app", loading = true, shown = "$https/", pending = "$local/")
        val other = ExploreLive.home(fallback, view, BhistView(), back, strings, DbrTabView(tab = "t1", origin = local, secure = true, lock = OriginLock.Local))
        assertEquals("arcorbs.game3.app", other.browser.host)
        assertEquals(SiteLock.Secure, other.browser.lock)
    }

    /** The bar's own rule, word for word: "closed" is asked which kind; a "local" it may come to say is taken as said. */
    @Test
    fun `the bar's closed lock is a padlock only where the core calls the address secure`() {
        fun bar(url: String, lock: String) = uniffi.vela_core_uniffi.BrowserAddressBar(url = url, host = url.substringAfter("://").substringBefore('/'), lock = lock)
        // As the core answers today.
        for ((url, expected) in listOf("$https/" to SiteLock.Secure, "$local/" to SiteLock.Local, "$lan/app" to SiteLock.Local, "$public/" to SiteLock.Insecure)) {
            val said = uniffi.vela_core_uniffi.browserAddressBar(url, null, null)
            assertEquals("$url → the bar says ${said.lock}", expected, ExploreLive.barLock(said))
        }
        assertEquals(SiteLock.Local, ExploreLive.barLock(bar("$local/", "local")))
        assertEquals(SiteLock.Insecure, ExploreLive.barLock(bar("$public/", "open")))
        assertEquals("no bar, no padlock", SiteLock.Insecure, ExploreLive.barLock(null))
    }

    /** Through the REAL machine: what the tab and the consent views say of each page, as this shell decodes them. */
    @Test
    fun `the machine's tab and consent views carry the lock`() = runBlocking<Unit> {
        val executor = BrowserExecutor(
            FakeStore(),
            object : BrowserExecutor.Ports {
                override fun deliver(tab: String, messageJson: String) = Unit
                override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer = BrowserExecutor.ReadAnswer.NoAnswer()
                override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? = null
                override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) = Unit
                override fun cancelSigning(tab: String, id: String) = Unit
                override fun saveConnectionRecord(row: JSONObject) = Unit
            },
        )
        val h = CoreHost(
            bridge = DappBrowserCore().asBridge(), scope = scope, initial = DbrView(), serializer = DbrView.serializer(),
            perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        )
        val account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        h.dispatch(DbrEvent.Start, DbrEvent.serializer())
        h.dispatch(DbrEvent.AccountsUpdated(listOf(account)), DbrEvent.serializer())
        h.dispatch(DbrEvent.AccountSwitched(account, 1.0e12), DbrEvent.serializer())
        for ((tab, origin) in listOf("t-local" to local, "t-https" to https)) {
            h.dispatch(DbrEvent.NavigationStarted(tab, "$origin/", 1.0e12), DbrEvent.serializer())
            h.dispatch(DbrEvent.PageMessage(tab, origin, true, JSONObject().put("t", "hello").put("doc", "d-$tab").toString(), 1.0e12), DbrEvent.serializer())
        }
        val view = withTimeoutOrNull(HANG_MS) { h.view.first { it.ready && it.tabs.size == 2 } } ?: throw AssertionError("the machine never showed both tabs: ${h.view.value}")
        val tabs = view.tabs.associateBy { it.tab }
        assertTrue("the wallet may be used from the local page", tabs.getValue("t-local").secure)
        assertEquals("…and it is not a padlock", OriginLock.Local, tabs.getValue("t-local").lock)
        assertEquals(OriginLock.Secure, tabs.getValue("t-https").lock)

        h.dispatch(
            DbrEvent.PageMessage("t-local", local, true, JSONObject().put("t", "req").put("doc", "d-t-local").put("id", "c1").put("method", "eth_requestAccounts").put("params", JSONArray()).toString(), 1.0e12 + 1),
            DbrEvent.serializer(),
        )
        val consent = (withTimeoutOrNull(HANG_MS) { h.view.first { it.consent != null } } ?: throw AssertionError("no consent sheet")).consent!!
        assertEquals(local, consent.origin)
        assertEquals(OriginLock.Local, consent.lock)
        assertFalse("the connect sheet draws no padlock for it", ExploreLive.consent(fallback.connection, consent, strings, ExploreLive.Identity()).lock == SiteLock.Secure)
    }

    private companion object {
        /** A hang limit, not a deadline. */
        const val HANG_MS = 120_000L
    }
}
