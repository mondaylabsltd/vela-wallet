//
//  HoldingsCache.swift
//  VelaWallet
//
//  The rows the home showed last time, per account and network space (home-1,
//  home-3): the web's `services/holdings-cache.ts`, the same key, entry shape
//  and 24 h TTL. Assets opens on them instead of a skeleton — the core lets
//  them stand (`holdings_restored`) until each chain answers, and a chain that
//  never answers keeps them and is listed unreachable.
//
//  A cache, not a record: `vela.holdingsCache` is a cache key in the core's
//  storage catalog, so Settings' clear-caches and the erase both take it.
//

import Foundation

enum HoldingsCache {
    /// `vela.holdingsCache` — one key, every account and space.
    static let storageKey = "vela.holdingsCache"
    /// A day: older rows are not believed.
    static let ttlMs: Double = 24 * 60 * 60 * 1000

    /// The entry's key: the address, trimmed and lowercased, then the space
    /// (`mainnet` / `testnet`) — the web's `key(address, space)`.
    static func key(address: String, space: String) -> String {
        "\(address.trimmingCharacters(in: .whitespaces).lowercased())#\(space)"
    }

    /// Keep the rows a settled round left for `address` in `space`, as the
    /// core wrote them (`BalanceToken`, snake_case).
    static func save(address: String, space: String, tokens: [[String: Any]], store: VelaStore,
                     nowMs: Double = Date().timeIntervalSince1970 * 1000) {
        var all = store.readObject(storageKey)
        all[key(address: address, space: space)] = ["at": nowMs, "tokens": tokens]
        store.writeObject(storageKey, all)
    }

    /// The rows last kept, or `[]` when there are none, they are a day old,
    /// or a row is not in the shape the core reads.
    static func load(address: String, space: String, store: VelaStore,
                     nowMs: Double = Date().timeIntervalSince1970 * 1000) -> [[String: Any]] {
        guard let entry = store.readObject(storageKey)[key(address: address, space: space)] as? [String: Any],
              let at = (entry["at"] as? NSNumber)?.doubleValue, nowMs - at <= ttlMs,
              let tokens = entry["tokens"] as? [[String: Any]]
        else { return [] }
        return tokens.filter(believed)
    }

    /// A stored row is only believed in the shape the core reads.
    private static func believed(_ row: [String: Any]) -> Bool {
        row["chain_id"] is NSNumber && row["symbol"] is String && row["balance"] is String
            && row["decimals"] is NSNumber
    }
}
