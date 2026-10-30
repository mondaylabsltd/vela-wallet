package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreScript
import app.getvela.wallet.core.crux.Wire
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.ChainRecovery
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeBoards
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendAddNetworkMsg
import app.getvela.wallet.feature.send.core.SendAddNetworkOutcome
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.send.core.SendPasskeyNote
import app.getvela.wallet.feature.send.core.SendReceiptOutcome
import app.getvela.wallet.feature.send.core.SendReceiptStatus
import app.getvela.wallet.feature.send.core.SendRecipientRisk
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendSubmitFailure
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.feature.send.core.SendTxErrorKey
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SendCore

/**
 * Send rounds 3–6 (P2.2) on the REAL send machine, each event this shell now
 * sends — `submit_slow`, `chain_answered`, `send_again`, `add_network_tapped`,
 * `retry_risk_check` — and what the confirm, the receipt and the picker then
 * draw through the live builders: the web's `live-send.ts` / `live-send-lock.ts`.
 */
class SendRoundsTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val payee = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "1.5", decimals = 18, price_usd = 1.0)
    private val hash = "0x" + "ab".repeat(32)

    private fun estimate(totalWei: String = "123000000000000") = FeeEstimateView(
        chain_id = 100, total_wei = totalWei, max_fee_per_gas = "1000000000", network_fee_per_gas = "1000000000",
        relayer_fee_per_gas = "0", bundler_gas_price = "1000000000", in_band_gas_basis = "123000", total_gas = "123000",
        deployed = true, tier = FeeTier.Standard, quoted = true, fee_asset = FeeAssetView.Native,
        fee_recipient = "0x2222222222222222222222222222222222222222",
    )

    private fun json(result: SendShellResult) = Wire.json.encodeToString(SendShellResult.serializer(), result)

    private val ctx by lazy { SendLive.Context(strings, mapOf(100 to "Gnosis"), emptyMap(), WalletLive.Money.dollars(), "Me", safe) }

    private fun confirmModel(view: SendView, fee: app.getvela.wallet.feature.send.core.FeeView? = null) =
        SendLive.confirm((FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model, view, ctx, fee)

    private fun receiptModel(view: SendView) =
        SendLive.receipt((FlowFixtures.build(FlowState.SD4B, strings).base as FlowBase.SendReceipt).model, view, ctx)

    /** The machine, its reads answered as a calm network would; the submit, the risk read and timers left out unless asked. */
    private class Machine(risk: () -> SendRecipientRisk?, xdai: SendToken, estimate: FeeEstimateView) {
        val script = CoreScript(SendCore().asBridge()) { operation ->
            fun json(result: SendShellResult) = Wire.json.encodeToString(SendShellResult.serializer(), result)
            when (operation.optString("type")) {
                "fetch_tokens" -> json(SendShellResult.TokensLoaded(tokens = listOf(xdai), chains = listOf(SendChainInfo(100, "chain-100", "XDAI"))))
                "load_account_credential" -> json(SendShellResult.AccountCredential("04" + "ab".repeat(64)))
                "estimate_fee" -> json(SendShellResult.FeeEstimated(SendFeeOutcome.Ok(estimate)))
                "probe_treasury" -> json(SendShellResult.TreasuryProbed(SendTreasuryProbe.Covered))
                "resolve_risk" -> json(SendShellResult.RiskResolved(risk()))
                "resolve_identity" -> json(SendShellResult.IdentityResolved(null))
                "simulate_calls" -> json(SendShellResult.SimResolved(null))
                "prewarm_fees" -> json(SendShellResult.FeesPrewarmed)
                "haptic" -> json(SendShellResult.HapticPlayed)
                "show_alert" -> json(SendShellResult.AlertAcknowledged)
                "persist_tx_records" -> json(SendShellResult.RecordsPersisted)
                "track_submitted" -> json(SendShellResult.TrackHandedOff)
                "clear_to_post" -> json(SendShellResult.PostCleared)
                "mark_admitted" -> json(SendShellResult.RecordsPersisted)
                "delete_tx_records" -> json(SendShellResult.RecordsPersisted)
                "track_withdrawn" -> json(SendShellResult.TrackHandedOff)
                "cancel_passkey_sign" -> json(SendShellResult.PasskeyCancelAcknowledged)
                else -> null
            }
        }

        fun send(event: SendEvent) = script.dispatch(Wire.json.encodeToString(SendEvent.serializer(), event))
        fun view(): SendView = Wire.json.decodeFromString(SendView.serializer(), script.viewJson())
        fun answer(result: SendShellResult, type: String = "submit_user_op") =
            assertTrue("the core waits on $type (waiting: ${script.waiting()})", script.resolve(type, Wire.json.encodeToString(SendShellResult.serializer(), result)))
    }

    private val calm = { SendRecipientRisk(is_contract = false, first_time = false) }

    private fun onConfirm(risk: () -> SendRecipientRisk? = calm, amount: String = "0.5"): Machine {
        val m = Machine(risk, xdai, estimate())
        m.send(SendEvent.Open(account = SendAccountRef(id = "cred", address = safe), display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)))
        m.send(SendEvent.FeeFailedChanged(false))
        m.send(SendEvent.SelectToken(SendLive.tokenId(m.view().tokens.single())))
        m.send(SendEvent.SetRecipient(payee))
        m.send(SendEvent.SetAmount(amount))
        m.send(SendEvent.Continue)
        assertEquals("on the confirm (waiting: ${m.script.waiting()})", SendStage.Confirm, m.view().stage)
        return m
    }

    /** Slid, the submit out — its passkey not open yet. */
    private fun submitting(): Machine {
        val m = onConfirm()
        assertTrue("open", m.view().can_confirm)
        m.send(SendEvent.SlideConfirm(m.view().review_digest))
        assertTrue("the submit is out (waiting: ${m.script.waiting()})", "submit_user_op" in m.script.waiting())
        return m
    }

    // -- submit_slow (r5reg-3) --------------------------------------------------------

    @Test
    fun `slow reads before the passkey are said with the network and the time, busy or not, and go when it opens`() {
        val m = submitting()
        m.send(SendEvent.SubmitSlow(elapsed_ms = 12_400, busy = false))
        val slow = m.view()
        assertEquals(12, slow.submit_slow?.seconds)
        assertFalse(slow.submit_slow!!.busy)
        val confirm = confirmModel(slow)
        assertFalse(confirm.ctaEnabled)
        assertEquals(strings.t(I18nKeys.Flows.SUBMIT_SLOW, mapOf("network" to "Gnosis", "seconds" to "12")), confirm.ctaHold)

        m.send(SendEvent.SubmitSlow(elapsed_ms = 14_000, busy = true))
        assertEquals(
            strings.t(I18nKeys.Flows.SUBMIT_SLOW_BUSY, mapOf("network" to "Gnosis", "seconds" to "14")),
            confirmModel(m.view()).ctaHold,
        )
        assertTrue("the words are the corpus's", confirmModel(m.view()).ctaHold!!.contains("Gnosis"))

        // The passkey opened: the wait is over, and nothing slow is said again.
        m.send(SendEvent.SigningStarted)
        assertNull(m.view().submit_slow)
        m.send(SendEvent.SubmitSlow(elapsed_ms = 15_000, busy = false))
        assertNull("no slow line once signing", m.view().submit_slow)
    }

    // -- chain_answered (round 6) ----------------------------------------------------

    @Test
    fun `a submit failed as the network's reopens the confirm when that network answers, and only then`() {
        val m = submitting()
        m.answer(SendShellResult.SubmitFailed(SendSubmitFailure.ChainUnreachable(busy = false)))
        val down = m.view()
        assertEquals(SendTxStatus.Error, down.tx_status)
        assertEquals(SendTxErrorKey.ChainUnreachable, down.tx_error)
        assertFalse(down.can_confirm)
        // The watch follows the form's chain while the line is up.
        assertEquals(100, ChainRecovery.chainOf(down))
        val words = confirmModel(down)
        assertEquals(strings.t(I18nKeys.Flows.TX_ERROR_CHAIN_UNREACHABLE, mapOf("network" to "Gnosis")), words.notice)

        // Another network's answer is not this one's.
        m.send(SendEvent.ChainAnswered(chain_id = 1))
        assertEquals(SendTxErrorKey.ChainUnreachable, m.view().tx_error)

        m.send(SendEvent.ChainAnswered(chain_id = 100))
        val back = m.view()
        assertEquals(SendTxStatus.Idle, back.tx_status)
        assertNull(back.tx_error)
        assertTrue("the confirm opened by itself", back.can_confirm)
        assertNull("nothing left to watch", ChainRecovery.chainOf(back))
    }

    @Test
    fun `only the network's failure is watched — busy too — never another error`() {
        val busy = submitting()
        busy.answer(SendShellResult.SubmitFailed(SendSubmitFailure.ChainUnreachable(busy = true)))
        assertEquals(SendTxErrorKey.ChainBusy, busy.view().tx_error)
        assertEquals(100, ChainRecovery.chainOf(busy.view()))
        assertEquals(strings.t(I18nKeys.Flows.TX_ERROR_CHAIN_BUSY, mapOf("network" to "Gnosis")), confirmModel(busy.view()).notice)

        val other = submitting()
        other.answer(SendShellResult.SubmitFailed(SendSubmitFailure.BundlerUnderfunded))
        assertNull(ChainRecovery.chainOf(other.view()))
        other.send(SendEvent.ChainAnswered(chain_id = 100))
        assertEquals("another error is kept as it is", SendTxStatus.Error, other.view().tx_status)
    }

    // -- passkey note (send-11) -------------------------------------------------------

    @Test
    fun `a passkey closed unsigned is said calmly over the confirm, which is the way to try again`() {
        val m = submitting()
        m.send(SendEvent.SigningStarted)
        m.answer(SendShellResult.SubmitFailed(SendSubmitFailure.PasskeyCancelled))
        val closed = m.view()
        assertEquals(SendPasskeyNote.Cancelled, closed.passkey_note)
        val confirm = confirmModel(closed)
        assertEquals(strings.t(I18nKeys.Flows.PASSKEY_CLOSED), confirm.notice)
        assertTrue("calm, never the warning ink", confirm.noticeCalm)
        assertNull("no Try again: the confirm is it", confirm.noticeAction)
        assertTrue(confirm.ctaEnabled)

        val timedOut = confirmModel(closed.copy(passkey_note = SendPasskeyNote.TimedOut))
        assertEquals(strings.t(I18nKeys.Flows.PASSKEY_TIMED_OUT), timedOut.notice)
    }

    // -- send_again (send-5) ----------------------------------------------------------

    @Test
    fun `a payment proven never sent offers Send again, back to the confirm with every value kept`() {
        val m = submitting()
        m.send(SendEvent.SigningStarted)
        // A lost reply: the tracker's NotSent is what proves it never went.
        m.answer(SendShellResult.Submitted(user_op_hash = hash, now_ms = 1_757_000_000_000.0, maybe_sent = true, submit_block = null))
        assertEquals("on the receipt (waiting: ${m.script.waiting()})", SendStage.Receipt, m.view().stage)
        val held = receiptModel(m.view())
        assertNull("no way on over one that may have left", held.retry)

        m.send(SendEvent.ReceiptUpdate(hash, SendReceiptOutcome.Failed(rejected = false, not_sent = true)))
        val notSent = m.view()
        assertEquals(SendReceiptStatus.NotSent, notSent.receipt?.status)
        assertTrue(notSent.receipt!!.can_send_again)
        val receipt = receiptModel(notSent)
        assertEquals(ReceiptStage.Failed, receipt.stage)
        assertEquals(strings.t(I18nKeys.Flows.RECEIPT_NOT_SENT), receipt.title)
        assertEquals(listOf(strings.t(I18nKeys.Flows.RECEIPT_NOT_SENT_BODY)), receipt.captions)
        assertEquals(strings.t(I18nKeys.Flows.TX_SEND_AGAIN), receipt.retry)
        assertFalse("Send again moves nothing: never the accent", receipt.retryAccent)
        assertFalse(receipt.ctaAccent)

        m.send(SendEvent.SendAgain)
        val again = m.view()
        assertEquals(SendStage.Confirm, again.stage)
        assertEquals(payee, again.recipient)
        assertEquals("0.5", again.confirm_amount)
        assertEquals(SendTxStatus.Idle, again.tx_status)
        assertNull(again.receipt)
    }

    @Test
    fun `a send another of the account's transactions went before offers Review again`() {
        val m = submitting()
        m.send(SendEvent.SigningStarted)
        m.answer(SendShellResult.Submitted(user_op_hash = hash, now_ms = 1_757_000_000_000.0, maybe_sent = false, submit_block = null))
        m.send(SendEvent.ReceiptUpdate(hash, SendReceiptOutcome.Failed(rejected = true, refusal = app.getvela.wallet.feature.send.core.RefusalReason.NonceUsed)))
        val refused = m.view()
        assertTrue(refused.receipt!!.can_send_again)
        val receipt = receiptModel(refused)
        assertEquals(strings.t(I18nKeys.Flows.RECEIPT_NOT_SENT), receipt.title)
        assertEquals(listOf(strings.t(I18nKeys.Flows.SIGN_WENT_FIRST)), receipt.captions)
        assertEquals(strings.t(I18nKeys.Flows.TX_REVIEW_AGAIN), receipt.retry)
        m.send(SendEvent.SendAgain)
        assertEquals(SendStage.Confirm, m.view().stage)
    }

    @Test
    fun `Send again is never offered, nor taken, over a receipt that may have been sent`() {
        val m = submitting()
        m.send(SendEvent.SigningStarted)
        m.answer(SendShellResult.Submitted(user_op_hash = hash, now_ms = 1_757_000_000_000.0, maybe_sent = true, submit_block = null))
        assertEquals(SendReceiptStatus.MaybeSent, m.view().receipt?.status)
        assertNull(receiptModel(m.view()).retry)
        m.send(SendEvent.SendAgain)
        assertEquals("the core keeps the receipt", SendStage.Receipt, m.view().stage)
    }

    // -- retry_risk_check (round 3) ---------------------------------------------------

    @Test
    fun `a history that could not be read is said neutrally with Check again, which asks it again`() {
        var answers = 0
        var risk: SendRecipientRisk? = SendRecipientRisk(is_contract = false, first_time = null, history_unread = true)
        val m = onConfirm(risk = { answers += 1; risk })
        val unread = m.view()
        assertTrue(unread.risk_unfinished)
        assertTrue(unread.risk_history_unread)
        val confirm = confirmModel(unread)
        assertEquals(strings.t(I18nKeys.Flows.RISK_HISTORY_UNREAD), confirm.recipientCheck)
        assertEquals(strings.t(I18nKeys.Flows.RISK_RETRY), confirm.recipientCheckRetry)
        assertNull("never a first-time tag built on a failed read", confirm.recipientTag)

        val before = answers
        risk = SendRecipientRisk(is_contract = false, first_time = true)
        m.send(SendEvent.RetryRiskCheck)
        assertTrue("asked again", answers > before)
        val read = m.view()
        assertFalse(read.risk_unfinished)
        assertNull(confirmModel(read).recipientCheck)
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), confirmModel(read).recipientTag)
    }

    // -- recipient identity (r4a-18) ---------------------------------------------------

    @Test
    fun `one of the person's own accounts never reads first transfer`() {
        val m = onConfirm(risk = { SendRecipientRisk(is_contract = false, first_time = true, known_name = "Savings", own = true) })
        val own = m.view()
        assertNull("the core withholds first_time for the person's own", own.recipient_risk?.first_time)
        assertNull(confirmModel(own).recipientTag)
    }

    @Test
    fun `the recipient's cautions come before any first-time tell, on the confirm and the form`() {
        val base = onConfirm(risk = { SendRecipientRisk(is_contract = false, first_time = true) }).view()
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), confirmModel(base).recipientTag)
        val checksum = base.copy(recipient_checksum_mismatch = true)
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_CHECKSUM), confirmModel(checksum).recipientTag)
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_CHECKSUM), SendLive.recipientNote(checksum, ctx))
        val zero = base.copy(recipient_warning = app.getvela.wallet.feature.send.core.SendRecipientWarning.ZeroAddress)
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_ZERO_ADDRESS), confirmModel(zero).recipientTag)
        val self = base.copy(recipient_warning = app.getvela.wallet.feature.send.core.SendRecipientWarning.OwnAddress)
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_SELF), SendLive.recipientNote(self, ctx))
    }

    // -- the frozen fee (r7-6) ---------------------------------------------------------

    @Test
    fun `while the passkey is up the fee row is the figure being signed, never the fee machine's failure`() {
        val m = submitting()
        m.send(SendEvent.SigningStarted)
        // The fee card goes on re-quoting; a newer figure reaches the machine.
        m.send(SendEvent.FeeUpdated(estimate("999000000000000")))
        val signing = m.view()
        assertEquals(SendTxStatus.Signing, signing.tx_status)
        assertEquals("the core keeps the frozen estimate", "123000000000000", signing.fee?.total_wei)
        val failed = FeeBoards.view(FeeBoards.Case.Retrying, chainId = 100, account = safe)
        val fee = confirmModel(signing, failed).facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertNotEquals("no dash over the fee being signed", "—", fee.value)
        assertNull(fee.note)
        assertFalse(fee.tap)
        // Once the submit is over, the failure is drawn again.
        m.answer(SendShellResult.SubmitFailed(SendSubmitFailure.PasskeyCancelled))
        val after = confirmModel(m.view(), failed).facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertEquals("—", after.value)
    }

    // -- max moved (r5reg-3) -----------------------------------------------------------

    @Test
    fun `a Max filled again from a newer quote says what it read, what is sent now, and why`() {
        val moved = SendView(
            selected_token = xdai,
            stage = SendStage.Confirm,
            confirm_amount = "1.4987",
            max_moved = app.getvela.wallet.feature.send.core.SendMaxMoved(from = "1.4988", to = "1.4987"),
        )
        assertEquals(
            strings.t(I18nKeys.Flows.MAX_MOVED, mapOf("from" to "1.4988", "to" to "1.4987", "symbol" to "XDAI")),
            confirmModel(moved).maxNote,
        )
        assertNull(confirmModel(moved.copy(max_moved = null)).maxNote)
    }

    @Test
    fun `the machine says a Max moved when a newer quote changes it`() {
        val m = Machine(calm, xdai, estimate())
        m.send(SendEvent.Open(account = SendAccountRef(id = "cred", address = safe), display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)))
        m.send(SendEvent.FeeFailedChanged(false))
        m.send(SendEvent.SelectToken(SendLive.tokenId(m.view().tokens.single())))
        m.send(SendEvent.SetRecipient(payee))
        m.send(SendEvent.FeeUpdated(estimate()))
        m.send(SendEvent.TapMax)
        val first = m.view().amount
        assertTrue("Max filled ($first)", first.isNotEmpty())
        m.send(SendEvent.FeeUpdated(estimate("223000000000000")))
        val moved = m.view().max_moved
        assertNotNull("the core says the Max moved (amount ${m.view().amount})", moved)
        assertEquals(first, moved!!.from)
        assertEquals(m.view().amount, moved.to)
    }

    // -- add_network_tapped (F6 / F27) -------------------------------------------------

    @Test
    fun `a request for a network the wallet lacks offers Add this network, busy while it runs, then why it did not`() {
        val m = Machine(calm, xdai, estimate())
        m.send(
            SendEvent.Open(
                account = SendAccountRef(id = "cred", address = safe),
                display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2),
                params = SendOpenParams(prefilled_recipient = payee, prefilled_chain_id = "48900", locked = true),
            ),
        )
        val locked = m.view()
        assertEquals("locked (waiting: ${m.script.waiting()})", SendStage.LockError, locked.stage)
        val drawn = (FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick).model
        val add = SendLive.pick(drawn, locked, ctx).notice?.add
        assertNotNull(add)
        assertEquals(strings.t(I18nKeys.Flows.LOCK_ADD_NETWORK), add!!.label)
        assertEquals(48_900, add.chainId)
        assertFalse(add.busy)
        assertNull(add.failed)

        m.send(SendEvent.AddNetworkTapped(chain_id = 48_900))
        assertTrue("the core asks the network added (waiting: ${m.script.waiting()})", "add_network" in m.script.waiting())
        val adding = m.view()
        assertTrue(adding.adding_network)
        assertTrue(SendLive.pick(drawn, adding, ctx).notice!!.add!!.busy)

        m.answer(SendShellResult.NetworkAdded(SendAddNetworkOutcome.NotFound), type = "add_network")
        val notFound = m.view()
        assertEquals(SendAddNetworkMsg.NetNotFound, notFound.add_network_msg)
        val said = SendLive.pick(drawn, notFound, ctx).notice!!.add!!
        assertFalse(said.busy)
        assertEquals(strings.t(I18nKeys.Flows.LOCK_NET_NOT_FOUND), said.failed)
    }

    // -- the gallery boards -------------------------------------------------------------

    @Test
    fun `every round board draws its state`() {
        fun confirm(state: FlowState) = (FlowFixtures.build(state, strings).base as FlowBase.SendConfirm).model
        val ethereum = FlowFixtures.BOARD_CHAIN
        assertEquals(strings.t(I18nKeys.Flows.SUBMIT_SLOW, mapOf("network" to ethereum, "seconds" to "12")), confirm(FlowState.SD3L).ctaHold)
        assertEquals(strings.t(I18nKeys.Flows.SUBMIT_SLOW_BUSY, mapOf("network" to ethereum, "seconds" to "12")), confirm(FlowState.SD3M).ctaHold)
        assertEquals(strings.t(I18nKeys.Flows.TX_ERROR_CHAIN_UNREACHABLE, mapOf("network" to ethereum)), confirm(FlowState.SD3N).notice)
        assertEquals(strings.t(I18nKeys.Flows.RISK_RETRY), confirm(FlowState.SD3O).recipientCheckRetry)
        assertNotNull(confirm(FlowState.SD3P).maxNote)
        assertEquals(strings.t(I18nKeys.Flows.PASSKEY_CLOSED), confirm(FlowState.SD3Q).notice)
        val own = confirm(FlowState.SD3R)
        assertNull(own.recipientTag)
        assertEquals("Savings", own.facts[1].value)
        assertNotEquals("—", confirm(FlowState.SD3S).facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }.value)
        val receipt = (FlowFixtures.build(FlowState.SD4E, strings).base as FlowBase.SendReceipt).model
        assertEquals(strings.t(I18nKeys.Flows.TX_SEND_AGAIN), receipt.retry)
        val failed = (FlowFixtures.build(FlowState.SD1M, strings).base as FlowBase.SendPick).model.notice!!.add!!
        assertEquals(strings.t(I18nKeys.Flows.LOCK_NET_NOT_FOUND), failed.failed)
        assertTrue((FlowFixtures.build(FlowState.SD1N, strings).base as FlowBase.SendPick).model.notice!!.add!!.busy)
    }
}
