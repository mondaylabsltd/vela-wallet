package app.getvela.wallet.feature.send.core

import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.TrustReceiptLog

/**
 * The `tx_tracker` machine's six arms (spec 043 T035).
 *
 * The core owns the cadence — receipt every 3 s, status every 12 s, a
 * two-minute wait window, abandon at a day — and decides when a hash is
 * confirmed, dropped, rejected or held. This file supplies a clock, two relay
 * calls, the feed's rows, and the two things the core asks the platform for
 * when a receipt lands: the notification, and the receipt's logs handed to
 * the trust machine (which is what lets a swap's output token appear).
 */
class TrackerExecutor(
    private val relay: RelayClient,
    private val feed: FeedExecutor,
    private val ports: TrackerPorts,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {

    /** What the platform owns: the notification and the trust machine's ear. */
    interface TrackerPorts {
        /** A verdict landed on these rows; the feed re-reads. */
        fun recordsPatched(ids: List<String>, status: TrackRecordStatus, txHash: String?)

        /** Notify the person (only if the app is away — the port decides). */
        fun notifyConfirmed(userOpHash: String, chainId: Int, txHash: String)

        /** The confirmed receipt's logs, for `token_trust::ReceiptLogsConfirmed`. */
        fun receiptLogsConfirmed(from: String, chainId: Int, logs: List<TrustReceiptLog>)
    }

    /** Logs by hash, from the receipt that resolved it, until `notify_confirmed` takes them. */
    private val receipts = HashMap<String, RelayClient.ReceiptAnswer.Resolved>()

    suspend fun perform(operation: TrackOperation): TrackShellResult = when (operation) {
        is TrackOperation.PollReceipt -> pollReceipt(operation.user_op_hash, operation.chain_id)
        is TrackOperation.PollStatus -> {
            val answer = relay.userOpStatus(operation.chain_id, operation.user_op_hash)
            if (answer == null) {
                TrackShellResult.StatusUnavailable(operation.user_op_hash, now())
            } else {
                TrackShellResult.Status(operation.user_op_hash, answer.first, answer.second, now())
            }
        }
        TrackOperation.LoadPendingTxs -> TrackShellResult.RecordsLoaded(
            records = feed.pendingRecords().mapNotNull { row ->
                val hash = row.optString("userOpHash").ifBlank { return@mapNotNull null }
                TrackPendingRecord(
                    record_id = row.optString("id").ifBlank { hash },
                    user_op_hash = hash,
                    chain_id = row.optInt("chainId"),
                    // The feed keeps seconds; the tracker's clock is milliseconds.
                    submitted_at_ms = row.optDouble("timestamp", 0.0) * 1000.0,
                )
            },
            now_ms = now(),
        )
        is TrackOperation.UpdateTxRecords -> {
            val status = when (operation.patch.status) {
                TrackRecordStatus.Confirmed -> "confirmed"
                TrackRecordStatus.Failed -> "failed"
            }
            feed.patchRecords(operation.ids, status, operation.patch.tx_hash)
            VelaLog.event("tracker.patch", status, "ids" to operation.ids.size, "tx" to operation.patch.tx_hash?.take(12))
            ports.recordsPatched(operation.ids, operation.patch.status, operation.patch.tx_hash)
            TrackShellResult.RecordsPatched
        }
        is TrackOperation.NotifyConfirmed -> {
            ports.notifyConfirmed(operation.user_op_hash, operation.chain_id, operation.tx_hash)
            synchronized(receipts) { receipts.remove(operation.user_op_hash.lowercase()) }?.let { receipt ->
                val sender = receipt.sender ?: senderOf(operation.user_op_hash)
                if (sender != null && receipt.logs.isNotEmpty()) {
                    ports.receiptLogsConfirmed(sender, operation.chain_id, receipt.logs)
                }
            }
            TrackShellResult.Notified
        }
        TrackOperation.Now -> TrackShellResult.Clock(now())
    }

    private suspend fun pollReceipt(hash: String, chainId: Int): TrackShellResult = when (val answer = relay.userOpReceipt(chainId, hash)) {
        RelayClient.ReceiptAnswer.Unreachable -> TrackShellResult.ReceiptUnreachable(hash, now())
        RelayClient.ReceiptAnswer.Pending -> TrackShellResult.ReceiptPending(hash, now())
        is RelayClient.ReceiptAnswer.Resolved -> {
            synchronized(receipts) { receipts[hash.lowercase()] = answer }
            when {
                !answer.confirmed -> TrackShellResult.ReceiptFailed(hash, answer.txHash, now())
                answer.logs.isNotEmpty() -> TrackShellResult.ReceiptWithLogs(hash, answer.txHash, now(), answer.logs)
                else -> TrackShellResult.Receipt(hash, answer.txHash, now())
            }
        }
    }

    /** The sender of a stored send, for a receipt that did not name one. */
    private suspend fun senderOf(hash: String): String? =
        feed.pendingRecords().firstOrNull { it.optString("userOpHash").equals(hash, ignoreCase = true) }
            ?.optString("from")?.ifBlank { null }

    /** What the core hears when an arm threw: nothing could be read. */
    fun neutralAnswer(operation: TrackOperation): TrackShellResult = when (operation) {
        is TrackOperation.PollReceipt -> TrackShellResult.ReceiptUnreachable(operation.user_op_hash, now())
        is TrackOperation.PollStatus -> TrackShellResult.StatusUnavailable(operation.user_op_hash, now())
        TrackOperation.LoadPendingTxs -> TrackShellResult.RecordsLoaded(emptyList(), now())
        is TrackOperation.UpdateTxRecords -> TrackShellResult.RecordsPatched
        is TrackOperation.NotifyConfirmed -> TrackShellResult.Notified
        TrackOperation.Now -> TrackShellResult.Clock(now())
    }
}
