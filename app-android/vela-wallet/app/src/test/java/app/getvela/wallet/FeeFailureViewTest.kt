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
import app.getvela.wallet.feature.send.core.FeeFailure
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
        withTimeout(10_000) { speed.fee.first { it.failure?.retrying == true } }
        withTimeout(10_000) { while (reads() <= first) delay(20) }

        speed.end()
        withTimeout(5_000) { speed.fee.first { it.failure == null && !it.busy } }
        val after = reads()
        delay(7_000)
        assertEquals("no read after the surface was left (every re-ask cancelled)", after, reads())
        speed.dispose()
    }
}
