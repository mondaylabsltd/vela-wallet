//
//  ChainTokens.swift
//  VelaWallet
//
//  What the token registry knows about one chain.
//
//  Ported from `app-web/vela-wallet/src/lib/services/chain-tokens.ts`, trimmed
//  to the half `token_trust` needs: the canonical **stablecoins** and the
//  **wrapped native** token, from
//  `<ethereumDataURL>/chains/eip155-<chainId>.json`.
//
//  ## Why the core wants these, and what happens without them
//
//  The stables are the `eth_getLogs` allowlist (invariant ②) and, with the
//  wrapped native, the trusted receive set. A chain whose registry cannot be
//  reached simply contributes **no facts**: its stables stay out of the
//  allowlist and its tokens stay unverified. That is the safe direction, and
//  it is why a failed fetch here is silence rather than an invented list.
//
//  The DEX half of this file's web original is not ported — the quote path is
//  deferred (see `Prices`), and copying a router table nothing calls would be
//  configuration nobody maintains.
//

import Foundation

@MainActor
final class ChainTokens {

    struct Facts {
        /// Canonical stablecoin contracts on this chain.
        let stables: [String]
        /// The wrapped native token, when the chain has one.
        let wrappedNative: String?
    }

    /// The registry is master data that changes on the order of weeks.
    private static let ttlMs: Double = 30 * 60 * 1000

    private let accounts: AccountStore
    private var cache: [Int: (facts: Facts, atMs: Double)] = [:]

    init(accounts: AccountStore) {
        self.accounts = accounts
    }

    /// One chain's registry facts, or `nil` when the registry could not be
    /// reached — which the caller must pass on as silence, never as an empty
    /// allowlist.
    func facts(chainId: Int) async -> Facts? {
        let now = Date().timeIntervalSince1970 * 1000
        if let cached = cache[chainId], now - cached.atMs < Self.ttlMs { return cached.facts }

        let endpoints = await accounts.loadServiceEndpoints()
        let base = (endpoints["ethereumDataURL"] as? String)
            .flatMap { $0.isEmpty ? nil : $0 } ?? NetDefaults.ethereumDataURL
        let body = await CoreHTTP.getJSON(
            "\(base)/chains/eip155-\(chainId).json",
            timeout: CoreHTTP.Timeout.ethereumData
        )
        guard let object = body as? [String: Any] else { return nil }

        let stables = (object["stables"] as? [[String: Any]] ?? []).compactMap {
            ($0["contract"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        }
        let wrapped = (object["wrappedNativeToken"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        let facts = Facts(stables: stables, wrappedNative: wrapped)
        cache[chainId] = (facts, now)
        return facts
    }
}
