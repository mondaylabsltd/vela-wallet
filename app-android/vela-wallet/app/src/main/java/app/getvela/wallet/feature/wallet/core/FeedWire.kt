package app.getvela.wallet.feature.wallet.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `activity_feed` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/activity_feed.rs`.
 *
 * Numeric types come from the RUST, never from the generated TypeScript, which
 * writes `u32`, `u64` and `f64` all as `number` — the blind spot 040 shipped a
 * bug through. Here: `chain_id`/`decimals`/`count`/`read_id`/`generation` are
 * `u32` → `Int`; `timestamp`, `day_start_ms`, `usd_value`, `now_ms` are `f64`
 * → `Double`.
 *
 * **`timestamp` is epoch SECONDS. `day_start_ms` is epoch MILLISECONDS.** They
 * sit next to each other in the same struct and differ by a factor of a
 * thousand; mixing them puts every row in 1970.
 */

// -- value types -------------------------------------------------------------

@Serializable
enum class FeedTxKind {
    @SerialName("send") Send,

    @SerialName("receive") Receive,

    @SerialName("dapp_tx") DappTx,

    @SerialName("sign_message") SignMessage,

    @SerialName("sign_typed_data") SignTypedData,

    @SerialName("connect") Connect,
}

@Serializable
enum class FeedTxStatus {
    @SerialName("pending") Pending,

    @SerialName("confirmed") Confirmed,

    @SerialName("failed") Failed,
}

@Serializable
enum class FeedDirection {
    @SerialName("in") In,

    @SerialName("out") Out,
}

/** `split` = one token → N recipients; `multi_select` = N tokens → 1 recipient. */
@Serializable
enum class FeedBatchKind {
    @SerialName("split") Split,

    @SerialName("multi_select") MultiSelect,
}

/**
 * One stored transaction, as this device persists it.
 *
 * `day_start_ms` is the shell's job and only the shell's: it is local midnight
 * for `timestamp` on THIS device, in THIS timezone, which is the one fact the
 * core cannot compute. Getting it wrong groups a payment under the wrong day.
 */
@Serializable
data class FeedTxRecord(
    val id: String,
    val user_op_hash: String = "",
    val tx_hash: String = "",
    val from: String,
    val to: String,
    val to_name: String? = null,
    val value: String,
    val symbol: String,
    val decimals: Int,
    val logo_urls: List<String>? = null,
    val chain_id: Int,
    /** Epoch **seconds**. */
    val timestamp: Double,
    /** Local-midnight epoch **milliseconds** for [timestamp]. */
    val day_start_ms: Double,
    val status: FeedTxStatus,
    /** `null` = a legacy untyped record, which the core reads as a send. */
    val kind: FeedTxKind? = null,
    val usd: String? = null,
)

@Serializable
data class FeedBatchTransfer(
    val to: String,
    val to_name: String? = null,
    val value: String,
    val symbol: String,
    val decimals: Int,
    val usd_value: Double,
    val logo_urls: List<String>? = null,
)

@Serializable
data class FeedBatch(
    val kind: FeedBatchKind,
    val count: Int,
    val total_usd: Double,
    val transfers: List<FeedBatchTransfer> = emptyList(),
    val ids: List<String> = emptyList(),
    val from: String,
    val chain_id: Int,
    val timestamp: Double,
    val status: FeedTxStatus,
    val tx_hash: String = "",
    val user_op_hash: String = "",
    val symbol: String? = null,
    val logo_urls: List<String>? = null,
    val to: String? = null,
    val to_name: String? = null,
)

/** One feed row's payload. The shell formats; the core decides what is in it. */
@Serializable
data class FeedItem(
    val id: String,
    val direction: FeedDirection,
    val counterparty: String? = null,
    /** The resolved name, or the one stored at send time. */
    val alias: String? = null,
    /** `null` for a multi-token batch row: mixed tokens cannot be summed. */
    val value: String? = null,
    val symbol: String = "",
    val decimals: Int? = null,
    /** Numeric USD, `0` when unknown. */
    val usd_value: Double = 0.0,
    val chain_id: Int,
    /** Epoch **seconds**. */
    val timestamp: Double,
    /** Local-midnight epoch **milliseconds**. */
    val day_start_ms: Double,
    val tx_hash: String? = null,
    val batch: FeedBatch? = null,
)

/**
 * A date header or an item, already interleaved in render order.
 *
 * The core emits them interleaved precisely so a shell cannot sort a header
 * away from the day it belongs to.
 */
@Serializable
sealed class FeedRow {
    @Serializable
    @SerialName("header")
    data class Header(
        val id: String,
        val day_start_ms: Double,
        /** A timestamp inside the day, from which the shell writes the label. */
        val timestamp: Double,
    ) : FeedRow()

    @Serializable
    @SerialName("item")
    data class Item(val item: FeedItem) : FeedRow()
}

@Serializable
data class FeedToast(
    val item_id: String,
    val value: String,
    val symbol: String,
    val deadline_ms: Double,
)

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class FeedOperation {
    @Serializable
    @SerialName("read_tx_store")
    data class ReadTxStore(val address: String, val read_id: Int) : FeedOperation()

    @Serializable
    @SerialName("scan_incoming_transfers")
    data class ScanIncomingTransfers(val address: String) : FeedOperation()

    @Serializable
    @SerialName("delete_tx_record")
    data class DeleteTxRecord(val id: String) : FeedOperation()

    @Serializable
    @SerialName("resolve_recipient_identity")
    data class ResolveRecipientIdentity(val addr: String) : FeedOperation()

    @Serializable
    @SerialName("timer")
    data class Timer(val ms: Int, val generation: Int) : FeedOperation()

    @Serializable
    @SerialName("haptic")
    data object Haptic : FeedOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class FeedShellResult {
    @Serializable
    @SerialName("store_loaded")
    data class StoreLoaded(
        val records: List<FeedTxRecord> = emptyList(),
        val now_ms: Double,
        /** Echoed from the read that produced it — the celebration's binding. */
        val read_id: Int,
    ) : FeedShellResult()

    @Serializable
    @SerialName("sync_completed")
    data class SyncCompleted(val new_count: Int) : FeedShellResult()

    @Serializable
    @SerialName("delete_committed")
    data class DeleteCommitted(val id: String) : FeedShellResult()

    @Serializable
    @SerialName("delete_failed")
    data class DeleteFailed(val id: String) : FeedShellResult()

    @Serializable
    @SerialName("alias_resolved")
    data class AliasResolved(val addr: String, val name: String? = null) : FeedShellResult()

    @Serializable
    @SerialName("toast_expired")
    data class ToastExpired(val generation: Int) : FeedShellResult()

    @Serializable
    @SerialName("haptic_played")
    data object HapticPlayed : FeedShellResult()
}

// -- what the screen sends ---------------------------------------------------

@Serializable
sealed class FeedEvent {
    @Serializable
    @SerialName("account_switched")
    data class AccountSwitched(val address: String) : FeedEvent()

    @Serializable
    @SerialName("focus_tick")
    data object FocusTick : FeedEvent()

    @Serializable
    @SerialName("live_tick")
    data object LiveTick : FeedEvent()

    @Serializable
    @SerialName("reconcile_completed")
    data class ReconcileCompleted(val resolved_count: Int) : FeedEvent()

    @Serializable
    @SerialName("privacy_changed")
    data class PrivacyChanged(val hidden: Boolean) : FeedEvent()

    @Serializable
    @SerialName("chain_filter_changed")
    data class ChainFilterChanged(val chain_id: Int? = null) : FeedEvent()

    @Serializable
    @SerialName("delete_requested")
    data class DeleteRequested(val id: String) : FeedEvent()
}

// -- what the screen renders -------------------------------------------------

@Serializable
data class FeedView(
    val rows: List<FeedRow> = emptyList(),
    /** Account-scoped records for the detail sheet — not tombstone-filtered. */
    val transactions: List<FeedTxRecord> = emptyList(),
    /** The row that just landed and should glow. */
    val new_item_id: String? = null,
    /** `null` while balance privacy is on — the core enforces that, not the shell. */
    val toast: FeedToast? = null,
)
