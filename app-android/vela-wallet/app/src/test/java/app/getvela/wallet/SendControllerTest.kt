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
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * The send controller over the REAL send core (the bridge, not a fake), with
 * the relay, the RPC pool and storage faked. What these pin is the shell's
 * half of a contract the core tests cannot see: the ids and events Android
 * hands the core.
 */
class SendControllerTest {
    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun tearDown() = scopes.forEach { it.cancel() }

    private class NoEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun token(symbol: String, chainId: Int, balance: String, address: String? = null) = BalanceToken(
        chain_id = chainId,
        symbol = symbol,
        name = symbol,
        balance = balance,
        decimals = if (address == null) 18 else 6,
        token_address = address,
        price_usd = 1.0,
    )

    private fun row(chainId: Long, symbol: String, name: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
    )

    private val holdings = BalanceView(
        tokens = listOf(
            token("USDT", 1, "53.48", "0xdac17f958d2ee523a2206206994597c13d831ec7"),
            token("POL", 137, "19.194439"),
            token("ETH", 42161, "0.002"),
        ),
    )

    private fun controller(): SendController {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val store = FakeStore()
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = NoEndpoints(),
            scope = scope,
            transport = FakeRpcTransport { _, _ -> RpcPostResult(RpcTransportOutcome.HttpError(503)) },
        )
        return SendController(
            scope = scope,
            relay = RelayClient(FakeRelayPort(), builtinBase = { "https://builtin.test" }, retryDelayMs = 0),
            pool = pool,
            feed = FeedExecutor(store = store, ownAccounts = { emptyList() }),
            accountStore = AccountStore(store),
            balances = { holdings },
            networks = { NetView(loaded = true, networks = listOf(row(1, "ETH", "Ethereum"), row(137, "POL", "Polygon"), row(42161, "ETH", "Arbitrum"))) },
            signer = { error("no signing in these tests") },
            haptic = {},
            refreshBalances = {},
        )
    }

    private val me = SendAccountRef(id = ME, address = ME, name = "Me")
    private val usd = SendDisplayContext(code = "USD", rate = 1.0, fiat_decimals = 2)

    private fun SendController.awaitView(what: String, test: (SendView) -> Boolean): SendView = runBlocking {
        withTimeout(10_000) { send.first(test) }.also { assertEquals(what, true, test(it)) }
    }

    /**
     * Issue #268: a token detail's Send opens on the form for THAT token. The
     * token rides into the open as the web's preselection, in the id format
     * the executor gives the core's token list (`chain-<id>`).
     */
    @Test
    fun `a token detail's send opens on the form for that token`() {
        val send = controller()
        send.open(
            account = me,
            display = usd,
            params = SendOpenParams(preselected_symbol = "POL", preselected_network = SendExecutor.network(137)),
        )

        val view = send.awaitView("on the form with POL") { it.stage == SendStage.EnterDetails && it.selected_token?.symbol == "POL" }
        assertEquals(137, view.selected_token?.chain_id)
    }

    /**
     * The device's version of #268: a send opened and closed earlier leaves
     * its token list on the machine. The shell used to pick against THAT list
     * before the new open reset it to the picker. With the preselection in the
     * open itself, a second send from a token detail still lands on the form.
     */
    @Test
    fun `a second send from a token detail still lands on the form`() {
        val send = controller()
        send.open(account = me, display = usd)
        send.awaitView("the first send lists the holdings") { it.stage == SendStage.SelectToken && it.tokens.size == 3 }

        send.open(
            account = me,
            display = usd,
            params = SendOpenParams(preselected_symbol = "ETH", preselected_network = SendExecutor.network(42161)),
        )

        val view = send.awaitView("on the form with Arbitrum ETH") { it.stage == SendStage.EnterDetails && it.selected_token?.chain_id == 42161 }
        assertEquals("ETH", view.selected_token?.symbol)
    }

    private companion object {
        const val ME = "0x576a2cc9e6adc0c95989fa6aa104290aa940c73f"
    }
}
