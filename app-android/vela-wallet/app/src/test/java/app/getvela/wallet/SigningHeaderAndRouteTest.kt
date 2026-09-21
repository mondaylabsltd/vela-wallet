package app.getvela.wallet

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.signing.core.IncomingRequest
import app.getvela.wallet.feature.signing.SigningFixtures
import app.getvela.wallet.feature.signing.SigningLive
import app.getvela.wallet.feature.signing.SigningScreenState
import app.getvela.wallet.feature.signing.core.ClearConfirm
import app.getvela.wallet.feature.signing.core.ClearSigningView
import app.getvela.wallet.feature.signing.core.ClearSurface
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.signing.core.GuardView
import app.getvela.wallet.feature.signing.core.SignMethodKind
import app.getvela.wallet.feature.signing.core.SignRequestView
import app.getvela.wallet.feature.signing.core.SignSurface
import app.getvela.wallet.feature.signing.core.SignView
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Founder review 2026-09-19, on the native sheets: the wallet's own request
 * wears the wallet's mark and name, a site gets its own icon with its initial
 * as the fallback, the chip carries the chain's logo — and a person can say
 * WHERE their passkey is, which also decides which key the ceremony is pinned
 * to (the core's rule, run here for real).
 */
class SigningHeaderAndRouteTest {
    private val strings: VelaStrings = run {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }
    private val drawn = SigningFixtures.build(SigningScreenState.CS1, strings)
    private val params = """[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x1"}]"""
    private val clear = ClearSigningView(resolving = false, resolved = true, result = null, surface = ClearSurface.BlindTransaction, confirm = ClearConfirm.ConfirmIntent("send"))

    private fun model(origin: String, transport: String, ctx: SigningLive.Context) = SigningLive.model(
        drawn,
        IncomingRequest("r1", "eth_sendTransaction", params, origin, transport, 1),
        SignView(surface = SignSurface.Sheet, request = SignRequestView("r1", "eth_sendTransaction", SignMethodKind.Transaction, params, origin, null, 1, null), confirm_gate_open = true),
        clear, GuardView(), FeeView(confirm_fee_ready = true), ctx,
    )

    private fun ctx(method: String = "auto", open: Boolean = false) =
        SigningLive.Context(strings, "Ethereum", Color.Red, "ETH", "Mine", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", chainId = 1, signMethod = method, signWithOpen = open)

    @Test
    fun `the wallet's own request is not a site`() {
        val own = model("https://getvela.app", SigningLive.WALLET_TRANSPORT, ctx())
        assertTrue(own.dappOwn)
        assertEquals("Vela Wallet" to "", own.dappName to own.dappHost)
        assertTrue(own.dappIconUrls.isEmpty())
    }

    @Test
    fun `a site gets its own icon over its initial - https only`() {
        val site = model("https://app.uniswap.org", "tab-1", ctx())
        assertFalse(site.dappOwn)
        assertEquals("app.uniswap.org", site.dappHost)
        assertEquals(listOf("https://app.uniswap.org/apple-touch-icon.png", "https://app.uniswap.org/favicon.ico"), site.dappIconUrls)
        assertTrue(SigningLive.siteIconUrls("http://app.uniswap.org").isEmpty())
        assertTrue(SigningLive.siteIconUrls("https://").isEmpty())
    }

    @Test
    fun `sign with - the create flow's words, the choice marked, auto by default`() {
        val auto = model("https://app.uniswap.org", "tab-1", ctx()).signWith!!
        assertEquals("Sign with" to "Automatic", auto.label to auto.value)
        assertEquals(listOf("Automatic", "This device", "Phone or tablet", "USB security key", "Clear Signer"), auto.options.map { it.title })
        assertEquals(listOf("auto"), auto.options.filter { it.selected }.map { it.id })
        assertFalse(auto.open)
        // Spec 071: the fourth way says what it promises; the other three need no line.
        assertEquals(
            listOf(null, null, null, null, "Check and sign on a separate page — what you see is what you sign."),
            auto.options.map { it.line },
        )

        val key = model("https://app.uniswap.org", "tab-1", ctx("security_key", open = true)).signWith!!
        assertEquals("USB security key", key.value)
        assertTrue(key.open)

        val clear = model("https://app.uniswap.org", "tab-1", ctx("clear_signer")).signWith!!
        assertEquals("Clear Signer", clear.value)
    }

    @Test
    fun `the core pins the key of the chosen kind, and auto routes nothing`() {
        val keys = """[{"credential_id":"apple","transports":"hybrid,internal"},{"credential_id":"yubikey","transports":"nfc,usb"}]"""
        assertNull(uniffi.vela_core_uniffi.signRoute(keys, "auto"))
        val route = JSONObject(uniffi.vela_core_uniffi.signRoute(keys, "security_key")!!)
        // The YubiKey, not the first key: a security key cannot answer for a credential it does not hold.
        assertEquals("yubikey", route.getString("credential_id"))
        assertEquals("usb,nfc,ble", route.getString("transports"))
        assertEquals("security_key", route.getString("method"))
    }

    @Test
    fun `fees read to six decimals, rounded up, never as zero`() {
        // The device printed ~0,000410400290875302 ETH and crushed the row's label.
        assertEquals("0.000411", app.getvela.wallet.feature.send.SendLive.feeFromBase("410400290875302", 18))
        assertEquals("1.27", app.getvela.wallet.feature.send.SendLive.feeFromBase("1270000", 6))
        assertEquals("0.00000000013", app.getvela.wallet.feature.send.SendLive.feeFromBase("123456789", 18))
        assertEquals("0", app.getvela.wallet.feature.send.SendLive.feeFromBase("0", 18))
    }
}
