//
//  DeviceStorage.swift
//  VelaWallet
//
//  What this device actually holds — and, since 072, what "erase" removes.
//
//  Until 058 the iOS page drew "2.4 MB · 216 records" on every phone, for every
//  account, forever. That is worse than showing nothing: 清除 sits beside those
//  numbers offering to free an amount nobody measured, and a person deciding
//  whether their history is worth keeping is reading a picture.
//
//  ## What each key IS is the core's (spec 072)
//
//  Which row a key belongs to, whether "clear all caches" takes it, whether an
//  erase does, how many records a value holds and how bytes are said — all of
//  it is `vela_core::storage_catalog`, the one copy every shell reads. Four
//  shells had kept their own and they had drifted: this file filed
//  `vela.balanceHidden`, a preference, as a balance cache, so "clear all
//  caches" un-hid somebody's balance; and the erase walked a hand-kept list of
//  eighteen keys, which is wrong by default — a key added next year is never
//  erased.
//
//  What stays here is what only a shell can do: enumerate its store, delete,
//  verify, and count bytes.
//

import Foundation
import VelaCore

enum DeviceStorage {

    enum Group: String {
        case user, cache, sessions
    }

    struct Item {
        let id: String
        let group: Group
    }

    /// The drawn rows, in the order the page draws them — the core's catalog.
    static let items: [Item] = storageItems().compactMap { record in
        Group(rawValue: record.group).map { Item(id: record.id, group: $0) }
    }

    /// Which drawn row a key belongs to; `nil` = the account records and the
    /// preferences, which are nobody's row. They are counted as the person's
    /// data and never swept by "clear all caches".
    static func item(of key: String) -> String? {
        storageItemOfKey(key: key)
    }

    struct ItemReport {
        let id: String
        let group: Group
        let keys: [String]
        let bytes: Int
        /// `nil` when the value is not a list — a row then shows a size alone
        /// rather than an invented "1".
        let records: Int?
    }

    struct Report {
        let items: [ItemReport]
        /// Every key of ours, rows and nobody's alike.
        let totalBytes: Int
        let totalRecords: Int
        /// The accounts and the preferences — your data nobody named.
        let unnamedBytes: Int

        func bytes(of group: Group) -> Int {
            items.filter { $0.group == group }.reduce(0) { $0 + $1.bytes }
                + (group == .user ? unnamedBytes : 0)
        }

        func item(_ id: String) -> ItemReport? { items.first { $0.id == id } }
    }

    static func measure(_ store: VelaStore) -> Report {
        // Every key the store holds, so `recipient_id:` — ours, and outside
        // the `vela.` namespace — is weighed with the rest.
        let keys = store.everyKey()
        let named = keys.compactMap { key in item(of: key).map { (key, $0) } }
        // Ours by the core's rule (`storage_catalog::is_ours`), in no row.
        let unnamed = keys.filter { storageIsOurs(key: $0) && item(of: $0) == nil }
        func bytes(_ key: String) -> Int { (store.rawValue(key) ?? "").utf8.count }

        let reports = items.map { item -> ItemReport in
            let own = named.filter { $0.1 == item.id }.map(\.0)
            let counted = own.compactMap { records(in: store.rawValue($0) ?? "") }
            return ItemReport(
                id: item.id,
                group: item.group,
                keys: own,
                bytes: own.reduce(0) { $0 + bytes($1) },
                records: counted.isEmpty ? nil : counted.reduce(0, +)
            )
        }
        let unnamedBytes = unnamed.reduce(0) { $0 + bytes($1) }
        return Report(
            items: reports,
            totalBytes: reports.reduce(0) { $0 + $1.bytes } + unnamedBytes,
            totalRecords: reports.reduce(0) { $0 + ($1.records ?? 0) },
            unnamedBytes: unnamedBytes
        )
    }

    /// A JSON array's length, or an object's list-shaped member; `nil` when the
    /// value is not a list at all.
    static func records(in value: String) -> Int? {
        storageRecordsIn(value: value).map(Int.init)
    }

    /// Remove exactly one row's keys.
    @discardableResult
    static func clear(_ store: VelaStore, item id: String) -> [String] {
        let keys = store.everyKey().filter { self.item(of: $0) == id }
        keys.forEach { store.remove($0) }
        return keys
    }

    /// Remove every cache key — the core's `is_cache_key`, exactly. The
    /// person's data, their preferences (a hidden balance among them) and
    /// their connections are untouched by construction. The caller decides
    /// what else to refresh.
    @discardableResult
    static func clearCaches(_ store: VelaStore) -> [String] {
        let keys = store.everyKey().filter { storageIsCacheKey(key: $0) }
        keys.forEach { store.remove($0) }
        return keys
    }

    // MARK: - Erase (spec 072)

    /// 抹除此设备: delete every key the core calls erasable, then look again.
    ///
    /// **A scan, never a list.** The store is enumerated and each key is asked
    /// `storageIsErasableKey`: everything of ours goes except the one record
    /// the core keeps (`vela.pendingUploads` — a passkey public key the index
    /// has never confirmed, whose deletion would make that key unfindable on
    /// every device).
    ///
    /// Returns what SURVIVED — empty is an erase that happened. A caller that
    /// sent the person to the first run over a non-empty answer would be
    /// telling them their phone is clean while their history is still on it.
    static func erase(_ store: VelaStore) -> [String] {
        store.everyKey().filter { storageIsErasableKey(key: $0) }.forEach { store.remove($0) }
        return store.everyKey().filter { storageIsErasableKey(key: $0) }.sorted()
    }

    // MARK: - Saying it

    /// "1.5 MB", "42 KB", "318 B" — in 1024s (the core's `bytes_display`), the
    /// number in the person's own number format. One decimal from KB up, so a
    /// megabyte and a half is not rounded to two.
    static func size(
        _ bytes: Int, format: NumberFormatKey = Formats.current.number
    ) -> (amount: String, unit: String) {
        let display = storageBytesDisplay(bytes: UInt64(max(0, bytes)))
        let digits = display.unit == "B" ? 0 : 1
        return (
            Formats.number(display.value, format, minimumFractionDigits: 0, maximumFractionDigits: digits),
            display.unit
        )
    }

    static func sizeText(_ bytes: Int, format: NumberFormatKey = Formats.current.number) -> String {
        let measured = size(bytes, format: format)
        return "\(measured.amount) \(measured.unit)"
    }
}
