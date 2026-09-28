package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.send.core.TrackEntryView
import app.getvela.wallet.feature.send.core.TrackOutcome
import app.getvela.wallet.feature.send.core.TrackStatus
import app.getvela.wallet.feature.signing.SigningAftercare
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningTone
import app.getvela.wallet.feature.signing.AmountLine
import app.getvela.wallet.feature.signing.core.SignEnding
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignPhase
import app.getvela.wallet.feature.signing.core.SignErrorNotice
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 079: after the approval the signing sheet is a receipt (the send's own
 * words), and once the core has answered the page the ending stays on screen.
 */
class SigningReceiptTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val op = "0xace642c724834405c56919fb198d6a6389cc2867ff3f3e3585b59d1921fe681d"
    private val tx = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f"
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Me", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", typicalS = 5)
    private val blocks = listOf(
        SigningBlock.Intent("Send", SigningTone.Neutral),
        SigningBlock.Amount(AmountLine(sign = "−", value = "0.001", symbol = "XDAI")),
    )

    private fun entry(status: TrackStatus, outcome: TrackOutcome, txHash: String? = null) =
        TrackEntryView(user_op_hash = op, chain_id = 100, status = status, tx_hash = txHash, submitted_at_ms = 1.0, outcome = outcome)

    @Test
    fun `an account that signs on the Trusted Signer's page gets a button, not a second slide`() {
        val drawn = app.getvela.wallet.feature.signing.SigningFixtures.build(app.getvela.wallet.feature.signing.SigningScreenState.CS1, strings)
        val request = app.getvela.wallet.feature.signing.core.IncomingRequest("r1", "personal_sign", "[\"0x48\",\"0x88cCA0EeDbF2C4426110bbFc998F048689266894\"]", "http://127.0.0.1:8137", "tab-1", 100)
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val clear = app.getvela.wallet.feature.signing.core.ClearSigningView()
        val guard = app.getvela.wallet.feature.signing.core.GuardView()
        val fee = app.getvela.wallet.feature.send.core.FeeView(confirm_fee_ready = true)
        val slide = SigningLive.model(drawn, request, sign, clear, guard, fee, ctx)
        assertTrue(!slide.confirmAsButton)
        val button = SigningLive.model(drawn, request, sign, clear, guard, fee, ctx.copy(trustedSignerRoute = true))
        assertTrue(button.confirmAsButton)
        assertEquals(strings.t("componentsUi.signing.openSigner"), button.confirmButtonLabel)
    }

    @Test
    fun `a page that never opened is the waiting card's to say, with a retry`() {
        val wait = SigningLive.trustedSignerWait(ctx.copy(trustedSignerWaiting = true, trustedSignerUnreachable = true))
        assertEquals(strings.t("componentsUi.signing.signerDown"), wait?.title)
        assertEquals(strings.t("connect.browser.retry"), wait?.reopen)
        assertEquals("", wait?.hint)
        val drawn = app.getvela.wallet.feature.signing.SigningFixtures.build(app.getvela.wallet.feature.signing.SigningScreenState.CS1, strings)
        val request = app.getvela.wallet.feature.signing.core.IncomingRequest("r1", "personal_sign", "[\"0x48\",\"0x88cCA0EeDbF2C4426110bbFc998F048689266894\"]", "http://127.0.0.1:8137", "tab-1", 100)
        val sheet = SigningLive.model(
            drawn, request, SignView(surface = SignSurface.Sheet, is_signing = true),
            app.getvela.wallet.feature.signing.core.ClearSigningView(), app.getvela.wallet.feature.signing.core.GuardView(),
            app.getvela.wallet.feature.send.core.FeeView(confirm_fee_ready = true),
            ctx.copy(trustedSignerWaiting = true, trustedSignerUnreachable = true),
        )
        assertTrue("no \"signing…\" above a page that did not open", sheet.blocks.none { it is SigningBlock.Sentence && it.text == strings.t("componentsUi.signing.signing") })
    }

    @Test
    fun `a failed quote is asked again only until the approval`() {
        val open = SignView(surface = SignSurface.Sheet)
        assertTrue(app.getvela.wallet.feature.signing.core.SigningController.mayRequote(open, answered = false))
        for (after in listOf(open.copy(is_signing = true), open.copy(is_submitting = true), open.copy(pending_op_hash = op), SignView(surface = SignSurface.Hidden))) {
            assertTrue("no re-quote once approved or closed: $after", !app.getvela.wallet.feature.signing.core.SigningController.mayRequote(after, answered = false))
        }
        assertTrue(!app.getvela.wallet.feature.signing.core.SigningController.mayRequote(open, answered = true))
    }

    @Test
    fun `a site whose name is its host says it once`() {
        val drawn = app.getvela.wallet.feature.signing.SigningFixtures.build(app.getvela.wallet.feature.signing.SigningScreenState.CS1, strings)
        val request = app.getvela.wallet.feature.signing.core.IncomingRequest("r1", "personal_sign", "[\"0x48\",\"0x88cCA0EeDbF2C4426110bbFc998F048689266894\"]", "http://127.0.0.1:8137", "tab-1", 100)
        val model = SigningLive.model(
            drawn, request, SignView(surface = SignSurface.Sheet),
            app.getvela.wallet.feature.signing.core.ClearSigningView(), app.getvela.wallet.feature.signing.core.GuardView(),
            app.getvela.wallet.feature.send.core.FeeView(confirm_fee_ready = true), ctx,
        )
        assertEquals("127.0.0.1:8137", model.dappName)
        assertEquals("", model.dappHost)
    }

    @Test
    fun `a request not yet approved has no receipt`() {
        assertNull(SigningLive.receipt(SignView(surface = SignSurface.Sheet), blocks, ctx))
    }

    @Test
    fun `the passkey, the submission and the wait are each named in the send's words`() {
        // Spec 082 RA9: the words are the core's phase, never the shell flags —
        // the network work before the passkey is "preparing", even while the
        // old `is_signing` flag is up.
        val preparing = SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_signing = true, phase = SignPhase.Preparing), blocks, ctx)
        assertEquals(ReceiptStage.Submitting, preparing?.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_PREPARING), preparing?.title)

        val signing = SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_signing = true, phase = SignPhase.AwaitingSignature), blocks, ctx)
        assertEquals(ReceiptStage.Submitting, signing?.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_SIGNING), signing?.title)

        assertNull(
            "no phase, no receipt: the flags alone say nothing",
            SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_signing = true), blocks, ctx),
        )

        val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_submitting = true, phase = SignPhase.Submitting), blocks, ctx)
        assertEquals(strings.t(I18nKeys.Flows.TX_SUBMITTING), submitting?.title)
        assertEquals(strings.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND), submitting?.cta)

        val waiting = SigningLive.receipt(
            SignView(surface = SignSurface.Sheet, pending_op_hash = op),
            blocks,
            ctx.copy(track = entry(TrackStatus.Pending, TrackOutcome.Landing)),
        )
        assertEquals(ReceiptStage.Submitted, waiting?.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_SUBMITTED_TITLE), waiting?.title)
        assertEquals("Send · −0.001 XDAI", waiting?.captions?.first())
        assertEquals("the chain's clock drives the ring", 5, waiting?.eta?.typicalS)
    }

    @Test
    fun `past the wait window the words say it is still on its way, never failed`() {
        val still = SigningLive.receipt(
            SignView(surface = SignSurface.Sheet, pending_op_hash = op),
            blocks,
            ctx.copy(track = entry(TrackStatus.AcceptedNotLanded, TrackOutcome.StillConfirming)),
        )
        assertEquals(ReceiptStage.Submitted, still?.stage)
        assertTrue(still!!.captions.contains(strings.t("componentsUi.signing.stillConfirming")))
        assertNull("no countdown for an op past its window", still.eta)
    }

    @Test
    fun `a message is signing then signed, never submitted to a network`() {
        val request = app.getvela.wallet.feature.signing.core.SignRequestView(
            id = "r1", method = "personal_sign", kind = app.getvela.wallet.feature.signing.core.SignMethodKind.PersonalSign,
            params_json = "[]", origin = "http://127.0.0.1:8137", chain_id = 100,
        )
        for (phase in listOf(SignPhase.Preparing, SignPhase.AwaitingSignature, SignPhase.Submitting)) {
            val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, request = request, phase = phase), blocks, ctx)
            assertEquals(phase.toString(), strings.t("componentsUi.signing.signing"), submitting?.title)
            assertTrue(submitting!!.captions.none { it == strings.t(I18nKeys.Flows.TX_BACKGROUND_HINT) })
        }
        val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, request = request, phase = SignPhase.Submitting), blocks, ctx)
        assertEquals(strings.t("componentsUi.signing.signing"), submitting?.title)
        assertTrue(submitting!!.captions.none { it == strings.t(I18nKeys.Flows.TX_BACKGROUND_HINT) })
    }

    @Test
    fun `a submission that failed is a failed receipt with the sheet's own sentence`() {
        val failed = SigningLive.receipt(
            SignView(surface = SignSurface.Sheet, error = SignErrorNotice(kind = SignErrorKind.SubmitFailed)),
            blocks,
            ctx,
        )
        assertEquals(ReceiptStage.Failed, failed?.stage)
        assertTrue(failed!!.captions.contains(strings.t("send.txErrorGeneric")))
    }

    /** Spec 082 RA8: the ending is the core's reading of the answer (`sign_ending_of`). */
    @Test
    fun `an answer becomes the ending the aftercare shows`() {
        val signed = SigningAftercare.of("personal_sign", 100, SignResponsePayload.Ok("0xsig"), null)
        assertEquals(SigningAftercare(100, SignEnding.Signed), signed)
        assertEquals(
            SigningAftercare(100, SignEnding.Landed(tx, op)),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(tx), op),
        )
        assertEquals(
            "the wait ran out: the page got the operation hash",
            SigningAftercare(100, SignEnding.StillConfirming(op)),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(op.uppercase().replace("0X", "0x")), op),
        )
        assertNull(SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Err(4001, SignErrorKind.UserRejected), op))
    }

    /**
     * Spec 082 RA8 (W3): a landed request is drawn confirmed only when the
     * tracker says so — the answer's tx hash is not proof it did what it said;
     * every `SignEndingState` has its own words.
     */
    @Test
    fun `every ending state is drawn in its own words, and only the tracker confirms`() {
        val landed = SigningAftercare(100, SignEnding.Landed(tx, op))
        val explorer = ctx.copy(explorerUrl = "https://gnosisscan.io")

        val notYet = SigningLive.aftercareReceipt(landed, "Send · −0.001 XDAI", explorer)
        assertEquals("no tracker entry yet: the ring, never a tick", ReceiptStage.Submitted, notYet.stage)

        val confirmed = SigningLive.aftercareReceipt(landed, null, explorer.copy(track = entry(TrackStatus.Confirmed, TrackOutcome.Final, tx)))
        assertEquals(ReceiptStage.Confirmed, confirmed.stage)
        assertEquals(tx, confirmed.hash?.copyValue)
        assertEquals(strings.t(I18nKeys.Flows.VIEW_ON_EXPLORER), confirmed.viewOnExplorer)

        val reverted = SigningLive.aftercareReceipt(landed, null, explorer.copy(track = entry(TrackStatus.Dropped, TrackOutcome.Final, tx)))
        assertEquals(ReceiptStage.Failed, reverted.stage)
        assertTrue("a revert says what it cost: ${reverted.captions}", reverted.captions.contains(strings.t("componentsTx.receipt.failedHint")))
        assertEquals(tx, reverted.hash?.copyValue)
        assertEquals(strings.t(I18nKeys.Flows.VIEW_ON_EXPLORER), reverted.viewOnExplorer)

        val still = SigningAftercare(100, SignEnding.StillConfirming(op))
        val notSent = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.NotSent, TrackOutcome.Final)))
        assertEquals(ReceiptStage.Failed, notSent.stage)
        assertTrue(notSent.captions.contains(strings.t("send.txErrorGeneric")))

        val waiting = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.AcceptedNotLanded, TrackOutcome.StillConfirming)))
        assertEquals(ReceiptStage.Submitted, waiting.stage)
        assertTrue(waiting.captions.contains(strings.t("componentsUi.signing.stillConfirming")))

        val landedLater = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.Confirmed, TrackOutcome.Final, tx)))
        assertEquals(ReceiptStage.Confirmed, landedLater.stage)

        val unknown = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.AcceptedNotLanded, TrackOutcome.Unknown)))
        assertTrue(unknown.captions.contains(strings.t("componentsUi.signing.unknownOutcome")))

        val maybe = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.Pending, TrackOutcome.MaybeSent)))
        assertEquals(strings.t("send.txSubmitting"), maybe.title)
        assertTrue(maybe.captions.contains(strings.t("componentsUi.signing.maybeSent")))
        assertEquals("the op hash, to quote", op, maybe.hash?.copyValue)
        assertEquals(strings.t("send.txCloseBackground"), maybe.cta)
        assertTrue("never failed, never try again", maybe.stage != ReceiptStage.Failed)

        val signed = SigningLive.aftercareReceipt(SigningAftercare(100, SignEnding.Signed), null, ctx)
        assertEquals(strings.t("clearSigning.alertSignedTitle"), signed.title)
    }

    /**
     * Spec 082 RA9: the sheet's words are the core's phase — "preparing" until
     * the passkey is up, "waiting for your signature" while it is,
     * "submitting" after; never a shell flag's guess.
     */
    @Test
    fun `the sheet's words follow the phase`() {
        fun title(phase: SignPhase) = SigningLive.receipt(SignView(surface = SignSurface.Sheet, phase = phase), blocks, ctx)?.title
        assertNull(title(SignPhase.Idle))
        assertEquals(strings.t("send.txPreparing"), title(SignPhase.Preparing))
        assertEquals(strings.t("send.txSigning"), title(SignPhase.AwaitingSignature))
        assertEquals(strings.t("send.txSubmitting"), title(SignPhase.Submitting))
        val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, phase = SignPhase.Submitting), blocks, ctx)!!
        assertTrue(submitting.captions.contains(strings.t("send.txBackgroundHint")))
    }

    /** Spec 082 RA10 (owner ruling 1): a lost reply is "may have been sent" on the live sheet too. */
    @Test
    fun `a lost reply reads may have been sent until the relay shows it holds it`() {
        val sheet = SignView(surface = SignSurface.Sheet, phase = SignPhase.Submitting, pending_op_hash = op, pending_op_maybe_sent = true)
        val maybe = SigningLive.receipt(sheet, blocks, ctx)!!
        assertEquals(strings.t("send.txSubmitting"), maybe.title)
        assertTrue(maybe.captions.contains(strings.t("componentsUi.signing.maybeSent")))
        assertEquals(op, maybe.hash?.copyValue)
        assertEquals(strings.t("send.txCloseBackground"), maybe.cta)

        val stillMaybe = SigningLive.receipt(sheet, blocks, ctx.copy(track = entry(TrackStatus.Pending, TrackOutcome.MaybeSent)))!!
        assertTrue(stillMaybe.captions.contains(strings.t("componentsUi.signing.maybeSent")))

        val acknowledged = SigningLive.receipt(sheet, blocks, ctx.copy(track = entry(TrackStatus.Pending, TrackOutcome.Landing)))!!
        assertEquals(strings.t("send.txSubmittedTitle"), acknowledged.title)
        assertTrue(!acknowledged.captions.contains(strings.t("componentsUi.signing.maybeSent")))
    }
}
