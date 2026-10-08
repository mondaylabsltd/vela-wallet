package app.getvela.wallet

import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.feature.browser.core.BrowserTabs
import app.getvela.wallet.feature.browser.core.DbrConsentView
import app.getvela.wallet.feature.browser.core.DbrSigningView
import app.getvela.wallet.feature.browser.core.DbrView
import app.getvela.wallet.feature.browser.core.ExploreEntry
import app.getvela.wallet.feature.browser.core.ExploreLanding
import app.getvela.wallet.feature.browser.core.ExploreOpenKind
import app.getvela.wallet.feature.browser.core.ExploreOpenTarget
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 099 navigation on Android, through the REAL core: where 探索 lands
 * (`explore_landing`), where an opened site goes (`browser_open_target`),
 * which tab the switcher lights (`browser_lit_tab`) and which tab waits on
 * the person (`browser_waiting_tab`). The rules are the core's; these prove
 * this shell hands it the view and the words in its shapes, reads its answer
 * back, and is wired to ask it where it must.
 */
class BrowserNavigationTest {
    private val dapp = ExploreTab("t1", url = "https://app.uniswap.org/swap", title = "Uniswap", host = "app.uniswap.org")
    private val other = ExploreTab("t2", url = "https://polymarket.com/", title = "Polymarket", host = "polymarket.com")
    private val start = ExploreTab("t3", url = null, title = "", host = "")

    private fun view(selected: String?, vararg tabs: ExploreTab) =
        ExploreView(tabs = tabs.toList(), selected_tab = selected, recent_tabs = tabs.map { it.id }, ready = true)

    // -- The words ----------------------------------------------------------------

    @Test
    fun `the entry, the kind and the answers are the core's words`() {
        assertEquals("\"section\"", Wire.json.encodeToString(ExploreEntry.serializer(), ExploreEntry.Section))
        assertEquals("\"reselect\"", Wire.json.encodeToString(ExploreEntry.serializer(), ExploreEntry.Reselect))
        assertEquals("\"page_opened\"", Wire.json.encodeToString(ExploreEntry.serializer(), ExploreEntry.PageOpened))
        assertEquals("\"address\"", Wire.json.encodeToString(ExploreOpenKind.serializer(), ExploreOpenKind.Address))
        assertEquals("\"site\"", Wire.json.encodeToString(ExploreOpenKind.serializer(), ExploreOpenKind.Site))
        assertEquals(ExploreLanding.Tab("t1"), Wire.json.decodeFromString(ExploreLanding.serializer(), """{"type":"tab","id":"t1"}"""))
        assertEquals(ExploreLanding.Home, Wire.json.decodeFromString(ExploreLanding.serializer(), """{"type":"home"}"""))
        assertEquals(ExploreOpenTarget.Resume("t2"), Wire.json.decodeFromString(ExploreOpenTarget.serializer(), """{"type":"resume","id":"t2"}"""))
        assertEquals(ExploreOpenTarget.NewTab, Wire.json.decodeFromString(ExploreOpenTarget.serializer(), """{"type":"new_tab"}"""))
        // The view's resume rows cross the wire, and a view from before them still reads.
        val decoded = Wire.json.decodeFromString(ExploreView.serializer(), """{"tabs":[],"selected_tab":null,"ready":true,"resumable":[{"id":"t1","url":"https://a.example/","title":"A","host":"a.example"}]}""")
        assertEquals(listOf("t1"), decoded.resumable.map { it.id })
        assertTrue(Wire.json.decodeFromString(ExploreView.serializer(), """{"tabs":[],"ready":true}""").resumable.isEmpty())
    }

    // -- Where 探索 lands --------------------------------------------------------

    @Test
    fun `entering from another section or re-tapping lands on the home, the dApp's tab kept`() {
        val live = view("t1", dapp, start)
        assertEquals(ExploreLanding.Home, BrowserTabs.landing(live, ExploreEntry.Section, waiting = null))
        assertEquals(ExploreLanding.Home, BrowserTabs.landing(live, ExploreEntry.Reselect, waiting = null))
    }

    @Test
    fun `a page opened from outside lands on its tab, once the open is in the view`() {
        assertEquals(ExploreLanding.Tab("t1"), BrowserTabs.landing(view("t1", dapp, start), ExploreEntry.PageOpened, waiting = null))
        // Before the open's tab is selected (a start page in front), the home.
        assertEquals(ExploreLanding.Home, BrowserTabs.landing(view("t3", dapp, start), ExploreEntry.PageOpened, waiting = null))
    }

    @Test
    fun `a tab with a request waiting on the person wins over every entry`() {
        val live = view("t2", dapp, other, start)
        for (entry in ExploreEntry.entries) {
            assertEquals("$entry", ExploreLanding.Tab("t1"), BrowserTabs.landing(live, entry, waiting = "t1"))
        }
        // A waiting tab the strip does not carry is no reason to leave the home.
        assertEquals(ExploreLanding.Home, BrowserTabs.landing(live, ExploreEntry.Section, waiting = "gone"))
    }

    @Test
    fun `the waiting tab is the consent's, else the signature's, read from the core's own JSON`() {
        val consent = DbrView(consent = DbrConsentView(tab = "t1", origin = "https://app.uniswap.org"), signing = DbrSigningView("t2", "9"))
        assertEquals("t1", BrowserTabs.waitingTab(dappViewJson = null, dapp = consent))
        assertEquals("t2", BrowserTabs.waitingTab(dappViewJson = null, dapp = DbrView(signing = DbrSigningView("t2", "9"))))
        assertEquals("t4", BrowserTabs.waitingTab(dappViewJson = null, dapp = DbrView(adding_network = DbrSigningView("t4", "1"))))
        assertNull(BrowserTabs.waitingTab(dappViewJson = null, dapp = DbrView()))
        // The core's raw view wins over this shell's re-encoded copy.
        assertEquals("t9", BrowserTabs.waitingTab(dappViewJson = """{"signing":{"tab":"t9","id":"1"}}""", dapp = consent))
    }

    // -- Where an opened site goes -------------------------------------------------

    @Test
    fun `an address typed over the home never loads over a live dApp`() {
        val target = BrowserTabs.openTarget(view("t1", dapp), shown = "t1", onPage = false, url = "https://example.org/", kind = ExploreOpenKind.Address)
        assertEquals(ExploreOpenTarget.NewTab, target)
        // Even the dApp's own origin: an address names a page, and the tab is left as it was.
        assertEquals(
            ExploreOpenTarget.NewTab,
            BrowserTabs.openTarget(view("t1", dapp), shown = "t1", onPage = false, url = "https://app.uniswap.org/pool", kind = ExploreOpenKind.Address),
        )
    }

    @Test
    fun `a selected start page gets its first page, and the page on screen loads what its bar asked`() {
        assertEquals(
            ExploreOpenTarget.Load("t3"),
            BrowserTabs.openTarget(view("t3", dapp, start), shown = "t3", onPage = false, url = "https://example.org/", kind = ExploreOpenKind.Address),
        )
        assertEquals(
            ExploreOpenTarget.Load("t1"),
            BrowserTabs.openTarget(view("t1", dapp, start), shown = "t1", onPage = true, url = "https://example.org/", kind = ExploreOpenKind.Address),
        )
    }

    @Test
    fun `a picked site already open in a tab is resumed, not opened twice`() {
        assertEquals(
            ExploreOpenTarget.Resume("t1"),
            BrowserTabs.openTarget(view("t3", dapp, other, start), shown = "t3", onPage = false, url = "https://APP.uniswap.org:443/", kind = ExploreOpenKind.Site),
        )
    }

    @Test
    fun `a full strip has no room for a new tab, so the core answers the selected one`() {
        // The machine drops a `tab_opened` past its 24 tabs: "new tab" there
        // would be an open that does nothing. The core says where it goes;
        // the shell has no rule of its own for it.
        fun strip(count: Int) = ExploreView(
            tabs = (1..count).map { ExploreTab("t$it", url = "https://site$it.example/", title = "", host = "site$it.example") },
            selected_tab = "t3",
            recent_tabs = listOf("t3"),
            ready = true,
        )
        for (kind in listOf(ExploreOpenKind.Address, ExploreOpenKind.Site)) {
            assertEquals(
                ExploreOpenTarget.Load("t3"),
                BrowserTabs.openTarget(strip(24), shown = "t3", onPage = false, url = "https://late.example/", kind = kind),
            )
        }
        // A picked site already open still comes back as it was left; one short of the cap, a new tab.
        assertEquals(
            ExploreOpenTarget.Resume("t7"),
            BrowserTabs.openTarget(strip(24), shown = "t3", onPage = false, url = "https://site7.example/", kind = ExploreOpenKind.Site),
        )
        assertEquals(
            ExploreOpenTarget.NewTab,
            BrowserTabs.openTarget(strip(23), shown = "t3", onPage = false, url = "https://late.example/", kind = ExploreOpenKind.Address),
        )
    }

    @Test
    fun `the switcher lights the page's tab, and over the home only a start page`() {
        val live = view("t1", dapp, start)
        assertEquals("t1", BrowserTabs.litTab(live, shown = "t1", onPage = true))
        assertNull("a dApp left for the home is not this tab", BrowserTabs.litTab(live, shown = "t1", onPage = false))
        assertEquals("t3", BrowserTabs.litTab(view("t3", dapp, start), shown = "t3", onPage = false))
    }

    // -- The wiring ----------------------------------------------------------------

    private val main = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle"), "app-android/vela-wallet/app/src/main/java/app/getvela/wallet")

    private fun source(path: String) = File(main, path).readText()

    @Test
    fun `every open asks the core's open target, and a picked site says it is one`() {
        val controller = source("feature/browser/core/BrowserController.kt")
        val open = controller.substringAfter("fun open(text: String").substringBefore("private fun loadInto(")
        assertTrue("open asks browser_open_target", "BrowserTabs.openTarget(" in open)
        assertTrue("an open from outside is never on a page", "onPage && !fromOutside" in open)
        val host = source("navigation/VelaNavHost.kt")
        assertTrue("favourites and recents are sites", "onOpenSite = { url -> browser.open(url, kind = app.getvela.wallet.feature.browser.core.ExploreOpenKind.Site) }" in host)
        assertTrue("the bar of the page on screen loads in it", "onOpenInPage = { browser.open(it, onPage = true) }" in host)
        assertTrue("a scanned address is a page handed in", "onOpenScanned = { browser.open(it, fromOutside = true) }" in host)
    }

    @Test
    fun `entering 探索 asks the core where it lands, with the waiting tab`() {
        val host = source("navigation/VelaNavHost.kt")
        assertTrue("a re-tap while 探索 is up is Reselect", "if (section == VelaTab.Explore) app.getvela.wallet.feature.browser.core.ExploreEntry.Reselect" in host)
        assertTrue("a page from outside is PageOpened", "enterExplore(app.getvela.wallet.feature.browser.core.ExploreEntry.PageOpened)" in host)
        assertTrue("the waiting tab is read at entry", "exploreWaiting = application.container.browser.waitingTab()" in host)
        assertTrue("the landing is the core's", "BrowserTabs.landing(exploreView, exploreEntry, exploreWaiting)" in host)
        assertTrue("the old engine-decides landing is gone", "initialView" !in host)
    }

    @Test
    fun `no engine is made for a tab nobody asked to see`() {
        val controller = source("feature/browser/core/BrowserController.kt")
        val reconcile = controller.substringAfter("private fun reconcile(view: ExploreView)").substringBefore("private fun newEngine(")
        assertTrue("reconcile mints only for the wanted tab", "selected.id in engines || selected.id == wanted" in reconcile)
        val select = controller.substringAfter("private fun resume(id: String)").substringBefore("fun landedHome()")
        assertTrue("selecting a tab is asking for it", "wanted = id" in select)
    }
}
