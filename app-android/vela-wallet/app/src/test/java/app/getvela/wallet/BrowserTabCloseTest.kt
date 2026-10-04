package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.browser.core.BrowserTabs
import app.getvela.wallet.feature.browser.core.ExploreDoc
import app.getvela.wallet.feature.browser.core.ExploreEvent
import app.getvela.wallet.feature.browser.core.ExploreExecutor
import app.getvela.wallet.feature.browser.core.ExploreOperation
import app.getvela.wallet.feature.browser.core.ExploreShellResult
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.browser.core.TabCloseScope
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.TabCloses
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 099 on Android, through the REAL core: Chrome's batch closes — close
 * other tabs, close tabs to the right, close all tabs. Which tabs a scope takes
 * is the core's (`explore_tabs_closed_by`), and they go in one `tabs_closed`
 * event whose selection rule is the core's too. These prove this shell hands
 * the core the strip and the scope in its shapes, and that one event is one
 * write with the selection the core promises.
 */
class BrowserTabCloseTest {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    private val store = CountingStore(FakeStore())

    @After
    fun stop() = scope.cancel()

    /** Counts the explore doc's writes: a batch close is ONE (the loop it replaced was one per tab). */
    private class CountingStore(private val inner: FakeStore) : KeyValueStore by inner {
        @Volatile var exploreWrites = 0
        val values get() = inner.values

        override suspend fun write(key: String, value: String): Boolean {
            if (key == ExploreExecutor.KEY) exploreWrites++
            return inner.write(key, value)
        }
    }

    private fun explore(): CoreHost<ExploreView> {
        val executor = ExploreExecutor(store)
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.ExploreSitesCore().asBridge(), scope = scope, initial = ExploreView(), serializer = ExploreView.serializer(),
            perform = JsonShell.perform(ExploreOperation.serializer(), ExploreShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(ExploreOperation.serializer(), ExploreShellResult.serializer(), fallback = ExploreShellResult.Written, answer = executor::neutralAnswer),
            onFault = { error -> throw AssertionError("core fault", error) },
        ).also { it.dispatch(ExploreEvent.Start, ExploreEvent.serializer()) }
    }

    private fun storedTabs(): Int? = store.values[ExploreExecutor.KEY]
        ?.let { runCatching { Wire.json.decodeFromString(ExploreDoc.serializer(), it) }.getOrNull() }
        ?.tabs?.size

    /** Five site tabs a…e, the last opened (e) selected, and their write landed. */
    private suspend fun fiveTabs(h: CoreHost<ExploreView>): List<String> {
        withTimeout(10_000) { h.view.first { it.ready } }
        listOf("a", "b", "c", "d", "e").forEachIndexed { i, host ->
            h.dispatch(ExploreEvent.TabOpened("https://$host.example/", host, 1.0e12 + i), ExploreEvent.serializer())
        }
        val view = withTimeout(10_000) { h.view.first { it.tabs.size == 5 } }
        withTimeout(10_000) { while (storedTabs() != 5) delay(20) }
        assertEquals(view.tabs.last().id, view.selected_tab)
        return view.tabs.map { it.id }
    }

    private fun closeBy(h: CoreHost<ExploreView>, scope: TabCloseScope) {
        val ids = BrowserTabs.closedBy(h.view.value.tabs, scope)
        h.dispatch(ExploreEvent.TabsClosed(ids), ExploreEvent.serializer())
    }

    // -- The wire ----------------------------------------------------------------

    @Test
    fun `the scope and the event are in the core's shapes`() {
        assertEquals("""{"type":"others","keep":"t-1"}""", Wire.json.encodeToString(TabCloseScope.serializer(), TabCloseScope.Others("t-1")))
        assertEquals("""{"type":"right","of":"t-1"}""", Wire.json.encodeToString(TabCloseScope.serializer(), TabCloseScope.Right("t-1")))
        assertEquals("""{"type":"all"}""", Wire.json.encodeToString(TabCloseScope.serializer(), TabCloseScope.All))
        assertEquals(
            """{"type":"tabs_closed","ids":["a","b"]}""",
            Wire.json.encodeToString(ExploreEvent.serializer(), ExploreEvent.TabsClosed(listOf("a", "b"))),
        )
    }

    // -- Which tabs a scope takes: the core's answer -------------------------------

    @Test
    fun `each scope names its tabs, and a tab the strip does not carry closes nothing`() {
        // A start-page tab (no url) among them: the strip goes over as the view has it.
        val strip = listOf(
            ExploreTab("t1", url = "https://a.example/", title = "A", host = "a.example"),
            ExploreTab("t2", url = null, title = "", host = ""),
            ExploreTab("t3", url = "https://c.example/", title = "C", host = "c.example"),
            ExploreTab("t4", url = "https://d.example/", title = "D", host = "d.example"),
        )
        assertEquals(listOf("t1", "t3", "t4"), BrowserTabs.closedBy(strip, TabCloseScope.Others("t2")))
        assertEquals(listOf("t3", "t4"), BrowserTabs.closedBy(strip, TabCloseScope.Right("t2")))
        assertEquals(emptyList<String>(), BrowserTabs.closedBy(strip, TabCloseScope.Right("t4")))
        assertEquals(listOf("t1", "t2", "t3", "t4"), BrowserTabs.closedBy(strip, TabCloseScope.All))
        assertEquals(emptyList<String>(), BrowserTabs.closedBy(strip, TabCloseScope.Others("gone")))
        assertEquals(emptyList<String>(), BrowserTabs.closedBy(strip, TabCloseScope.Right("gone")))
        // One tab: nothing is "other".
        assertEquals(emptyList<String>(), BrowserTabs.closedBy(strip.take(1), TabCloseScope.Others("t1")))
    }

    // -- One event, the core's selection ------------------------------------------

    @Test
    fun `close other tabs on a tab that is not selected leaves that tab, selected`() = runBlocking {
        val h = explore()
        val ids = fiveTabs(h)
        closeBy(h, TabCloseScope.Others(ids[1]))
        val view = withTimeout(10_000) { h.view.first { it.tabs.size == 1 } }
        assertEquals(listOf(ids[1]), view.tabs.map { it.id })
        assertEquals(ids[1], view.selected_tab)
        assertEquals(listOf(ids[1]), view.recent_tabs)
    }

    @Test
    fun `close tabs to the right of a tab left of the selection moves the selection to that tab`() = runBlocking {
        val h = explore()
        val ids = fiveTabs(h)
        closeBy(h, TabCloseScope.Right(ids[1]))
        val view = withTimeout(10_000) { h.view.first { it.tabs.size == 2 } }
        assertEquals(ids.take(2), view.tabs.map { it.id })
        assertEquals("the nearest survivor, to the closed selection's left", ids[1], view.selected_tab)
    }

    @Test
    fun `a selection that survives stays where it is`() = runBlocking {
        val h = explore()
        val ids = fiveTabs(h)
        h.dispatch(ExploreEvent.TabSelected(ids[0]), ExploreEvent.serializer())
        withTimeout(10_000) { h.view.first { it.selected_tab == ids[0] } }
        closeBy(h, TabCloseScope.Right(ids[2]))
        val view = withTimeout(10_000) { h.view.first { it.tabs.size == 3 } }
        assertEquals(ids.take(3), view.tabs.map { it.id })
        assertEquals(ids[0], view.selected_tab)
    }

    @Test
    fun `close all tabs is one write and leaves the start page, nothing selected`() = runBlocking {
        val h = explore()
        fiveTabs(h)
        val before = store.exploreWrites
        closeBy(h, TabCloseScope.All)
        val view = withTimeout(10_000) { h.view.first { it.tabs.isEmpty() } }
        assertNull(view.selected_tab)
        assertTrue(view.recent_tabs.isEmpty())
        withTimeout(10_000) { while (storedTabs() != 0) delay(20) }
        delay(200)
        assertEquals("one persist for five tabs", 1, store.exploreWrites - before)
    }

    @Test
    fun `nothing named is no write`() = runBlocking {
        val h = explore()
        val ids = fiveTabs(h)
        val before = store.exploreWrites
        closeBy(h, TabCloseScope.Right(ids.last()))
        delay(200)
        assertEquals(5, h.view.value.tabs.size)
        assertEquals(0, store.exploreWrites - before)
    }

    // -- The menu --------------------------------------------------------------------

    @Test
    fun `a tab's menu is close, then the three batch closes, the empty ones greyed`() {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val strings = I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
        val copy = ExploreFixtures.buildState(ExploreScreenState.E5, strings).tabsScreen
        val menu = ExploreFixtures.tabMenu(copy, TabCloses(others = true, right = false))
        assertEquals(
            listOf(strings.t("explore.closeTab"), strings.t("explore.closeOtherTabs"), strings.t("explore.closeTabsToRight"), strings.t("explore.closeAllTabs")),
            menu.map { it.label },
        )
        assertEquals(listOf("Close tab", "Close other tabs", "Close tabs to the right", "Close all tabs"), menu.map { it.label })
        assertEquals(listOf(true, true, false, true), menu.map { it.enabled })
        assertEquals(
            listOf(ExploreFixtures.TAB_MENU_CLOSE, ExploreFixtures.TAB_MENU_OTHERS, ExploreFixtures.TAB_MENU_RIGHT, ExploreFixtures.TAB_MENU_ALL),
            menu.map { it.id },
        )
    }
}
