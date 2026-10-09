package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BhistEvent
import app.getvela.wallet.feature.browser.core.BhistExecutor
import app.getvela.wallet.feature.browser.core.BhistOperation
import app.getvela.wallet.feature.browser.core.BhistShellResult
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.ExploreDoc
import app.getvela.wallet.feature.browser.core.ExploreEvent
import app.getvela.wallet.feature.browser.core.ExploreExecutor
import app.getvela.wallet.feature.browser.core.ExploreOperation
import app.getvela.wallet.feature.browser.core.ExploreShellResult
import app.getvela.wallet.feature.browser.core.ExploreView
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.BrowserHistoryCore
import uniffi.vela_core_uniffi.ExploreSitesCore

/**
 * Spec 044 T021: the browser's memory through the real machines and the
 * store shape the other clients read — favourites deduped by origin, tabs
 * with the core's selection rule, recents deduped by origin, and all of
 * it back after a "process death" (a second host over the same store).
 */
class ExploreMachineTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = FakeStore()

    @After
    fun stop() = scope.cancel()

    private fun explore(): CoreHost<ExploreView> {
        val executor = ExploreExecutor(store)
        return CoreHost(
            bridge = ExploreSitesCore().asBridge(), scope = scope, initial = ExploreView(), serializer = ExploreView.serializer(),
            perform = JsonShell.perform(ExploreOperation.serializer(), ExploreShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(ExploreOperation.serializer(), ExploreShellResult.serializer(), fallback = ExploreShellResult.Written, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("core fault", error) },
        ).also { it.dispatch(ExploreEvent.Start, ExploreEvent.serializer()) }
    }

    private fun history(loaded: CompletableDeferred<Unit>): CoreHost<BhistView> {
        val executor = BhistExecutor(store, onLoaded = { loaded.complete(Unit) })
        return CoreHost(
            bridge = BrowserHistoryCore().asBridge(), scope = scope, initial = BhistView(), serializer = BhistView.serializer(),
            perform = JsonShell.perform(BhistOperation.serializer(), BhistShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(BhistOperation.serializer(), BhistShellResult.serializer(), fallback = BhistShellResult.Written, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("core fault", error) },
        ).also { it.dispatch(BhistEvent.Start, BhistEvent.serializer()) }
    }

    /**
     * Issue #330, after a restart: both system groups hidden come back hidden
     * from the store, and the start page still draws the way to Manage groups.
     */
    @Test
    fun `every group hidden survives a restart, and Manage groups stays reachable`() = runBlocking {
        val h = explore()
        withTimeout(10_000) { h.view.first { it.ready } }
        h.dispatch(ExploreEvent.FavoriteAdded("https://app.uniswap.org/", "Uniswap", 1.0e12), ExploreEvent.serializer())
        h.dispatch(ExploreEvent.SystemGroupHiddenSet(app.getvela.wallet.feature.browser.core.ExploreSystemGroup.Favorites, true), ExploreEvent.serializer())
        h.dispatch(ExploreEvent.SystemGroupHiddenSet(app.getvela.wallet.feature.browser.core.ExploreSystemGroup.Recent, true), ExploreEvent.serializer())
        withTimeout(10_000) { h.view.first { it.favorites_hidden && it.recent_hidden } }
        // The bytes trail the view (see the test below): wait for the write.
        withTimeout(10_000) {
            while (
                store.values[ExploreExecutor.KEY]
                    ?.let { runCatching { Wire.json.decodeFromString(ExploreDoc.serializer(), it) }.getOrNull() }
                    ?.hidden_system?.size != 2
            ) kotlinx.coroutines.delay(20)
        }

        // "Process death": a second host over the same store.
        val again = withTimeout(10_000) { explore().view.first { it.ready } }
        assertTrue(again.favorites_hidden && again.recent_hidden)
        val strings = run {
            val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
            app.getvela.wallet.core.i18n.I18nRuntime { tag -> java.io.File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
        }
        val fallback = app.getvela.wallet.feature.explore.ExploreFixtures.buildState(app.getvela.wallet.feature.explore.ExploreScreenState.E2, strings)
        val page = app.getvela.wallet.feature.browser.ExploreLive.home(fallback, again, BhistView(), null, strings)
        assertEquals(strings.t("explore.edit"), page.favorites?.action)
        assertTrue(page.favorites!!.tiles.isEmpty())
    }

    /**
     * Issue #329: app.uniswap.org, starred while it had failed to load, was
     * pinned as "网页无法打开" — the WebView's error page's title. The star now
     * names a favourite by the core's rule (`browserPinnedTitle` over the last
     * visit `browserLoadVisit` made): the site's last good title, else its host.
     */
    @Test
    fun `a page that failed to load is pinned by its host, never the error page's title`() = runBlocking {
        val h = explore()
        withTimeout(10_000) { h.view.first { it.ready } }
        val failedUrl = "https://app.uniswap.org/"
        // The error page is no visit — failed, and read at its chrome-error:// address.
        val errorPage = uniffi.vela_core_uniffi.browserLoadVisit("chrome-error://chromewebdata/", "网页无法打开", null, false, null)
        assertNull(errorPage)
        assertNull(uniffi.vela_core_uniffi.browserLoadVisit(failedUrl, "网页无法打开", null, true, null))
        // The page before was another site: its title is not this one's either.
        val before = uniffi.vela_core_uniffi.browserLoadVisit("https://bscscan.com/", "BscScan", null, false, null)
        assertNull(uniffi.vela_core_uniffi.browserPinnedTitle(failedUrl, before))
        val title = uniffi.vela_core_uniffi.browserPinnedTitle(failedUrl, errorPage)
        h.dispatch(ExploreEvent.FavoriteAdded(failedUrl, title, 1.0e12), ExploreEvent.serializer())
        val pinned = withTimeout(10_000) { h.view.first { it.favorites.isNotEmpty() } }
        assertEquals("app.uniswap.org", pinned.favorites.single().name)

        // Once the site has loaded, its own title names it — even under a later failure.
        val good = uniffi.vela_core_uniffi.browserLoadVisit("https://app.uniswap.org/swap", "Uniswap Interface", null, false, null)
        assertEquals("Uniswap Interface", uniffi.vela_core_uniffi.browserPinnedTitle(failedUrl, good))
    }

    /**
     * Issue #425: the Xiaomi's Uniswap favourite, pinned under v0.9.5 while the
     * site had failed, still read "网页无法打开" in v0.9.6 — #329's fix named
     * NEW favourites and read the stored ones as they were. Through the real
     * machine and this store's bytes: a document from before the rule has the
     * name nobody chose replaced by the host and written back; the site's next
     * good load (the core's visit) names it; and a second host over the same
     * bytes keeps that name — the rule's marker survives this client's
     * serializer, or every launch would reset the name again.
     */
    @Test
    fun `a favourite stored under an error page's title is named by its host, then by the site's next good load`() = runBlocking {
        store.values[ExploreExecutor.KEY] = """
            {"favorites":[{"origin":"https://app.uniswap.org","url":"https://app.uniswap.org/","host":"app.uniswap.org","name":"网页无法打开","renamed":false,"added_ms":1.759051383E12}],
             "groups":[],"tabs":[],"selected_tab":null,"hidden_system":[]}
        """.trimIndent()
        val h = explore()
        val repaired = withTimeout(10_000) { h.view.first { it.ready } }
        assertEquals("app.uniswap.org", repaired.favorites.single().name)
        withTimeout(10_000) {
            while (stored()?.name_rule != 1) kotlinx.coroutines.delay(20)
        }
        assertEquals("app.uniswap.org", stored()!!.favorites.single().name)

        // The error page is no visit, so it can name nothing.
        assertNull(uniffi.vela_core_uniffi.browserLoadVisit("https://app.uniswap.org/", "网页无法打开", null, true, null))
        val visit = uniffi.vela_core_uniffi.browserLoadVisit(
            "https://app.uniswap.org/swap", "Uniswap | Trade Crypto on DeFi's Leading Exchange", null, false, null,
        )!!
        h.dispatch(ExploreEvent.PageLoaded(url = visit.url, title = visit.title), ExploreEvent.serializer())
        val titled = withTimeout(10_000) { h.view.first { it.favorites.single().name != "app.uniswap.org" } }
        assertEquals("Uniswap | Trade Crypto on DeFi's Leading Exchange", titled.favorites.single().name)
        withTimeout(10_000) {
            while (stored()?.favorites?.single()?.name != "Uniswap | Trade Crypto on DeFi's Leading Exchange") kotlinx.coroutines.delay(20)
        }

        // "Process death": the next launch reads the name, not the host.
        val again = withTimeout(10_000) { explore().view.first { it.ready } }
        assertEquals("Uniswap | Trade Crypto on DeFi's Leading Exchange", again.favorites.single().name)
    }

    private fun stored(): ExploreDoc? =
        store.values[ExploreExecutor.KEY]?.let { runCatching { Wire.json.decodeFromString(ExploreDoc.serializer(), it) }.getOrNull() }

    @Test
    fun `favourites and tabs survive a second host over the same store`() = runBlocking {
        val h = explore()
        withTimeout(10_000) { h.view.first { it.ready } }
        h.dispatch(ExploreEvent.FavoriteAdded("https://app.uniswap.org/swap?x=1", "Uniswap", 1.0e12), ExploreEvent.serializer())
        h.dispatch(ExploreEvent.FavoriteAdded("https://app.uniswap.org/pool", null, 1.0e12 + 1), ExploreEvent.serializer())
        val fav = withTimeout(10_000) { h.view.first { it.favorites.isNotEmpty() && it.favorites.first().url.endsWith("/pool") } }
        assertEquals("deduped by origin, url refreshed, name kept", 1, fav.favorites.size)
        assertEquals("https://app.uniswap.org", fav.favorites.single().origin)
        assertEquals("Uniswap", fav.favorites.single().name)
        h.dispatch(ExploreEvent.TabOpened("https://app.uniswap.org/swap", "Uniswap", 1.0e12 + 3), ExploreEvent.serializer())
        val one = withTimeout(10_000) { h.view.first { it.tabs.size == 1 } }
        assertEquals("opening a tab selects it", one.tabs.single().id, one.selected_tab)
        assertEquals("app.uniswap.org", one.tabs.single().host)
        h.dispatch(ExploreEvent.TabOpened(null, null, 1.0e12 + 4), ExploreEvent.serializer())
        val two = withTimeout(10_000) { h.view.first { it.tabs.size == 2 } }
        assertNull("the start page tab has no url", two.tabs[1].url)
        assertEquals(two.tabs[1].id, two.selected_tab)
        // A second host over the same bytes: what the next launch would see.
        //
        // The bytes trail the view — the core updates its model and THEN asks
        // the shell to persist, the same trap the recents test below names.
        // A host restores from the STORE, so opening the second one off a view
        // that has raced ahead of the write reads a doc that is one dispatch
        // short: green on a fast machine, red on a loaded runner. Wait for the
        // write the restore is about (the second tab, whose doc carries the
        // favourite with it) before reading it back.
        withTimeout(10_000) {
            while (
                store.values[ExploreExecutor.KEY]
                    ?.let { runCatching { Wire.json.decodeFromString(ExploreDoc.serializer(), it) }.getOrNull() }
                    ?.tabs?.size != 2
            ) kotlinx.coroutines.delay(20)
        }
        val again = explore()
        val restored = withTimeout(10_000) { again.view.first { it.ready } }
        assertEquals(listOf("Uniswap"), restored.favorites.map { it.name })
        assertEquals(2, restored.tabs.size)
        assertEquals(two.selected_tab, restored.selected_tab)
    }

    /**
     * Issue #465: custom groups are gone. A document written before — a group
     * holding a favourite, a hidden empty one, Recent dApps hidden — goes
     * through THIS store's decoder (the core never sees a key it drops), keeps
     * every favourite and the hidden section, and the next write has no
     * `groups` at all. A decode failure here would read as "no document" and
     * the next write would erase the person's favourites.
     */
    @Test
    fun `a document from before custom groups went keeps every favourite and loses its groups on the next write`() = runBlocking {
        store.values[ExploreExecutor.KEY] = """
            {"favorites":[
               {"origin":"https://curve.fi","url":"https://curve.fi/","host":"curve.fi","name":"Curve","renamed":false,"added_ms":1.0E12},
               {"origin":"https://app.uniswap.org","url":"https://app.uniswap.org/","host":"app.uniswap.org","name":"Uniswap","renamed":true,"added_ms":1.0E12}],
             "groups":[
               {"id":"g1","name":"交易","members":["https://curve.fi"],"hidden":false,"created_ms":1.0E12},
               {"id":"g2","name":"New group","members":[],"hidden":true,"created_ms":1.0E12}],
             "tabs":[],"selected_tab":null,"hidden_system":["recent"],"name_rule":1}
        """.trimIndent()
        val h = explore()
        val loaded = withTimeout(10_000) { h.view.first { it.ready } }
        assertEquals(listOf("https://curve.fi", "https://app.uniswap.org"), loaded.favorites.map { it.origin })
        assertEquals(listOf("Curve", "Uniswap"), loaded.favorites.map { it.name })
        assertTrue("the hidden section stays hidden", loaded.recent_hidden)

        h.dispatch(ExploreEvent.TabOpened("https://curve.fi/", "Curve", 1.0e12 + 1), ExploreEvent.serializer())
        withTimeout(10_000) {
            while (stored()?.tabs?.size != 1) kotlinx.coroutines.delay(20)
        }
        val raw = store.values[ExploreExecutor.KEY]!!
        assertTrue("the next write drops the groups: $raw", "\"groups\"" !in raw)
        assertEquals(listOf("https://curve.fi", "https://app.uniswap.org"), stored()!!.favorites.map { it.origin })
    }

    @Test
    fun `recents are one row per origin, latest url, and are cleared and persisted`() = runBlocking {
        val loaded = CompletableDeferred<Unit>()
        val h = history(loaded)
        withTimeout(10_000) { loaded.await() }
        kotlinx.coroutines.delay(50)
        h.dispatch(BhistEvent.VisitRecorded("https://app.uniswap.org/swap", "Uniswap", null, 1.0e12), BhistEvent.serializer())
        h.dispatch(BhistEvent.VisitRecorded("https://app.uniswap.org/pool", "Uniswap · Pool", null, 1.0e12 + 5), BhistEvent.serializer())
        h.dispatch(BhistEvent.VisitRecorded("http://127.0.0.1:8137/", "   ", null, 1.0e12 + 6), BhistEvent.serializer())
        val view = withTimeout(10_000) { h.view.first { it.entries.size == 2 && it.entries.any { e -> e.url.endsWith("/pool") } } }
        val uni = view.entries.first { it.origin == "https://app.uniswap.org" }
        assertEquals("Uniswap · Pool", uni.title)
        assertEquals("127.0.0.1:8137", view.entries.first { it.origin.startsWith("http://127") }.host)
        withTimeout(10_000) { while (store.values[BhistExecutor.KEY]?.contains("app.uniswap.org") != true) kotlinx.coroutines.delay(20) }
        h.dispatch(BhistEvent.ClearAll, BhistEvent.serializer())
        withTimeout(10_000) { h.view.first { it.entries.isEmpty() } }
        // The view empties before the write lands (the core updates its model,
        // then asks the shell to persist): wait for the bytes, not the view.
        withTimeout(10_000) { while (store.values[BhistExecutor.KEY] != "[]") kotlinx.coroutines.delay(20) }
        assertEquals("[]", store.values[BhistExecutor.KEY])
    }
}
