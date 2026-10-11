package app.getvela.wallet

import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.core.marks.Marks
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
import app.getvela.wallet.feature.send.core.SendFeeCoin
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
        money = WalletLive.Money.of(CurrencyView(code = "USD", committed = true)),
        fromName = "Me",
        fromAddress = safe,
    )

    private class Endpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)))
    }

    private val gnosis = NetNetworkRow(id = "gnosis", chain_id = 100, display_name = "Gnosis", native_symbol = "XDAI")

    private fun controller(tokens: List<BalanceToken>, networks: List<NetNetworkRow> = listOf(gnosis), accounts: FakeStore = FakeStore()): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val pool = RpcPool(store = FakeStore(), endpoints = Endpoints(), scope = scope, transport = FakeRpcTransport { _, _ -> FakeRpcTransport.body("0x") })
        runBlocking { pool.start() }
        return SendController(
            scope = scope,
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() }),
            accountStore = AccountStore(accounts),
            balances = { BalanceView(address = safe, tokens = tokens.filter { it.price_usd != null }, unpriced_tokens = tokens.filter { it.price_usd == null }) },
            networks = { NetView(loaded = true, networks = networks) },
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
        // The core spells a built-in chain's own coin the registry's way.
        assertEquals("xDAI", tokenMode.unitSuffix)

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
            strings.t(I18nKeys.Flows.DENOM_TOGGLE_NO_RATE, mapOf("code" to "EUR", "symbol" to "xDAI")),
            live.warning,
        )
    }

    /**
     * C10 on the real machines: the fee row's coin with no estimate in hand
     * is the CORE's (`SendView.fee_coin`). This relay answers nothing, so
     * every quote fails: BNB Chain's own coin while nobody chose — never the
     * empty disc — and USDT the moment USDT is chosen, before (and without)
     * any estimate of it.
     */
    @Test
    fun `with no estimate the fee row wears the cores coin`() {
        Marks.base = "https://data.example/"
        try {
            val usdt = "0x55d398326f99059fF775485246999027B3197955"
            val send = controller(
                listOf(
                    BalanceToken(chain_id = 56, symbol = "BNB", name = "BNB", balance = "0.01", decimals = 18, price_usd = 600.0),
                    BalanceToken(chain_id = 56, symbol = "USDT", name = "Tether USD", balance = "50", decimals = 18, token_address = usdt, price_usd = 1.0),
                ),
                networks = listOf(gnosis, NetNetworkRow(id = "bsc", chain_id = 56, display_name = "BNB Chain", native_symbol = "BNB")),
            )
            send.open(SendAccountRef(id = "cred", address = safe), SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
            val loaded = send.settle { it.tokens.size == 2 }
            send.selectToken(SendLive.tokenId(loaded.tokens.single { it.token_address == null }))
            val failed = send.settle { it.stage == SendStage.EnterDetails && it.selected_token != null && !it.estimating_gas && !it.fee_busy }
            assertNull("this relay prices nothing", failed.fee)
            assertEquals(SendFeeCoin(symbol = "BNB", contract = null, chain_id = 56), failed.fee_coin)
            val drawn = SendLive.form(sd2(), failed, send.fee.value, ctx()).fee
            assertEquals(WalletLive.mark(56, "BNB", null), drawn.mark)
            assertTrue("BNB's logo, not an empty disc", drawn.mark.logoUrls.isNotEmpty())

            send.chooseFeeToken(usdt)
            val chosen = send.settle { it.fee_coin?.contract != null }
            assertNull(chosen.fee)
            assertEquals("USDT", chosen.fee_coin!!.symbol)
            assertTrue(chosen.fee_coin!!.contract.equals(usdt, ignoreCase = true))
            assertEquals(56, chosen.fee_coin!!.chain_id)
            val usdtRow = SendLive.form(sd2(), chosen, send.fee.value, ctx()).fee
            assertEquals(WalletLive.mark(56, "USDT", chosen.fee_coin!!.contract), usdtRow.mark)
            assertTrue(usdtRow.mark.logoUrls.first().startsWith("https://data.example/assets/eip155-56/"))
        } finally {
            Marks.base = ""
        }
    }

    private fun sd2() = (FlowFixtures.build(FlowState.SD2, strings).base as FlowBase.SendForm).model

    /**
     * PR 2 notes 10 and 13 on the real machines. Nothing answers, so the
     * account read fails as the chain's: the form's fee row says why (the
     * dash, the reason naming Gnosis), the core retries it by itself — and
     * Continue's estimate passes the fee machine's own failure through, so
     * the alert says the chain is out of reach, never "could not build a
     * valid transaction estimate".
     */
    @Test
    fun `a failed fee says why on the form, and continue's alert names its cause`() {
        // The account on this device: Continue reaches the fee, not "account unavailable".
        val record = org.json.JSONObject().put("id", "cred").put("name", "Me").put("address", safe).put("public_key_hex", "04" + "ab".repeat(64))
        val send = controller(listOf(xdai(1.0)), accounts = FakeStore(mapOf("vela.accounts" to "[$record]", "vela.activeAccountIndex" to "0")))
        send.pickOnly(SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
        send.setRecipient("0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        send.setAmount("0.5")
        send.settle { it.can_continue }
        send.continueTapped()
        val alert = runBlocking { withTimeout(30_000) { send.alert.first { it != null } } }!!
        assertEquals(
            app.getvela.wallet.feature.send.core.SendAlertKind.EstimateFailed(
                app.getvela.wallet.feature.send.core.SendEstimateFailure.Fee(app.getvela.wallet.feature.send.core.FeeFailure.ChainRead(rate_limited = false)),
            ),
            alert,
        )
        val (title, body) = SendLive.alertText(alert, strings, "Gnosis")
        assertEquals(strings.t(I18nKeys.Flows.ALERT_ESTIMATE_TITLE), title)
        assertEquals(strings.t(I18nKeys.Flows.ALERT_ESTIMATE_CHAIN_DOWN_BODY, mapOf("chain" to "Gnosis")), body)

        val failed = runBlocking { withTimeout(15_000) { send.fee.first { it.failure != null } } }
        val row = SendLive.form(sd2(), send.send.value, failed, ctx(), SendLive.SpeedInputs(send.speed.value) { null }).fee
        assertEquals("—", row.value)
        assertEquals(strings.t(I18nKeys.Flows.FEE_REASON_CHAIN_DOWN, mapOf("chain" to "Gnosis")), row.reason)
        assertTrue("the core retries it by itself", failed.failure!!.auto_retry)
        // A tap on the failed row is the retry, not the coin list.
        assertEquals(app.getvela.wallet.feature.send.core.FeeFailureRow.Tap.Retry, send.feeTapped())
        send.left()
    }

    @Test
    fun `an account holding nothing says so, not that nothing matched`() {
        val send = controller(emptyList())
        val before = send.commits.value
        send.open(SendAccountRef(id = "cred", address = safe), SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2))
        // An empty account's settled view can `equal` the one before the open,
        // and StateFlow never re-emits an equal value: waiting on the VIEW for
        // "it changed" timed out now and then. The commit counter always moves.
        runBlocking { withTimeout(15_000) { send.commits.first { it > before && !send.send.value.loading } } }
        val view = send.send.value
        val drawn = FlowFixtures.build(FlowState.SD1, strings).base as FlowBase.SendPick
        val pick = SendLive.pick(drawn.model, view, ctx())
        assertTrue(pick.rows.isEmpty())
        assertEquals(strings.t(I18nKeys.Flows.NO_TOKENS_WITH_BALANCE), pick.empty)
    }
}
