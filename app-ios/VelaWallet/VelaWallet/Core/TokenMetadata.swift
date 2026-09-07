//
//  TokenMetadata.swift
//  VelaWallet
//
//  An ERC-20's `symbol()` and `decimals()`, read from the contract.
//
//  Ported from `app-web/vela-wallet/src/lib/services/token-metadata.ts`. It
//  exists because of one visible defect: a first receipt of an unfamiliar token
//  had no metadata, and an 18-decimals fallback renders a 6-decimals stablecoin
//  as **"+0 tokens"**. `token_trust` would rather withhold the row than show
//  that (invariant ③), so this is what unblocks it.
//
//  ## No static table here, on purpose
//
//  Web keeps a `knownToken` list to skip the RPC for the twenty famous
//  contracts. The core carries the same table (`token_trust.rs KNOWN_TOKENS`)
//  and **only asks for addresses it is not in** — so a Swift copy would be a
//  second table that can drift from the one making the decisions.
//
//  ## Every requested address is answered
//
//  Resolved or not. An omitted address leaves the core's metadata gate
//  permanently unmet and the scan chain never finishes — silence wedges it,
//  where `meta: null` is a fact it can act on.
//

import Foundation
import VelaCore

@MainActor
final class TokenMetadata {

    struct Meta: Equatable {
        let symbol: String
        let decimals: Int
    }

    /// Tokens per batch — two sub-calls each, which keeps the calldata bounded.
    private static let batchSize = 40

    private let store: VelaStore
    private let pool: RpcPool
    /// `nil` means "looked up and unresolvable": a dud contract is not asked
    /// again this session. Negatives stay in memory only, in case the miss was
    /// a transient RPC failure rather than the contract.
    private var memo: [String: Meta?] = [:]

    init(store: VelaStore, pool: RpcPool) {
        self.store = store
        self.pool = pool
    }

    /// Resolve every address on one chain. Keyed lowercased; unresolved
    /// addresses are simply absent from the map.
    func resolve(chainId: Int, addresses: [String]) async -> [String: Meta] {
        var out: [String: Meta] = [:]
        let wanted = Array(Set(addresses.map { $0.lowercased() }.filter { !$0.isEmpty }))
        guard !wanted.isEmpty else { return out }

        var misses: [String] = []
        for address in wanted {
            if let remembered = memo[Self.memoKey(chainId, address)] {
                if let meta = remembered { out[address] = meta }
            } else if let stored = readStored(chainId: chainId, address: address) {
                memo[Self.memoKey(chainId, address)] = stored
                out[address] = stored
            } else {
                misses.append(address)
            }
        }
        guard !misses.isEmpty else { return out }

        guard let symbolCall = Multicall.selector("symbol()"),
              let decimalsCall = Multicall.selector("decimals()")
        else { return out }

        for start in stride(from: 0, to: misses.count, by: Self.batchSize) {
            let batch = Array(misses[start..<min(start + Self.batchSize, misses.count)])
            var calls: [Multicall3Call] = []
            for address in batch {
                calls.append(Multicall.call(address, symbolCall))
                calls.append(Multicall.call(address, decimalsCall))
            }
            // A whole unreachable batch leaves these unresolved and **not
            // memoised**: the contract said nothing, the network did.
            guard case .ok(let results) = await Multicall.aggregate3(
                chainId: chainId, calls: calls, pool: pool
            ), results.count == calls.count else { continue }

            for (index, address) in batch.enumerated() {
                let symbolResult = results[index * 2]
                let decimalsResult = results[index * 2 + 1]
                var meta: Meta?
                if symbolResult.success, decimalsResult.success {
                    let symbol = Self.decodeString(symbolResult.returnData)
                    if !symbol.isEmpty, let decimals = Self.decodeDecimals(decimalsResult.returnData) {
                        meta = Meta(symbol: symbol, decimals: decimals)
                    }
                }
                memo[Self.memoKey(chainId, address)] = meta
                if let meta {
                    out[address] = meta
                    writeStored(chainId: chainId, address: address, meta: meta)
                }
            }
        }
        return out
    }

    // MARK: - The persistent cache

    /// `vela.tokenMeta.<chainId>.<address>` — web's key, so a token resolved in
    /// one client is not re-read in the other.
    private static func storeKey(_ chainId: Int, _ address: String) -> String {
        "vela.tokenMeta.\(chainId).\(address)"
    }

    private static func memoKey(_ chainId: Int, _ address: String) -> String {
        "\(chainId):\(address)"
    }

    private func readStored(chainId: Int, address: String) -> Meta? {
        let stored = store.readObject(Self.storeKey(chainId, address))
        guard let symbol = stored["symbol"] as? String, !symbol.isEmpty,
              let decimals = (stored["decimals"] as? NSNumber)?.intValue,
              decimals >= 0, decimals <= 36
        else { return nil }
        return Meta(symbol: symbol, decimals: decimals)
    }

    private func writeStored(chainId: Int, address: String, meta: Meta) {
        store.writeObject(Self.storeKey(chainId, address),
                          ["symbol": meta.symbol, "decimals": meta.decimals])
    }

    // MARK: - Decoding

    /// A `decimals()` word, or `nil` when it is not a usable one.
    ///
    /// Deliberately NOT `Prices.feedDecimals`, which answers 8 for anything it
    /// cannot read. That default is right for a Chainlink feed and wrong here
    /// twice over: it would invent a scale for an unreadable answer, and it
    /// would turn a legitimate `decimals() == 0` token into an 8.
    static func decodeDecimals(_ returnData: Data) -> Int? {
        guard returnData.count >= 32,
              let text = TokenReads.scaled(bytes: returnData.subdata(in: 0..<32), decimals: 0),
              let value = Int(text), value >= 0, value <= 36
        else { return nil }
        return value
    }

    /// An ABI-encoded `string` return — `[offset][length][data]`.
    ///
    /// Falls back to a `bytes32` reading for the legacy tokens (MKR) that
    /// answer with one fixed word, and decodes as UTF-8 so a multibyte symbol
    /// (`USD₮0`) survives intact.
    static func decodeString(_ returnData: Data) -> String {
        guard returnData.count >= 32 else { return "" }
        if returnData.count < 64 { return utf8(returnData.prefix(32)) }
        let lengthWord = returnData.subdata(in: 32..<64)
        guard let text = TokenReads.scaled(bytes: lengthWord, decimals: 0),
              let length = Int(text), length > 0, length <= 4_096,
              64 + length <= returnData.count
        else {
            // Not offset-encoded — read the head as `bytes32`.
            return utf8(returnData.prefix(32))
        }
        return utf8(returnData.subdata(in: 64..<(64 + length)))
    }

    /// UTF-8 up to the first NUL — the `bytes32` padding terminator.
    private static func utf8<C: Collection>(_ bytes: C) -> String where C.Element == UInt8 {
        let trimmed = Array(bytes.prefix { $0 != 0 })
        return String(decoding: trimmed, as: UTF8.self)
    }
}
