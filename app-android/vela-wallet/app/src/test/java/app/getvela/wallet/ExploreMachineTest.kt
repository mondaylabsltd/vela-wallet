package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.browser.core.BhistEvent
import app.getvela.wallet.feature.browser.core.BhistExecutor
import app.getvela.wallet.feature.browser.core.BhistOperation
import app.getvela.wallet.feature.browser.core.BhistShellResult
import app.getvela.wallet.feature.browser.core.BhistView
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
 * store shape the other clients read — favourites deduped by origin, groups,
 * tabs with the core's selection rule, recents deduped by origin, and all of
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

    @Test
    fun `favourites, a group and tabs survive a second host over the same store`() = runBlocking {
        val h = explore()
        withTimeout(10_000) { h.view.first { it.ready } }
        h.dispatch(ExploreEvent.FavoriteAdded("https://app.uniswap.org/swap?x=1", "Uniswap", 1.0e12), ExploreEvent.serializer())
        h.dispatch(ExploreEvent.FavoriteAdded("https://app.uniswap.org/pool", null, 1.0e12 + 1), ExploreEvent.serializer())
        val fav = withTimeout(10_000) { h.view.first { it.favorites.isNotEmpty() && it.favorites.first().url.endsWith("/pool") } }
        assertEquals("deduped by origin, url refreshed, name kept", 1, fav.favorites.size)
        assertEquals("https://app.uniswap.org", fav.favorites.single().origin)
        assertEquals("Uniswap", fav.favorites.single().name)
        h.dispatch(ExploreEvent.GroupCreated("交易", 1.0e12 + 2), ExploreEvent.serializer())
        val grouped = withTimeout(10_000) { h.view.first { it.groups.isNotEmpty() } }
        val groupId = grouped.groups.single().id
        h.dispatch(ExploreEvent.GroupMemberAdded(groupId, "https://app.uniswap.org"), ExploreEvent.serializer())
        withTimeout(10_000) { h.view.first { it.groups.single().sites.size == 1 } }
        h.dispatch(ExploreEvent.TabOpened("https://app.uniswap.org/swap", "Uniswap", 1.0e12 + 3), ExploreEvent.serializer())
        val one = withTimeout(10_000) { h.view.first { it.tabs.size == 1 } }
        assertEquals("opening a tab selects it", one.tabs.single().id, one.selected_tab)
        assertEquals("app.uniswap.org", one.tabs.single().host)
        h.dispatch(ExploreEvent.TabOpened(null, null, 1.0e12 + 4), ExploreEvent.serializer())
        val two = withTimeout(10_000) { h.view.first { it.tabs.size == 2 } }
        assertNull("the start page tab has no url", two.tabs[1].url)
        assertEquals(two.tabs[1].id, two.selected_tab)
        // A second host over the same bytes: what the next launch would see.
        val again = explore()
        val restored = withTimeout(10_000) { again.view.first { it.ready } }
        assertEquals(listOf("Uniswap"), restored.favorites.map { it.name })
        assertEquals(listOf("交易"), restored.groups.map { it.name })
        assertEquals(2, restored.tabs.size)
        assertEquals(two.selected_tab, restored.selected_tab)
        withTimeout(10_000) { while (store.values[ExploreExecutor.KEY]?.contains("\"favorites\"") != true) kotlinx.coroutines.delay(20) }
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
