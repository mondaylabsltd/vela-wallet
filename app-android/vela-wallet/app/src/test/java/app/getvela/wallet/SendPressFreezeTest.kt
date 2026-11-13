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
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeAssetView
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendFeeMoved
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendMaxMoved
import app.getvela.wallet.feature.send.core.SendRecipientRisk
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.feature.send.core.SendTxStatus
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SendCore

/**
 * §34 (R20 S1): charged = shown, frozen at the press — on the REAL send
 * machine, drawn by this shell's own confirm builder.
 *
 * The R20 figures: a never-deployed account holding 0.3 USDC on Arc Testnet,
 * Max on the confirm 0.0675 with a 0.2325 fee; a quote of 0.2941125 landed
 * between the press and the signature, the core returned the button without
 * a word and re-derived the Max under it, and the next press signed 0.0058875
 * nobody had confirmed.
 */
class SendPressFreezeTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val account = "0x8275e99B9c20f1dA503492D37f6832ea0104f2eC"
    private val payee = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"
    private val usdc = SendToken(network = "arc-testnet", chain_id = 5_042_002, symbol = "USDC", balance = "0.3", decimals = 18, price_usd = 1.0)

    /** A native fee on Arc for an account its first send deploys. */
    private fun fee(wei: String) = FeeEstimateView(
        chain_id = 5_042_002, total_wei = wei, max_fee_per_gas = "0", network_fee_per_gas = "20000000000",
        relayer_fee_per_gas = "0", bundler_gas_price = "20000000000", in_band_gas_basis = "20000000000", total_gas = "450000",
        deployed = false, tier = FeeTier.Fast, quoted = true, fee_asset = FeeAssetView.Native,
        fee_recipient = "0x5c3D966577b66a12B815119f1af452Ed7adf260c",
    )

    private val ctx by lazy { SendLive.Context(strings, mapOf(5_042_002 to "Arc Testnet"), emptyMap(), WalletLive.Money.dollars(), "R20", account) }

    private fun confirmModel(view: SendView) =
        SendLive.confirm((FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model, view, ctx)

    /** The real core; the press's own reads (the credential, the relay's float) held while `holding`. */
    private inner class Machine {
        var quote: FeeEstimateView = fee("267375000000000000")
        var holding = false
        val script = CoreScript(SendCore().asBridge()) { operation ->
            fun json(result: SendShellResult) = Wire.json.encodeToString(SendShellResult.serializer(), result)
            when (operation.optString("type")) {
                "fetch_tokens" -> json(SendShellResult.TokensLoaded(tokens = listOf(usdc), chains = listOf(SendChainInfo(5_042_002, "arc-testnet", "USDC"))))
                "load_account_credential" -> if (holding) null else json(SendShellResult.AccountCredential("04" + "ab".repeat(64)))
                "estimate_fee" -> json(SendShellResult.FeeEstimated(SendFeeOutcome.Ok(quote)))
                "probe_treasury" -> if (holding) null else json(SendShellResult.TreasuryProbed(SendTreasuryProbe.Covered))
                "resolve_risk" -> json(SendShellResult.RiskResolved(SendRecipientRisk(is_contract = false, first_time = false)))
                "resolve_identity" -> json(SendShellResult.IdentityResolved(null))
                "simulate_calls" -> json(SendShellResult.SimResolved(null))
                "prewarm_fees" -> json(SendShellResult.FeesPrewarmed)
                "haptic" -> json(SendShellResult.HapticPlayed)
                "show_alert" -> json(SendShellResult.AlertAcknowledged)
                else -> null
            }
        }

        fun send(event: SendEvent) = script.dispatch(Wire.json.encodeToString(SendEvent.serializer(), event))
        fun view(): SendView = Wire.json.decodeFromString(SendView.serializer(), script.viewJson())

        /** The press's reads answered now. */
        fun release() {
            holding = false
            fun json(result: SendShellResult) = Wire.json.encodeToString(SendShellResult.serializer(), result)
            if ("load_account_credential" in script.waiting()) {
                script.resolve("load_account_credential", json(SendShellResult.AccountCredential("04" + "ab".repeat(64))))
            }
            if ("probe_treasury" in script.waiting()) {
                script.resolve("probe_treasury", json(SendShellResult.TreasuryProbed(SendTreasuryProbe.Covered)))
            }
        }

        fun submit(): JSONObject? = script.waitingOperations().firstOrNull { it.optString("type") == "submit_user_op" }
    }

    @Test
    fun `a quote moved under the press signs nothing, says so beside the confirm, and the next press signs the new figures`() {
        val m = Machine()
        m.send(SendEvent.Open(account = SendAccountRef(id = "cred", address = account), display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)))
        m.send(SendEvent.FeeFailedChanged(false))
        m.send(SendEvent.SelectToken(SendLive.tokenId(m.view().tokens.single())))
        m.send(SendEvent.SetRecipient(payee))
        m.send(SendEvent.FeeUpdated(fee("267375000000000000")))
        m.send(SendEvent.TapMax)
        // Continue: the pre-check prices 0.2325 (the deployment in it).
        m.quote = fee("232500000000000000")
        m.send(SendEvent.Continue)
        var view = m.view()
        assertEquals("on the confirm (waiting: ${m.script.waiting()})", SendStage.Confirm, view.stage)
        assertEquals("0.0675", view.confirm_amount)
        assertEquals("232500000000000000", view.fee?.total_wei)
        assertTrue(view.can_confirm)

        // The press; the fee card's next quote lands while its reads are out.
        m.holding = true
        m.send(SendEvent.SlideConfirm(view.review_digest))
        m.send(SendEvent.FeeUpdated(fee("294112500000000000")))
        view = m.view()
        assertEquals(SendTxStatus.Preparing, view.tx_status)
        assertEquals("the amount pressed stays drawn", "0.0675", view.confirm_amount)
        assertEquals("the fee pressed stays drawn", "232500000000000000", view.fee?.total_wei)
        assertEquals(I18nKeys.Flows.FEE_SIGNING, view.fee_signing_key)
        val feeRow = confirmModel(view).facts.first { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertEquals(strings.t(I18nKeys.Flows.FEE_SIGNING), feeRow.note)

        // The signature's moment: the quote moved — nothing signed.
        m.release()
        assertNull("nothing is signed (waiting: ${m.script.waiting()})", m.submit())
        view = m.view()
        assertEquals(SendTxStatus.Idle, view.tx_status)
        assertNull(view.fee_signing_key)
        // r21-2 (§36): Max is cut DOWN to the places it is shown at, and sends exactly that.
        assertEquals("0.005887", view.confirm_amount)
        assertEquals("294112500000000000", view.fee?.total_wei)
        assertEquals(SendFeeMoved(key = I18nKeys.Flows.FEE_MOVED_MAX, max = SendMaxMoved(from = "0.0675", to = "0.005887")), view.fee_moved)
        val stopped = confirmModel(view)
        assertEquals(
            strings.t(I18nKeys.Flows.FEE_MOVED_MAX, mapOf("from" to "0.0675", "to" to "0.005887", "symbol" to "USDC")),
            stopped.notice,
        )
        assertTrue("said calmly", stopped.noticeCalm)
        assertNull("one sentence for one move", stopped.maxNote)
        assertTrue(view.can_confirm)

        // The next press signs exactly what is drawn (r21-2: the Max as shown, 0.005887).
        m.send(SendEvent.SlideConfirm(view.review_digest))
        val submit = m.submit() ?: error("the next press is signed (waiting: ${m.script.waiting()})")
        val review = submit.getJSONObject("review")
        assertEquals("5887000000000000", review.getJSONArray("transfers").getJSONObject(0).getString("amount"))
        assertEquals("294112500000000000", review.getJSONObject("fee").getString("amount"))
        assertFalse(m.view().fee_moved != null)
    }
}
