package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Nothing above moves while the fee is measured (device-found on the Xiaomi:
 * the whole sheet moved ~33 px on every fee refresh, speed pick and 30 s
 * re-quote). The live sheet is bottom-anchored and wraps its content, so a
 * line that comes and goes anywhere in it moves everything above: the sheet's
 * HEIGHT must hold, and the confirm must stay where it is. The web's test
 * walks the same two sequences (open, landed, re-quote, new speed, landed).
 *
 * Emulator only (never on the owner's phone, whose wallet
 * `connectedAndroidTest` would wipe):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SigningSheetStillTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SigningSheetStillTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("zh") }
    }

    private val measuringNote = "正在计算网络费用…"
    private val noCoin = "所有代币都不够付这笔网络费"

    /** The wallet's own key backup (CS36), its speed control folded away: a short sheet that fits the window. */
    private val base: SigningScreenModel by lazy {
        val drawn = SigningFixtures.build(SigningScreenState.CS36, strings)
        drawn.copy(fee = (drawn.fee as FeeModel.OnChain).copy(speed = null, warning = null, refreshing = false, measuring = false))
    }
    private val landed by lazy { base.fee as FeeModel.OnChain }
    /** The 30 s re-quote: the figure stays, the gate shuts and says why. */
    private val requote by lazy { landed.copy(refreshing = true, measuring = true) }
    /** A new speed: no figure of its own yet. */
    private val newSpeed by lazy { landed.copy(value = strings.t("componentsUi.gas.estimating"), refreshing = true, measuring = true) }

    private fun open(fee: FeeModel.OnChain) = base.copy(fee = fee, confirmEnabled = true, confirmBlockLine = null)
    private fun shut(fee: FeeModel.OnChain, note: String? = measuringNote) = base.copy(fee = fee, confirmEnabled = false, confirmBlockLine = note)

    private data class Frame(val sheetHeight: Float, val confirmTop: Float)

    private fun walk(steps: List<SigningScreenModel>): List<Frame> {
        var current by mutableStateOf(steps.first())
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = current, onConfirm = {}, modifier = Modifier.testTag(SHEET))
                }
            }
        }
        return steps.map { step ->
            current = step
            compose.waitForIdle()
            Frame(
                sheetHeight = compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height,
                confirmTop = compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode().boundsInRoot.top,
            )
        }
    }

    @Test
    fun aFundedWalletsSheetHoldsStill() {
        val frames = walk(
            listOf(
                shut(newSpeed),
                open(landed),
                shut(requote),
                open(landed),
                shut(newSpeed),
                open(landed),
            ),
        )
        frames.forEachIndexed { index, frame -> assertEquals("frame $index", frames[0], frame) }
        // The held note is room only: not drawn for TalkBack while the gate is open.
        compose.onNodeWithText(measuringNote).assertDoesNotExist()
    }

    @Test
    fun aWalletNoCoinOfWhichCanPayHoldsStill() {
        val short = landed.copy(warning = noCoin)
        val frames = walk(
            listOf(
                shut(newSpeed),
                shut(short, note = null),
                shut(requote),
                shut(short, note = null),
                shut(newSpeed),
                shut(short, note = null),
            ),
        )
        // From the first landing on (the first measurement had no shortfall
        // to hold yet), not a pixel.
        frames.drop(1).forEachIndexed { index, frame -> assertEquals("frame ${index + 1}", frames[1], frame) }
    }

    @Test
    fun aHeldShortfallIsSilentAndGoesWhenTheFeeLandsWithNothingToSay() {
        val short = landed.copy(warning = noCoin)
        var current by mutableStateOf(shut(short, note = null))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = current, onConfirm = {}, modifier = Modifier.testTag(SHEET))
                }
            }
        }
        compose.onNodeWithText(noCoin).assertExists()
        current = shut(requote)
        compose.waitForIdle()
        // Measured again: the verdict is about the last quote — kept for its room, not said.
        compose.onNodeWithText(noCoin, useUnmergedTree = true).assertDoesNotExist()
        compose.onNodeWithText(measuringNote).assertExists()
        val held = compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height
        current = shut(short, note = null)
        compose.waitForIdle()
        assertEquals(held, compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height)
        // The coin can pay now: the shortfall goes for good.
        current = open(landed)
        compose.waitForIdle()
        current = shut(requote)
        compose.waitForIdle()
        compose.onNodeWithText(noCoin, useUnmergedTree = true).assertDoesNotExist()
    }

    /**
     * Item 12 (the 102 device run): the simulation's verdict lands a second
     * after the sheet opens — on a chain whose nodes cannot simulate, as
     * 「Vela 未能检查这笔交易的结果」 on every request — and the sheet grew
     * upward under the eye. Its place is kept from the first frame (CS57), so
     * the card arriving (CS58) changes neither the sheet's height nor where
     * the confirm is; and it is not said to TalkBack before it is there.
     */
    @Test
    fun theSimulationsVerdictLandingMovesNothing() {
        val waiting = SigningFixtures.build(SigningScreenState.CS57, strings)
        val landed = SigningFixtures.build(SigningScreenState.CS58, strings)
        val couldNotCheck = strings.t("componentsUi.signing.simUnavailableWarning")
        var current by mutableStateOf(waiting)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = current, onConfirm = {}, modifier = Modifier.testTag(SHEET))
                }
            }
        }
        fun frame() = Frame(
            sheetHeight = compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height,
            confirmTop = compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode().boundsInRoot.top,
        )
        compose.waitForIdle()
        val before = frame()
        // The room is unseen and unsaid.
        compose.onNodeWithText(couldNotCheck).assertDoesNotExist()
        current = landed
        compose.waitForIdle()
        compose.onNodeWithText(couldNotCheck).assertExists()
        assertEquals("the card arriving moved the sheet", before, frame())
        // And the sheet with no place kept is shorter: the room is real.
        current = waiting.copy(blocks = waiting.blocks.filterNot { it is app.getvela.wallet.feature.signing.SigningBlock.Held })
        compose.waitForIdle()
        assertTrue("no room was being held", frame().sheetHeight < before.sheetHeight)
    }

    private companion object {
        const val SHEET = "signing-sheet"
    }
}
