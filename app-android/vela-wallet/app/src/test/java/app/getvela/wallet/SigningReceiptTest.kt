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
import kotlinx.serialization.json.JsonPrimitive
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
        val signed = SigningAftercare.of("personal_sign", 100, SignResponsePayload.Ok(JsonPrimitive("0xsig")), null)
        assertEquals(SigningAftercare(100, SignEnding.Signed), signed)
        assertEquals(
            SigningAftercare(100, SignEnding.Landed(tx, op)),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(JsonPrimitive(tx)), op),
        )
        assertEquals(
            "the wait ran out: the page got the operation hash",
            SigningAftercare(100, SignEnding.StillConfirming(op)),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(JsonPrimitive(op.uppercase().replace("0X", "0x"))), op),
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
    /**
     * Spec 082 RJ3: a relay refusal is "refused, nothing was sent" — on the
     * live sheet (`failure_refused`) and in the ending (`Refused`) — and never
     * the "please try again" of a failure a retry could fix.
     */
    @Test
    fun `a refusal says refused and never try again`() {
        val retry = strings.t("send.txErrorGeneric")
        val refusedWords = strings.t(I18nKeys.Flows.SIGN_REFUSED)
        val sheet = SignView(
            surface = SignSurface.Sheet,
            error = SignErrorNotice(SignErrorKind.SubmitFailed, "the network refused this transaction; nothing was sent"),
            failure_refused = true,
        )
        val live = SigningLive.receipt(sheet, blocks, ctx)!!
        assertEquals(ReceiptStage.Failed, live.stage)
        assertEquals(strings.t(I18nKeys.Flows.STATUS_FAILED), live.title)
        assertTrue(live.captions.contains(refusedWords))
        assertTrue("no Retry words: ${live.captions}", !live.captions.contains(retry))
        val warning = SigningLive.statusBlocks(sheet, strings).filterIsInstance<SigningBlock.Warning>().single()
        assertEquals(refusedWords, warning.text)

        // Not refused: the plain failure a retry can fix keeps its words.
        val plain = SigningLive.receipt(sheet.copy(failure_refused = false), blocks, ctx)!!
        assertTrue(plain.captions.contains(retry))
        assertEquals("a refusal has no Try again", null, live.retry)
        assertTrue(live.ctaAccent)

        // Spec 096 F8: nothing was sent and it was no refusal — the core holds
        // the page's answer and says so; Try again beside Done, the accent on it.
        val retryable = SigningLive.receipt(sheet.copy(failure_refused = false, failure_retryable = true), blocks, ctx)!!
        assertEquals(strings.t(I18nKeys.Flows.TX_RETRY), retryable.retry)
        assertEquals(strings.t(I18nKeys.Flows.DONE), retryable.cta)
        assertTrue(!retryable.ctaAccent)

        val ending = SigningLive.aftercareReceipt(app.getvela.wallet.feature.signing.core.SignEndingState.Refused, "Send", ctx)
        assertEquals(ReceiptStage.Failed, ending.stage)
        assertEquals(strings.t(I18nKeys.Flows.STATUS_FAILED), ending.title)
        assertEquals(listOf("Send", refusedWords), ending.captions)
    }

    /**
     * Spec 082 RJ3/RJ4: the ending follows the tracker — its `rejected` is the
     * Refused ending; but an answer that already holds a tx hash is never drawn
     * "refused" or "not sent" (the review's rule): it follows to the real verdict.
     */
    @Test
    fun `the tracker's rejection is the refused ending, never over a landed answer`() {
        val still = SigningAftercare(100, SignEnding.StillConfirming(op))
        assertEquals(app.getvela.wallet.feature.signing.core.SignEndingState.Refused, still.state(entry(TrackStatus.Rejected, TrackOutcome.Final)))
        val landed = SigningAftercare(100, SignEnding.Landed(tx, op))
        val state = landed.state(entry(TrackStatus.Rejected, TrackOutcome.Final))
        assertTrue("a landed answer follows on: $state", state is app.getvela.wallet.feature.signing.core.SignEndingState.Following)
    }

    /**
     * Spec 082 RJ1: the write-ahead gate. A submit POSTs only on the core's
     * clearance for a record the store really took; no clearance in time, or
     * one for a record the store refused, is no POST.
     */
    @Test
    fun `the write-ahead gate lets a POST go only for a record on disk`() = kotlinx.coroutines.runBlocking {
        val gate = app.getvela.wallet.feature.send.core.WriteAhead(waitMs = 200L)
        val hash = "0xAbC"

        val silent = gate.expect("r1|abc")
        assertTrue("no clearance in time: no POST", !silent.await())

        val refused = gate.expect("r1|abc")
        assertTrue("the store never took it", !gate.clear("r1|abc", hash))
        assertTrue(!refused.await())

        gate.written(hash)
        val cleared = gate.expect("r1|abc")
        assertTrue(gate.clear("r1|abc", hash))
        assertTrue(cleared.await())

        assertTrue("a clearance nobody waits for posts nothing", !gate.clear("r2|abc", hash))
    }

    /**
     * Spec 082 RJ1 (review): "on disk" means THIS attempt's write. The same op
     * signed again — the same nonce, calls and fee give the same hash (DX9's
     * runs 1–2 shared `0x7df211ed…`) — after the first attempt's row was
     * withdrawn: a store that refuses the new write must not be cleared by the
     * mark the first write left. The Send path's gate lives as long as the app.
     */
    @Test
    fun `an earlier write of the same op never clears a later attempt the store refused`() = kotlinx.coroutines.runBlocking {
        val gate = app.getvela.wallet.feature.send.core.WriteAhead(waitMs = 200L)
        val hash = "0xAbC"

        // Attempt 1: written, cleared, posted — then not sent, its row withdrawn.
        val first = gate.expect("abc", hash)
        gate.written(hash)
        assertTrue(gate.clear("abc", hash))
        assertTrue(first.await())

        // Attempt 2, the same op: the store refuses the write.
        val second = gate.expect("abc", hash)
        assertTrue("no record this time: no POST", !gate.clear("abc", hash))
        assertTrue(!second.await())

        // Attempt 3: written, but its clearance came after the wait ran out
        // (nobody posted). Attempt 4 — the store refuses again — is not
        // cleared by attempt 3's mark either.
        val late = gate.expect("abc", hash)
        gate.written(hash)
        assertTrue("the wait ran out", !late.await())
        assertTrue("a late clearance posts nothing", !gate.clear("abc", hash))
        val fourth = gate.expect("abc", hash)
        assertTrue("attempt 3's write is not attempt 4's", !gate.clear("abc", hash))
        assertTrue(!fourth.await())

        // Attempt 5: written, and no clearance ever came (its pipeline went
        // away). Attempt 6 — the store refuses — starts from nothing.
        val fifth = gate.expect("abc", hash)
        gate.written(hash)
        assertTrue(!fifth.await())
        val sixth = gate.expect("abc", hash)
        assertTrue("attempt 5's write is not attempt 6's", !gate.clear("abc", hash))
        assertTrue(!sixth.await())
    }
}
