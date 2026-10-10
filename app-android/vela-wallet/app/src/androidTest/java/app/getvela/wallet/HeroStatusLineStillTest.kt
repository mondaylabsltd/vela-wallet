package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletHomeModel
import app.getvela.wallet.feature.wallet.WalletScreen
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.components.BALANCE_STATUS_PLACE_TAG
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The integration's note 26b: the line under the home's total keeps its
 * place. It comes and goes with the wallet's state — "Some balances are still
 * updating." on a cold start with a cached total, "Can't reach 3 networks
 * right now" when networks drop, the empty wallet's live line — and each
 * arrival pushed the refresh control, Receive, Send and the page under them
 * down by its height (88 px at 440 dpi, 32 dp), each departure pulled them up.
 *
 * One home, one composition, its hero walked through every state the line
 * has: none → still updating → can't reach → none → the empty wallet's live
 * line → none. The refresh control, the actions and the first section must
 * not move, and the line's own place must not change size.
 *
 * Emulator only:
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.HeroStatusLineStillTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * ```
 */
@RunWith(AndroidJUnit4::class)
class HeroStatusLineStillTest {

    @get:Rule
    val compose = createComposeRule()

    private val strings = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }
            .apply { initialize("en") }
    }

    private fun board(state: WalletScreenState) = WalletFixtures.buildMobileState(state, strings)

    @Test
    fun theLineUnderTheTotalComingAndGoingMovesNothing() {
        val plain = board(WalletScreenState.H1)
        // The same home with only its hero's line changed — what a session does.
        fun with(state: WalletScreenState) = plain.copy(balance = board(state).balance.copy(refresh = plain.balance.refresh))
        val updating = with(WalletScreenState.H6)
        val unreachable = with(WalletScreenState.H9)
        // The empty wallet's live line, in the same place (its figure is $0.00).
        val live = plain.copy(balance = board(WalletScreenState.H2).balance.copy(refresh = plain.balance.refresh))
        assertEquals(null, plain.balance.status)
        assertTrue(updating.balance.status != null && unreachable.balance.status != null)
        assertEquals(BalanceStateKind.ZeroLive, live.balance.state)

        val anchors = listOf(plain.balance.refresh!!.updated!!, plain.actions.receive, plain.actions.send, plain.activitySection.title)
        fun places() = anchors.map { text ->
            compose.onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().first().boundsInRoot.let { it.top to it.bottom }
        }
        fun place() = compose.onNodeWithTag(BALANCE_STATUS_PLACE_TAG, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot

        var current by mutableStateOf<WalletHomeModel>(plain)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { WalletScreen(model = current) }
            }
        }
        compose.waitForIdle()
        val before = places()
        val room = place()
        val measured = StringBuilder("no line: ${anchors.zip(before)} place ${room.top}..${room.bottom}")
        for ((name, next) in listOf(
            "still updating" to updating,
            "can't reach" to unreachable,
            "no line again" to plain,
            "the live line" to live,
            "no line, last" to plain,
        )) {
            current = next
            compose.waitForIdle()
            val after = places()
            measured.append("\n$name: ${anchors.zip(after)}")
            assertEquals("the hero's line ($name) moved the page\n$measured", before, after)
            assertEquals("…or changed the size of its own place ($name)", room.top to room.bottom, place().let { it.top to it.bottom })
        }
        android.util.Log.i("HeroStatusLineStill", measured.toString())
        // The line is said when it is there, and only then.
        current = unreachable
        compose.waitForIdle()
        compose.onNodeWithText(unreachable.balance.status!!.text).assertExists()
        current = plain
        compose.waitForIdle()
        compose.onNodeWithText(unreachable.balance.status!!.text).assertDoesNotExist()
    }
}
