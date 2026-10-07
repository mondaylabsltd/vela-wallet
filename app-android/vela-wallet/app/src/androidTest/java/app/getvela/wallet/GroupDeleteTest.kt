package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.assertCountEquals
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
 * Issue #439: in Manage groups, a group of the person's own wore a trash
 * icon that did nothing — no tap reached the browser, so a group, once made,
 * could not be removed. Here the live model answers the delete as the core
 * does, and the row must be gone.
 *
 * ```bash
 * adb -s <device> shell am instrument -w -e class app.getvela.wallet.GroupDeleteTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class GroupDeleteTest {

    @get:Rule
    val compose = createComposeRule()

    @Test
    fun theTrashDeletesTheGroup() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val strings = I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
        // E3 opens with Manage groups up: two system groups, then 交易 and 预测市场.
        var model by mutableStateOf(ExploreFixtures.buildState(ExploreScreenState.E3, strings))
        val deleted = mutableListOf<String>()
        val live = ExploreCallbacks(
            onOpenSite = {},
            onTabOpen = {},
            onTabClose = {},
            onTabNew = {},
            onTabsCloseAll = {},
            onGroupToggle = { _, _ -> },
            onGroupNew = {},
            onGroupDelete = { id ->
                deleted += id
                // The core's answer: the group leaves the view.
                model = model.copy(
                    groupManageSheet = model.groupManageSheet.copy(
                        rows = model.groupManageSheet.rows.filterNot { it.id == id },
                    ),
                )
            },
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
        val delete = strings.t("explore.delete")

        // System groups carry no trash: one each for 交易 and 预测市场, and
        // the first is 交易's.
        compose.onAllNodesWithContentDescription(delete).assertCountEquals(2)
        compose.onAllNodesWithContentDescription(delete).onFirst().performClick()
        compose.waitForIdle()

        assertEquals(listOf("trading"), deleted)
        compose.onAllNodesWithContentDescription(delete).assertCountEquals(1)
    }
}
