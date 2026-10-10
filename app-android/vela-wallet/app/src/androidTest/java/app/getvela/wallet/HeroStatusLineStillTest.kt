package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_TEST_TAG
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

    /**
     * PR 3 final note F19: an empty wallet around its first read, as the REAL
     * balance machine says it (H15 / H16 / H17). "Checking…" while the read
     * is out over last session's cached zero, "Live · listening for payments"
     * once every network answered, "Can't reach 3 networks right now" when
     * three did not — one place, three lines, and nothing under it moves. A
     * cached zero drew "Live" over a wallet nothing had read, then swapped it
     * for "Can't reach".
     */
    @Test
    fun checkingLiveAndCantReachStandInTheOnePlaceAndMoveNothing() {
        val checking = board(WalletScreenState.H15)
        val live = board(WalletScreenState.H16)
        val missing = board(WalletScreenState.H17)
        val saysChecking = strings.t("componentsUi.funding.checking")
        val saysLive = strings.t("home.liveIndicator")
        val saysMissing = "Can't reach 3 networks right now"
        assertEquals(saysChecking, checking.balance.checkingText)
        assertEquals(null, checking.balance.liveText)
        assertEquals(saysLive, live.balance.liveText)
        assertEquals(saysMissing, missing.balance.status?.text)
        assertEquals(null, missing.balance.liveText)

        fun top(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.let { it.top to it.bottom }
        fun actions() = listOf(checking.actions.receive, checking.actions.send).map { text ->
            compose.onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().first().boundsInRoot.let { it.top to it.bottom }
        }
        fun measure() = Triple(top(BALANCE_STATUS_PLACE_TAG), top(BALANCE_REFRESH_TEST_TAG), actions())

        var current by mutableStateOf<WalletHomeModel>(checking)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { WalletScreen(model = current) }
            }
        }
        compose.waitForIdle()
        val before = measure()
        val measured = StringBuilder("checking: $before")
        compose.onNodeWithText(saysChecking).assertExists()
        compose.onNodeWithText(saysLive).assertDoesNotExist()
        for ((name, next, said) in listOf(
            Triple("live", live, saysLive),
            Triple("can't reach", missing, saysMissing),
            Triple("checking again", checking, saysChecking),
            Triple("can't reach, straight from checking", missing, saysMissing),
            Triple("live, last", live, saysLive),
        )) {
            current = next
            compose.waitForIdle()
            val after = measure()
            measured.append("\n$name: $after")
            assertEquals("the hero's line ($name) moved the page or changed its place\n$measured", before, after)
            // Exactly one of the three is said at a time.
            for (line in listOf(saysChecking, saysLive, saysMissing)) {
                val nodes = compose.onAllNodesWithText(line, useUnmergedTree = true).fetchSemanticsNodes()
                assertEquals("\"$line\" while the line says \"$said\"", if (line == said) 1 else 0, nodes.size)
            }
        }
        android.util.Log.i("HeroStatusLineStill", measured.toString())
    }
}
