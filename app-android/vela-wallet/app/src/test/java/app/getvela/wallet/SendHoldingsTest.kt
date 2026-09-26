package app.getvela.wallet

import app.getvela.wallet.feature.onboarding.core.AccountStore
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.send.core.SendAccountRef
import app.getvela.wallet.feature.send.core.SendController
import app.getvela.wallet.feature.send.core.SendDisplayContext
import app.getvela.wallet.feature.send.core.SendExecutor
import app.getvela.wallet.feature.send.core.SendOpenParams
import app.getvela.wallet.feature.send.core.SendStage
import app.getvela.wallet.feature.send.core.SendView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.HoldingsFeed
import app.getvela.wallet.feature.wallet.core.HoldingsRound
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcPostResult
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.RpcTransportOutcome
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Spec 078: Send's holdings ARE the asset list's — one source (the balance
 * machine), over the REAL send core. The picker is answered from the round
 * the dashboard settled for this account; when none has, it waits (showing
 * what streamed in) instead of answering "nothing held"; and a round settled
 * later reaches an open send as `holdings_updated`, so a balance refreshed
 * behind the form is the balance beside the token.
 */
class SendHoldingsTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private class NoEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private class Feed : HoldingsFeed {
        override val view = MutableStateFlow(BalanceView())
        override val settled = MutableStateFlow<HoldingsRound?>(null)

        fun settle(view: BalanceView, at: Double) {
            this.view.value = view.copy(last_refreshed_at_ms = at)
            settled.value = HoldingsRound(address = view.address!!, atMs = at, view = this.view.value)
        }
    }

    private fun token(symbol: String, chainId: Int, balance: String) = BalanceToken(
        chain_id = chainId, symbol = symbol, name = symbol, balance = balance, decimals = 18, price_usd = 1.0,
    )

    private val port = FakeRelayPort()

    private fun controller(feed: HoldingsFeed, refresh: () -> Unit = {}): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val store = FakeStore()
        return SendController(
            scope = scope,
            relay = RelayClient(port, builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = RpcPool(
                store = FakeStore(),
                endpoints = NoEndpoints(),
                scope = scope,
                transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
            ),
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accountStore = AccountStore(store),
            balances = { error("read through the holdings feed, never the bare view") },
            networks = {
                NetView(
                    loaded = true,
                    networks = listOf(
                        NetNetworkRow(id = "gnosis", chain_id = 100, display_name = "Gnosis", native_symbol = "XDAI"),
                        NetNetworkRow(id = "polygon", chain_id = 137, display_name = "Polygon", native_symbol = "POL"),
                    ),
                )
            },
            signer = { error("no signing in these tests") },
            haptic = {},
            refreshBalances = refresh,
            holdings = feed,
        )
    }

    private val me = SendAccountRef(id = ME, address = ME, name = "Me")
    private val usd = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)

    private fun SendController.awaitView(what: String, test: (SendView) -> Boolean): SendView = runBlocking {
        runCatching { withTimeout(10_000) { send.first(test) } }
            .getOrElse { throw AssertionError("$what — last view: ${send.value}", it) }
    }

    @Test
    fun `the picker waits for the first settled round instead of answering nothing held`() = runBlocking<Unit> {
        val feed = Feed()
        val send = controller(feed)
        send.open(account = me, display = usd)
        // One chain has streamed in: shown, display-only.
        feed.view.value = BalanceView(address = ME, tokens = listOf(token("XDAI", 100, "0.7")))
        send.awaitView("the chain that answered, shown while the round is out") { v -> v.tokens.map { it.symbol } == listOf("XDAI") }
        delay(200)
        assertTrue("still loading its answer, never an empty list", send.send.value.stage == SendStage.SelectToken)
        feed.settle(BalanceView(address = ME, tokens = listOf(token("XDAI", 100, "0.7"), token("POL", 137, "19.19"))), at = 1_000.0)
        send.awaitView("the settled round") { v -> v.tokens.size == 2 }
    }

    @Test
    fun `a round settled later reaches the open form as the balance beside the token`() = runBlocking<Unit> {
        val feed = Feed()
        feed.settle(BalanceView(address = ME, tokens = listOf(token("POL", 137, "19.194439"), token("XDAI", 100, "0.7"))), at = 1_000.0)
        val send = controller(feed)
        send.open(account = me, display = usd, params = SendOpenParams(preselected_symbol = "POL", preselected_network = SendExecutor.network(137)))
        send.awaitView("on the form with POL") { it.stage == SendStage.EnterDetails && it.selected_token?.balance == "19.194439" }

        feed.settle(BalanceView(address = ME, tokens = listOf(token("POL", 137, "12.5"), token("XDAI", 100, "0.7"))), at = 2_000.0)
        send.awaitView("the new round's balance on the form") { it.selected_token?.balance == "12.5" }

        // Another account's round is not this screen's.
        feed.settle(BalanceView(address = OTHER, tokens = listOf(token("POL", 137, "99"))), at = 3_000.0)
        delay(300)
        assertEquals("12.5", send.send.value.selected_token?.balance)
    }

    /**
     * The desktop's retry-once rule: a round in which no chain answered (a
     * proxy blip at launch) is read again once before the picker refuses —
     * it used to say "could not load tokens" until the next ten-minute poll.
     */
    @Test
    fun `a round that reached nothing is read again once, and the next round answers`() = runBlocking<Unit> {
        val feed = Feed()
        // Real stamps: a round older than five minutes is also read again behind the answer.
        val now = System.currentTimeMillis().toDouble()
        feed.settle(BalanceView(address = ME, tokens = emptyList(), failed_chain_ids = listOf(1, 100, 137)), at = now)
        val refreshes = java.util.concurrent.atomic.AtomicInteger()
        val send = controller(feed) {
            refreshes.incrementAndGet()
            feed.settle(BalanceView(address = ME, tokens = listOf(token("XDAI", 100, "0.7"), token("POL", 137, "19.19"))), at = now + 1_000.0)
        }
        send.open(account = me, display = usd)
        send.awaitView("the re-read round's holdings") { it.stage == SendStage.SelectToken && it.tokens.size == 2 }
        assertEquals(1, refreshes.get())
        assertEquals(null, send.alert.value)
    }

    @Test
    fun `only a second round that reached nothing is the refusal`() = runBlocking<Unit> {
        val feed = Feed()
        // Real stamps: a round older than five minutes is also read again behind the answer.
        val now = System.currentTimeMillis().toDouble()
        feed.settle(BalanceView(address = ME, tokens = emptyList(), failed_chain_ids = listOf(1, 100)), at = now)
        val refreshes = java.util.concurrent.atomic.AtomicInteger()
        val send = controller(feed) {
            refreshes.incrementAndGet()
            feed.settle(BalanceView(address = ME, tokens = emptyList(), failed_chain_ids = listOf(1, 100)), at = now + 1_000.0)
        }
        send.open(account = me, display = usd)
        withTimeout(10_000) { send.alert.first { it != null } }
        assertEquals(app.getvela.wallet.feature.send.core.SendAlertKind.LoadTokensFailed, send.alert.value)
        assertEquals(1, refreshes.get())
    }

    @Test
    fun `an open picker reads ahead the fees of the chains the person holds`() = runBlocking<Unit> {
        val feed = Feed()
        feed.settle(BalanceView(address = ME, tokens = listOf(token("XDAI", 100, "0.7"), token("POL", 137, "19.19"))), at = 1_000.0)
        val send = controller(feed)
        send.open(account = me, display = usd)
        send.awaitView("the picker") { it.stage == SendStage.SelectToken && it.tokens.size == 2 }
        withTimeout(10_000) {
            while (port.calls.count { it.endsWith("vela_getInBandGasQuote") } < 2) delay(20)
        }
        assertTrue(port.calls.toString(), port.calls.any { it.endsWith("pimlico_getUserOperationGasPrice") })
    }

    private companion object {
        const val ME = "0x576a2cc9e6adc0c95989fa6aa104290aa940c73f"
        const val OTHER = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    }
}
