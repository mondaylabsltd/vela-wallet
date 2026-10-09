package app.getvela.wallet

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.browser.core.BrowserController
import app.getvela.wallet.feature.browser.core.BrowserEngine
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Spec 099 navigation on a REAL [BrowserController] — the real core, real
 * WebViews — where the JVM suite could only pin the source:
 *
 * - an open from the home never shows the dApp left there while its new tab
 *   is on its way (the old page flashed up before the new one).
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.BrowserControllerWakeTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 * Its own in-memory store: the app's browser documents are never touched.
 */
@RunWith(AndroidJUnit4::class)
class BrowserControllerWakeTest {

    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    private class MemoryStore(initial: Map<String, String> = emptyMap()) : KeyValueStore {
        val values = ConcurrentHashMap(initial)
        override suspend fun read(key: String): String? = values[key]
        override suspend fun write(key: String, value: String): Boolean {
            values[key] = value
            return true
        }
        override suspend fun remove(vararg keys: String): Boolean {
            keys.forEach { values.remove(it) }
            return true
        }
        override suspend fun allKeys(): List<String> = values.keys.toList()
    }

    private val scopes = mutableListOf<CoroutineScope>()
    private val controllers = mutableListOf<BrowserController>()

    private fun controller(store: KeyValueStore): BrowserController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate).also { scopes += it }
        return BrowserController(context = context, scope = scope, store = store).also { controllers += it }
    }

    /** On the main thread, as the app drives it; the waits suspend, never block it. */
    private fun <T> onMain(block: suspend CoroutineScope.() -> T): T =
        runBlocking(Dispatchers.Main) { withTimeout(TIMEOUT_MS) { block() } }

    private suspend fun BrowserController.ready() = explore.first { it.ready }

    private suspend fun BrowserController.frontIs(tab: String): BrowserEngine =
        current.first { it?.id == tab }!!

    @After
    fun tearDown() {
        onMain {
            // Every engine goes with its tab; then nothing runs.
            controllers.forEach { it.closeAllTabs() }
            controllers.forEach { c -> c.explore.first { v -> v.tabs.none { it.url != null } } }
        }
        scopes.forEach { it.cancel() }
    }

    @Test
    fun anOpenFromTheHomeNeverShowsTheDappLeftThere() {
        onMain {
            val browser = controller(MemoryStore())
            browser.start()
            browser.ready()
            browser.open(ONE)
            val one = browser.explore.first { it.tabs.size == 1 && it.selected_tab != null }.selected_tab!!
            val left = browser.frontIs(one)
            browser.landedHome()

            // The new tab is on its way: the dApp left for the home goes out
            // of front first — it is never what shows while the open lands.
            val opening = fronts(browser, act = { browser.open(TWO) }) { it != null && it !== left }
            val two = browser.explore.value.selected_tab!!
            assertSame(left, opening.first())
            assertTrue("nothing in front while the open lands: $opening", opening.drop(1).first() == null)
            assertTrue("never the dApp left there: $opening", opening.drop(1).none { it === left })
            assertEquals(two, opening.last()?.id)

            // A resume row for the first tab: the same — out of front, then that
            // tab, live as it was left (the same engine: no reload).
            browser.landedHome()
            val resuming = fronts(browser, act = { browser.selectTab(one) }) { it === left }
            assertTrue("nothing in front while the resume lands: $resuming", resuming.drop(1).first() == null)
            assertTrue("never the other tab's page: $resuming", resuming.drop(1).none { it?.id == two })
            assertSame(left, resuming.last())
        }
    }

    /**
     * Every engine put in front while [act] runs, until one satisfies [until]
     * — the first entry is the one in front before. Recorded as each is set
     * (an unconfined collector), so a page that stayed in front through the
     * wait shows up as itself, and one taken out shows up as `null`.
     */
    private suspend fun CoroutineScope.fronts(
        browser: BrowserController,
        act: () -> Unit,
        until: (BrowserEngine?) -> Boolean,
    ): List<BrowserEngine?> {
        val seen = mutableListOf<BrowserEngine?>()
        val recorder = launch(Dispatchers.Unconfined) {
            browser.current.collect { engine ->
                seen += engine
                // Out of front only while a page is coming: the screen then keeps the home or the switcher.
                if (seen.size > 1 && engine == null) assertTrue("nothing in front means a page is coming", browser.pageComing.value)
            }
        }
        act()
        while (seen.size < 2 || !until(seen.last())) delay(10)
        recorder.cancel()
        assertFalse("the page is in front: nothing is coming any more", browser.pageComing.value)
        return seen.toList()
    }

    private companion object {
        const val TIMEOUT_MS = 20_000L
        const val ONE_HOST = "example.com"
        const val TWO_HOST = "example.org"
        const val ONE = "https://$ONE_HOST/"
        const val TWO = "https://$TWO_HOST/"
    }
}
