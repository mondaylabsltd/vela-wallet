package app.getvela.wallet

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.feature.settings.core.NetCustomNetwork
import app.getvela.wallet.feature.settings.core.NetEndpointField
import app.getvela.wallet.feature.settings.core.NetHealthBody
import app.getvela.wallet.feature.settings.core.NetNetworkConfig
import app.getvela.wallet.feature.settings.core.NetOperation
import app.getvela.wallet.feature.settings.core.NetProviderKeys
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

    @Test
    fun aClearedProviderKeyIsRemovedNotEmptied() = runBlocking {
        // The core's invariant ⑦. "" and absent are different: one falls back
        // to the public endpoint, the other fails every request with a key that
        // is not a key.
        val store = FakeStore()
        executor(store).perform(
            NetOperation.WriteRpcProviders(NetProviderKeys(alchemy = "abc", drpc = null)),
        )
        val stored = JSONObject(store.values[KeyValueStore.Keys.RPC_PROVIDERS])
        assertEquals("abc", stored.getString("alchemy"))
        assertFalse("a null key must not be stored at all", stored.has("drpc"))
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
