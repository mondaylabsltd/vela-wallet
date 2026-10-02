//
//  TrackerExecutor.swift
//  VelaWallet
//
//  The only place the `tx_tracker` core touches the outside world.
//
//  Eight operations, ported from `app-android/.../feature/send/core/
//  TrackerExecutor.kt` (spec 043). Every cadence decision is the core's — how
//  often to poll, how long to wait, when to give up — and this supplies the
//  clock, the transport and the notification.
//
//  ## Spec 082: an op whose submit reply was lost is followed to its end
//
//  `find_op_event` reads the EntryPoint's `UserOperationEvent` for it through
//  the pool and answers what came back — logs, or the endpoint's error, and
//  the head — without judging any of it (ruling 8, T180). `holdings_moved`
//  re-reads the balance after an op of ours landed (RE8, G26). And a pending
//  row carries `maybeSent` / `submitBlock` back into the core on a relaunch
//  (T183), so a restart does not forget what the submit could not prove.
//
//  ## A relay that names the tx is confirmed through the chain (spec 082 RJ4)
//
//  When the relay's status names the bundle's transaction, the core asks for
//  `tx_receipt`: `eth_getTransactionReceipt` through the chain pool, whose
//  `result` goes back AS IT CAME — `null` is "not mined", no answer is `nil`
//  — and the core finds the op's own event in its logs (EX13: a landed op
//  read "not on chain yet" for six minutes while the relay said `included`).
//  Nothing here reads the receipt, and no token is admitted from it: its
//  logs are the whole bundle's.
//
//  ## The receipt's logs are the ONLY way a token is ever admitted
//
//  `notify_confirmed` hands the logs this executor just polled to
//  `token_trust::ReceiptLogsConfirmed`. That is the founder's standing ruling:
//  a token is admitted from a **confirm-time receipt**, never from a sign-time
//  simulation, because a simulation is something a site can author and a
//  receipt is not.
//

import Foundation
import VelaCore

@MainActor
final class TrackerExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "poll_receipt", "poll_status", "load_pending_txs",
        "update_tx_records", "notify_confirmed", "holdings_moved", "find_op_event", "now",
        "tx_receipt",
    ]

    /// What the app owns: the notification, and the auto-add hand-off.
    struct Ports {
        var notifyConfirmed: (_ userOpHash: String, _ chainId: Int, _ txHash: String) -> Void = { _, _, _ in }
        /// The authentic logs, on their way to `token_trust`.
        var receiptLogs: (_ from: String, _ chainId: Int, _ logs: [[String: Any]]) -> Void = { _, _, _ in }
        /// Records changed on disk — the feed re-reads.
        var recordsPatched: () -> Void = {}
        /// An op of ours landed on this chain (confirmed, or failed with gas
        /// spent): the balance is read again, without a pull (RE8).
        var holdingsMoved: (_ chainId: Int) -> Void = { _ in }
    }

    private let store: VelaStore
    private let relay: RelayClient
    var ports: Ports

    /// The logs of a receipt already polled, kept until `notify_confirmed`
    /// asks for them. The core routes the two as separate operations, and
    /// re-polling would be a second round trip for an answer already in hand.
    private var logsByHash: [String: (sender: String, logs: [[String: Any]])] = [:]

    init(store: VelaStore, relay: RelayClient, ports: Ports = Ports()) {
        self.store = store
        self.relay = relay
        self.ports = ports
    }

    func perform(_ operation: [String: Any]) async -> String {
        let hash = operation["user_op_hash"] as? String ?? ""
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0

        switch operation["type"] as? String ?? "" {

        case "poll_receipt":
            return await pollReceipt(hash: hash, chainId: chainId)

        case "poll_status":
            guard let answer = await relay.userOpStatus(chainId: chainId, userOpHash: hash) else {
                // An older relay, or one that could not be asked. Not a verdict.
                VelaLog.notice(.tracker, "op=\(VelaLog.short(hash)) status unavailable")
                return CoreJSON.string([
                    "type": "status_unavailable", "user_op_hash": hash, "now_ms": Self.nowMs,
                ])
            }
            return CoreJSON.string([
                "type": "status",
                "user_op_hash": hash,
                "status": answer.status,
                "stage": answer.stage.map { $0 as Any } ?? NSNull(),
                "now_ms": Self.nowMs,
                // The bundle the relay names while no receipt has — a link
                // for the explorer, never a verdict (RA7).
                "tx_hash": answer.txHash.map { $0 as Any } ?? NSNull(),
            ])

        case "find_op_event":
            let found = await relay.findOpEvent(
                chainId: chainId,
                entryPoint: operation["entry_point"] as? String ?? "",
                topic0: operation["topic0"] as? String ?? "",
                userOpHash: hash,
                fromBlock: (operation["from_block"] as? NSNumber)?.uint64Value,
                toBlock: (operation["to_block"] as? NSNumber)?.uint64Value
            )
            return CoreJSON.string([
                "type": "op_event",
                "user_op_hash": hash,
                "now_ms": Self.nowMs,
                "logs_json": found.logsJson.map { $0 as Any } ?? NSNull(),
                "error_json": found.errorJson.map { $0 as Any } ?? NSNull(),
                "head_block": found.head.map { $0 as Any } ?? NSNull(),
            ])

        case "tx_receipt":
            let txHash = operation["tx_hash"] as? String ?? ""
            let receipt = await relay.transactionReceiptJson(chainId: chainId, txHash: txHash)
            VelaLog.notice(
                .tracker,
                "tx receipt op=\(VelaLog.short(hash)) tx=\(VelaLog.short(txHash)) chain=\(chainId) answer=\(receipt == nil ? "none" : receipt == "null" ? "not_mined" : "receipt")"
            )
            return CoreJSON.string([
                "type": "tx_receipt", "user_op_hash": hash, "now_ms": Self.nowMs,
                "receipt_json": receipt.map { $0 as Any } ?? NSNull(),
            ])

        case "holdings_moved":
            VelaLog.notice(.balance, "balance refresh (holdings_moved) chain=\(chainId)")
            ports.holdingsMoved(chainId)
            return CoreJSON.string(["type": "notified"])

        case "load_pending_txs":
            // Derived from the store, which is why a force-quit loses nothing.
            let records = TxRecords.pending(store: store).compactMap(Self.pendingWire)
            return CoreJSON.string([
                "type": "records_loaded", "records": records, "now_ms": Self.nowMs,
            ])

        case "update_tx_records":
            let ids = operation["ids"] as? [String] ?? []
            let patch = operation["patch"] as? [String: Any] ?? [:]
            var fields: [String: Any] = ["status": patch["status"] as? String ?? "confirmed"]
            if let txHash = patch["tx_hash"] as? String, !txHash.isEmpty {
                fields["txHash"] = txHash
            }
            // Spec 097: how it ended — what its receipt proved it moved, or
            // why it failed — kept beside the status, verbatim, for the feed.
            if let settlement = patch["settlement"] as? [String: Any] {
                fields["settlement"] = settlement
            }
            TxRecords.patch(ids: ids, fields: fields, store: store)
            ports.recordsPatched()
            return CoreJSON.string(["type": "records_patched"])

        case "notify_confirmed":
            let txHash = operation["tx_hash"] as? String ?? ""
            VelaLog.notice(.tracker, "tracker confirmed op=\(VelaLog.short(hash)) chain=\(chainId)")
            ports.notifyConfirmed(hash, chainId, txHash)
            // The authentic logs, to the one entry point that may admit a
            // token. Nothing else in the app is allowed to. A confirmation
            // the chain's own event found (ruling 8) has no receipt behind it
            // and nothing held here: no token is admitted from it.
            if let held = logsByHash.removeValue(forKey: hash), !held.logs.isEmpty {
                ports.receiptLogs(held.sender, chainId, held.logs)
            }
            return CoreJSON.string(["type": "notified"])

        case "now":
            return CoreJSON.string(["type": "clock", "now_ms": Self.nowMs])

        default:
            VelaLog.failure(.tracker, kind: "unhandled_operation", "\(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    /// The core's own mapping, spelled out in its doc and not re-decided here:
    /// an RPC error or a throw is **unreachable**; no result or no transaction
    /// hash is **pending**; `success != false` is a receipt; `success == false`
    /// is a failure.
    private func pollReceipt(hash: String, chainId: Int) async -> String {
        switch await relay.userOpReceipt(chainId: chainId, userOpHash: hash) {
        case .unreachable:
            return CoreJSON.string([
                "type": "receipt_unreachable", "user_op_hash": hash, "now_ms": Self.nowMs,
            ])
        case .pending:
            return CoreJSON.string([
                "type": "receipt_pending", "user_op_hash": hash, "now_ms": Self.nowMs,
            ])
        case .resolved(let confirmed, let txHash, let sender, let logs):
            guard confirmed else {
                return CoreJSON.string([
                    "type": "receipt_failed", "user_op_hash": hash,
                    "tx_hash": txHash, "now_ms": Self.nowMs,
                ])
            }
            logsByHash[hash] = (sender ?? "", logs)
            // `receipt_with_logs` rather than `receipt`: the core reads the
            // logs for an `ExecutionFailure` inside an operation the
            // EntryPoint counted as a success — the money did not move and the
            // record must say `failed`, not `confirmed`.
            return CoreJSON.string([
                "type": "receipt_with_logs", "user_op_hash": hash,
                "tx_hash": txHash, "now_ms": Self.nowMs, "logs": logs,
            ])
        }
    }

    /// One stored row as `TrackPendingRecord` — with what the submit could
    /// not prove (spec 082 T183): a may-have-been-sent op, and the head read
    /// before its POST, go back into the core as they were written, so a
    /// relaunch follows it as one. A row from before 082 carries neither and
    /// reads `false` / absent.
    static func pendingWire(_ record: [String: Any]) -> [String: Any]? {
        guard let id = record["id"] as? String, !id.isEmpty,
              let hash = record["userOpHash"] as? String, !hash.isEmpty
        else { return nil }
        var wire: [String: Any] = [
            "record_id": id,
            "user_op_hash": hash,
            "chain_id": (record["chainId"] as? NSNumber)?.intValue ?? 0,
            // Stored in SECONDS; the core counts in milliseconds.
            "submitted_at_ms": ((record["timestamp"] as? NSNumber)?.doubleValue ?? 0) * 1000,
            "maybe_sent": record["maybeSent"] as? Bool ?? false,
        ]
        if let block = record["submitBlock"] as? NSNumber { wire["submit_block"] = block.uint64Value }
        return wire
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        let hash = operation["user_op_hash"] as? String ?? ""
        switch operation["type"] as? String ?? "" {
        case "poll_receipt":
            return CoreJSON.string([
                "type": "receipt_unreachable", "user_op_hash": hash, "now_ms": nowMs,
            ])
        case "poll_status":
            return CoreJSON.string([
                "type": "status_unavailable", "user_op_hash": hash, "now_ms": nowMs,
            ])
        case "load_pending_txs":
            return CoreJSON.string(["type": "records_loaded", "records": [], "now_ms": nowMs])
        case "update_tx_records":
            return CoreJSON.string(["type": "records_patched"])
        case "notify_confirmed", "holdings_moved":
            return CoreJSON.string(["type": "notified"])
        case "find_op_event":
            // No answer: the core asks the same window again next tick.
            return CoreJSON.string([
                "type": "op_event", "user_op_hash": hash, "now_ms": nowMs,
                "logs_json": NSNull(), "error_json": NSNull(), "head_block": NSNull(),
            ])
        case "tx_receipt":
            // No answer: asked again at the receipt cadence.
            return CoreJSON.string([
                "type": "tx_receipt", "user_op_hash": hash, "now_ms": nowMs, "receipt_json": NSNull(),
            ])
        default:
            return CoreJSON.string(["type": "clock", "now_ms": nowMs])
        }
    }

    private static var nowMs: Double { Date().timeIntervalSince1970 * 1000 }
}
