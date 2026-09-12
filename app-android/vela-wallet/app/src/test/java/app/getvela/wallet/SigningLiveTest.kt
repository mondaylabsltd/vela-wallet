package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.send.core.FeeView
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
import org.junit.Assert.assertTrue
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
}
