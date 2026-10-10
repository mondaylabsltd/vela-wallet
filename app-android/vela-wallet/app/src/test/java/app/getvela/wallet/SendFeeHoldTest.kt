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
import app.getvela.wallet.feature.send.core.FeeBoards
import app.getvela.wallet.feature.send.core.FeeEstimateView
import app.getvela.wallet.feature.send.core.FeeFailedWord
import app.getvela.wallet.feature.send.core.FeeTier
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendChainInfo
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendEvent
import app.getvela.wallet.feature.send.core.SendFeeOutcome
import app.getvela.wallet.feature.send.core.SendShellResult
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendToken
import app.getvela.wallet.feature.send.core.SendTreasuryProbe
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.wallet.WalletLive
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.SendCore

/**
 * PR 2 integration (core d9b00e47f), on the REAL send machine: while the fee
 * card has failed — a failure, or the core's own re-ask after one — the
 * confirm is held and the figure kept from Continue is dropped, so the row
 * draws the failure and the confirm never opens on a figure the fee machine
 * discarded between two re-asks; a settled quote (`FeeUpdated`) opens it again.
 * The bridge tells the machine once per change, and a fresh journey's first
 * word always goes ([FeeFailedWord]).
 */
class SendFeeHoldTest {

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private val xdai = SendToken(network = "chain-100", chain_id = 100, symbol = "XDAI", balance = "1.5", decimals = 18, price_usd = 1.0)

    private fun estimate(totalWei: String = "123000000000000") = FeeEstimateView(
        chain_id = 100, total_wei = totalWei, max_fee_per_gas = "1000000000", network_fee_per_gas = "1000000000",
        relayer_fee_per_gas = "0", bundler_gas_price = "1000000000", in_band_gas_basis = "123000", total_gas = "123000",
        deployed = true, tier = FeeTier.Standard, quoted = true, fee_asset = FeeAssetView.Native,
        fee_recipient = "0x2222222222222222222222222222222222222222",
    )

    private fun json(result: SendShellResult) = Wire.json.encodeToString(SendShellResult.serializer(), result)

    /** The send machine on its confirm page, Continue's estimate in hand — every read answered as a calm network would. */
    private fun onConfirm(): Pair<CoreScript, () -> SendView> {
        val script = CoreScript(SendCore().asBridge()) { operation ->
            when (operation.optString("type")) {
                "fetch_tokens" -> json(SendShellResult.TokensLoaded(tokens = listOf(xdai), chains = listOf(SendChainInfo(100, "chain-100", "XDAI"))))
                "load_account_credential" -> json(SendShellResult.AccountCredential("04" + "ab".repeat(64)))
                "estimate_fee" -> json(SendShellResult.FeeEstimated(SendFeeOutcome.Ok(estimate())))
                "probe_treasury" -> json(SendShellResult.TreasuryProbed(SendTreasuryProbe.Covered))
                "resolve_risk" -> json(SendShellResult.RiskResolved(null))
                "resolve_identity" -> json(SendShellResult.IdentityResolved(null))
                "simulate_calls" -> json(SendShellResult.SimResolved(null))
                "prewarm_fees" -> json(SendShellResult.FeesPrewarmed)
                "haptic" -> json(SendShellResult.HapticPlayed)
                "show_alert" -> json(SendShellResult.AlertAcknowledged)
                // Timers stay out: the board is the moment on screen.
                else -> null
            }
        }
        fun send(event: SendEvent) = script.dispatch(Wire.json.encodeToString(SendEvent.serializer(), event))
        fun view() = Wire.json.decodeFromString(SendView.serializer(), script.viewJson())
        send(SendEvent.Open(account = SendAccountRef(id = "cred", address = safe), display = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)))
        send(SendEvent.FeeFailedChanged(false))
        // The token as the core lists it (its display symbol is the core's: "xDAI").
        send(SendEvent.SelectToken(SendLive.tokenId(view().tokens.single())))
        send(SendEvent.SetRecipient("0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141"))
        send(SendEvent.SetAmount("0.5"))
        send(SendEvent.Continue)
        val confirm = view()
        assertEquals("on the confirm page (still waiting on: ${script.waiting()})", SendStage.Confirm, confirm.stage)
        assertNotNull("the estimate in hand", confirm.fee)
        assertNotNull(confirm.selected_token)
        assertTrue("open before the fee fails", confirm.can_confirm)
        return script to ::view
    }

    @Test
    fun `a failed fee holds the confirm and drops its figure, and a settled quote opens it again`() {
        val (script, view) = onConfirm()
        fun send(event: SendEvent) = script.dispatch(Wire.json.encodeToString(SendEvent.serializer(), event))

        // The fee card failed (a catch-up that could not price): the bridge says so.
        send(SendEvent.FeeFailedChanged(true))
        val held = view()
        assertFalse("held while the fee card has failed", held.can_confirm)
        assertNull("the figure the fee machine discarded is gone", held.fee)

        // Drawn: the failure's figure and reason, and ONE line under the held confirm — the sheet's.
        val fee = FeeBoards.view(FeeBoards.Case.Retrying, chainId = 100, account = safe)
        val drawn = (FlowFixtures.build(FlowState.SD3, strings).base as FlowBase.SendConfirm).model
        val ctx = SendLive.Context(strings, mapOf(100 to "Gnosis"), emptyMap(), WalletLive.Money.dollars(), "Me", safe)
        val confirm = SendLive.confirm(drawn, held, ctx, fee)
        assertFalse(confirm.ctaEnabled)
        assertEquals(strings.t(I18nKeys.Flows.FEE_RETRYING), confirm.ctaHold)
        val feeFact = confirm.facts.single { it.label == strings.t(I18nKeys.Flows.EST_FEE) }
        assertEquals("—", feeFact.value)
        assertEquals(strings.t(I18nKeys.Flows.FEE_REASON_CHAIN_DOWN, mapOf("chain" to "Gnosis")), feeFact.note)

        // The core's re-ask out (busy) and still failed: still held — no open↔held flicker between re-asks.
        send(SendEvent.FeeBusyChanged(true))
        send(SendEvent.FeeBusyChanged(false))
        assertFalse(view().can_confirm)

        // A settled quote: the figure is back and the confirm opens.
        send(SendEvent.FeeUpdated(estimate("124000000000000")))
        val back = view()
        assertEquals("124000000000000", back.fee?.total_wei)
        assertTrue("open again once the fee is priced", back.can_confirm)
        assertNull(SendLive.confirm(drawn, back, ctx, app.getvela.wallet.feature.send.core.FeeView()).ctaHold)
    }

    @Test
    fun `the bridge tells each change once, and a fresh journey its first word`() {
        val word = FeeFailedWord()
        assertEquals(SendEvent.FeeFailedChanged(false), word.news(false))
        assertNull(word.news(false))
        assertEquals(SendEvent.FeeFailedChanged(true), word.news(true))
        assertNull("deduped", word.news(true))
        word.forget()
        assertEquals("a fresh journey has been told nothing", SendEvent.FeeFailedChanged(true), word.news(true))
        assertEquals(
            """{"type":"fee_failed_changed","failed":true}""",
            Wire.json.encodeToString(SendEvent.serializer(), SendEvent.FeeFailedChanged(true)),
        )
    }
}
