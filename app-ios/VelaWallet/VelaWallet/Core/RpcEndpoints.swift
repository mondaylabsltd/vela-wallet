//
//  RpcEndpoints.swift
//  VelaWallet
//
//  Which endpoints exist for a chain — and nothing about which one to use.
//
//  Ported from `app-web/vela-wallet/src/lib/services/rpc-pool-endpoints.ts`,
//  and the split it documents is the one `rpc_pool.rs` insists on:
//
//      Deliberately *not* decision-making: building each chain's candidate list
//      from config, probing a single URL, parsing a provider's range-cap
//      wording. Which endpoint to try, when to ban, how long to back off — the
//      core's.
//
//  ## Bans are collected THROUGH, not filtered here
//
//  The core's invariant ⑧: bans are core state and the core excludes them at
//  **selection**. A shell that filtered banned URLs out of collection would
//  hide from the core the very endpoints its self-rescue exists to reconsider —
//  so `isBanned` is passed and always answers `false`, exactly as web's
//  `NEVER_BANNED` does, and the parameter is kept so the shape matches the
//  source it was ported from.
//
//  ## The tiers are a cold-start order, not a ranking
//
//  `user` before `provider` before `default` before `public` before the chain
//  index. The core scores on this only until it has measured latency of its
//  own, after which the order here stops mattering.
//

import Foundation

/// One candidate, before the core has any stats for it.
struct CollectedEndpoint {
    let url: String
    /// The core's `RpcSource`, snake_case on the wire.
    let source: String
}

enum RpcEndpoints {

    /// `vela.rpc.banned` — one key, one format, every client.
    static let banStorageKey = "vela.rpc.banned"

    /// Build the candidate list for one chain.
    ///
    /// Every source is independently `try`-free: a shelf that cannot be read
    /// contributes nothing rather than failing the collection, because a chain
    /// with *some* endpoints is usable and a chain with none is not.
    static func collect(
        chainId: Int,
        store: VelaStore,
        accounts: AccountStore
    ) async -> [CollectedEndpoint] {
        var seen = Set<String>()
        var out: [CollectedEndpoint] = []

        func add(_ url: String?, _ source: String) {
            guard let url, !url.isEmpty, !seen.contains(url) else { return }
            seen.insert(url)
            out.append(CollectedEndpoint(url: url, source: source))
        }

        let builtin = ChainCatalog.meta(chainId)

        // 1. The person's own override, and only when it differs from the
        //    built-in — otherwise the same URL would occupy two tiers.
        let configured = store.readList(VelaStore.Key.networkConfig)
            .first { ($0["chainId"] as? NSNumber)?.intValue == chainId }
        if let override = configured?["rpcURL"] as? String, override != builtin?.rpcURL {
            add(override, "user")
        }

        // 2. Provider keys, in declaration order as the cold-start tiebreak.
        let keys = store.readObject(VelaStore.Key.rpcProviders)
        for provider in RpcProvider.allCases {
            guard let key = keys[provider.rawValue] as? String else { continue }
            add(provider.url(chainId: chainId, key: key), "provider")
        }

        // 3. The built-in default, then a custom network's own RPC.
        add(builtin?.rpcURL, "default")
        let custom = store.readList(VelaStore.Key.customNetworks)
            .first { ($0["chainId"] as? NSNumber)?.intValue == chainId }
        add(custom?["rpcURL"] as? String, "default")

        // 4. Curated public fallbacks.
        for url in ChainCatalog.publicRPCs[chainId] ?? [] { add(url, "public") }

        // 5/6. The chain index: the first few as primary, the rest as deep
        //      fallback. A chain nobody built in is reachable ONLY through
        //      these, which is why a failed index fetch is survivable and a
        //      missing one is not an error.
        let indexed = await chainIndexRPCs(chainId: chainId, accounts: accounts)
        for url in indexed.prefix(5) { add(url, "builtin") }
        for url in indexed.dropFirst(5).prefix(15) { add(url, "fallback") }

        return out
    }

    /// The bundler candidates for one chain. One base today — the configurable
    /// service endpoint, or its default — because Vela runs the relay itself.
    ///
    /// **The URL carries `/{chainId}`, and that is the contract rather than a
    /// convenience.** A bundler pool stores JSON-RPC URLs as `${base}/${chain}`
    /// and the core strips the suffix when it needs the REST base
    /// (`rpc_pool::strip_chain_suffix`). Handing it a bare base sends every
    /// operation to the relay's root, which answers nothing — and because spec
    /// 051 made no bundler calls at all, the bare list looked correct for a
    /// whole cut. It surfaced in 052 as a fee quote that never settled, with no
    /// error anywhere: the shell answered "no quotes", the core waited, and the
    /// screen said 估算中… forever.
    static func collectBundlers(chainId: Int, accounts: AccountStore) async -> [CollectedEndpoint] {
        let base = await builtinBundlerBase(accounts: accounts)
        return [CollectedEndpoint(url: "\(base)/\(chainId)", source: "default")]
    }

    /// The relay host, from Settings › Service nodes › Vela Relay, or the one
    /// Vela ships (`getBuiltinBundlerUrl()`).
    ///
    /// One reader, because the pool and the relay client both need it and two
    /// readers is how a configured relay ends up honoured on one path and not
    /// the other.
    static func builtinBundlerBase(accounts: AccountStore) async -> String {
        let endpoints = await accounts.loadServiceEndpoints()
        var base = (endpoints["bundlerServiceURL"] as? String)
            .flatMap { $0.isEmpty ? nil : $0 }
            ?? NetDefaults.bundlerServiceURL
        while base.hasSuffix("/") { base.removeLast() }
        return base
    }

    // MARK: - Bans

    /// Read the persisted ban map. Corrupt or absent reads as none banned,
    /// which is the safe direction: the core will re-ban anything still broken
    /// on its first failure, whereas a phantom ban would hide a working
    /// endpoint with nothing to clear it.
    static func loadBans(store: VelaStore) -> [[String: Any]] {
        store.readList(banStorageKey).compactMap { stored in
            guard let url = stored["url"] as? String, !url.isEmpty else { return nil }
            return [
                "url": url,
                "banned_at_ms": (stored["bannedAt"] as? NSNumber)?.doubleValue ?? 0,
                "permanent": stored["permanent"] as? Bool ?? false,
            ]
        }
    }

    /// Write it back in the shape every other client reads.
    static func saveBans(_ entries: [[String: Any]], store: VelaStore) {
        store.writeList(banStorageKey, entries.compactMap { wire in
            guard let url = wire["url"] as? String else { return nil }
            return [
                "url": url,
                "bannedAt": (wire["banned_at_ms"] as? NSNumber)?.doubleValue ?? 0,
                "permanent": wire["permanent"] as? Bool ?? false,
            ]
        })
    }

    // MARK: - The chain index

    /// `/chains/eip155-{id}.json` → its https RPC URLs.
    ///
    /// Filtered to https and to URLs without an unfilled key placeholder,
    /// because the index lists templates like `https://…/${API_KEY}` that are
    /// not endpoints until somebody substitutes a key.
    private static func chainIndexRPCs(chainId: Int, accounts: AccountStore) async -> [String] {
        let endpoints = await accounts.loadServiceEndpoints()
        let base = (endpoints["ethereumDataURL"] as? String)
            .flatMap { $0.isEmpty ? nil : $0 } ?? NetDefaults.ethereumDataURL
        let body = await CoreHTTP.getJSON(
            "\(base)/chains/eip155-\(chainId).json",
            timeout: CoreHTTP.Timeout.ethereumData
        )
        let rpcs = (body as? [String: Any])?["rpc"] as? [Any] ?? []
        return rpcs.compactMap { $0 as? String }.filter {
            $0.hasPrefix("https://") && !$0.contains("${") && !$0.contains("API_KEY")
        }
    }
}
