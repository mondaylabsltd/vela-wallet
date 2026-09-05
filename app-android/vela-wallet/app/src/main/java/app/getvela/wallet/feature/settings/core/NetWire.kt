package app.getvela.wallet.feature.settings.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `network_admin` machine's wire types, in Kotlin.
 *
 * A transcription of `rust/crates/vela-core/src/app/network_admin.rs` (2,900
 * lines of rules — none of them repeated here), checked against the generated
 * mirrors `app-web/vela-wallet/src/lib/core/generated/Net*.ts` by
 * `CoreWireDriftTest`.
 *
 * Sixteen operations, six of which are storage and ten of which are network.
 * Spec 040 answers the six and fail-closes the ten; every one of those arms
 * carries `// live in 041` so the next spec can find its own work by grep.
 */

// ---------------------------------------------------------------------------
// Stored shapes — these cross BOTH ways (an operation carries them out, a
// result carries them back), so they must round-trip exactly.
// ---------------------------------------------------------------------------

/** A stored custom network — 1:1 with `vela.customNetworks`. */
@Serializable
data class NetCustomNetwork(
    /** `custom-{chainId}`. */
    val id: String,
    val display_name: String,
    val chain_id: Long,
    val icon_label: String,
    val icon_color: String,
    val icon_bg: String,
    val logo_url: String,
    val is_l2: Boolean,
    val rpc_url: String,
    val explorer_url: String,
    val bundler_url: String,
    val native_symbol: String,
    /** ISO 8601 — stamped by the shell on the triggering event. */
    val added_at_iso: String,
)

/** A per-network endpoint override — 1:1 with `vela.networkConfig`. */
@Serializable
data class NetNetworkConfig(
    val chain_id: Long,
    val rpc_url: String,
    val explorer_url: String,
    val bundler_url: String,
)

/** The four service endpoints as the core sends them — complete, non-null. */
@Serializable
data class NetServiceEndpoints(
    val ethereum_data_url: String,
    val passkey_index_url: String,
    val bundler_service_url: String,
    val fiat_rates_url: String,
)

/**
 * The same four as *read back*, where absent is a real answer.
 *
 * Not the same type as [NetServiceEndpoints] and deliberately not merged with
 * it: "nothing configured" and "configured to empty string" are different
 * facts, and the core decides what each means.
 */
@Serializable
data class NetStoredEndpoints(
    val ethereum_data_url: String? = null,
    val passkey_index_url: String? = null,
    val bundler_service_url: String? = null,
    val fiat_rates_url: String? = null,
)

/** One API key per provider. `null` = not configured; a cleared key is removed. */
@Serializable
data class NetProviderKeys(
    val alchemy: String? = null,
    val drpc: String? = null,
    val ankr: String? = null,
)

@Serializable
enum class NetEndpointField {
    @SerialName("ethereum_data") EthereumData,

    @SerialName("passkey_index") PasskeyIndex,

    @SerialName("bundler_service") BundlerService,

    @SerialName("fiat_rates") FiatRates,
}

@Serializable
enum class NetProviderId {
    @SerialName("alchemy") Alchemy,

    @SerialName("drpc") Drpc,

    @SerialName("ankr") Ankr,
}

@Serializable
enum class NetOverrideField {
    @SerialName("rpc") Rpc,

    @SerialName("explorer") Explorer,
}

// ---------------------------------------------------------------------------
// What the screen renders
// ---------------------------------------------------------------------------

@Serializable
data class NetView(
    val loaded: Boolean = false,
    val networks: List<NetNetworkRow> = emptyList(),
    val wizard: NetWizardView = NetWizardView(),
    val endpoints: List<NetEndpointView> = emptyList(),
    val providers: List<NetProviderView> = emptyList(),
    val last_added_chain_id: Long? = null,
)

/**
 * One row of the network editor.
 *
 * `rpc_health` and `explorer_health` are `null` for the whole of spec 040 —
 * nothing can probe an endpoint until 041 — and the settings screen renders
 * that as *unknown* rather than as a green tick. // live in 041
 */
@Serializable
data class NetNetworkRow(
    val id: String,
    val chain_id: Long,
    val display_name: String,
    val native_symbol: String,
    val is_custom: Boolean = false,
    val rpc_url: String = "",
    val explorer_url: String = "",
    val bundler_url: String = "",
    val rpc_health: NetProbeHealth? = null,
    val explorer_health: NetProbeHealth? = null,
    val rpc_chain_mismatch: NetChainMismatch? = null,
    val rpc_save_deferred: Boolean = false,
)

@Serializable
data class NetChainMismatch(
    val expected_chain_id: Long,
    val reported_chain_id: Long,
)

@Serializable
sealed class NetProbeHealth {
    @Serializable
    @SerialName("checking")
    data object Checking : NetProbeHealth()

    @Serializable
    @SerialName("ok")
    data class Ok(val latency_ms: Long) : NetProbeHealth()

    @Serializable
    @SerialName("error")
    data object Error : NetProbeHealth()
}

@Serializable
sealed class NetServiceHealth {
    @Serializable
    @SerialName("checking")
    data object Checking : NetServiceHealth()

    @Serializable
    @SerialName("ok")
    data class Ok(val latency_ms: Long, val rate_count: Long? = null) : NetServiceHealth()

    @Serializable
    @SerialName("not_https")
    data object NotHttps : NetServiceHealth()

    @Serializable
    @SerialName("unreachable")
    data class Unreachable(
        val http_status: Long? = null,
        val latency_ms: Long? = null,
    ) : NetServiceHealth()

    @Serializable
    @SerialName("invalid_response")
    data class InvalidResponse(val latency_ms: Long) : NetServiceHealth()
}

@Serializable
data class NetEndpointView(
    val field: NetEndpointField,
    val value: String,
    /** The placeholder — what this endpoint is when nobody has overridden it. */
    val default_value: String,
    val health: NetServiceHealth,
)

@Serializable
data class NetProviderView(
    val provider: NetProviderId,
    val key: String,
    val has_key: Boolean,
    val test: NetProviderTestView? = null,
)

@Serializable
data class NetProviderTestView(
    val done: Boolean,
    val results: List<NetProviderNetRow> = emptyList(),
    val ok_count: Long,
    val total: Long,
)

@Serializable
data class NetProviderNetRow(val chain_id: Long, val ok: Boolean, val latency_ms: Long)

@Serializable
enum class NetWizardPhase {
    @SerialName("idle") Idle,

    @SerialName("searching") Searching,

    @SerialName("suggested") Suggested,

    @SerialName("resolving") Resolving,

    @SerialName("checking") Checking,

    @SerialName("checked") Checked,

    @SerialName("error") Error,
}

@Serializable
data class NetWizardView(
    val phase: NetWizardPhase = NetWizardPhase.Idle,
    val query: String = "",
    val custom_rpc: String = "",
    val suggestions: List<NetChainIndexEntry> = emptyList(),
    val chain_info: NetChainInfo? = null,
    val compat: NetCompatibility? = null,
    val error: NetWizardErrorKind? = null,
    val can_add: Boolean = false,
)

@Serializable
data class NetChainIndexEntry(
    val chain_id: Long,
    val name: String,
    val short_name: String,
    val native_currency_symbol: String,
    val has_logo: Boolean,
)

@Serializable
data class NetChainInfo(
    val chain_id: Long,
    val name: String,
    val short_name: String,
    val native_name: String,
    val native_symbol: String,
    val native_decimals: Long,
    val rpc_url: String,
    val rpc_urls: List<String> = emptyList(),
    val explorer_url: String,
    val logo_url: String,
    val is_testnet: Boolean,
)

@Serializable
data class NetCompatibility(
    val chain_id: Long,
    val compatible: Boolean,
    val contracts: List<NetContractStatus> = emptyList(),
    val p256_available: Boolean? = null,
    val best_rpc_url: String? = null,
    val best_rpc_latency_ms: Long? = null,
    val rpc_failure: NetRpcFailureKind? = null,
)

@Serializable
data class NetContractStatus(val name: String, val address: String, val deployed: Boolean)

@Serializable
enum class NetRpcFailureKind {
    @SerialName("no_https_candidates") NoHttpsCandidates,

    @SerialName("all_probes_failed") AllProbesFailed,
}

@Serializable
sealed class NetWizardErrorKind {
    @Serializable
    @SerialName("already_added")
    data class AlreadyAdded(val chain_id: Long) : NetWizardErrorKind()

    @Serializable
    @SerialName("not_found")
    data class NotFound(val chain_id: Long) : NetWizardErrorKind()

    @Serializable
    @SerialName("no_rpc_endpoint")
    data object NoRpcEndpoint : NetWizardErrorKind()

    @Serializable
    @SerialName("not_compatible")
    data class NotCompatible(val chain_id: Long) : NetWizardErrorKind()
}

// ---------------------------------------------------------------------------
// What the screen sends
// ---------------------------------------------------------------------------

/**
 * `NetEvent` — every variant, because the settings surface can reach all of
 * them once its fields accept typing (spec 040 phase 4).
 *
 * `now_iso` on the add events is the shell's: the core takes no clock, so the
 * timestamp stamped on a saved network comes from here.
 */
@Serializable
sealed class NetEvent {
    @Serializable
    @SerialName("started")
    data object Started : NetEvent()

    @Serializable
    @SerialName("search_input")
    data class SearchInput(val query: String) : NetEvent()

    @Serializable
    @SerialName("chain_selected")
    data class ChainSelected(val chain_id: Long, val keep_custom_rpc: Boolean) : NetEvent()

    @Serializable
    @SerialName("custom_rpc_edited")
    data class CustomRpcEdited(val value: String) : NetEvent()

    @Serializable
    @SerialName("add_confirmed")
    data class AddConfirmed(val now_iso: String) : NetEvent()

    @Serializable
    @SerialName("wizard_reset")
    data object WizardReset : NetEvent()

    @Serializable
    @SerialName("add_by_chain_id_requested")
    data class AddByChainIdRequested(val chain_id: Long, val now_iso: String) : NetEvent()

    @Serializable
    @SerialName("delete_confirmed")
    data class DeleteConfirmed(val id: String) : NetEvent()

    @Serializable
    @SerialName("override_expanded")
    data class OverrideExpanded(val chain_id: Long) : NetEvent()

    @Serializable
    @SerialName("override_field_edited")
    data class OverrideFieldEdited(
        val chain_id: Long,
        val field: NetOverrideField,
        val value: String,
    ) : NetEvent()

    @Serializable
    @SerialName("override_blurred")
    data class OverrideBlurred(val chain_id: Long) : NetEvent()

    @Serializable
    @SerialName("endpoints_opened")
    data object EndpointsOpened : NetEvent()

    @Serializable
    @SerialName("endpoint_edited")
    data class EndpointEdited(val field: NetEndpointField, val value: String) : NetEvent()

    @Serializable
    @SerialName("endpoint_blurred")
    data class EndpointBlurred(val field: NetEndpointField) : NetEvent()

    @Serializable
    @SerialName("endpoints_refresh_requested")
    data object EndpointsRefreshRequested : NetEvent()

    @Serializable
    @SerialName("reset_endpoints_to_defaults")
    data object ResetEndpointsToDefaults : NetEvent()

    @Serializable
    @SerialName("providers_opened")
    data object ProvidersOpened : NetEvent()

    @Serializable
    @SerialName("provider_key_edited")
    data class ProviderKeyEdited(val provider: NetProviderId, val value: String) : NetEvent()

    @Serializable
    @SerialName("provider_key_blurred")
    data class ProviderKeyBlurred(val provider: NetProviderId) : NetEvent()

    @Serializable
    @SerialName("provider_test_requested")
    data class ProviderTestRequested(val provider: NetProviderId) : NetEvent()
}

// ---------------------------------------------------------------------------
// What the core asks the shell to do
// ---------------------------------------------------------------------------

@Serializable
sealed class NetOperation {
    @Serializable
    @SerialName("read_store")
    data object ReadStore : NetOperation()

    @Serializable
    @SerialName("write_custom_networks")
    data class WriteCustomNetworks(val networks: List<NetCustomNetwork>) : NetOperation()

    @Serializable
    @SerialName("write_network_configs")
    data class WriteNetworkConfigs(val configs: List<NetNetworkConfig>) : NetOperation()

    @Serializable
    @SerialName("write_service_endpoints")
    data class WriteServiceEndpoints(val endpoints: NetServiceEndpoints) : NetOperation()

    @Serializable
    @SerialName("write_rpc_providers")
    data class WriteRpcProviders(val keys: NetProviderKeys) : NetOperation()

    @Serializable
    @SerialName("start_search_debounce")
    data class StartSearchDebounce(val ms: Long) : NetOperation()

    @Serializable
    @SerialName("fetch_search_index")
    data object FetchSearchIndex : NetOperation()

    @Serializable
    @SerialName("fetch_chain_info")
    data class FetchChainInfo(val chain_id: Long) : NetOperation()

    @Serializable
    @SerialName("probe_rpc")
    data class ProbeRpc(val url: String) : NetOperation()

    @Serializable
    @SerialName("probe_reachable")
    data class ProbeReachable(val url: String) : NetOperation()

    @Serializable
    @SerialName("rpc_get_code")
    data class RpcGetCode(val url: String, val address: String) : NetOperation()

    @Serializable
    @SerialName("rpc_call_p256")
    data class RpcCallP256(val url: String) : NetOperation()

    @Serializable
    @SerialName("fetch_service_health")
    data class FetchServiceHealth(
        val field: NetEndpointField,
        val base_url: String,
    ) : NetOperation()

    @Serializable
    @SerialName("fetch_fiat_rates")
    data class FetchFiatRates(val url: String) : NetOperation()

    @Serializable
    @SerialName("invalidate_pools")
    data class InvalidatePools(val chain_id: Long? = null) : NetOperation()

    @Serializable
    @SerialName("clear_bundler_cache")
    data class ClearBundlerCache(val chain_id: Long) : NetOperation()
}

// ---------------------------------------------------------------------------
// What the shell observed
// ---------------------------------------------------------------------------

@Serializable
sealed class NetShellResult {
    @Serializable
    @SerialName("store_loaded")
    data class StoreLoaded(
        val custom_networks: List<NetCustomNetwork> = emptyList(),
        val network_configs: List<NetNetworkConfig> = emptyList(),
        val endpoints: NetStoredEndpoints = NetStoredEndpoints(),
        val provider_keys: NetProviderKeys = NetProviderKeys(),
    ) : NetShellResult()

    @Serializable
    @SerialName("written")
    data object Written : NetShellResult()

    @Serializable
    @SerialName("debounce_elapsed")
    data object DebounceElapsed : NetShellResult()

    @Serializable
    @SerialName("search_index")
    data class SearchIndex(val chains: List<NetChainIndexEntry> = emptyList()) : NetShellResult()

    @Serializable
    @SerialName("chain_info")
    data class ChainInfo(val chain_id: Long, val data: NetRawChainData? = null) : NetShellResult()

    @Serializable
    @SerialName("probed")
    data class Probed(
        val url: String,
        /** `null` = failed / timed out / no parseable id. */
        val reported_chain_id: Long? = null,
        val latency_ms: Long,
    ) : NetShellResult()

    @Serializable
    @SerialName("reachable")
    data class Reachable(val url: String, val ok: Boolean, val latency_ms: Long) : NetShellResult()

    @Serializable
    @SerialName("code")
    data class Code(
        val url: String,
        val address: String,
        val code: String? = null,
    ) : NetShellResult()

    @Serializable
    @SerialName("p256_call")
    data class P256Call(val url: String, val result: String? = null) : NetShellResult()

    @Serializable
    @SerialName("service_health")
    data class ServiceHealth(
        val field: NetEndpointField,
        val body: NetHealthBody,
        val latency_ms: Long,
    ) : NetShellResult()

    @Serializable
    @SerialName("fiat_rates")
    data class FiatRates(val body: NetHealthBody, val latency_ms: Long) : NetShellResult()

    @Serializable
    @SerialName("invalidated")
    data object Invalidated : NetShellResult()

    @Serializable
    @SerialName("bundler_cache_cleared")
    data object BundlerCacheCleared : NetShellResult()
}

@Serializable
data class NetRawChainData(
    val chain_id: Long? = null,
    val name: String? = null,
    val short_name: String? = null,
    val native_currency_name: String? = null,
    val native_currency_symbol: String? = null,
    val native_currency_decimals: Long? = null,
    val rpc: List<String> = emptyList(),
    val explorers: List<String> = emptyList(),
    val testnet: Boolean = false,
)

@Serializable
sealed class NetHealthBody {
    @Serializable
    @SerialName("failed")
    data object Failed : NetHealthBody()

    @Serializable
    @SerialName("http_error")
    data class HttpError(val status: Long) : NetHealthBody()

    @Serializable
    @SerialName("identity")
    data class Identity(
        val service: String? = null,
        val status: String? = null,
    ) : NetHealthBody()

    @Serializable
    @SerialName("rates")
    data class Rates(val rate_count: Long) : NetHealthBody()
}
