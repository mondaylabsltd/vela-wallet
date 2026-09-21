package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.ReceiptStage
import app.getvela.wallet.feature.send.core.SendAlertKind
import app.getvela.wallet.feature.send.core.SendFeeIssueView
import app.getvela.wallet.feature.send.core.SendAmountWarning
import app.getvela.wallet.feature.send.core.SendTreasuryAsset
import app.getvela.wallet.feature.send.core.SendTreasuryStatus
import app.getvela.wallet.feature.send.core.SendTxErrorKey
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.SendReceiptStatus
import app.getvela.wallet.feature.send.core.SendReceiptView
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import app.getvela.wallet.feature.send.core.SendRecipientDraft
import app.getvela.wallet.feature.send.core.SendDuplicateRowView
import app.getvela.wallet.feature.send.core.SendRowFieldState
import app.getvela.wallet.feature.send.core.SendSplitRowIssue
import app.getvela.wallet.feature.send.core.SendRecipientIdentity
import app.getvela.wallet.feature.send.core.SendRecipientRisk
import app.getvela.wallet.feature.flows.RecipientAction
import app.getvela.wallet.feature.flows.SendFormMode
import app.getvela.wallet.feature.send.core.SendMultiSpecView
import app.getvela.wallet.feature.flows.BatchUnit
import app.getvela.wallet.feature.send.core.BatchPreviewRow
import app.getvela.wallet.feature.send.core.BatchRateStatus
import app.getvela.wallet.feature.send.core.BatchRecipient
import app.getvela.wallet.feature.send.core.BatchView
import app.getvela.wallet.feature.send.core.BatchUnit as WireBatchUnit
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendScan
import org.junit.Test
import app.getvela.wallet.core.format.Formats
import app.getvela.wallet.core.format.NumberFormatKey

/**
 * The live send builders (spec 043 T031): every figure from the view, the
 * fixture only lending its labels. Each case is a number the drawn screen
 * used to show that the machine never said.
 */
class SendLiveTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val me = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val recipient = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"

    private fun ctx(currency: CurrencyView = CurrencyView(code = "USD")) = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis", 56 to "BNB Chain"),
        explorers = mapOf(100 to "https://gnosisscan.io"),
        money = WalletLive.Money.of(currency),
        fromName = "Me",
        fromAddress = me,
    )

    private val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "0.71697", decimals = 18, token_address = null, price_usd = 1.0)

    private fun fee() = FeeEstimateView(
        chain_id = 100, total_wei = "2100000000000000", max_fee_per_gas = "1", network_fee_per_gas = "1", relayer_fee_per_gas = "1",
        bundler_gas_price = "1", in_band_gas_basis = "1", total_gas = "1", deployed = true, tier = FeeTier.Fast, quoted = true,
        fee_asset = FeeAssetView.Native, fee_recipient = "0x2222222222222222222222222222222222222222",
    )

    /**
     * Issue 201: the fee was the one figure on the send screens with no money
     * beside it. The amount had its "≈" line; the fee did not, so a person who
     * does not track the coin's price could not tell what a transfer cost.
     */
    @Test
    fun `the fee row says what the fee costs, in money as well as in the coin`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val bnb = xdai.copy(chain_id = 56, symbol = "BNB", price_usd = 600.0)
        val quote = fee().copy(chain_id = 56, total_wei = "91000000000000")
        // The fee names the chain's own coin; this ctx knows chain 56 by name.
        val view = SendView(stage = SendStage.EnterDetails, selected_token = bnb, tokens = listOf(bnb), recipient = recipient, amount = "0.001", fee = quote)

        // The relay's published row prices it…
        val published = FeeView(options = listOf(feeOption(symbol = "BNB", contract = null, usdPrice = "600")))
        val live = SendLive.form(drawn.model, view, published, ctx())
        assertEquals("0.000091 BNB · ≈$0.05", live.fee.value)

        // …and with no published price, the balances the form already carries.
        assertEquals("0.000091 BNB · ≈$0.05", SendLive.form(drawn.model, view, FeeView(), ctx()).fee.value)

        // Nothing can price it ⇒ the coin alone, never an invented figure.
        val blind = view.copy(selected_token = bnb.copy(price_usd = null), tokens = listOf(bnb.copy(price_usd = null)))
        assertEquals("0.000091 BNB", SendLive.form(drawn.model, blind, FeeView(), ctx()).fee.value)

        // Under half a cent the coin amount is the honest primary.
        val dust = view.copy(fee = quote.copy(total_wei = "1000000000000"))
        assertEquals("0.000001 BNB", SendLive.form(drawn.model, dust, FeeView(), ctx()).fee.value)

        // The person's own currency, at the committed rate only.
        val eur = ctx(CurrencyView(code = "EUR", rate = 2.0, committed = true))
        // ×2 on 0.0546 USD, written with the preset's own two places.
        assertEquals("0.000091 BNB · ≈€0.10", SendLive.form(drawn.model, view, FeeView(), eur).fee.value)
    }

    /**
     * The founder's ruling of 2026-09-17: the confirm page named the coin in
     * words while every row beneath it carried art.
     */
    @Test
    fun `the confirm page draws the coin it is about to send`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val view = SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee())
        assertEquals("XDAI", SendLive.confirm(drawn.model, view, ctx()).mark?.ticker)
        // A sweep moves several coins; one mark would name the wrong one.
        assertNull(SendLive.confirm(drawn.model, view.copy(multi_select_mode = true), ctx()).mark)
    }

    private fun feeOption(symbol: String, contract: String?, usdPrice: String?) = FeeOptionView(
        symbol = symbol, contract = contract, decimals = 18, balance = "1500000000000000000",
        recipient = recipient, usd_balance = "900", usd_price = usdPrice, amount = "91000000000000",
        insufficient = false, selected = true,
    )

    @Test
    fun `the pick lists the person's holdings under the core's ids`() {
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val live = SendLive.pick(drawn.model, SendView(tokens = listOf(xdai)), ctx())
        assertEquals(1, live.rows.size)
        assertEquals("chain-100_native_XDAI", live.rows.single().id)
        assertEquals("XDAI", live.rows.single().ticker)
        assertEquals("Gnosis", live.rows.single().chain)
        assertTrue(live.rows.single().balance.startsWith("0.71697"))
        assertNull("multi-select is 045", live.selection)
    }

    @Test
    fun `the form shows what was typed, the fee the session settled, and the core's gate`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val view = SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient, amount = "0.001", token_amount = "0.001", fee = fee(), can_continue = true)
        val live = SendLive.form(drawn.model, view, FeeView(), ctx())
        assertEquals("0.001", live.amount!!.raw)
        assertEquals(recipient, live.recipient!!.raw)
        assertTrue(live.fee.value, live.fee.value.contains("0.0021") && live.fee.value.contains("xDAI"))
        assertTrue(live.ctaEnabled)
        assertEquals("the door into a split (045)", strings.t(I18nKeys.Flows.ADD_RECIPIENT), live.addRecipient)
        assertTrue(live.header.title.contains("XDAI"))

        val gated = SendLive.form(drawn.model, view.copy(can_continue = false, fee = null, estimating_gas = true), FeeView(), ctx())
        assertFalse(gated.ctaEnabled)
        assertEquals(strings.t("componentsUi.gas.estimating"), gated.fee.value)
    }

    @Test
    fun `the confirm page prints the core's confirm_amount and the settled fee`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val view = SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true)
        val live = SendLive.confirm(drawn.model, view, ctx(CurrencyView(code = "GBP", rate = 0.78, committed = true)))
        assertEquals("0.001 XDAI", live.amount)
        assertTrue(live.subline.startsWith("≈ £"))
        assertTrue(live.facts.any { it.value.contains("0x7687") })
        assertTrue(live.facts.any { it.value.contains("0.0021") })
        assertTrue(live.ctaEnabled)
        assertFalse(SendLive.confirm(drawn.model, view.copy(sending = true), ctx()).ctaEnabled)
    }

    @Test
    fun `the receipt follows the machine's stage, hash and explorer`() {
        val c = ctx()
        val submitting = SendView(stage = SendStage.Receipt, selected_token = xdai, tx_status = SendTxStatus.Signing)
        val a = SendLive.receipt((FlowFixtures.build(FlowState.SD4A, strings).base as FlowBase.SendReceipt).model, submitting, c)
        assertEquals(ReceiptStage.Submitting, a.stage)
        assertNull(a.hash)

        val submitted = submitting.copy(
            tx_status = SendTxStatus.Submitting,
            user_op_hash = "0xop",
            receipt = SendReceiptView(status = SendReceiptStatus.Submitted, amount = "0.001", usd_value = 0.0, typical_inclusion_s = 5),
        )
        val b = SendLive.receipt((FlowFixtures.build(FlowState.SD4B, strings).base as FlowBase.SendReceipt).model, submitted, c)
        assertEquals(ReceiptStage.Submitted, b.stage)
        assertTrue(b.captions.any { it.contains("Gnosis") && it.contains("5") })

        val confirmed = submitted.copy(
            tx_hash = "0x1234567890abcdef1234567890abcdef",
            receipt = SendReceiptView(status = SendReceiptStatus.Confirmed, amount = "0.001", usd_value = 0.0),
        )
        val d = SendLive.receipt((FlowFixtures.build(FlowState.SD4C, strings).base as FlowBase.SendReceipt).model, confirmed, c)
        assertEquals(ReceiptStage.Confirmed, d.stage)
        assertNotNull(d.hash)
        assertTrue(d.hash!!.value.startsWith("0x12345678"))
        assertNotNull(d.viewOnExplorer)
        assertEquals("https://gnosisscan.io/tx/0x1234567890abcdef1234567890abcdef", SendLive.explorerUrl(confirmed, c))
        assertTrue(d.title.contains("0.001") && d.title.contains("XDAI"))
    }

    /**
     * Issue 199 (web cf2a9e17): the wait counted up from a still clock and
     * said "almost there" six seconds into fifteen. With the relay's clock the
     * receipt counts DOWN inside the typical time, says "almost" only past it,
     * "slow" past twice it — and the ring eases toward full without closing.
     */
    @Test
    fun `the submitted receipt counts the wait down and fills its ring`() {
        val drawn = (FlowFixtures.build(FlowState.SD4B, strings).base as FlowBase.SendReceipt).model
        val submitted = SendView(
            stage = SendStage.Receipt, selected_token = xdai, tx_status = SendTxStatus.Submitting, user_op_hash = "0xop",
            receipt = SendReceiptView(status = SendReceiptStatus.Submitted, amount = "0.001", usd_value = 0.0, submitted_at_ms = 1_000_000.0, typical_inclusion_s = 15),
        )
        val live = SendLive.receipt(drawn, submitted, ctx())
        val eta = live.eta!!
        // The typical line moves into the counted pair, not said twice.
        assertFalse(live.captions.any { it.contains("typically") })
        assertEquals(6, eta.elapsedS(1_006_900))
        assertEquals(0, eta.elapsedS(999_000))
        assertEquals(listOf("Gnosis typically confirms in ~15s", "~9s remaining"), eta.lines(6))
        assertEquals("20s elapsed — almost there", eta.lines(20)[1])
        assertEquals(strings.t(I18nKeys.Flows.TX_SLOW_CONFIRM), eta.lines(30)[1])
        // ~70% at the typical time, never full while waiting.
        assertEquals(0.69f, eta.progress(15), 0.01f)
        assertTrue(eta.progress(0) == 0f && eta.progress(10_000) < 0.93f)
        assertTrue(eta.progress(10) < eta.progress(11))

        // Without the relay's clock there is nothing to count: the typical time, said once.
        val noClock = submitted.copy(receipt = submitted.receipt!!.copy(submitted_at_ms = null))
        val still = SendLive.receipt(drawn, noClock, ctx())
        assertNull(still.eta)
        assertTrue(still.captions.any { it.contains("Gnosis") && it.contains("15") })
        // A custom network with no typical time: no line, no ring to fill.
        assertNull(SendLive.receipt(drawn, submitted.copy(receipt = submitted.receipt!!.copy(typical_inclusion_s = null)), ctx()).eta)
    }

    @Test
    fun `the fee sheet lists the session's options with their balances`() {
        val drawn = FlowFixtures.build(FlowState.SD2F, strings).sheet as FlowSheet.FeeToken
        val fee = FeeView(
            fee = fee(),
            options = listOf(
                FeeOptionView(symbol = "XDAI", contract = null, decimals = 18, balance = "716970000000000000", recipient = "0x2", usd_balance = "0.7", amount = "2100000000000000", selected = true),
                FeeOptionView(symbol = "USDC", contract = "0x3333333333333333333333333333333333333333", decimals = 6, balance = "5000000", recipient = "0x2", usd_balance = "5", amount = "2100", selected = false),
            ),
        )
        val live = SendLive.feeSheet(drawn.model, fee, ctx())
        assertEquals(listOf("XDAI", "USDC"), live.rows.map { it.symbol })
        assertTrue(live.rows[0].selected)
        assertTrue(live.rows[1].fee.contains("0.0021") && live.rows[1].fee.contains("USDC"))
        assertTrue(live.rows[0].balanceLabel.contains("0.71697"))
    }

    /** Issue 211: a coin the core judged unable to pay is dimmed and says why, not drawn like the rest. */
    @Test
    fun `the fee sheet marks a coin that cannot pay`() {
        val drawn = FlowFixtures.build(FlowState.SD2F, strings).sheet as FlowSheet.FeeToken
        val fee = FeeView(
            fee = fee(),
            options = listOf(
                FeeOptionView(symbol = "XDAI", contract = null, decimals = 18, balance = "716970000000000000", recipient = "0x2", usd_balance = "0.7", amount = "2100000000000000", selected = true),
                FeeOptionView(symbol = "USDC", contract = "0x3333333333333333333333333333333333333333", decimals = 6, balance = "0", recipient = "0x2", usd_balance = "0", amount = "2100", insufficient = true),
            ),
        )
        val live = SendLive.feeSheet(drawn.model, fee, ctx())
        assertFalse(live.rows[0].insufficient)
        assertTrue(live.rows[1].insufficient)
        assertEquals(strings.t(I18nKeys.Flows.WARN_INSUFFICIENT_GAS, mapOf("sym" to "USDC")), live.rows[1].insufficientNote)
    }

    /** The web's `recipientNote`: "name · source", else the first-time tell, else nothing. */
    @Test
    fun `the recipient note names the source of the name`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val base = SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient)
        fun note(view: SendView) = SendLive.form(drawn.model, view, FeeView(), ctx()).recipient!!.note
        assertEquals("alice.eth · ENS", note(base.copy(recipient_identity = SendRecipientIdentity(name = "alice.eth", source = "ENS"))))
        assertEquals("Alice", note(base.copy(recipient_identity = SendRecipientIdentity(name = "Alice"))))
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), note(base.copy(recipient_risk = SendRecipientRisk(first_time = true))))
        assertNull(note(base))
    }

    /** Issue 209: a filter that hid every row is a different sentence from an empty account. */
    @Test
    fun `an empty pick says why it is empty`() {
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        assertEquals(strings.t(I18nKeys.Flows.NO_TOKENS_WITH_BALANCE), SendLive.pick(drawn.model, SendView(), ctx()).empty)
        val hidden = SendLive.pick(drawn.model, SendView(tokens = listOf(xdai)), ctx(), classFilter = "stable")
        assertTrue(hidden.rows.isEmpty())
        assertEquals(strings.t(I18nKeys.Flows.NO_MATCHING_TOKENS), hidden.empty)
    }

    /** Issue 231: the web's `unitAdornment` — a sign leads, a code or a ticker follows, nothing defaults to "$". */
    @Test
    fun `the amount's unit sits on the figure`() {
        assertEquals(null to "BNB", SendLive.unitAdornment(null, "BNB"))
        assertEquals(null to null, SendLive.unitAdornment(null, ""))
        assertEquals("$" to null, SendLive.unitAdornment("USD", "BNB"))
        assertEquals("€" to null, SendLive.unitAdornment("EUR", "BNB"))
        assertEquals(null to "PLN", SendLive.unitAdornment("PLN", "BNB"))
        assertEquals(null to "XYZ", SendLive.unitAdornment("XYZ", "BNB"))
        // The ⇄ row obeys the core's two flags, and the typed unit is the figure's.
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val view = SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient, amount = "1", token_amount = "1")
        val hidden = SendLive.form(drawn.model, view, FeeView(), ctx()).amount!!
        assertFalse(hidden.denomShown)
        assertEquals("XDAI", hidden.denomLabel)
        val dimmed = SendLive.form(drawn.model, view.copy(denom_toggle_shown = true), FeeView(), ctx()).amount!!
        assertTrue(dimmed.denomShown)
        assertFalse(dimmed.denomEnabled)
    }

    @Test
    fun `the flow state is the stage, plus the sheet the shell opened`() {
        assertEquals(FlowState.SD1, SendLive.flowState(SendView(), feeSheetOpen = false))
        assertEquals(FlowState.SD2, SendLive.flowState(SendView(stage = SendStage.EnterDetails), false))
        assertEquals(FlowState.SD2E, SendLive.flowState(SendView(stage = SendStage.EnterDetails, show_contact_picker = true), false))
        assertEquals(FlowState.SD2F, SendLive.flowState(SendView(stage = SendStage.EnterDetails), true))
        assertEquals(FlowState.SD3, SendLive.flowState(SendView(stage = SendStage.Confirm), false))
        assertEquals(FlowState.SD4A, SendLive.flowState(SendView(stage = SendStage.Receipt), false))
        assertEquals(FlowState.SD4C, SendLive.flowState(SendView(stage = SendStage.Receipt, receipt = SendReceiptView(status = SendReceiptStatus.Confirmed, amount = "1", usd_value = 0.0)), false))
    }

    // -- phase 5: refusals in the core's words --------------------------------

    @Test
    fun `the form's warning is the core's amount warning, or the same-asset fee ceiling`() {
        val short = SendView(stage = SendStage.EnterDetails, selected_token = xdai, amount_warning = SendAmountWarning.NotEnoughToken("XDAI"))
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY), SendLive.formWarning(short, ctx()))
        val gas = SendView(stage = SendStage.EnterDetails, selected_token = xdai, amount_warning = SendAmountWarning.NeedGas("XDAI"))
        assertTrue(SendLive.formWarning(gas, ctx())!!.contains("XDAI"))
        // #210: the fee alone outruns the balance — the state `Max` fills 0 for.
        val overFee = SendView(stage = SendStage.EnterDetails, selected_token = xdai, amount_warning = SendAmountWarning.InsufficientGas("XDAI"))
        assertEquals(strings.t(I18nKeys.Flows.WARN_INSUFFICIENT_GAS, mapOf("sym" to "XDAI")), SendLive.formWarning(overFee, ctx()))
        assertNull(SendLive.formWarning(SendView(stage = SendStage.EnterDetails, selected_token = xdai), ctx()))
        // Device-found: the core's figures are base units; the sentence must not be.
        val ceiling = SendView(
            stage = SendStage.EnterDetails, selected_token = xdai,
            same_asset_fee_issue = SendFeeIssueView(symbol = "XDAI", transfer_amount = "5000000000000000000", balance = "628970000000000000", fee_amount = "10000000000000000", total = "5010000000000000000", max_transfer_amount = "618970000000000000"),
        )
        val sentence = SendLive.formWarning(ceiling, ctx())!!
        assertTrue(sentence, sentence.contains("0.61897") && sentence.contains("0.01") && !sentence.contains("000000000"))
    }

    @Test
    fun `half-typed text gets the placeholder identicon, a real address its own`() {
        val drawn = (FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model
        val typing = SendLive.form(drawn, SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = "0xabc"), FeeView(), ctx())
        assertEquals("0x0000000000000000000000000000000000000000", typing.recipient!!.identiconSeed)
        val done = SendLive.form(drawn, SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient), FeeView(), ctx())
        assertEquals(recipient, done.recipient!!.identiconSeed)
    }

    @Test
    fun `the confirm page names what stopped it and offers the one action`() {
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val refused = SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true, tx_status = SendTxStatus.Error, tx_error = SendTxErrorKey.BundlerFund)
        val confirm = SendLive.confirm(drawn, refused, ctx())
        assertEquals(strings.t(I18nKeys.Flows.TX_ERROR_BUNDLER_FUND), confirm.notice)
        assertEquals(strings.t(I18nKeys.Flows.TX_RETRY), confirm.noticeAction)
        assertFalse(confirm.ctaEnabled)

        val low = SendView(
            stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true,
            treasury_bootstrap = SendTreasuryStatus(chain_id = 100, address = "0x1111111111111111111111111111111111111111", asset = SendTreasuryAsset.Native, balance = "100000000000000000", floor = "1000000000000000000", bootstrap_needed = true),
        )
        val treasury = SendLive.confirm(drawn, low, ctx())
        assertTrue(treasury.notice!!.contains(strings.t(I18nKeys.Flows.TREASURY_TITLE)))
        assertTrue("the hint carries the shortfall in the chain's coin", treasury.notice!!.contains("0.9"))
        assertEquals(strings.t(I18nKeys.Flows.TREASURY_RETRY), treasury.noticeAction)
        assertFalse(treasury.ctaEnabled)

        val fine = SendLive.confirm(drawn, SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true), ctx())
        assertNull(fine.notice)
        assertTrue(fine.ctaEnabled)
    }

    @Test
    fun `while the prompt is up the confirm page offers Cancel and nothing else`() {
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val signing = SendLive.confirm(drawn, SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true, sending = true, tx_status = SendTxStatus.Signing), ctx())
        assertEquals(strings.t(I18nKeys.Flows.TX_PREPARING_BIOMETRIC), signing.notice)
        assertEquals(strings.t(I18nKeys.Flows.CANCEL), signing.noticeAction)
        assertFalse(signing.ctaEnabled)
    }

    @Test
    fun `every alert kind has a title in the corpus`() {
        val kinds = listOf(
            SendAlertKind.InvalidAddress, SendAlertKind.InvalidAmount, SendAlertKind.InsufficientBalance(SendAmountWarning.NeedGas("XDAI")),
            SendAlertKind.SplitOverBalance, SendAlertKind.LoadTokensFailed, SendAlertKind.AccountUnavailable,
        )
        kinds.forEach { kind ->
            val (title, _) = SendLive.alertText(kind, strings)
            assertTrue("$kind has words", title.isNotBlank() && !title.startsWith("send.") && !title.startsWith("componentsUi."))
        }
        val (_, body) = SendLive.alertText(SendAlertKind.InsufficientBalance(SendAmountWarning.NeedGas("XDAI")), strings)
        assertTrue(body.contains("XDAI"))
    }

    // -- Spec 045 US1: the split --------------------------------------------

    private val splitView = SendView(
        stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, split_mode = true,
        recipients = listOf(
            SendRecipientDraft("rcpt_1", recipient, "0.001", "Founder"),
            SendRecipientDraft("rcpt_2", me, "0.001"),
            SendRecipientDraft("rcpt_3", "", ""),
        ),
        confirm_amount = "0.002", can_continue = false,
    )

    @Test
    fun `the split form draws the core's rows, editable, and its total`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val live = SendLive.form(drawn.model, splitView, FeeView(), ctx())
        assertEquals(SendFormMode.Split, live.mode)
        assertNull(live.amount)
        assertNull(live.recipient)
        assertNull(live.addRecipient)
        assertEquals(3, live.recipients.size)
        assertEquals("Founder", live.recipients[0].name)
        assertEquals("rcpt_1", live.recipients[0].id)
        assertEquals("0.001", live.recipients[0].amountValue)
        assertEquals("0.001 XDAI", live.recipients[0].amount)
        assertEquals("0x88cC…6894", live.recipients[1].name)
        assertEquals("", live.recipients[2].name)
        assertEquals("0x0000000000000000000000000000000000000000", live.recipients[2].identiconSeed)
        assertEquals(listOf(RecipientAction.Add, RecipientAction.Contacts, RecipientAction.Import), live.recipientActions.map { it.id })
        assertEquals("0.002 XDAI · ≈ $0.00", live.summary?.value)
        assertTrue(live.summary!!.label.contains("3"))
        assertFalse(live.ctaEnabled)
    }

    /**
     * Issues 203–206: a dark Continue with forty rows said nothing. The core
     * says which row needs what, which row repeats which, whether the rows
     * outrun the balance and how much is left; the form words all four.
     */
    @Test
    fun `the split form says why Continue is dark`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val view = splitView.copy(
            recipients = listOf(
                SendRecipientDraft("rcpt_1", recipient, "0.1"),
                SendRecipientDraft("rcpt_2", recipient, "0.2"),
                SendRecipientDraft("rcpt_3", "0x1234", "1,5"),
                SendRecipientDraft("rcpt_4", "", ""),
            ),
            split_duplicates = listOf(SendDuplicateRowView("rcpt_2", 1)),
            split_row_issues = listOf(
                SendSplitRowIssue("rcpt_3", 3, SendRowFieldState.Invalid, SendRowFieldState.Invalid),
                SendSplitRowIssue("rcpt_4", 4, SendRowFieldState.Empty, SendRowFieldState.Empty),
            ),
            split_remaining = "0.41697",
            confirm_amount = "",
        )
        val live = SendLive.form(drawn.model, view, FeeView(), ctx())

        assertNull(live.recipients[0].duplicateNote)
        assertEquals("Same address as recipient 1", live.recipients[1].duplicateNote)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_BAD_ADDRESS), live.recipients[2].addressNote)
        assertEquals(strings.t(I18nKeys.Flows.BAD_AMOUNT), live.recipients[2].amountNote)
        // An empty field is unfinished, not wrong.
        assertNull(live.recipients[3].addressNote)
        assertNull(live.recipients[3].amountNote)
        // The first unfinished row, and what it needs.
        assertEquals("Recipient 3 needs an address.", live.hint)
        assertEquals("0.41697 XDAI left", live.summary?.remaining)
        assertFalse(live.summary!!.over)
        assertNull(live.warning)

        // Only the amount missing names the amount; a busy pre-check says nothing.
        val needsAmount = view.copy(split_row_issues = listOf(SendSplitRowIssue("rcpt_4", 4, SendRowFieldState.Ok, SendRowFieldState.Empty)))
        assertEquals("Recipient 4 needs an amount.", SendLive.form(drawn.model, needsAmount, FeeView(), ctx()).hint)
        assertNull(SendLive.form(drawn.model, needsAmount.copy(estimating_gas = true), FeeView(), ctx()).hint)

        // Over the balance: the live refusal, the figure in the refusal colour,
        // no "left" — and never the single form's stale amount warning.
        val over = view.copy(split_row_issues = emptyList(), split_over_balance = true, split_remaining = null, amount_warning = SendAmountWarning.NeedGas("XDAI"))
        val overLive = SendLive.form(drawn.model, over, FeeView(), ctx())
        assertTrue(overLive.summary!!.over)
        assertNull(overLive.summary!!.remaining)
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY), overLive.warning)
        assertNull(overLive.hint)
    }

    @Test
    fun `a single form offers the door into a split`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val live = SendLive.form(drawn.model, SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai), FeeView(), ctx())
        assertEquals(SendFormMode.Single, live.mode)
        assertEquals(strings.t(I18nKeys.Flows.ADD_RECIPIENT), live.addRecipient)
        assertTrue(live.recipients.isEmpty())
        assertNull(live.summary)
    }

    @Test
    fun `the split's confirm names the count and every person below it`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val view = splitView.copy(stage = SendStage.Confirm, recipients = splitView.recipients.take(2), can_confirm = true, fee = fee())
        val live = SendLive.confirm(drawn.model, view, ctx())
        assertEquals("0.002 XDAI", live.amount)
        val to = live.facts.first { it.label == strings.t(I18nKeys.Flows.TO_LABEL) }
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to "2")), to.value)
        assertEquals(2, live.breakdown.size)
        assertEquals("Founder", live.breakdown[0].label)
        assertEquals("0.001 XDAI", live.breakdown[0].value)
        assertEquals(recipient, live.breakdown[0].identiconSeed)
        assertEquals("0x88cC…6894", live.breakdown[1].label)
    }

    // -- Spec 045 US2: the sweep pick and form -------------------------------

    private val usdc = SendToken(network = "chain-100", chain_id = 100, symbol = "USDC", balance = "3", decimals = 6, token_address = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83", price_usd = 1.0)
    private val eth = SendToken(network = "chain-1", chain_id = 1, symbol = "ETH", balance = "0.01", decimals = 18, token_address = null, price_usd = 3000.0)

    @Test
    fun `the pick offers the sweep door, then ticks, dims and counts once picking`() {
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val tokens = listOf(xdai, usdc, eth)
        val plain = SendLive.pick(drawn.model, SendView(tokens = tokens), ctx())
        assertNull(plain.selection)
        assertNull(plain.notice)
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_TITLE), plain.cta.label)
        assertFalse(plain.cta.accent)

        val unpinned = SendLive.pick(drawn.model, SendView(tokens = tokens), ctx(), sweepPicking = true)
        assertEquals(listOf(false, false, false), unpinned.selection!!.selected)
        assertEquals(listOf(false, false, false), unpinned.selection!!.dimmed)
        assertNull(unpinned.notice)
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_TITLE), unpinned.header.title)

        val pinned = SendLive.pick(
            drawn.model,
            SendView(tokens = tokens, multi_chain_id = 100, multi_selected_ids = listOf(SendLive.tokenId(xdai))),
            ctx(),
            sweepPicking = true,
        )
        assertEquals(listOf(true, false, false), pinned.selection!!.selected)
        assertEquals(listOf(false, false, true), pinned.selection!!.dimmed)
        assertTrue(pinned.notice!!.text.contains("Gnosis"))
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_CONTINUE, mapOf("n" to "1", "chain" to "Gnosis")), pinned.cta.label)
        assertTrue(pinned.cta.accent)
        assertEquals(strings.t(I18nKeys.Flows.SELECT_ALL_VALUABLE), pinned.selection!!.selectAll)
    }

    @Test
    fun `the sweep form lists the picked rows with the core's amounts and one recipient`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val view = SendView(
            stage = SendStage.EnterDetails, tokens = listOf(xdai, usdc, eth), selected_token = xdai,
            multi_select_mode = true, multi_chain_id = 100,
            multi_selected_ids = listOf(SendLive.tokenId(xdai), SendLive.tokenId(usdc)),
            multi_specs = listOf(SendMultiSpecView(token_address = null, decimals = 18, amount = "0.4")),
            recipient = recipient, can_continue = true,
        )
        val live = SendLive.form(drawn.model, view, FeeView(), ctx())
        assertEquals(SendFormMode.Sweep, live.mode)
        assertNull(live.token)
        assertNull(live.amount)
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_SUMMARY, mapOf("n" to "2", "chain" to "Gnosis")), live.sweepSummary)
        assertEquals(listOf("XDAI", "USDC"), live.sweepRows.map { it.symbol })
        assertEquals("0.4", live.sweepRows[0].amount)
        assertEquals("3", live.sweepRows[1].amount)
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_SAME_RECIPIENT), live.recipient!!.note)
        assertEquals(recipient, live.recipient!!.raw)
        assertTrue(live.ctaEnabled)
    }

    // -- Spec 045 US3: the batch sheet ---------------------------------------

    @Test
    fun `the batch sheet says what the core parsed, priced and gated`() {
        val drawn = FlowFixtures.build(FlowState.SD2C, strings).sheet as FlowSheet.BatchImport
        val view = SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, split_mode = true, show_batch_import = true)
        val loading = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Fiat, fiat_code = "GBP", rate_status = BatchRateStatus.Loading), view, ctx())
        assertEquals(BatchUnit.Fiat, loading.unit)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_RATE_LOADING), loading.rateValue)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_APPLY_EMPTY), loading.cta)
        assertTrue(loading.ctaDisabled)
        assertTrue(loading.rows.isEmpty())

        val priced = SendLive.batchImport(
            drawn.model,
            BatchView(
                opened = true, unit = WireBatchUnit.Token, fiat_code = "GBP", raw_text = "a,1", rate_status = BatchRateStatus.Ok, rate_input = "0.78", rate_edited = true,
                preview = listOf(
                    BatchPreviewRow(line = 1, name = "Founder", address = recipient, valid = true, raw_amount = "0.001", token_amount = "0.001", ok = true),
                    BatchPreviewRow(line = 2, address = "0x12zz", valid = false, raw_amount = "5", token_amount = "", ok = false),
                ),
                rejected = 1, recipient_count = 1, total_token = "0.001", can_apply = true,
                recipients = listOf(BatchRecipient(recipient, "0.001", "Founder")),
            ),
            view, ctx(),
        )
        assertEquals(BatchUnit.Token, priced.unit)
        assertEquals("0.78 GBP", priced.rateValue)
        assertEquals("0.78", priced.rateInput)
        assertTrue(priced.rateEdited)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_PARSED_COUNT, mapOf("n" to "1")), priced.parsedLabel)
        assertEquals(listOf(true, false), priced.rows.map { it.ok })
        assertEquals("Founder", priced.rows[0].address)
        assertEquals("0.001 XDAI", priced.rows[0].conversion)
        assertEquals("5", priced.rows[1].conversion)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_REJECTED_ONE, mapOf("count" to "1")), priced.rejectedText)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_APPLY_ONE, mapOf("count" to "1")), priced.cta)
        assertFalse(priced.ctaDisabled)
        assertEquals(FlowState.SD2C, SendLive.flowState(view, feeSheetOpen = false))
    }

    /** Issue #272: the refusal that dims the button reads as a warning, not as helper text. */
    @Test
    fun `an over-balance total is a warning, a saved template is not`() {
        val drawn = FlowFixtures.build(FlowState.SD2C, strings).sheet as FlowSheet.BatchImport
        val view = SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, show_batch_import = true)

        val over = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Token, over_balance = true, recipient_count = 3), view, ctx())
        assertEquals(strings.t(I18nKeys.Flows.BATCH_OVER_BALANCE, mapOf("sym" to "XDAI")), over.note)
        assertTrue(over.noteWarning)
        assertTrue(over.ctaDisabled)

        val saved = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Token, template_saved = true), view, ctx())
        assertEquals(strings.t(I18nKeys.Flows.BATCH_TEMPLATE_SAVED), saved.note)
        assertFalse(saved.noteWarning)
    }

    /** Issue #271: with someone already on the form, the sheet says the import ADDS — and offers the other. */
    @Test
    fun `the batch sheet says whether an import adds to or replaces the form's rows`() {
        val drawn = FlowFixtures.build(FlowState.SD2C, strings).sheet as FlowSheet.BatchImport
        val ready = BatchView(opened = true, unit = WireBatchUnit.Token, recipient_count = 1, can_apply = true, recipients = listOf(BatchRecipient(recipient, "0.001", null)))

        val empty = SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, split_import_room = 60)
        assertNull("nobody on the form: nothing to add to", SendLive.batchImport(drawn.model, ready, empty, ctx()).merge)

        val typed = empty.copy(recipient = recipient, split_import_room = 59)
        val adds = SendLive.batchImport(drawn.model, ready, typed, ctx())
        assertEquals(strings.t(I18nKeys.Flows.BATCH_ADDS_TO_ROWS), adds.merge)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_REPLACE_INSTEAD), adds.mergeAction)

        val replaces = SendLive.batchImport(drawn.model, ready, typed, ctx(), replaces = true)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_REPLACES_ROWS), replaces.merge)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_ADD_INSTEAD), replaces.mergeAction)
    }

    // -- Spec 045 US4: the treasury pause's second exit ---------------------

    @Test
    fun `the treasury pause offers retry and not-now, with the facts kept`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val paused = SendView(
            stage = SendStage.Confirm, tokens = listOf(xdai), selected_token = xdai, recipient = recipient, confirm_amount = "0.001",
            fee = fee(), can_confirm = true,
            treasury_bootstrap = SendTreasuryStatus(chain_id = 100, address = me, asset = SendTreasuryAsset.Native, balance = "0", floor = "1000000000000000000", bootstrap_needed = true),
        )
        val live = SendLive.confirm(drawn.model, paused, ctx())
        assertEquals(strings.t(I18nKeys.Flows.TREASURY_RETRY), live.noticeAction)
        assertEquals(strings.t(I18nKeys.Flows.FUNDING_CANCEL), live.noticeSecondary)
        assertFalse(live.ctaEnabled)
        assertEquals(4, live.facts.size)
        val resumed = SendLive.confirm(drawn.model, paused.copy(treasury_bootstrap = null), ctx())
        assertNull(resumed.notice)
        assertNull(resumed.noticeSecondary)
        assertTrue(resumed.ctaEnabled)
    }

    // -- Spec 046 US3: the scanner ------------------------------------------

    @Test
    fun `the scanner is its own state while the core's flag is up, and a decode becomes the core's scan`() {
        assertEquals(FlowState.S1, SendLive.flowState(SendView(stage = SendStage.SelectToken, show_scanner = true), feeSheetOpen = false))
        assertEquals(FlowState.S1, SendLive.flowState(SendView(stage = SendStage.EnterDetails, show_scanner = true), feeSheetOpen = false))
        assertEquals(FlowState.SD1, SendLive.flowState(SendView(stage = SendStage.SelectToken), feeSheetOpen = false))
        val request = SendController.scanOf("ethereum:$recipient@100?value=1000000000000000") as SendScan.Request
        assertEquals(recipient, request.recipient)
        assertEquals(100, request.chain_id)
        assertEquals("1000000000000000", request.amount_base_units)
        assertNull(request.token_address)
        val text = SendController.scanOf("  $recipient ") as SendScan.Text
        assertEquals(recipient, text.data)
        assertTrue(SendController.scanOf("ethereum:0xddafbb505ad214d7b80b1f830fccc89b60fb7a83@1/approve?address=$recipient&uint256=1") is SendScan.Text)
    }

    /** Spec 049: every drawn amount takes the preset's mark; the editable field keeps its raw digits. */
    @Test
    fun `the form and the confirm follow the number preset, the raw amount does not`() {
        val saved = Formats.current
        Formats.current = Formats(NumberFormatKey.DotComma)
        try {
            val form = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
            val view = SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient, amount = "0.001", token_amount = "0.001", fee = fee(), can_continue = true)
            val live = SendLive.form(form.model, view, FeeView(), ctx())
            assertEquals("0.001", live.amount!!.raw)
            assertTrue(live.token!!.detail, live.token!!.detail.contains("0,71697"))
            assertTrue(live.fee.value, live.fee.value.contains("0,0021"))

            val confirmDrawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
            val confirm = SendLive.confirm(confirmDrawn.model, SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(), can_confirm = true), ctx())
            assertEquals("0,001 XDAI", confirm.amount)
        } finally {
            Formats.current = saved
        }
    }
}
