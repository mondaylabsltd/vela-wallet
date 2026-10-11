package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeBoards
import app.getvela.wallet.feature.send.core.FeeCall
import app.getvela.wallet.feature.send.core.FeeExecutor
import app.getvela.wallet.feature.send.core.FeeFailedWord
import app.getvela.wallet.feature.send.core.FeeFailure
import app.getvela.wallet.feature.send.core.FeeFailureRow
import app.getvela.wallet.feature.send.core.FeeFailureTap
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendEstimateFailure
import app.getvela.wallet.feature.send.core.SendFeeCoin
import app.getvela.wallet.feature.send.core.SendPreviousPending
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.send.core.SpeedControl
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * PR 2 notes 1, 10 and 13: a fee that failed is said ONCE — the core's
 * `FeeView.failure` — on the send form's row, the send confirm's line, the
 * signing sheet's row and its footer; the core retries by itself and the
 * shell schedules nothing; Continue's alert is worded by the failure's cause.
 * Every view here is the real fee machine's (`FeeBoards`, or a live session).
 */
class FeeFailureViewTest {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    @After
    fun stop() = scope.cancel()

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private fun board(case: FeeBoards.Case): FeeView = FeeBoards.view(case, chainId = 100, account = safe)

    private val sheetCtx get() = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Me", safe, chainId = 100)

    private fun sendCtx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = emptyMap(),
        money = WalletLive.Money.dollars(),
        fromName = "Me",
        fromAddress = safe,
    )

    private fun formView(fee: FeeView) = SendView(
        stage = SendStage.EnterDetails,
        selected_token = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "1.5", decimals = 18, price_usd = 1.0),
        recipient = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141",
        amount = "0.5",
        fee_busy = fee.busy,
        fee_coin = SendFeeCoin(symbol = "XDAI", contract = null, chain_id = 100),
    )

    private fun formRow(fee: FeeView) = SendLive.form(
        (FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model,
        formView(fee),
        fee,
        sendCtx(),
        SendLive.SpeedInputs(FeeSpeedView(), feeViewOf = { null }),
    ).fee

    private fun sheetRow(fee: FeeView) = SigningLive.feeModel(ClearSigningView(), fee, sheetCtx) as FeeModel.OnChain

    @Test
    fun `a failure the core retries by itself asks for no tap, on the row or under the confirm`() {
        val fee = board(FeeBoards.Case.ChainDown)
        val failure = fee.failure!!
        assertEquals(FeeFailure.ChainRead(rate_limited = false), fee.failed)
        assertEquals(fee.failed, failure.failure)
        assertTrue(failure.auto_retry)
        assertFalse(failure.retrying)
        assertNull("the dash, never \"Tap to retry\"", failure.figure_key)
        assertEquals(I18nKeys.Flows.FEE_RETRYING, failure.footer_key)

        val reason = strings.t(I18nKeys.Flows.FEE_REASON_CHAIN_DOWN, mapOf("chain" to "Gnosis"))
        val sheet = sheetRow(fee)
        assertEquals("—", sheet.value)
        assertEquals(reason, sheet.warning)
        assertTrue("a tap still asks at once", sheet.tappable)

        val form = formRow(fee)
        assertEquals("—", form.value)
        assertEquals(reason, form.reason)
        assertFalse(form.refreshing)
        val tap = strings.t(I18nKeys.Flows.FEE_TAP_TO_RETRY)
        assertTrue("no tap asked for anywhere", listOf(sheet.value, sheet.warning, form.value, form.reason).none { it == tap })
    }

    @Test
    fun `while the core's re-ask is out the reason stays and the sign turns`() {
        val fee = board(FeeBoards.Case.Retrying)
        assertTrue("a re-ask is out", fee.busy)
        assertNull("the run on screen has not failed (yet)", fee.failed)
        val failure = fee.failure!!
        assertTrue(failure.retrying)
        assertEquals(FeeFailure.ChainRead(rate_limited = false), failure.failure)
        assertEquals(I18nKeys.Flows.FEE_RETRYING, failure.footer_key)

        val reason = strings.t(I18nKeys.Flows.FEE_REASON_CHAIN_DOWN, mapOf("chain" to "Gnosis"))
        val sheet = sheetRow(fee)
        assertEquals("never \"Estimating…\" in its place", "—", sheet.value)
        assertEquals(reason, sheet.warning)
        assertTrue(sheet.refreshing && sheet.measuring)

        val form = formRow(fee)
        assertEquals("—", form.value)
        assertEquals(reason, form.reason)
        assertTrue("the measuring sign turns", form.refreshing)
    }

    @Test
    fun `a failure only a tap fixes says so on the row and under the confirm`() {
        val fee = board(FeeBoards.Case.TapOnly)
        assertEquals(FeeFailure.MissingPublicKey, fee.failed)
        val failure = fee.failure!!
        assertFalse(failure.auto_retry)
        assertEquals(I18nKeys.Flows.FEE_TAP_TO_RETRY, failure.figure_key)
        assertEquals(I18nKeys.Flows.FEE_FAILED, failure.footer_key)
        val tap = strings.t(I18nKeys.Flows.FEE_TAP_TO_RETRY)
        assertEquals(tap, sheetRow(fee).value)
        assertEquals(tap, formRow(fee).value)
    }

    @Test
    fun `a fault inside the app is never the chain's`() {
        val fee = board(FeeBoards.Case.Internal)
        assertEquals(FeeFailure.Internal, fee.failed)
        assertTrue(fee.failure!!.auto_retry)
        val reason = strings.t(I18nKeys.Flows.FEE_REASON_INTERNAL)
        assertEquals(reason, sheetRow(fee).warning)
        assertEquals(reason, formRow(fee).reason)
        assertFalse(reason.contains("Gnosis"))
    }

    /**
     * Send's confirm: ONE line under the held confirm — the previous
     * transaction's when it holds it, else the failed fee's own footer, the
     * very line the signing sheet draws.
     */
    @Test
    fun `the send confirm's one line is the sheet's`() {
        val fee = board(FeeBoards.Case.Retrying)
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val held = formView(fee).copy(stage = SendStage.Confirm, can_confirm = false)
        val confirm = SendLive.confirm(drawn, held, sendCtx(), fee)
        assertEquals(strings.t(I18nKeys.Flows.FEE_RETRYING), confirm.ctaHold)
        val feeFact = confirm.facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertEquals("—", feeFact.value)
        assertEquals(strings.t(I18nKeys.Flows.FEE_REASON_CHAIN_DOWN, mapOf("chain" to "Gnosis")), feeFact.note)

        val previous = held.copy(previous_pending = SendPreviousPending(chain_id = 100, user_op_hash = "0x" + "f1".repeat(32), key = I18nKeys.Flows.PREVIOUS_PENDING))
        assertEquals("the previous transaction's line comes first", strings.t(I18nKeys.Flows.PREVIOUS_PENDING), SendLive.confirm(drawn, previous, sendCtx(), fee).ctaHold)
        assertNull("no failure, no hold: no line", SendLive.confirm(drawn, held.copy(can_confirm = true), sendCtx(), FeeView()).ctaHold)
    }

    // -- PR 2 polish: a control does exactly what its words say ------------------

    private fun confirmOf(fee: FeeView, chainOfForm: Int = 100): app.getvela.wallet.feature.flows.SendConfirmModel {
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val view = formView(fee).copy(
            stage = SendStage.Confirm,
            can_confirm = false,
            selected_token = formView(fee).selected_token!!.copy(chain_id = chainOfForm, network = "chain-$chainOfForm"),
        )
        return SendLive.confirm(drawn, view, sendCtx(), fee)
    }

    private fun feeFact(confirm: app.getvela.wallet.feature.flows.SendConfirmModel) =
        confirm.facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }

    /**
     * The relay answered that the operation fails in the coin chosen, and
     * another coin is on offer: the row says "Pay with another coin" and its
     * tap opens the coins — on the form, the confirm's fee line and the sheet;
     * under the held confirm the fact, asking for no tap.
     */
    @Test
    fun `a fee that would fail opens the coins, and says so`() {
        val fee = board(FeeBoards.Case.WouldFailChooseCoin)
        val failure = fee.failure!!
        assertEquals(FeeFailure.WouldFail, failure.failure)
        assertEquals(FeeFailureTap.ChooseCoin, failure.tap)
        assertEquals(I18nKeys.Flows.FEE_PAY_WITH_ANOTHER_COIN, failure.figure_key)
        assertEquals(I18nKeys.Flows.FEE_WOULD_FAIL, failure.footer_key)
        assertEquals("the question it answered", 100, failure.chain_id)
        assertEquals(FeeBoards.USDC, failure.fee_token)
        assertEquals(FeeFailureRow.Tap.OpenCoins, FeeFailureRow.tap(failure))

        val words = strings.t(I18nKeys.Flows.FEE_PAY_WITH_ANOTHER_COIN)
        val form = formRow(fee)
        assertEquals(words, form.value)
        assertTrue("a control", form.opens)
        assertEquals("the form, holding no confirm, says the fact", strings.t(I18nKeys.Flows.FEE_WOULD_FAIL), form.reason)
        val sheet = sheetRow(fee)
        assertEquals(words, sheet.value)
        assertTrue(sheet.tappable && sheet.chevron)
        val confirm = confirmOf(fee)
        assertEquals(words, feeFact(confirm).value)
        assertTrue("the confirm's fee line is the control too", feeFact(confirm).tap)
        assertEquals(strings.t(I18nKeys.Flows.FEE_WOULD_FAIL), confirm.ctaHold)
        assertFalse("never \"Tap it to retry\"", confirm.ctaHold == strings.t(I18nKeys.Flows.FEE_FAILED))
    }

    /** No other coin left: the dash, and the row is no control anywhere. */
    @Test
    fun `a fee that would fail with no other coin is no control`() {
        val fee = board(FeeBoards.Case.WouldFailNothing)
        val failure = fee.failure!!
        assertEquals(FeeFailureTap.None, failure.tap)
        assertNull(failure.figure_key)
        assertEquals(FeeFailureRow.Tap.Ignore, FeeFailureRow.tap(failure))
        val form = formRow(fee)
        assertEquals("—", form.value)
        assertFalse("no tap target, no chevron", form.opens)
        assertEquals("never a dash with no why", strings.t(I18nKeys.Flows.FEE_WOULD_FAIL), form.reason)
        val sheet = sheetRow(fee)
        assertEquals("—", sheet.value)
        assertFalse(sheet.tappable)
        assertFalse(sheet.chevron)
        val confirm = confirmOf(fee)
        assertEquals("—", feeFact(confirm).value)
        assertFalse(feeFact(confirm).tap)
        assertEquals(strings.t(I18nKeys.Flows.FEE_WOULD_FAIL), confirm.ctaHold)
    }

    /**
     * Only a tap fixes it, and the confirm's footer says "Tap it to retry":
     * the line that shows the failure on the confirm is the retry, as the
     * form's row is — never a dash nobody can tap under a line asking for one.
     */
    @Test
    fun `the tap-only footer's line is a retry on the confirm`() {
        val fee = board(FeeBoards.Case.TapOnly)
        assertEquals(FeeFailureTap.Retry, fee.failure!!.tap)
        assertEquals(FeeFailureRow.Tap.Retry, FeeFailureRow.tap(fee.failure))
        val confirm = confirmOf(fee)
        assertEquals(strings.t(I18nKeys.Flows.FEE_FAILED), confirm.ctaHold)
        assertEquals(strings.t(I18nKeys.Flows.FEE_TAP_TO_RETRY), feeFact(confirm).value)
        assertTrue("tappable, as its footer says", feeFact(confirm).tap)
        // While the core's own re-ask is out a second tap asks nothing.
        assertEquals(FeeFailureRow.Tap.Ignore, FeeFailureRow.tap(board(FeeBoards.Case.Retrying).failure))
        // No failure: the row's own tap, the coins.
        assertEquals(FeeFailureRow.Tap.OpenCoins, FeeFailureRow.tap(null))
        assertFalse("no failure, no tap on the confirm's fee line", feeFact(confirmOf(FeeView())).tap)
    }

    /** A retried failure's row is a retry: no chevron saying "coins" over it, its room kept. */
    @Test
    fun `a failure the row retries draws no coin chevron`() {
        val withCoins = board(FeeBoards.Case.ChainDown).copy(
            options = board(FeeBoards.Case.WouldFailChooseCoin).options,
        )
        val sheet = sheetRow(withCoins)
        assertTrue(sheet.tappable)
        assertFalse(sheet.chevron)
        assertTrue("its room kept, so the figure does not move", sheet.chevronRoom)
    }

    /** A key this build does not know draws the dash, never a dotted path; an unknown footer draws nothing. */
    @Test
    fun `an unknown key is the dash, never a dotted path`() {
        val failure = board(FeeBoards.Case.TapOnly).failure!!.copy(figure_key = "componentsUi.gas.somethingNew", footer_key = "componentsUi.signing.confirmBlock.somethingNew")
        assertEquals("—", FeeFailureRow.figure(failure, strings))
        assertNull(FeeFailureRow.footer(failure, strings))
        assertEquals(strings.t(I18nKeys.Flows.FEE_PAY_WITH_ANOTHER_COIN), FeeFailureRow.figure(failure.copy(figure_key = I18nKeys.Flows.FEE_PAY_WITH_ANOTHER_COIN), strings))
    }

    /**
     * Right after a token switch the form names another chain before the fee
     * machine is asked about it: the old chain's failure is dropped whole —
     * no figure, no reason, no line, no tap of its own — and it does not hold
     * the send machine's confirm.
     */
    @Test
    fun `another chain's failure is dropped whole`() {
        val fee = board(FeeBoards.Case.TapOnly)
        assertEquals(100, fee.failure!!.chain_id)
        // The form on chain 1 now; the failure is chain 100's.
        val onOther = formView(fee).copy(selected_token = formView(fee).selected_token!!.copy(chain_id = 1, network = "chain-1"))
        val row = SendLive.form(
            (FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model,
            onOther,
            fee,
            sendCtx(),
            SendLive.SpeedInputs(FeeSpeedView(), feeViewOf = { null }),
        ).fee
        assertNull("no reason line", row.reason)
        assertFalse("no \"Tap to retry\"", row.value == strings.t(I18nKeys.Flows.FEE_TAP_TO_RETRY))
        assertTrue("the row's own tap, the coins", row.opens)
        val confirm = confirmOf(fee, chainOfForm = 1)
        assertNull("no line under the confirm", confirm.ctaHold)
        assertNull(feeFact(confirm).note)
        assertFalse(feeFact(confirm).tap)
        assertNull(FeeFailureRow.forChain(fee, 1))
        assertEquals(fee.failure, FeeFailureRow.forChain(fee, 100))
        // The bridge to the send machine: not failed for this form.
        assertFalse(FeeFailedWord.failed(fee, onOther))
        assertTrue(FeeFailedWord.failed(fee, formView(fee)))
        // A failure built without a run (no chain) is taken as it is.
        val unchained = fee.copy(failure = fee.failure!!.copy(chain_id = null))
        assertTrue(FeeFailedWord.failed(unchained, onOther))
    }

    /** The new fields cross the wire; a core that predates them reads `retry`, no chain, no coin. */
    @Test
    fun `the failure's tap, chain and coin decode, with the old defaults`() {
        val json = app.getvela.wallet.core.crux.Wire.json
        val full = json.decodeFromString(
            app.getvela.wallet.feature.send.core.FeeFailureView.serializer(),
            """{"failure":"would_fail","reason_key":null,"auto_retry":false,"retrying":false,"figure_key":"componentsUi.gas.payWithAnotherCoin","footer_key":"componentsUi.signing.confirmBlock.feeWouldFail","tap":"choose_coin","chain_id":4217,"fee_token":"0x20c0000000000000000000000000000000000000"}""",
        )
        assertEquals(FeeFailureTap.ChooseCoin, full.tap)
        assertEquals(4217, full.chain_id)
        assertEquals("0x20c0000000000000000000000000000000000000", full.fee_token)
        assertEquals(FeeFailureTap.None, json.decodeFromString(app.getvela.wallet.feature.send.core.FeeFailureView.serializer(), """{"failure":"would_fail","footer_key":"x","tap":"nothing"}""").tap)
        val old = json.decodeFromString(app.getvela.wallet.feature.send.core.FeeFailureView.serializer(), """{"failure":"missing_public_key","footer_key":"componentsUi.signing.confirmBlock.feeFailed"}""")
        assertEquals(FeeFailureTap.Retry, old.tap)
        assertNull(old.chain_id)
        assertNull(old.fee_token)
    }

    /** PR 2 note 13: Continue's alert, worded by the fee machine's own failure, passed through as it is. */
    @Test
    fun `continue's alert is worded by its cause`() {
        fun body(failure: SendEstimateFailure) = SendLive.alertText(SendAlertKind.EstimateFailed(failure), strings, "Gnosis").second
        val chainDown = body(SendEstimateFailure.Fee(board(FeeBoards.Case.ChainDown).failed!!))
        assertEquals(strings.t(I18nKeys.Flows.ALERT_ESTIMATE_CHAIN_DOWN_BODY, mapOf("chain" to "Gnosis")), chainDown)
        assertTrue(chainDown.contains("Gnosis"))
        val internal = body(SendEstimateFailure.Fee(board(FeeBoards.Case.Internal).failed!!))
        assertEquals(strings.t(I18nKeys.Flows.FEE_REASON_INTERNAL), internal)
        assertFalse(internal.contains("Gnosis"))
        for (other in listOf(SendEstimateFailure.Fee(FeeFailure.QuoteUnavailable), SendEstimateFailure.Fee(FeeFailure.WouldFail), SendEstimateFailure.Timeout, SendEstimateFailure.Other)) {
            assertEquals(strings.t(I18nKeys.Flows.ALERT_ESTIMATE_BODY), body(other))
        }
        assertEquals(strings.t(I18nKeys.Flows.ALERT_ESTIMATE_TITLE), SendLive.alertText(SendAlertKind.EstimateFailed(SendEstimateFailure.Other), strings).first)
        // The boards draw the same: SD2H by the chain, SD2I by the app.
        assertEquals(SendAlertKind.EstimateFailed(SendEstimateFailure.Fee(FeeFailure.ChainRead(false))), FlowFixtures.alert(FlowState.SD2H))
        assertEquals(SendAlertKind.EstimateFailed(SendEstimateFailure.Fee(FeeFailure.Internal)), FlowFixtures.alert(FlowState.SD2I))
    }

    // -- the core's own retry, answered by the shell's executor ---------------------

    private val port = FakeRelayPort()

    private fun speedControl(): SpeedControl {
        val relay = RelayClient(port, builtinBase = { "https://relay.test" }, retryDelayMs = 0)
        return SpeedControl(scope, relay, FeeExecutor(relay, keyHexes = { emptyList() }), { FeeTier.Standard }, { "comma_dot" }, area = "test").start()
    }

    private fun reads() = port.calls.count { it.endsWith("eth_getCode") }

    /**
     * The core asks again by itself (3 s after the failure): the shell's
     * executor answers its `StartTtl` and nothing else schedules a retry. A
     * surface that is left ([SpeedControl.end], Send's `left`) stops
     * answering: no read goes after it.
     */
    @Test
    fun `the core retries a failed fee by itself, and a left surface stops it`() = runBlocking<Unit> {
        // Nothing answers: the account read fails, as the chain's.
        val speed = speedControl()
        val calls = listOf(FeeCall(to = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141", value = "1000000000000000"))
        val quoted = withTimeout(20_000) { speed.quote(100, safe, true, calls, null) } as SpeedControl.Quoted.Settled
        assertEquals(FeeFailure.ChainRead(rate_limited = false), quoted.view.failed)
        val first = reads()
        // The re-ask the core asked for (3 s) is a real new read, and it says it is retrying meanwhile.
        //
        // That read is HELD in flight here, as `SigningFeeRetryTest` holds
        // its own: answered at once, "retrying" is a moment between two
        // identical failures, `speed.fee` is a conflated flow, and on a
        // loaded machine its collector was never scheduled inside either of
        // the two moments the ten seconds held (PR 489's proof runs).
        val asked = kotlinx.coroutines.CompletableDeferred<Unit>()
        val release = kotlinx.coroutines.CompletableDeferred<Unit>()
        port.before = { method ->
            if (method == "eth_getCode") {
                asked.complete(Unit)
                release.await()
            }
        }
        withTimeout(20_000) { asked.await() }
        assertTrue("the re-ask is a new read of the account", reads() > first)
        withTimeout(20_000) { speed.fee.first { it.failure?.retrying == true } }
        // The read is answered (it fails again) and the schedule goes on.
        release.complete(Unit)

        speed.end()
        withTimeout(5_000) { speed.fee.first { it.failure == null && !it.busy } }
        val after = reads()
        delay(7_000)
        assertEquals("no read after the surface was left (every re-ask cancelled)", after, reads())
        speed.dispose()
    }
}
