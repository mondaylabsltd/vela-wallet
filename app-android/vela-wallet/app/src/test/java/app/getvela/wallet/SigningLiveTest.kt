package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.core.SignRequestView
import app.getvela.wallet.feature.signing.core.SignMethodKind
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import app.getvela.wallet.feature.signing.core.SigningController
import app.getvela.wallet.feature.wallet.core.TrustSimJudgment
import app.getvela.wallet.feature.signing.SigningTone
import org.junit.Test

/** Spec 044: the signing sheet is the four views, in the corpus's words. */
class SigningLiveTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val drawn = SigningFixtures.build(SigningScreenState.CS1, strings)
    private val founder = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val ctx = SigningLive.Context(strings, "Gnosis", Color.Red, "XDAI", "Parallel space", "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
    private fun request(params: String) = IncomingRequest("r1", "eth_sendTransaction", params, "http://127.0.0.1:8137", "tab-1", 100)

    @Test
    fun `a plain native transfer reads as Send, not as a blind contract call`() {
        val params = """[{"to":"$founder","value":"0x38d7ea4c68000"}]"""
        val clear = ClearSigningView(resolving = false, resolved = true, result = null, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))
        val sign = SignView(surface = SignSurface.Sheet, request = SignRequestView("r1", "eth_sendTransaction", SignMethodKind.Transaction, params, "http://127.0.0.1:8137", null, 100, null), confirm_gate_open = true)
        val model = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = true), ctx)
        val intent = model.blocks.filterIsInstance<SigningBlock.Intent>().single()
        assertEquals(strings.t("componentsUi.signing.intentSend"), intent.text)
        val amount = model.blocks.filterIsInstance<SigningBlock.Amount>().single()
        assertEquals("0.001", amount.line.value)
        assertEquals("XDAI", amount.line.symbol)
        assertEquals(founder, model.blocks.filterIsInstance<SigningBlock.Party>().single().address)
        assertEquals("127.0.0.1:8137", model.dappHost)
        assertEquals(strings.t("componentsUi.signing.confirmSend"), model.confirmAction)
        assertTrue(model.confirmEnabled)
    }

    @Test
    fun `the slide waits for the guard and the fee, and a contract call with bytes stays blind`() {
        val params = """[{"to":"$founder","data":"0xdeadbeef"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.Confirm)
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val blocked = SigningLive.model(drawn, request(params), sign, clear, GuardView(confirm_allowed = false), FeeView(confirm_fee_ready = true), ctx)
        assertFalse(blocked.confirmEnabled)
        val noFee = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = false), ctx)
        assertFalse(noFee.confirmEnabled)
        val blind = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = true), ctx)
        assertTrue(blind.blocks.any { it is SigningBlock.Warning })
        assertEquals(strings.t("componentsUi.signing.confirmLabel"), blind.confirmAction)
    }

    @Test
    fun `the guard's verdict draws the cap, the chips the core offers, and the custom field`() {
        val detected = app.getvela.wallet.feature.signing.core.GuardDetectedApproval(
            kind = app.getvela.wallet.feature.signing.core.GuardApprovalKind.Erc20Approve, token_address = "0xdd", spender = "0x1111111111111111111111111111111111111111",
            amount_raw = null, is_unbounded = true, editable = true, locus = app.getvela.wallet.feature.signing.core.GuardLocus.CalldataWord(1),
        )
        val editor = app.getvela.wallet.feature.signing.core.GuardEditorView(mode = null, requested_finite = false, has_balance_cap = false)
        val blocked = GuardView(surface = app.getvela.wallet.feature.signing.core.GuardSurface.ApprovalEditor, detected = detected, meta = app.getvela.wallet.feature.signing.core.GuardTokenMetaView("USDC", 6, true, false), editor = editor, confirm_allowed = false)
        val blocks = SigningLive.guardBlocks(blocked, strings)
        val allowance = blocks.filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals(strings.t("componentsUi.signingApprove.unlimitedValue"), allowance.value)
        assertEquals(app.getvela.wallet.feature.signing.AllowanceChip.ChipState.Disabled, allowance.chips.first { it.id == "requested" }.state)
        assertTrue(allowance.note!!.contains(strings.t("componentsUi.signingApprove.choosePrompt")))
        assertTrue(blocks.any { it is SigningBlock.Warning })
        val custom = blocked.copy(editor = editor.copy(mode = app.getvela.wallet.feature.signing.core.GuardEditorMode.Custom, custom_text = "1", display_amount_raw = "1000000", choice = app.getvela.wallet.feature.signing.core.GuardChoice.Amount("1000000")), confirm_allowed = true)
        val bounded = SigningLive.guardBlocks(custom, strings).filterIsInstance<SigningBlock.Allowance>().single()
        assertEquals("1 USDC", bounded.value)
        assertEquals("1", bounded.custom!!.value)
        assertEquals(app.getvela.wallet.feature.signing.AllowanceChip.ChipState.Selected, bounded.chips.first { it.id == "custom" }.state)
    }

    // -- Spec 046 US1: the balance-change block --------------------------------

    @Test
    fun `the balance block says what moves, as the trust machine judged it`() {
        val ready = SigningController.SimOutcome.Ready(
            listOf(
                TrustSimJudgment.Native("-1000000000000000"),
                TrustSimJudgment.Erc20Trusted(token = "0xddaf", delta = "12000000", symbol = "USDC", decimals = 6),
                TrustSimJudgment.Erc20Unverified(token = "0xbad", delta = "5"),
            ),
        )
        val blocks = SigningLive.simBlocks(ready, ctx)
        val balances = blocks.single() as SigningBlock.Balances
        assertEquals(strings.t("componentsUi.signing.balanceChangesTitle"), balances.title)
        assertEquals(listOf("XDAI", "USDC", strings.t("componentsUi.signing.balanceUnverifiedToken")), balances.rows.map { it.symbol })
        assertEquals(listOf("−0.001", "+12", "+5"), balances.rows.map { it.delta })
        assertEquals(listOf(SigningTone.Neutral, SigningTone.Success, SigningTone.Caution), balances.rows.map { it.tone })
        assertEquals(strings.t("componentsUi.signing.unverifiedWarning"), balances.note)

        val none = SigningLive.simBlocks(SigningController.SimOutcome.Ready(emptyList()), ctx).single() as SigningBlock.Balances
        assertEquals(strings.t("componentsUi.signing.simResultNoChange"), none.note)
        assertTrue(none.rows.isEmpty())
        val unavailable = SigningLive.simBlocks(SigningController.SimOutcome.Unavailable, ctx).single() as SigningBlock.Warning
        assertEquals(strings.t("componentsUi.signing.simUnavailableWarning"), unavailable.text)
        assertTrue(SigningLive.simBlocks(null, ctx).isEmpty())
    }

    // -- Issue #262: the coin that pays --------------------------------------

    private fun estimate(asset: FeeAssetView, totalWei: String) = FeeEstimateView(
        chain_id = 1, total_wei = totalWei, max_fee_per_gas = "0", network_fee_per_gas = "1", relayer_fee_per_gas = "0",
        bundler_gas_price = "1", in_band_gas_basis = "1", total_gas = "300000", deployed = false, tier = FeeTier.Fast,
        quoted = true, fee_asset = asset, fee_recipient = "0x3e59292e18417f814112f731e7163534c6d2fe3c",
    )

    private val eth = FeeOptionView(symbol = "ETH", decimals = 18, balance = "0", recipient = "0x3e59", usd_balance = "0", amount = "400000000000000", insufficient = true, selected = true)
    private val usdt = FeeOptionView(symbol = "USDT", contract = "0xdAC17F958D2ee523a2206206994597C13D831ec7", decimals = 6, balance = "2000000", recipient = "0x3e59", usd_balance = "2", amount = "1020000")

    /** The report's own wallet: 0 ETH and 2 USDT on Ethereum, quoted in ETH. */
    @Test
    fun `a fee in a coin the account does not hold says so, and the list offers the one it has`() {
        val fee = FeeView(fee = estimate(FeeAssetView.Native, "400000000000000"), options = listOf(eth, usdt), confirm_fee_ready = false)

        val closed = SigningLive.feeModel(ClearSigningView(), fee, ctx) as FeeModel.OnChain
        assertEquals(strings.t("send.warnInsufficientGas", mapOf("sym" to "ETH")), closed.warning)
        assertTrue("two coins: the row opens a list", closed.tappable)
        assertTrue(closed.options.isEmpty())

        val open = SigningLive.feeModel(ClearSigningView(), fee, ctx.copy(feeOpen = true)) as FeeModel.OnChain
        assertEquals(listOf("native", usdt.contract), open.options.map { it.id })
        assertTrue("ETH cannot pay: shown, not pickable", open.options[0].disabled)
        assertFalse(open.options[1].disabled)
    }

    /** A coin that pays is not a warning, and one coin alone has no list to open. */
    @Test
    fun `a fee the account can pay carries no warning`() {
        val fee = FeeView(fee = estimate(FeeAssetView.Native, "1"), options = listOf(eth.copy(balance = "1", amount = "1", insufficient = false)), confirm_fee_ready = true)

        val model = SigningLive.feeModel(ClearSigningView(), fee, ctx) as FeeModel.OnChain
        assertNull(model.warning)
        assertFalse(model.tappable)
    }

    /** The approve signs the coin that was picked, in that coin's units (the send core's own rule). */
    @Test
    fun `the approve carries the picked stablecoin and its own amount`() {
        val picked = FeeView(
            fee = estimate(FeeAssetView.Erc20(token = usdt.contract!!, decimals = 6, amount = "1020000", symbol = "USDT"), "0"),
            fee_token = usdt.contract,
            confirm_fee_ready = true,
        )
        val opts = SigningController.approveOpts(picked, ClearSigningView(), GuardView())
        assertEquals(usdt.contract, opts.gas_fee_token)
        assertEquals("1020000", opts.quoted_fee?.amount)

        val native = SigningController.approveOpts(FeeView(fee = estimate(FeeAssetView.Native, "400000000000000"), confirm_fee_ready = true), ClearSigningView(), GuardView())
        assertNull(native.gas_fee_token)
        assertEquals("400000000000000", native.quoted_fee?.amount)
    }
}
