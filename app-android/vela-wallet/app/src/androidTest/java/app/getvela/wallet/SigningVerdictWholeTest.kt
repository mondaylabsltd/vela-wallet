package app.getvela.wallet

import android.graphics.Bitmap
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsNode
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasAnyAncestor
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import app.getvela.wallet.feature.signing.BODY_TAG
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.flows.FeeSpeedModel
import app.getvela.wallet.feature.signing.FOOTER_TAG
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.HEADER_TAG
import app.getvela.wallet.feature.signing.OPEN_SIGNER_TAG
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheet
import app.getvela.wallet.feature.signing.SigningSheetContent
import app.getvela.wallet.feature.signing.VERDICT_PLACE_TAG
import app.getvela.wallet.feature.signing.VERDICT_SHOWN_TAG
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * The device round, item 1 — SECURITY: the simulation's verdict is never
 * hidden under a fold.
 *
 * The verdict is the one part of the signing sheet a site cannot write. Its
 * place used to be a fixed height with a scroll of its own, a fade and a
 * "more" mark, so a third balance row or the warning under an unverified
 * token sat partly below a fold — over a confirm that was live. Now:
 *
 * 1. the place is a MINIMUM height (the room kept from the first frame —
 *    `SigningSheetStillTest` measures that the usual verdicts move nothing);
 * 2. a taller verdict is shown WHOLE: the place is as tall as the verdict,
 *    with no scroll inside it and nothing clipped;
 * 3. when the sheet no longer fits the screen its BODY scrolls, and the
 *    confirm stays in a footer outside the scroll — fully on screen, at the
 *    same place with no verdict, a one-row verdict and a tall one;
 * 4. when the verdict lands, or grows, the body brings it into view — all of
 *    it when it fits the body's frame, else from its top;
 * 5. and the header stays at the top, outside the scroll: the ✕ is the one
 *    way to refuse, and a body that scrolls by itself to a tall verdict must
 *    not leave the confirm in sight and the refusal out of it.
 *
 * Measured on the real layout: [SigningSheetContent] hosted as the live sheet
 * hosts it (bottom-anchored, as tall as its content up to a cap — the modal
 * sheet's column) on a screen with room to spare, on this screen, on a short
 * one and on one lower than the verdict itself; and the modal [SigningSheet]
 * itself, photographed. The board is CS67: four balance rows, the last a
 * token nothing verified, and its warning.
 *
 * Emulator only (never on the owner's phone, whose wallet
 * `connectedAndroidTest` would wipe):
 *
 * ```bash
 * adb -s <emulator> shell am instrument -w -e class app.getvela.wallet.SigningVerdictWholeTest \
 *   app.getvela.wallet.test/androidx.test.runner.AndroidJUnitRunner
 * adb -s <emulator> logcat -d -s SigningVerdictWhole:I      # the measurements
 * adb -s <emulator> shell wm size 1080x1500                 # and again on a short screen; `wm size reset` after
 * ```
 */
@RunWith(AndroidJUnit4::class)
class SigningVerdictWholeTest {

    @get:Rule
    val compose = createComposeRule()

    private val context = InstrumentationRegistry.getInstrumentation().targetContext

    private fun words(tag: String) =
        I18nRuntime { name -> context.assets.open("i18n/$name.json").use { it.readBytes() } }.apply { initialize(tag) }

    /** The screens the sheet is laid out on. */
    private enum class Screen(val cap: Dp?, val scale: Float) {
        /** A screen with room for the whole sheet under the tallest verdict: nothing has to scroll. */
        Roomy(cap = null, scale = 0.45f),

        /** This device's own. */
        This(cap = null, scale = 1f),

        /** Too short for the sheet: the body scrolls. */
        Short(cap = 520.dp, scale = 1f),

        /** Lower than the tall verdict itself (a phone on its side): the verdict is shown from its top. */
        Low(cap = 330.dp, scale = 1f),
    }

    @Test
    fun aTallVerdictIsWholeAndTheConfirmDoesNotMoveOnARoomyScreen() = whole(Screen.Roomy, "zh")

    @Test
    fun aTallVerdictIsWholeAndTheConfirmDoesNotMoveOnThisScreen() = whole(Screen.This, "zh")

    @Test
    fun aTallVerdictIsWholeAndTheConfirmDoesNotMoveOnThisScreenInEnglish() = whole(Screen.This, "en")

    @Test
    fun onAShortScreenTheBodyScrollsToTheVerdictAndTheConfirmDoesNotMove() = whole(Screen.Short, "zh")

    @Test
    fun onAShortScreenTheBodyScrollsToTheVerdictAndTheConfirmDoesNotMoveInEnglish() = whole(Screen.Short, "en")

    @Test
    fun aVerdictTallerThanTheBodysFrameIsShownFromItsTop() = whole(Screen.Low, "en")

    /**
     * The sheet's content as the live sheet hosts it (`VelaModalSheet`): at
     * the bottom of the screen, as tall as its content, up to a cap.
     */
    @Composable
    private fun Hosted(screen: Screen, words: I18nRuntime, model: () -> SigningScreenModel) {
        CompositionLocalProvider(LocalVelaStrings provides words) {
            VelaTheme(darkTheme = false) {
                val device = LocalDensity.current
                // More room is more dp on the same glass.
                CompositionLocalProvider(LocalDensity provides Density(device.density * screen.scale, device.fontScale)) {
                    Box(modifier = Modifier.fillMaxSize().testTag(SCREEN), contentAlignment = Alignment.BottomCenter) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .then(screen.cap?.let { Modifier.heightIn(max = it) } ?: Modifier)
                                .background(VelaTheme.colors.bgRaised),
                        ) {
                            SigningSheetContent(model = model(), onConfirm = {}, onClose = {}, modifier = Modifier.testTag(SHEET))
                        }
                    }
                }
            }
        }
    }

    /** A node's own box in the root, as laid out — whatever an ancestor clips of it. */
    private fun SemanticsNode.laid(): Rect = Rect(positionInRoot, Size(size.width.toFloat(), size.height.toFloat()))

    private fun node(tag: String) = compose.onNodeWithTag(tag, useUnmergedTree = true).fetchSemanticsNode()

    /** The ✕'s words, as the live sheet says them (the boards leave them out). */
    private fun closeWords(words: I18nRuntime) = words.t(I18nKeys.Flow.CLOSE)

    /** A board with the live sheet's ✕: the one control that refuses. */
    private fun board(state: SigningScreenState, words: I18nRuntime) =
        SigningFixtures.build(state, words).copy(closeLabel = closeWords(words))

    /**
     * The header is pinned: the ✕ is displayed, whole, on the [screen] and in
     * the header — wherever the body is scrolled to — and nothing of the body
     * shows above the header's lower edge or under the footer's upper one.
     * Returns the ✕'s box.
     */
    private fun closeIsInSight(words: I18nRuntime, at: String): Rect {
        val close = compose.onNodeWithContentDescription(closeWords(words))
        close.assertIsDisplayed()
        val box = close.fetchSemanticsNode()
        val header = node(HEADER_TAG).laid()
        assertEquals("the ✕ is cut — $at", box.laid(), box.boundsInRoot)
        assertTrue("the ✕ is off the screen: ${box.laid()} in ${node(SCREEN).laid()} — $at", node(SCREEN).laid().contains(box.laid()))
        assertTrue("the ✕ is outside the header: ${box.laid()} in $header — $at", header.contains(box.laid()))
        assertEquals("the header is not the sheet's first thing — $at", node(SHEET).laid().top, header.top, 0.5f)
        val body = node(BODY_TAG).laid()
        assertTrue("the body starts above the header's end: $body under $header — $at", body.top >= header.bottom - 0.5f)
        // What is SEEN of the body's lines is inside the body's frame.
        val lines = compose.onAllNodes(
            hasAnyAncestor(hasTestTag(BODY_TAG)) and SemanticsMatcher.keyIsDefined(SemanticsProperties.Text),
            useUnmergedTree = true,
        ).fetchSemanticsNodes()
        for (line in lines) {
            val seen = line.boundsInRoot
            if (seen.height <= 0f || seen.width <= 0f) continue
            assertTrue("a line of the body shows outside its frame: $seen in $body — $at", seen.top >= body.top - 0.5f && seen.bottom <= body.bottom + 0.5f)
        }
        return box.laid()
    }

    /** How far the body can scroll, and how far it has. */
    private fun bodyScroll() = node(BODY_TAG).config[SemanticsProperties.VerticalScrollAxisRange].let { it.value() to it.maxValue() }

    private fun whole(screen: Screen, language: String) {
        val words = words(language)
        val waiting = board(SigningScreenState.CS57, words)
        // The same request: only its verdict arrived.
        fun landed(state: SigningScreenState) = board(state, words).copy(state = waiting.state, requestKey = waiting.requestKey)
        var current by mutableStateOf(waiting)
        compose.setContent { Hosted(screen, words) { current } }
        compose.waitForIdle()
        val log = StringBuilder("${screen.name} / $language — screen ${node(SCREEN).laid().size}, cap ${screen.cap ?: "the screen"}")

        // No verdict yet: where the confirm is, and how tall the place is kept.
        val confirm = node(CONFIRM_TAG).laid()
        val footer = node(FOOTER_TAG).laid()
        val least = node(VERDICT_PLACE_TAG).size.height
        val header = node(HEADER_TAG).laid()
        val close = closeIsInSight(words, "no verdict yet")
        log.append("\n\"Checking…\": sheet ${node(SHEET).size.height} header ${header.top}..${header.bottom} ✕ $close confirm ${confirm.top}..${confirm.bottom} place $least px body ${node(BODY_TAG).size.height} px scrolls ${bodyScroll()}")
        assertTrue("the confirm is not whole on the screen: $confirm in ${node(SCREEN).laid()}\n$log", node(SCREEN).laid().contains(confirm))

        fun check(kind: String, state: SigningScreenState, tall: Boolean) {
            current = landed(state)
            compose.waitForIdle()
            val place = node(VERDICT_PLACE_TAG)
            val shown = node(VERDICT_SHOWN_TAG)
            val body = node(BODY_TAG).laid()
            val (scrolled, range) = bodyScroll()
            log.append(
                "\n$kind: sheet ${node(SHEET).size.height} confirm ${node(CONFIRM_TAG).laid().top}..${node(CONFIRM_TAG).laid().bottom}" +
                    " place ${place.size.height} px = verdict ${shown.size.height} px (kept: $least)" +
                    " body ${body.height} px scrolled $scrolled of $range, place at ${place.laid().top}..${place.laid().bottom} in ${body.top}..${body.bottom}",
            )

            // 3. The confirm has not moved, and is whole on the screen.
            assertEquals("the confirm moved under $kind\n$log", confirm, node(CONFIRM_TAG).laid())
            assertEquals("the footer moved under $kind\n$log", footer, node(FOOTER_TAG).laid())
            compose.onNodeWithTag(CONFIRM_TAG).assertIsDisplayed()
            assertEquals("the confirm is cut\n$log", confirm, node(CONFIRM_TAG).boundsInRoot)
            // …in a footer the body ends above: nothing of the body is under it.
            assertTrue("the body runs under the footer\n$log", body.bottom <= footer.top + 0.5f)

            // 5. The ✕ is in sight, whatever the body scrolled to — and on a
            // sheet as tall as its screen it has not moved either.
            val closeNow = closeIsInSight(words, "$kind\n$log")
            log.append(", ✕ ${closeNow.top}..${closeNow.bottom}")
            if (screen.cap != null) {
                assertEquals("the ✕ moved under $kind\n$log", close, closeNow)
                assertEquals("the header moved under $kind\n$log", header, node(HEADER_TAG).laid())
            }

            // 1 / 2. The place is at least the room kept, and exactly a taller verdict's height.
            if (tall) {
                assertTrue("$kind is not taller than the room kept: the board proves nothing\n$log", shown.size.height > least)
                assertEquals("the place is not as tall as $kind\n$log", shown.size.height, place.size.height)
            } else {
                assertEquals("the usual verdict changed the place's height\n$log", least, place.size.height)
            }
            // No scroll inside the place — the body's is the only one in the sheet.
            val scrolls = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange), useUnmergedTree = true).fetchSemanticsNodes()
            assertEquals("a scroll besides the body's\n$log", listOf(BODY_TAG), scrolls.map { it.config.getOrNull(SemanticsProperties.TestTag) })

            // Every line of the verdict is laid out inside the place at its
            // full height: the title, each row's coin and figure, the warning.
            val block = (current.blocks.filterIsInstance<SigningBlock.Held>().single().shown as SigningBlock.Balances)
            val expected = listOf(block.title) + block.rows.flatMap { listOf(it.symbol, it.delta) } + listOfNotNull(block.note)
            val lines = compose.onAllNodes(
                hasAnyAncestor(hasTestTag(VERDICT_SHOWN_TAG)) and SemanticsMatcher.keyIsDefined(SemanticsProperties.Text),
                useUnmergedTree = true,
            ).fetchSemanticsNodes()
            assertEquals("the verdict's lines\n$log", expected, lines.map { line -> line.config[SemanticsProperties.Text].joinToString("") { it.text } })
            for (line in lines) {
                val text = line.config[SemanticsProperties.Text].joinToString("") { it.text }
                assertTrue("\"$text\" is laid out outside the place: ${line.laid()} in ${place.laid()}\n$log", place.laid().contains(line.laid()))
                val layouts = mutableListOf<TextLayoutResult>()
                line.config[SemanticsActions.GetTextLayoutResult].action?.invoke(layouts)
                // Every line of the text is drawn: none cut at the box's
                // height, none dropped past a line limit, none ellipsized.
                // (Not `hasVisualOverflow`: a text narrower than the width it
                // was offered reports a width overflow it does not have.)
                assertTrue(
                    "\"$text\" is cut: ${layouts.map { "${it.size} of ${it.multiParagraph.height} px, ${it.lineCount} lines" }}\n$log",
                    layouts.isNotEmpty() && layouts.none { layout ->
                        layout.didOverflowHeight || layout.multiParagraph.didExceedMaxLines || (0 until layout.lineCount).any(layout::isLineEllipsized)
                    },
                )
            }

            // 4. Landed: the body has brought it into view.
            if (place.laid().height <= body.height) {
                assertTrue("$kind fits the body's frame and is not all in view: ${place.laid()} in $body\n$log", body.contains(place.laid()))
                assertEquals("…so nothing of it is cut", place.laid(), place.boundsInRoot)
            } else {
                assertEquals("$kind is taller than the body's frame and is not shown from its top\n$log", body.top, place.laid().top, 1f)
            }

            // 3, the other half: when the sheet is taller than the screen the
            // body scrolls — and each line can be brought fully into view,
            // the confirm where it was.
            if (range > 0f) {
                for (text in expected) {
                    val line = compose.onNode(hasText(text) and hasAnyAncestor(hasTestTag(VERDICT_SHOWN_TAG)), useUnmergedTree = true)
                    line.performScrollTo()
                    compose.waitForIdle()
                    val at = line.fetchSemanticsNode()
                    assertEquals("\"$text\" cannot be brought fully into view\n$log", at.laid(), at.boundsInRoot)
                    assertTrue("\"$text\" is under the footer\n$log", at.laid().bottom <= footer.top + 0.5f)
                    assertTrue("\"$text\" is under the header\n$log", at.laid().top >= node(HEADER_TAG).laid().bottom - 0.5f)
                    assertEquals("scrolling the body moved the confirm\n$log", confirm, node(CONFIRM_TAG).laid())
                    assertEquals("scrolling the body moved the ✕\n$log", closeNow, closeIsInSight(words, "\"$text\" brought into view, $kind\n$log"))
                }
            }
        }

        check("one balance row", SigningScreenState.CS63, tall = false)
        check("FOUR rows and the unverified token's warning", SigningScreenState.CS67, tall = true)
        val (_, tallRange) = bodyScroll()
        check("three balance rows", SigningScreenState.CS65, tall = true)
        check("an unverified token and its warning", SigningScreenState.CS66, tall = true)
        // Back to a usual one: the place is its least again.
        check("two balance rows", SigningScreenState.CS64, tall = false)
        android.util.Log.i(TAG, log.toString())

        // The screens are what they claim to be, so each case above was the
        // one it is named for.
        when (screen) {
            Screen.Roomy -> assertEquals("the roomy screen had to scroll under the tall verdict\n$log", 0f, tallRange)
            Screen.This -> Unit
            Screen.Short, Screen.Low -> assertTrue("the short screen did not have to scroll\n$log", tallRange > 0f)
        }
    }

    /**
     * Device-found on the iPhone in this round: a live "Send dust" sheet moved
     * its confirm 70 pt in its first four seconds — not for the verdict, for
     * lines arriving late INSIDE THE FEE BLOCK: a line's room reserved under
     * the fee while it still said "Estimating…", then the speed row's
     * two-line free-upgrade note, then the red "no coin can pay this fee".
     * This sheet draws the same lines. The confirm and its note are in the
     * footer, at the bottom edge of the sheet and outside the scroll: nothing
     * the body holds — a verdict, a fee line, a speed note — can move them.
     * On a bottom-anchored sheet the top edge may rise; the confirm may not.
     */
    @Test
    fun aFeeBlockThatGrowsAfterTheSheetOpensDoesNotMoveTheConfirm() = feeGrows(Screen.This, "zh")

    @Test
    fun aFeeBlockThatGrowsAfterTheSheetOpensDoesNotMoveTheConfirmInEnglish() = feeGrows(Screen.This, "en")

    @Test
    fun aFeeBlockThatGrowsOnAShortScreenDoesNotMoveTheConfirm() = feeGrows(Screen.Short, "zh")

    /** The fee block's late lines, in the order the phone showed them, over one request. */
    private fun feeSteps(words: I18nRuntime): List<Pair<String, SigningScreenModel>> {
        val base = board(SigningScreenState.CS57, words)
        val landed = base.fee as FeeModel.OnChain
        val measuring = words.t("componentsUi.signing.confirmBlock.feeMeasuring")
        val noCoin = words.t(I18nKeys.Flows.FEE_NO_COIN_PAYS)
        val estimating = landed.copy(value = words.t("componentsUi.gas.estimating"), speed = null, warning = null, refreshing = true, measuring = true, reserve = null)
        // The free upgrade: the tier in force is the fastest, and the row says why.
        val speed = FeeSpeedModel(
            label = words.t(I18nKeys.Flows.FEE_SPEED_LABEL),
            value = words.t(I18nKeys.Flows.GAS_TIER_FAST),
            open = false,
            onceNote = words.t(I18nKeys.Flows.FEE_SPEED_ONCE),
            freeNote = words.t(I18nKeys.Flows.FEE_SPEED_FREE),
            gasPriceLabel = words.t(I18nKeys.Flows.GAS_PRICE_LABEL),
            gasPriceLine = false,
            options = emptyList(),
        )
        fun shut(fee: FeeModel.OnChain, note: String?) = base.copy(fee = fee, confirmEnabled = false, confirmBlockLine = note)
        return listOf(
            "opened: \"Estimating…\", no line under the fee" to shut(estimating, measuring),
            "a line's room reserved under the fee, still estimating" to shut(estimating.copy(reserve = noCoin), measuring),
            "the fee landed, with the speed row and its free-upgrade note" to
                base.copy(fee = landed.copy(speed = speed, warning = null, refreshing = false, measuring = false), confirmEnabled = true, confirmBlockLine = null),
            "…and no coin can pay it" to shut(landed.copy(speed = speed, warning = noCoin, refreshing = false, measuring = false), null),
            "the speed control opened" to shut(landed.copy(speed = speed.copy(open = true), warning = noCoin, refreshing = false, measuring = false), null),
        )
    }

    private fun feeGrows(screen: Screen, language: String) {
        val words = words(language)
        val steps = feeSteps(words)
        var current by mutableStateOf(steps.first().second)
        compose.setContent { Hosted(screen, words) { current } }
        compose.waitForIdle()
        val confirm = node(CONFIRM_TAG).laid()
        val footer = node(FOOTER_TAG).laid()
        val log = StringBuilder("${screen.name} / $language — screen ${node(SCREEN).laid().size}, cap ${screen.cap ?: "the screen"}")
        val tops = mutableListOf<Float>()
        val bodies = mutableListOf<Float>()
        for ((step, model) in steps) {
            current = model
            compose.waitForIdle()
            val sheet = node(SHEET).laid()
            // What the body holds, laid out: its frame when it fits, more when it scrolls.
            val held = node(BODY_TAG).size.height + bodyScroll().second
            log.append("\n$step: sheet ${sheet.top}..${sheet.bottom} (${sheet.height} px), body holds $held px, confirm ${node(CONFIRM_TAG).laid().top}..${node(CONFIRM_TAG).laid().bottom}, scrolls ${bodyScroll()}")
            assertEquals("the confirm moved ($step)\n$log", confirm, node(CONFIRM_TAG).laid())
            assertEquals("the footer moved ($step)\n$log", footer, node(FOOTER_TAG).laid())
            assertEquals("the confirm is cut ($step)\n$log", confirm, node(CONFIRM_TAG).boundsInRoot)
            // The ✕ rides the sheet's top edge — which may rise — and stays in sight.
            val close = closeIsInSight(words, "$step\n$log")
            log.append(", ✕ ${close.top}..${close.bottom}")
            tops += sheet.top
            bodies += held
        }
        android.util.Log.i(TAG, "the fee block grows — $log")
        // The fee block did grow — the sheet's top rose, or its body got more
        // to scroll — so this test would have seen a confirm that follows it.
        assertTrue("the fee block never grew: this measured nothing\n$log", bodies.last() > bodies.first() + 1f)
        assertTrue("the sheet's top went DOWN as the fee block grew\n$log", tops.zipWithNext().all { (before, after) -> after <= before + 0.5f })
    }

    /**
     * Every board of the canon through the same two-part layout, on this
     * screen and on a short one: a board that has a confirm has it in the
     * footer — whole, on screen, the sheet's last thing — however tall its
     * body; the boards with none (a refusal, a receipt) draw none; and the
     * two cards that stand in the confirm's place — the hand-off to the
     * signing page (spec 102) and the wait on it (spec 071) — are still
     * there, in the body, reachable by scrolling it.
     */
    @Test
    fun everyBoardKeepsItsConfirmWholeOnThisScreen() = everyBoard(Screen.This)

    @Test
    fun everyBoardKeepsItsConfirmWholeOnAShortScreen() = everyBoard(Screen.Short)

    private fun everyBoard(screen: Screen) {
        val words = words("zh")
        var current by mutableStateOf(board(SigningScreenState.CS1, words))
        compose.setContent { Hosted(screen, words) { current } }
        var confirms = 0
        var handoffs = 0
        var waits = 0
        for (state in SigningScreenState.entries) {
            val model = board(state, words)
            current = model
            compose.waitForIdle()
            val sheet = node(SHEET).laid()
            val at = "$state on ${screen.name}: sheet $sheet in ${node(SCREEN).laid()}"
            assertTrue("the sheet is not on the screen — $at", node(SCREEN).laid().contains(sheet))
            assertEquals("the sheet is not at the bottom — $at", node(SCREEN).laid().bottom, sheet.bottom, 0.5f)
            // Every form of the sheet keeps its header, and its ✕ in sight.
            val close = closeIsInSight(words, at)
            val receipt = model.receipt != null && model.trustedSignerWait == null
            val waiting = model.trustedSignerWait != null
            val handoff = model.handoff != null && model.confirmAction != null
            if (model.confirmAction != null && !receipt && !waiting && !handoff) {
                confirms++
                val confirm = node(CONFIRM_TAG)
                val footer = node(FOOTER_TAG).laid()
                compose.onNodeWithTag(CONFIRM_TAG).assertIsDisplayed()
                assertEquals("the confirm is cut — $at", confirm.laid(), confirm.boundsInRoot)
                assertTrue("the confirm is outside its footer — $at", footer.contains(confirm.laid()))
                assertEquals("the footer is not the sheet's last thing — $at", sheet.bottom, footer.bottom, 0.5f)
                assertTrue("the body runs under the footer — $at", node(BODY_TAG).laid().bottom <= footer.top + 0.5f)
            } else {
                compose.onNodeWithTag(CONFIRM_TAG).assertDoesNotExist()
                compose.onNodeWithTag(FOOTER_TAG).assertDoesNotExist()
                // Nothing under the body: it is the rest of the sheet.
                assertEquals("the body does not end the sheet — $at", sheet.bottom, node(BODY_TAG).laid().bottom, 0.5f)
            }
            if (waiting) {
                waits++
                val cancel = compose.onNode(hasText(model.trustedSignerWait!!.cancel) and hasAnyAncestor(hasTestTag(BODY_TAG)), useUnmergedTree = true)
                cancel.performScrollTo()
                compose.waitForIdle()
                assertEquals("the waiting card's Cancel cannot be reached — $at", cancel.fetchSemanticsNode().laid(), cancel.fetchSemanticsNode().boundsInRoot)
                assertEquals("reaching it moved the ✕ — $at", close, closeIsInSight(words, at))
            } else if (handoff) {
                handoffs++
                val open = compose.onNodeWithTag(OPEN_SIGNER_TAG, useUnmergedTree = true)
                open.performScrollTo()
                compose.waitForIdle()
                assertEquals("the hand-off card cannot be reached — $at", open.fetchSemanticsNode().laid(), open.fetchSemanticsNode().boundsInRoot)
                assertEquals("reaching it moved the ✕ — $at", close, closeIsInSight(words, at))
            }
        }
        android.util.Log.i(TAG, "every board on ${screen.name}: ${SigningScreenState.entries.size} drawn — $confirms with a pinned confirm, $handoffs hand-off cards, $waits waiting cards")
        assertTrue("no board with a confirm was drawn", confirms > 30)
        assertTrue("no hand-off board was drawn", handoffs > 0)
        assertTrue("no waiting board was drawn", waits > 0)
    }

    /**
     * "A sheet that fits looks exactly as it did" is a claim about every
     * pixel, so it is checked against a record, not asserted from memory:
     * every board's layout written down — each text and each described mark
     * with its own box, as laid out — into the app's cache. Run it before and
     * after a change to the sheet's structure and `diff` the two files: a
     * board whose body is at its top is laid out the same wherever its parts
     * are hosted, so the files must be identical.
     *
     * ```bash
     * adb -s <emulator> exec-out run-as app.getvela.wallet cat cache/signing-layout-Roomy.txt > before.txt
     * ```
     */
    @Test
    fun theLayoutOfEveryBoardIsWrittenDownOnARoomyScreen() = writeLayouts(Screen.Roomy)

    @Test
    fun theLayoutOfEveryBoardIsWrittenDownOnThisScreen() = writeLayouts(Screen.This)

    private fun writeLayouts(screen: Screen) {
        val words = words("zh")
        var current by mutableStateOf(SigningFixtures.build(SigningScreenState.CS1, words))
        compose.setContent { Hosted(screen, words) { current } }
        val out = StringBuilder()
        for (state in SigningScreenState.entries) {
            // Each board its own request, so each opens at its top.
            current = SigningFixtures.build(state, words).let { it.copy(requestKey = "layout-${state.name}") }
            compose.waitForIdle()
            val (scrolled, range) = bodyScroll()
            out.append("== ${state.name} sheet ${node(SHEET).laid()} body scrolled $scrolled of $range\n")
            val marks = compose.onAllNodes(
                SemanticsMatcher.keyIsDefined(SemanticsProperties.Text) or SemanticsMatcher.keyIsDefined(SemanticsProperties.ContentDescription),
                useUnmergedTree = true,
            ).fetchSemanticsNodes()
            for (mark in marks) {
                val said = mark.config.getOrNull(SemanticsProperties.Text)?.joinToString("") { it.text }
                    ?: mark.config.getOrNull(SemanticsProperties.ContentDescription)?.joinToString("")
                out.append("${mark.laid()} $said\n")
            }
            assertTrue("${state.name} drew nothing", marks.isNotEmpty())
        }
        java.io.File(context.cacheDir, "signing-layout-${screen.name}.txt").writeText(out.toString())
        android.util.Log.i(TAG, "layouts on ${screen.name}: ${SigningScreenState.entries.size} boards, ${out.lines().size} lines, hash ${out.toString().hashCode()}")
    }

    /**
     * The modal sheet itself, on whatever screen this runs on: the confirm is
     * where it was with no verdict, under a one-row verdict and under the
     * tall one — above the navigation bar, whole — and the tall verdict's
     * place is as tall as the verdict. Photographed for the review (light
     * and dark, both languages) into the app's cache.
     */
    @Test
    fun onTheModalSheetTheConfirmStaysPutUnderATallVerdict() {
        var words by mutableStateOf(words("zh"))
        var dark by mutableStateOf(false)
        var current by mutableStateOf<SigningScreenModel?>(null)
        var root: android.view.View? = null
        compose.setContent {
            root = LocalView.current
            CompositionLocalProvider(LocalVelaStrings provides words) {
                VelaTheme(darkTheme = dark) {
                    Box(modifier = Modifier.fillMaxSize().background(VelaTheme.colors.bgBase))
                    current?.let { model -> SigningSheet(model = model, onDismiss = {}) }
                }
            }
        }
        val log = StringBuilder("the modal sheet")
        for (language in listOf("zh", "en")) {
            words = words(language)
            val waiting = board(SigningScreenState.CS57, words)
            fun landed(state: SigningScreenState) = board(state, words).copy(state = waiting.state, requestKey = waiting.requestKey)
            val statusBar = root?.let { ViewCompat.getRootWindowInsets(it) }?.getInsets(WindowInsetsCompat.Type.statusBars())?.top ?: 0
            /** The ✕ on the modal: displayed, whole, under the status bar and inside the window. */
            fun closeInTheWindow(step: String): Rect {
                val close = compose.onNodeWithContentDescription(closeWords(words))
                close.assertIsDisplayed()
                val box = close.fetchSemanticsNode()
                val seen = box.boundsInWindow
                assertEquals("the ✕ is cut ($step)\n$log", box.size.height.toFloat(), seen.height)
                assertEquals("the ✕ is cut ($step)\n$log", box.size.width.toFloat(), seen.width)
                val window = root?.rootView
                assertTrue(
                    "the ✕ is outside the window ($step): $seen in ${window?.width}x${window?.height}, status bar $statusBar\n$log",
                    window == null || (seen.top >= statusBar && seen.left >= 0f && seen.right <= window.width && seen.bottom <= window.height),
                )
                return seen
            }
            current = waiting
            // The sheet slides in; the test clock runs its animation to the end.
            compose.waitUntil(10_000) { runCatching { node(CONFIRM_TAG) }.isSuccess }
            compose.waitForIdle()
            val confirm = node(CONFIRM_TAG).boundsInWindow
            val least = node(VERDICT_PLACE_TAG).size.height
            val navigationBar = root?.let { ViewCompat.getRootWindowInsets(it) }?.getInsets(WindowInsetsCompat.Type.navigationBars())?.bottom ?: 0
            val window = root?.rootView?.height ?: 0
            log.append("\n$language \"Checking…\": confirm ${confirm.top}..${confirm.bottom} of a $window px window (navigation bar $navigationBar), place $least px, body ${node(BODY_TAG).size.height} px scrolls ${bodyScroll()}")
            for ((kind, state) in listOf("one row" to SigningScreenState.CS63, "four rows and the warning" to SigningScreenState.CS67)) {
                current = landed(state)
                compose.waitForIdle()
                val place = node(VERDICT_PLACE_TAG)
                val shown = node(VERDICT_SHOWN_TAG)
                log.append("\n$language $kind: confirm ${node(CONFIRM_TAG).boundsInWindow.top}..${node(CONFIRM_TAG).boundsInWindow.bottom}, place ${place.size.height} px = verdict ${shown.size.height} px, body ${node(BODY_TAG).size.height} px scrolls ${bodyScroll()}")
                assertEquals("the confirm moved under $kind\n$log", confirm, node(CONFIRM_TAG).boundsInWindow)
                assertEquals("the confirm is cut\n$log", node(CONFIRM_TAG).size.height.toFloat(), node(CONFIRM_TAG).boundsInWindow.height)
                assertTrue("the confirm is under the navigation bar\n$log", window == 0 || confirm.bottom <= window - navigationBar)
                assertEquals("the place is not the verdict's height\n$log", maxOf(least, shown.size.height), place.size.height)
                val close = closeInTheWindow(kind)
                log.append(", ✕ ${close.top}..${close.bottom} (status bar $statusBar)")
                if (state == SigningScreenState.CS67) {
                    assertTrue(shown.size.height > least)
                    // In view once landed: whole, or from its top.
                    val body = node(BODY_TAG).boundsInWindow
                    val at = Rect(place.positionInWindow, Size(place.size.width.toFloat(), place.size.height.toFloat()))
                    if (at.height <= body.height) assertTrue("not all in view: $at in $body\n$log", body.contains(at))
                    else assertEquals("not from its top\n$log", body.top, at.top, 1f)
                    for (night in listOf(false, true)) {
                        dark = night
                        compose.waitForIdle()
                        save("verdict-tall-$language-${if (night) "dark" else "light"}")
                    }
                    dark = false
                }
            }
            // And the fee block growing after the sheet has opened (the
            // iPhone's finding): the modal's top edge may rise, the confirm's
            // place in the window may not change.
            val steps = feeSteps(words)
            current = steps.first().second
            compose.waitForIdle()
            val under = node(CONFIRM_TAG).boundsInWindow
            for ((step, model) in steps) {
                current = model
                compose.waitForIdle()
                log.append("\n$language fee — $step: confirm ${node(CONFIRM_TAG).boundsInWindow.top}..${node(CONFIRM_TAG).boundsInWindow.bottom}, body ${node(BODY_TAG).size.height} px scrolls ${bodyScroll()}")
                assertEquals("the confirm moved in the window ($step)\n$log", under, node(CONFIRM_TAG).boundsInWindow)
                assertEquals("the confirm is cut ($step)\n$log", node(CONFIRM_TAG).size.height.toFloat(), node(CONFIRM_TAG).boundsInWindow.height)
                closeInTheWindow(step)
            }
            // (Not compared with the verdict boards above: this walk opens on
            // a shut confirm, whose one line is the footer's own and keeps
            // its room from the first frame — the boards have none.)
            // The next language opens its own sheet.
            current = null
            compose.waitForIdle()
        }
        android.util.Log.i(TAG, log.toString())
    }

    private fun Rect.contains(other: Rect) =
        other.left >= left - 0.5f && other.right <= right + 0.5f && other.top >= top - 0.5f && other.bottom <= bottom + 0.5f

    /** Evidence for the review: the screen as measured, in the app's cache. */
    private fun save(name: String) {
        val shot = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
        java.io.File(context.cacheDir, "$name.png").outputStream().use { shot.compress(Bitmap.CompressFormat.PNG, 100, it) }
    }

    private companion object {
        const val TAG = "SigningVerdictWhole"
        const val SHEET = "signing-sheet"
        const val SCREEN = "signing-screen"
    }
}
