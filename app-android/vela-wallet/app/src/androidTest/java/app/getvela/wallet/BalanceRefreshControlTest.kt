package app.getvela.wallet

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.foundation.layout.Row
import androidx.compose.ui.test.click
import androidx.compose.ui.test.performTouchInput
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
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_STAMP_TAG
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_TEST_TAG
import app.getvela.wallet.feature.wallet.components.BalanceRefreshStamp
import app.getvela.wallet.feature.wallet.components.BalanceRefreshControl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Issue 462: the hero's refresh control on a real renderer. It is a button;
 * while it turns a tap does nothing; and it is the SAME box in every state,
 * so it never moves under the finger that tapped it.
 *
 * The 2026-10-11 walk, item 4: it is the glyph, at the END of the line under
 * the total, and its words — "Updated 2m", "Updating…" — are said in that
 * line's place ([BalanceRefreshStamp]). On screen or not, they are its name.
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

    private val model = BalanceRefreshModel(updated = "Updated 2m", updating = "Updating…", idleLabel = "Refresh balance")

    private fun named(name: String) = SemanticsMatcher.expectValue(SemanticsProperties.ContentDescription, listOf(name))

    @Test
    fun itTurnsInPlaceAndRefusesASecondTap() {
        var spinning by mutableStateOf(false)
        var taps = 0
        compose.setContent {
            VelaTheme(darkTheme = false) {
                Row {
                    BalanceRefreshStamp(model = model, spinning = spinning, onRefresh = { taps++; spinning = true })
                    BalanceRefreshControl(model = model, spinning = spinning, onRefresh = { taps++; spinning = true })
                }
            }
        }
        val control = compose.onNodeWithTag(BALANCE_REFRESH_TEST_TAG)
        val stamp = compose.onNodeWithTag(BALANCE_REFRESH_STAMP_TAG, useUnmergedTree = true)
        control.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
        control.assertIsEnabled()
        control.assert(named("Updated 2m"))
        val idle = control.fetchSemanticsNode().boundsInRoot
        val words = stamp.fetchSemanticsNode().boundsInRoot

        control.performClick()
        compose.waitForIdle()
        assertEquals(1, taps)
        control.assert(named("Updating…"))
        control.assertIsNotEnabled()
        assertEquals("the control keeps its box while it turns", idle, control.fetchSemanticsNode().boundsInRoot)
        assertEquals("…and its words keep theirs", words, stamp.fetchSemanticsNode().boundsInRoot)

        control.performClick()
        compose.waitForIdle()
        assertEquals("a tap while it turns does nothing", 1, taps)

        spinning = false
        compose.waitForIdle()
        assertEquals(idle, control.fetchSemanticsNode().boundsInRoot)
        control.assertIsEnabled()
        control.assert(named("Updated 2m"))
    }

    /**
     * Before any read has settled the control has no words to show. It still
     * has a name, and the name is not "Updating…" over a control at rest; once
     * it turns, or has read, its words name it — in a box that is the same
     * size through all three.
     */
    @Test
    fun theGlyphIsNamedInEveryStateAndIsOneSize() {
        var updated by mutableStateOf<String?>(null)
        var spinning by mutableStateOf(false)
        compose.setContent {
            VelaTheme(darkTheme = false) {
                BalanceRefreshControl(model = model.copy(updated = updated), spinning = spinning, onRefresh = { spinning = true })
            }
        }
        val control = compose.onNodeWithTag(BALANCE_REFRESH_TEST_TAG)
        control.assert(named("Refresh balance"))
        control.assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, Role.Button))
        control.assertIsEnabled()
        val box = control.fetchSemanticsNode().boundsInRoot
        val dp = compose.density.density
        assertTrue("a finger's width: ${box.width / dp}dp", box.width >= 44 * dp - 1)

        control.performClick()
        compose.waitForIdle()
        control.assert(named("Updating…"))
        assertEquals(box, control.fetchSemanticsNode().boundsInRoot)

        updated = "Updated now"
        spinning = false
        compose.waitForIdle()
        control.assert(named("Updated now"))
        assertEquals(box, control.fetchSemanticsNode().boundsInRoot)
    }

    /** The words are drawn once and said once: on screen in the line, to TalkBack as the control's name. */
    @Test
    fun theWordsAreDrawnInTheLineAndSaidByTheControl() {
        var taps = 0
        compose.setContent {
            VelaTheme(darkTheme = false) {
                Row {
                    BalanceRefreshStamp(model = model, spinning = false, onRefresh = { taps++ })
                    BalanceRefreshControl(model = model, spinning = false, onRefresh = { taps++ })
                }
            }
        }
        // Drawn: the stamp has a size. Unsaid: no text of it reaches the tree a screen reader walks.
        assertTrue(compose.onNodeWithTag(BALANCE_REFRESH_STAMP_TAG, useUnmergedTree = true).fetchSemanticsNode().size.width > 0)
        compose.onNodeWithText("Updated 2m").assertDoesNotExist()
        compose.onNodeWithTag(BALANCE_REFRESH_TEST_TAG).assert(named("Updated 2m"))
        // A tap on the words refreshes, as a tap on the glyph does.
        compose.onNodeWithTag(BALANCE_REFRESH_STAMP_TAG, useUnmergedTree = true).performTouchInput { click() }
        compose.waitForIdle()
        assertEquals(1, taps)
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
