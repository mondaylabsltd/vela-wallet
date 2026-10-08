package app.getvela.wallet

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.explore.ExploreCallbacks
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreen
import app.getvela.wallet.feature.explore.ExploreScreenState
import app.getvela.wallet.feature.explore.ExploreView
import app.getvela.wallet.feature.wallet.components.VelaTab
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Spec 099 navigation on the screen (boards E2/E4/E6): while a page is up the
 * app's tab bar stays under it and the browser draws one bar at the top; ‹
 * and system Back walk to 探索's home with the tab kept; 探索 again starts a
 * new visit from the host's landing; the home resumes a tab in one tap and
 * carries no tab-count box; Forward lives in ⋯, greyed with nothing ahead.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.ExploreNavigationTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class ExploreNavigationTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = I18nRuntime { tag ->
        InstrumentationRegistry.getInstrumentation().targetContext.assets.open("i18n/$tag.json").use { it.readBytes() }
    }.apply { initialize("en") }

    private val opened = mutableListOf<String>()
    private val selected = mutableListOf<VelaTab>()
    private val live = ExploreCallbacks(
        onOpenSite = {},
        onTabOpen = { id -> opened += id },
        onTabClose = {},
        onTabNew = {},
        onTabsCloseAll = {},
        onGroupToggle = { _, _ -> },
        onSiteMenuPick = {},
        onBookmark = {},
        onRecentClear = {},
    )

    /**
     * The screen as the host drives it: [landing] for each visit, and 探索
     * tapped again is a new visit (the host's `Reselect`), whose landing is
     * the home.
     */
    private fun show(state: ExploreScreenState, landing: ExploreView) {
        val model = ExploreFixtures.buildState(state, strings)
        compose.setContent {
            var visit by remember { mutableIntStateOf(0) }
            var lands by remember { mutableStateOf(landing) }
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    ExploreScreen(
                        model = model,
                        live = live,
                        landing = lands,
                        visit = visit,
                        page = { Box(Modifier.fillMaxSize().background(Color.White)) },
                        onPageBack = { false },
                        onSelectTab = { tab ->
                            selected += tab
                            if (tab == VelaTab.Explore) {
                                lands = ExploreView.Start
                                visit += 1
                            }
                        },
                    )
                }
            }
        }
    }

    private val openTabs get() = strings.t("explore.openTabs", mapOf("n" to "4"))

    @Test
    fun aPageHasOneTopBarAndTheAppTabBarUnderIt() {
        show(ExploreScreenState.E4, ExploreView.Browsing)
        compose.onNodeWithContentDescription(strings.t("explore.back")).assertExists()
        compose.onNodeWithContentDescription(strings.t("explore.siteMenu")).assertExists()
        compose.onNodeWithContentDescription(strings.t("explore.tabs")).assertExists()
        compose.onNodeWithText("4").assertExists()
        // The leading ✕ that closed the tab is gone; closing lives in ⋯ and the switcher.
        compose.onNodeWithContentDescription(strings.t("explore.closePage")).assertDoesNotExist()
        // The wallet is one tap away.
        compose.onNodeWithContentDescription(strings.t("componentsUi.mainNav.wallet")).performClick()
        assertEquals(listOf(VelaTab.Wallet), selected)
    }

    @Test
    fun backWithNoHistoryIsTheHomeAndARowResumesTheTab() {
        show(ExploreScreenState.E4, ExploreView.Browsing)
        compose.onNodeWithContentDescription(strings.t("explore.back")).performClick()
        compose.onNodeWithText(openTabs).assertExists()
        // The row's host line: the favourites grid has a Polymarket tile too.
        compose.onNodeWithText("polymarket.com").performClick()
        assertEquals(listOf("polymarket"), opened)
        compose.onNodeWithContentDescription(strings.t("explore.back")).assertExists()
    }

    @Test
    fun systemBackLeavesThePageForTheHome() {
        show(ExploreScreenState.E4, ExploreView.Browsing)
        Espresso.pressBack()
        compose.onNodeWithText(openTabs).assertExists()
    }

    @Test
    fun exploreAgainWhileBrowsingIsTheHome() {
        show(ExploreScreenState.E4, ExploreView.Browsing)
        compose.onNodeWithContentDescription(strings.t("componentsUi.mainNav.explore")).performClick()
        assertEquals(listOf(VelaTab.Explore), selected)
        compose.onNodeWithText(openTabs).assertExists()
        compose.onNodeWithContentDescription(strings.t("explore.back")).assertDoesNotExist()
    }

    @Test
    fun theHomeHasNoCountBoxAndItsHeaderOpensTheSwitcher() {
        show(ExploreScreenState.E2, ExploreView.Start)
        compose.onNodeWithText("4").assertDoesNotExist()
        compose.onNodeWithText(strings.t("explore.tabs")).performClick()
        compose.onNodeWithText(strings.t("explore.done")).performClick()
        // Done goes back where the switcher was opened from: the home.
        compose.onNodeWithText(openTabs).assertExists()
    }

    @Test
    fun forwardIsInTheMenuGreyedWithNothingAhead() {
        show(ExploreScreenState.E4, ExploreView.Browsing)
        compose.onNodeWithContentDescription(strings.t("explore.siteMenu")).performClick()
        compose.onNodeWithText(strings.t("explore.forward")).assertIsNotEnabled()
        compose.onNodeWithText(strings.t("explore.closePage")).assertExists()
    }
}
