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
import app.getvela.wallet.feature.send.core.SplitRows
import app.getvela.wallet.feature.send.core.SendRecipientIdentity
import app.getvela.wallet.feature.send.core.SendNameSource
import app.getvela.wallet.feature.send.core.SendPayee
import app.getvela.wallet.feature.send.core.SendReceiptCoin
import app.getvela.wallet.feature.send.core.SendReceiptKind
import app.getvela.wallet.feature.send.core.SendReceiptTransfer
import app.getvela.wallet.feature.flows.FactLead
import app.getvela.wallet.feature.send.core.SendRecipientRisk
import app.getvela.wallet.feature.flows.RecipientAction
import app.getvela.wallet.feature.flows.SendFormMode
import app.getvela.wallet.feature.send.core.SendMultiSpecView
import app.getvela.wallet.feature.flows.BatchUnit
import app.getvela.wallet.feature.send.core.BatchPreviewRow
import app.getvela.wallet.feature.send.core.BatchParseError
import app.getvela.wallet.feature.send.core.BatchParseReason
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
        // ×2 on 0.0546 USD = 0.1092, written with the preset's own two places,
        // rounded half up like every money figure (spec 078 round 2).
        assertEquals("0.000091 BNB · ≈€0.11", SendLive.form(drawn.model, view, FeeView(), eur).fee.value)
    }

    /**
     * Spec 078: the balance beside the token on the form is the asset list's
     * row, digit for digit — the one token-amount rule, called on the same
     * holding — and a Max's exact figure (balance less a fee to the wei) is
     * never printed whole on the confirm page or the receipt.
     */
    @Test
    fun `the token card and the confirm write the asset list's figure, never eighteen digits`() {
        val exact = "0.043790209243313861"
        val eth = xdai.copy(symbol = "ETH", balance = "0.0439686")
        val home = WalletLive.home(
            app.getvela.wallet.feature.wallet.WalletFixtures.buildMobileState(app.getvela.wallet.feature.wallet.WalletScreenState.H1, strings),
            app.getvela.wallet.feature.wallet.core.BalanceView(
                display_total_usd = 1.0,
                tokens = listOf(app.getvela.wallet.feature.wallet.core.BalanceToken(chain_id = 100, symbol = "ETH", name = "ETH", balance = eth.balance, decimals = 18, price_usd = 1.0)),
            ),
            app.getvela.wallet.feature.wallet.core.FeedView(),
            CurrencyView(code = "USD"),
            strings,
            mapOf(100 to "Gnosis"),
        )
        val row = home.assetRows.single().balance
        assertEquals("0.043969 ETH", row)

        val form = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val onForm = SendView(stage = SendStage.EnterDetails, selected_token = eth, tokens = listOf(eth), amount = "0.04379", token_amount = exact)
        val card = SendLive.form(form.model, onForm, FeeView(), ctx()).token!!.detail
        assertTrue(card, card.endsWith(" 0.043969"))
        assertEquals(row.substringBefore(" "), card.substringAfterLast(" "))

        val sd3 = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val confirming = SendView(stage = SendStage.Confirm, selected_token = eth, recipient = recipient, confirm_amount = exact, token_amount = exact, fee = fee())
        val headline = SendLive.confirm(sd3.model, confirming, ctx())
        assertEquals("0.04379", headline.amount)
        // The unit is its own piece beside the figure (spec 078 round 2).
        assertEquals("ETH", headline.amountUnit)
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

    /**
     * Device-found: the core resolves `first_time` only while the confirm page
     * is up (`confirm_probes`), so the form's note never had it. The page that
     * signs says it.
     */
    @Test
    fun `the confirm page says it is the first time sending to this address`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val view = SendView(stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee())
        assertNull(SendLive.confirm(drawn.model, view, ctx()).recipientTag)
        val first = view.copy(recipient_risk = SendRecipientRisk(first_time = true))
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), SendLive.confirm(drawn.model, first, ctx()).recipientTag)
        assertNull(SendLive.confirm(drawn.model, view.copy(recipient_risk = SendRecipientRisk(first_time = false)), ctx()).recipientTag)
    }

    /**
     * Spec 096 F12: the WBNB-to-the-WBNB-contract send said only "First time
     * sending here". The core's token-contract verdict goes first, on the
     * confirm and on the form, where it is drawn as a warning.
     */
    @Test
    fun `a token contract recipient is said before the slide`() {
        val confirmDrawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val token = SendView(
            stage = SendStage.Confirm, selected_token = xdai, recipient = recipient, confirm_amount = "0.001", fee = fee(),
            recipient_risk = SendRecipientRisk(first_time = true), recipient_is_token_contract = true,
        )
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_TOKEN_CONTRACT), SendLive.confirm(confirmDrawn.model, token, ctx()).recipientTag)

        val formDrawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val field = SendLive.form(formDrawn.model, token.copy(stage = SendStage.EnterDetails), FeeView(), ctx()).recipient!!
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_TOKEN_CONTRACT), field.note)
        assertTrue(field.noteWarning)
        val plain = SendLive.form(formDrawn.model, token.copy(stage = SendStage.EnterDetails, recipient_is_token_contract = false), FeeView(), ctx()).recipient!!
        assertFalse(plain.noteWarning)
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
        assertEquals("0.001", live.amount)
        assertEquals("XDAI", live.amountUnit)
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
     * Spec 097 F (S3): a two-coin sweep's success screen said "Send ETH |
     * Sent 0.000418 ETH | To Wallet · Base" although 0.034929 USDC moved in
     * the same operation (Android captioned it "2 recipients" besides — a
     * sweep is ONE recipient and N coins). The receipt lists every coin.
     */
    @Test
    fun `a sweep's receipt lists every coin it sent and names none alone`() {
        val baseEth = SendToken(network = "chain-8453", chain_id = 8453, symbol = "ETH", balance = "0.001", decimals = 18, token_address = null, price_usd = 2400.0)
        val c = SendLive.Context(strings, mapOf(8453 to "Base"), emptyMap(), WalletLive.Money.of(CurrencyView(code = "USD")), "Me", me)
        val wallet = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        val coins = listOf(
            SendReceiptCoin(amount = "0.000418", symbol = "ETH", token_address = null, usd_value = 1.0),
            SendReceiptCoin(amount = "0.034929", symbol = "USDC", token_address = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", usd_value = 0.03),
        )
        val confirmed = SendView(
            stage = SendStage.Receipt, selected_token = baseEth, recipient = wallet, multi_select_mode = true,
            tx_status = SendTxStatus.Confirmed, tx_hash = "0x1234567890abcdef1234567890abcdef",
            receipt = SendReceiptView(
                status = SendReceiptStatus.Confirmed, kind = SendReceiptKind.MultiSelect,
                transfers = listOf(
                    SendReceiptTransfer(to = wallet, to_name = "Wallet", amount = "0.000418", symbol = "ETH", usd_value = 1.0),
                    SendReceiptTransfer(to = wallet, to_name = "Wallet", amount = "0.034929", symbol = "USDC", usd_value = 0.03),
                ),
                coins = coins, amount = "", usd_value = 1.03,
            ),
        )
        val d = SendLive.receipt((FlowFixtures.build(FlowState.SD4C, strings).base as FlowBase.SendReceipt).model, confirmed, c)
        assertEquals("Send tokens", d.header.title)
        assertEquals(strings.t(I18nKeys.Flows.TX_SENT), d.title)
        assertFalse(d.title.contains("0.000418"))
        assertEquals(listOf("To Wallet · Base", "0.000418 ETH", "0.034929 USDC"), d.captions)

        // Waiting on the chain, the same coins are on the screen.
        val submitted = confirmed.copy(
            tx_status = SendTxStatus.Submitting, tx_hash = null,
            receipt = confirmed.receipt!!.copy(status = SendReceiptStatus.Submitted),
        )
        val b = SendLive.receipt((FlowFixtures.build(FlowState.SD4B, strings).base as FlowBase.SendReceipt).model, submitted, c)
        assertEquals("Send tokens", b.header.title)
        assertTrue(b.captions.containsAll(listOf("0.000418 ETH", "0.034929 USDC")))

        // A sweep whose native line the gas reserve dropped sent one coin: the
        // title names it, and no coin line repeats it.
        val one = confirmed.copy(
            receipt = confirmed.receipt!!.copy(
                transfers = confirmed.receipt!!.transfers.drop(1),
                coins = coins.drop(1),
                amount = "0.034929",
                usd_value = 0.03,
            ),
        )
        val o = SendLive.receipt((FlowFixtures.build(FlowState.SD4C, strings).base as FlowBase.SendReceipt).model, one, c)
        assertEquals("Sent 0.034929 USDC", o.title)
        assertEquals(listOf("To Wallet · Base"), o.captions)
    }

    /**
     * Spec 097 F (S3): a split's headline is its TOTAL — the core's one coin,
     * summed over the rows it signed — never the form's live figure, never a
     * single row.
     */
    @Test
    fun `a split's receipt title is the total the core summed`() {
        val wallet = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        val confirmed = SendView(
            stage = SendStage.Receipt, selected_token = xdai, split_mode = true,
            // What the form says now is not what was signed; the receipt is.
            confirm_amount = "0.001",
            tx_status = SendTxStatus.Confirmed, tx_hash = "0x1234567890abcdef1234567890abcdef",
            receipt = SendReceiptView(
                status = SendReceiptStatus.Confirmed, kind = SendReceiptKind.Split,
                transfers = listOf(
                    SendReceiptTransfer(to = recipient, to_name = "Founder", amount = "0.001", symbol = "XDAI", usd_value = 0.001),
                    SendReceiptTransfer(to = wallet, amount = "0.0025", symbol = "XDAI", usd_value = 0.0025),
                ),
                coins = listOf(SendReceiptCoin(amount = "0.0035", symbol = "XDAI", usd_value = 0.0035)),
                amount = "0.0035", usd_value = 0.0035,
            ),
        )
        val d = SendLive.receipt((FlowFixtures.build(FlowState.SD4C, strings).base as FlowBase.SendReceipt).model, confirmed, ctx())
        assertEquals("Send XDAI", d.header.title)
        assertEquals("Sent 0.0035 XDAI", d.title)
        assertEquals(
            listOf("2 recipients · Gnosis", "Founder · 0.001 XDAI", "0x14fB…eA5c · 0.0025 XDAI"),
            d.captions,
        )
    }

    /**
     * Spec 082 RA10 (owner ruling 1): a lost reply is "Submitting…", it may
     * have been sent, with the op hash and a close that keeps it running —
     * never a failure, never a Retry. "Not sent" (the relay never had it) is
     * the plain failure, with no hash and no explorer.
     */
    @Test
    fun `a lost reply may have been sent and a never-sent op is the plain failure`() {
        val c = ctx()
        val drawn = (FlowFixtures.build(FlowState.SD4B, strings).base as FlowBase.SendReceipt).model
        val op = "0x" + "7a".repeat(32)
        val maybe = SendView(
            stage = SendStage.Receipt, selected_token = xdai, tx_status = SendTxStatus.Submitting, user_op_hash = op,
            receipt = SendReceiptView(status = SendReceiptStatus.MaybeSent, amount = "0.001", usd_value = 0.0, typical_inclusion_s = 5),
        )
        val m = SendLive.receipt(drawn, maybe, c)
        assertEquals(ReceiptStage.Submitting, m.stage)
        assertEquals(strings.t(I18nKeys.Flows.TX_SUBMITTING), m.title)
        assertEquals(listOf(strings.t("componentsUi.signing.maybeSent")), m.captions)
        assertEquals(op, m.hash?.copyValue)
        assertEquals(strings.t(I18nKeys.Flows.TX_CLOSE_BACKGROUND), m.cta)
        assertNull("nothing to show on an explorer yet", m.viewOnExplorer)
        assertEquals(FlowState.SD4B, SendLive.flowState(maybe, false))
        assertEquals(ReceiptStage.Submitting, SendLive.receiptStage(maybe))

        val never = maybe.copy(receipt = maybe.receipt!!.copy(status = SendReceiptStatus.NotSent))
        val n = SendLive.receipt(drawn, never, c)
        assertEquals(ReceiptStage.Failed, n.stage)
        assertEquals(strings.t(I18nKeys.Flows.STATUS_FAILED), n.title)
        assertEquals(listOf(strings.t(I18nKeys.Flows.TX_ERROR_GENERIC)), n.captions)
        assertNull(n.hash)
        assertNull(n.viewOnExplorer)
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

    /**
     * Spec 097 F (S2): the form's line is the core's payee — the name and
     * whose word it is — else the first-time tell, else nothing. The real
     * pass printed the resolver's own label raw: "Wallet · passkey".
     */
    @Test
    fun `the recipient note says whose word the name is, never the resolver's label`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val base = SendView(stage = SendStage.EnterDetails, selected_token = xdai, recipient = recipient)
        fun note(view: SendView) = SendLive.form(drawn.model, view, FeeView(), ctx()).recipient!!.note
        fun payee(name: String?, source: SendNameSource?) = listOf(SendPayee(recipient, name, source))
        // The public registry: the corpus's tag, whatever the resolver called it.
        val registry = base.copy(
            recipient_identity = SendRecipientIdentity(name = "Wallet", source = "passkey"),
            payees = payee("Wallet", SendNameSource.Registry),
        )
        assertEquals("Wallet · Vela User", note(registry))
        assertFalse(note(registry)!!.contains("passkey"))
        assertEquals("alice.eth · ENS", note(base.copy(payees = payee("alice.eth", SendNameSource.Service("ENS")))))
        // The person's own word carries no tag.
        assertEquals("Alice", note(base.copy(payees = payee("Alice", SendNameSource.Own))))
        // A name whose source did not read is not drawn: untagged, it would pass for their own.
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), note(base.copy(payees = payee("Wallet", null), recipient_risk = SendRecipientRisk(first_time = true))))
        // The identity alone is not the payee: nothing the core did not name.
        assertNull(note(base.copy(recipient_identity = SendRecipientIdentity(name = "Wallet", source = "passkey"))))
        assertEquals(strings.t(I18nKeys.Flows.FIRST_TIME_SEND), note(base.copy(recipient_risk = SendRecipientRisk(first_time = true))))
        assertNull(note(base))
        // The sweep form keeps its "same address" line.
        val sweep = registry.copy(multi_select_mode = true, tokens = listOf(xdai), multi_selected_ids = listOf(SendLive.tokenId(xdai)))
        assertEquals(strings.t(I18nKeys.Flows.MULTI_SEND_SAME_RECIPIENT), note(sweep))
    }

    /**
     * Spec 097 F (S2): the confirm's To row showed "Wallet" — a name anyone
     * can register in the public registry — with the address one tap away on
     * the identicon. A name never stands in for the address on the page that
     * signs: the name with whose word it is, the short address under it.
     */
    @Test
    fun `the confirm's To row names the payee over the short address, and whose word the name is`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val wallet = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c"
        val view = SendView(
            stage = SendStage.Confirm, selected_token = xdai, recipient = wallet, confirm_amount = "0.001", fee = fee(),
            recipient_identity = SendRecipientIdentity(name = "Wallet", source = "passkey"),
            payees = listOf(SendPayee(wallet, "Wallet", SendNameSource.Registry)),
        )
        fun to(view: SendView) = SendLive.confirm(drawn.model, view, ctx()).facts.first { it.label == strings.t(I18nKeys.Flows.TO_LABEL) }

        val registry = to(view)
        assertEquals("Wallet", registry.value)
        assertEquals("Vela User · 0x14fB…eA5c", registry.detail)
        assertEquals(FactLead.Identicon(wallet), registry.lead)
        assertFalse(registry.mono)

        // The person's own name: no tag, the address still under it.
        val own = to(view.copy(payees = listOf(SendPayee(wallet, "Savings", SendNameSource.Own))))
        assertEquals("Savings", own.value)
        assertEquals("0x14fB…eA5c", own.detail)

        // Nobody named them: the short address alone, in mono.
        val bare = to(view.copy(payees = listOf(SendPayee(wallet))))
        assertEquals("0x14fB…eA5c", bare.value)
        assertTrue(bare.mono)
        assertNull(bare.detail)

        // A sweep's one recipient is drawn the same way.
        assertEquals(registry, to(view.copy(multi_select_mode = true)))
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

    /**
     * Issue #332: Home → Scan lands on the picker with the address the code
     * read, and the picker showed no sign of it — the person could not tell the
     * scan had worked. It says whom they are paying now, as the confirm does.
     */
    @Test
    fun `the pick says whom the money is for once a recipient is held`() {
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val held = SendLive.pick(drawn.model, SendView(tokens = listOf(xdai), recipient = recipient), ctx())
        assertEquals(
            app.getvela.wallet.feature.flows.FactRowModel(
                label = strings.t(I18nKeys.Flows.TO_LABEL),
                value = SendLive.shortAddress(recipient),
                lead = app.getvela.wallet.feature.flows.FactLead.Identicon(recipient),
                mono = true,
            ),
            held.recipient,
        )
        // The core's payee rides with the address, as on the confirm (spec 097 F).
        val named = SendLive.pick(
            drawn.model,
            SendView(tokens = listOf(xdai), recipient = recipient, payees = listOf(SendPayee(recipient, "Wallet", SendNameSource.Registry))),
            ctx(),
        )
        assertEquals("Wallet", named.recipient?.value)
        assertEquals("Vela User · ${SendLive.shortAddress(recipient)}", named.recipient?.detail)
        assertFalse(named.recipient!!.mono)
        // The sweep's picker is about the same person.
        assertEquals(held.recipient, SendLive.pick(drawn.model, SendView(tokens = listOf(xdai), recipient = recipient), ctx(), sweepPicking = true).recipient)
        // Nobody held, no line; text that is not an address gets no artwork.
        assertNull(SendLive.pick(drawn.model, SendView(tokens = listOf(xdai)), ctx()).recipient)
        assertNull(SendLive.pick(drawn.model, SendView(tokens = listOf(xdai), recipient = "hello"), ctx()).recipient?.lead)
    }

    /**
     * Issue #312: a code that named a network. The core lists only that
     * network's holdings; the picker says which network ("BNB Chain payments
     * only"), and a home filter left on another network does not hide them.
     */
    @Test
    fun `the pick says which network a scanned code named and no filter hides it`() {
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val bnb = xdai.copy(network = "chain-56", chain_id = 56, symbol = "BNB")
        val named = SendView(tokens = listOf(bnb), recipient = recipient, request_chain_id = 56)
        val live = SendLive.pick(drawn.model, named, ctx(), chainFilter = 100)
        assertEquals(listOf("BNB"), live.rows.map { it.ticker })
        // No pill: a network sheet could choose nothing the list would follow.
        assertNull(live.header.pill)
        assertEquals(strings.t(I18nKeys.Flows.SHARE_CARD_NETWORK_NOTE, mapOf("network" to "BNB Chain")), live.notice?.text)
        assertEquals(listOf(0), SendLive.visibleTokens(named, 100, "all"))
        // Nothing held there: the notice is why the list is empty.
        val nothing = SendLive.pick(drawn.model, named.copy(tokens = emptyList()), ctx())
        assertEquals(strings.t(I18nKeys.Flows.NO_TOKENS_WITH_BALANCE), nothing.empty)
        assertNotNull(nothing.notice)
        // No network named, no notice.
        assertNull(SendLive.pick(drawn.model, SendView(tokens = listOf(xdai)), ctx()).notice)
    }

    /** Issue #326: the token card is the way to another asset wherever the core says — and only there. */
    @Test
    fun `the token card offers another asset only where the core does`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val view = SendView(stage = SendStage.EnterDetails, selected_token = xdai, tokens = listOf(xdai), recipient = recipient)
        assertEquals(
            strings.t(I18nKeys.Flows.SELECT_TOKEN_TITLE),
            SendLive.form(drawn.model, view.copy(can_change_token = true), FeeView(), ctx()).token?.change,
        )
        assertNull(SendLive.form(drawn.model, view, FeeView(), ctx()).token?.change)
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
    fun `a split says the same-asset ceiling first, then over-balance, then nothing`() {
        // The core measures the ceiling against the rows' TOTAL; its sentence
        // names the most that can be sent, so it outranks "exceeds your balance".
        val split = SendView(
            stage = SendStage.EnterDetails, selected_token = xdai, split_mode = true, split_over_balance = true,
            amount_warning = SendAmountWarning.NotEnoughToken("XDAI"),
        )
        val ceiling = split.copy(
            same_asset_fee_issue = SendFeeIssueView(symbol = "XDAI", transfer_amount = "6000000000000000000", balance = "5000000000000000000", fee_amount = "100000000000000000", total = "6100000000000000000", max_transfer_amount = "4900000000000000000"),
        )
        val sentence = SendLive.formWarning(ceiling, ctx())!!
        assertTrue(sentence, sentence.contains("4.9") && !sentence.contains("000000000"))
        assertEquals(strings.t(I18nKeys.Flows.ALERT_INSUFFICIENT_BODY), SendLive.formWarning(split, ctx()))
        assertNull(SendLive.formWarning(split.copy(split_over_balance = false), ctx()))
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

    /** The web's `fillEmpty`: offered only while it would do something, and never with a rejected figure. */
    @Test
    fun `the split form offers one amount for every empty row`() {
        val drawn = FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm
        val offered = splitView.copy(
            recipients = listOf(SendRecipientDraft("rcpt_1", recipient, "0.50"), SendRecipientDraft("rcpt_2", me, "")),
            split_row_issues = listOf(SendSplitRowIssue("rcpt_2", 2, SendRowFieldState.Ok, SendRowFieldState.Empty)),
        )
        val fill = SendLive.form(drawn.model, offered, FeeView(), ctx()).fillEmpty
        assertEquals("Use 0.5 XDAI for the empty rows", fill?.label)
        // The figure goes in exactly as typed.
        assertEquals("0.50", fill?.amount)
        // No empty row, nothing to fill.
        assertNull(SendLive.form(drawn.model, offered.copy(split_row_issues = emptyList()), FeeView(), ctx()).fillEmpty)
        // A figure the core rejects is never the one that is copied.
        val rejected = offered.copy(
            recipients = listOf(SendRecipientDraft("rcpt_1", recipient, "1,5"), SendRecipientDraft("rcpt_2", me, "")),
            split_row_issues = listOf(
                SendSplitRowIssue("rcpt_1", 1, SendRowFieldState.Ok, SendRowFieldState.Invalid),
                SendSplitRowIssue("rcpt_2", 2, SendRowFieldState.Ok, SendRowFieldState.Empty),
            ),
        )
        assertNull(SendLive.form(drawn.model, rejected, FeeView(), ctx()).fillEmpty)
        // A single form never offers it.
        assertNull(SendLive.form(drawn.model, offered.copy(split_mode = false), FeeView(), ctx()).fillEmpty)
        // The dispatch: only the empty rows take the figure; ids ride along.
        val filled = SplitRows.emptyFilled(offered.recipients, "0.50")
        assertEquals(listOf("0.50", "0.50"), filled.map { it.amount })
        assertEquals(offered.recipients[0], filled[0])
        assertEquals("rcpt_2", filled[1].id)
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
        val view = splitView.copy(
            stage = SendStage.Confirm, recipients = splitView.recipients.take(2), can_confirm = true, fee = fee(),
            // The core's payees, row by row: a row's own name is the person's word.
            payees = listOf(SendPayee(recipient, "Founder", SendNameSource.Own), SendPayee(me)),
        )
        val live = SendLive.confirm(drawn.model, view, ctx())
        assertEquals("0.002 XDAI", live.amount)
        val to = live.facts.first { it.label == strings.t(I18nKeys.Flows.TO_LABEL) }
        assertEquals(strings.t(I18nKeys.Flows.RECIPIENT_COUNT, mapOf("count" to "2")), to.value)
        assertEquals(2, live.breakdown.size)
        assertEquals("Founder", live.breakdown[0].label)
        // Spec 097 F: a name never stands in for the address it pays.
        assertEquals("0x7687…D141", live.breakdown[0].detail)
        assertFalse(live.breakdown[0].mono)
        assertEquals("0.001 XDAI", live.breakdown[0].value)
        assertEquals(recipient, live.breakdown[0].identiconSeed)
        assertEquals("0x88cC…6894", live.breakdown[1].label)
        assertTrue(live.breakdown[1].mono)
        assertNull(live.breakdown[1].detail)
        // A row's name is the core's payee, tag and all.
        val tagged = SendLive.confirm(drawn.model, view.copy(payees = listOf(SendPayee(recipient, "bob.eth", SendNameSource.Service("ENS")), SendPayee(me))), ctx())
        assertEquals("bob.eth", tagged.breakdown[0].label)
        assertEquals("ENS · 0x7687…D141", tagged.breakdown[0].detail)
    }

    /**
     * SD3c: a sweep has no one figure (`confirm_amount` is empty), and the
     * confirm page drew the single-send hero over it — the first coin's
     * symbol, "≈ $0.00" and no coin at all. It lists every coin it moves,
     * each the amount the core reserved (what the signature moves).
     */
    @Test
    fun `a sweep's confirm lists every coin it moves, at the amounts the core reserved`() {
        val drawn = FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm
        val view = SendView(
            stage = SendStage.Confirm, tokens = listOf(xdai, usdc), selected_token = xdai, recipient = recipient,
            multi_select_mode = true, multi_chain_id = 100,
            multi_selected_ids = listOf(SendLive.tokenId(xdai), SendLive.tokenId(usdc)),
            multi_specs = listOf(
                SendMultiSpecView(token_address = null, decimals = 18, amount = "0.4"),
                SendMultiSpecView(token_address = usdc.token_address, decimals = 6, amount = "2.5"),
            ),
            confirm_amount = "", fee = fee(), can_confirm = true,
            payees = listOf(SendPayee(recipient, "Wallet", SendNameSource.Registry)),
        )
        val live = SendLive.confirm(drawn.model, view, ctx())
        assertNull(live.mark)
        assertEquals(strings.t(I18nKeys.Flows.ASSETS_COUNT, mapOf("n" to "2")), live.amount)
        assertNull(live.amountUnit)
        assertEquals(strings.t(I18nKeys.Flows.CONFIRM_TOTAL_LINE, mapOf("fiat" to "$2.90", "network" to "Gnosis")), live.subline)
        assertEquals(listOf("XDAI", "USDC"), live.breakdown.map { it.label })
        assertEquals(listOf("0.4 XDAI · ≈$0.40", "2.5 USDC · ≈$2.50"), live.breakdown.map { it.value })
        assertTrue(live.breakdown.all { it.lead != null && it.identiconSeed == null })
        val to = live.facts.first { it.label == strings.t(I18nKeys.Flows.TO_LABEL) }
        assertEquals("Wallet", to.value)
        assertEquals("Vela User · ${SendLive.shortAddress(recipient)}", to.detail)
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
        // …and no MAX chip on any of them (dead-controls #3). Those amounts
        // ARE the maximum — the core's specs, each balance less the fee's
        // reserve — so the chip had nothing to fill, and the event behind it
        // knows only the single selected token. An empty label draws none.
        assertEquals(listOf("", ""), live.sweepRows.map { it.max })
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
        // Lines READ, not rows kept (the web's `seen`).
        assertEquals(strings.t(I18nKeys.Flows.BATCH_PARSED_COUNT, mapOf("n" to "2")), priced.parsedLabel)
        assertEquals(listOf(true, false), priced.rows.map { it.ok })
        assertEquals("Founder", priced.rows[0].address)
        assertEquals("0.001 XDAI", priced.rows[0].conversion)
        assertEquals("an unconverted row shows no figure, not the raw one", "—", priced.rows[1].conversion)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_BAD_ADDRESS), priced.rows[1].note)
        assertNull(priced.rows[0].note)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_REJECTED_ONE, mapOf("count" to "1")), priced.rejectedText)
        assertEquals(strings.t(I18nKeys.Flows.BATCH_APPLY_ONE, mapOf("count" to "1")), priced.cta)
        assertFalse(priced.ctaDisabled)
        assertEquals(FlowState.SD2C, SendLive.flowState(view, feeSheetOpen = false))
    }

    /**
     * The import sheet says its reasons (the web's `liveBatchImport`): every
     * refused line in sheet order with why, a duplicate marked, the file's
     * failure while nothing parsed, and the total read against the balance —
     * or against what is left when the import adds to rows already typed.
     */
    @Test
    fun `the batch sheet names each refused line, the file error and the total`() {
        val drawn = FlowFixtures.build(FlowState.SD2C, strings).sheet as FlowSheet.BatchImport
        val view = SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, split_mode = true, show_batch_import = true)
        val batch = BatchView(
            opened = true, unit = WireBatchUnit.Token, fiat_code = "GBP", raw_text = "…", rate_status = BatchRateStatus.Ok,
            preview = listOf(
                BatchPreviewRow(line = 1, address = recipient, valid = true, raw_amount = "1", token_amount = "1", ok = true),
                BatchPreviewRow(line = 4, address = recipient, valid = true, dup = true, raw_amount = "2", token_amount = "2", ok = false),
            ),
            errors = listOf(
                BatchParseError(line = 2, raw = "0x12zz,5", reason = BatchParseReason.NoAddress),
                BatchParseError(line = 3, raw = "$recipient,abc", reason = BatchParseReason.NoAmount),
            ),
            rejected = 3, recipient_count = 1, total_token = "1", total_fiat = "0.78", can_apply = true,
        )
        val live = SendLive.batchImport(drawn.model, batch, view, ctx())

        assertEquals(strings.t(I18nKeys.Flows.BATCH_PARSED_COUNT, mapOf("n" to "4")), live.parsedLabel)
        assertEquals("sheet order", listOf(recipient, "0x12zz,5", "$recipient,abc", recipient), live.rows.map { it.address })
        assertEquals(
            listOf(null, strings.t(I18nKeys.Flows.BATCH_BAD_ADDRESS), strings.t(I18nKeys.Flows.BAD_AMOUNT), strings.t(I18nKeys.Flows.BATCH_DUP)),
            live.rows.map { it.note },
        )
        assertNull("a file error about a list that parsed is about nothing", live.fileError)

        val total = live.total!!
        assertEquals(
            "${strings.t(I18nKeys.Flows.SPLIT_TOTAL)} · ${strings.t(I18nKeys.Flows.RECIPIENT_COUNT_ONE, mapOf("count" to "1"))}",
            total.label,
        )
        assertTrue(total.value.startsWith("1 XDAI"))
        assertTrue("the fiat total rides beside it", total.value.endsWith("GBP"))
        assertEquals(strings.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "0.71697 XDAI")), total.remaining)

        // Adding to someone already on the form: what is LEFT, not the balance.
        val typed = view.copy(split_import_room = 59, split_remaining = "2.25")
        assertEquals(
            strings.t(I18nKeys.Flows.SPLIT_REMAINING, mapOf("amount" to "2.25 XDAI")),
            SendLive.batchImport(drawn.model, batch, typed, ctx()).total!!.remaining,
        )
        assertEquals(
            "replacing them draws from the whole balance again",
            strings.t(I18nKeys.Flows.BALANCE_LABEL, mapOf("amount" to "0.71697 XDAI")),
            SendLive.batchImport(drawn.model, batch, typed, ctx(), replaces = true).total!!.remaining,
        )

        val failed = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Token, file_error = true), view, ctx())
        assertEquals(
            "${strings.t(I18nKeys.Flows.BATCH_IMPORT_FAILED_TITLE)}. ${strings.t(I18nKeys.Flows.BATCH_IMPORT_FAILED_BODY)}",
            failed.fileError,
        )
        assertNull("nobody parsed: no total", failed.total)

        // 087: a file in a legacy code page says how to save it — not "use a
        // CSV", which it is.
        val legacy = SendLive.batchImport(
            drawn.model,
            BatchView(
                opened = true,
                unit = WireBatchUnit.Token,
                file_error = true,
                file_failure = app.getvela.wallet.feature.send.core.BatchFileFailure.UnsupportedEncoding,
            ),
            view,
            ctx(),
        )
        assertEquals(
            "${strings.t(I18nKeys.Flows.BATCH_IMPORT_FAILED_TITLE)}. ${strings.t(I18nKeys.Contacts.IMPORT_FAIL_ENCODING)}",
            legacy.fileError,
        )
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

    /** Device-found: "In XDAI" still explained a USD rate the core ignores in token mode. */
    @Test
    fun `a token-denominated import says it converts nothing`() {
        val drawn = FlowFixtures.build(FlowState.SD2C, strings).sheet as FlowSheet.BatchImport
        val view = SendView(stage = SendStage.EnterDetails, tokens = listOf(xdai), selected_token = xdai, show_batch_import = true)

        val token = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Token, fiat_code = "USD"), view, ctx())
        assertEquals(strings.t(I18nKeys.Flows.BATCH_TOKEN_HINT, mapOf("sym" to "XDAI")), token.rateHint)
        val fiat = SendLive.batchImport(drawn.model, BatchView(opened = true, unit = WireBatchUnit.Fiat, fiat_code = "USD"), view, ctx())
        assertEquals(strings.t(I18nKeys.Flows.BATCH_RATE_HINT, mapOf("code" to "USD", "sym" to "XDAI")), fiat.rateHint)
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
            assertEquals("0,001", confirm.amount)
            assertEquals("XDAI", confirm.amountUnit)
        } finally {
            Formats.current = saved
        }
    }
}
