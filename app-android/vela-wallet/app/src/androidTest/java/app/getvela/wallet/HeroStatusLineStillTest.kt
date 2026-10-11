package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.foundation.layout.requiredWidth
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.unit.dp
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_STAMP_TAG
import app.getvela.wallet.feature.wallet.components.BALANCE_REFRESH_TEST_TAG
import app.getvela.wallet.feature.wallet.components.BALANCE_STATE_LINE_TAG
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

        val anchors = listOf(BALANCE_REFRESH_TEST_TAG, plain.actions.receive, plain.actions.send, plain.activitySection.title)
        fun places() = anchors.map { anchor ->
            // The refresh control by its tag (its words are on screen only while the line has nothing else to say).
            val node = if (anchor == BALANCE_REFRESH_TEST_TAG) {
                compose.onNodeWithTag(anchor, useUnmergedTree = true).fetchSemanticsNode()
            } else {
                compose.onAllNodesWithText(anchor, useUnmergedTree = true).fetchSemanticsNodes().first()
            }
            node.boundsInRoot.let { it.top to it.bottom }
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

    /**
     * PR 3 final note F16: the line under the total is ONE line, always. A
     * sentence longer than the line — "Al momento non riusciamo a caricare
     * l'elenco dei token di Tempo" on a narrow phone, and this app's own
     * "Something went wrong inside Vela. If it keeps happening, reopen the
     * app." on any — shrinks to no less than 85 %, then ends in an ellipsis.
     * It wrapped to a second line, which grew the line's kept place and
     * pushed the refresh control, Receive and Send down. The whole sentence
     * is the line's content description.
     */
    @Test
    fun aSentenceLongerThanTheLineIsOneLineWithAnEllipsisAndMovesNothing() {
        val measured = StringBuilder()
        var current by mutableStateOf<WalletHomeModel?>(null)
        var width by mutableStateOf(320)
        var words by mutableStateOf(strings)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides words) {
                VelaTheme(darkTheme = false) {
                    androidx.compose.foundation.layout.Box(Modifier.requiredWidth(width.dp)) { current?.let { WalletScreen(model = it) } }
                }
            }
        }
        fun bounds(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot.let { it.top to it.bottom }
        // H14: Tempo's token list could not be loaded (the real machine); H11: a read that failed inside Vela.
        for ((locale, state) in listOf("it" to WalletScreenState.H14, "es-MX" to WalletScreenState.H14, "en" to WalletScreenState.H11, "ru" to WalletScreenState.H11)) {
            val localized = runtime(locale)
            for (dp in listOf(320, 360)) {
                for (scale in listOf(1f, 1.35f)) {
                    val plain = WalletFixtures.buildMobileState(WalletScreenState.H1, localized).copy(textScale = scale)
                    val long = plain.copy(balance = WalletFixtures.buildMobileState(state, localized).balance.copy(refresh = plain.balance.refresh))
                    val sentence = long.balance.status!!.text
                    // The sentence's first words alone, which fit: what they measure at full size.
                    val probe = sentence.take(PROBE)
                    val short = plain.copy(balance = plain.balance.copy(status = long.balance.status!!.copy(text = probe)))
                    words = localized
                    width = dp
                    current = plain
                    compose.waitForIdle()
                    val before = listOf(bounds(BALANCE_STATUS_PLACE_TAG), bounds(BALANCE_REFRESH_TEST_TAG))
                    current = short
                    compose.waitForIdle()
                    val whole = layoutOf(probe)
                    current = long
                    compose.waitForIdle()
                    val after = listOf(bounds(BALANCE_STATUS_PLACE_TAG), bounds(BALANCE_REFRESH_TEST_TAG))
                    // The full sentence is what a screen reader says…
                    val layout = layoutOf(sentence)
                    // The same words' advance in both: how far the font shrank.
                    val at = layout.getHorizontalPosition(PROBE - 1, true)
                    val full = whole.getHorizontalPosition(PROBE - 1, true)
                    val shrunk = at / full
                    measured.append("\n$locale ${dp}dp ×$scale \"$sentence\": lines ${layout.lineCount}, ellipsis ${layout.isLineEllipsized(0)}, font ×${"%.3f".format(shrunk)} ($at of $full px), place $before -> $after")
                    // …and the line itself is one line that never grows its place.
                    assertEquals("one line\n$measured", 1, layout.lineCount)
                    assertEquals("the long sentence moved the page or grew its place\n$measured", before, after)
                    assertTrue("never smaller than 85 %\n$measured", shrunk >= 0.85f - ROUNDING)
                    // Shrunk as far as it goes before a word is cut.
                    if (layout.isLineEllipsized(0)) assertTrue("cut only after shrinking\n$measured", shrunk <= 0.85f + ROUNDING)
                }
            }
        }
        android.util.Log.i("HeroStatusLineStill", measured.toString())
        // The check is not vacuous: the longest sentence at the narrowest width IS cut.
        assertTrue(measured.toString(), measured.contains("ellipsis true"))
    }

    /**
     * The 2026-10-11 phone walk, item 4: on a cold start a lone ⟳ sat on a
     * line of its own under 「● 正在检查…」, and once the read settled an empty
     * line stood between the figure and 「⟳ 上次更新 · 刚刚」 — the status had
     * one row and the refresh control another.
     *
     * They are ONE line now, the control at its end. One home, one
     * composition, its hero walked through every state it has — the first
     * read out (checking), read (updated), a refresh the person asked for
     * (updating), a network that failed, every network out of reach
     * (offline), a fault inside the app, the empty wallet's live line, and
     * the balance hidden: the figure, the line, the control at its end and
     * the first row under the hero are where they were, and the line and the
     * control are the size they were. The control is INSIDE the line's own
     * row in every state — never a row of its own.
     */
    @Test
    fun theStatusAndTheRefreshAreOneLineOfOneHeightInEveryState() {
        val zh = runtime("zh")
        fun board(state: WalletScreenState) = WalletFixtures.buildMobileState(state, zh)
        val read = board(WalletScreenState.H1)
        // The same home with only its hero changed — what a session does.
        fun hero(state: WalletScreenState, refresh: app.getvela.wallet.feature.wallet.BalanceRefreshModel? = read.balance.refresh) =
            read.copy(balance = board(state).balance.copy(refresh = refresh))
        val never = WalletFixtures.refresh(zh, read = false)
        val states = listOf(
            "checking (the first read is out)" to hero(WalletScreenState.H15, never),
            "updated" to read,
            "still updating" to hero(WalletScreenState.H6),
            "a network failed" to hero(WalletScreenState.H9),
            "offline (every network out of reach)" to hero(WalletScreenState.H17),
            "a fault inside the app" to hero(WalletScreenState.H11),
            "live (an empty wallet)" to hero(WalletScreenState.H16),
            "hidden" to hero(WalletScreenState.H10),
            "updated, again" to read,
            // Last: the screen holds a spin it was shown for 650 ms of the device's own clock, which a test's does not run.
            "updating (the person's refresh)" to read.copy(balance = read.balance.copy(refresh = WalletFixtures.refresh(zh, refreshing = true))),
        )
        assertEquals("the walk's cold start says Checking…", zh.t("componentsUi.funding.checking"), states[0].second.balance.checkingText)
        assertEquals(BalanceStateKind.Hidden, states.first { it.first == "hidden" }.second.balance.state)

        var current by mutableStateOf<WalletHomeModel>(states.first().second)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides zh) {
                VelaTheme(darkTheme = false) { WalletScreen(model = current) }
            }
        }
        fun box(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        data class Hero(val label: Float, val line: Pair<Float, Float>, val place: Pair<Float, Float>, val control: List<Float>, val under: Float, val section: Float) {
            override fun toString() = "label top $label · line ${line.first}..${line.second} (${line.second - line.first} px) · control ${control[1]}..${control[3]} at x ${control[0]}..${control[2]} · first row under the hero $under · next section $section"
        }
        fun hero(model: WalletHomeModel): Hero {
            val line = box(BALANCE_STATE_LINE_TAG)
            val place = box(BALANCE_STATUS_PLACE_TAG)
            val control = box(BALANCE_REFRESH_TEST_TAG)
            fun topOf(text: String) = compose.onAllNodesWithText(text, useUnmergedTree = true).fetchSemanticsNodes().first().boundsInRoot.top
            return Hero(
                // The hero's first thing: its label 「总余额 · …」 — every state has one.
                label = compose.onAllNodesWithText(model.balance.label, substring = true, useUnmergedTree = true).fetchSemanticsNodes().first().boundsInRoot.top,
                line = line.top to line.bottom,
                place = place.top to place.bottom,
                control = listOf(control.left, control.top, control.right, control.bottom),
                under = topOf(model.actions.receive),
                section = topOf(model.activitySection.title),
            )
        }
        compose.waitForIdle()
        val first = hero(states.first().second)
        val measured = StringBuilder()
        for ((name, model) in states) {
            current = model
            compose.waitForIdle()
            val now = hero(model)
            // Evidence for the review: the screen as measured, in the app's cache.
            InstrumentationRegistry.getInstrumentation().let { run ->
                run.uiAutomation.takeScreenshot()?.let { shot ->
                    val file = name.replace(Regex("[^A-Za-z0-9]+"), "-").trim('-')
                    java.io.File(run.targetContext.cacheDir, "hero-line-$file.png").outputStream().use { shot.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
                }
            }
            measured.append("\n$name: $now")
            assertEquals("the hero moved or changed size at \"$name\"$measured", first, now)
            // One line: the control stands inside the line's own row, beside the status's place — not under it.
            assertTrue("the control is not on the status's line ($name)$measured", now.control[1] >= now.line.first && now.control[3] <= now.line.second)
            assertTrue("the control is under the status, not beside it ($name)$measured", now.control[0] >= box(BALANCE_STATUS_PLACE_TAG).right - 1)
            // What the line says: the status, the first read, the live line — else how fresh the figure is.
            val says = model.balance.status?.text ?: model.balance.checkingText ?: model.balance.liveText
            val stamp = compose.onAllNodes(androidx.compose.ui.test.hasTestTag(BALANCE_REFRESH_STAMP_TAG), useUnmergedTree = true).fetchSemanticsNodes()
            assertEquals("the control's words stand in the line exactly when nothing else does ($name: 「$says」)", if (says == null) 1 else 0, stamp.size)
        }
        android.util.Log.i("pr4-hero", "the hero's line, zh, ${compose.density.density}x:$measured")
        // The hero is the figure's label down to the line's end, and the page starts a fixed step below it.
        android.util.Log.i("pr4-hero", "hero ${first.line.second - first.label} px tall (label ${first.label} → line end ${first.line.second}); Receive at ${first.under}")
    }

    /** The laid-out text of the status line that says [sentence] (its content description). */
    private fun layoutOf(sentence: String): androidx.compose.ui.text.TextLayoutResult {
        val node = compose.onNodeWithContentDescription(sentence, useUnmergedTree = true).fetchSemanticsNode()
        val layouts = mutableListOf<androidx.compose.ui.text.TextLayoutResult>()
        node.config[androidx.compose.ui.semantics.SemanticsActions.GetTextLayoutResult].action!!.invoke(layouts)
        return layouts.first()
    }

    private fun runtime(locale: String) = InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
        I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize(locale) }
    }

    private companion object {
        /** How many of the sentence's first characters are measured for the font's size. */
        const val PROBE = 14

        /** The font steps down by a quarter of an sp, and positions are whole pixels. */
        const val ROUNDING = 0.03f
    }
}
