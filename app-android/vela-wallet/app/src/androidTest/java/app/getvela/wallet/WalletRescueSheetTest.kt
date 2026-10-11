package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.wallet.WalletRescue
import app.getvela.wallet.feature.wallet.components.WalletRescueSheet
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Spec 092 device pass (2026-10-02): on the Xiaomi, the ✕ of a row's RPC fix
 * closed the whole sheet back to Home instead of returning to the list.
 *
 * Drives the real sheet — the wallet route's `WalletRescueSheet`, the same
 * close button and the same Material dismissal (Back stands for the swipe and
 * the scrim: all three arrive as the sheet's dismiss request, after Material
 * has hidden it).
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.WalletRescueSheetTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class WalletRescueSheetTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }
    private val model = SettingsFixtures.buildState(SettingsScreenState.SR6, strings)
    private var rescue by mutableStateOf(WalletRescue(SettingsOverlay.Unreachable))

    private fun show() {
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    WalletRescueSheet(rescue = rescue, model = model, onMove = { rescue = it })
                }
            }
        }
        compose.waitForIdle()
    }

    private fun openFix() {
        compose.onAllNodesWithText(model.unreachable.rows[1].action!!)[1].performClick()
        compose.waitForIdle()
        assertEquals(SettingsOverlay.RpcFix, rescue.overlay)
        assertEquals(model.unreachable.rows[1].chainId.toLong(), rescue.chainId)
        compose.onNodeWithText(model.rpcFix.title).assertExists()
    }

    @Test
    fun theFixsCloseButtonReturnsToTheListOverHome() {
        show()
        openFix()
        compose.onNodeWithContentDescription(model.closeLabel).performClick()
        compose.waitForIdle()
        assertEquals(SettingsOverlay.Unreachable, rescue.overlay)
        // The list is on screen again — not a sheet hidden behind its own state.
        compose.onNodeWithText(model.unreachable.title).assertExists()
        compose.onNodeWithText(model.unreachable.rows[0].name).assertExists()

        // The list's own ✕ then closes it.
        compose.onNodeWithContentDescription(model.closeLabel).performClick()
        compose.waitForIdle()
        assertEquals(SettingsOverlay.None, rescue.overlay)
    }

    /**
     * The integration's note 4: a network on the list for its TOKEN LIST
     * (Tempo — its RPC answers) draws no "Fix", and nothing on its row opens
     * the RPC editor. The real balance machine's view (SR7).
     */
    @Test
    fun aNetworkDownForItsTokenListOffersNoRpcFix() {
        val tempo = SettingsFixtures.buildState(SettingsScreenState.SR7, strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    WalletRescueSheet(rescue = rescue, model = tempo, onMove = { rescue = it })
                }
            }
        }
        compose.waitForIdle()
        compose.onNodeWithText("Can't load Tempo's token list right now").assertExists()
        compose.onNodeWithText("Tempo").assertExists()
        compose.onAllNodesWithText(strings.t(app.getvela.wallet.core.i18n.I18nKeys.SettingsUi.RPC_FIX)).assertCountEquals(0)
        compose.onNodeWithText("Tempo").performClick()
        compose.waitForIdle()
        assertEquals("the row is no door to the RPC editor", SettingsOverlay.Unreachable, rescue.overlay)
    }

    private fun showBreakdown(state: SettingsScreenState): app.getvela.wallet.feature.settings.BalanceDetailModel {
        val board = SettingsFixtures.buildState(state, strings)
        rescue = WalletRescue(SettingsOverlay.BalanceDetail)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    WalletRescueSheet(rescue = rescue, model = board, onMove = { rescue = it })
                }
            }
        }
        compose.waitForIdle()
        return board.balanceDetail
    }

    /**
     * PR 3 final note F20: "Networks still updating — These networks couldn't
     * be reached, so your cached balance is shown until they recover." headed
     * an EMPTY list. The sheet a person opens from "Some tokens couldn't be
     * priced." told a healthy wallet that networks could not be reached and
     * its balance was cached. A heading is drawn with its rows, and not
     * without them. The real balance machine's round (SR3E).
     */
    @Test
    fun theBreakdownDrawsNoStillUpdatingHeadingOverAnEmptyList() {
        val detail = showBreakdown(SettingsScreenState.SR3E)
        assertEquals("Networks still updating", detail.sectionPending)
        compose.onNodeWithText(detail.title).assertExists()
        compose.onNodeWithText(detail.sectionPending).assertDoesNotExist()
        compose.onNodeWithText(detail.pendingNote).assertDoesNotExist()
        // What it does have is headed: the networks that answered, and the token with no price.
        compose.onNodeWithText(detail.sectionDone).assertExists()
        compose.onNodeWithText("Gnosis").assertExists()
        compose.onNodeWithText("ODD").assertExists()
        // The home line's sentence is that last heading: said once, not twice.
        compose.onAllNodesWithText("Some tokens couldn't be priced.").assertCountEquals(1)
    }

    /**
     * …and WITH its rows when there are some — where the row of a network
     * whose token list did not load says the core's short status (F21):
     * "Token list unavailable", never "RPC unavailable" over an RPC that
     * answers. The real machine's round (SR3D).
     */
    @Test
    fun theBreakdownHeadsANetworkOutOfReachAndSaysItsShortStatus() {
        val detail = showBreakdown(SettingsScreenState.SR3D)
        compose.onNodeWithText(detail.sectionPending).assertExists()
        compose.onNodeWithText(detail.pendingNote).assertExists()
        compose.onNodeWithText("Tempo").assertExists()
        compose.onNodeWithText("Token list unavailable").assertExists()
        compose.onAllNodesWithText("RPC unavailable").assertCountEquals(0)
        compose.onNodeWithText(strings.t(app.getvela.wallet.core.i18n.I18nKeys.SettingsUi.BALANCE_DETAIL_RETRY)).assertExists()
    }

    /**
     * PR 3 final note F16: the home's line is ONE line and cuts a sentence
     * longer than it — so the sheet the line opens says the sentence in full,
     * at its top. A read that failed inside the app (SR3F).
     */
    @Test
    fun theBreakdownLeadsWithTheHomeLinesWholeSentence() {
        val detail = showBreakdown(SettingsScreenState.SR3F)
        val sentence = "Something went wrong inside Vela. If it keeps happening, reopen the app."
        assertEquals(sentence, detail.lead)
        val lead = compose.onNodeWithTag(app.getvela.wallet.feature.settings.SHEET_LEAD_TAG, useUnmergedTree = true)
        lead.assertTextEquals(sentence)
        // Whole: on as many lines as it takes, never cut.
        val layouts = mutableListOf<androidx.compose.ui.text.TextLayoutResult>()
        lead.fetchSemanticsNode().config[androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult].action!!.invoke(layouts)
        assertEquals(false, layouts.first().hasVisualOverflow)
        // At the top: above the total and every row.
        val top = lead.fetchSemanticsNode().boundsInRoot.top
        assertEquals(true, top < compose.onNodeWithText(detail.summary).fetchSemanticsNode().boundsInRoot.top)
        assertEquals(true, top > compose.onNodeWithText(detail.title).fetchSemanticsNode().boundsInRoot.top)
    }

    @Test
    fun aSwipeOrBackOnTheFixClosesTheWholeSheet() {
        show()
        openFix()
        Espresso.pressBack()
        compose.waitForIdle()
        assertEquals(SettingsOverlay.None, rescue.overlay)
        compose.onNodeWithText(model.unreachable.title).assertDoesNotExist()
    }
}
