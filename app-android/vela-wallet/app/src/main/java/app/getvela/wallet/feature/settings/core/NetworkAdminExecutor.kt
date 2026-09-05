package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `network_admin` core touches the outside world.
 *
 * Sixteen operations: **six storage, one timer, nine network**. Spec 040 has
 * no network layer, so the nine answer with the same "unknown" shapes the web
 * contract defined, each marked `// live in 041` — grep for that marker to
 * find spec 041's inbox, and watch the count go to zero as it lands.
 *
 * Port source:
 * `app-web/vela-wallet/src/lib/settings/core/network-admin-executor.ts`.
 *
 * **The stored shapes are camelCase and the wire shapes are snake_case**, and
 * the codecs below are the whole translation. That is not cosmetic: those bytes
 * are what the web and desktop clients read, so a field written as `rpc_url`
 * instead of `rpcURL` is a network this wallet can see and its siblings cannot
 * — with nothing anywhere reporting a fault.
 *
 * Coercion is hygiene, never policy. A junk record reads as its empty value
 * rather than being dropped, for the reason the web port records: serde
 * rejecting a malformed `store_loaded` would strand the core *unloaded*
 * forever, and every later write would be silently discarded.
 */
class NetworkAdminExecutor(private val store: KeyValueStore) {

    @Suppress("LongMethod")
    suspend fun perform(operation: NetOperation): NetShellResult = when (operation) {

        // -- storage: live -----------------------------------------------------

        is NetOperation.ReadStore -> NetShellResult.StoreLoaded(
            custom_networks = decodeCustomNetworks(store.read(KeyValueStore.Keys.CUSTOM_NETWORKS)),
            network_configs = decodeNetworkConfigs(store.read(KeyValueStore.Keys.NETWORK_CONFIG)),
            endpoints = decodeStoredEndpoints(store.read(KeyValueStore.Keys.SERVICE_ENDPOINTS)),
            provider_keys = decodeProviderKeys(store.read(KeyValueStore.Keys.RPC_PROVIDERS)),
        )

        is NetOperation.WriteCustomNetworks -> {
            store.write(
                KeyValueStore.Keys.CUSTOM_NETWORKS,
                JSONArray().apply {
                    operation.networks.forEach { put(encodeCustomNetwork(it)) }
                }.toString(),
            )
            NetShellResult.Written
        }

        is NetOperation.WriteNetworkConfigs -> {
            store.write(
                KeyValueStore.Keys.NETWORK_CONFIG,
                JSONArray().apply {
                    operation.configs.forEach { put(encodeNetworkConfig(it)) }
                }.toString(),
            )
            NetShellResult.Written
        }

        // The core sends the COMPLETE merged record — four strings, written
        // verbatim. Merging here instead would mean two places deciding what an
        // unset endpoint is, and the onboarding sheet reads this same key.
        is NetOperation.WriteServiceEndpoints -> {
            store.write(
                KeyValueStore.Keys.SERVICE_ENDPOINTS,
                JSONObject().apply {
                    put("ethereumDataURL", operation.endpoints.ethereum_data_url)
                    put("passkeyIndexURL", operation.endpoints.passkey_index_url)
                    put("bundlerServiceURL", operation.endpoints.bundler_service_url)
                    put("fiatRatesURL", operation.endpoints.fiat_rates_url)
                }.toString(),
            )
            NetShellResult.Written
        }

        // A cleared key is REMOVED, not stored as "" — the core's invariant ⑦,
        // and the difference between "no key configured" and "a key that is the
        // empty string" is the difference between falling back to the public
        // endpoint and failing every request.
        is NetOperation.WriteRpcProviders -> {
            store.write(
                KeyValueStore.Keys.RPC_PROVIDERS,
                JSONObject().apply {
                    operation.keys.alchemy?.let { put("alchemy", it) }
                    operation.keys.drpc?.let { put("drpc", it) }
                    operation.keys.ankr?.let { put("ankr", it) }
                }.toString(),
            )
            NetShellResult.Written
        }

        // -- the one timer: live ----------------------------------------------

        // A real wait. The core owns how long (it sends the ms); the shell owns
        // only the sleeping. Cancellation is the driver's — a superseded search
        // has its job cancelled and this answer is never delivered.
        is NetOperation.StartSearchDebounce -> {
            delay(operation.ms)
            NetShellResult.DebounceElapsed
        }

        // -- network: fail-closed until spec 041 -------------------------------
        //
        // Every arm answers the shape the core's failure twin defines. None of
        // them guesses: an empty index is not "no such chain", an unreachable
        // probe is not "incompatible", and a null code is not "not a contract".
        // The core decides what each absence means.

        // live in 041
        is NetOperation.FetchSearchIndex -> NetShellResult.SearchIndex(chains = emptyList())

        // live in 041
        is NetOperation.FetchChainInfo ->
            NetShellResult.ChainInfo(chain_id = operation.chain_id, data = null)

        // live in 041
        is NetOperation.ProbeRpc -> NetShellResult.Probed(
            url = operation.url,
            reported_chain_id = null,
            latency_ms = 0,
        )

        // live in 041
        is NetOperation.ProbeReachable ->
            NetShellResult.Reachable(url = operation.url, ok = false, latency_ms = 0)

        // live in 041
        is NetOperation.RpcGetCode ->
            NetShellResult.Code(url = operation.url, address = operation.address, code = null)

        // live in 041
        is NetOperation.RpcCallP256 ->
            NetShellResult.P256Call(url = operation.url, result = null)

        // live in 041
        is NetOperation.FetchServiceHealth -> NetShellResult.ServiceHealth(
            field = operation.field,
            body = NetHealthBody.Failed,
            latency_ms = 0,
        )

        // live in 041
        is NetOperation.FetchFiatRates ->
            NetShellResult.FiatRates(body = NetHealthBody.Failed, latency_ms = 0)

        // -- acknowledged no-ops ----------------------------------------------
        //
        // Not fail-closed: there is genuinely nothing to invalidate. Android
        // gets an RPC pool and a bundler cache in spec 041, and these arms
        // become real then. // live in 041
        is NetOperation.InvalidatePools -> NetShellResult.Invalidated
        is NetOperation.ClearBundlerCache -> NetShellResult.BundlerCacheCleared
    }

    /**
     * What to answer when the shell itself threw.
     *
     * Exhaustive over the sealed hierarchy, so an operation added in Rust and
     * transcribed into [NetOperation] cannot be forgotten here.
     */
    @Suppress("CyclomaticComplexMethod")
    fun neutralAnswer(operation: NetOperation): NetShellResult = when (operation) {
        is NetOperation.ReadStore -> NetShellResult.StoreLoaded()
        is NetOperation.WriteCustomNetworks,
        is NetOperation.WriteNetworkConfigs,
        is NetOperation.WriteServiceEndpoints,
        is NetOperation.WriteRpcProviders,
        -> NetShellResult.Written
        is NetOperation.StartSearchDebounce -> NetShellResult.DebounceElapsed
        is NetOperation.FetchSearchIndex -> NetShellResult.SearchIndex()
        is NetOperation.FetchChainInfo -> NetShellResult.ChainInfo(operation.chain_id, null)
        is NetOperation.ProbeRpc -> NetShellResult.Probed(operation.url, null, 0)
        is NetOperation.ProbeReachable -> NetShellResult.Reachable(operation.url, false, 0)
        is NetOperation.RpcGetCode -> NetShellResult.Code(operation.url, operation.address, null)
        is NetOperation.RpcCallP256 -> NetShellResult.P256Call(operation.url, null)
        is NetOperation.FetchServiceHealth ->
            NetShellResult.ServiceHealth(operation.field, NetHealthBody.Failed, 0)
        is NetOperation.FetchFiatRates -> NetShellResult.FiatRates(NetHealthBody.Failed, 0)
        is NetOperation.InvalidatePools -> NetShellResult.Invalidated
        is NetOperation.ClearBundlerCache -> NetShellResult.BundlerCacheCleared
    }

    // -- codecs: wire (snake_case) ↔ stored (camelCase) ----------------------

    private fun decodeCustomNetworks(raw: String?): List<NetCustomNetwork> =
        objects(raw).map { record ->
            NetCustomNetwork(
                id = record.string("id"),
                display_name = record.string("displayName"),
                chain_id = record.optLong("chainId", 0),
                icon_label = record.string("iconLabel"),
                icon_color = record.string("iconColor"),
                icon_bg = record.string("iconBg"),
                logo_url = record.string("logoURL"),
                is_l2 = record.optBoolean("isL2", false),
                rpc_url = record.string("rpcURL"),
                explorer_url = record.string("explorerURL"),
                bundler_url = record.string("bundlerURL"),
                native_symbol = record.string("nativeSymbol"),
                added_at_iso = record.string("addedAt"),
            )
        }

    private fun encodeCustomNetwork(network: NetCustomNetwork): JSONObject = JSONObject().apply {
        put("id", network.id)
        put("displayName", network.display_name)
        put("chainId", network.chain_id)
        put("iconLabel", network.icon_label)
        put("iconColor", network.icon_color)
        put("iconBg", network.icon_bg)
        put("logoURL", network.logo_url)
        put("isL2", network.is_l2)
        put("rpcURL", network.rpc_url)
        put("explorerURL", network.explorer_url)
        put("bundlerURL", network.bundler_url)
        put("nativeSymbol", network.native_symbol)
        put("addedAt", network.added_at_iso)
    }

    private fun decodeNetworkConfigs(raw: String?): List<NetNetworkConfig> =
        objects(raw).map { record ->
            NetNetworkConfig(
                chain_id = record.optLong("chainId", 0),
                rpc_url = record.string("rpcURL"),
                explorer_url = record.string("explorerURL"),
                bundler_url = record.string("bundlerURL"),
            )
        }

    private fun encodeNetworkConfig(config: NetNetworkConfig): JSONObject = JSONObject().apply {
        put("chainId", config.chain_id)
        put("rpcURL", config.rpc_url)
        put("explorerURL", config.explorer_url)
        put("bundlerURL", config.bundler_url)
    }

    /** Absent fields stay absent: the core applies the defaults, not this. */
    private fun decodeStoredEndpoints(raw: String?): NetStoredEndpoints {
        val record = obj(raw) ?: return NetStoredEndpoints()
        return NetStoredEndpoints(
            ethereum_data_url = record.optionalString("ethereumDataURL"),
            passkey_index_url = record.optionalString("passkeyIndexURL"),
            bundler_service_url = record.optionalString("bundlerServiceURL"),
            fiat_rates_url = record.optionalString("fiatRatesURL"),
        )
    }

    private fun decodeProviderKeys(raw: String?): NetProviderKeys {
        val record = obj(raw) ?: return NetProviderKeys()
        return NetProviderKeys(
            alchemy = record.optionalString("alchemy"),
            drpc = record.optionalString("drpc"),
            ankr = record.optionalString("ankr"),
        )
    }

    // -- json hygiene --------------------------------------------------------

    private fun obj(raw: String?): JSONObject? =
        raw?.let { runCatching { JSONObject(it) }.getOrNull() }

    /** Absent, unparseable or not-an-array all read as empty. */
    private fun objects(raw: String?): List<JSONObject> {
        val array = raw?.let { runCatching { JSONArray(it) }.getOrNull() } ?: return emptyList()
        return (0 until array.length()).mapNotNull { array.optJSONObject(it) }
    }

    /** `optString` answers the four-character string "null" for a JSON null. */
    private fun JSONObject.string(key: String): String =
        if (isNull(key)) "" else optString(key)

    private fun JSONObject.optionalString(key: String): String? =
        if (isNull(key)) null else optString(key).takeIf { it.isNotEmpty() }
}
