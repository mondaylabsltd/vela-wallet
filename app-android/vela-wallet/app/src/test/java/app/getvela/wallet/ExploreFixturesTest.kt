package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.ExploreSheet
import app.getvela.wallet.feature.explore.ExploreView
import app.getvela.wallet.feature.explore.TileModel
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 022 gates for the explore layer.
 *
 * The failure this file exists for is Android-specific: `VelaStrings.t()`
 * returns the KEY when a lookup misses, so a typo ships as
 * "explore.startTitle" rendered on screen. Nothing else catches that — not the
 * compiler, not a preview, not a screenshot somebody skims.
 */
class ExploreFixturesTest {

    private val repoRoot = File(
        System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)"),
    )

    private fun zhStrings(): I18nRuntime = I18nRuntime { tag ->
        File(repoRoot, "assets/i18n/$tag.json").readBytes()
    }.apply { initialize("zh") }

    /** Every string a state carries, flattened — the echo check's input. */
    private fun stringsOf(state: ExploreScreenState): List<String> {
        val m = ExploreFixtures.buildState(state, zhStrings())
        val out = mutableListOf(
            m.title, m.searchPlaceholder, m.scanLabel,
            m.tabsScreen.title, m.tabsScreen.done, m.tabsScreen.newTab,
            m.tabsScreen.closeAll, m.tabsScreen.close,
            m.nav.wallet, m.nav.contacts, m.nav.explore, m.nav.settings,
            m.connection.title, m.connection.statusLine, m.connection.switchLabel,
            m.connection.networkLabel, m.connection.explainer, m.connection.disconnect,
            m.connection.footnote,
        )
        m.empty?.let { out += listOf(it.title, it.caption, it.cta) }
        m.favorites?.let { out += listOf(it.title, it.action) }
        m.groups.forEach { g -> out += g.title }
        m.siteMenuSheet.items.forEach { out += it.label }
        m.groupManageSheet.rows.forEach { row -> out += listOfNotNull(row.title, row.meta) }
        out += m.groupManageSheet.newGroup
        return out
    }

    @Test
    fun noStringEchoesItsCorpusKey() {
        for (state in ExploreScreenState.entries) {
            for (value in stringsOf(state)) {
                assertFalse(
                    "`$value` in $state looks like an unresolved key",
                    value.startsWith("explore.") || value.startsWith("componentsUi."),
                )
                assertTrue("empty string in $state", value.isNotBlank())
            }
        }
    }

    @Test
    fun noTemplateIsLeftUnfilled() {
        for (state in ExploreScreenState.entries) {
            for (value in stringsOf(state)) {
                assertFalse("`$value` still carries a {{var}}", value.contains("{{"))
            }
        }
    }

    @Test
    fun e1IsTheEmptyStartPage() {
        val e1 = ExploreFixtures.buildState(ExploreScreenState.E1, zhStrings())
        assertNotNull(e1.empty)
        assertNull(e1.favorites)
        assertTrue(e1.groups.isEmpty())
        assertNull(e1.tabCountLabel)
    }

    @Test
    fun e2CarriesEightTilesAndThreeGroups() {
        val e2 = ExploreFixtures.buildState(ExploreScreenState.E2, zhStrings())
        assertEquals(8, e2.favorites?.tiles?.size)
        assertTrue(e2.favorites?.tiles?.last() is TileModel.Add)
        assertEquals(listOf("recent", "trading", "prediction"), e2.groups.map { it.id })
        // Custom group names are what a person typed — never translated.
        assertEquals(listOf("交易", "预测市场"), e2.groups.drop(1).map { it.title })
    }

    @Test
    fun sheetsOpenOnlyWhereTheMockOpensThem() {
        val zh = zhStrings()
        assertTrue(
            ExploreFixtures.buildState(ExploreScreenState.E3, zh).sheet is ExploreSheet.GroupManage,
        )
        assertTrue(
            ExploreFixtures.buildState(ExploreScreenState.E6, zh).sheet is ExploreSheet.SiteMenu,
        )
        assertTrue(
            ExploreFixtures.buildState(ExploreScreenState.E7, zh).sheet is ExploreSheet.Connection,
        )
        for (state in listOf(ExploreScreenState.E1, ExploreScreenState.E2, ExploreScreenState.E4)) {
            assertNull(ExploreFixtures.buildState(state, zh).sheet)
        }
    }

    @Test
    fun viewsMatchTheirMocks() {
        val zh = zhStrings()
        for (state in listOf(ExploreScreenState.E4, ExploreScreenState.E6, ExploreScreenState.E7)) {
            assertEquals(ExploreView.Browsing, ExploreFixtures.buildState(state, zh).view)
        }
        assertEquals(
            ExploreView.Tabs,
            ExploreFixtures.buildState(ExploreScreenState.E5, zh).view,
        )
        assertEquals(
            ExploreView.Start,
            ExploreFixtures.buildState(ExploreScreenState.E2, zh).view,
        )
    }

    @Test
    fun e5SelectsTheTabItWasOpenedFrom() {
        val tabs = ExploreFixtures.buildState(ExploreScreenState.E5, zhStrings()).tabs
        assertEquals("uniswap", tabs.first { it.selected }.id)
    }

    @Test
    fun systemGroupsCanBeHiddenButNeverDeleted() {
        val rows = ExploreFixtures.buildState(ExploreScreenState.E3, zhStrings())
            .groupManageSheet.rows
        assertEquals(listOf("favorites", "recent"), rows.filter { it.system }.map { it.id })
    }

    @Test
    fun theStandInPageIsTheSitesContent() {
        val page = ExploreFixtures.buildState(ExploreScreenState.E4, zhStrings()).browser.page
        assertEquals("兑换", page.title)
        assertEquals(listOf("ETH", "USDC"), page.fields.map { it.symbol })
        assertEquals(ExploreFixtures.Brand.uniswap, page.ctaTint)
    }
}
