package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithContentDescription
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
