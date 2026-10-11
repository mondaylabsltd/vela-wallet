package app.getvela.wallet

import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.getvela.wallet.core.designsystem.theme.VelaTheme
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.LocalVelaStrings
import androidx.compose.ui.platform.ViewRootForTest
import androidx.compose.ui.semantics.SemanticsNode
import app.getvela.wallet.core.designsystem.components.VELA_SHEET_CONTENT_TAG
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeBoards
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.HEADER_TAG
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningSheet
import app.getvela.wallet.feature.signing.TechModel
import app.getvela.wallet.feature.signing.core.ClearSigningView
import kotlin.math.roundToInt
import org.junit.Assert.assertNotNull
import app.getvela.wallet.feature.signing.BODY_TAG
import app.getvela.wallet.feature.signing.CONFIRM_TAG
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningScreenModel
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.SigningSheetContent
import app.getvela.wallet.feature.signing.VERDICT_PLACE_TAG
import app.getvela.wallet.feature.signing.VERDICT_SHOWN_TAG
import app.getvela.wallet.feature.signing.VERDICT_WAITING_TAG
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
     *
     * The integration's note 12: and so for EVERY verdict the sheet usually
     * ends on, not that card alone — the place is as tall as the tallest. The
     * sheet's height and the confirm's top, before the verdict and after
     * each kind: "could not check", "expected to fail" with its reason and
     * at the longest reason the core prints, "no asset changes", one balance
     * row (a send), two (a swap). In both languages: the lines wrap
     * differently.
     *
     * PR 3 final note F2: the place is not blank while the verdict is out (a
     * skeleton, said as "Checking…").
     *
     * The device round, item 1: that height is the place's MINIMUM. A verdict
     * taller than it is shown whole and the place grows — it has no scroll
     * of its own any more, no fade and no "more" mark, which is asserted
     * here for the usual verdicts too; the tall ones, and the footer that
     * keeps the confirm where it is under them, are `SigningVerdictWholeTest`.
     */
    @Test
    fun theSimulationsVerdictLandingMovesNothing() = verdictsLandStill(strings)

    @Test
    fun theSimulationsVerdictLandingMovesNothingInEnglish() = verdictsLandStill(
        InstrumentationRegistry.getInstrumentation().targetContext.let { context ->
            I18nRuntime { tag -> context.assets.open("i18n/$tag.json").use { it.readBytes() } }.apply { initialize("en") }
        },
    )

    private fun verdictsLandStill(words: I18nRuntime) {
        val waiting = SigningFixtures.build(SigningScreenState.CS57, words)
        val held = waiting.blocks.filterIsInstance<SigningBlock.Held>().single()
        val couldNotCheck = words.t("componentsUi.signing.simUnavailableWarning")
        var current by mutableStateOf(waiting)
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides words) {
                VelaTheme(darkTheme = false) {
                    SigningSheetContent(model = current, onConfirm = {}, modifier = Modifier.testTag(SHEET))
                }
            }
        }
        fun frame() = Frame(
            sheetHeight = compose.onNodeWithTag(SHEET).fetchSemanticsNode().boundsInRoot.height,
            confirmTop = compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode().boundsInRoot.top,
        )
        fun place() = compose.onNodeWithTag(VERDICT_PLACE_TAG, useUnmergedTree = true).fetchSemanticsNode().boundsInRoot
        fun placeSize() = compose.onNodeWithTag(VERDICT_PLACE_TAG, useUnmergedTree = true).fetchSemanticsNode().size
        /** How far the body can scroll: nothing on a screen the sheet fits. */
        fun bodyRange() = compose.onNodeWithTag(BODY_TAG, useUnmergedTree = true).fetchSemanticsNode()
            .config[SemanticsProperties.VerticalScrollAxisRange].maxValue()
        /** The sheet's vertical scrolls: the body, and nothing else. */
        fun scrolls() = compose.onAllNodes(SemanticsMatcher.keyIsDefined(SemanticsProperties.VerticalScrollAxisRange), useUnmergedTree = true).fetchSemanticsNodes()
        compose.waitForIdle()
        val before = frame()
        val room = place()
        val roomSize = placeSize()
        // The sheet fits this screen (a normal phone's): nothing scrolls, so
        // the place is also WHERE it was. On a screen too short for the sheet
        // the body scrolls the landed verdict into view — on purpose — and
        // only the sizes and the confirm are held to.
        val holds = bodyRange()
        val fits = holds == 0f
        // The rooms are unseen and unsaid.
        compose.onNodeWithText(couldNotCheck).assertDoesNotExist()
        compose.onNodeWithText(words.t("componentsUi.signing.balanceChangesTitle")).assertDoesNotExist()
        // PR 3 final note F2: and the place is not BLANK meanwhile — a
        // skeleton fills it, the size of the place, said as "Checking…".
        val skeleton = compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).fetchSemanticsNode()
        assertEquals("the skeleton is the place", roomSize, skeleton.size)
        if (fits) assertEquals("the skeleton is the place", room, skeleton.boundsInRoot)
        compose.onNodeWithContentDescription(words.t("componentsUi.funding.checking")).assertExists()

        // The same request: only its verdict arrived.
        fun landed(state: SigningScreenState) = SigningFixtures.build(state, words).copy(state = waiting.state, requestKey = waiting.requestKey)
        val kinds = listOf(
            "could not check" to landed(SigningScreenState.CS58),
            "expected to fail, with its reason" to landed(SigningScreenState.CS61),
            // The room the place keeps for a revert: the core's own cap.
            "expected to fail, at the longest reason the core prints" to
                waiting.copy(blocks = waiting.blocks.map { if (it is SigningBlock.Held) it.copy(shown = held.rooms[1]) else it }),
            "no asset changes" to landed(SigningScreenState.CS62),
            "one balance row" to landed(SigningScreenState.CS63),
            "two balance rows" to landed(SigningScreenState.CS64),
        )
        val measured = StringBuilder("before (\"Checking…\"): sheet ${before.sheetHeight} confirm ${before.confirmTop} place ${room.top}..${room.bottom} (${roomSize.height} px)${if (fits) "" else ", the body scrolls by ${bodyRange()} px"}")
        for ((kind, model) in kinds) {
            current = model
            compose.waitForIdle()
            val after = frame()
            val shown = compose.onNodeWithTag(VERDICT_SHOWN_TAG, useUnmergedTree = true).fetchSemanticsNode()
            measured.append("\n$kind: sheet ${after.sheetHeight} confirm ${after.confirmTop}, verdict ${shown.size.height} px in a place of ${placeSize().height}")
            assertEquals("the verdict arriving ($kind) moved the sheet\n$measured", before, after)
            assertEquals("…or resized its own place ($kind)\n$measured", roomSize, placeSize())
            if (fits) assertEquals("…or moved its own place ($kind)\n$measured", room, place())
            // The skeleton is gone the moment a verdict is there.
            compose.onNodeWithTag(VERDICT_WAITING_TAG, useUnmergedTree = true).assertDoesNotExist()
            // The usual verdicts are no taller than the place — and whole in it.
            assertTrue("$kind is taller than the room kept for it\n$measured", shown.size.height <= roomSize.height)
            assertEquals("$kind is cut by its place", shown.size.height.toFloat(), shown.boundsInRoot.height)
            // The place has no scroll of its own: the body is the one scroll.
            assertEquals("a scroll besides the body's ($kind)", listOf(BODY_TAG), scrolls().map { it.config.getOrNull(SemanticsProperties.TestTag) })
        }
        compose.onNodeWithText(words.t("componentsUi.signing.balanceChangesTitle")).assertExists()
        android.util.Log.i("SigningSheetStill", measured.toString())

        // Back to "could not check": said now.
        current = landed(SigningScreenState.CS58)
        compose.waitForIdle()
        compose.onNodeWithText(couldNotCheck).assertExists()
        // And the sheet with no place kept is shorter: the room is real, and
        // this test can see a sheet move.
        current = waiting.copy(blocks = waiting.blocks.filterNot { it is SigningBlock.Held })
        compose.waitForIdle()
        if (fits) {
            assertTrue("no room was being held", frame().sheetHeight < before.sheetHeight)
        } else {
            // The sheet is the screen's height either way: its body holds less.
            assertTrue("no room was being held", bodyRange() < holds)
        }
    }

    // -- the 2026-10-11 walk, item 2: the fee landing moved the ✕ ---------------------------
    //
    // On the phone the sheet's top went 243 → 166 → 89 px as the fee landed
    // (two 77 px steps), and the ✕ from y 337 to 183: first an empty gap under
    // the fee card — the "no coin can pay" line's room, a second before its
    // words — then the speed's note inside the card. The confirm stayed; the
    // header, the amount, the recipient and the verdict all moved, and the ✕
    // moved under a finger. Offline it did the same by another road:
    // estimating → "can't reach Gnosis" → priced.
    //
    // Here the REAL sheet (`SigningSheet`: the modal, anchored at the bottom)
    // shows the transfer the walk signed, and its fee card is the LIVE
    // builder's (`SigningLive.feeModel`) over the fee views the core sends for
    // each moment. Measured on the SCREEN, in px, at whatever window this runs
    // in: the ✕'s top, the header's top, the sheet's top edge and the
    // confirm's top. "Before" is the same walk with the fee's rooms taken out
    // of the model — the sheet as it was; "after" is the model as built.

    private val gnosis by lazy {
        SigningLive.Context(strings, "Gnosis", androidx.compose.ui.graphics.Color(0xFF04795B), "xDAI", "Parallel space", ACCOUNT, chainId = 100)
    }

    private fun estimate() = FeeEstimateView(
        chain_id = 100, total_wei = "10000000000000000", max_fee_per_gas = "1000000000", network_fee_per_gas = "1000000000",
        relayer_fee_per_gas = "0", bundler_gas_price = "1000000000", in_band_gas_basis = "123000", total_gas = "123000",
        deployed = true, tier = FeeTier.Fast, quoted = true, fee_asset = FeeAssetView.Native, fee_recipient = RELAY,
    )

    private fun coin(balance: String, short: Boolean) = FeeOptionView(
        symbol = "xDAI", decimals = 18, balance = balance, recipient = RELAY, usd_balance = if (short) "0" else "5", usd_price = "1",
        amount = "10000000000000000", insufficient = short, selected = true,
    )

    /** The speed in force: the person's "standard" while nothing is priced; "fast", with its note, where it costs nothing more. */
    private fun speed(free: Boolean) = SendLive.SpeedInputs(
        FeeSpeedView(tier = if (free) FeeTier.Fast else FeeTier.Standard, preferred = FeeTier.Standard, free = free, free_note = free),
    ) { null }

    private val measuring = "componentsUi.signing.confirmBlock.feeMeasuring"

    /** One moment of the sheet: the fee as the core states it, and the line the gate says under the confirm. */
    private inner class Moment(val name: String, val fee: FeeView, val free: Boolean = false, val open: Boolean = false, val under: String? = null)

    private fun estimating() = Moment("estimating", FeeView(busy = true), under = measuring)
    private fun reserveKnown() = Moment("estimating, nothing to pay from", FeeView(busy = true, nothing_to_pay_from = true), under = measuring)
    private fun priced(free: Boolean) = Moment(
        if (free) "priced, with the speed's note" else "priced",
        FeeView(fee = estimate(), options = listOf(coin("5000000000000000000", short = false)), confirm_fee_ready = true),
        free = free, open = true,
    )
    private fun cannotPay() = Moment(
        "priced, no coin can pay",
        FeeView(fee = estimate(), options = listOf(coin("0", short = true)), no_coin_pays = true, nothing_to_pay_from = true),
        free = true,
    )
    private fun failed(case: FeeBoards.Case) = FeeBoards.view(case, chainId = 100, account = ACCOUNT).let { view ->
        Moment("failed (${case.name})", view, under = view.failure?.footer_key)
    }

    private data class Place(val close: Int, val header: Int, val sheetTop: Int, val confirm: Int) {
        override fun toString() = "✕ $close · header $header · sheet top $sheetTop · confirm $confirm"
    }

    private fun onScreenTop(node: SemanticsNode): Int {
        val view = (node.root as ViewRootForTest).view
        val at = IntArray(2).also(view::getLocationOnScreen)
        return (at[1] + node.positionInRoot.y).roundToInt()
    }

    private fun stillThroughTheFee(walk: String, moments: List<Moment>) {
        // The walk's own request — an amount, a recipient, the verdict already
        // in ("could not check"), the fee card, who signs — with no technical
        // details folded under it: only the fee is on its way.
        val base = SigningFixtures.build(SigningScreenState.CS58, strings).let { drawn ->
            drawn.copy(
                closeLabel = CLOSE,
                blocks = drawn.blocks.filter { it is SigningBlock.Amount || it is SigningBlock.Party || it is SigningBlock.Held },
                tech = TechModel(title = drawn.tech.title, copyLabel = drawn.tech.copyLabel, explorerLabel = drawn.tech.explorerLabel),
            )
        }
        fun model(moment: Moment, rooms: Boolean): SigningScreenModel {
            val fee = SigningLive.feeModel(ClearSigningView(), moment.fee, gnosis, speed(moment.free)) as FeeModel.OnChain
            assertNotNull("the live builder names the card's rooms", fee.rooms)
            return base.copy(
                requestKey = if (rooms) "after" else "before",
                fee = if (rooms) fee else fee.copy(rooms = null),
                confirmEnabled = moment.open,
                confirmBlockLine = moment.under?.let { strings.t(it) },
            )
        }
        var current by mutableStateOf(model(moments.first(), rooms = false))
        compose.setContent {
            CompositionLocalProvider(LocalVelaStrings provides strings) {
                VelaTheme(darkTheme = false) { SigningSheet(model = current, onDismiss = {}) }
            }
        }
        compose.waitUntil(60_000) { runCatching { compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode() }.isSuccess }
        compose.waitForIdle()
        fun place() = Place(
            close = onScreenTop(compose.onNodeWithContentDescription(CLOSE).fetchSemanticsNode()),
            header = onScreenTop(compose.onNodeWithTag(HEADER_TAG).fetchSemanticsNode()),
            // The sheet has no handle: its content's top is its own top edge.
            sheetTop = onScreenTop(compose.onNodeWithTag(VELA_SHEET_CONTENT_TAG).fetchSemanticsNode()),
            confirm = onScreenTop(compose.onNodeWithTag(CONFIRM_TAG).fetchSemanticsNode()),
        )
        fun through(rooms: Boolean) = moments.map { moment ->
            current = model(moment, rooms)
            compose.waitForIdle()
            moment.name to place()
        }
        val before = through(rooms = false)
        save("$walk-before-end")
        val after = through(rooms = true)
        save("$walk-after-end")
        val window = InstrumentationRegistry.getInstrumentation().targetContext
            .getSystemService(android.view.WindowManager::class.java).currentWindowMetrics.bounds
        val table = buildString {
            append("$walk — window ${window.width()}x${window.height()} px\n")
            moments.indices.forEach { i ->
                append("  ${before[i].first}\n    before: ${before[i].second}\n    after:  ${after[i].second}\n")
            }
        }
        android.util.Log.i(TAG, table)

        val first = after.first().second
        after.forEach { (moment, place) -> assertEquals("with its rooms kept, the sheet moved at \"$moment\"\n$table", first, place) }
        // The room is real, and this test can see a sheet move: without it the
        // sheet's top is not where it ends — unless the window is so short
        // that the sheet stands at its cap from the first frame either way.
        val body = compose.onNodeWithTag(BODY_TAG, useUnmergedTree = true).fetchSemanticsNode()
            .config[SemanticsProperties.VerticalScrollAxisRange].maxValue()
        if (body == 0f) {
            assertTrue("without the rooms nothing moved either: this walk proves nothing\n$table", before.map { it.second.sheetTop }.toSet().size > 1)
            assertTrue("…and the ✕ moved with it\n$table", before.map { it.second.close }.toSet().size > 1)
        }
        // The confirm is the footer's and stays whatever the body does — before and after.
        assertEquals("the confirm moved\n$table", 1, (before + after).map { it.second.confirm }.toSet().size)
    }

    /** Evidence for the review: the screen as measured, in the app's cache. */
    private fun save(name: String) {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val shot = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
        val file = name.replace(Regex("[^A-Za-z0-9]+"), "-").trim('-')
        java.io.File(context.cacheDir, "signing-still-$file.png").outputStream().use {
            shot.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it)
        }
    }

    /** The walk on the phone: estimating → the core learns nothing can pay → the fee lands, with the speed's note, and no coin can pay. */
    @Test
    fun theFeeLandingAsNoCoinCanPayMovesNeitherTheCloseNorTheSheetsTop() =
        stillThroughTheFee("fee priced + can't pay", listOf(estimating(), reserveKnown(), cannotPay(), reserveKnown(), cannotPay()))

    /** A funded wallet: the fee lands and the confirm opens — with the speed's note on a network where "fast" is free, and without it. */
    @Test
    fun theFeeLandingPricedMovesNeitherTheCloseNorTheSheetsTop() =
        stillThroughTheFee("fee priced", listOf(estimating(), priced(free = false), estimating(), priced(free = true), estimating(), priced(free = true)))

    /** The chain's nodes do not answer: the reason under the card, the retry the core makes by itself, and the next failure. */
    @Test
    fun theFeeFailingMovesNeitherTheCloseNorTheSheetsTop() =
        stillThroughTheFee("fee failed", listOf(estimating(), failed(FeeBoards.Case.ChainDown), failed(FeeBoards.Case.Retrying), estimating(), failed(FeeBoards.Case.ChainDown)))

    /** The walk's step 16, in airplane mode: estimating → failed → (the network back) priced, and no coin can pay. */
    @Test
    fun offlineThenBackMovesNeitherTheCloseNorTheSheetsTop() =
        stillThroughTheFee("offline: estimating → failed → priced", listOf(estimating(), failed(FeeBoards.Case.ChainDown), failed(FeeBoards.Case.Retrying), cannotPay()))

    /** The same in English: the sentences wrap differently, and the rooms are the sentences' own. */
    @Test
    fun theFeeLandingInEnglishMovesNeitherTheCloseNorTheSheetsTop() {
        strings.setLocale("en")
        stillThroughTheFee("English: estimating → failed → priced + can't pay", listOf(estimating(), failed(FeeBoards.Case.ChainDown), reserveKnown(), cannotPay(), priced(free = true)))
    }

    private companion object {
        const val SHEET = "signing-sheet"
        const val TAG = "pr4-still"
        const val CLOSE = "关闭"
        const val ACCOUNT = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        const val RELAY = "0x2222222222222222222222222222222222222222"
    }
}
