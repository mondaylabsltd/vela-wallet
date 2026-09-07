//
//  TokenReads.swift
//  VelaWallet
//
//  Reading one address's holdings across every chain.
//
//  Ported from `app-web/vela-wallet/src/lib/services/wallet-api.ts` and
//  `token-reads.ts`. This is the service layer `FetchTokens { address, force,
//  pull }` delegates to: the operation **names no chain and no URL**, because
//  the core rules on what comes back rather than on how it was fetched.
//
//  ## Every read goes through the pool
//
//  FR-002. Not `CoreHTTP` directly — a ban is a fact about the network, and a
//  second caller with its own endpoint list is how the Expo client got a ban
//  map that disagreed with itself.
//
//  ## The balance is a HUMAN DECIMAL STRING
//
//  `balance_dashboard.rs:298`: "human decimal string (never a JSON number)".
//  Desktop's 031 wrote raw units and the defect hid for as long as prices were
//  `None` — the first real price would have multiplied a total by 10^18. The
//  conversion is `scaled(_:decimals:)` below and it is the only place it
//  happens.
//
//  ## Tempo is excluded from native balances, and not by a threshold
//
//  Tempo (4217) has no native coin — gas is a TIP-20 stablecoin — and its RPC
//  answers the same ~4.24e75 constant for **every** address. Its symbol is
//  `USD`, so a stablecoin peg would price that garbage at $1 and put ~4e57
//  dollars into somebody's total. The guard is the chain's declared gas model,
//  never an invented "that number looks too big" rule: a threshold would also
//  reject a genuine whale.
//

import Foundation
import VelaCore

/// Multicall3, at the same address on every chain that has it.
private let multicall3 = "0xcA11bde05977b3631167028862bE2a173976CA11"

enum TokenReads {

    /// One chain's answer. `failed` and `rateLimited` are carried separately
    /// because the core renders them differently: a failed chain is offered a
    /// fix, a throttled one is not (invariant ④).
    struct ChainResult {
        let chainId: Int
        let tokens: [[String: Any]]
        let failed: Bool
        let rateLimited: Bool
    }

    /// Read the native coin and every known ERC-20 for one address, on one
    /// chain.
    ///
    /// The native balance and the token batch are one `eth_getBalance` and one
    /// `aggregate3`. A chain that fails either is reported failed rather than
    /// contributing a partial figure the core would have to treat as complete.
    static func read(
        address: String,
        chainId: Int,
        tokens: [CustomTokenRef],
        pool: RpcPool
    ) async -> ChainResult {
        var out: [[String: Any]] = []
        var failed = false
        var rateLimited = false

        // 1. The native coin — unless the chain has none.
        let meta = ChainCatalog.meta(chainId)
        if meta?.gasModel != .tempo {
            switch await pool.call(
                chainId: chainId, method: "eth_getBalance", params: [address, "latest"]
            ) {
            case .ok(let value):
                if let hex = value as? String, let scaled = scaled(hex: hex, decimals: 18) {
                    out.append([
                        "chain_id": chainId,
                        "symbol": meta?.nativeSymbol ?? "",
                        "name": meta?.displayName ?? "",
                        "balance": scaled,
                        "decimals": 18,
                        "token_address": NSNull(),
                        "price_usd": NSNull(),
                        "spam": false,
                    ])
                }
            case .failed(let throttled):
                failed = true
                rateLimited = throttled
            case .rangeCap:
                // Meaningless for a balance read; treated as a failure rather
                // than silently dropped.
                failed = true
            }
        }

        // 2. The ERC-20s, in one batch.
        if !tokens.isEmpty {
            let batch = await readTokenBalances(
                address: address, chainId: chainId, tokens: tokens, pool: pool
            )
            out.append(contentsOf: batch.tokens)
            failed = failed || batch.failed
            rateLimited = rateLimited || batch.rateLimited
        }

        return ChainResult(chainId: chainId, tokens: out, failed: failed, rateLimited: rateLimited)
    }

    /// `aggregate3` of one `balanceOf` per token.
    ///
    /// `allowFailure` is `true` on every call: a token that reverts — a proxy
    /// mid-upgrade, a contract that is not really an ERC-20 — must not cost the
    /// others their answer. A reverted entry keeps its slot, which is what lets
    /// results be matched to tokens by index.
    private static func readTokenBalances(
        address: String,
        chainId: Int,
        tokens: [CustomTokenRef],
        pool: RpcPool
    ) async -> ChainResult {
        guard let owner = try? erc20EncodeBalanceOf(ownerHex: address) else {
            return ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
        }
        let calls = tokens.map {
            Multicall3Call(target: $0.address, allowFailure: true, callData: owner)
        }
        guard let calldata = try? multicall3EncodeAggregate3(calls: calls) else {
            return ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
        }

        let outcome = await pool.call(
            chainId: chainId,
            method: "eth_call",
            params: [["to": multicall3, "data": "0x" + calldata.hexString], "latest"]
        )
        guard case .ok(let value) = outcome else {
            if case .failed(let throttled) = outcome {
                return ChainResult(chainId: chainId, tokens: [], failed: true,
                                   rateLimited: throttled)
            }
            return ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
        }
        guard let hex = value as? String,
              let data = Data(hexString: hex),
              let results = try? multicall3DecodeAggregate3(data: data)
        else {
            // The chain answered with something this build cannot read. Failed,
            // not empty: "no tokens" is a claim, and this is not evidence for it.
            return ChainResult(chainId: chainId, tokens: [], failed: true, rateLimited: false)
        }

        var out: [[String: Any]] = []
        for (index, result) in results.enumerated() where index < tokens.count {
            guard result.success,
                  let balance = scaled(bytes: result.returnData,
                                       decimals: tokens[index].decimals)
            else { continue }
            out.append([
                "chain_id": chainId,
                "symbol": tokens[index].symbol,
                "name": tokens[index].name,
                "balance": balance,
                "decimals": tokens[index].decimals,
                "token_address": tokens[index].address,
                "price_usd": NSNull(),
                "spam": false,
            ])
        }
        return ChainResult(chainId: chainId, tokens: out, failed: false, rateLimited: false)
    }

    // MARK: - Raw units → human decimal

    /// `0xa8867319d2da000`, 18 → `"0.75897"`.
    ///
    /// Done in **decimal string arithmetic**, not `Double`: a `Double` carries
    /// 15–16 significant digits and a token balance routinely needs more, so
    /// converting through one silently rounds somebody's money.
    static func scaled(hex: String, decimals: Int) -> String? {
        let digits = hex.hasPrefix("0x") || hex.hasPrefix("0X")
            ? String(hex.dropFirst(2)) : hex
        guard !digits.isEmpty else { return nil }
        var value = "0"
        for character in digits {
            guard let nibble = character.hexDigitValue else { return nil }
            value = addDecimal(multiplyDecimal(value, by: 16), String(nibble))
        }
        return placeDecimalPoint(value, decimals: decimals)
    }

    static func scaled(bytes: Data, decimals: Int) -> String? {
        guard !bytes.isEmpty else { return nil }
        return scaled(hex: bytes.hexString, decimals: decimals)
    }

    /// Insert the point and trim, without ever becoming a number.
    private static func placeDecimalPoint(_ digits: String, decimals: Int) -> String {
        guard decimals > 0 else { return digits }
        let padded = String(repeating: "0", count: max(0, decimals + 1 - digits.count)) + digits
        let split = padded.index(padded.endIndex, offsetBy: -decimals)
        let whole = String(padded[..<split])
        var fraction = String(padded[split...])
        while fraction.hasSuffix("0") { fraction.removeLast() }
        return fraction.isEmpty ? whole : "\(whole).\(fraction)"
    }

    private static func multiplyDecimal(_ value: String, by factor: Int) -> String {
        var carry = 0
        var out = ""
        for character in value.reversed() {
            let product = (character.wholeNumberValue ?? 0) * factor + carry
            out.append(Character(String(product % 10)))
            carry = product / 10
        }
        while carry > 0 {
            out.append(Character(String(carry % 10)))
            carry /= 10
        }
        let result = String(out.reversed()).drop { $0 == "0" }
        return result.isEmpty ? "0" : String(result)
    }

    private static func addDecimal(_ value: String, _ other: String) -> String {
        var carry = 0
        var out = ""
        let a = Array(value.reversed()), b = Array(other.reversed())
        for index in 0..<max(a.count, b.count) {
            let sum = (index < a.count ? a[index].wholeNumberValue ?? 0 : 0)
                + (index < b.count ? b[index].wholeNumberValue ?? 0 : 0) + carry
            out.append(Character(String(sum % 10)))
            carry = sum / 10
        }
        if carry > 0 { out.append(Character(String(carry))) }
        let result = String(out.reversed()).drop { $0 == "0" }
        return result.isEmpty ? "0" : String(result)
    }
}

/// One ERC-20 the wallet knows about, as the balance reader needs it.
struct CustomTokenRef {
    let address: String
    let symbol: String
    let name: String
    let decimals: Int
    let chainId: Int
}

// MARK: - Hex

extension Data {
    /// Lowercase, no `0x`. uniffi maps Rust's `Vec<u8>` to `Data`, so this is
    /// the type every bridge byte string arrives as.
    var hexString: String { map { String(format: "%02x", $0) }.joined() }

    init?(hexString: String) {
        let digits = hexString.hasPrefix("0x") || hexString.hasPrefix("0X")
            ? String(hexString.dropFirst(2)) : hexString
        guard digits.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(digits.count / 2)
        var index = digits.startIndex
        while index < digits.endIndex {
            let next = digits.index(index, offsetBy: 2)
            guard let byte = UInt8(digits[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        self = Data(bytes)
    }
}
