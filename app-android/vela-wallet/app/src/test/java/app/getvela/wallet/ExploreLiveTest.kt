package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.ExploreLive
import app.getvela.wallet.feature.browser.core.BhistEntry
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.EngineState
import app.getvela.wallet.feature.browser.core.ExploreGroupView
import app.getvela.wallet.feature.browser.core.ExploreSite
import app.getvela.wallet.feature.browser.core.ExploreTab
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.GroupAction
import app.getvela.wallet.feature.explore.TileModel
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
    private val curve = ExploreSite(origin = "https://curve.fi", url = "https://curve.fi/", host = "curve.fi", name = "Curve", added_ms = 2.0)

    @Test
    fun `favourites, groups, recents and tabs are the core's, and a site's id opens it`() {
        val view = ExploreView(
            favorites = listOf(uniswap),
            groups = listOf(ExploreGroupView("g1", "交易", hidden = false, sites = listOf(curve)), ExploreGroupView("g2", "隐藏", hidden = true, sites = listOf(curve))),
            tabs = listOf(ExploreTab("t1", "https://app.uniswap.org/swap", "Uniswap", "app.uniswap.org"), ExploreTab("t2", null, "", "")),
            selected_tab = "t1",
            ready = true,
        )
        val history = BhistView(listOf(BhistEntry("https://curve.fi", "https://curve.fi/dex", "curve.fi", "", "", 3.0)))
        val engine = EngineState(url = "https://app.uniswap.org/swap", origin = "https://app.uniswap.org", host = "app.uniswap.org", secure = true, title = "Uniswap", canBack = true)
        val model = ExploreLive.home(fallback, view, history, engine, strings)
        assertNull(model.empty)
        val tiles = model.favorites!!.tiles
        assertEquals(2, tiles.size)
        assertEquals("https://app.uniswap.org/swap", (tiles[0] as TileModel.Site).site.id)
        assertTrue(tiles[1] is TileModel.Add)
        assertEquals(listOf("recent", "g1"), model.groups.map { it.id })
        assertEquals(GroupAction.Clear, model.groups[0].action)
        assertEquals("an untitled recent falls back to its host", "curve.fi", model.groups[0].sites.single().name)
        assertEquals("https://curve.fi/dex", model.groups[0].sites.single().id)
        assertEquals(2, model.tabs.size)
        assertTrue(model.tabs[0].selected)
        assertTrue(model.tabs[1].startPage)
        assertEquals(strings.t("explore.startPage"), model.tabs[1].title)
        assertEquals("2", model.tabCountLabel)
        assertTrue("the page is a favourite", model.browser.bookmarked)
        assertTrue(model.browser.canBack)
        assertEquals("app.uniswap.org", model.browser.host)
        assertEquals(listOf("favorites", "recent", "g1", "g2"), model.groupManageSheet.rows.map { it.id })
        assertTrue(model.groupManageSheet.rows[3].hidden)
        assertEquals("app.uniswap.org", model.siteMenuSheet.site.host)
    }

    @Test
    fun `nothing remembered is the empty start page`() {
        val model = ExploreLive.home(fallback, ExploreView(ready = true), BhistView(), null, strings)
        assertNotNull(model.empty)
        assertNull(model.favorites)
        assertTrue(model.groups.isEmpty())
        assertNull(model.tabCountLabel)
        assertFalse(model.browser.bookmarked)
    }

    @Test
    fun `a host's letter and colour are stable, and hidden system groups stay off the page`() {
        assertEquals("A", ExploreLive.letterOf("app.uniswap.org"))
        assertEquals("1", ExploreLive.letterOf("127.0.0.1:8137"))
        assertEquals("?", ExploreLive.letterOf("···"))
        assertEquals(ExploreLive.tintOf("app.uniswap.org"), ExploreLive.tintOf("app.uniswap.org"))
        assertNotEquals(ExploreLive.tintOf("app.uniswap.org"), ExploreLive.tintOf("polymarket.com"))
        val hidden = ExploreLive.home(fallback, ExploreView(favorites = listOf(uniswap), favorites_hidden = true, recent_hidden = true, ready = true), BhistView(listOf(BhistEntry("https://curve.fi", "https://curve.fi/", "curve.fi", "Curve", "", 1.0))), null, strings)
        assertNull(hidden.favorites)
        assertTrue(hidden.groups.isEmpty())
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

        assertNull(ExploreLive.scannedUrl("0x2222222222222222222222222222222222222222"))
        assertNull(ExploreLive.scannedUrl("wc:7f6e504bfad60b48@2?relay-protocol=irn&symKey=5"))
        assertNull(ExploreLive.scannedUrl("ethereum:0x2222222222222222222222222222222222222222@1"))
        assertNull(ExploreLive.scannedUrl("velawallet://pay?a=1"))
        assertNull(ExploreLive.scannedUrl("hello world"))
        assertNull(ExploreLive.scannedUrl("localhost"))
        assertNull(ExploreLive.scannedUrl(""))
    }
}
