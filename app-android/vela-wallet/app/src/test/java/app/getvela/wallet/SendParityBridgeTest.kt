package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.flows.FlowBase
import app.getvela.wallet.feature.flows.FlowFixtures
import app.getvela.wallet.feature.flows.FlowState
import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.SendLive
import app.getvela.wallet.feature.send.core.FeeView
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The send form's ⇄ row, the figure's unit and the empty picker, end to end on
 * the real `send` machine (issues 197, 209, 231): the builder must say what the
 * CORE decided — `denom_toggle_shown` / `denom_toggle_enabled` /
 * `denom_toggle_reason` and `amount_fiat_code` — never what the shell guesses.
 */
class SendParityBridgeTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root") ?: error("vela.repo.root not set — run via Gradle")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }.apply { initialize("en") }
    }

    private val safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private fun ctx() = SendLive.Context(
        strings = strings,
        chainNames = mapOf(100 to "Gnosis"),
        explorers = emptyMap(),
        money = WalletLive.Money.of(CurrencyView(code = "USD")),
        fromName = "Me",
        fromAddress = safe,
    )

    private class Endpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)))
    }

    private fun controller(tokens: List<BalanceToken>): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val pool = RpcPool(store = FakeStore(), endpoints = Endpoints(), scope = scope, transport = FakeRpcTransport { _, _ -> FakeRpcTransport.body("0x") })
        runBlocking { pool.start() }
        return SendController(
            scope = scope,
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() }),
            accountStore = AccountStore(FakeStore()),
            balances = { BalanceView(address = safe, tokens = tokens.filter { it.price_usd != null }, unpriced_tokens = tokens.filter { it.price_usd == null }) },
            networks = { NetView(loaded = true, networks = listOf(NetNetworkRow(id = "gnosis", chain_id = 100, display_name = "Gnosis", native_symbol = "XDAI"))) },
            signer = { error("no signing in this test") },
            haptic = {},
            refreshBalances = {},
        )
    }

    private fun xdai(price: Double?) = BalanceToken(chain_id = 100, symbol = "XDAI", name = "xDAI", balance = "1.5", decimals = 18, price_usd = price)

    private fun SendController.settle(predicate: (SendView) -> Boolean): SendView =
        runBlocking { withTimeout(15_000) { send.first(predicate) } }

    /** Open, wait for the core's token list, pick the only token. */
    private fun SendController.pickOnly(display: SendDisplayContext): SendView {
        open(SendAccountRef(id = "cred", address = safe), display)
        val loaded = settle { it.tokens.isNotEmpty() }
        selectToken(SendLive.tokenId(loaded.tokens.single()))
        return settle { it.stage == SendStage.EnterDetails && it.selected_token != null }
    }

    private fun form(view: SendView) =
        SendLive.form((FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model, view, FeeView(), ctx())

    @Test
    fun `a priced token offers a live swap, and the unit follows the figure`() {
        val send = controller(listOf(xdai(1.0)))
        val view = send.pickOnly(SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
        assertTrue(view.denom_toggle_shown)
        assertTrue(view.denom_toggle_enabled)
        val tokenMode = form(view).amount!!
        assertTrue(tokenMode.denomShown)
        assertTrue(tokenMode.denomEnabled)
        // Token units: the ticker follows the figure.
        assertNull(tokenMode.unitPrefix)
        assertEquals("XDAI", tokenMode.unitSuffix)

        send.toggleFiatInput()
        val fiat = send.settle { it.amount_fiat_code != null }
        assertEquals("USD", fiat.amount_fiat_code)
        val fiatMode = form(fiat).amount!!
        // The figure's own currency leads it.
        assertEquals("$", fiatMode.unitPrefix)
        assertNull(fiatMode.unitSuffix)
        assertEquals("USD", fiatMode.denomLabel)
    }

    @Test
    fun `an unpriced token draws no swap row at all`() {
        val send = controller(listOf(xdai(null)))
        val view = send.pickOnly(SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
        assertFalse(view.denom_toggle_shown)
        val amount = form(view).amount!!
        assertFalse(amount.denomShown)
    }

    @Test
    fun `a currency with no rate dims the swap and says why`() {
        val send = controller(listOf(xdai(1.0)))
        val view = send.pickOnly(SendDisplayContext(code = "EUR", rate = null, fiat_decimals = 2))
        assertTrue(view.denom_toggle_shown)
        assertFalse(view.denom_toggle_enabled)
        assertNotNull(view.denom_toggle_reason)
        val live = form(view)
        assertTrue(live.amount!!.denomShown)
        assertFalse(live.amount!!.denomEnabled)
        assertEquals(
            strings.t(I18nKeys.Flows.DENOM_TOGGLE_NO_RATE, mapOf("code" to "EUR", "symbol" to "XDAI")),
            live.warning,
        )
    }

    @Test
    fun `an account holding nothing says so, not that nothing matched`() {
        val send = controller(emptyList())
        val before = send.send.value
        send.open(SendAccountRef(id = "cred", address = safe), SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
        val view = send.settle { it !== before && !it.loading }
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val pick = SendLive.pick(drawn.model, view, ctx())
        assertTrue(pick.rows.isEmpty())
        assertEquals(strings.t(I18nKeys.Flows.NO_TOKENS_WITH_BALANCE), pick.empty)
    }
}
