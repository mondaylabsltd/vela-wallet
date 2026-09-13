package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `token_trust` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/token_trust.rs`.
 *
 * This is the machine that decides which incoming transfer a person is
 * actually told about. **The shell hands over raw logs and nothing else** —
 * whether a log is a real Transfer to this wallet, whether the token is
 * trusted, whether an unresolvable symbol may be shown, and whether a token
 * may be added to the person's list are all the core's, and every one of them
 * is a place a scam token gets in if a shell starts deciding.
 *
 * Numeric types from the Rust: `chain_id`, `log_index`, `decimals`, `cap` are
 * `u32` → `Int`; `block_number`, `timestamp_sec`, `now_ms` are `f64` → `Double`
 * (a block number crosses as a double because it can exceed 2^32, and the core
 * takes it as one).
 */

// -- value types -------------------------------------------------------------

/**
 * A raw `eth_getLogs` entry, untouched.
 *
 * Passed through verbatim on purpose: this core, not the endpoint, decides
 * what a log means. A shell that pre-filtered would be deciding.
 */
@Serializable
data class TrustRawLog(
    val address: String,
    val topics: List<String> = emptyList(),
    val data: String = "",
    val transaction_hash: String = "",
    /** Hex quantity; absent reads as `0x0`. */
    val block_number: String? = null,
    /** Hex quantity; absent reads as `0x0`. */
    val log_index: String? = null,
)

@Serializable
data class TrustReceiptLog(
    val address: String,
    val topics: List<String> = emptyList(),
    val data: String = "",
)

@Serializable
enum class TrustDeltaKind {
    @SerialName("native") Native,

    @SerialName("erc20") Erc20,
}

@Serializable
data class TrustAssetDelta(
    val kind: TrustDeltaKind,
    val token: String? = null,
    /** Signed decimal string in the asset's smallest unit. */
    val delta: String,
)

@Serializable
data class TrustTokenMeta(val symbol: String, val decimals: Int)

@Serializable
data class TrustCustomToken(
    /** `"{chainId}_{contractAddress}"` — the de-dupe identity. */
    val id: String,
    val chain_id: Int,
    val contract_address: String,
    val symbol: String,
    val name: String,
    val decimals: Int,
)

/**
 * One entry of a metadata answer.
 *
 * `meta = null` means "looked up and unresolvable" — a fact worth remembering,
 * and **not** licence to invent a default symbol. A token whose name cannot be
 * read is exactly the one not to display confidently.
 */
@Serializable
data class TrustMetaEntry(val addr: String, val meta: TrustTokenMeta? = null)

/** How an `eth_getLogs` ended, on the axis the core acts on. */
@Serializable
sealed class TrustLogsOutcome {
    @Serializable
    @SerialName("ok")
    data class Ok(val logs: List<TrustRawLog> = emptyList()) : TrustLogsOutcome()

    /**
     * The endpoint refused the span. `cap` is the block count it will allow;
     * `0` means it capped without saying a number, and the core stays
     * conservative rather than guessing.
     */
    @Serializable
    @SerialName("range_capped")
    data class RangeCapped(val cap: Int) : TrustLogsOutcome()

    @Serializable
    @SerialName("failed")
    data object Failed : TrustLogsOutcome()
}

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class TrustOperation {
    @Serializable
    @SerialName("rpc_block_number")
    data class RpcBlockNumber(val address: String, val chain_id: Int) : TrustOperation()

    @Serializable
    @SerialName("rpc_get_logs")
    data class RpcGetLogs(
        val address: String,
        val chain_id: Int,
        val from_block: String,
        val to_block: String,
        val recipient_topic: String,
        /** The allowlist. An empty list is not "everything" — it is nothing. */
        val contracts: List<String> = emptyList(),
    ) : TrustOperation()

    @Serializable
    @SerialName("rpc_get_block_by_number")
    data class RpcGetBlockByNumber(
        val address: String,
        val chain_id: Int,
        val block: String,
    ) : TrustOperation()

    @Serializable
    @SerialName("multicall_erc20_meta")
    data class MulticallErc20Meta(
        val chain_id: Int,
        val addrs: List<String> = emptyList(),
    ) : TrustOperation()

    @Serializable
    @SerialName("read_custom_tokens")
    data object ReadCustomTokens : TrustOperation()

    @Serializable
    @SerialName("write_custom_token")
    data class WriteCustomToken(val token: TrustCustomToken) : TrustOperation()

    @Serializable
    @SerialName("invalidate_token_cache")
    data class InvalidateTokenCache(val address: String) : TrustOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class TrustShellResult {
    @Serializable
    @SerialName("block_number")
    data class BlockNumber(
        val address: String,
        val chain_id: Int,
        val block_hex: String? = null,
    ) : TrustShellResult()

    @Serializable
    @SerialName("logs")
    data class Logs(
        val address: String,
        val chain_id: Int,
        val outcome: TrustLogsOutcome,
    ) : TrustShellResult()

    @Serializable
    @SerialName("block_timestamp")
    data class BlockTimestamp(
        val address: String,
        val chain_id: Int,
        val block_number: Double,
        /** `null` = the header could not be read; the transfer falls back to now. */
        val timestamp_sec: Double? = null,
        val now_ms: Double,
    ) : TrustShellResult()

    @Serializable
    @SerialName("erc_meta")
    data class ErcMeta(
        val chain_id: Int,
        val entries: List<TrustMetaEntry> = emptyList(),
    ) : TrustShellResult()

    /** `tokens = null` = the read itself failed, which fails admission closed. */
    @Serializable
    @SerialName("custom_tokens")
    data class CustomTokens(val tokens: List<TrustCustomToken>? = null) : TrustShellResult()

    @Serializable
    @SerialName("token_written")
    data class TokenWritten(val ok: Boolean) : TrustShellResult()

    @Serializable
    @SerialName("cache_invalidated")
    data object CacheInvalidated : TrustShellResult()
}

// -- what the shell tells it -------------------------------------------------

@Serializable
sealed class TrustEvent {
    /** Which chains this account actually uses. Empty = a brand-new wallet. */
    @Serializable
    @SerialName("held_chains_snapshot")
    data class HeldChainsSnapshot(
        val address: String,
        val chain_ids: List<Int> = emptyList(),
    ) : TrustEvent()

    /** The ERC-20s held on one chain — a cold cache means an empty trusted set. */
    @Serializable
    @SerialName("held_tokens_snapshot")
    data class HeldTokensSnapshot(
        val address: String,
        val chain_id: Int,
        val tokens: List<String> = emptyList(),
    ) : TrustEvent()

    @Serializable
    @SerialName("registry_tokens_snapshot")
    data class RegistryTokensSnapshot(
        val chain_id: Int,
        val stables: List<String> = emptyList(),
        val wrapped_native: String? = null,
    ) : TrustEvent()

    @Serializable
    @SerialName("custom_tokens_loaded")
    data class CustomTokensLoaded(
        val tokens: List<TrustCustomToken> = emptyList(),
    ) : TrustEvent()

    /** One poll across the held chains. Single-flight: the core ignores a second. */
    @Serializable
    @SerialName("poll_requested")
    data class PollRequested(val address: String) : TrustEvent()

    /**
     * **Authentic** receipt logs of a confirmed transaction — the only path
     * that may ever add a token to somebody's list.
     */
    @Serializable
    @SerialName("receipt_logs_confirmed")
    data class ReceiptLogsConfirmed(
        val from: String,
        val chain_id: Int,
        val logs: List<TrustReceiptLog> = emptyList(),
    ) : TrustEvent()

    /** **Untrusted** sign-time simulation deltas: shown, never written. */
    @Serializable
    @SerialName("sim_deltas_computed")
    data class SimDeltasComputed(
        val address: String,
        val chain_id: Int,
        val deltas: List<TrustAssetDelta> = emptyList(),
    ) : TrustEvent()
}

// -- what the screen renders -------------------------------------------------

@Serializable
data class TrustIncomingView(
    /** `{chainId}-{txHash}-{logIndex}` — the de-dupe identity across polls. */
    val id: String,
    val chain_id: Int,
    val token: String? = null,
    val is_native: Boolean,
    val from: String,
    /** The raw on-chain amount, NOT divided by decimals. */
    val value: String,
    val tx_hash: String,
    val block_number: Double,
    val log_index: Int,
    /** Unix seconds: block time, falling back to when it was seen. */
    val timestamp_sec: Double,
    val symbol: String? = null,
    val decimals: Int? = null,
)

@Serializable
sealed class TrustSimJudgment {
    @Serializable
    @SerialName("native")
    data class Native(val delta: String) : TrustSimJudgment()

    @Serializable
    @SerialName("erc20_trusted")
    data class Erc20Trusted(
        val token: String,
        val delta: String,
        val symbol: String,
        val decimals: Int,
    ) : TrustSimJudgment()

    @Serializable
    @SerialName("erc20_unverified")
    data class Erc20Unverified(
        val token: String? = null,
        val delta: String,
    ) : TrustSimJudgment()
}

@Serializable
data class TrustSimView(
    val address: String,
    val chain_id: Int,
    /** False while the metadata behind the judgement is still resolving. */
    val ready: Boolean = false,
    val judgments: List<TrustSimJudgment> = emptyList(),
)

@Serializable
data class TrustView(
    val address: String? = null,
    val scanning: Boolean = false,
    /** Newest first: block descending, then log index. */
    val incoming: List<TrustIncomingView> = emptyList(),
    val sim: TrustSimView? = null,
)
