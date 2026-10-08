package app.getvela.wallet

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.feature.wallet.BalanceRefreshModel
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_TEST_TAG
import app.getvela.wallet.feature.wallet.components.BalanceRefreshControl
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue 462: the hero's "↻ Updated 2m" on a real renderer. It is a button;
 * while it turns it reads "Updating…" and a tap does nothing; and it is the
 * SAME box in both states, so it never moves under the finger that tapped it.
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.BalanceRefreshControlTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class BalanceRefreshControlTest {

    @get:Rule
    val compose = createComposeRule()

    private val model = BalanceRefreshModel(updated = "Updated 2m", updating = "Updating…")

    @Test
    fun itTurnsInPlaceAndRefusesASecondTap() {
        var spinning by mutableStateOf(false)
        var taps = 0
        compose.setContent {
            VelaTheme(darkTheme = false) {
                BalanceRefreshControl(model = model, spinning = spinning, onRefresh = { taps++; spinning = true })
            }
        }
        val control = compose.onNodeWithTag(BALANCE_REFRESH_TEST_TAG)
        control.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
        control.assertIsEnabled()
        compose.onNodeWithText("Updated 2m").assertExists()
        val idle = control.fetchSemanticsNode().boundsInRoot

        control.performClick()
        compose.waitForIdle()
        assertEquals(1, taps)
        compose.onNodeWithText("Updating…").assertExists()
        control.assertIsNotEnabled()
        assertEquals("the control keeps its box while it turns", idle, control.fetchSemanticsNode().boundsInRoot)

        control.performClick()
        compose.waitForIdle()
        assertEquals("a tap while it turns does nothing", 1, taps)

        spinning = false
        compose.waitForIdle()
        assertEquals(idle, control.fetchSemanticsNode().boundsInRoot)
        control.assertIsEnabled()
    }

    @Test
    fun theGallerysControlTakesNoTap() {
        compose.setContent {
            VelaTheme(darkTheme = false) {
                BalanceRefreshControl(model = model, spinning = false, onRefresh = null)
            }
        }
        compose.onNodeWithTag(BALANCE_REFRESH_TEST_TAG).assertIsNotEnabled()
    }
}
