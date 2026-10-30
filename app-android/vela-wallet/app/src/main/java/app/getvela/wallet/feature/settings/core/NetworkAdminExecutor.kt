package app.getvela.wallet.feature.settings.core

import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.delay
import org.json.JSONArray
import org.json.JSONObject

/**
 * The only place the `network_admin` core touches the outside world.
 *
 * Sixteen operations: **six storage, one timer, nine network**. The nine were
 * fail-closed for the whole of spec 040, which had no network layer; they are
 * live now, and the one that is still a no-op says why in place.
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
class NetworkAdminExecutor(
    private val store: KeyValueStore,
    /**
     * The network checks. `null` means this executor has no way to reach a
     * network — the settings machines boot before the pool exists — and every
     * probe then answers the same "nothing observed" shape it answered for the
     * whole of spec 040. Fail-closed, and the core reads it as unknown rather
     * than as broken.
     */
    private val probes: NetworkProbes? = null,
    /** Forget an endpoint's history in the pool; `null` = every chain. */
    private val invalidatePools: (Long?) -> Unit = {},
    /** Spec 043: forget what the relay client learned (`RelayClient.clearCaches`). */
    private val clearBundlerCache: () -> Unit = {},
    /**
     * Spec 100: a page's add-network request is over — carry the ending to the
     * browser machine (`add_network_answered`). Read when it happens, so the
     * container can wire it after this executor exists.
     */
    private val dappAddSettled: () -> ((tab: String, id: String, outcome: app.getvela.wallet.feature.browser.core.DappAddOutcome) -> Unit)? = { null },
    /**
     * The core's one-time settling of `vela.shownNetworks`
     * (`shown_networks_settle`): the stored bytes (`""`: none) and the
     * evidence JSON, answered as a `NetShownSettled`. A seam for tests.
     */
    private val settleShown: (stored: String, evidenceJson: String) -> String = { stored, evidence ->
        uniffi.vela_core_uniffi.shownNetworksSettle(stored, evidence)
    },
) {

    @Suppress("LongMethod")
    suspend fun perform(operation: NetOperation): NetShellResult = when (operation) {

        // -- storage: live -----------------------------------------------------

        is NetOperation.ReadStore -> {
            val custom = decodeCustomNetworks(store.read(KeyValueStore.Keys.CUSTOM_NETWORKS))
            val configs = decodeNetworkConfigs(store.read(KeyValueStore.Keys.NETWORK_CONFIG))
            NetShellResult.StoreLoaded(
                custom_networks = custom,
                network_configs = configs,
                endpoints = decodeStoredEndpoints(store.read(KeyValueStore.Keys.SERVICE_ENDPOINTS)),
                provider_keys = decodeProviderKeys(store.read(KeyValueStore.Keys.RPC_PROVIDERS)),
                // The device's shown Vela networks, settled (once, by the
                // core) before the ledger reads them: never a list drawn
                // before it. Absent: every network is shown.
                shown_networks = shownNetworks(custom, configs),
            )
        }

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

        // The provider keys are retired (the core's `retire_provider_keys`):
        // the networks they served get that URL as their own RPC, and the keys
        // go ONLY once that write is on disk. Not best effort — what really
        // happened is the answer, so a failure keeps the keys and the next
        // load computes the same again. Nothing here logs: the keys ride
        // inside those URLs.
        is NetOperation.RetireProviderKeys -> {
            val configsWritten = operation.configs?.let { configs ->
                store.write(
                    KeyValueStore.Keys.NETWORK_CONFIG,
                    JSONArray().apply { configs.forEach { put(encodeNetworkConfig(it)) } }.toString(),
                )
            } ?: true
            val keysRemoved = configsWritten && store.remove(KeyValueStore.Keys.RPC_PROVIDERS)
            NetShellResult.ProviderKeysRetired(configs_written = configsWritten, keys_removed = keysRemoved)
        }

        // A Vela network shown again or taken out of the lists. The lists the
        // person sees follow the network machine's rows, which the core has
        // already redrawn; this is the record a relaunch reads.
        is NetOperation.WriteShownNetworks -> {
            store.write(
                KeyValueStore.Keys.SHOWN_NETWORKS,
                app.getvela.wallet.core.crux.Wire.json.encodeToString(NetShownNetworks.serializer(), operation.record),
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

        // -- network -----------------------------------------------------------
        //
        // Every arm reports what it observed and nothing more. None of them
        // guesses: an empty index is not "no such chain", an unreachable probe
        // is not "incompatible", and a null code is not "not a contract". The
        // core decides what each absence means — and with no probes wired at
        // all, every one of these answers the same shape it did in 040.

        is NetOperation.FetchSearchIndex ->
            NetShellResult.SearchIndex(chains = probes?.searchIndex().orEmpty())

        is NetOperation.FetchChainInfo -> NetShellResult.ChainInfo(
            chain_id = operation.chain_id,
            data = probes?.chainInfo(operation.chain_id),
        )

        is NetOperation.ProbeRpc -> {
            val probe = probes?.probeRpc(operation.url)
            NetShellResult.Probed(
                url = operation.url,
                // What the endpoint SAYS it is. Whether that matches the chain
                // it was configured for is the core's verdict, not a check made
                // here.
                reported_chain_id = probe?.reportedChainId,
                latency_ms = probe?.latencyMs?.toLong() ?: 0,
            )
        }

        is NetOperation.ProbeReachable -> {
            val reach = probes?.probeReachable(operation.url)
            NetShellResult.Reachable(
                url = operation.url,
                ok = reach?.ok ?: false,
                latency_ms = reach?.latencyMs?.toLong() ?: 0,
            )
        }

        is NetOperation.RpcGetCode -> {
            val code = probes?.getCode(operation.url, operation.address)
            // The compatibility check is several of these in a row, and when it
            // stalls the screen shows a candidate with no verdict and no
            // button. From outside, "still checking" and "asked nobody" look
            // identical — the same blind spot the balance and scan executors
            // each had once.
            VelaLog.event(
                "net.getCode",
                "read",
                "url" to operation.url,
                "at" to operation.address,
                "code" to (code?.length ?: -1),
            )
            NetShellResult.Code(operation.url, operation.address, code)
        }

        is NetOperation.RpcCallP256 -> {
            val result = probes?.callP256(operation.url)
            VelaLog.event("net.p256", "called", "url" to operation.url, "got" to (result ?: "-"))
            NetShellResult.P256Call(url = operation.url, result = result)
        }

        is NetOperation.FetchServiceHealth -> {
            val health = probes?.serviceHealth(operation.base_url)
            NetShellResult.ServiceHealth(
                field = operation.field,
                body = healthBody(health),
                latency_ms = health?.latencyMs?.toLong() ?: 0,
            )
        }

        is NetOperation.FetchFiatRates -> {
            val health = probes?.fiatRates(operation.url)
            NetShellResult.FiatRates(
                body = when {
                    health == null -> NetHealthBody.Failed
                    health.httpStatus != null -> NetHealthBody.HttpError(health.httpStatus.toLong())
                    // Zero rates IS a body — an endpoint that answered with an
                    // empty table. Whether that counts as healthy is the core's.
                    else -> NetHealthBody.Rates((health.rateCount ?: 0).toLong())
                },
                latency_ms = health?.latencyMs?.toLong() ?: 0,
            )
        }

        // Forget what the pool learned about these endpoints, so a person who
        // has just fixed a URL is not still routed around it.
        is NetOperation.InvalidatePools -> {
            invalidatePools(operation.chain_id)
            NetShellResult.Invalidated
        }

        // The relay client's memory — deployment, nonces, resolved base URLs —
        // is what a person clearing caches expects to go with the pool's.
        is NetOperation.ClearBundlerCache -> {
            clearBundlerCache()
            NetShellResult.BundlerCacheCleared
        }

        is NetOperation.DappAddSettled -> {
            dappAddSettled()?.invoke(operation.tab, operation.id, operation.outcome)
            NetShellResult.Written
        }
    }

    /**
     * A health response in the core's vocabulary.
     *
     * Four shapes, and the distinction between them is the whole point: nothing
     * answered, an HTTP status came back, or the service identified itself.
     * Collapsing "404" into "failed" would tell somebody their endpoint is
     * unreachable when it is answering perfectly and just does not have that
     * path.
     */
    private fun healthBody(health: NetworkProbes.Health?): NetHealthBody = when {
        health == null -> NetHealthBody.Failed
        health.httpStatus != null -> NetHealthBody.HttpError(health.httpStatus.toLong())
        health.service == null && health.status == null -> NetHealthBody.Failed
        else -> NetHealthBody.Identity(service = health.service, status = health.status)
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
        is NetOperation.WriteShownNetworks,
        -> NetShellResult.Written
        // Nothing is known to have been written: the keys stay, the next load retries.
        is NetOperation.RetireProviderKeys -> NetShellResult.ProviderKeysRetired(configs_written = false, keys_removed = false)
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
        is NetOperation.DappAddSettled -> NetShellResult.Written
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
                // Spec 105: absent on a record from before the question — the
                // core reads that as a testnet; a flag that is not a boolean
                // is read as absent rather than guessed at.
                is_testnet = record.opt("isTestnet") as? Boolean,
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
        // Only when the person said: an absent flag must stay absent (spec 105).
        network.is_testnet?.let { put("isTestnet", it) }
    }

    private fun decodeNetworkConfigs(raw: String?): List<NetNetworkConfig> =
        objects(raw).map { record ->
            NetNetworkConfig(
                chain_id = record.optLong("chainId", 0),
                rpc_url = record.string("rpcURL"),
                explorer_url = record.string("explorerURL"),
                bundler_url = record.string("bundlerURL"),
                // "Only use my node" (2026-10-28): `onlyMine`, beside the
                // network's settings — the web's and desktop's bytes.
                only_mine = record.optBoolean("onlyMine", false),
            )
        }

    private fun encodeNetworkConfig(config: NetNetworkConfig): JSONObject = JSONObject().apply {
        put("chainId", config.chain_id)
        put("rpcURL", config.rpc_url)
        put("explorerURL", config.explorer_url)
        put("bundlerURL", config.bundler_url)
        // Only when on, as the web writes it: an absent flag reads as off.
        if (config.only_mine) put("onlyMine", true)
    }

    /**
     * The device's `vela.shownNetworks`: the stored record, else — ONCE — the
     * one the core settles from what this device holds (`shown_networks::settle`),
     * stored before it is used. The evidence is read as it is; what counts is
     * the core's.
     *
     * Settled only once a balance round has been read
     * ([KeyValueStore.Keys.HELD_CHAIN_IDS] present): a settling that could not
     * see the person's holdings might hide a network that holds their money,
     * and the core never re-decides a stored record. Until then nothing is
     * hidden (absent = every network shown), and the next load settles.
     */
    private suspend fun shownNetworks(custom: List<NetCustomNetwork>, configs: List<NetNetworkConfig>): NetShownNetworks? {
        val wire = app.getvela.wallet.core.crux.Wire.json
        val stored = runCatching { store.read(KeyValueStore.Keys.SHOWN_NETWORKS) }.getOrNull()
        if (!stored.isNullOrBlank()) {
            return runCatching { wire.decodeFromString(NetShownNetworks.serializer(), stored) }.getOrNull()
        }
        val held = runCatching { store.read(KeyValueStore.Keys.HELD_CHAIN_IDS) }.getOrNull()
            ?.let { raw -> runCatching { JSONArray(raw) }.getOrNull() }
            ?: return null
        val evidence = NetUseEvidence(
            balance_chain_ids = held.longs(),
            activity_chain_ids = chainIdsOf(store.read(KeyValueStore.Keys.TRANSACTIONS)),
            multisig_chain_ids = emptyList(),
            chosen_chain_ids = chainIdsOf(store.read(KeyValueStore.Keys.CUSTOM_TOKENS)),
            overrides = configs,
            added_chain_ids = custom.map { it.chain_id },
        )
        val settled = runCatching {
            wire.decodeFromString(
                NetShownSettled.serializer(),
                settleShown("", wire.encodeToString(NetUseEvidence.serializer(), evidence)),
            )
        }.getOrNull() ?: return null
        if (settled.write) {
            // Best effort: the record still holds for this session, and the
            // next load settles again on the same evidence.
            val written = runCatching {
                store.write(KeyValueStore.Keys.SHOWN_NETWORKS, wire.encodeToString(NetShownNetworks.serializer(), settled.record))
            }.getOrDefault(false)
            VelaLog.event("net.shown", "settled", "shown" to settled.record.shown.size, "written" to written)
        }
        return settled.record
    }

    /** The `chainId` of every record in a stored array (activity, custom tokens); unreadable: none. */
    private fun chainIdsOf(raw: String?): List<Long> =
        objects(raw).mapNotNull { record -> record.optLong("chainId", 0).takeIf { it > 0 } }.distinct()

    private fun JSONArray.longs(): List<Long> =
        (0 until length()).mapNotNull { (opt(it) as? Number)?.toLong()?.takeIf { id -> id > 0 } }

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
