package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowSheet
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.flows.ReceiptStage
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
import org.junit.Test

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
        chainNames = mapOf(100 to "Gnosis"),
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
        assertNull("split is 045", live.addRecipient)
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
}
