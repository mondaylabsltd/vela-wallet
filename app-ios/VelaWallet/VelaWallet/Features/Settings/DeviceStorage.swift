//
//  DeviceStorage.swift
//  VelaWallet
//
//  What this device actually holds — the port of Android's
//  `feature/settings/core/DeviceStorage.kt` (spec 047 D4), itself the web's
//  `device-storage.ts`.
//
//  Until 058 the iOS page drew "2.4 MB · 216 records" on every phone, for every
//  account, forever. That is worse than showing nothing: 清除 sits beside those
//  numbers offering to free an amount nobody measured, and a person deciding
//  whether their history is worth keeping is reading a picture.
//
//  Two rules carried from the Android file, and both are about honesty:
//
//  1. **Filed by key, not by guess.** `item(of:)` is the same mapping on all
//     three clients, so "browsing data" means the same keys everywhere and
//     clearing it removes exactly those.
//  2. **Records are counted only where they exist.** A value that is not a list
//     has no record count, and the row then shows a size alone rather than an
//     invented "1".
//

import Foundation
import WebKit

enum DeviceStorage {

    enum Group: String {
        case user, cache, sessions
    }

    struct Item {
        let id: String
        let group: Group
    }

    /// The drawn rows, in the order the page draws them.
    static let items: [Item] = [
        Item(id: "transactions", group: .user),
        Item(id: "contacts", group: .user),
        Item(id: "custom", group: .user),
        Item(id: "browsing", group: .user),
        Item(id: "balances", group: .cache),
        Item(id: "rates", group: .cache),
        Item(id: "scan", group: .cache),
        Item(id: "dapps", group: .sessions),
    ]

    /// Which drawn row a key belongs to; `nil` = the account records and the
    /// preferences, which are nobody's row — they are not offered for deletion
    /// here, so counting them under a row that can be cleared would be a lie
    /// about what clearing does.
    static func item(of key: String) -> String? {
        switch true {
        case key == VelaStore.Key.transactionHistory: return "transactions"
        case key.hasPrefix("vela.contacts"), key == "vela.contactGroups": return "contacts"
        case key == VelaStore.Key.customTokens, key == VelaStore.Key.customNetworks:
            return "custom"
        case key == "vela.browserHistory", key == "vela.explore", key == "vela.bhist":
            return "browsing"
        case key == VelaStore.Key.balanceCache, key == "vela.balanceHidden": return "balances"
        case key.hasPrefix("vela.fiatRates"), key.hasPrefix("vela.fxRates"),
             key.hasPrefix("vela.fiatFeedAddrs"):
            return "rates"
        case key.hasPrefix("vela.receiveWatch"), key.hasPrefix("vela.trust"),
             key == "vela.rpc.banned":
            return "scan"
        case key.hasPrefix("vela.perm."): return "dapps"
        default: return nil
        }
    }

    struct ItemReport {
        let id: String
        let group: Group
        let keys: [String]
        let bytes: Int
        /// `nil` when the value is not a list — see rule 2.
        let records: Int?
    }

    struct Report {
        let items: [ItemReport]
        let totalBytes: Int
        let totalRecords: Int

        func bytes(of group: Group) -> Int {
            items.filter { $0.group == group }.reduce(0) { $0 + $1.bytes }
        }

        func item(_ id: String) -> ItemReport? { items.first { $0.id == id } }
    }

    static func measure(_ store: VelaStore) -> Report {
        let keys = store.allKeys()
        let values = Dictionary(uniqueKeysWithValues: keys.map { ($0, store.rawValue($0) ?? "") })
        let reports = items.map { item -> ItemReport in
            let own = keys.filter { self.item(of: $0) == item.id }
            let bytes = own.reduce(0) { $0 + ((values[$1] ?? "").utf8.count) }
            let counted = own.compactMap { records(in: values[$0] ?? "") }
            return ItemReport(
                id: item.id,
                group: item.group,
                keys: own,
                bytes: bytes,
                records: counted.isEmpty ? nil : counted.reduce(0, +)
            )
        }
        return Report(
            items: reports,
            totalBytes: reports.reduce(0) { $0 + $1.bytes },
            totalRecords: reports.reduce(0) { $0 + ($1.records ?? 0) }
        )
    }

    /// A JSON array's length, or an object's list-shaped member; `nil` when the
    /// value is not a list at all.
    static func records(in value: String) -> Int? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let data = trimmed.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data)
        else { return nil }
        if let array = parsed as? [Any] { return array.count }
        guard let object = parsed as? [String: Any] else { return nil }
        for name in ["items", "contacts", "entries", "records", "grants"] {
            if let array = object[name] as? [Any] { return array.count }
        }
        return nil
    }

    /// Remove exactly one row's keys.
    @discardableResult
    static func clear(_ store: VelaStore, item id: String) -> [String] {
        let keys = store.allKeys().filter { self.item(of: $0) == id }
        keys.forEach { store.remove($0) }
        return keys
    }

    // MARK: - Erase this device (spec 081 FR-017)

    /// The only `vela.` key an erase leaves behind.
    ///
    /// A record in `vela.pendingUploads` is a passkey public key the index
    /// service has never confirmed. The next launch's retry needs no account
    /// list to re-send it, but a DELETED record can never be retried — and
    /// that credential then cannot be found at sign-in on any device. Erasing
    /// it would downgrade "recoverable" to "possibly ruined", which is strictly
    /// worse here than at sign-out, because the account list is going too.
    static let eraseKeepKeys: Set<String> = [VelaStore.Key.pendingUploads]

    /// Erase this device, and say what survived.
    ///
    /// ## Why a prefix sweep and not the list this replaced
    ///
    /// `RootView.eraseThisDevice` used to walk eighteen hand-written key
    /// names. It missed fourteen groups — the account records, the pending
    /// uploads, the service endpoints, the fee tier, `vela.balanceHidden`,
    /// `vela.rpc.banned`, the receive watches, the trust marks, the browsing
    /// history, the `vela.perm.*` grants, the fiat feed addresses — and it
    /// missed them **silently**, because nothing about a delete-list fails
    /// when the app grows a key. So the direction is inverted: enumerate what
    /// is actually stored, drop everything under `vela.`, name the exception.
    /// A key a future feature writes is erased on the day it is first written.
    ///
    /// ## And the three stores that are not `UserDefaults`
    ///
    /// `WKWebsiteDataStore.default()` holds every dApp the person browsed —
    /// cookies, localStorage, IndexedDB, service workers — and the old path
    /// never touched it, so "erase this device" left somebody signed in to the
    /// sites they had visited. `URLCache.shared` and the logo store's own
    /// 32 MB disk cache hold the images the wallet fetched, which is a
    /// readable list of the tokens and chains it holds.
    ///
    /// - Returns: the `vela.` keys still present afterwards. Empty means the
    ///   device is clean; anything else is a failed erase the caller must show
    ///   rather than report as success.
    @MainActor
    static func eraseDevice(
        _ store: VelaStore,
        keep: Set<String> = eraseKeepKeys
    ) async -> [String] {
        for key in store.allKeys() where !keep.contains(key) {
            store.remove(key)
        }

        // Every dApp's cookies, local storage, databases and caches. The
        // completion-handler form on purpose: the erase must not continue to
        // its verification while WebKit is still deleting.
        let types = WKWebsiteDataStore.allWebsiteDataTypes()
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            WKWebsiteDataStore.default().removeData(
                ofTypes: types, modifiedSince: .distantPast
            ) { continuation.resume() }
        }

        // The images this wallet fetched: the shared cache, and the logo
        // store's own disk cache, which is not part of it.
        URLCache.shared.removeAllCachedResponses()
        LogoStore.forgetAll()

        // Verify by re-reading. The sweep above is not evidence of anything.
        return store.allKeys().filter { !keep.contains($0) }
    }

    /// Remove every cache row's keys. The caller decides what else to refresh.
    @discardableResult
    static func clearCaches(_ store: VelaStore) -> [String] {
        let cacheIds = Set(items.filter { $0.group == .cache }.map(\.id))
        let keys = store.allKeys().filter { key in
            guard let id = item(of: key) else { return false }
            return cacheIds.contains(id)
        }
        keys.forEach { store.remove($0) }
        return keys
    }

    // MARK: - Saying it

    /// "1.0 MB", "42 KB", "318 B" — the web's own thresholds, so three clients
    /// describe the same bytes the same way.
    static func size(_ bytes: Int) -> (amount: String, unit: String) {
        if bytes >= 1_048_576 {
            return (String(format: "%.1f", Double(bytes) / 1_048_576), "MB")
        }
        if bytes >= 1024 {
            return (String(Int((Double(bytes) / 1024).rounded())), "KB")
        }
        return (String(bytes), "B")
    }

    static func sizeText(_ bytes: Int) -> String {
        let measured = size(bytes)
        return "\(measured.amount) \(measured.unit)"
    }
}
