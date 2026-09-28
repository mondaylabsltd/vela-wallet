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
import app.getvela.wallet.feature.signing.core.SignErrorKind
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
        val signing = SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_signing = true), blocks, ctx)
        assertEquals(ReceiptStage.Submitting, signing?.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_SIGNING), signing?.title)

        val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, is_submitting = true), blocks, ctx)
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
        val submitting = SigningLive.receipt(SignView(surface = SignSurface.Sheet, request = request, is_submitting = true), blocks, ctx)
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

    @Test
    fun `an answer becomes the ending the aftercare shows`() {
        val signed = SigningAftercare.of("personal_sign", 100, SignResponsePayload.Ok("0xsig"), null)
        assertEquals(SigningAftercare.Signed(100), signed)
        assertEquals(
            SigningAftercare.Landed(100, tx),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(tx), op),
        )
        assertEquals(
            "the wait ran out: the page got the operation hash",
            SigningAftercare.StillConfirming(100, op),
            SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Ok(op.uppercase().replace("0X", "0x")), op),
        )
        assertNull(SigningAftercare.of("eth_sendTransaction", 100, SignResponsePayload.Err(4001, SignErrorKind.UserRejected), op))
    }

    @Test
    fun `the aftercare ticks for a landing and follows the tracker for an op still on its way`() {
        val landed = SigningLive.aftercareReceipt(SigningAftercare.Landed(100, tx), "Send · −0.001 XDAI", ctx.copy(explorerUrl = "https://gnosisscan.io"))
        assertEquals(ReceiptStage.Confirmed, landed.stage)
        assertEquals(tx, landed.hash?.copyValue)
        assertEquals(strings.t(I18nKeys.Flows.VIEW_ON_EXPLORER), landed.viewOnExplorer)

        val still = SigningAftercare.StillConfirming(100, op)
        val waiting = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.AcceptedNotLanded, TrackOutcome.StillConfirming)))
        assertEquals(ReceiptStage.Submitted, waiting.stage)
        assertTrue(waiting.captions.contains(strings.t("componentsUi.signing.stillConfirming")))

        val landedLater = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.Confirmed, TrackOutcome.Final, tx)))
        assertEquals(ReceiptStage.Confirmed, landedLater.stage)

        val unknown = SigningLive.aftercareReceipt(still, null, ctx.copy(track = entry(TrackStatus.AcceptedNotLanded, TrackOutcome.Unknown)))
        assertTrue(unknown.captions.contains(strings.t("componentsUi.signing.unknownOutcome")))

        val signed = SigningLive.aftercareReceipt(SigningAftercare.Signed(100), null, ctx)
        assertEquals(strings.t("clearSigning.alertSignedTitle"), signed.title)
    }
}
