package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BhistEntry
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.DbrTabView
import app.getvela.wallet.feature.browser.core.EngineState
import app.getvela.wallet.feature.browser.core.ExploreSite
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.GroupAction
import app.getvela.wallet.feature.explore.TileModel
import app.getvela.wallet.feature.wallet.core.RpcPoolView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/** Spec 044 T021: the explore screen is the core's views, in the drawn model's slots. */
class ExploreLiveTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val fallback = ExploreFixtures.buildState(ExploreScreenState.E2, strings)
    private val uniswap = ExploreSite(origin = "https://app.uniswap.org", url = "https://app.uniswap.org/swap", host = "app.uniswap.org", name = "Uniswap", added_ms = 1.0)

    @Test
    fun `favourites, recents and tabs are the core's, and a site's id opens it`() {
        val view = ExploreView(
            favorites = listOf(uniswap),
            tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/swap", "Uniswap", "app.uniswap.org"), ExploreTab("t2", null, "", "")),
            selected_tab = "t1",
            ready = true,
        )
        val history = BhistView(listOf(BhistEntry("https://curve.fi", "https://curve.fi/dex", "curve.fi", "", "", 3.0)))
        val engine = EngineState(url = "https://app.uniswap.org/swap", origin = "https://app.uniswap.org", host = "app.uniswap.org", title = "Uniswap", canBack = true, shown = "https://app.uniswap.org/swap")
        // The lock is the CORE's word (spec 070), not the engine's guess.
        val tab = DbrTabView(tab = "t1", origin = "https://app.uniswap.org", secure = true)
        val model = ExploreLive.home(fallback, view, history, engine, strings, tab)
        assertNull(model.empty)
        val tiles = model.favorites!!.tiles
        assertEquals(2, tiles.size)
        assertEquals("https://app.uniswap.org/swap", (tiles[0] as TileModel.Site).site.id)
        assertTrue(tiles[1] is TileModel.Add)
        assertEquals("issue #465: Recent dApps is the only group under Favorites", listOf("recent"), model.groups.map { it.id })
        assertEquals(GroupAction.Clear, model.groups[0].action)
        assertEquals("an untitled recent falls back to its host", "curve.fi", model.groups[0].sites.single().name)
        assertEquals("https://curve.fi/dex", model.groups[0].sites.single().id)
        assertEquals(2, model.tabs.size)
        assertTrue(model.tabs[0].selected)
        assertTrue(model.tabs[1].startPage)
        assertEquals(strings.t("explore.startPage"), model.tabs[1].title)
        assertEquals("the bar's box counts every tab, start pages included", 2, model.browser.tabCount)
        // The view carries no `resumable` here, so there is no resume section:
        // the rows are the core's, never derived from the strip by this shell.
        assertNull(model.resume)
        assertTrue("the page is a favourite", model.browser.bookmarked)
        assertTrue(model.browser.canBack)
        assertEquals("app.uniswap.org", model.browser.host)
        // Issue #465: Manage groups is the two sections, each with its eye — no
        // "System" tag on Recent dApps, since there is nothing else to tell it from.
        assertEquals(listOf("favorites", "recent"), model.groupManageSheet.rows.map { it.id })
        // A plural (explore.siteCount_one/_other, chosen by the core's CLDR
        // rule): one favourite is "1 site", never "1 sites".
        assertEquals("1 site", model.groupManageSheet.rows[0].meta)
        assertNull(model.groupManageSheet.rows[1].meta)
        assertTrue(model.groupManageSheet.rows.none { it.hidden })
        assertEquals("app.uniswap.org", model.siteMenuSheet.site.host)
        assertTrue(model.browser.secure)
        assertTrue(model.siteMenuSheet.secure)
        assertEquals("a favourite's row unpins it", strings.t("explore.removeFromFavorites"), model.siteMenuSheet.items.first { it.id == "favorite" }.label)
        assertTrue("Disconnect is offered only to a connected site", model.siteMenuSheet.items.none { it.id == "disconnect" })
    }

    @Test
    fun `an insecure page is never drawn with a lock, and a crashed tab says so`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", "http://evil.example/", "", "evil.example")), selected_tab = "t1", ready = true)
        val engine = EngineState(url = "http://evil.example/", origin = "http://evil.example", host = "evil.example", shown = "http://evil.example/")
        val tab = DbrTabView(tab = "t1", origin = "http://evil.example", secure = false, crashed = true)
        val model = ExploreLive.home(fallback, view, BhistView(), engine, strings, tab)
        assertFalse(model.browser.secure)
        assertTrue(model.browser.crashed)
        assertFalse(model.connection.secure)
        // Spec 079 (owner): the open lock alone says it — no words either way.
        assertEquals("", model.siteMenuSheet.statusLine)
        assertFalse(model.siteMenuSheet.secure)
    }

    /** Spec 079: a failed page says why in the core's words, and says it is retrying. */
    @Test
    fun `a failed page carries its reason and its retry`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/", "", "app.uniswap.org")), selected_tab = "t1", ready = true)
        val failure = uniffi.vela_core_uniffi.BrowserLoadFailure(`class` = "offline", reasonKey = "explore.loadOffline", autoRetry = true)
        val engine = EngineState(url = "https://app.uniswap.org/", origin = "https://app.uniswap.org", host = "app.uniswap.org", failed = true, failure = failure, retrying = true, failedUrl = "https://app.uniswap.org/")
        val model = ExploreLive.home(fallback, view, BhistView(), engine, strings)
        assertTrue(model.browser.failed)
        assertEquals(strings.t("explore.loadOffline"), model.browser.failureReason)
        assertTrue(model.browser.retrying)
        // Spec 082 RE1: the panel names the failed host, with no lock.
        assertEquals("app.uniswap.org", model.browser.host)
        assertFalse(model.browser.lockShown)
    }

    /**
     * Issue #329: the star lights for what it pins — the address the bar
     * names. A first load that failed before anything committed has no engine
     * origin; the star pinned nothing then, and could not show a pin.
     */
    @Test
    fun `the star follows the failed address the bar names`() {
        val view = ExploreView(
            favorites = listOf(uniswap),
            tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/", "", "app.uniswap.org")),
            selected_tab = "t1",
            ready = true,
        )
        val failure = uniffi.vela_core_uniffi.BrowserLoadFailure(`class` = "offline", reasonKey = "explore.loadOffline", autoRetry = true)
        val engine = EngineState(failed = true, failure = failure, failedUrl = "https://app.uniswap.org/")
        val model = ExploreLive.home(fallback, view, BhistView(), engine, strings)
        assertEquals("app.uniswap.org", model.browser.host)
        assertTrue("the failed site is a favourite, and the star says so", model.browser.bookmarked)
    }

    /**
     * The wiring the rule needs (issue #329): the star pins what the bar names,
     * by the core's rule over the tab's last visit — never the engine's title,
     * which was the WebView's error page's ("网页无法打开") — and the visit is
     * remembered only from a load the core's visit rule accepted.
     */
    @Test
    fun `the star pins the bar's address under the core's name rule`() {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        val source = File(root, "app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/browser/core/BrowserController.kt").readText()
        val toggle = source.substringAfter("fun toggleFavorite()").substringBefore("fun removeFavorite(")
        assertTrue(toggle.contains("addressBar().url"))
        assertTrue(toggle.contains("browserPinnedTitle(url, lastVisits[engine.id])"))
        assertFalse("never the engine's own title", toggle.contains("state.title"))
        val visit = source.substringAfter("browserLoadVisit(").substringBefore("scope.launch")
        assertTrue("the last visit is the core's visit", visit.contains("lastVisits[tab] = visit"))
        // Issue #425: the same visit — and only it — tells the favourites the
        // site loaded, so a tile its host stands in for takes the site's title.
        assertTrue(visit.contains("ExploreEvent.PageLoaded(url = visit.url, title = visit.title)"))
        assertEquals("one place says a page loaded", 1, Regex("ExploreEvent\\.PageLoaded\\(").findAll(source).count())
    }

    /**
     * Spec 082 RE1 (G28): what the bar names is the core's rule. A fresh tab
     * names nothing — never the drawn fixture's host with an open lock; a
     * load pending in an empty tab names its host with no lock; a load under
     * way never renames a tab that shows a document (the page-initiated
     * spoof).
     */
    @Test
    fun `the bar names the committed document, never a fixture or a pending load over a page`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", null, "", "")), selected_tab = "t1", ready = true)
        val fresh = ExploreLive.home(fallback, view, BhistView(), null, strings)
        assertEquals("", fresh.browser.host)
        assertEquals("", fresh.browser.url)
        assertFalse(fresh.browser.lockShown)
        assertFalse(fresh.browser.secure)

        val opening = ExploreLive.home(fallback, view, BhistView(), EngineState(loading = true, pending = "https://app.uniswap.org/swap"), strings)
        assertEquals("app.uniswap.org", opening.browser.host)
        assertFalse("no lock for a page that is not there yet", opening.browser.lockShown)

        val over = EngineState(url = "https://curve.fi/", loading = true, shown = "https://curve.fi/", pending = "https://evil.example/")
        val shown = ExploreLive.home(fallback, view, BhistView(), over, strings)
        assertEquals("curve.fi", shown.browser.host)
        assertEquals("https://curve.fi/", shown.browser.url)
        assertTrue(shown.browser.lockShown)
        assertTrue(shown.browser.secure)
    }

    /** Spec 082 RE2: the watchdog's one decision, in the engine's units (progress 0–100). */
    @Test
    fun `a load that gets nowhere in the core's time is given up, a slow one that answers is not`() {
        val budget = app.getvela.wallet.feature.browser.core.LoadWatch.giveUpMs()
        assertEquals(20_000L, budget)
        assertTrue(app.getvela.wallet.feature.browser.core.LoadWatch.givesUp(budget, committed = false, progressPercent = 10))
        assertFalse("getting somewhere", app.getvela.wallet.feature.browser.core.LoadWatch.givesUp(budget, committed = false, progressPercent = 60))
        assertFalse("committed", app.getvela.wallet.feature.browser.core.LoadWatch.givesUp(budget, committed = true, progressPercent = 10))
        assertFalse("not yet", app.getvela.wallet.feature.browser.core.LoadWatch.givesUp(budget - 1, committed = false, progressPercent = 10))
        val stalled = uniffi.vela_core_uniffi.browserLoadStalled()
        assertEquals("timeout", stalled.`class`)
        assertTrue(stalled.autoRetry)
        assertTrue(uniffi.vela_core_uniffi.browserLoadRetryWhenNetworkReturns(stalled.`class`))
        // RD9: a proxy that cannot be used says so, in the corpus's words.
        val proxy = uniffi.vela_core_uniffi.browserLoadClassify("android", -5L, null, false)!!
        assertEquals("proxy", proxy.`class`)
        assertEquals(strings.t("explore.loadProxy"), strings.t(proxy.reasonKey))
    }

    /** Spec 082 RE6/RE7: the consent names the site once, in its header; a recent says a host-named site once. */
    @Test
    fun `the consent asks in its header, and a recent named by its host says it once`() {
        val consent = ExploreLive.consent(
            fallback.connection,
            app.getvela.wallet.feature.browser.core.DbrConsentView(tab = "t1", origin = "https://app.uniswap.org"),
            strings,
            ExploreLive.Identity(),
            secure = true,
        )
        assertEquals(strings.t("connect.browser.title", mapOf("host" to "app.uniswap.org")), consent.title)
        assertEquals(strings.t("connect.browser.body"), consent.explainer)
        assertEquals("no footnote: the header's close is the cancel", "", consent.footnote)

        val titled = ExploreLive.siteOf(BhistEntry("https://curve.fi", "https://curve.fi/dex", "curve.fi", "Curve", "", 3.0))
        assertEquals("Curve", titled.name)
        assertEquals("curve.fi", titled.subtitle)
        for (title in listOf("", "Curve.FI")) {
            val once = ExploreLive.siteOf(BhistEntry("https://curve.fi", "https://curve.fi/dex", "curve.fi", title, "", 3.0))
            assertEquals(title, "curve.fi", once.name)
            assertEquals(title, "", once.subtitle)
        }
    }

    /**
     * Spec 096 F11: the consent names the account and the network the CORE says
     * a Connect shares — `DbrConsentView.address` and `.chain_id` — not the
     * session's active account and the front tab's chain.
     */
    @Test
    fun `the consent names the core's account and the asking site's network`() {
        val active = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val identity = ExploreLive.Identity(accountName = "Parallel Multi", accountAddress = active, chainName = "BNB Chain", chainId = 1)
        val asked = ExploreLive.consent(
            fallback.connection,
            app.getvela.wallet.feature.browser.core.DbrConsentView(tab = "t2", origin = "https://pancakeswap.finance", address = active.lowercase(), chain_id = 56),
            strings,
            identity,
            secure = true,
        )
        assertEquals("Parallel Multi", asked.accountName)
        assertEquals(ExploreLive.shortAddress(active.lowercase()), asked.accountAddress)
        assertEquals(active.lowercase(), asked.accountSeed)
        assertEquals("the asking site's chain, never the front tab's", app.getvela.wallet.core.marks.Marks.chainLogoUrl(56), asked.networkLogoUrl)

        // A grant about to go to another account is named by that account.
        val other = "0xa9ae000000000000000000000000000000002b2b"
        val switching = ExploreLive.consent(
            fallback.connection,
            app.getvela.wallet.feature.browser.core.DbrConsentView(tab = "t2", origin = "https://pancakeswap.finance", address = other, chain_id = 56),
            strings,
            identity,
            secure = true,
        )
        assertEquals(ExploreLive.shortAddress(other), switching.accountAddress)
        assertEquals(other, switching.accountSeed)
        assertTrue("never the active account's name", switching.accountName != "Parallel Multi")
    }

    /**
     * Spec 097 E (S3): a site the core holds no grant for — never connected,
     * or just disconnected — says so, and the panel offers nothing that
     * implies access: no "can see your address", no Disconnect, no "requests
     * appear here", no account it sees. The desktop pass found the panel
     * saying all of it after a disconnect, with only the status word gone.
     */
    @Test
    fun `a site that is not connected says so and offers no access`() {
        val engine = EngineState(url = "https://app.aave.com/", origin = "https://app.aave.com", host = "app.aave.com", shown = "https://app.aave.com/")
        val me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        val identity = ExploreLive.Identity(accountName = "Parallel Multi", accountAddress = me, chainName = "BNB Chain", chainId = 56)
        val gone = ExploreLive.connection(
            fallback.connection, engine, strings,
            DbrTabView(tab = "t1", origin = "https://app.aave.com", connected_address = null, chain_id = 56, secure = true),
            identity,
        )
        assertFalse(gone.connected)
        assertEquals(strings.t("home.connEmptyTitle"), gone.statusLine)
        assertEquals("it can see nothing", "", gone.explainer)
        assertEquals("no requests come from a site with no grant", "", gone.footnote)

        val granted = ExploreLive.connection(
            fallback.connection, engine, strings,
            DbrTabView(tab = "t1", origin = "https://app.aave.com", connected_address = me, chain_id = 56, secure = true),
            identity,
        )
        assertTrue(granted.connected)
        assertEquals(strings.t("explore.connectedTag"), granted.statusLine)
        assertEquals(strings.t("explore.connectionExplainer"), granted.explainer)
        assertEquals(strings.t("explore.autoRequestHint"), granted.footnote)
    }

    /** Spec 079: the page's chain notice is the pool's verdict — never for a chain merely busy. */
    @Test
    fun `a chain the pool could not reach is named once, and a busy one is not`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", "http://127.0.0.1:8137/", "", "127.0.0.1:8137")), selected_tab = "t1", ready = true)
        val engine = EngineState(url = "http://127.0.0.1:8137/", origin = "http://127.0.0.1:8137", host = "127.0.0.1:8137", shown = "http://127.0.0.1:8137/")
        val tab = DbrTabView(tab = "t1", origin = "http://127.0.0.1:8137", chain_id = 100)
        val identity = ExploreLive.Identity(chainName = "Gnosis", chainId = 100)
        val down = RpcPoolView(failed_chains = listOf(100))
        val busy = RpcPoolView(failed_chains = listOf(100), rate_limited_chains = listOf(100))
        val other = RpcPoolView(failed_chains = listOf(1))

        val named = ExploreLive.home(fallback, view, BhistView(), engine, strings, tab, identity, pool = down)
        assertEquals(strings.t("explore.chainDown", mapOf("chain" to "Gnosis")), named.browser.chainNotice)
        assertTrue(named.browser.chainNotice!!.contains("Gnosis"))
        assertNull(ExploreLive.home(fallback, view, BhistView(), engine, strings, tab, identity, pool = busy).browser.chainNotice)
        assertNull(ExploreLive.home(fallback, view, BhistView(), engine, strings, tab, identity, pool = other).browser.chainNotice)
        assertNull("no page, no notice", ExploreLive.home(fallback, view, BhistView(), null, strings, tab, identity, pool = down).browser.chainNotice)

        // Spec 082 RF1: one full pass that reached nothing is enough to say so
        // while the dApp waits — unless the chain is only busy.
        val unreached = RpcPoolView(unreached_chains = listOf(100))
        assertEquals(named.browser.chainNotice, ExploreLive.home(fallback, view, BhistView(), engine, strings, tab, identity, pool = unreached).browser.chainNotice)
        val unreachedBusy = RpcPoolView(unreached_chains = listOf(100), rate_limited_chains = listOf(100))
        assertNull(ExploreLive.home(fallback, view, BhistView(), engine, strings, tab, identity, pool = unreachedBusy).browser.chainNotice)
    }

    /** Spec 079: a tab card shows its page's snapshot; a start page keeps the drawing. */
    @Test
    fun `a tab with a page carries its snapshot, a start page does not`() {
        val view = ExploreView(
            tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/", "Uniswap", "app.uniswap.org"), ExploreTab("t2", null, "", "")),
            selected_tab = "t1",
            ready = true,
        )
        // A plain JVM test has no android.graphics.Bitmap; any ImageBitmap stands in.
        val image = java.lang.reflect.Proxy.newProxyInstance(
            javaClass.classLoader,
            arrayOf(androidx.compose.ui.graphics.ImageBitmap::class.java),
        ) { proxy, method, args -> if (method.name == "equals") proxy === args?.firstOrNull() else if (method.name == "hashCode") 1 else null } as androidx.compose.ui.graphics.ImageBitmap
        val model = ExploreLive.home(fallback, view, BhistView(), null, strings, snapshots = mapOf("t1" to image, "t2" to image))
        assertEquals(image, model.tabs.first { it.id == "t1" }.snapshot)
        assertNull(model.tabs.first { it.id == "t2" }.snapshot)
    }

    /** Spec 079 (owner): network rows carry a logo and the home screen's balance, never a zero; accounts their identicon. */
    @Test
    fun `network rows carry a logo and what the account holds, accounts their identicon`() {
        val nets = listOf(100L, 56L, 1L, 42161L).map { app.getvela.wallet.feature.settings.core.NetNetworkRow(id = it.toString(), chain_id = it, display_name = "", native_symbol = "") }
        fun token(chain: Int, balance: String, price: Double?, spam: Boolean = false) =
            app.getvela.wallet.feature.wallet.core.BalanceToken(chain_id = chain, symbol = "T", name = "T", balance = balance, decimals = 18, price_usd = price, spam = spam)
        val balances = app.getvela.wallet.feature.wallet.core.BalanceView(
            tokens = listOf(
                token(100, "2.5", 1.0), token(100, "1000", 1.0, spam = true),
                token(56, "0.001", 1.0),
                token(42161, "3", 2000.0),
            ),
            failed_chain_ids = listOf(42161),
        )
        val rows = ExploreLive.networkOptions(nets, mapOf(100 to "Gnosis", 56 to "BNB Chain", 1 to "Ethereum"), siteChain = 100, balances = balances, fiat = { "$" + "%.2f".format(it) })
        val gnosis = rows.first { it.id == "100" }
        assertEquals("Gnosis", gnosis.label)
        assertTrue(gnosis.selected)
        // The logo host is configured at runtime; here it may be unset, so the
        // row must carry exactly what the marks helper names for that chain.
        assertEquals(app.getvela.wallet.core.marks.Marks.chainLogoUrl(100), gnosis.logoUrl)
        assertEquals("spam never counts", "$2.50", gnosis.amount)
        assertNull("dust says nothing", rows.first { it.id == "56" }.amount)
        assertNull("nothing held says nothing, never 0", rows.first { it.id == "1" }.amount)
        assertNull("a chain whose read failed says nothing", rows.first { it.id == "42161" }.amount)
        assertNull("hidden balances stay hidden", ExploreLive.networkOptions(nets, emptyMap(), 100, balances.copy(hidden = true)) { "x" }.first { it.id == "100" }.amount)

        val accounts = ExploreLive.accountOptions(
            listOf(app.getvela.wallet.feature.onboarding.core.SessionAccountRow(0, "Main", "0x88cCA0EeDbF2C4426110bbFc998F048689266894"), app.getvela.wallet.feature.onboarding.core.SessionAccountRow(1, "", "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")),
            active = "0x76875E38FC6BC2DEDCAED807CE00782DB5C0D141",
        )
        assertEquals("0x88cCA0EeDbF2C4426110bbFc998F048689266894", accounts[0].identiconSeed)
        assertTrue(accounts[1].selected)
        assertEquals(ExploreLive.shortAddress("0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"), accounts[1].label)
    }

    @Test
    fun `nothing remembered is the empty start page`() {
        val model = ExploreLive.home(fallback, ExploreView(ready = true), BhistView(), null, strings)
        assertNotNull(model.empty)
        assertNull(model.favorites)
        assertTrue(model.groups.isEmpty())
        assertNull("no tab with a page, no resume section", model.resume)
        assertFalse(model.browser.bookmarked)
    }

    /**
     * Spec 099 navigation: the home's resume rows are the core's `resumable`,
     * in its order and cap — the header counts every tab (the switcher's
     * number) and its action is the switcher's word; a row's id is its TAB's,
     * and it is named by the recents' own label rule.
     */
    @Test
    fun `the resume section is the core's resumable, under a header that counts every tab`() {
        val a = ExploreTab("t1", "https://app.uniswap.org/swap", "Uniswap", "app.uniswap.org")
        val b = ExploreTab("t2", "https://polymarket.com/", "", "polymarket.com")
        val view = ExploreView(
            tabs = listOf(a, b, ExploreTab("t3", null, "", "")),
            selected_tab = "t3",
            recent_tabs = listOf("t2", "t3", "t1"),
            // The core's order, which this shell must keep — not the strip's.
            resumable = listOf(b, a),
            ready = true,
        )
        val resume = ExploreLive.home(fallback, view, BhistView(), null, strings).resume!!
        assertEquals(strings.t("explore.openTabs", mapOf("n" to "3")), resume.title)
        assertEquals(strings.t("explore.tabs"), resume.action)
        assertEquals(listOf("t2", "t1"), resume.tabs.map { it.id })
        assertEquals("an untitled tab is said once, by its host", "polymarket.com", resume.tabs[0].name)
        assertEquals("", resume.tabs[0].subtitle)
        assertEquals("Uniswap", resume.tabs[1].name)
        assertEquals("app.uniswap.org", resume.tabs[1].subtitle)
        // Before the mirror is live there is no section — never one to correct a frame later.
        assertNull(ExploreLive.home(fallback, view.copy(ready = false), BhistView(), null, strings).resume)
    }

    /**
     * Spec 099 navigation: the site menu leads with Forward (the toolbar is
     * gone) — greyed, never hidden, when there is nothing ahead; its refresh
     * row stops a load under way; the star row says what it will do.
     */
    @Test
    fun `the site menu's forward follows the page, and refresh stops a load under way`() {
        val view = ExploreView(tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/", "", "app.uniswap.org")), selected_tab = "t1", ready = true)
        fun menu(engine: EngineState) = ExploreLive.home(fallback, view, BhistView(), engine, strings).siteMenuSheet.items
        val idle = menu(EngineState(url = "https://app.uniswap.org/", origin = "https://app.uniswap.org", host = "app.uniswap.org", shown = "https://app.uniswap.org/"))
        assertEquals(listOf("forward", "refresh", "share", "copy", "favorite", "system", "close"), idle.map { it.id })
        assertFalse("nothing ahead: greyed", idle.first().enabled)
        assertEquals(strings.t("explore.forward"), idle.first().label)
        assertEquals(strings.t("explore.addToFavorites"), idle.first { it.id == "favorite" }.label)
        val ahead = menu(EngineState(url = "https://app.uniswap.org/", origin = "https://app.uniswap.org", host = "app.uniswap.org", canForward = true, shown = "https://app.uniswap.org/"))
        assertTrue(ahead.first().enabled)
        val loading = menu(EngineState(url = "https://app.uniswap.org/", origin = "https://app.uniswap.org", host = "app.uniswap.org", loading = true, shown = "https://app.uniswap.org/"))
        assertEquals("the refresh row's place, so nothing under it moves", ExploreLive.STOP, loading[1].id)
        assertEquals(strings.t("connect.dapp.stop"), loading[1].label)
    }

    @Test
    fun `a host's letter and colour are stable, and hidden system groups stay off the page`() {
        // Spec 079: the core's letter rule skips the `app.` everyone puts in front.
        assertEquals("U", ExploreLive.letterOf("app.uniswap.org"))
        assertEquals("1", ExploreLive.letterOf("127.0.0.1:8137"))
        assertEquals("?", ExploreLive.letterOf("···"))
        assertEquals(ExploreLive.tintOf("app.uniswap.org"), ExploreLive.tintOf("app.uniswap.org"))
        assertNotEquals(ExploreLive.tintOf("app.uniswap.org"), ExploreLive.tintOf("polymarket.com"))
        val hidden = ExploreLive.home(fallback, ExploreView(favorites = listOf(uniswap), favorites_hidden = true, recent_hidden = true, ready = true), BhistView(listOf(BhistEntry("https://curve.fi", "https://curve.fi/", "curve.fi", "Curve", "", 1.0))), null, strings)
        assertTrue("a hidden Favorites draws no tiles", hidden.favorites!!.tiles.isEmpty())
        assertTrue(hidden.groups.isEmpty())
    }

    /**
     * Issue #330: with Favorites and Recent hidden, Explore showed the search
     * field alone — the Favorites heading's Edit was the only way to Manage
     * groups, and it went with the hidden section. The heading stays (no
     * tiles), so hidden groups can always be shown again; and a page with no
     * favourites yet keeps it too, over its add tile.
     */
    @Test
    fun `with every group hidden, the Favorites heading stays the way to Manage groups`() {
        val recents = BhistView(listOf(BhistEntry("https://curve.fi", "https://curve.fi/", "curve.fi", "Curve", "", 1.0)))
        val everyHidden = ExploreLive.home(
            fallback,
            ExploreView(favorites = listOf(uniswap), favorites_hidden = true, recent_hidden = true, ready = true),
            recents, null, strings,
        )
        val heading = everyHidden.favorites!!
        assertEquals(strings.t("explore.favorites"), heading.title)
        assertEquals("its Edit opens Manage groups", strings.t("explore.edit"), heading.action)
        assertTrue(heading.tiles.isEmpty())
        assertTrue("every group is off the page", everyHidden.groups.isEmpty())
        assertNull("not the empty start page", everyHidden.empty)
        // Manage groups still lists both, each with its eye.
        assertEquals(listOf("favorites", "recent"), everyHidden.groupManageSheet.rows.map { it.id })
        assertTrue(everyHidden.groupManageSheet.rows.all { it.hidden })

        // Only Favorites hidden: Recent's heading offers Clear, never Manage.
        val favoritesHidden = ExploreLive.home(fallback, ExploreView(favorites = listOf(uniswap), favorites_hidden = true, ready = true), recents, null, strings)
        assertEquals(strings.t("explore.edit"), favoritesHidden.favorites?.action)
        assertEquals(listOf("recent"), favoritesHidden.groups.map { it.id })

        // No favourites yet, Recent hidden: the heading stays, over its add tile.
        val none = ExploreLive.home(fallback, ExploreView(recent_hidden = true, ready = true), recents, null, strings)
        assertEquals(strings.t("explore.edit"), none.favorites?.action)
        assertTrue(none.favorites!!.tiles.single() is TileModel.Add)

        // Shown again: the tiles come back under the same heading.
        val shown = ExploreLive.home(fallback, ExploreView(favorites = listOf(uniswap), ready = true), recents, null, strings)
        assertEquals(2, shown.favorites!!.tiles.size)
    }
    /**
     * Issue #273: the explore scanner opens web addresses only. A URL opens as
     * read, a bare host the address bar would open gets its `https://`, and
     * everything else — an address, a WalletConnect or payment link, words —
     * is refused rather than guessed into a URL.
     */
    @Test
    fun `a scanned code opens only when it is a web address`() {
        assertEquals("https://app.uniswap.org/swap?x=1", ExploreLive.scannedUrl("  https://app.uniswap.org/swap?x=1 \n"))
        assertEquals("http://192.168.1.2:8080/", ExploreLive.scannedUrl("http://192.168.1.2:8080/"))
        assertEquals("https://Example.com/A", ExploreLive.scannedUrl("HTTPS://Example.com/A"))
        assertEquals("https://app.uniswap.org", ExploreLive.scannedUrl("app.uniswap.org"))
        assertEquals("https://example.com:8443/x", ExploreLive.scannedUrl("example.com:8443/x"))

        assertNull(ExploreLive.scannedUrl("velawallet://pay?a=1"))
        assertNull(ExploreLive.scannedUrl("hello world"))
        assertNull(ExploreLive.scannedUrl("localhost"))
        assertNull(ExploreLive.scannedUrl(""))
    }

    /** D1 option (b), spec 070: every code has a place to go, or a sentence saying why not. */
    @Test
    fun `a scanned payment goes to send and a WalletConnect code is named`() {
        val address = "0x2222222222222222222222222222222222222222"
        assertEquals(ExploreLive.Scanned.Payment(address), ExploreLive.scanned(address))
        assertEquals(ExploreLive.Scanned.Payment("ethereum:$address@1"), ExploreLive.scanned("ethereum:$address@1"))
        assertEquals(ExploreLive.Scanned.WalletConnect, ExploreLive.scanned("wc:7f6e504bfad60b48@2?relay-protocol=irn&symKey=5"))
        assertEquals(ExploreLive.Scanned.Unrecognized, ExploreLive.scanned("hello world"))
        assertEquals(ExploreLive.Scanned.Url("https://app.uniswap.org"), ExploreLive.scanned("app.uniswap.org"))
    }
}
