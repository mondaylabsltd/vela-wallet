package app.getvela.wallet

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.FOOTER_TAG
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent
import app.getvela.wallet.feature.signing.VERDICT_PLACE_TAG
import app.getvela.wallet.feature.signing.VERDICT_SHOWN_TAG
import app.getvela.wallet.feature.signing.VERDICT_WAITING_TAG
import app.getvela.wallet.feature.signing.gallery.SigningGalleryScreen
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * PR 3 on the drawn sheet — fix C (the confirm waits for the simulation's
 * verdict) and fix B (an unverified token is a direction, never a figure).
 *
 * **The confirm does not move.** While it waits, one line stands under it —
 * "Checking what this transaction does…" — in the place the confirm's note
 * already has; when the verdict lands the line goes and its room stays; when
 * the core's deadline passes instead, the could-not-check caution stands in
 * the verdict's place. Measured on the real layout, hosted as the live sheet
 * hosts it (bottom-anchored, as tall as its content): the confirm's box is
 * the same in every frame of one request, in both languages.
 *
 * **The boards a person can open** (CS69 held, CS70 waited out, CS68 two
 * unverified tokens) are mounted through the gallery screen itself — the
 * route `vela.startDestination signing-gallery` opens. The two wait boards
 * have their confirm at one place: the waited-out board keeps the room of
 * the line the held one says, as the live sheet does.
 *
 * Emulator only (never on the owner's phone, whose wallet
 * `connectedAndroidTest` would wipe):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SigningConfirmWaitTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * adb -s <emulator> logcat -d -s SigningConfirmWait:I      # the measurements
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SigningConfirmWaitTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private fun words(tag: String) =
        I18nRuntime { name -> context.assets.open("i18n/$name.json").use { it.readBytes() } }.apply { initialize(tag) }

    /** A node's own box in the root, as laid out — whatever an ancestor clips of it. */
    private fun SemanticsNode.laid(): Rect = Rect(positionInRoot, Size(size.width.toFloat(), size.height.toFloat()))

    private fun node(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode()

    private fun texts(under: String): List<String> = compose.onAllNodes(
        hasAnyAncestor(hasTestTag(under)) and SemanticsMatcher.keyIsDefined(SemanticsProperties.Text),
        useUnmergedTree = true,
    ).fetchSemanticsNodes().map { line -> line.config[SemanticsProperties.Text].joinToString("") { it.text } }

    // -- one request, frame by frame --------------------------------------------------

    @Test
    fun theConfirmDoesNotMoveWhileItWaitsForTheVerdict() = waits("zh")

    @Test
    fun theConfirmDoesNotMoveWhileItWaitsForTheVerdictInEnglish() = waits("en")

    private fun waits(language: String) {
        val words = words(language)
        val checkingLine = words.t("componentsUi.signing.confirmBlock.simChecking")
        val measuringLine = words.t("componentsUi.signing.confirmBlock.feeMeasuring")
        val couldNotCheck = words.t("componentsUi.signing.simUnavailableWarning")
        // The held board is the request; every other frame is the same
        // request with only its verdict's place and its gate moved on.
        val held = SigningFixtures.build(SigningScreenState.CS69, words)
        fun same(model: SigningScreenModel) = model.copy(state = held.state, requestKey = held.requestKey)
        fun withVerdict(state: SigningScreenState) = same(SigningFixtures.build(state, words))
        val steps = listOf(
            // As a live sheet opens: the fee still being worked out, the simulation out.
            "the fee measured, the simulation out" to held.copy(confirmBlockLine = measuringLine),
            "held for the verdict (the hold line)" to held,
            "the verdict landed: one balance row" to withVerdict(SigningScreenState.CS63),
            "held again (a board cannot; the line coming back)" to held,
            "the verdict landed: could not check" to withVerdict(SigningScreenState.CS58),
            "held" to held,
            "waited out: the caution in the verdict's place" to same(SigningFixtures.build(SigningScreenState.CS70, words)),
            "a late verdict takes the caution's place: two rows" to withVerdict(SigningScreenState.CS64),
        )
        var current by mutableStateOf(steps.first().second)
        var taps = 0
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides words) {
                VelaTheme(darkTheme = false) {
                    // As the modal sheet hosts it: at the bottom, as tall as its content.
                    Box(modifier = Modifier.fillMaxSize().testTag(SCREEN), contentAlignment = Alignment.BottomCenter) {
                        Column(modifier = Modifier.fillMaxWidth().background(VelaTheme.colors.bgRaised)) {
                            SigningSheetContent(model = current, onConfirm = { taps++ }, onClose = {}, modifier = Modifier.testTag(SHEET))
                        }
                    }
                }
            }
        }
        compose.waitForIdle()
        val confirm = node(CONFIRM_TAG).laid()
        val footer = node(FOOTER_TAG).laid()
        val sheet = node(SHEET).laid()
        val place = node(VERDICT_PLACE_TAG).laid()
        val log = StringBuilder("$language — screen ${node(SCREEN).laid().size}")
        for ((step, model) in steps) {
            current = model
            compose.waitForIdle()
            val now = node(CONFIRM_TAG).laid()
            log.append("\n$step: confirm ${now.left},${now.top}..${now.right},${now.bottom} footer ${node(FOOTER_TAG).laid().top}..${node(FOOTER_TAG).laid().bottom} sheet ${node(SHEET).laid().top}..${node(SHEET).laid().bottom} place ${node(VERDICT_PLACE_TAG).laid().top}..${node(VERDICT_PLACE_TAG).laid().bottom} enabled=${model.confirmEnabled} line=${model.confirmBlockLine}")
            assertEquals("the confirm moved ($step)\n$log", confirm, now)
            assertEquals("the footer moved ($step)\n$log", footer, node(FOOTER_TAG).laid())
            assertEquals("the sheet changed its size ($step)\n$log", sheet, node(SHEET).laid())
            assertEquals("the verdict's place moved or changed its size ($step)\n$log", place, node(VERDICT_PLACE_TAG).laid())
            assertEquals("the confirm is cut ($step)\n$log", now, node(CONFIRM_TAG).boundsInRoot)
            val button = compose.onNodeWithTag(CONFIRM_TAG).assertIsDisplayed()
            if (model.confirmEnabled) {
                button.assertIsEnabled()
                // The line's room is kept, unseen and unsaid.
                compose.onNodeWithText(checkingLine).assertDoesNotExist()
                compose.onNodeWithText(measuringLine).assertDoesNotExist()
            } else {
                button.assertIsNotEnabled()
                compose.onNodeWithText(model.confirmBlockLine!!).assertIsDisplayed()
            }
            // The verdict's place: its skeleton while checking, else what landed.
            val shown = model.blocks.filterIsInstance<SigningBlock.Held>().single().shown
            if (shown == null) {
                compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).assertExists()
                compose.onNodeWithTag(VERDICT_SHOWN_TAG, useUnmergedTree = true).assertDoesNotExist()
            } else {
                compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).assertDoesNotExist()
                compose.onNodeWithTag(VERDICT_SHOWN_TAG, useUnmergedTree = true).assertExists()
            }
        }
        android.util.Log.i(TAG, "one request — $log")

        // Held: the one line, a confirm that takes no tap, the skeleton.
        current = held
        compose.waitForIdle()
        assertFalse(held.confirmEnabled)
        compose.onNodeWithText(checkingLine).assertIsDisplayed()
        compose.onNodeWithTag(CONFIRM_TAG).performClick()
        compose.waitForIdle()
        assertEquals("a held confirm took a tap", 0, taps)
        // Waited out: the sentence, in the verdict's place — and nowhere under the confirm.
        current = same(SigningFixtures.build(SigningScreenState.CS70, words))
        compose.waitForIdle()
        assertEquals(listOf(couldNotCheck), texts(VERDICT_SHOWN_TAG))
        assertTrue("a line under an open confirm: ${texts(FOOTER_TAG)}", texts(FOOTER_TAG).none { it == checkingLine || it == couldNotCheck })
        // Open now: one tap confirms.
        compose.onNodeWithTag(CONFIRM_TAG).performClick()
        compose.waitForIdle()
        assertEquals(1, taps)
        // And the test can see a sheet move: on a screen the sheet fits,
        // without the place kept it is shorter.
        if (sheet.top > node(SCREEN).laid().top + 1f) {
            current = held.copy(blocks = held.blocks.filterNot { it is SigningBlock.Held })
            compose.waitForIdle()
            assertTrue("no room was being held\n$log", node(SHEET).laid().height < sheet.height)
        }
    }

    // -- the boards, as a person opens them -------------------------------------------

    /**
     * The gallery screen itself (`vela.startDestination signing-gallery`,
     * `vela.signingState <id>`), one board after another.
     *
     * The two wait boards have their confirm at ONE place — held (CS69, its
     * line said) and waited out (CS70, that line's room kept unseen): what a
     * person flipping between them sees is what the live sheet does. The
     * verdict boards that never said a line (CS57 checking, CS58 could not
     * check, CS63 one row) agree among themselves, and have none: on a
     * screen the sheet fills, the footer is the frame's last thing, so a
     * board with a line under its confirm has the confirm that one line
     * higher — as every held board has (CS45, CS46…). Their verdict's place
     * is the same in all five.
     *
     * CS69 is shut over its line and stays so — longer than the core's four
     * seconds: no timer runs on a board — and CS70 is open over the caution.
     */
    @Test
    fun theWaitBoardsOpenFromTheGalleryWithTheirConfirmAtOnePlace() {
        val words = words("zh")
        var state by mutableStateOf("CS57")
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides words) {
                // A gallery opened on [state], as the launch extra opens it.
                key(state) { SigningGalleryScreen(systemDarkTheme = false, initialState = state) }
            }
        }
        val log = StringBuilder("the gallery")
        val confirms = LinkedHashMap<String, Rect>()
        val places = LinkedHashMap<String, Rect>()
        for (board in listOf("CS57", "CS58", "CS63", "CS69", "CS70")) {
            state = board
            compose.waitForIdle()
            val confirm = node(CONFIRM_TAG)
            compose.onNodeWithTag(CONFIRM_TAG).assertIsDisplayed()
            assertEquals("$board: the confirm is cut", confirm.laid(), confirm.boundsInRoot)
            confirms[board] = confirm.laid()
            places[board] = node(VERDICT_PLACE_TAG).laid()
            log.append("\n$board: confirm ${confirm.laid().left},${confirm.laid().top}..${confirm.laid().right},${confirm.laid().bottom} footer ${node(FOOTER_TAG).laid().top}..${node(FOOTER_TAG).laid().bottom} place ${node(VERDICT_PLACE_TAG).laid().top}..${node(VERDICT_PLACE_TAG).laid().bottom} said under the confirm ${texts(FOOTER_TAG).drop(1)}")
        }
        android.util.Log.i(TAG, log.toString())
        // Held and waited out: one place.
        assertEquals("the confirm is not where it was while it waited\n$log", confirms.getValue("CS69"), confirms.getValue("CS70"))
        // The boards with no line: one place among themselves.
        for (board in listOf("CS58", "CS63")) assertEquals("$board's confirm is not where CS57's is\n$log", confirms.getValue("CS57"), confirms.getValue(board))
        // Never lower than theirs, and by no more than the footer's one line.
        val lift = confirms.getValue("CS57").top - confirms.getValue("CS69").top
        assertTrue("the wait boards' confirm is BELOW the others'\n$log", lift >= 0f)
        assertEquals("only up and down\n$log", confirms.getValue("CS57").left to confirms.getValue("CS57").right, confirms.getValue("CS69").left to confirms.getValue("CS69").right)
        // The verdict's place: the same box on all five.
        for ((board, place) in places) assertEquals("$board's verdict place is not CS57's\n$log", places.getValue("CS57"), place)

        // CS69: held, and it stays held.
        state = "CS69"
        compose.waitForIdle()
        val checkingLine = words.t("componentsUi.signing.confirmBlock.simChecking")
        compose.onNodeWithTag(CONFIRM_TAG).assertIsNotEnabled()
        compose.onNodeWithText(checkingLine).assertIsDisplayed()
        compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).assertExists()
        Thread.sleep(4_600)
        compose.waitForIdle()
        compose.onNodeWithTag(CONFIRM_TAG).assertIsNotEnabled()
        compose.onNodeWithText(checkingLine).assertIsDisplayed()
        compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).assertExists()

        // CS70: open, no line said (its room is unseen), the caution where "checking" was.
        state = "CS70"
        compose.waitForIdle()
        compose.onNodeWithTag(CONFIRM_TAG).assertIsEnabled()
        compose.onNodeWithText(checkingLine).assertDoesNotExist()
        assertEquals("nothing is said under an open confirm", 1, texts(FOOTER_TAG).size)
        assertEquals(listOf(words.t("componentsUi.signing.simUnavailableWarning")), texts(VERDICT_SHOWN_TAG))
    }

    /**
     * Fix B, drawn (CS68, from the gallery): one unverified token leaving and
     * one arriving — "−" and "+" beside the label, the warning under them,
     * and not one digit anywhere in the verdict. This sheet printed 「未验证代币
     * +5,000,000,000,000,000,000,000.00」.
     */
    @Test
    fun anUnverifiedTokensRowIsDrawnAsItsDirectionAndNoFigure() {
        for (language in listOf("zh", "en")) {
            val words = words(language)
            val label = words.t("componentsUi.signing.balanceUnverifiedToken")
            val expected = listOf(
                words.t("componentsUi.signing.balanceChangesTitle"),
                label, "−",
                label, "+",
                words.t("componentsUi.signing.unverifiedWarning"),
            )
            assertEquals(expected, drawnVerdict(language, "CS68"))
            assertTrue("a digit in the verdict: $expected", expected.none { line -> line.any(Char::isDigit) })
            // The tall boards with one such row among verified ones: its row is the sign alone.
            for (board in listOf("CS66", "CS67")) {
                val lines = drawnVerdict(language, board)
                val at = lines.indexOf(label)
                assertTrue("$board draws no unverified row: $lines", at >= 0)
                assertEquals("$board: $lines", "+", lines[at + 1])
                assertTrue("$board: the figure is on the sheet: $lines", lines.none { it.filter(Char::isDigit).contains("5000000") })
            }
        }
    }

    private var mounted: ((String, String) -> Unit)? = null

    /** The texts in the verdict's place of [board], opened from the gallery in [language]. */
    private fun drawnVerdict(language: String, board: String): List<String> {
        val show = mounted ?: run {
            var strings by mutableStateOf(words(language))
            var state by mutableStateOf(board)
            compose.setContent {
                CompositionLocalProvider(LocalVelaStrings provides strings) {
                    key(state, strings) { SigningGalleryScreen(systemDarkTheme = false, initialState = state) }
                }
            }
            val set: (String, String) -> Unit = { tag, name ->
                strings = words(tag)
                state = name
            }
            mounted = set
            set
        }
        show(language, board)
        compose.waitForIdle()
        return texts(VERDICT_SHOWN_TAG)
    }

    private companion object {
        const val TAG = "SigningConfirmWait"
        const val SHEET = "signing-sheet"
        const val SCREEN = "signing-screen"
    }
}
