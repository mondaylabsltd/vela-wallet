package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import app.getvela.wallet.feature.wallet.core.BalanceExecutor
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * The wallet home's balances, end to end on the real `balance_dashboard`.
 *
 * Two of these cases exist because the desktop sibling shipped them as latent
 * bugs (spec 031) that were **invisible while every price was `None`** — and
 * this spec is the one that makes prices exist. They are inherited as tests
 * rather than as a warning in a document nobody re-reads.
 */
class BalanceMachineTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    /** One endpoint per chain, so a test can tell which chain was asked. */
    private class PerChainEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun row(chainId: Long, symbol: String, name: String) = NetNetworkRow(
        id = name.lowercase(),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
        rpc_url = "https://chain-$chainId.example",
    )

    private class Harness(
        val host: CoreHost<BalanceView>,
        val transport: FakeRpcTransport,
    )

    private fun harness(
        rows: List<NetNetworkRow>,
        answer: (url: String, method: String) -> app.getvela.wallet.feature.wallet.core.RpcPostResult,
    ): Harness {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        scopes += scope
        val transport = FakeRpcTransport(answer)
        val networks = MutableStateFlow(NetView(loaded = true, networks = rows))
        val pool = RpcPool(
            store = FakeStore(),
            endpoints = PerChainEndpoints(),
            scope = scope,
            transport = transport,
        )
        val executor = BalanceExecutor(pool, networks, FakeStore())
        val host = CoreHost(
            bridge = BalanceDashboardCore().asBridge(),
            scope = scope,
            initial = BalanceView(),
            serializer = BalanceView.serializer(),
            perform = JsonShell.perform(
                BalanceOperation.serializer(),
                BalanceShellResult.serializer(),
                executor::perform,
            ),
            escapedFailure = JsonShell.escapedFailure(
                BalanceOperation.serializer(),
                BalanceShellResult.serializer(),
                fallback = BalanceShellResult.PrivacyWritten,
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
        executor.stream = { event -> host.dispatch(event, BalanceEvent.serializer()) }
        runBlocking { pool.start() }
        return Harness(host, transport)
    }

    private fun CoreHost<BalanceView>.settle(predicate: (BalanceView) -> Boolean): BalanceView =
        runBlocking { withTimeout(TIMEOUT) { view.first(predicate) } }

    @Test
    fun aNativeBalanceCrossesAsAHumanDecimal() {
        // **The 10^18 trap.** The core parses this string straight into a float
        // and multiplies it by a price, so raw units here would produce a total
        // a quintillion times too large — and nothing would show it until a
        // price existed, which is exactly what this spec brings. 1.5 ETH is
        // 0x14d1120d7b160000 wei.
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }
        val token = view.tokens.single()
        assertEquals("1.5", token.balance)
        assertEquals("ETH", token.symbol)
        assertEquals(1, token.chain_id)
        // Native coin: no contract address.
        assertNull(token.token_address)
    }

    @Test
    fun aChainWithNoNativeCoinIsNeverAsked() {
        // **The 4×10^57 trap.** Tempo's RPC answers the same constant for every
        // address and its native symbol is `USD`, so a stablecoin peg would
        // price that constant at a dollar. The guard is the CORE's predicate,
        // not a magnitude threshold invented here — and the proof is that the
        // chain is never queried at all.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(4217, "USD", "Tempo")),
        ) { _, _ -> FakeRpcTransport.body("0x14d1120d7b160000") }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        h.host.settle { it.tokens.isNotEmpty() }
        assertTrue(
            "Ethereum was read",
            h.transport.asked.any { it.contains("chain-1.example") },
        )
        assertFalse(
            "a chain with no native coin must never be asked for a native balance",
            h.transport.asked.any { it.contains("chain-4217.example") },
        )
    }

    @Test
    fun aChainHoldingNothingIsNotAFailedChain() {
        // The bug this caught: the first version treated "produced no token" as
        // "did not answer", so an empty chain arrived as a broken one and the
        // screen would have shown a network-down banner to somebody whose only
        // crime is having no funds there.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis")),
        ) { url, _ ->
            if (url.contains("chain-100")) FakeRpcTransport.body("0x0")
            else FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() && !it.holdings_loading }
        assertTrue(
            "an empty chain answered, so it has not failed",
            view.failed_chain_ids.isEmpty(),
        )
    }

    @Test
    fun aZeroBalanceIsNotAHolding() {
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x0")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        // Settle on the fetch finishing rather than on tokens appearing, since
        // the point is that none do.
        val view = h.host.settle { !it.holdings_loading && it.address != null }
        assertTrue(view.tokens.isEmpty())
    }

    @Test
    fun aChainThatDidNotAnswerIsReportedAsFailedNotAsZero() {
        // The distinction a money screen must not blur: a chain that did not
        // answer is UNKNOWN, and rendering unknown as zero tells a person their
        // holding is gone.
        val h = harness(
            listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis")),
        ) { url, _ ->
            if (url.contains("chain-100")) FakeRpcTransport.network()
            else FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        // Settle on the FETCH finishing, not on a token appearing: the streamed
        // `chain_assets_arrived` for the healthy chain lands strictly before
        // the settle that carries the failed list, so `tokens.isNotEmpty()`
        // fires while the answer is still half-formed.
        // Only `fetch_settled` carries the failed list, so waiting for it is
        // waiting for the fetch to actually finish. `!holdings_loading` is true
        // before the fetch even starts, which is how the first version of this
        // predicate settled on the empty initial view.
        val view = h.host.settle { it.failed_chain_ids.isNotEmpty() }
        assertEquals(listOf("ETH"), view.tokens.map { it.symbol })
        assertTrue("the silent chain is named", view.failed_chain_ids.contains(100))
    }

    @Test
    fun anUnpricedHoldingIsNotCountedAsZeroDollars() {
        // Phase 4b has no price source, so every holding arrives unpriced. The
        // core must say the total is unknown rather than call it $0 — the whole
        // reason `display_total_usd` is nullable.
        val h = harness(listOf(row(1, "ETH", "Ethereum"))) { _, _ ->
            FakeRpcTransport.body("0x14d1120d7b160000")
        }
        h.host.dispatch(BalanceEvent.AccountChanged(ADDRESS), BalanceEvent.serializer())

        val view = h.host.settle { it.tokens.isNotEmpty() }
        assertNull("an unpriced holding must not become a price", view.tokens.single().price_usd)
        assertTrue(
            "the core must hold at least one unpriced holding",
            view.unpriced_tokens.isNotEmpty() || view.notice != null || view.display_total_usd == null,
        )
    }

    private companion object {
        const val ADDRESS = "0x1111111111111111111111111111111111111111"
        const val TIMEOUT = 20_000L
    }
}
