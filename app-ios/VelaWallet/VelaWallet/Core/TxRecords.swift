//
//  TxRecords.swift
//  VelaWallet
//
//  `vela.transactionHistory` — the local transaction store.
//
//  Ported from `app-web/vela-wallet/src/lib/services/records.ts` (the tx half)
//  and `transactions-model.ts`. These are the **bytes every client shares**
//  (FR-005): a record written on the web reads on the phone, so the field names
//  are camelCase and the cap is the same 200.
//
//  ## The store is the source of truth, even about emptiness
//
//  `activity_feed.rs` says so, and web's `loadTransactions().catch(() => [])`
//  means it literally: an unreadable store answers an EMPTY feed rather than a
//  failure. A machine told the read failed would stall holding the home screen;
//  a machine told the store is empty renders an empty feed and reads again on
//  the next tick.
//
//  ## `withTxLock`, satisfied structurally
//
//  Web queues its read-modify-writes behind one promise chain. Here every
//  writer is `@MainActor` and no `await` sits between the read and the write,
//  so two writers cannot interleave — the lock is the actor, not a queue.
//

import Foundation

enum TxRecords {

    /// The newest 200, the Expo cap every client applies.
    static let cap = 200

    /// Every stored record, newest first as written.
    @MainActor
    static func load(store: VelaStore) -> [[String: Any]] {
        store.readList(VelaStore.Key.transactionHistory)
    }

    /// Merge new records in: de-duped by id, newest first, capped.
    ///
    /// Answers **how many were actually new** — the count the core's
    /// celebration is built on, so a repeat scan of the same window must
    /// answer zero rather than re-celebrating a receipt somebody already saw.
    @MainActor
    @discardableResult
    static func merge(_ incoming: [[String: Any]], store: VelaStore) -> Int {
        guard !incoming.isEmpty else { return 0 }
        let existing = load(store: store)
        let ids = Set(existing.compactMap { $0["id"] as? String })
        let fresh = incoming.filter { record in
            guard let id = record["id"] as? String else { return false }
            return !ids.contains(id)
        }
        guard !fresh.isEmpty else { return 0 }

        var merged = fresh + existing
        merged.sort { timestamp($0) > timestamp($1) }
        if merged.count > cap { merged = Array(merged.prefix(cap)) }
        store.writeList(VelaStore.Key.transactionHistory, merged)
        return fresh.count
    }

    /// Remove one record. A missing id writes nothing — the core's optimistic
    /// delete already removed the row, and rewriting an identical file would
    /// only risk losing a concurrent write.
    @MainActor
    static func delete(id: String, store: VelaStore) {
        let existing = load(store: store)
        let next = existing.filter { ($0["id"] as? String) != id }
        guard next.count != existing.count else { return }
        store.writeList(VelaStore.Key.transactionHistory, next)
    }

    static func timestamp(_ record: [String: Any]) -> Double {
        (record["timestamp"] as? NSNumber)?.doubleValue ?? 0
    }

    // MARK: - The stored shape → the core's vocabulary

    /// The six `type` values the feed can speak about.
    static let kinds = ["send", "receive", "dapp_tx", "sign_message", "sign_typed_data", "connect"]
    static let statuses = ["pending", "confirmed", "failed"]

    /// One stored record as `FeedTxRecord`, or `nil` when it is not a record
    /// this machine can speak about.
    ///
    /// A `type` that is present but unknown is **dropped rather than guessed
    /// at**: it can be neither a feed item nor a transfer today, and inventing
    /// a kind for it would be a lie the core would act on. Numbers are coerced
    /// fail-closed for the same reason — the store is an unvalidated JSON
    /// parse, and a field serde could not accept would fault the core into a
    /// feed that never loads.
    static func toWire(_ record: [String: Any]) -> [String: Any]? {
        let rawKind = record["type"] as? String
        if let rawKind, !kinds.contains(rawKind) { return nil }
        let timestamp = timestamp(record)
        let status = record["status"] as? String
        return [
            "id": string(record["id"]),
            "user_op_hash": string(record["userOpHash"]),
            "tx_hash": string(record["txHash"]),
            "from": string(record["from"]),
            "to": string(record["to"]),
            "to_name": (record["toName"] as? String).map { $0 as Any } ?? NSNull(),
            "value": string(record["value"]),
            "symbol": string(record["symbol"]),
            "decimals": count(record["decimals"]),
            "logo_urls": (record["logoUrls"] as? [Any])
                .map { $0.compactMap { $0 as? String } as Any } ?? NSNull(),
            "chain_id": count(record["chainId"]),
            "timestamp": timestamp,
            // The grouping key the core cannot compute: the device's timezone
            // is the shell's. A record written at 23:30 local heads its own day.
            "day_start_ms": dayStartMs(timestamp),
            "status": statuses.contains(status ?? "") ? status! : "confirmed",
            "kind": rawKind.map { $0 as Any } ?? NSNull(),
            "usd": (record["usd"] as? String).map { $0 as Any } ?? NSNull(),
        ]
    }

    /// Local midnight for a unix-seconds timestamp, in epoch ms.
    static func dayStartMs(_ seconds: Double) -> Double {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        let date = Date(timeIntervalSince1970: seconds)
        let midnight = calendar.startOfDay(for: date)
        return midnight.timeIntervalSince1970 * 1000
    }

    private static func string(_ value: Any?) -> String { value as? String ?? "" }

    /// A non-negative integer, or 0 — a `u32` field serde must be able to take.
    private static func count(_ value: Any?) -> Int {
        guard let number = value as? NSNumber else { return 0 }
        let value = number.intValue
        return value >= 0 ? value : 0
    }
}
