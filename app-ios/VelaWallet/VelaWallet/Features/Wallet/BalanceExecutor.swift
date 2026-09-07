//
//  BalanceExecutor.swift
//  VelaWallet
//
//  The only place the `balance_dashboard` core touches the outside world.
//
//  Ported from web's `wallet-api.ts` + `balance-cache.ts` (spec 025). Seven
//  operations; the fetch is `Core/TokenReads.swift` and every chain read inside
//  it goes through `RpcPool` (FR-002).
//
//  ## `FetchTokens` names no chain
//
//  `FetchTokens { address, force, pull }` — the core delegates the entire
//  multi-chain read and rules only on what comes back. Which chains to read is
//  therefore the shell's, and it is the person's own network list: the twelve
//  built-ins plus whatever they added, which is `vela.customNetworks`.
//
//  ## What "failed" has to mean here
//
//  A chain that could not be read is reported **failed**, never as zero and
//  never omitted. Both of those would make the total look complete when it is
//  not, and the core has `balance_partial` precisely so the screen can say "at
//  least this much" instead of a number that is quietly wrong.
//

import Foundation

@MainActor
final class BalanceExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "fetch_tokens",
        "fetch_account_assets",
        "read_balance_cache",
        "read_balance_cache_many",
        "write_balance_cache",
        "start_retry_timer",
        "write_privacy",
    ]

    /// The cached total's shelf life. A day old is still worth showing while a
    /// fetch runs; a week old is a number nobody should read as current.
    private static let cacheTTLMs: Double = 24 * 60 * 60 * 1000

    /// `vela.balanceHidden` — the tap-to-hide state, so it survives a relaunch.
    private static let privacyKey = "vela.balanceHidden"

    private let store: VelaStore
    private let pool: RpcPool
    /// The Chainlink map, cached across refreshes. One instance, because twelve
    /// chains asking Ethereum mainnet for the same five feeds is eleven
    /// round trips nobody needs.
    private let prices: Prices
    /// What this read found, published for the receipt scan: which chains to
    /// watch, which tokens are held, and what they were worth. Web gets the
    /// same three facts out of `fetchTokens`' cache.
    private let held: HeldTokens

    init(store: VelaStore, pool: RpcPool, held: HeldTokens) {
        self.store = store
        self.pool = pool
        self.prices = Prices(pool: pool)
        self.held = held
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "fetch_tokens":
            return await fetch(address: operation["address"] as? String ?? "",
                               pull: operation["pull"] as? Bool ?? false)

        // The per-account read behind the switcher. Same fetch, but its failure
        // is `tokens: null` rather than a chain list, because a switcher row
        // shows one figure or none.
        case "fetch_account_assets":
            let address = operation["address"] as? String ?? ""
            let chains = chainIds()
            let chainlinkPrices = await prices.mainnetPrices()
            var tokens: [[String: Any]] = []
            var anyFailed = false
            for chainId in chains {
                let result = await TokenReads.read(
                    address: address, chainId: chainId,
                    tokens: customTokens(chainId: chainId), pool: pool,
                    chainlinkPrices: chainlinkPrices
                )
                tokens.append(contentsOf: result.tokens)
                anyFailed = anyFailed || result.failed
            }
            return CoreJSON.string([
                "type": "account_assets_fetched",
                "address": address,
                "tokens": anyFailed ? NSNull() : tokens,
            ])

        case "read_balance_cache":
            let address = (operation["address"] as? String ?? "").lowercased()
            return CoreJSON.string([
                "type": "cached_total_loaded",
                "address": operation["address"] as? String ?? "",
                "usd": cache()[address].map { $0 as Any } ?? NSNull(),
            ])

        case "read_balance_cache_many":
            let addresses = operation["addresses"] as? [String] ?? []
            let cached = cache()
            return CoreJSON.string([
                "type": "cached_balances_loaded",
                "balances": addresses.compactMap { address in
                    cached[address.lowercased()].map { ["address": address, "usd": $0] }
                },
            ])

        case "write_balance_cache":
            writeCache(address: operation["address"] as? String ?? "",
                       usd: (operation["usd"] as? NSNumber)?.doubleValue ?? 0)
            return CoreJSON.string(["type": "balance_cache_written"])

        case "start_retry_timer":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string([
                "type": "retry_elapsed",
                "timer_id": (operation["timer_id"] as? NSNumber)?.intValue ?? 0,
            ])

        case "write_privacy":
            store.writeString(Self.privacyKey,
                              (operation["hidden"] as? Bool ?? false) ? "1" : nil)
            return CoreJSON.string(["type": "privacy_written"])

        default:
            // See `ContactsExecutor`: logged, not trapped. `fetch_errored` is
            // the answer that leaves the core able to retry rather than stalled
            // holding the home screen blank.
            print("[vela-wallet] balance_dashboard: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string([
                "type": "fetch_errored", "address": "", "pull": false,
            ])
        }
    }

    /// Whether the person had the figure hidden last time. Read at boot so the
    /// core hydrates rather than flashing a balance somebody chose to conceal.
    var hiddenAtLaunch: Bool { store.readString(Self.privacyKey) == "1" }

    // MARK: - The fetch

    /// Read every chain, in parallel, and report which ones failed.
    ///
    /// Parallel because twelve sequential round trips is twelve times the
    /// latency for no benefit — the pool is shared and its ban map is one piece
    /// of state, but the calls themselves are independent.
    private func fetch(address: String, pull: Bool) async -> String {
        guard !address.isEmpty else {
            return CoreJSON.string(["type": "fetch_errored", "address": address, "pull": pull])
        }

        // Priced before the fan-out, and once: the mainnet feed batch is one
        // read the twelve chains then share. It is deliberately NOT inside the
        // group — twelve tasks racing to fill one three-minute cache would send
        // twelve identical calls to Ethereum on every cold refresh.
        let chainlinkPrices = await prices.mainnetPrices()

        let results = await withTaskGroup(of: TokenReads.ChainResult.self) { group in
            for chainId in chainIds() {
                let tokens = customTokens(chainId: chainId)
                group.addTask { [pool] in
                    await TokenReads.read(address: address, chainId: chainId,
                                          tokens: tokens, pool: pool,
                                          chainlinkPrices: chainlinkPrices)
                }
            }
            var collected: [TokenReads.ChainResult] = []
            for await result in group { collected.append(result) }
            return collected
        }

        // Published before the core is answered: the same tick's scan asks for
        // the held set, and a set one refresh out of date is a token watched
        // that is no longer there — or worse, one that is not yet.
        held.record(address: address, tokens: results.flatMap(\.tokens))

        return CoreJSON.string([
            "type": "fetch_settled",
            "address": address,
            "pull": pull,
            "tokens": results.flatMap(\.tokens),
            // Failed, never zero and never omitted — the core needs both lists
            // to know its total is a floor rather than a sum.
            "failed_chain_ids": results.filter(\.failed).map(\.chainId),
            "rate_limited_chain_ids": results.filter(\.rateLimited).map(\.chainId),
            "now_ms": Date().timeIntervalSince1970 * 1000,
        ])
    }

    /// Which chains to read: the built-ins plus whatever the person added.
    private func chainIds() -> [Int] {
        var ids = ChainCatalog.chains.map(\.chainId)
        for network in store.readList(VelaStore.Key.customNetworks) {
            guard let chainId = (network["chainId"] as? NSNumber)?.intValue,
                  !ids.contains(chainId)
            else { continue }
            ids.append(chainId)
        }
        return ids
    }

    /// The ERC-20s the wallet knows about on one chain, from
    /// `vela.customTokens` — the camelCase record every client writes.
    private func customTokens(chainId: Int) -> [CustomTokenRef] {
        store.readList(VelaStore.Key.customTokens).compactMap { stored in
            guard (stored["chainId"] as? NSNumber)?.intValue == chainId,
                  let address = stored["contractAddress"] as? String, !address.isEmpty
            else { return nil }
            return CustomTokenRef(
                address: address,
                symbol: stored["symbol"] as? String ?? "",
                name: stored["name"] as? String ?? "",
                decimals: (stored["decimals"] as? NSNumber)?.intValue ?? 18,
                chainId: chainId
            )
        }
    }

    // MARK: - The cached total

    /// `vela.balanceCache` — `address → { usd, at }`, the Expo bytes.
    ///
    /// An entry past its TTL is **dropped rather than returned stale**: the
    /// core would otherwise present a day-and-a-half-old figure as the last
    /// known one, and the screen has no way to say how old it is.
    private func cache() -> [String: Double] {
        let now = Date().timeIntervalSince1970 * 1000
        var out: [String: Double] = [:]
        for (address, value) in store.readObject(VelaStore.Key.balanceCache) {
            guard let entry = value as? [String: Any],
                  let usd = (entry["usd"] as? NSNumber)?.doubleValue,
                  let at = (entry["at"] as? NSNumber)?.doubleValue,
                  now - at <= Self.cacheTTLMs
            else { continue }
            out[address.lowercased()] = usd
        }
        return out
    }

    private func writeCache(address: String, usd: Double) {
        var raw = store.readObject(VelaStore.Key.balanceCache)
        raw[address.lowercased()] = ["usd": usd, "at": Date().timeIntervalSince1970 * 1000]
        store.writeObject(VelaStore.Key.balanceCache, raw)
    }
}
