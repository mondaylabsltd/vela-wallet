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
 * Founder review 2026-09-19, on the native sheets: a site gets its own icon
 * with its initial as the fallback, the chip carries the chain's logo; the
 * wallet's own request (2026-10-08) names no requester at all. Where the passkey is was
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

    private fun model(origin: String, transport: String, ctx: SigningLive.Context, firstParty: Boolean = false) = SigningLive.model(
        drawn,
        IncomingRequest("r1", "eth_sendTransaction", params, origin, transport, 1),
        SignView(
            surface = SignSurface.Sheet,
            request = SignRequestView("r1", "eth_sendTransaction", SignMethodKind.Transaction, params, origin, null, 1, null, first_party = firstParty),
            confirm_gate_open = true,
        ),
        clear, GuardView(), FeeView(confirm_fee_ready = true), ctx,
    )

    private fun ctx() =
        SigningLive.Context(strings, "Ethereum", Color.Red, "ETH", "Mine", "0x88cCA0EeDbF2C4426110bbFc998F048689266894", chainId = 1)

    /**
     * The wallet's own request (the core's `first_party`) names no requester —
     * no mark, no "Vela Wallet", no chip: its header is its intent and the ✕,
     * and the intent is not said again below. Only the core's word makes it
     * so: a page on the wallet's own origin, sending the same request, is a page.
     */
    @Test
    fun `the wallet's own request is not a site`() {
        val own = model("https://getvela.app", SigningLive.WALLET_TRANSPORT, ctx(), firstParty = true)
        assertTrue(own.dappOwn)
        assertEquals("" to "", own.dappName to own.dappHost)
        assertTrue(own.dappIconUrls.isEmpty())
        assertEquals(strings.t("componentsUi.signing.intentContractCall"), own.headline)
        assertTrue(own.blocks.none { it is app.getvela.wallet.feature.signing.SigningBlock.Intent })

        val page = model("https://getvela.app", "tab-1", ctx())
        assertFalse(page.dappOwn)
        assertEquals(null, page.headline)
        assertEquals("getvela.app", page.dappName)
        assertTrue(page.blocks.first() is app.getvela.wallet.feature.signing.SigningBlock.Intent)
    }

    @Test
    fun `a site gets its own icon over its initial - https only`() {
        val site = model("https://app.uniswap.org", "tab-1", ctx())
        assertFalse(site.dappOwn)
        // Its name is its host, said once (spec 079 F14).
        assertEquals("app.uniswap.org", site.dappName)
        assertEquals("", site.dappHost)
        assertEquals(listOf("https://app.uniswap.org/apple-touch-icon.png", "https://app.uniswap.org/favicon.ico"), site.dappIconUrls)
        assertTrue(SigningLive.siteIconUrls("http://app.uniswap.org").isEmpty())
        assertTrue(SigningLive.siteIconUrls("https://").isEmpty())
    }

    /**
     * Spec 082 RE7/RE13 (G12): the name is the core's `browserSiteLabel` — a
     * host said once, whole, its port kept; the header wraps it to two lines
     * rather than cutting the end a spoofer controls.
     */
    @Test
    fun `a long host is the whole name, said once`() {
        val long = "app.a-rather-long-subdomain-name.of-some-decentralised-exchange.example"
        val site = model("https://$long", "tab-1", ctx())
        assertEquals(long, site.dappName)
        assertEquals("", site.dappHost)
        assertEquals("127.0.0.1:8137", model("http://127.0.0.1:8137", "tab-1", ctx()).dappName)
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
