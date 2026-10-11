package app.getvela.wallet

import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowHost
import app.getvela.wallet.feature.flows.FlowScreenModel
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsOverlay
import app.getvela.wallet.feature.settings.SettingsScreenModel
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletHomeModel
import app.getvela.wallet.feature.wallet.WalletRescue
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.components.WalletRescueSheet
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Withholding a fiat figure never moves the layout (the core's rule,
 * `app::display_currency`): while the display currency is not the person's
 * yet every fiat figure keeps its room, and when the currency commits the
 * figure lands in place — nothing else on the screen moves.
 *
 * Each case draws a surface with the currency on its way (a cold start with
 * CNY stored), swaps the SAME composition to the committed frame — what a
 * session does, a second or two after launch — and compares where everything
 * that is on both frames sits. The send form and the signing sheet are walked
 * twice: landing in CNY (the worth wraps the fee onto a second line) and in a
 * currency short enough to fit after all (the row keeps the room it took).
 *
 * Emulator only (never on the owner's phone, whose wallet
 * `connectedAndroidTest` would wipe):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.CurrencyLandingStillTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class CurrencyLandingStillTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    /** Where each of [texts] sits (its top and bottom in the root), in order; a text not on screen is `null`. */
    private fun places(texts: List<String>): List<Pair<Float, Float>?> = texts.map { text ->
        compose.onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().firstOrNull()?.boundsInRoot?.let { it.top to it.bottom }
    }

    private fun <T> walk(first: T, then: List<T>, anchors: List<String>, content: @Composable (T) -> Unit): List<List<Pair<Float, Float>?>> {
        var current by mutableStateOf(first)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { content(current) }
            }
        }
        compose.waitForIdle()
        // A sheet slides in: wait for its anchors to be there.
        compose.waitUntil(10_000) { places(anchors).all { it != null } }
        compose.waitForIdle()
        val frames = mutableListOf(places(anchors))
        for (next in then) {
            current = next
            compose.waitForIdle()
            frames += places(anchors)
        }
        return frames
    }

    private fun assertStill(surface: String, anchors: List<String>, frames: List<List<Pair<Float, Float>?>>) {
        assertTrue("$surface: every anchor is on the first frame: ${anchors.zip(frames[0])}", frames[0].all { it != null })
        frames.drop(1).forEachIndexed { index, frame ->
            assertEquals("$surface: frame ${index + 1} moved (${anchors.zip(frames[0].zip(frame))})", frames[0], frame)
        }
    }

    // -- the send form: the "≈" line under the amount, and the fee's worth --------

    private fun form(state: FlowState) = FlowFixtures.build(state, strings)

    /**
     * The landed form as the LIVE builder hands it over: with no word about
     * room (`worthRoom` false — the currency has committed). The landed BOARD
     * sets it, standing in for what the row remembers; here the row itself
     * has to have kept it.
     */
    private fun asBuilt(landed: FlowScreenModel): FlowScreenModel {
        val model = (landed.base as FlowBase.SendForm).model
        return landed.copy(base = FlowBase.SendForm(model.copy(fee = model.fee.copy(worthRoom = false), speed = model.speed?.copy(worthRoom = false))))
    }

    /** The landed form with a worth short enough to fit beside the label after all (a dollar figure). */
    private fun shortLanding(landed: FlowScreenModel): FlowScreenModel {
        val model = (landed.base as FlowBase.SendForm).model
        return landed.copy(
            base = FlowBase.SendForm(
                model.copy(
                    amount = model.amount!!.copy(fiat = "≈ $120.00"),
                    fee = model.fee.copy(value = "0.000123 ETH · ≈$0.31"),
                ),
            ),
        )
    }

    @Test
    fun theSendFormHoldsStillWhenTheCurrencyLands() {
        val waiting = form(FlowState.SD2N)
        val landed = asBuilt(form(FlowState.SD2O))
        val model = (waiting.base as FlowBase.SendForm).model
        val anchors = listOf(model.header.title, model.recipient!!.label, model.fee.label, model.speed!!.label, model.cta)
        assertStill("send form", anchors, walk(waiting, listOf(landed, shortLanding(landed), landed), anchors) { FlowHost(model = it) })
    }

    @Test
    fun theConfirmHoldsStillWhenTheCurrencyLands() {
        val waiting = form(FlowState.SD3J)
        val landed = form(FlowState.SD3K)
        val model = (waiting.base as FlowBase.SendConfirm).model
        val anchors = model.facts.map { it.label } + model.cta
        assertStill("send confirm", anchors, walk(waiting, listOf(landed), anchors) { FlowHost(model = it) })
    }

    // -- the token page: its worth and its price ----------------------------------

    @Test
    fun theTokenPageHoldsStillWhenTheCurrencyLands() {
        val waiting = form(FlowState.T2W)
        val landed = form(FlowState.T2C)
        val model = (waiting.sheet as FlowSheet.TokenDetail).model
        val anchors = listOf(model.balance, model.receive, model.send) + model.facts.map { it.label } + model.transactionsTitle
        assertStill("token page", anchors, walk(waiting, listOf(landed), anchors) { FlowHost(model = it) })
    }

    // -- the signing sheet: the fee's worth ---------------------------------------

    private data class Frame(val sheetHeight: Float, val confirmTop: Float)

    private fun sheetFrame() = Frame(
        sheetHeight = compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height,
        confirmTop = compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode().boundsInRoot.top,
    )

    @Test
    fun theSigningSheetHoldsStillWhenTheCurrencyLands() {
        val waiting = SigningFixtures.build(SigningScreenState.CS59, strings)
        // As the live builder hands it over once the currency has committed:
        // no word about room — the row itself has to have kept it.
        val landed = SigningFixtures.build(SigningScreenState.CS60, strings).let { board ->
            board.copy(fee = (board.fee as FeeModel.OnChain).copy(worthRoom = false))
        }
        // …and landing in a currency whose worth fits on the one line after all.
        val short = landed.copy(fee = (landed.fee as FeeModel.OnChain).copy(value = "~0.000123 ETH · ≈$0.31"))
        var current by mutableStateOf<SigningScreenModel>(waiting)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = current, onConfirm = {}, modifier = Modifier.testTag(SHEET))
                }
            }
        }
        compose.waitForIdle()
        val before = sheetFrame()
        for ((name, next) in listOf("CNY" to landed, "a short worth" to short, "CNY again" to landed)) {
            // The same request: what the sheet holds across frames is held per request.
            current = next.copy(state = waiting.state, requestKey = waiting.requestKey)
            compose.waitForIdle()
            assertEquals("the fee's worth landing ($name) moved the sheet", before, sheetFrame())
        }
        compose.onAllNodesWithText("~0.000123 ETH · ≈CN¥2.24").fetchSemanticsNodes().let { assertEquals(1, it.size) }
    }

    // -- the balance sheet: its total and each network's worth --------------------

    @Test
    fun theBalanceSheetHoldsStillWhenTheCurrencyLands() {
        val waiting = SettingsFixtures.buildState(SettingsScreenState.SR3B, strings)
        val landed = SettingsFixtures.buildState(SettingsScreenState.SR3C, strings)
        val detail = waiting.balanceDetail
        val anchors = listOf(detail.title, detail.sectionPending, detail.sectionDone) + (detail.pending + detail.done).map { it.name }
        val rescue = WalletRescue(SettingsOverlay.BalanceDetail)
        assertStill(
            "balance sheet", anchors,
            walk<SettingsScreenModel>(waiting, listOf(landed), anchors) { WalletRescueSheet(rescue = rescue, model = it, onMove = {}) },
        )
        // The figures did land.
        assertEquals(1, compose.onAllNodesWithText(landed.balanceDetail.summary).fetchSemanticsNodes().size)
    }

    // -- the home: its total and each holding's worth -----------------------------

    @Test
    fun theHomeHoldsStillWhenTheCurrencyLands() {
        val waiting = WalletFixtures.buildMobileState(WalletScreenState.H13, strings)
        val landed = WalletFixtures.buildMobileState(WalletScreenState.H13B, strings)
        // What is on the first screen: the actions under the hero, the two
        // section titles, and the first holding (its amount, which is not fiat).
        val anchors = listOf(waiting.actions.receive, waiting.actions.send, waiting.activitySection.title, waiting.assetsSection.title, waiting.assetRows.first().balance)
        assertStill("home", anchors, walk<WalletHomeModel>(waiting, listOf(landed), anchors) { WalletScreen(model = it) })
    }

    private companion object {
        const val SHEET = "signing-sheet"
    }
}
