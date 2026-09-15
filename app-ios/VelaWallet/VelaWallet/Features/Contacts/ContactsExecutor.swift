//
//  ContactsExecutor.swift
//  VelaWallet
//
//  The only place the `contacts` core touches the outside world on iOS.
//
//  Ported from `app-web/vela-wallet/src/lib/contacts/core/contacts-executor.ts`
//  (spec 024), which was itself ported from the Expo services. Same seven
//  operations, same stored bytes, same failure answers — a record written by
//  any client reads on every other, which is the whole point of porting rather
//  than rewriting.
//
//  ## No business `if`
//
//  Dedup, ordering, tombstone semantics, favourites-first, the
//  history-derived merge, what counts as a valid address — all of it is decided
//  and tested in Rust. This file translates shapes and answers questions.
//
//  ## Wire vs stored: two alphabets, and mixing them is the bug
//
//  The core speaks snake_case (`resolved_name`, `last_used_ms`). The three
//  storage keys hold the camelCase records every other client writes
//  (`resolvedName`, `lastUsed`). Translating is this file's job.
//
//  Two shape rules are load-bearing and easy to get wrong:
//
//  1. **`vela.contacts.dismissed` is an OBJECT**, `address → epoch ms`, while
//     the wire is a list of tombstones. Writing it as a list makes every other
//     client's tombstones vanish and deleted contacts come back.
//  2. **Absent optionals are OMITTED, never written as `null`.** The stored
//     JSON must stay identical to what the Expo services produce.
//
//  ## Failure contract
//
//  Nothing throws out of here. Every operation is answered exactly once, with
//  the variant the core models — chosen for what it makes the core do NEXT.
//  `history_failed` beats an empty list because an empty list is a claim ("this
//  person has never sent to anyone") while the failure is the truth ("I could
//  not look").
//

import Foundation

@MainActor
final class ContactsExecutor {

    /// Every operation this executor is required to handle
    /// (contracts/shell-operations.md).
    static let operations = [
        "read_store",
        "write_contacts",
        "write_dismissed",
        "write_groups",
        "load_send_history",
        "resolve_identity",
        "classify_recipient",
    ]

    private let store: VelaStore
    /// The name waterfall. `nil` keeps the fail-closed answers — which is the
    /// state every hermetic test drives, and a real one on a device with no
    /// network.
    private let identity: RecipientIdentity?
    /// The routing pool, for `eth_getCode`. Same rule: absent means unknown,
    /// never a verdict.
    private let pool: RpcPool?

    init(store: VelaStore, identity: RecipientIdentity? = nil, pool: RpcPool? = nil) {
        self.store = store
        self.identity = identity
        self.pool = pool
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "read_store":
            return CoreJSON.string([
                "type": "store_loaded",
                "contacts": store.readList(VelaStore.Key.contacts).map(Self.contactToWire),
                "tombstones": Self.tombstonesToWire(store.readObject(VelaStore.Key.contactsDismissed)),
                "groups": store.readList(VelaStore.Key.contactGroups).map(Self.groupToWire),
            ])

        // The three writes are best effort by contract: the core's in-memory
        // ledger is authoritative, and a storage failure it cannot undo must
        // not stall the machine mid-mutation.
        case "write_contacts":
            let contacts = operation["contacts"] as? [[String: Any]] ?? []
            store.writeList(VelaStore.Key.contacts, contacts.map(Self.contactToStored))
            return Self.written

        case "write_dismissed":
            let tombstones = operation["tombstones"] as? [[String: Any]] ?? []
            store.writeObject(VelaStore.Key.contactsDismissed, Self.tombstonesToStored(tombstones))
            return Self.written

        case "write_groups":
            let groups = operation["groups"] as? [[String: Any]] ?? []
            store.writeList(VelaStore.Key.contactGroups, groups.map(Self.groupToStored))
            return Self.written

        // ---------------------------------------------------------------
        // Fail-closed until the infrastructure exists. Each answers the
        // core's own unknown variant — never an invented value.
        // ---------------------------------------------------------------

        // The local send history — which is where the book's auto-suggested
        // contacts come from, and how the core knows an address is not a first
        // interaction.
        //
        // **Only `send` rows.** A `dapp_tx` reaches a router, a token contract
        // or a dApp, and counting those as people would pollute the very trust
        // signal the address-poisoning warning rests on (`contacts.rs`'s module
        // doc says so, and the web narrows the same way).
        //
        // An unreadable store still answers `history_failed` rather than an
        // empty list: empty would tell the core nobody has ever been paid, and
        // then every address wears the warning forever. `TxRecords.load`
        // answers an empty list for a corrupt store, so the two cases are
        // separated by whether the KEY exists at all.
        case "load_send_history":
            guard let txs = Self.sendHistory(store: store) else {
                return CoreJSON.string(["type": "history_failed"])
            }
            return CoreJSON.string(["type": "history_loaded", "txs": txs])

        // The name waterfall: the passkey index, then the on-chain name
        // services. `null` is "nobody could name them" and is never cached —
        // an absence today is not a fact about the address (invariant ⑦).
        case "resolve_identity":
            let address = operation["address"] as? String ?? ""
            let found = await identity?.resolve(address)
            return CoreJSON.string([
                "type": "identity_resolved",
                "address": address,
                "identity": found.map { ["name": $0.name, "source": $0.source] as Any }
                    ?? NSNull(),
            ])

        // `eth_getCode`, routed. The RAW code goes back: the core owns both
        // projections (what kind of contact this is, and the risk badge).
        //
        // `code: null` stays UNKNOWN. An empty string means "definitely an
        // EOA", which the core shows as a settled classification — so a failed
        // read must never become one.
        case "classify_recipient":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let address = operation["address"] as? String ?? ""
            var code: Any = NSNull()
            if let pool {
                let outcome = await pool.call(
                    chainId: chainId, method: "eth_getCode", params: [address, "latest"]
                )
                if case .ok(let value) = outcome, let hex = value as? String { code = hex }
            }
            return CoreJSON.string([
                "type": "recipient_classified",
                "chain_id": chainId,
                "address": address,
                "code": code,
            ])

        default:
            // An operation this build does not know. Swift cannot check a JSON
            // tag for exhaustiveness the way Rust checks a `match`, so a
            // machine that grew an operation would otherwise hang here forever
            // with no diagnostic at all.
            //
            // Logged rather than trapped. `assertionFailure` would be louder,
            // and louder is wrong here: it is a no-op in Release, so the ONLY
            // builds it affects are the debug ones run on a real device — it
            // would turn a recoverable wire mismatch into a crash for exactly
            // the person best placed to report it.
            //
            // `store_loaded` with empty stores is the answer that leaves the
            // core in a state a person can still use — loaded, with nothing in
            // it — rather than stalled before its first render.
            print("[vela-wallet] contacts: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string([
                "type": "store_loaded",
                "contacts": [], "tombstones": [], "groups": [],
            ])
        }
    }

    private static let written = CoreJSON.string(["type": "written"])
}

// MARK: - Stored ⇄ wire

extension ContactsExecutor {

    /// One stored contact → the core's `Contact`.
    ///
    /// Coercion, not policy. Junk is flattened to the empty/zero value rather
    /// than rejected: serde refuses a mistyped record, and a refused
    /// `store_loaded` strands the machine unloaded forever, silently dropping
    /// every write that follows.
    static func contactToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "address": (stored["address"] as? String ?? "").lowercased(),
            "name": text(stored["name"]),
            "resolved_name": text(stored["resolvedName"]),
            "resolved_source": text(stored["resolvedSource"]),
            "kind": kind(stored["kind"]),
            "favorite": stored["favorite"] as? Bool ?? false,
            "note": text(stored["note"]),
            // `u32` on the wire — sent as an Int so a fractional stored value
            // cannot arrive as `3.5` and be refused by serde.
            "tx_count": max(0, Int(number(stored["txCount"]).rounded(.towardZero))),
            "last_used_ms": number(stored["lastUsed"]),
            "first_seen_ms": number(stored["firstSeen"]),
            "source": (stored["source"] as? String) == "auto" ? "auto" : "manual",
        ]
    }

    /// The core's `Contact` → one stored contact.
    ///
    /// Optional fields are OMITTED when absent rather than written as `null`,
    /// so the bytes stay identical to what the other clients produce.
    static func contactToStored(_ wire: [String: Any]) -> [String: Any] {
        var out: [String: Any] = [
            "address": wire["address"] as? String ?? "",
            "kind": wire["kind"] as? String ?? "unknown",
            "favorite": wire["favorite"] as? Bool ?? false,
            "txCount": (wire["tx_count"] as? NSNumber)?.intValue ?? 0,
            "lastUsed": (wire["last_used_ms"] as? NSNumber)?.doubleValue ?? 0,
            "firstSeen": (wire["first_seen_ms"] as? NSNumber)?.doubleValue ?? 0,
            "source": wire["source"] as? String ?? "manual",
        ]
        if let name = wire["name"] as? String { out["name"] = name }
        if let resolved = wire["resolved_name"] as? String { out["resolvedName"] = resolved }
        if let source = wire["resolved_source"] as? String { out["resolvedSource"] = source }
        if let note = wire["note"] as? String { out["note"] = note }
        return out
    }

    static func groupToWire(_ stored: [String: Any]) -> [String: Any] {
        [
            "id": stored["id"] as? String ?? "",
            "name": stored["name"] as? String ?? "",
            // A group whose colour was never set arrives as `null`, which the
            // core's `Option<String>` reads as absent.
            "color": text(stored["color"]),
            "members": (stored["members"] as? [Any] ?? [])
                .compactMap { ($0 as? String)?.lowercased() },
        ]
    }

    static func groupToStored(_ wire: [String: Any]) -> [String: Any] {
        var out: [String: Any] = [
            "id": wire["id"] as? String ?? "",
            "name": wire["name"] as? String ?? "",
            "members": wire["members"] as? [Any] ?? [],
        ]
        if let color = wire["color"] as? String { out["color"] = color }
        return out
    }

    /// Stored object (`address → epoch ms`) → the core's tombstone list.
    static func tombstonesToWire(_ stored: [String: Any]) -> [[String: Any]] {
        stored.map { address, at in
            ["address": address.lowercased(), "dismissed_at_ms": number(at)]
        }
    }

    /// The core's tombstone list → the stored object.
    ///
    /// The pivot is the whole reason this function exists. Writing an array
    /// here is silently accepted by `UserDefaults` and read as empty by every
    /// client, which resurrects every deleted contact.
    static func tombstonesToStored(_ wire: [[String: Any]]) -> [String: Any] {
        var out: [String: Any] = [:]
        for entry in wire {
            guard let address = entry["address"] as? String else { continue }
            out[address] = (entry["dismissed_at_ms"] as? NSNumber)?.doubleValue ?? 0
        }
        return out
    }

    // MARK: Coercion helpers — deserialization hygiene, never policy

    /// An empty string is treated as absent, matching web's `str()`.
    static func text(_ value: Any?) -> Any {
        guard let string = value as? String, !string.isEmpty else { return NSNull() }
        return string
    }

    static func number(_ value: Any?) -> Double {
        guard let number = value as? NSNumber else { return 0 }
        let double = number.doubleValue
        return double.isFinite ? double : 0
    }

    static func kind(_ value: Any?) -> String {
        let raw = value as? String ?? ""
        return ["eoa", "account", "unknown"].contains(raw) ? raw : "unknown"
    }
    /// The outgoing sends this device recorded, as `ContactHistoryTx`.
    ///
    /// `nil` means the store could not be read at all — which is a different
    /// fact from "nothing has been sent", and the core has a variant for each.
    static func sendHistory(store: VelaStore) -> [[String: Any]]? {
        guard store.hasKey(VelaStore.Key.transactionHistory) else { return nil }
        return TxRecords.load(store: store).compactMap { record in
            guard (record["type"] as? String) == "send" else { return nil }
            guard let to = record["to"] as? String, !to.isEmpty else { return nil }
            return [
                "kind": "send",
                "to": to,
                "to_name": (record["toName"] as? String).map { $0 as Any } ?? NSNull(),
                // The store keeps SECONDS; the core counts milliseconds.
                "timestamp_ms": ((record["timestamp"] as? NSNumber)?.doubleValue ?? 0) * 1000,
            ]
        }
    }

}
