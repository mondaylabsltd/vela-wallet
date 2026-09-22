package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeOptionView
import app.getvela.wallet.feature.send.core.FeeSpeedOptionView
import app.getvela.wallet.feature.send.core.FeeSpeedView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.FeeModel
import app.getvela.wallet.feature.signing.SigningBlock
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.SignErrorKind
import app.getvela.wallet.feature.signing.core.SignExecutor
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearProvenance
import app.getvela.wallet.feature.signing.core.ClearSignField
import app.getvela.wallet.feature.signing.core.ClearSignResult
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

    /**
     * Spec 081 FR-008: the sheet says where a description came from, and only
     * where there is something to say. A fetched descriptor is the descriptor
     * service's word over plain HTTP; everything else either IS the app's own
     * word or already has its own line.
     */
    @Test
    fun `a fetched description says it was never authenticated, and the others say nothing`() {
        val params = """[{"to":"$founder","data":"0xdeadbeef"}]"""
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        fun warningsFor(provenance: ClearProvenance): List<String> {
            val result = ClearSignResult(
                intent = "Stake",
                fields = listOf(ClearSignField(label = "To", value = "0x…", format = "addressName", address = founder)),
                contract_address = founder,
                provenance = provenance,
            )
            val clear = ClearSigningView(resolved = true, result = result, surface = ClearSurface.ClearSign, confirm = ClearConfirm.Confirm)
            val model = SigningLive.model(drawn, request(params), sign, clear, GuardView(), FeeView(confirm_fee_ready = true), ctx)
            return model.blocks.filterIsInstance<SigningBlock.Warning>().map { it.text }
        }
        assertEquals(
            listOf(strings.t("componentsUi.signing.descriptorFetchedWarning")),
            warningsFor(ClearProvenance.Fetched),
        )
        for (quiet in listOf(ClearProvenance.BuiltIn, ClearProvenance.PinnedMatch, ClearProvenance.Standard, ClearProvenance.None)) {
            assertEquals(quiet.name, emptyList<String>(), warningsFor(quiet))
        }
    }

    private fun estimate(tier: FeeTier, totalWei: String) = FeeEstimateView(
        chain_id = 100, total_wei = totalWei, max_fee_per_gas = "2000000000", network_fee_per_gas = "0",
        relayer_fee_per_gas = "0", bundler_gas_price = "0", in_band_gas_basis = "0", effective_gas_price = "1000000000",
        max_gas_price = "2000000000", total_gas = "0", deployed = true, tier = tier, quoted = true,
        fee_asset = FeeAssetView.Native, fee_recipient = "0xfee",
    )

    /** Spec 069: the sheet's fee card carries the send form's speed control, drawn by the same builder. */
    @Test
    fun `the fee card carries the speed control, each option its own fee`() {
        val params = """[{"to":"$founder","value":"0x38d7ea4c68000"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        val fast = estimate(FeeTier.Fast, "2100000000000000")
        val speed = FeeSpeedView(
            tier = FeeTier.Fast,
            open = true,
            options = listOf(
                FeeSpeedOptionView(FeeTier.Fast, selected = true, fee = fast, gas_price = "1 ~ 2 gwei"),
                FeeSpeedOptionView(FeeTier.Standard, measuring = true),
                FeeSpeedOptionView(FeeTier.Slow, fee = estimate(FeeTier.Slow, "1000000000000000")),
            ),
        )
        val model = SigningLive.model(
            drawn, request(params), sign, clear, GuardView(), FeeView(fee = fast, confirm_fee_ready = true), ctx,
            speed = SendLive.SpeedInputs(speed) { null },
        )
        val fee = model.fee as FeeModel.OnChain
        val control = fee.speed ?: error("the sheet draws the speed control")
        assertEquals(strings.t(I18nKeys.Flows.FEE_SPEED_LABEL), control.label)
        assertEquals(strings.t(I18nKeys.Flows.GAS_TIER_FAST), control.value)
        assertEquals(listOf("fast", "standard", "slow"), control.options.map { it.id })
        // Each option in its row's own words, minus the row's "~".
        assertEquals(fee.value, "~" + control.options[0].value)
        assertEquals("…", control.options[1].value)
        assertTrue(control.options[2].value, control.options[2].value.startsWith("0.001 XDAI"))
        assertEquals("1 ~ 2 gwei", control.options[0].gasPrice)
        assertTrue(model.confirmEnabled)
    }

    @Test
    fun `a speed just picked says estimating, and the slide waits for its own figure (issue 681)`() {
        val params = """[{"to":"$founder","value":"0x38d7ea4c68000"}]"""
        val clear = ClearSigningView(resolved = true, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))
        val sign = SignView(surface = SignSurface.Sheet, confirm_gate_open = true)
        // The core's gate is still open — on the speed just left.
        val left = FeeView(fee = estimate(FeeTier.Fast, "2100000000000000"), confirm_fee_ready = true)
        val picked = SendLive.SpeedInputs(FeeSpeedView(tier = FeeTier.Slow, picked = true)) { null }
        val model = SigningLive.model(drawn, request(params), sign, clear, GuardView(), left, ctx, speed = picked)
        assertEquals(strings.t("componentsUi.gas.estimating"), (model.fee as FeeModel.OnChain).value)
        assertFalse("the slide never signs the speed walked away from", model.confirmEnabled)
        // Its own figure lands: the row and the slide follow.
        val landed = FeeView(fee = estimate(FeeTier.Slow, "1000000000000000"), confirm_fee_ready = true)
        val settled = SigningLive.model(drawn, request(params), sign, clear, GuardView(), landed, ctx, speed = picked)
        assertTrue((settled.fee as FeeModel.OnChain).value.startsWith("~0.001 XDAI"))
        assertTrue(settled.confirmEnabled)
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

    /**
     * Spec 081, device-found. A refused request answered the page with
     * `-32603` and the refused FUNCTION as its message — "addOwnerWithThreshold"
     * on its own, which reads as a label, not an answer. The page now gets a
     * sentence and a machine-readable `kind`, the way the web shell already
     * sends one.
     */
    @Test
    fun aRefusedRequestTellsThePageWhatHappenedAndWhy() {
        val json = SignExecutor.responseJson(
            "rid-1",
            SignResponsePayload.Err(
                code = -32603,
                kind = SignErrorKind.SelfCallBlocked,
                message = "addOwnerWithThreshold",
            ),
        )
        val error = json.getJSONObject("error")
        assertEquals(-32603, error.getInt("code"))
        assertEquals("self_call_blocked", error.getString("kind"))
        assertTrue(
            "the message must explain, not just name: ${error.getString("message")}",
            error.getString("message").startsWith("This request would change who controls the wallet") &&
                error.getString("message").contains("addOwnerWithThreshold"),
        )

        // An ordinary rejection keeps its own words and still carries a kind.
        val rejected = SignExecutor.responseJson(
            "rid-2",
            SignResponsePayload.Err(code = 4001, kind = SignErrorKind.UserRejected, message = null),
        ).getJSONObject("error")
        assertEquals("User rejected the request", rejected.getString("message"))
        assertEquals("user_rejected", rejected.getString("kind"))
    }
}
