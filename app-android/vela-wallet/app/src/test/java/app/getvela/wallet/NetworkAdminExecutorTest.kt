package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.NetCustomNetwork
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetHealthBody
import app.getvela.wallet.feature.settings.core.NetNetworkConfig
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetShownNetworks
import app.getvela.wallet.feature.settings.core.NetServiceEndpoints
import app.getvela.wallet.feature.settings.core.NetShellResult
import app.getvela.wallet.feature.settings.core.NetworkAdminExecutor
import kotlinx.coroutines.runBlocking
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The `network_admin` shell contract
 * (`specs/040-android-live-shell/contracts/shell-operations.md`).
 *
 * Two things this file is really about:
 *
 * 1. **The stored bytes are a cross-client contract.** The camelCase keys
 *    asserted below are what the web and desktop wallets read. A field written
 *    as `rpc_url` instead of `rpcURL` gives this device a network its siblings
 *    cannot see, and nothing anywhere reports a fault.
 * 2. **Nine operations are deliberately fail-closed** until spec 041, and each
 *    must answer the shape the core's failure twin defines — never a guess. An
 *    empty search index is not "no such chain"; a null code is not "not a
 *    contract".
 */
class NetworkAdminExecutorTest {

    private fun executor(store: KeyValueStore = FakeStore()) = NetworkAdminExecutor(store)

    // -- read_store ----------------------------------------------------------

    @Test
    fun anEmptyDeviceLoadsNothingConfigured() = runBlocking {
        val loaded = executor().perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertTrue(loaded.custom_networks.isEmpty())
        assertTrue(loaded.network_configs.isEmpty())
        assertNull(loaded.endpoints.bundler_service_url)
        assertNull(loaded.provider_keys.alchemy)
    }

    @Test
    fun storedRecordsAreReadInTheOtherClientsShapes() = runBlocking {
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CUSTOM_NETWORKS to JSONArray().put(
                    JSONObject()
                        .put("id", "custom-7777")
                        .put("displayName", "Testnet")
                        .put("chainId", 7777)
                        .put("iconLabel", "T")
                        .put("iconColor", "#fff")
                        .put("iconBg", "#000")
                        .put("logoURL", "")
                        .put("isL2", true)
                        .put("rpcURL", "https://rpc.example")
                        .put("explorerURL", "https://scan.example")
                        .put("bundlerURL", "https://bundler.example")
                        .put("nativeSymbol", "TST")
                        .put("addedAt", "2026-09-05T00:00:00Z"),
                ).toString(),
                KeyValueStore.Keys.NETWORK_CONFIG to JSONArray().put(
                    JSONObject()
                        .put("chainId", 1)
                        .put("rpcURL", "https://eth.example")
                        .put("explorerURL", "")
                        .put("bundlerURL", ""),
                ).toString(),
                KeyValueStore.Keys.SERVICE_ENDPOINTS to
                    JSONObject().put("bundlerServiceURL", "https://b.example").toString(),
                KeyValueStore.Keys.RPC_PROVIDERS to
                    JSONObject().put("alchemy", "key-1").toString(),
            ),
        )

        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded

        assertEquals(1, loaded.custom_networks.size)
        assertEquals("Testnet", loaded.custom_networks[0].display_name)
        assertEquals(7777L, loaded.custom_networks[0].chain_id)
        assertTrue(loaded.custom_networks[0].is_l2)
        assertEquals("2026-09-05T00:00:00Z", loaded.custom_networks[0].added_at_iso)
        assertEquals(1L, loaded.network_configs[0].chain_id)
        assertEquals("https://b.example", loaded.endpoints.bundler_service_url)
        // Absent stays absent — the core applies the defaults, not the shell.
        assertNull(loaded.endpoints.fiat_rates_url)
        assertEquals("key-1", loaded.provider_keys.alchemy)
        assertNull(loaded.provider_keys.drpc)
    }

    @Test
    fun junkReadsAsEmptyRatherThanStrandingTheCore() = runBlocking {
        // The core would never leave `loaded` if `store_loaded` failed to
        // arrive, and every later write would be silently dropped. So garbage
        // coerces rather than throwing.
        val store = FakeStore(
            mapOf(
                KeyValueStore.Keys.CUSTOM_NETWORKS to "{not json at all",
                KeyValueStore.Keys.NETWORK_CONFIG to "\"a string, not an array\"",
                KeyValueStore.Keys.RPC_PROVIDERS to "[]",
            ),
        )
        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertTrue(loaded.custom_networks.isEmpty())
        assertTrue(loaded.network_configs.isEmpty())
        assertNull(loaded.provider_keys.ankr)
    }

    // -- the writes ----------------------------------------------------------

    @Test
    fun aCustomNetworkIsStoredInCamelCase() = runBlocking {
        val store = FakeStore()
        val network = NetCustomNetwork(
            id = "custom-42",
            display_name = "Forty Two",
            chain_id = 42,
            icon_label = "F",
            icon_color = "#111",
            icon_bg = "#222",
            logo_url = "https://logo.example",
            is_l2 = false,
            rpc_url = "https://rpc.example",
            explorer_url = "https://scan.example",
            bundler_url = "https://bundler.example",
            native_symbol = "FTT",
            added_at_iso = "2026-09-05T10:00:00Z",
        )

        val result = executor(store).perform(NetOperation.WriteCustomNetworks(listOf(network)))
        assertEquals(NetShellResult.Written, result)

        val stored = JSONArray(store.values[KeyValueStore.Keys.CUSTOM_NETWORKS]).getJSONObject(0)
        // The exact keys the sibling clients read.
        assertEquals("Forty Two", stored.getString("displayName"))
        assertEquals(42, stored.getInt("chainId"))
        assertEquals("https://rpc.example", stored.getString("rpcURL"))
        assertEquals("https://logo.example", stored.getString("logoURL"))
        assertEquals("2026-09-05T10:00:00Z", stored.getString("addedAt"))
        assertFalse(stored.getBoolean("isL2"))
    }

    @Test
    fun serviceEndpointsAreWrittenWhole() = runBlocking {
        val store = FakeStore()
        executor(store).perform(
            NetOperation.WriteServiceEndpoints(
                NetServiceEndpoints(
                    ethereum_data_url = "https://data.example",
                    passkey_index_url = "https://index.example",
                    bundler_service_url = "https://bundler.example",
                    fiat_rates_url = "https://rates.example",
                ),
            ),
        )
        val stored = JSONObject(store.values[KeyValueStore.Keys.SERVICE_ENDPOINTS])
        // The same key onboarding's endpoint sheet reads — one storage home.
        assertEquals("https://index.example", stored.getString("passkeyIndexURL"))
        assertEquals(4, stored.length())
    }

    // -- retire_provider_keys (the core's `retire_provider_keys`) -------------

    @Test
    fun retiringTheKeysWritesTheConfigsFirstAndOnlyThenRemovesTheKeys() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.RPC_PROVIDERS to """{"alchemy":"k"}"""))
        val answer = executor(store).perform(
            NetOperation.RetireProviderKeys(listOf(NetNetworkConfig(100, "https://gnosis.example/v2/k", "", "", only_mine = false))),
        )
        assertEquals(NetShellResult.ProviderKeysRetired(configs_written = true, keys_removed = true), answer)
        assertFalse("the keys are gone once their URLs are the networks' own", store.values.containsKey(KeyValueStore.Keys.RPC_PROVIDERS))
        val written = JSONArray(store.values[KeyValueStore.Keys.NETWORK_CONFIG]).getJSONObject(0)
        assertEquals("https://gnosis.example/v2/k", written.getString("rpcURL"))
    }

    @Test
    fun aRetirementWithNothingToMigrateStillRemovesTheKeys() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.RPC_PROVIDERS to """{"drpc":"k"}"""))
        assertEquals(
            NetShellResult.ProviderKeysRetired(configs_written = true, keys_removed = true),
            executor(store).perform(NetOperation.RetireProviderKeys(null)),
        )
        assertFalse(store.values.containsKey(KeyValueStore.Keys.NETWORK_CONFIG))
    }

    @Test
    fun aRetirementWhoseWriteFailedKeepsTheKeys() = runBlocking {
        // Not best effort: the answer is what really happened, and the keys
        // stay for the next load to try again.
        val store = FakeStore(mapOf(KeyValueStore.Keys.RPC_PROVIDERS to """{"alchemy":"k"}"""))
        store.refuseWrites = true
        assertEquals(
            NetShellResult.ProviderKeysRetired(configs_written = false, keys_removed = false),
            executor(store).perform(NetOperation.RetireProviderKeys(listOf(NetNetworkConfig(1, "u", "", "")))),
        )
        assertTrue(store.values.containsKey(KeyValueStore.Keys.RPC_PROVIDERS))
        assertEquals(
            NetShellResult.ProviderKeysRetired(configs_written = false, keys_removed = false),
            executor(store).neutralAnswer(NetOperation.RetireProviderKeys(null)),
        )
    }

    // -- "Only use my node" travels with the network's settings -------------

    @Test
    fun onlyMineIsReadAndWrittenAsTheOtherClientsSpellIt() = runBlocking {
        val store = FakeStore(
            mapOf(KeyValueStore.Keys.NETWORK_CONFIG to """[{"chainId":100,"rpcURL":"http://mine","explorerURL":"","bundlerURL":"","onlyMine":true}]"""),
        )
        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertTrue("read back, it must not be switched off", loaded.network_configs.single().only_mine)
        executor(store).perform(NetOperation.WriteNetworkConfigs(loaded.network_configs))
        assertTrue(JSONArray(store.values[KeyValueStore.Keys.NETWORK_CONFIG]).getJSONObject(0).getBoolean("onlyMine"))
        executor(store).perform(NetOperation.WriteNetworkConfigs(listOf(NetNetworkConfig(100, "http://mine", "", "", only_mine = false))))
        assertFalse("off is absent, as the web writes it", JSONArray(store.values[KeyValueStore.Keys.NETWORK_CONFIG]).getJSONObject(0).has("onlyMine"))
    }

    // -- shown networks (the core's `shown_networks`) -----------------------

    @Test
    fun aStoredShownRecordIsHandedOverAsItIs() = runBlocking {
        val store = FakeStore(mapOf(KeyValueStore.Keys.SHOWN_NETWORKS to """{"version":1,"shown":[988]}"""))
        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertEquals(NetShownNetworks(version = 1, shown = listOf(988)), loaded.shown_networks)
    }

    @Test
    fun aDeviceIsNotSettledBeforeABalanceRoundWasRead() = runBlocking {
        // Settling without the holdings could hide a network that holds the
        // person's money, and the core never re-decides a stored record.
        val store = FakeStore()
        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertNull("absent: every network is shown", loaded.shown_networks)
        assertFalse(store.values.containsKey(KeyValueStore.Keys.SHOWN_NETWORKS))
    }

    @Test
    fun aDeviceIsSettledOnceByTheCoreAndTheRecordStored() = runBlocking {
        // The core's own rule, through the real library: a network shown on
        // the person's word stays shown where they hold something.
        val optIn = uniffi.vela_core_uniffi.shownNetworksHidden("{\"version\":1,\"shown\":[]}").first().toLong()
        val store = FakeStore(mapOf(KeyValueStore.Keys.HELD_CHAIN_IDS to "[$optIn]"))
        val loaded = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertTrue("held there, so kept shown", optIn in loaded.shown_networks!!.shown)
        val stored = JSONObject(store.values[KeyValueStore.Keys.SHOWN_NETWORKS])
        assertEquals(optIn, stored.getJSONArray("shown").getLong(0))
        // Read again: the stored record stands, nothing is re-decided.
        store.values[KeyValueStore.Keys.HELD_CHAIN_IDS] = "[]"
        val again = executor(store).perform(NetOperation.ReadStore) as NetShellResult.StoreLoaded
        assertEquals(loaded.shown_networks, again.shown_networks)
    }

    @Test
    fun aShownRecordIsWrittenAsTheCoreSpellsIt() = runBlocking {
        val store = FakeStore()
        assertEquals(
            NetShellResult.Written,
            executor(store).perform(NetOperation.WriteShownNetworks(NetShownNetworks(version = 1, shown = listOf(988, 4326)))),
        )
        val stored = JSONObject(store.values[KeyValueStore.Keys.SHOWN_NETWORKS])
        assertEquals(1, stored.getInt("version"))
        assertEquals(listOf(988L, 4326L), (0 until 2).map { stored.getJSONArray("shown").getLong(it) })
    }

    @Test
    fun aRefusedWriteStillAnswers() = runBlocking {
        val store = FakeStore()
        store.refuseWrites = true
        assertEquals(
            NetShellResult.Written,
            executor(store).perform(NetOperation.WriteNetworkConfigs(listOf(NetNetworkConfig(1, "", "", "")))),
        )
        assertTrue(store.values.isEmpty())
    }

    // -- the fail-closed nine ------------------------------------------------

    @Test
    fun everyNetworkOperationAnswersItsUnknownShape() = runBlocking {
        val net = executor()

        assertEquals(
            NetShellResult.SearchIndex(emptyList()),
            net.perform(NetOperation.FetchSearchIndex),
        )
        assertEquals(
            NetShellResult.ChainInfo(8453, null),
            net.perform(NetOperation.FetchChainInfo(8453)),
        )
        assertEquals(
            NetShellResult.Probed("https://rpc", null, 0),
            net.perform(NetOperation.ProbeRpc("https://rpc")),
        )
        assertEquals(
            NetShellResult.Reachable("https://rpc", false, 0),
            net.perform(NetOperation.ProbeReachable("https://rpc")),
        )
        assertEquals(
            NetShellResult.Code("https://rpc", "0xabc", null),
            net.perform(NetOperation.RpcGetCode("https://rpc", "0xabc")),
        )
        assertEquals(
            NetShellResult.P256Call("https://rpc", null),
            net.perform(NetOperation.RpcCallP256("https://rpc")),
        )
        assertEquals(
            NetShellResult.ServiceHealth(NetEndpointField.FiatRates, NetHealthBody.Failed, 0),
            net.perform(
                NetOperation.FetchServiceHealth(NetEndpointField.FiatRates, "https://rates"),
            ),
        )
        assertEquals(
            NetShellResult.FiatRates(NetHealthBody.Failed, 0),
            net.perform(NetOperation.FetchFiatRates("https://rates")),
        )
        assertEquals(NetShellResult.Invalidated, net.perform(NetOperation.InvalidatePools(null)))
        assertEquals(
            NetShellResult.BundlerCacheCleared,
            net.perform(NetOperation.ClearBundlerCache(1)),
        )
    }

    @Test
    fun theDebounceIsARealWait() = runBlocking {
        // The core owns how long; the shell owns only the sleeping. A no-op
        // here would make every keystroke fetch, which is the behaviour the
        // debounce exists to prevent.
        val started = System.nanoTime()
        val result = executor().perform(NetOperation.StartSearchDebounce(40))
        val elapsedMs = (System.nanoTime() - started) / 1_000_000
        assertEquals(NetShellResult.DebounceElapsed, result)
        assertTrue("waited $elapsedMs ms, expected at least 30", elapsedMs >= 30)
    }

    @Test
    fun everyOperationHasANeutralAnswer() {
        // Exhaustiveness is the compiler's (a `when` over a sealed hierarchy
        // with no `else`); this pins the two that could plausibly be given the
        // wrong shape by hand.
        val net = executor()
        assertEquals(
            NetShellResult.StoreLoaded(),
            net.neutralAnswer(NetOperation.ReadStore),
        )
        assertEquals(
            NetShellResult.Probed("u", null, 0),
            net.neutralAnswer(NetOperation.ProbeRpc("u")),
        )
    }
}
