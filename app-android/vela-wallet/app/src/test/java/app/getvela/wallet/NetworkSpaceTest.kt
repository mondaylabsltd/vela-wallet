package app.getvela.wallet

import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.I18nRuntime
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.settings.SettingsFixtures
import app.getvela.wallet.feature.settings.SettingsLive
import app.getvela.wallet.feature.settings.SettingsScreenState
import app.getvela.wallet.feature.settings.core.NetEvent
import app.getvela.wallet.feature.settings.core.NetNetworkRow
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetView
import app.getvela.wallet.feature.settings.core.NetworkAdminExecutor
import app.getvela.wallet.feature.settings.core.NetworkSpace
import app.getvela.wallet.feature.settings.core.NetworkSpacePreference
import app.getvela.wallet.feature.settings.core.NetworkSpaces
import app.getvela.wallet.feature.wallet.AssetFiatModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletFixtures
import app.getvela.wallet.feature.wallet.WalletLive
import app.getvela.wallet.feature.wallet.WalletScreenState
import app.getvela.wallet.feature.wallet.core.BalanceExecutor
import app.getvela.wallet.feature.wallet.core.BalanceOperation
import app.getvela.wallet.feature.wallet.core.BalanceShellResult
import app.getvela.wallet.feature.wallet.core.BalanceToken
import app.getvela.wallet.feature.wallet.core.BalanceView
import app.getvela.wallet.feature.wallet.core.FeedEvent
import app.getvela.wallet.feature.wallet.core.FeedView
import app.getvela.wallet.feature.wallet.core.inSpace
import app.getvela.wallet.feature.settings.core.CurrencyView
import app.getvela.wallet.core.crux.Wire
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.json.JSONArray
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The mainnet space and the testnet space (spec 105), on the real Rust.
 *
 * What a shell can get wrong here without any other test failing: a list
 * that offers the other space's networks, a person's word on a network lost
 * on the way to the store (or a word they never gave written for them), a
 * testnet's coins counted as money, a saved total of one space standing in
 * for the other's.
 */
class NetworkSpaceTest {

    private val scopes = mutableListOf<CoroutineScope>()

    @After
    fun stopEveryMachine() {
        scopes.forEach { it.cancel() }
        scopes.clear()
    }

    private fun scope() = CoroutineScope(SupervisorJob() + Dispatchers.Default).also { scopes += it }

    private val strings: VelaStrings by lazy {
        val root = System.getProperty("vela.repo.root")
            ?: error("vela.repo.root not set — run via Gradle (testOptions wires it)")
        I18nRuntime { tag -> File(root, "assets/i18n/$tag.json").readBytes() }
            .apply { initialize("en") }
    }

    private fun host(store: KeyValueStore): CoreHost<NetView> {
        val executor = NetworkAdminExecutor(store)
        return CoreHost(
            bridge = uniffi.vela_core_uniffi.NetworkAdminCore().asBridge(),
            scope = scope(),
            initial = NetView(),
            serializer = NetView.serializer(),
            perform = JsonShell.perform(NetOperation.serializer(), NetShellResult.serializer(), executor::perform),
            escapedFailure = JsonShell.escapedFailure(
                NetOperation.serializer(),
                NetShellResult.serializer(),
                fallback = NetShellResult.StoreLoaded(),
                answer = executor::neutralAnswer,
            ),
            onFault = { error -> throw AssertionError("shell fault: $error", error) },
        )
    }

    private fun CoreHost<NetView>.settle(predicate: (NetView) -> Boolean): NetView =
        runBlocking { withTimeout(TIMEOUT_MS) { view.first(predicate) } }

    private fun custom(chainId: Int, name: String, isTestnet: Boolean?) = JSONObject()
        .put("id", "custom-$chainId").put("displayName", name).put("chainId", chainId)
        .put("iconLabel", name.take(1)).put("iconColor", "#111").put("iconBg", "#222").put("logoURL", "")
        .put("isL2", false).put("rpcURL", "https://rpc.example").put("explorerURL", "").put("bundlerURL", "")
        .put("nativeSymbol", "X").put("addedAt", "2026-09-01T00:00:00Z")
        .also { record -> isTestnet?.let { record.put("isTestnet", it) } }

    private fun loaded(store: FakeStore): Pair<CoreHost<NetView>, NetView> {
        val host = host(store)
        host.dispatch(NetEvent.Started, NetEvent.serializer())
        return host to host.settle { it.loaded && it.networks.isNotEmpty() }
    }

    // -- which networks a space holds -----------------------------------------

    @Test
    fun theBuiltInsSplitIntoTwentyFourMainnetsAndNineTestnets() {
        val (_, view) = loaded(FakeStore())
        assertEquals(33, view.networks.size)
        val mainnets = NetworkSpaces.chainIds(NetworkSpace.Mainnet, view.networks)
        val testnets = NetworkSpaces.chainIds(NetworkSpace.Testnet, view.networks)
        assertEquals(24, mainnets.size)
        assertEquals(
            setOf(11155111, 84532, 421614, 11155420, 80002, 97, 43113, 5042002, 42431),
            testnets.toSet(),
        )
        assertTrue("Ethereum is real value", 1 in mainnets)
        assertTrue("no chain is in both", mainnets.intersect(testnets.toSet()).isEmpty())
        // In the rows' order: the list a person sees keeps the core's order.
        assertEquals(view.networks.map { it.chain_id.toInt() }.filter { it in testnets }, testnets)
    }

    @Test
    fun aNetworkThePersonAddedIsInTheSpaceTheySaidAndUnsaidIsATestnet() {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CUSTOM_NETWORKS to JSONArray()
                    .put(custom(7777, "Old", isTestnet = null))
                    .put(custom(8888, "Real", isTestnet = false))
                    .toString(),
            ),
        )
        val (_, view) = loaded(store)
        assertTrue(view.networks.first { it.chain_id == 7777L }.is_testnet)
        assertFalse(view.networks.first { it.chain_id == 8888L }.is_testnet)
        assertTrue(8888 in NetworkSpaces.chainIds(NetworkSpace.Mainnet, view.networks))
        assertTrue(7777 in NetworkSpaces.chainIds(NetworkSpace.Testnet, view.networks))

        val mainnet = NetworkSpaces.view(NetworkSpace.Mainnet, view)
        assertTrue(mainnet.networks.none { it.is_testnet })
        assertEquals(view.endpoints, mainnet.endpoints)
    }

    @Test
    fun movingANetworkStoresThePersonsWordInTheSharedShape() {
        val store = FakeStore(
            mapOf(KeyValueStore.Keys.CUSTOM_NETWORKS to JSONArray().put(custom(7777, "Old", isTestnet = null)).toString()),
        )
        val (host, _) = loaded(store)
        host.dispatch(NetEvent.CustomSpaceChanged("custom-7777", testnet = false), NetEvent.serializer())
        host.settle { view -> view.networks.any { it.chain_id == 7777L && !it.is_testnet } }
        val stored = runBlocking {
            withTimeout(TIMEOUT_MS) {
                while (store.values[KeyValueStore.Keys.CUSTOM_NETWORKS]?.contains("isTestnet") != true) kotlinx.coroutines.delay(10)
                JSONArray(store.values[KeyValueStore.Keys.CUSTOM_NETWORKS]).getJSONObject(0)
            }
        }
        // camelCase, as the web and the desktop read it.
        assertEquals(false, stored.get("isTestnet"))
        assertFalse(stored.has("is_testnet"))
    }

    @Test
    fun aRecordWithoutTheWordIsWrittenBackWithoutIt() = runBlocking {
        val store = FakeStore(
            mapOf(KeyValueStore.Keys.CUSTOM_NETWORKS to JSONArray().put(custom(7777, "Old", isTestnet = null)).toString()),
        )
        val executor = NetworkAdminExecutor(store)
        val loaded = executor.perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertNull(loaded.custom_networks.single().is_testnet)
        executor.perform(NetOperation.WriteCustomNetworks(loaded.custom_networks))
        // Absent stays absent: nobody answered for the person.
        assertFalse(JSONArray(store.values[KeyValueStore.Keys.CUSTOM_NETWORKS]).getJSONObject(0).has("isTestnet"))
        Unit
    }

    @Test
    fun unreadableRowsAreNoChains() {
        assertTrue(NetworkSpaces.chainIds(NetworkSpace.Mainnet, emptyList()).isEmpty())
        val view = NetView(loaded = true)
        assertEquals(view, NetworkSpaces.view(NetworkSpace.Testnet, view))
    }

    // -- the preference ----------------------------------------------------------

    @Test
    fun aDeviceStartsInTheMainnetSpaceAndAnythingUnreadableIsMainnet() {
        for ((stored, expected) in listOf(null to NetworkSpace.Mainnet, "nonsense" to NetworkSpace.Mainnet, "testnet" to NetworkSpace.Testnet)) {
            val store = FakeStore(stored?.let { mapOf(NetworkSpacePreference.KEY to it) }.orEmpty())
            val preference = NetworkSpacePreference(store, scope()).also { it.load() }
            runBlocking { withTimeout(TIMEOUT_MS) { preference.ready() } }
            assertEquals("stored $stored", expected, preference.space.value)
        }
    }

    @Test
    fun theSwitchIsStoredInTheCoresSpelling() {
        val store = FakeStore()
        val preference = NetworkSpacePreference(store, scope()).also { it.load() }
        preference.set(NetworkSpace.Testnet)
        assertEquals(NetworkSpace.Testnet, preference.space.value)
        runBlocking {
            withTimeout(TIMEOUT_MS) { while (store.values[NetworkSpacePreference.KEY] != "testnet") kotlinx.coroutines.delay(10) }
        }
    }

    @Test
    fun clearingCachesKeepsTheSpace() {
        // The core's storage catalog: a user setting, never a cache.
        assertFalse(uniffi.vela_core_uniffi.storageIsCacheKey(NetworkSpacePreference.KEY))
    }

    // -- money --------------------------------------------------------------------

    private fun token(chainId: Int, symbol: String, price: Double?) = BalanceToken(
        chain_id = chainId, symbol = symbol, name = symbol, balance = "2", decimals = 18, price_usd = price,
    )

    @Test
    fun theOtherSpacesHoldingsNeverStandInForThese() {
        val view = BalanceView(
            tokens = listOf(token(1, "ETH", 3000.0), token(11155111, "ETH", 3000.0)),
            display_total_usd = 12000.0,
        )
        val testnet = inSpace(view, setOf(11155111))
        assertEquals(listOf(11155111), testnet.tokens.map { it.chain_id })
        assertNull("a total that counted the other space is withheld", testnet.display_total_usd)
        // Nothing to narrow: the view as it was.
        assertEquals(view, inSpace(view, setOf(1, 11155111)))
        assertEquals(view, inSpace(view, null))
    }

    @Test
    fun theTestnetSpaceReadsAndWritesNoSavedTotal() = runBlocking {
        val address = "0x00000000000000000000000000000000000000aa"
        val cache = JSONObject().put(address, JSONObject().put("usd", 1383.28).put("at", System.currentTimeMillis())).toString()
        val store = FakeStore(mapOf(KeyValueStore.Keys.BALANCE_CACHE to cache))
        var testnet = true
        val executor = BalanceExecutor(
            // Never asked: a cache read reaches no chain.
            pool = app.getvela.wallet.feature.wallet.core.RpcPool(
                FakeStore(),
                object : app.getvela.wallet.feature.wallet.core.RpcEndpointSource {
                    override suspend fun forChain(chainId: Int) = app.getvela.wallet.feature.wallet.core.RpcSeeds()
                },
                scope(),
                FakeRpcTransport { _, _, _ -> error("no chain is read here") },
            ),
            networks = MutableStateFlow(NetView()),
            store = store,
            testnet = { testnet },
        )
        val read = executor.perform(BalanceOperation.ReadBalanceCache(address)) as BalanceShellResult.CachedTotalLoaded
        assertNull(read.usd)
        executor.perform(BalanceOperation.WriteBalanceCache(address, 5.0))
        assertEquals("a testnet total is never saved", cache, store.values[KeyValueStore.Keys.BALANCE_CACHE])
        testnet = false
        val mainnet = executor.perform(BalanceOperation.ReadBalanceCache(address)) as BalanceShellResult.CachedTotalLoaded
        assertEquals(1383.28, mainnet.usd ?: 0.0, 0.001)
    }

    @Test
    fun theTestnetHomeShowsNoFigureAndNoWorth() {
        val view = BalanceView(
            address = "0xabc",
            tokens = listOf(token(11155111, "ETH", 3000.0)),
            display_total_usd = 6000.0,
            last_refreshed_at_ms = 1.0,
        )
        val words = strings.t(I18nKeys.NetworkSpace.NO_VALUE)
        val home = WalletLive.home(
            WalletFixtures.buildMobileState(WalletScreenState.H1, strings), view, FeedView(),
            CurrencyView(code = "USD", committed = true), strings, mapOf(11155111 to "Sepolia"), noValue = words,
        )
        assertEquals(BalanceStateKind.NoValue, home.balance.state)
        assertEquals(words, home.balance.noValueText)
        assertNull("no figure", home.balance.integer)
        assertEquals(AssetFiatModel.None, home.assetRows.single().fiat)

        val mainnet = WalletLive.home(
            WalletFixtures.buildMobileState(WalletScreenState.H1, strings), view, FeedView(),
            CurrencyView(code = "USD", committed = true), strings, mapOf(11155111 to "Sepolia"),
        )
        assertEquals(BalanceStateKind.Normal, mainnet.balance.state)
    }

    // -- the settings list ---------------------------------------------------------

    @Test
    fun theNetworkListIsTheSpaceInFrontsAndAnAddedOneMoves() {
        val view = NetView(
            loaded = true,
            networks = listOf(
                NetNetworkRow(id = "ethereum", chain_id = 1, display_name = "Ethereum", native_symbol = "ETH"),
                NetNetworkRow(id = "sepolia", chain_id = 11155111, display_name = "Sepolia", native_symbol = "ETH", is_testnet = true),
                NetNetworkRow(id = "custom-7777", chain_id = 7777, display_name = "Mine", native_symbol = "X", is_custom = true, is_testnet = true),
            ),
        )
        val base = SettingsFixtures.buildState(SettingsScreenState.ST1, strings)
        val testnet = SettingsLive.withNetworks(base, view, strings, space = NetworkSpace.Testnet)
        assertEquals(listOf("Sepolia", "Mine"), testnet.networks.map { it.name })
        assertEquals(strings.t(I18nKeys.NetworkSpace.MOVE_TO_MAINNET), testnet.networks.last().move)
        assertNull("a built-in does not move", testnet.networks.first().move)
        assertEquals(strings.t(I18nKeys.NetworkSpace.HEADING_TESTNET), testnet.networksSpace?.heading)
        assertEquals(strings.t(I18nKeys.NetworkSpace.TO_MAINNET), testnet.networksSpace?.switchLabel)

        val mainnet = SettingsLive.withNetworks(base, view, strings, space = NetworkSpace.Mainnet)
        assertEquals(listOf("Ethereum"), mainnet.networks.map { it.name })
        // The gallery: every network, no heading.
        assertNull(SettingsLive.withNetworks(base, view, strings).networksSpace)
    }

    @Test
    fun theFeedIsToldTheSpacesChainsInTheCoresShape() {
        val json = Wire.json.encodeToString(FeedEvent.serializer(), FeedEvent.SpaceChainsChanged(listOf(1, 10)))
        assertEquals("""{"type":"space_chains_changed","chain_ids":[1,10]}""", json)
        // `null` = every chain: the field is absent, which the core reads as none.
        assertEquals("""{"type":"space_chains_changed"}""", Wire.json.encodeToString(FeedEvent.serializer(), FeedEvent.SpaceChainsChanged(null)))
    }

    @Test
    fun everySpaceWordResolves() {
        listOf(
            I18nKeys.NetworkSpace.MAINNET, I18nKeys.NetworkSpace.TESTNET, I18nKeys.NetworkSpace.BAR,
            I18nKeys.NetworkSpace.TITLE, I18nKeys.NetworkSpace.MAINNET_HINT, I18nKeys.NetworkSpace.TESTNET_HINT,
            I18nKeys.NetworkSpace.TO_TESTNET, I18nKeys.NetworkSpace.TO_MAINNET, I18nKeys.NetworkSpace.NO_VALUE,
            I18nKeys.NetworkSpace.MOVE_TO_MAINNET, I18nKeys.NetworkSpace.MOVE_TO_TESTNET,
            I18nKeys.NetworkSpace.WIZARD_QUESTION, I18nKeys.NetworkSpace.WIZARD_TESTNET, I18nKeys.NetworkSpace.WIZARD_MAINNET,
            I18nKeys.NetworkSpace.HEADING_MAINNET, I18nKeys.NetworkSpace.HEADING_TESTNET,
            "settings.agent.action.setNetworkSpace",
        ).forEach { key -> assertTrue(key, strings.t(key) != key) }
    }

    private companion object {
        const val TIMEOUT_MS = 10_000L
    }
}
