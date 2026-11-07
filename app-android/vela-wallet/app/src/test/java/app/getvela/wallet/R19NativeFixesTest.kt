package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.i18n.I18nCatalogs
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.wallet.SectionMode
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletHomeModel
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceEvent
import app.getvela.wallet.feature.wallet.core.BalanceExecutor
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.FeedOperation
import app.getvela.wallet.feature.wallet.core.FeedShellResult
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.RpcEndpointSeed
import app.getvela.wallet.feature.wallet.core.RpcEndpointSource
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcSeeds
import app.getvela.wallet.feature.wallet.core.RpcSource
import app.getvela.wallet.feature.wallet.core.feedFollowsBalance
import java.io.File
import java.util.concurrent.CopyOnWriteArrayList
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.vela_core_uniffi.ActivityFeedCore
import uniffi.vela_core_uniffi.BalanceDashboardCore

/**
 * Round 19's Android fixes (docs/multisig-machines.md §33):
 *
 * - r18-1: "No activity yet" is never said while a network of the space is
 *   unread — the balance's reads are handed to the feed (`balance_read`).
 */
class R19NativeFixesTest {

    private val repoRoot = File(System.getProperty("vela.repo.root") ?: error("vela.repo.root not set"))
    private val assets = File(repoRoot, "assets/i18n")
    private val strings: I18nRuntime = I18nRuntime(
        readOnDemand = { namespace, tag -> File(assets, I18nCatalogs.assetPath(tag, namespace).removePrefix("i18n/")).takeIf { it.isFile }?.readBytes() },
    ) { tag -> File(assets, "$tag.json").readBytes() }.apply {
        initialize("en")
        include(I18nCatalogs.MULTISIG)
    }

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stop() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun scope() = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }

    // -- r18-1: a hung network, through the real balance and feed cores ------

    private class PerChainEndpoints : RpcEndpointSource {
        override suspend fun forChain(chainId: Int) = RpcSeeds(
            rpc = listOf(RpcEndpointSeed("https://chain-$chainId.example", RpcSource.Default)),
        )
    }

    private fun row(chainId: Long, symbol: String, name: String) = NetNetworkRow(
        id = name.lowercase().replace(' ', '-'),
        chain_id = chainId,
        display_name = name,
        native_symbol = symbol,
        rpc_url = "https://chain-$chainId.example",
    )

    private val chainNames = mapOf(11155111 to "Sepolia", 11155420 to "OP Sepolia")
    private val rows = listOf(row(11155111, "ETH", "Sepolia"), row(11155420, "ETH", "OP Sepolia"))

    private class World(val balance: CoreHost<BalanceView>, val feed: CoreHost<FeedView>)

    /**
     * The home's two machines as the app runs them: the real
     * `balance_dashboard` over a pool whose OP Sepolia node never answers
     * ([hung]), the real `activity_feed` over an empty store and a scan that
     * reads everything it asks (an empty account: the scan reads nowhere the
     * account holds or lives) — and the shell's wiring between them.
     */
    private fun world(hung: Boolean, deadlineMs: Long = 2_500): World {
        val scope = scope()
        val transport = FakeRpcTransport(stall = { url -> hung && url.contains("chain-11155420") }) { _, _ -> FakeRpcTransport.body("0x0") }
        val pool = RpcPool(store = FakeStore(), endpoints = PerChainEndpoints(), scope = scope, transport = transport)
        val executor = BalanceExecutor(
            pool = pool,
            networks = MutableStateFlow(NetView(loaded = true, networks = rows)),
            store = FakeStore(),
            chainInfo = { app.getvela.wallet.feature.wallet.core.ChainDoc.Absent },
            mainnetPrices = { emptyMap() },
            chainDeadlineMs = deadlineMs,
        )
        val balance = CoreHost(
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
        executor.stream = { event -> balance.dispatch(event, BalanceEvent.serializer()) }
        runBlocking { pool.start() }
        val feedExecutor = FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() })
        feedExecutor.scan = { FeedExecutor.ScanAnswer(0) }
        val feed = CoreHost(
            bridge = ActivityFeedCore().asBridge(),
            scope = scope,
            initial = FeedView(),
            serializer = FeedView.serializer(),
            perform = JsonShell.perform(FeedOperation.serializer(), FeedShellResult.serializer(), feedExecutor::perform),
            escapedFailure = JsonShell.escapedFailure(
                FeedOperation.serializer(),
                FeedShellResult.serializer(),
                fallback = FeedShellResult.HapticPlayed,
                answer = feedExecutor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
        // The production wiring under test.
        feedFollowsBalance(scope, balance.view) { event -> feed.dispatch(event, FeedEvent.serializer()) }
        feed.dispatch(FeedEvent.SpaceChainsChanged(rows.map { it.chain_id.toInt() }), FeedEvent.serializer())
        balance.dispatch(BalanceEvent.AccountChanged(ACCOUNT), BalanceEvent.serializer())
        feed.dispatch(FeedEvent.AccountSwitched(ACCOUNT), FeedEvent.serializer())
        return World(balance, feed)
    }

    private fun home(balance: BalanceView, feed: FeedView): WalletHomeModel = WalletLive.home(
        WalletFixtures.buildMobileState(WalletScreenState.H1, strings),
        balance,
        feed,
        CurrencyView(code = "USD", committed = true),
        strings,
        chainNames,
        shownAccount = ACCOUNT,
    )

    private val noActivity get() = strings.t("home.emptyNoActivity")

    private fun saysNoActivity(model: WalletHomeModel) =
        model.activitySection.mode == SectionMode.Empty && model.activitySection.empty?.title == noActivity

    /** Every home the two machines drew, sampled on every move of either. */
    private fun drawn(world: World): CopyOnWriteArrayList<Pair<BalanceView, WalletHomeModel>> {
        val frames = CopyOnWriteArrayList<Pair<BalanceView, WalletHomeModel>>()
        scope().launch {
            combine(world.balance.view, world.feed.view) { b, f -> b to f }.collect { (b, f) -> frames += b to home(b, f) }
        }
        return frames
    }

    @Test
    fun `r18-1 no 'No activity yet' while a network is hung, then the unread line, never the claim`() {
        val world = world(hung = true)
        val frames = drawn(world)
        // The hung network's read runs out at the deadline: OP Sepolia is unread.
        runBlocking { withTimeout(20_000) { world.balance.view.first { it.networks_read } } }
        val line = strings.t("home.activityUnreadOne", mapOf("network" to "OP Sepolia"))
        val said = runBlocking {
            withTimeout(10_000) {
                while (frames.none { it.second.activitySection.empty?.title == line }) kotlinx.coroutines.delay(25)
                frames.last { it.second.activitySection.empty?.title == line }.second
            }
        }
        assertEquals("the unread line in the empty state's place", SectionMode.Empty, said.activitySection.mode)
        runBlocking { kotlinx.coroutines.delay(1_500) }
        val claimed = frames.filter { saysNoActivity(it.second) }
        assertTrue(
            "'No activity yet' was drawn while OP Sepolia was unread: ${claimed.map { it.first.networks_read to it.first.unread_chain_ids }}",
            claimed.isEmpty(),
        )
        // While the read was out, the honest surface was the skeleton.
        val beforeRead = frames.filter { !it.first.networks_read }.map { it.second.activitySection.mode }
        assertTrue("frames while the hung read was out", beforeRead.isNotEmpty())
        assertTrue(beforeRead.all { it == SectionMode.Loading })
    }

    @Test
    fun `r18-1 every network read and nothing there - 'No activity yet' is said`() {
        val world = world(hung = false)
        val frames = drawn(world)
        val said = runBlocking {
            withTimeout(20_000) {
                while (frames.none { saysNoActivity(it.second) }) kotlinx.coroutines.delay(25)
                frames.first { saysNoActivity(it.second) }
            }
        }
        assertTrue("said only once the balance's first read was over", said.first.networks_read)
        assertEquals(emptyList<Int>(), said.first.unread_chain_ids)
        assertNotEquals(SectionMode.Loading, said.second.activitySection.mode)
    }

    @Test
    fun `r18-1 the balance's word reaches the feed's wire and the unread network is named, never by its id`() {
        val view = app.getvela.wallet.core.crux.Wire.json.decodeFromString(
            BalanceView.serializer(),
            """{"address":"$ACCOUNT","networks_read":true,"unread_chain_ids":[11155420],"tokens":[]}""",
        )
        val event = app.getvela.wallet.feature.wallet.core.balanceRead(view)
        assertEquals(FeedEvent.BalanceRead(ACCOUNT, read = true, unread_chain_ids = listOf(11155420)), event)
        val wire = app.getvela.wallet.core.crux.Wire.json.encodeToString(FeedEvent.serializer(), event)
        assertTrue(wire, wire.contains("\"type\":\"balance_read\"") && wire.contains("\"unread_chain_ids\":[11155420]"))
        // The core's name for a network this device's lists do not have.
        val feed = FeedView(account = ACCOUNT, unread_chains = listOf(987_654_321), unread_key = "home.activityUnreadOne", unread_network = "Far Chain")
        assertEquals(
            strings.t("home.activityUnreadOne", mapOf("network" to "Far Chain")),
            WalletLive.unreadLine(feed, strings, chainNames),
        )
        val nameless = WalletLive.unreadLine(feed.copy(unread_network = null), strings, chainNames)!!
        assertTrue(nameless, !nameless.contains("987654321"))
    }

    companion object {
        const val ACCOUNT = "0x1111111111111111111111111111111111111111"
    }
}
