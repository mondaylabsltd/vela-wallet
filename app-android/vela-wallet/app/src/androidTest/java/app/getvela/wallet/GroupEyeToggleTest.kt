package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.explore.ExploreCallbacks
import app.getvela.wallet.feature.explore.ExploreFixtures
import app.getvela.wallet.feature.explore.ExploreScreen
import app.getvela.wallet.feature.explore.ExploreScreenState
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue #410: in Manage groups, hiding a group and tapping its eye again at
 * once did nothing — the toggle read the row from the sheet as it opened,
 * which still said "shown", so the second tap asked to hide it again. Here the
 * live model answers each tap as the core does, and the second tap must ask
 * to show.
 *
 * ```bash
 * adb -s <device> shell am instrument -w -e class app.getvela.wallet.GroupEyeToggleTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class GroupEyeToggleTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun theEyeTogglesBackAndForthInTheSameSheet() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
        // E3 opens with Manage groups up.
        var model by mutableStateOf(ExploreFixtures.buildState(ExploreScreenState.E3, strings))
        val asked = mutableListOf<Pair<String, Boolean>>()
        val live = ExploreCallbacks(
            onOpenSite = {},
            onTabOpen = {},
            onTabClose = {},
            onTabNew = {},
            onTabsCloseAll = {},
            onGroupToggle = { id, hidden ->
                asked += id to hidden
                // The core's answer: the row's hidden flag follows the ask.
                model = model.copy(
                    groupManageSheet = model.groupManageSheet.copy(
                        rows = model.groupManageSheet.rows.map { row ->
                            if (row.id == id) row.copy(hidden = hidden) else row
                        },
                    ),
                )
            },
            onGroupNew = {},
            onSiteMenuPick = {},
            onBookmark = {},
            onRecentClear = {},
        )
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    ExploreScreen(model = model, live = live)
                }
            }
        }
        val hide = strings.t("explore.hide")
        val show = strings.t("explore.show")

        // Favorites is the first row.
        compose.onAllNodesWithContentDescription(hide).onFirst().performClick()
        compose.waitForIdle()
        compose.onAllNodesWithContentDescription(show).onFirst().performClick()
        compose.waitForIdle()

        assertEquals(listOf("favorites" to true, "favorites" to false), asked)
    }
}
