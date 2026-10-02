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

        /** Spec 082 RE8: an op of ours landed on [chainId] — re-read the balances. */
        fun holdingsMoved(chainId: Int) {}
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
                TrackShellResult.Status(operation.user_op_hash, answer.status, answer.stage, now(), answer.txHash)
            }
        }
        TrackOperation.LoadPendingTxs -> TrackShellResult.RecordsLoaded(
            records = feed.pendingRecords().mapNotNull(::pendingRecord),
            now_ms = now(),
        )
        // Ruling 8: the pool's answer as it came — the core judges a range
        // error, never this file.
        is TrackOperation.FindOpEvent -> {
            val answer = relay.findOpEvent(
                chainId = operation.chain_id,
                entryPoint = operation.entry_point,
                topic0 = operation.topic0,
                userOpHash = operation.user_op_hash,
                fromBlock = operation.from_block,
                toBlock = operation.to_block,
            )
            TrackShellResult.OpEvent(
                user_op_hash = operation.user_op_hash,
                now_ms = now(),
                logs_json = answer.logsJson,
                error_json = answer.errorJson,
                head_block = answer.headBlock,
            )
        }
        is TrackOperation.HoldingsMoved -> {
            VelaLog.event("tracker.holdings", "moved", "chain" to operation.chain_id)
            ports.holdingsMoved(operation.chain_id)
            TrackShellResult.Notified
        }
        is TrackOperation.UpdateTxRecords -> {
            val status = when (operation.patch.status) {
                TrackRecordStatus.Confirmed -> "confirmed"
                TrackRecordStatus.Failed -> "failed"
            }
            feed.patchRecords(operation.ids, status, operation.patch.tx_hash, operation.patch.settlement)
            VelaLog.event("tracker.patch", status, "ids" to operation.ids.size, "tx" to operation.patch.tx_hash?.take(12))
            ports.recordsPatched(operation.ids, operation.patch.status, operation.patch.tx_hash)
            TrackShellResult.RecordsPatched
        }
        is TrackOperation.NotifyConfirmed -> {
            ports.notifyConfirmed(operation.user_op_hash, operation.chain_id, operation.tx_hash)
            // A confirmation found by its chain event (ruling 8) has no receipt
            // behind it, so no logs reach the trust machine: token auto-add
            // stays with authentic receipt logs only.
            synchronized(receipts) { receipts.remove(operation.user_op_hash.lowercase()) }?.let { receipt ->
                val sender = receipt.sender ?: senderOf(operation.user_op_hash)
                if (sender != null && receipt.logs.isNotEmpty()) {
                    ports.receiptLogsConfirmed(sender, operation.chain_id, receipt.logs)
                }
            }
            TrackShellResult.Notified
        }
        TrackOperation.Now -> TrackShellResult.Clock(now())
        // Spec 082 RJ4: the relay named the bundle tx — its receipt through
        // the chain pool, as it came; the core finds the op's own event in it.
        is TrackOperation.TxReceipt -> {
            val receipt = relay.txReceipt(operation.chain_id, operation.tx_hash)
            VelaLog.event(
                "tracker.receipt", "by tx",
                "op" to operation.user_op_hash.take(12), "tx" to operation.tx_hash.take(12), "chain" to operation.chain_id,
                "answer" to when (receipt) {
                    null -> "no answer"
                    "null" -> "not mined"
                    else -> "mined"
                },
            )
            TrackShellResult.TxReceipt(operation.user_op_hash, now(), receipt)
        }
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
        is TrackOperation.HoldingsMoved -> TrackShellResult.Notified
        // No answer: the core asks the same window again next tick.
        is TrackOperation.FindOpEvent -> TrackShellResult.OpEvent(operation.user_op_hash, now())
        TrackOperation.Now -> TrackShellResult.Clock(now())
        // No answer: the core asks again at the receipt cadence.
        is TrackOperation.TxReceipt -> TrackShellResult.TxReceipt(operation.user_op_hash, now(), null)
    }

    companion object {
        /**
         * A stored row as the tracker's pending record (spec 082 T184): the
         * two flags the submit wrote with it come back, so a restart keeps a
         * may-have-been-sent op following as one and its landing check starts
         * where the submit did. A row stored before 082 has neither: `false`
         * and unknown.
         */
        fun pendingRecord(row: org.json.JSONObject): TrackPendingRecord? {
            val hash = row.optString("userOpHash").ifBlank { return null }
            return TrackPendingRecord(
                record_id = row.optString("id").ifBlank { hash },
                user_op_hash = hash,
                chain_id = row.optInt("chainId"),
                // The feed keeps seconds; the tracker's clock is milliseconds.
                submitted_at_ms = row.optDouble("timestamp", 0.0) * 1000.0,
                maybe_sent = row.optBoolean("maybeSent", false),
                submit_block = if (row.has("submitBlock") && !row.isNull("submitBlock")) {
                    row.optLong("submitBlock", -1L).takeIf { it >= 0 }
                } else {
                    null
                },
            )
        }
    }
}
