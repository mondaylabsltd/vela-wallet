package app.getvela.wallet

import android.util.Log
import androidx.datastore.preferences.core.edit
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.data.velaStore
import app.getvela.wallet.dev.ParallelSpaceHook
import app.getvela.wallet.feature.browser.core.BrowserController
import app.getvela.wallet.feature.browser.core.BrowserEngine
import app.getvela.wallet.feature.browser.core.BrowserExecutor
import app.getvela.wallet.feature.browser.core.DbrProviderState
import app.getvela.wallet.feature.browser.core.DbrTabView
import java.net.InetAddress
import java.net.ServerSocket
import java.util.concurrent.CopyOnWriteArrayList
import kotlin.concurrent.thread
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeoutOrNull
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Device walk 2026-10-11, item 5 — the proof that stands in for a connect on
 * a real phone with the owner's account: a walk through the parallel space
 * leaves the person's connected sites exactly as they were.
 *
 * **What this drives, all of it the app's own** (nothing faked but the two
 * web pages, which this test serves on the device's loopback):
 * - the app's process-wide `AppContainer`: its real `SessionController`
 *   (the core's `session` machine over the app's DataStore), its real
 *   `BrowserController` (the core's `dapp_browser` machine, the real
 *   `BrowserExecutor`, real `WebView`s carrying the core's provider script)
 *   and the wiring between them (`session.view` → `accountsChanged`);
 * - the debug build's real door: [ParallelSpaceHook.enter] /
 *   [ParallelSpaceHook.leave] (`DebugParallelSpace`: the flag, the fixture
 *   record appended and removed, the wallet to go back to);
 * - the app's real site store: the `vela.perm.*` / `vela.chain.*` keys of
 *   the `vela_onboarding` DataStore file, read back from the DataStore.
 *
 * **The walk.** A wallet of the person's is on the device (a record written
 * the way the space writes its own — it needs no key, nothing is signed). In
 * it, a page asks `eth_requestAccounts` through the provider, the consent is
 * approved, and the person picks a network for the site: a grant and a chain
 * are now in the store. Enter the space → the page, still open, is no longer
 * connected → connect the SAME site inside, connect a NEW one, switch the
 * first to another network → leave. The site store read before, inside and
 * after is byte for byte the same, the DataStore never showed any other
 * content in between, and the page is connected to the person's account on
 * the person's network again.
 *
 * What it does NOT drive: a tap on the consent sheet (the approval is the
 * controller call the sheet's button makes), and a real passkey (nothing is
 * signed). An emulator, never a person's phone: it writes accounts.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ParallelSpaceSitesDeviceTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ParallelSpaceSitesDeviceTest {

    private val app get() = InstrumentationRegistry.getInstrumentation().targetContext.applicationContext as VelaWalletApplication

    /** One origin on the device's loopback: every request is answered with a page that can ask to connect. */
    private class Site {
        private val server = ServerSocket(0, 16, InetAddress.getByName("127.0.0.1"))
        val origin = "http://127.0.0.1:${server.localPort}"
        val url = "$origin/"

        init {
            thread(isDaemon = true, name = "pr4-site-${server.localPort}") {
                while (!server.isClosed) {
                    runCatching {
                        server.accept().use { socket ->
                            val reader = socket.getInputStream().bufferedReader()
                            while (!reader.readLine().isNullOrEmpty()) Unit
                            val body = PAGE.toByteArray()
                            socket.getOutputStream().apply {
                                write("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: ${body.size}\r\nConnection: close\r\n\r\n".toByteArray())
                                write(body)
                                flush()
                            }
                        }
                    }
                }
            }
        }

        fun close() = runCatching { server.close() }
    }

    private val sites = mutableListOf<Site>()
    private fun site() = Site().also { sites += it }

    /** On the main thread, as the app drives its controllers; the waits suspend, never block it. */
    private fun <T> onMain(block: suspend CoroutineScope.() -> T): T = runBlocking(Dispatchers.Main) { block() }

    /**
     * Woken by what it waits for, never by a clock. [HANG_MS] is not a
     * deadline anything is measured against: it turns a step that never
     * happens into a failure that names the step.
     */
    private suspend fun <T> settled(step: String, wait: suspend () -> T): T {
        Log.i(TAG, "… $step")
        return withTimeoutOrNull(HANG_MS) { wait() } ?: throw AssertionError("never happened: $step")
    }

    /** The site store as the DataStore holds it now: every `vela.perm.*` / `vela.chain.*` key and its exact value. */
    private suspend fun siteStore(): Map<String, String> = app.velaStore.data.first().siteKeys()

    private fun androidx.datastore.preferences.core.Preferences.siteKeys(): Map<String, String> = asMap()
        .filterKeys { it.name.startsWith(BrowserExecutor.GRANT_PREFIX) || it.name.startsWith(BrowserExecutor.CHAIN_PREFIX) }
        .map { it.key.name to it.value.toString() }
        .toMap()
        .toSortedMap()

    private fun BrowserController.tab(id: String): DbrTabView? = dapp.value.tabs.firstOrNull { it.tab == id }

    /** Opens [site] in a new tab from 探索's home; the tab's id and its engine, its page loaded and offered the wallet. */
    private suspend fun BrowserController.openSite(site: Site): Pair<String, BrowserEngine> {
        landedHome()
        open(site.url)
        val id = settled("a tab shows ${site.origin}") {
            explore.first { view -> view.tabs.any { it.id == view.selected_tab && it.url.orEmpty().startsWith(site.origin) } }.selected_tab!!
        }
        val engine = settled("the tab's page is in front") { current.first { it?.id == id } }!!
        settled("the page at ${site.origin} loaded") { engine.state.first { !it.loading && it.shown.orEmpty().startsWith(site.origin) } }
        settled("the page was offered the wallet") { dapp.first { view -> view.tabs.any { it.tab == id && it.provider == DbrProviderState.Offered } } }
        return id to engine
    }

    /** The page asks to connect, through the provider; the consent that comes up is approved. The account it was for. */
    private suspend fun BrowserController.connect(id: String, engine: BrowserEngine, site: Site): String {
        selectTab(id)
        settled("the tab is in front") { current.first { it === engine } }
        engine.webView.evaluateJavascript("connect()", null)
        val consent = settled("the consent sheet for ${site.origin}") { dapp.first { it.consent?.origin == site.origin } }.consent!!
        assertEquals(id, consent.tab)
        consentApproved()
        settled("the page at ${site.origin} is connected") { dapp.first { view -> view.consent == null && view.tabs.any { it.tab == id && it.connected_address != null } } }
        return consent.address.orEmpty()
    }

    @After
    fun tearDown() {
        // Whatever happened: out of the space, no tab, no grant, no wallet left behind.
        runCatching {
            onMain {
                val container = app.container
                if (ParallelSpaceHook.active()) {
                    ParallelSpaceHook.leave()
                    withTimeoutOrNull(HANG_MS) { container.session.view.first { view -> view.accounts.none { ParallelSpaceHook.isSpaceAccount(it.address) } } }
                }
                container.browser.closeAllTabs()
                container.browser.revokeAll()
                withTimeoutOrNull(HANG_MS) { app.velaStore.data.first { prefs -> prefs.siteKeys().keys.none { it.startsWith(BrowserExecutor.GRANT_PREFIX) } } }
                // A site's network outlives its grant (a disconnect keeps it): this test's own are taken out by hand.
                app.velaStore.edit { prefs ->
                    sites.forEach { prefs.remove(androidx.datastore.preferences.core.stringPreferencesKey(BrowserExecutor.chainKey(it.origin))) }
                }
                container.session.removeFixtureAccount(PERSON_ID, null)
            }
        }.onFailure { Log.w(TAG, "clean-up did not finish", it) }
        sites.forEach { it.close() }
    }

    @Test
    fun aWalkThroughTheSpaceLeavesThePersonsSitesByteForByte() {
        val shared = site()
        val insideOnly = site()
        onMain {
            val container = app.container
            val session = container.session
            settled("the session has read the store") { session.view.first { !it.loading } }
            assertFalse("start outside the space", ParallelSpaceHook.active())
            assertTrue("no test wallet on the device yet", session.view.value.accounts.none { ParallelSpaceHook.isSpaceAccount(it.address) })

            // -- the person's wallet, and the site they connected ---------------------
            session.addAccount(JSONObject(PERSON))
            settled("the person's wallet is in front") { session.view.first { it.address.equals(PERSON_ADDRESS, ignoreCase = true) } }
            container.settings.startNetworks()
            settled("the wallet's networks are read") { container.settings.networks.first { view -> view.loaded && view.networks.any { it.chain_id.toInt() == PERSON_CHAIN } } }
            val browser = container.browser
            browser.start()
            settled("the browser has read its documents") { browser.explore.first { it.ready } }
            settled("the browser has read the site store") { browser.dapp.first { it.ready } }

            val (tab, engine) = browser.openSite(shared)
            assertEquals("the consent names the person's account", PERSON_ADDRESS.lowercase(), browser.connect(tab, engine, shared).lowercase())
            browser.pickSiteChain(PERSON_CHAIN)
            val before = settled("the grant and the site's network are in the store") {
                app.velaStore.data.first { prefs ->
                    val keys = prefs.siteKeys()
                    keys[BrowserExecutor.chainKey(shared.origin)] == PERSON_CHAIN.toString() && keys.containsKey(BrowserExecutor.grantKey(shared.origin))
                }.siteKeys()
            }
            Log.i(TAG, "site store BEFORE: $before")
            assertEquals(PERSON_ADDRESS.lowercase(), JSONObject(before.getValue(BrowserExecutor.grantKey(shared.origin))).getString("address").lowercase())
            assertEquals(PERSON_ADDRESS.lowercase(), browser.tab(tab)?.connected_address?.lowercase())

            // Every content the site store shows from here on, as the DataStore reports it.
            val seen = CopyOnWriteArrayList<Map<String, String>>()
            val recorder = launch(Dispatchers.Default) {
                app.velaStore.data.map { it.siteKeys() }.distinctUntilChanged().collect { seen += it }
            }
            settled("the recorder has the first content") { app.velaStore.data.first { seen.isNotEmpty() } }

            // -- in ---------------------------------------------------------------------
            ParallelSpaceHook.enter()
            val fixture = ParallelSpaceHook.fixtureAccount()!!.address
            settled("the test wallet is in front") { session.view.first { it.address.equals(fixture, ignoreCase = true) } }
            settled("the open page is no longer connected: the space has its own sites") {
                browser.dapp.first { view -> view.tabs.any { it.tab == tab && it.connected_address == null } }
            }
            assertTrue("Settings' list shows none of the person's sites inside", browser.dapp.value.sites.isEmpty())

            // -- inside: the same site, a new one, another network ------------------------
            assertEquals("inside, the consent names the test wallet", fixture.lowercase(), browser.connect(tab, engine, shared).lowercase())
            val (newTab, newEngine) = browser.openSite(insideOnly)
            assertEquals(fixture.lowercase(), browser.connect(newTab, newEngine, insideOnly).lowercase())
            browser.selectTab(tab)
            settled("the first site's tab is in front again") { browser.current.first { it === engine } }
            browser.pickSiteChain(SPACE_CHAIN)
            settled("the space's page is on the network picked inside") { browser.dapp.first { view -> view.tabs.any { it.tab == tab && it.chain_id == SPACE_CHAIN } } }
            assertEquals(setOf(shared.origin, insideOnly.origin), browser.dapp.value.sites.map { it.origin }.toSet())
            val inside = siteStore()
            Log.i(TAG, "site store INSIDE: $inside")
            assertEquals("inside the space the site store is the person's, untouched", before, inside)

            // -- out --------------------------------------------------------------------
            ParallelSpaceHook.leave()
            settled("the test wallet's record is gone and the person's wallet is in front") {
                session.view.first { view -> view.address.equals(PERSON_ADDRESS, ignoreCase = true) && view.accounts.none { it.address.equals(fixture, ignoreCase = true) } }
            }
            settled("the page is connected to the person's account again, on their network") {
                browser.dapp.first { view ->
                    view.tabs.any { it.tab == tab && it.connected_address.equals(PERSON_ADDRESS, ignoreCase = true) && it.chain_id == PERSON_CHAIN }
                }
            }
            assertNull("what was connected inside ended with the visit", browser.tab(newTab)?.connected_address)
            assertEquals("Settings' list is the person's again", listOf(shared.origin), browser.dapp.value.sites.map { it.origin })
            assertEquals(PERSON_ADDRESS.lowercase(), browser.dapp.value.sites.single().address.lowercase())

            // One more thing through the same machine, after the way out, so
            // everything the walk asked of the store has been done: the page
            // is asked for nothing new, the store is read last.
            browser.inspectorOpened(tab)
            settled("the machine answered after the way out") { browser.dapp.first { it.inspector?.tab == tab } }
            browser.inspectorClosed()
            // … and an empty edit of the DataStore itself: it takes its turn behind every edit already asked for.
            app.velaStore.edit { }
            val after = siteStore()
            recorder.cancel()
            Log.i(TAG, "site store AFTER: $after")
            Log.i(TAG, "site store contents seen in between: ${seen.size}")
            assertEquals("the site store, byte for byte", before, after)
            assertEquals("the DataStore never showed any other content in between: $seen", listOf(before), seen.toList())
            assertNotNull(after[BrowserExecutor.grantKey(shared.origin)])
            assertNull("the site met inside left nothing in the store", after[BrowserExecutor.grantKey(insideOnly.origin)])
        }
    }

    private companion object {
        const val TAG = "pr4-space"

        /** A hang limit, not a deadline (see [settled]). */
        const val HANG_MS = 180_000L

        const val PERSON_ID = "pr4-person-cred"
        const val PERSON_ADDRESS = "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984"

        /** Gnosis for the person's site; Base inside the space. Both are networks every Vela ships with. */
        const val PERSON_CHAIN = 100
        const val SPACE_CHAIN = 8453

        /** A wallet record as the space writes its own (`ParallelSpaceBinding`): whole, with its keys — and no key anybody holds. */
        const val PERSON = """{"id":"$PERSON_ID","name":"PR4 person","address":"$PERSON_ADDRESS",
            "public_key_hex":"04cd","created_at_iso":"2026-10-11T04:00:00.000Z",
            "keys":[{"credential_id":"$PERSON_ID","public_key_hex":"04cd","name":"PR4 person","transports":"internal"}]}"""

        const val PAGE = """<!doctype html><html><head><meta charset="utf-8"><title>PR4 site</title>
<script>
function connect() {
  return window.ethereum.request({ method: 'eth_requestAccounts' })
    .then(function (a) { document.title = 'connected ' + a[0]; }, function (e) { document.title = 'refused ' + e.code; });
}
</script></head><body><p>PR4 parallel-space walk</p></body></html>"""
    }
}
