//
//  HeldTokens.swift
//  VelaWallet
//
//  What the last balance read found — shared, because two machines need it.
//
//  Web gets this for free from `fetchTokens`' five-minute cache, which
//  `getCachedHeldTokens` and `activity.ts`'s chain list both read. There is no
//  such cache here (the balance executor fetches straight through), so this is
//  it: a small, in-memory record of the most recent read, written by
//  `BalanceExecutor` and read by the receipt scan.
//
//  ## Three questions it answers, and why each is somebody else's business
//
//  - **Which chains does this account use?** The scan set. Scanning twelve
//    chains for an account that holds one is twenty-four `eth_getLogs` nobody
//    asked for; an empty answer means "brand-new wallet" and `token_trust`
//    falls back to its own default monitor chains.
//  - **Which ERC-20s does it hold on a chain?** `token_trust`'s trusted receive
//    set. A cold cache is an empty set, which leaves everything received
//    unverified — the safe direction, and the reason this never guesses.
//  - **What was a token worth?** The receipt's ingest valuation. A real price
//    if the balance read had one, and the stablecoin fallback otherwise.
//
//  It is deliberately NOT persisted. A held set restored from disk would be a
//  claim about the chain made by a file, and the trusted receive set is exactly
//  the wrong place for that.
//

import Foundation

@MainActor
final class HeldTokens {

    struct Holding {
        let symbol: String
        /// `nil`: the read did not say (r12-1 — never 18 assumed).
        let decimals: Int?
        let priceUsd: Double?
    }

    /// Lowercased address → chain id → the holdings read there.
    private var byAccount: [String: [Int: [String: Holding]]] = [:]

    /// `nil` token = the chain's native coin, which is how the balance rows and
    /// the receipt rows both name it.
    private static func key(_ token: String?) -> String {
        token.map { $0.lowercased() } ?? "native"
    }

    /// Record one completed balance read. Replaces the account's whole entry:
    /// a token that has left the wallet must leave the trusted set with it.
    func record(address: String, tokens: [[String: Any]]) {
        var byChain: [Int: [String: Holding]] = [:]
        var positive = Set<Int>()
        for token in tokens {
            guard let chainId = (token["chain_id"] as? NSNumber)?.intValue else { continue }
            if token["spam"] as? Bool != true,
               (token["balance"] as? String ?? "").contains(where: { ("1"..."9").contains($0) }) {
                positive.insert(chainId)
            }
            let holding = Holding(
                symbol: token["symbol"] as? String ?? "",
                decimals: (token["decimals"] as? NSNumber)?.intValue,
                priceUsd: (token["price_usd"] as? NSNumber)?.doubleValue
            )
            byChain[chainId, default: [:]][Self.key(token["token_address"] as? String)] = holding
        }
        byAccount[address.lowercased()] = byChain
        heldOn[address.lowercased()] = positive
    }

    /// The chains this account was last seen holding something on.
    func chainIds(address: String) -> [Int] {
        Array((byAccount[address.lowercased()] ?? [:]).keys).sorted()
    }

    /// Every chain any account was last seen holding something on — the
    /// shown-networks evidence (a holding of zero is never recorded here,
    /// so what is listed is money).
    func allChainIds() -> [Int] {
        Array(Set(heldOn.values.joined())).sorted()
    }

    /// Lowercased address → the chains a balance above zero was read on,
    /// spam left out (a spam token is not money) — `allChainIds`' source.
    private var heldOn: [String: Set<Int>] = [:]

    /// The ERC-20 contracts held on one chain — the native row excluded, since
    /// it is not a contract anybody can be received from.
    func erc20Addresses(address: String, chainId: Int) -> [String] {
        let chain = byAccount[address.lowercased()]?[chainId] ?? [:]
        return chain.keys.filter { $0 != "native" }
    }

    func holding(address: String, chainId: Int, token: String?) -> Holding? {
        byAccount[address.lowercased()]?[chainId]?[Self.key(token)]
    }

    /// Whether a balance read of this account has completed at all (R12
    /// S4): until one has, what it holds — the networks a scan looks at —
    /// is not known yet, even when it is nothing.
    func recorded(address: String) -> Bool {
        byAccount[address.lowercased()] != nil
    }
}
