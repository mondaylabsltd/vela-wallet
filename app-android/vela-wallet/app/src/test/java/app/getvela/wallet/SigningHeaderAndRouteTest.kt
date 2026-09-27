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
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Founder review 2026-09-19, on the native sheets: the wallet's own request
 * wears the wallet's mark and name, a site gets its own icon with its initial
 * as the fallback, the chip carries the chain's logo. Where the passkey is was
 * said at sign-in (founder, 2026-09-26): the sheet asks nothing about it.
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

    private fun ctx() =
        SigningLive.Context(strings, "Ethereum", Color.Red, "ETH", "Mine", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", chainId = 1)

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

    /**
     * Founder, 2026-09-26: 「这个账户只能用当前登录的钥匙签名」. The sheet names the
     * signer and nothing else about the key — no "Sign with", no methods.
     */
    @Test
    fun `the sheet asks nothing about where the passkey is`() {
        val sheet = model("https://app.uniswap.org", "tab-1", ctx()).toString()
        listOf(
            "componentsUi.signing.signWith",
            "common.automatic",
            "onboarding.create.methodPlatformTitle",
            "onboarding.create.methodHybridTitle",
            "onboarding.create.methodSecurityKeyTitle",
        ).forEach { key ->
            assertFalse("the sheet says \"${strings.t(key)}\" ($key)", sheet.contains(strings.t(key)))
        }
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
