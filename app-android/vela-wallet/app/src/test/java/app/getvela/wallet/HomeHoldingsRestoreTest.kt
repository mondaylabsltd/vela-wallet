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
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.ChainDoc
import app.getvela.wallet.feature.wallet.core.HoldingsCache
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
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
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * P2.4: the home's first frame from what it held last time
 * (`holdings_restored`, the web's `holdings-cache.ts`), a page that draws no
 * balance noting the account without reading a network (`account_noted`,
 * perf-10), and a switch of space starting the account over
 * (`space_changed`, home-7) — on the real `balance_dashboard`.
 */
class HomeHoldingsRestoreTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEverything() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

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

    private class Harness(val host: CoreHost<BalanceView>, val transport: FakeRpcTransport)

    private fun harness(stall: (String) -> Boolean = { false }): Harness {
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }
        val transport = FakeRpcTransport(stall = stall) { _, _ -> FakeRpcTransport.body("0x14d1120d7b160000") }
        val networks = MutableStateFlow(NetView(loaded = true, networks = listOf(row(1, "ETH", "Ethereum"), row(100, "XDAI", "Gnosis"))))
        val pool = RpcPool(store = FakeStore(), endpoints = PerChainEndpoints(), scope = scope, transport = transport)
        val executor = BalanceExecutor(
            pool = pool,
            networks = networks,
            store = FakeStore(),
            chainInfo = { ChainDoc.Absent },
            mainnetPrices = { emptyMap() },
            chainDeadlineMs = 60_000,
        )
        val host = CoreHost(
            bridge = BalanceDashboardCore().asBridge(),
            scope = scope,
            initial = BalanceView(),
            serializer = BalanceView.serializer(),
            perform = JsonShell.perform(BalanceOperation.serializer(), BalanceShellResult.serializer(), executor::perform),
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

    private fun CoreHost<BalanceView>.send(event: BalanceEvent) = dispatch(event, BalanceEvent.serializer())

    private val kept = listOf(
        BalanceToken(chain_id = 100, symbol = "XDAI", name = "xDAI", balance = "3.25", decimals = 18, token_address = null, price_usd = 1.0),
        BalanceToken(chain_id = 1, symbol = "ETH", name = "Ether", balance = "0.5", decimals = 18, token_address = null, price_usd = 2000.0),
    )

    // -- holdings_restored --------------------------------------------------------

    @Test
    fun theKeptRowsOpenTheListBeforeAnyNetworkAnswers() {
        // Every network holds its connection open: nothing will answer in this test.
        val h = harness(stall = { true })
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        h.host.send(BalanceEvent.HoldingsRestored(ADDRESS, kept))
        val view = h.host.settle { it.tokens.size == 2 }
        assertEquals(setOf("XDAI", "ETH"), view.tokens.map { it.symbol }.toSet())
        assertEquals("USD-sorted", "ETH", view.tokens.first().symbol)
        assertTrue("still reading", view.last_refreshed_at_ms == null)
    }

    @Test
    fun anotherAccountsRowsAreNotRestored() {
        val h = harness(stall = { true })
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        h.host.send(BalanceEvent.HoldingsRestored(OTHER, kept))
        h.host.send(BalanceEvent.HoldingsRestored(ADDRESS, kept.take(1)))
        val view = h.host.settle { it.tokens.isNotEmpty() }
        assertEquals(listOf("XDAI"), view.tokens.map { it.symbol })
    }

    @Test
    fun aChainsOwnAnswerReplacesItsKeptRow() {
        val h = harness()
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        val settled = h.host.settle { it.last_refreshed_at_ms != null }
        // Once a round has settled, what it found is newer: the kept rows are ignored.
        h.host.send(BalanceEvent.HoldingsRestored(ADDRESS, kept))
        runBlocking { delay(300) }
        val after = h.host.view.value
        assertEquals(settled.tokens.map { it.balance }, after.tokens.map { it.balance })
        assertTrue("1.5 read, not the kept 0.5", after.tokens.none { it.balance == "0.5" })
    }

    // -- account_noted -------------------------------------------------------------

    @Test
    fun aNotedAccountIsSetAndNoNetworkIsRead() {
        val h = harness()
        h.host.send(BalanceEvent.AccountNoted(ADDRESS))
        val view = h.host.settle { it.address == ADDRESS }
        assertEquals(ADDRESS, view.address)
        runBlocking { delay(400) }
        assertTrue("no chain asked: ${h.transport.asked}", h.transport.asked.isEmpty())
        // The home showing the balance reads it.
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        h.host.settle { it.last_refreshed_at_ms != null }
        assertTrue(h.transport.asked.isNotEmpty())
    }

    @Test
    fun aNotedAccountTakesItsKeptRows() {
        val h = harness()
        h.host.send(BalanceEvent.AccountNoted(ADDRESS))
        h.host.send(BalanceEvent.HoldingsRestored(ADDRESS, kept))
        assertEquals(2, h.host.settle { it.tokens.size == 2 }.tokens.size)
        assertTrue(h.transport.asked.isEmpty())
    }

    // -- space_changed ---------------------------------------------------------------

    @Test
    fun aSwitchOfSpaceStartsTheAccountOverWithAForcedRead() {
        val h = harness()
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        val first = h.host.settle { it.last_refreshed_at_ms != null }.last_refreshed_at_ms!!
        val asked = h.transport.asked.size
        h.host.send(BalanceEvent.SpaceChanged)
        val again = h.host.settle { (it.last_refreshed_at_ms ?: 0.0) > first }
        assertTrue("read again", h.transport.asked.size > asked)
        assertEquals(ADDRESS, again.address)
    }

    @Test
    fun aSwitchOfSpaceWithNoAccountReadsNothing() {
        val h = harness()
        h.host.send(BalanceEvent.SpaceChanged)
        runBlocking { delay(300) }
        assertTrue(h.transport.asked.isEmpty())
    }

    @Test
    fun aSwitchOfSpaceTakesTheNewSpacesKeptRows() {
        val h = harness(stall = { true })
        h.host.send(BalanceEvent.AccountChanged(ADDRESS))
        h.host.send(BalanceEvent.SpaceChanged)
        h.host.send(BalanceEvent.HoldingsRestored(ADDRESS, kept))
        assertEquals(2, h.host.settle { it.tokens.size == 2 }.tokens.size)
    }

    // -- the cache --------------------------------------------------------------------

    @Test
    fun theCacheKeepsRowsPerAccountAndSpace() = runBlocking {
        val store = FakeStore()
        val cache = HoldingsCache(store, now = { 1_000L })
        cache.save(ADDRESS.uppercase().replace("0X", "0x"), "mainnet", kept)
        assertEquals(kept, cache.load(ADDRESS, "mainnet"))
        assertTrue("another space keeps its own", cache.load(ADDRESS, "testnet").isEmpty())
        assertTrue("another account keeps its own", cache.load(OTHER, "mainnet").isEmpty())
        assertTrue(store.values[HoldingsCache.KEY]!!.contains("${ADDRESS.lowercase()}#mainnet"))
    }

    @Test
    fun aDayOldEntryIsNotBelieved() = runBlocking {
        var clock = 0L
        val cache = HoldingsCache(FakeStore(), now = { clock })
        cache.save(ADDRESS, "mainnet", kept)
        clock = HoldingsCache.TTL_MS
        assertEquals(kept, cache.load(ADDRESS, "mainnet"))
        clock = HoldingsCache.TTL_MS + 1
        assertTrue(cache.load(ADDRESS, "mainnet").isEmpty())
    }

    @Test
    fun aCorruptStoreOrRowReadsAsEmptyOrIsLeftOut() = runBlocking {
        val store = FakeStore(mapOf(HoldingsCache.KEY to "{not json"))
        val cache = HoldingsCache(store, now = { 5L })
        assertTrue(cache.load(ADDRESS, "mainnet").isEmpty())
        store.values[HoldingsCache.KEY] =
            """{"${ADDRESS.lowercase()}#mainnet":{"at":5,"tokens":[{"chain_id":"one"},""" +
            """{"chain_id":1,"symbol":"ETH","name":"Ether","balance":"0.5","decimals":18,"token_address":null,"price_usd":2000.0,"spam":false}]}}"""
        assertEquals(listOf("ETH"), cache.load(ADDRESS, "mainnet").map { it.symbol })
    }

    @Test
    fun theCacheKeyIsInTheCoresStorageCatalog() {
        // Settings' cache clear removes what the core's catalog lists.
        assertEquals("balances", uniffi.vela_core_uniffi.storageItemOfKey(HoldingsCache.KEY))
        assertTrue(uniffi.vela_core_uniffi.storageIsCacheKey(HoldingsCache.KEY))
    }

    companion object {
        const val TIMEOUT = 15_000L
        const val ADDRESS = "0x14fb1fb21751e29f7ec48dc450017552e3d1ea5c"
        const val OTHER = "0x2222222222222222222222222222222222222222"
    }
}
