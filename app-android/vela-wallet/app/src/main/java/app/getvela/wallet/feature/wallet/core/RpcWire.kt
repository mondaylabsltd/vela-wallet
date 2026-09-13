package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `rpc_pool` machine's wire types, in Kotlin.
 *
 * A transcription of `rust/crates/vela-core/src/app/rpc_pool.rs`. Every routing
 * rule stays there: six-tier source scoring, EMA latency, failure cooldowns,
 * temporary and permanent bans, the four-way error classification, the
 * three-pass jittered sweep, the fastest-RPC race, and the
 * failed-versus-rate-limited verdict.
 *
 * **The core is a pure router.** It decides *which URL next and why*; it
 * carries no JSON-RPC params and no response bodies. That is why
 * [RpcOperation.JsonRpcPost] names a `method` and no `params` — the shell holds
 * the payload for the call it started, keyed by `call_id`.
 *
 * **Numerics come from the Rust, not the mirror** (research D5). ts-rs writes
 * `u32`, `u16`, `i32` and `f64` all as `number`, and serde rejects a float
 * where an integer belongs:
 *
 * - `chain_id`, `timeout_ms`, `delay_ms`, `reported` → `u32` → `Int`
 * - `status` → `u16` → `Int`
 * - `code` → `i32` → `Int`
 * - `banned_at_ms`, `now_ms`, `latency_ms`, `max_span` → `f64` → `Double`
 */

// -- endpoints and bans ------------------------------------------------------

/** Where an endpoint came from — the first tier of the core's scoring. */
@Serializable
enum class RpcSource {
    @SerialName("user") User,

    @SerialName("provider") Provider,

    @SerialName("default") Default,

    @SerialName("public") Public,

    @SerialName("builtin") Builtin,

    @SerialName("fallback") Fallback,
}

@Serializable
enum class RpcKind {
    @SerialName("rpc") Rpc,

    @SerialName("bundler") Bundler,
}

@Serializable
data class RpcEndpointSeed(val url: String, val source: RpcSource)

/**
 * An endpoint the core has ruled against.
 *
 * Persisted, and **shared with the other clients**: a device that banned a
 * dead endpoint should agree with its siblings rather than each rediscovering
 * it. Temporary bans last an hour, permanent ones a day — the core's rule,
 * evaluated against `now_ms`.
 */
@Serializable
data class RpcBanEntry(
    val url: String,
    val banned_at_ms: Double,
    val permanent: Boolean = false,
)

// -- what the transport observed ---------------------------------------------

@Serializable
data class RpcErrorInfo(val code: Int? = null, val message: String? = null)

/**
 * What the shell's fetch observed, **mechanically**.
 *
 * HTTP status and body shape are transport facts. Everything meaningful about
 * them — is this a rate limit, is this endpoint worth banning, should the
 * range be capped — is the core's to decide from these.
 */
@Serializable
sealed class RpcTransportOutcome {
    /** A JSON-RPC response arrived; `error` is its `error` member, if any. */
    @Serializable
    @SerialName("response")
    data class Response(val error: RpcErrorInfo? = null) : RpcTransportOutcome()

    @Serializable
    @SerialName("http_error")
    data class HttpError(val status: Int) : RpcTransportOutcome()

    @Serializable
    @SerialName("non_json")
    data object NonJson : RpcTransportOutcome()

    @Serializable
    @SerialName("timeout")
    data object Timeout : RpcTransportOutcome()

    @Serializable
    @SerialName("network")
    data object Network : RpcTransportOutcome()
}

/** How a routed call ended, in the core's words. */
@Serializable
sealed class RpcCallVerdict {
    /** Use the body the shell kept for this url. */
    @Serializable
    @SerialName("respond")
    data class Respond(val url: String) : RpcCallVerdict()

    /** The endpoint capped the block range; retry within `max_span`. */
    @Serializable
    @SerialName("range_cap")
    data class RangeCap(val url: String, val max_span: Double) : RpcCallVerdict()

    /**
     * Nothing answered.
     *
     * `rate_limited` is the difference between "this chain is broken" and
     * "come back in a moment", and the two must not look the same on screen.
     */
    @Serializable
    @SerialName("failed")
    data class Failed(val rate_limited: Boolean = false) : RpcCallVerdict()

    @Serializable
    @SerialName("bundler_base")
    data class BundlerBase(val base_url: String? = null) : RpcCallVerdict()

    @Serializable
    @SerialName("best_rpc_url")
    data class BestRpcUrl(val url: String? = null) : RpcCallVerdict()
}

// -- what the screen renders -------------------------------------------------

@Serializable
data class RpcPoolView(
    val failed_chains: List<Int> = emptyList(),
    val rate_limited_chains: List<Int> = emptyList(),
    val banned: List<RpcBanEntry> = emptyList(),
)

// -- what the shell sends ----------------------------------------------------

@Serializable
sealed class RpcEvent {
    @Serializable
    @SerialName("bans_loaded")
    data class BansLoaded(val entries: List<RpcBanEntry> = emptyList()) : RpcEvent()

    @Serializable
    @SerialName("call_requested")
    data class CallRequested(
        val call_id: String,
        val chain_id: Int,
        val kind: RpcKind,
        val method: String,
        val now_ms: Double,
    ) : RpcEvent()

    @Serializable
    @SerialName("bundler_base_requested")
    data class BundlerBaseRequested(
        val call_id: String,
        val chain_id: Int,
        val now_ms: Double,
    ) : RpcEvent()

    @Serializable
    @SerialName("best_rpc_url_requested")
    data class BestRpcUrlRequested(
        val call_id: String,
        val chain_id: Int,
        val now_ms: Double,
    ) : RpcEvent()

    @Serializable
    @SerialName("invalidate_all")
    data object InvalidateAll : RpcEvent()

    @Serializable
    @SerialName("refresh_chain")
    data class RefreshChain(val chain_id: Int) : RpcEvent()
}

// -- what the core asks the shell to do --------------------------------------

@Serializable
sealed class RpcOperation {
    @Serializable
    @SerialName("load_pool_config")
    data class LoadPoolConfig(val chain_id: Int) : RpcOperation()

    /**
     * POST one JSON-RPC call to one URL.
     *
     * No `params`: the core routes, it does not carry payloads. The shell looks
     * up what to send by `call_id`.
     */
    @Serializable
    @SerialName("json_rpc_post")
    data class JsonRpcPost(
        val call_id: String,
        val url: String,
        val method: String,
        /**
         * Bundler calls only: the verified same-chain RPC URL for the
         * `X-Rpc-Url` header. `null` ⇒ send no header.
         */
        val x_rpc_url: String? = null,
        val timeout_ms: Int,
    ) : RpcOperation()

    @Serializable
    @SerialName("probe_chain_id")
    data class ProbeChainId(
        val chain_id: Int,
        val url: String,
        val timeout_ms: Int,
    ) : RpcOperation()

    @Serializable
    @SerialName("draw_jitter")
    data class DrawJitter(val call_id: String) : RpcOperation()

    @Serializable
    @SerialName("start_backoff")
    data class StartBackoff(val call_id: String, val delay_ms: Int) : RpcOperation()

    @Serializable
    @SerialName("persist_bans")
    data class PersistBans(val entries: List<RpcBanEntry> = emptyList()) : RpcOperation()

    /** The call is over. The shell settles whoever is waiting on it. */
    @Serializable
    @SerialName("conclude")
    data class Conclude(val call_id: String, val verdict: RpcCallVerdict) : RpcOperation()
}

// -- what the shell observed -------------------------------------------------

/**
 * Every result self-identifies (`call_id` / chain / url) so a stale answer is
 * dropped by construction, and every time-bearing result carries `now_ms`.
 */
@Serializable
sealed class RpcShellResult {
    @Serializable
    @SerialName("pool_config")
    data class PoolConfig(
        val chain_id: Int,
        val rpc_endpoints: List<RpcEndpointSeed> = emptyList(),
        val bundler_endpoints: List<RpcEndpointSeed> = emptyList(),
        val now_ms: Double,
    ) : RpcShellResult()

    @Serializable
    @SerialName("post_outcome")
    data class PostOutcome(
        val call_id: String,
        val url: String,
        val outcome: RpcTransportOutcome,
        val latency_ms: Double,
        val now_ms: Double,
    ) : RpcShellResult()

    @Serializable
    @SerialName("chain_id_probed")
    data class ChainIdProbed(
        val chain_id: Int,
        val url: String,
        val reported: Int? = null,
        val latency_ms: Double,
        val now_ms: Double,
    ) : RpcShellResult()

    @Serializable
    @SerialName("jitter")
    data class Jitter(val call_id: String, val value: Double) : RpcShellResult()

    @Serializable
    @SerialName("backoff_elapsed")
    data class BackoffElapsed(val call_id: String, val now_ms: Double) : RpcShellResult()

    @Serializable
    @SerialName("persisted")
    data object Persisted : RpcShellResult()

    @Serializable
    @SerialName("concluded")
    data object Concluded : RpcShellResult()
}
