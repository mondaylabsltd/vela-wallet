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
import app.getvela.wallet.navigation.LaunchFront
import app.getvela.wallet.feature.onboarding.core.SessionRoute
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

    // -- r18-3: a cold start with a multisig in front ------------------------

    private val ann = """{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","public_key_hex":"04ab","created_at_iso":"2026-08-25T10:00:00.000Z","keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}"""
    private val multisig = "0x5afe00000000000000000000000000000000cafe"

    /**
     * The launch as the app runs it: the real session machine over a real
     * store, the stored front read before it boots, the hub loading
     * [hubAfterMs] after the session says Wallet, the NavHost settling on the
     * wallet [navAfterMs] after it. Every screen the root would draw, in order.
     */
    private fun launch(store: FakeStore, hubAfterMs: Long = 400, navAfterMs: Long = 150): List<LaunchFront.Screen> = runBlocking {
        val session = app.getvela.wallet.feature.onboarding.core.SessionController(
            app.getvela.wallet.feature.onboarding.core.AccountStore(store),
            scope(),
        )
        val stored = MutableStateFlow<LaunchFront.Stored>(LaunchFront.Stored.Unread)
        val hubLoaded = MutableStateFlow(false)
        val settled = MutableStateFlow(false)
        val screens = CopyOnWriteArrayList<LaunchFront.Screen>()
        val sampler = scope().launch {
            combine(session.view, stored, hubLoaded, settled) { s, f, h, n -> LaunchFront.screen(s.allowedRoute, f, h, expired = false, walletSettled = n) }
                .collect { screens += it }
        }
        // The app's order (VelaWalletApplication): the front read, then the boot.
        stored.value = LaunchFront.read(store)
        session.boot()
        withTimeout(10_000) { session.view.first { it.allowedRoute != SessionRoute.Loading } }
        if (session.view.value.allowedRoute == SessionRoute.Wallet) {
            scope().launch { kotlinx.coroutines.delay(navAfterMs); settled.value = true }
            scope().launch { kotlinx.coroutines.delay(hubAfterMs); hubLoaded.value = true }
        }
        kotlinx.coroutines.delay(maxOf(hubAfterMs, navAfterMs) + 300)
        sampler.cancel()
        screens.toList().distinct()
    }

    @Test
    fun `r18-3 a cold start with a multisig in front never draws Welcome or the Vela account's home`() {
        val store = FakeStore(mapOf("vela.accounts" to "[$ann]", LaunchFront.ACTIVE_KEY to multisig))
        val screens = launch(store)
        assertEquals("ground until the hub has the multisig, then the wallet: $screens", listOf(LaunchFront.Screen.Ground, LaunchFront.Screen.Wallet), screens)
        // Hub loaded before the NavHost settled: still the ground until it has.
        assertEquals(listOf(LaunchFront.Screen.Ground, LaunchFront.Screen.Wallet), launch(FakeStore(mapOf("vela.accounts" to "[$ann]", LaunchFront.ACTIVE_KEY to multisig)), hubAfterMs = 50, navAfterMs = 300))
    }

    @Test
    fun `r18-3 a Vela account in front waits only for the store, and no account is Welcome after the ground`() {
        assertEquals(listOf(LaunchFront.Screen.Ground, LaunchFront.Screen.Wallet), launch(FakeStore(mapOf("vela.accounts" to "[$ann]")), hubAfterMs = 5_000))
        assertEquals(listOf(LaunchFront.Screen.Ground, LaunchFront.Screen.Welcome), launch(FakeStore()))
    }

    @Test
    fun `r18-3 the rule - Loading is ground, the hold is bounded, a stored value is read as the core wrote it`() {
        val ms = LaunchFront.Stored.Multisig(multisig)
        for (stored in listOf(LaunchFront.Stored.Unread, LaunchFront.Stored.Vela, ms)) {
            assertEquals(LaunchFront.Screen.Ground, LaunchFront.screen(SessionRoute.Loading, stored, hubLoaded = true, expired = true))
            assertEquals(LaunchFront.Screen.Welcome, LaunchFront.screen(SessionRoute.Onboarding, stored, hubLoaded = false, expired = false))
        }
        assertEquals(LaunchFront.Screen.Ground, LaunchFront.screen(SessionRoute.Wallet, ms, hubLoaded = false, expired = false))
        assertEquals("bounded", LaunchFront.Screen.Wallet, LaunchFront.screen(SessionRoute.Wallet, ms, hubLoaded = false, expired = true))
        assertEquals(LaunchFront.Screen.Ground, LaunchFront.screen(SessionRoute.Wallet, LaunchFront.Stored.Unread, hubLoaded = true, expired = false))
        assertEquals(LaunchFront.Screen.Ground, LaunchFront.screen(SessionRoute.Wallet, LaunchFront.Stored.Vela, hubLoaded = true, expired = false, walletSettled = false))
        assertEquals(ms, LaunchFront.stored(multisig))
        assertEquals(ms, LaunchFront.stored("\"$multisig\"\n"))
        assertEquals(LaunchFront.Stored.Vela, LaunchFront.stored(null))
        assertEquals(LaunchFront.Stored.Vela, LaunchFront.stored("  "))
        assertEquals(2_000L, LaunchFront.HOLD_MS)
    }

    // -- r18-16: the chip and the body change in the same frame --------------

    private val alice = "0xD400866e00B055B20752a826CD5C89b811de130b"
    private val bob = "0x031d7D57c99CAF891e1C250554691Fd12D84772b"
    private val safe = "0x7f3cE90000000000000000000000000000A0de87"
    private val hash = "0x" + "cd".repeat(32)
    private val tx = "0x" + "ab".repeat(32)

    private val msWords = app.getvela.wallet.feature.multisig.MultisigWords(
        strings = strings,
        accounts = app.getvela.wallet.feature.multisig.core.MsAccountsView(),
        localNames = mapOf(alice.lowercase() to "Interleave"),
        networkName = { if (it == 5042002) "Arc Testnet" else null },
        nativeSymbol = { "USDC" },
    )

    /** One transaction's sheet model, the proposals view at one moment of an execution from here. */
    private fun sheet(status: String, approvers: List<String>, extra: String): app.getvela.wallet.feature.multisig.MsProposalModel {
        val approvals = approvers.joinToString(",") { """{"owner":"$it"}""" }
        val view = app.getvela.wallet.feature.multisig.core.MsProposalsView.of(
            app.getvela.wallet.core.crux.Wire.json.parseToJsonElement(
                """
                {"followed":"$safe","standings":{},$extra
                 "items":[{"chainId":5042002,"safeTxHash":"$hash","nonce":"5","proposer":"$bob","approvals":[$approvals],"approved":${approvers.size},"threshold":2,
                  "status":"$status","canApprove":[],"canExecute":${if (status == "done") "[]" else "[\"$alice\"]"},"cancelledBy":null,"sameNonce":[],"executing":false,"createdMs":1,
                  "unsent":false,"renameUndeployed":false,"review":{"action":{"kind":"call","to":"$bob","value":"0","data":"0x"},"sign":{"allowed":true,"anyway":false}}}],
                 "chains":[{"chainId":5042002,"members":{"deployed":true,"owners":["$alice","$bob"],"threshold":2,"nonce":"5"},"items":[],"mailbox":true}]}
                """.trimIndent(),
            ),
        )
        return app.getvela.wallet.feature.multisig.MsProposalModel(view.items.single(), view, msWords, null)
    }

    private val executing get() = msWords.t("status.executing")
    private val executed get() = msWords.t("proposal.executed")

    /** What the chip says and what the body says, as the sheet draws them. */
    private fun agree(frame: String, m: app.getvela.wallet.feature.multisig.MsProposalModel) {
        val bodySaysExecuted = m.saysExecuted && m.executedTx != null ||
            m.sheetNotes.any { it.id == "executed" } ||
            m.approvalsCount == executed ||
            m.executedFacts != null
        if (m.statusLine == executing) {
            assertTrue("$frame: the chip says Executing… over an Executed body (${m.executedFacts}, ${m.approvalsCount})", !bodySaysExecuted)
            assertTrue("$frame: never 'isn't recorded' while it is being executed here", m.executedFacts == null)
            assertEquals("$frame: the members, never a stripped list", listOf(alice, bob), m.listedMembers)
        } else {
            assertTrue("$frame: the chip moved on (${m.statusLine}) but the body did not", bodySaysExecuted)
        }
    }

    @Test
    fun `r18-16 from the press to the receipt, the chip and the body say the same, and change together`() {
        val press = """"actions":{"execute:$hash":{"kind":"working"}},"""
        // 1. The press is out; the mailbox still counts 1 (the one press carries the last approval).
        agree("press", sheet("ready", listOf(bob), press))
        // 2. The mailbox says done and the timeline has the executed transaction, but this
        //    device's flight is still waiting on its receipt and the frozen facts are not in.
        val timeline = """"timelines":{"5042002:$hash":[{"kind":"executed","tx_hash":"$tx","who":"$alice"}]},"""
        val waiting = """"executions":{"5042002:$hash":{"kind":"waiting","txHash":"$tx"}},"""
        val mid = sheet("done", listOf(bob), press + timeline + waiting)
        assertEquals(executing, mid.statusLine)
        agree("mailbox done, receipt out", mid)
        // 2b. The press settled, the receipt still waiting.
        agree("press settled, receipt out", sheet("done", listOf(bob), """"actions":{"execute:$hash":{"kind":"done"}},""" + timeline + waiting))
        // 2c. The receipt judged executed while the press's action is still working: the verdict speaks.
        val judged = """"executions":{"5042002:$hash":{"kind":"executed","txHash":"$tx"}},"""
        val frozen = """"frozen":{"5042002:$hash":{"executor":"$alice","approvers":["$alice","$bob"],"threshold":2,"executorApproved":true}},"""
        val resolvedWorking = sheet("done", listOf(bob), press + timeline + judged + frozen)
        assertNotEquals(executing, resolvedWorking.statusLine)
        agree("judged, press still working", resolvedWorking)
        // 3. Resolved: Executed on the chip and in the body, in the same frame.
        val done = sheet("done", listOf(alice, bob), """"actions":{"execute:$hash":{"kind":"done"}},""" + timeline + judged + frozen)
        assertEquals(executed, done.statusLine)
        agree("executed", done)
    }

    companion object {
        const val ACCOUNT = "0x1111111111111111111111111111111111111111"
    }
}
