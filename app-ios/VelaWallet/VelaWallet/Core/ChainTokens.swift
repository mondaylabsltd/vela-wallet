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
//  ## Three outcomes, not two (PR 2 polish)
//
//  The document is `doc` (a 2xx, parsed), `absent` (a server that answered
//  404 — there is no such document) or `unread` (no answer, a timeout, a 5xx,
//  a 429 or any other non-2xx, or a body that is not one). The first two are
//  facts and are kept for the TTL; `unread` is not, and is never cached — the
//  next read asks again. The balance read needs the difference: on a chain
//  with no native coin (Tempo) the registry's stablecoins are everything there
//  is to read, so an unread document is a chain NOT READ, never one that
//  answered holding nothing (`BalanceExecutor`). Other callers (token trust)
//  take both misses as "no facts".
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
        /// The stablecoins with their symbols — what the balance read plan
        /// (`balanceReadPlan`, spec 082 RE9) counts at their peg.
        var stableRefs: [(symbol: String, contract: String)] = []
    }

    /// One chain's registry document, as the fetch found it.
    enum Document {
        /// A 2xx with a JSON object body: the chain's facts.
        case doc(Facts)
        /// The server answered 404: there is no document for this chain — a
        /// definitive answer, kept like a document.
        case absent
        /// Nothing definitive: no answer, a timeout, a 5xx, a 429 or any other
        /// non-2xx, or a body that is not a JSON object. Never cached.
        case unread

        /// The facts, or `nil` for both misses — the "no facts" every caller
        /// but the balance read wants.
        var facts: Facts? {
            if case .doc(let facts) = self { return facts }
            return nil
        }

        var isUnread: Bool {
            if case .unread = self { return true }
            return false
        }
    }

    /// The registry is master data that changes on the order of weeks.
    private static let ttlMs: Double = 30 * 60 * 1000

    /// The registry's base URL — the person's `ethereumDataURL`, else the default.
    private let base: () async -> String
    /// One GET, classified (`CoreHTTP.getREST`): the seam a test answers.
    private let fetch: (String) async -> CoreHTTP.RestAnswer
    private let now: () -> Double
    /// Only `doc` and `absent` live here; `unread` is never kept.
    private var cache: [Int: (document: Document, atMs: Double)] = [:]

    convenience init(accounts: AccountStore) {
        self.init(
            base: {
                let endpoints = await accounts.loadServiceEndpoints()
                return (endpoints["ethereumDataURL"] as? String)
                    .flatMap { $0.isEmpty ? nil : $0 } ?? NetDefaults.ethereumDataURL
            },
            fetch: { url in await CoreHTTP.getREST(url, timeout: CoreHTTP.Timeout.ethereumData) }
        )
    }

    init(
        base: @escaping () async -> String,
        fetch: @escaping (String) async -> CoreHTTP.RestAnswer,
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }
    ) {
        self.base = base
        self.fetch = fetch
        self.now = now
    }

    /// One chain's registry facts, or `nil` when there are none to be had —
    /// no document, or one that could not be read — which the caller must
    /// pass on as silence, never as an empty allowlist.
    func facts(chainId: Int) async -> Facts? {
        await document(chainId: chainId).facts
    }

    /// One chain's registry document: `doc`, `absent` or `unread`.
    func document(chainId: Int) async -> Document {
        let at = now()
        if let cached = cache[chainId], at - cached.atMs < Self.ttlMs { return cached.document }
        let url = "\(await base())/chains/eip155-\(chainId).json"
        let document = Self.classify(await fetch(url))
        // A miss that proves nothing is asked again next time.
        if !document.isUnread { cache[chainId] = (document, at) }
        return document
    }

    /// The fetch's answer as a document. Only a server's 404 is `absent`; a
    /// 2xx whose body is not a JSON object is `unread`, like any other answer
    /// that could not be read.
    static func classify(_ answer: CoreHTTP.RestAnswer) -> Document {
        switch answer {
        case .ok(let object):
            return .doc(facts(object))
        case .status(404):
            return .absent
        case .status, .failed:
            return .unread
        }
    }

    private static func facts(_ object: [String: Any]) -> Facts {
        let rows = object["stables"] as? [[String: Any]] ?? []
        let stables = rows.compactMap {
            ($0["contract"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        }
        let refs: [(symbol: String, contract: String)] = rows.compactMap { row in
            guard let contract = row["contract"] as? String, !contract.isEmpty else { return nil }
            return (row["symbol"] as? String ?? "", contract)
        }
        let wrapped = (object["wrappedNativeToken"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        return Facts(stables: stables, wrappedNative: wrapped, stableRefs: refs)
    }
}
