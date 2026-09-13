package app.getvela.wallet.feature.send.core

import app.getvela.wallet.feature.wallet.core.TrustReceiptLog
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * The `tx_tracker` machine's wire, transcribed from
 * `rust/crates/vela-core/src/app/tx_tracker.rs` (spec 043).
 *
 * After a submit the core follows the operation to a receipt. It owns the
 * cadence (receipt every 3 s, status every 12 s, a 120 s wait window, abandon
 * at 24 h); the shell owns a clock, two relay calls, the record store and the
 * notification. A receipt's logs cross untouched so `token_trust` can judge
 * them — the shell does not read a log.
 *
 * Numerics: `chain_id` `u32` → `Int`; `now_ms`, `submitted_at_ms` `f64` →
 * `Double`.
 */

// -- value types -------------------------------------------------------------

/** The relay's own word for where an operation is. */
@Serializable
enum class TrackLifecycle {
    @SerialName("not_found") NotFound,

    @SerialName("queued") Queued,

    @SerialName("not_submitted") NotSubmitted,

    @SerialName("submitted") Submitted,

    @SerialName("rejected") Rejected,

    @SerialName("included") Included,

    @SerialName("failed") Failed,
}

/** The core's verdict per tracked hash — what the receipt and the feed say. */
@Serializable
enum class TrackStatus {
    @SerialName("pending") Pending,

    @SerialName("fee_held") FeeHeld,

    @SerialName("confirmed") Confirmed,

    @SerialName("dropped") Dropped,

    @SerialName("rejected") Rejected,

    @SerialName("unreachable") Unreachable,

    @SerialName("accepted_not_landed") AcceptedNotLanded,
}

@Serializable
enum class TrackRecordStatus {
    @SerialName("confirmed") Confirmed,

    @SerialName("failed") Failed,
}

@Serializable
data class TrackRecordPatch(val status: TrackRecordStatus, val tx_hash: String? = null)

@Serializable
data class TrackPendingRecord(
    val record_id: String,
    val user_op_hash: String,
    val chain_id: Int,
    val submitted_at_ms: Double,
)

@Serializable
data class TrackEntryView(
    val user_op_hash: String,
    val chain_id: Int,
    val record_ids: List<String> = emptyList(),
    val status: TrackStatus,
    val tx_hash: String? = null,
    val polling: Boolean = false,
    val submitted_at_ms: Double? = null,
)

@Serializable
data class TrackView(val entries: List<TrackEntryView> = emptyList())

// -- what the machine asks for -----------------------------------------------

@Serializable
sealed class TrackOperation {
    @Serializable
    @SerialName("poll_receipt")
    data class PollReceipt(val user_op_hash: String, val chain_id: Int) : TrackOperation()

    @Serializable
    @SerialName("poll_status")
    data class PollStatus(val user_op_hash: String, val chain_id: Int) : TrackOperation()

    @Serializable
    @SerialName("load_pending_txs")
    data object LoadPendingTxs : TrackOperation()

    @Serializable
    @SerialName("update_tx_records")
    data class UpdateTxRecords(val ids: List<String>, val patch: TrackRecordPatch) : TrackOperation()

    @Serializable
    @SerialName("notify_confirmed")
    data class NotifyConfirmed(
        val user_op_hash: String,
        val chain_id: Int,
        val tx_hash: String,
    ) : TrackOperation()

    @Serializable
    @SerialName("now")
    data object Now : TrackOperation()
}

// -- what the shell observed -------------------------------------------------

@Serializable
sealed class TrackShellResult {
    @Serializable
    @SerialName("clock")
    data class Clock(val now_ms: Double) : TrackShellResult()

    @Serializable
    @SerialName("receipt")
    data class Receipt(
        val user_op_hash: String,
        val tx_hash: String,
        val now_ms: Double,
    ) : TrackShellResult()

    /** The receipt AND its logs — the ones `token_trust` will read at `notify_confirmed`. */
    @Serializable
    @SerialName("receipt_with_logs")
    data class ReceiptWithLogs(
        val user_op_hash: String,
        val tx_hash: String,
        val now_ms: Double,
        val logs: List<TrustReceiptLog> = emptyList(),
    ) : TrackShellResult()

    @Serializable
    @SerialName("receipt_failed")
    data class ReceiptFailed(
        val user_op_hash: String,
        val tx_hash: String,
        val now_ms: Double,
    ) : TrackShellResult()

    @Serializable
    @SerialName("receipt_pending")
    data class ReceiptPending(val user_op_hash: String, val now_ms: Double) : TrackShellResult()

    /** The relay could not be asked — not "not yet", and the core keeps the difference. */
    @Serializable
    @SerialName("receipt_unreachable")
    data class ReceiptUnreachable(val user_op_hash: String, val now_ms: Double) : TrackShellResult()

    @Serializable
    @SerialName("status")
    data class Status(
        val user_op_hash: String,
        val status: TrackLifecycle,
        val stage: String? = null,
        val now_ms: Double,
    ) : TrackShellResult()

    @Serializable
    @SerialName("status_unavailable")
    data class StatusUnavailable(val user_op_hash: String, val now_ms: Double) : TrackShellResult()

    @Serializable
    @SerialName("records_loaded")
    data class RecordsLoaded(
        val records: List<TrackPendingRecord> = emptyList(),
        val now_ms: Double,
    ) : TrackShellResult()

    @Serializable
    @SerialName("records_patched")
    data object RecordsPatched : TrackShellResult()

    @Serializable
    @SerialName("notified")
    data object Notified : TrackShellResult()
}

// -- what the shell tells it -------------------------------------------------

@Serializable
sealed class TrackEvent {
    /** From the send machine's `track_submitted`, after the records are on disk. */
    @Serializable
    @SerialName("submitted")
    data class Submitted(
        val user_op_hash: String,
        val record_ids: List<String>,
        val chain_id: Int,
    ) : TrackEvent()

    @Serializable
    @SerialName("tick")
    data object Tick : TrackEvent()

    @Serializable
    @SerialName("app_resumed")
    data object AppResumed : TrackEvent()

    @Serializable
    @SerialName("home_focused")
    data object HomeFocused : TrackEvent()

    @Serializable
    @SerialName("abort")
    data class Abort(val user_op_hash: String) : TrackEvent()
}
