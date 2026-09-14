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
    /// chain — and price the native coin while the batch is open.
    ///
    /// Two round trips: `eth_getBalance`, and one `aggregate3` carrying every
    /// token's `balanceOf` plus this chain's Chainlink native feed. A chain
    /// that fails a BALANCE read is reported failed rather than contributing a
    /// partial figure the core would have to treat as complete.
    ///
    /// `chainlinkPrices` is the Ethereum-mainnet map, fetched once per refresh
    /// and passed in — twelve chains must not each re-read mainnet.
    static func read(
        address: String,
        chainId: Int,
        tokens: [CustomTokenRef],
        pool: RpcPool,
        chainlinkPrices: [String: Double] = [:]
    ) async -> ChainResult {
        var out: [[String: Any]] = []
        var failed = false
        var rateLimited = false

        let meta = ChainCatalog.meta(chainId)
        let hasNativeCoin = meta?.gasModel != .tempo

        // 1. The native coin's balance — unless the chain has none.
        var nativeBalance: String?
        if hasNativeCoin {
            switch await pool.call(
                chainId: chainId, method: "eth_getBalance", params: [address, "latest"]
            ) {
            case .ok(let value):
                if let hex = value as? String { nativeBalance = scaled(hex: hex, decimals: 18) }
            case .failed(let throttled):
                failed = true
                rateLimited = throttled
            case .rangeCap:
                // Meaningless for a balance read; treated as a failure rather
                // than silently dropped.
                failed = true
            case .rpcError:
                // The endpoint answered and the answer was "no". The balance is
                // therefore UNKNOWN, and an unknown balance is a failed chain —
                // the alternative is a total that quietly omits a coin somebody
                // owns. Before spec 052 this arrived as `.ok(nil)` and the
                // chain reported success with nothing in it.
                failed = true
            }
        }

        // 2. The ERC-20 balances and this chain's own price feed, in one batch.
        let feed = hasNativeCoin ? Prices.nativeFeeds[chainId] : nil
        let batch = await batch(address: address, chainId: chainId, tokens: tokens,
                                priceFeed: feed, pool: pool)

        // A batch that carried no balances cannot report a balance failure.
        // Marking the chain failed because a PRICE feed did not answer would
        // draw "this network is unreachable" over a network that answered
        // every question about somebody's money.
        if batch.failed && !tokens.isEmpty {
            failed = true
            rateLimited = rateLimited || batch.rateLimited
        }

        if let nativeBalance {
            out.append([
                "chain_id": chainId,
                "symbol": meta?.nativeSymbol ?? "",
                "name": meta?.displayName ?? "",
                "balance": nativeBalance,
                "decimals": 18,
                "token_address": NSNull(),
                "price_usd": Prices.choose(
                    local: batch.localPrice,
                    mainnet: Prices.mainnetPrice(symbol: meta?.nativeSymbol ?? "",
                                                 in: chainlinkPrices)
                ).map { $0 as Any } ?? NSNull(),
                "spam": false,
            ])
        }
        out.append(contentsOf: batch.tokens)

        return ChainResult(chainId: chainId, tokens: out, failed: failed, rateLimited: rateLimited)
    }

    /// What one chain's batch came back with.
    private struct Batch {
        var tokens: [[String: Any]] = []
        /// This chain's own Chainlink native/USD read — the ladder's local
        /// rung. `nil` when the chain has no feed, or the feed did not answer.
        var localPrice: Double?
        var failed = false
        var rateLimited = false
    }

    /// `aggregate3` of one `balanceOf` per token, plus `latestRoundData()` on
    /// the chain's native feed when it has one.
    ///
    /// `allowFailure` is `true` on every call: a token that reverts — a proxy
    /// mid-upgrade, a contract that is not really an ERC-20 — must not cost the
    /// others their answer. A reverted entry keeps its slot, which is what lets
    /// results be matched to tokens by index.
    private static func batch(
        address: String,
        chainId: Int,
        tokens: [CustomTokenRef],
        priceFeed: String?,
        pool: RpcPool
    ) async -> Batch {
        var calls: [Multicall3Call] = []
        if !tokens.isEmpty {
            guard let owner = try? erc20EncodeBalanceOf(ownerHex: address) else {
                return Batch(failed: true)
            }
            calls = tokens.map { Multicall.call($0.address, owner) }
        }
        if let priceFeed, let latestRound = Multicall.selector("latestRoundData()") {
            calls.append(Multicall.call(priceFeed, latestRound))
        }
        guard !calls.isEmpty else { return Batch() }

        let outcome = await Multicall.aggregate3(chainId: chainId, calls: calls, pool: pool)
        guard case .ok(let results) = outcome else {
            // The chain answered with nothing this build can read. Failed, not
            // empty: "no tokens" is a claim, and this is not evidence for it.
            var refused = Batch(failed: true)
            if case .failed(let throttled) = outcome { refused.rateLimited = throttled }
            return refused
        }

        var out = Batch()
        for (index, result) in results.enumerated() where index < tokens.count {
            guard result.success,
                  let balance = scaled(bytes: result.returnData,
                                       decimals: tokens[index].decimals)
            else { continue }
            out.tokens.append([
                "chain_id": chainId,
                "symbol": tokens[index].symbol,
                "name": tokens[index].name,
                "balance": balance,
                "decimals": tokens[index].decimals,
                "token_address": tokens[index].address,
                // An ERC-20's price needs the DEX quote path, which this cut
                // does not have. `nil` is the drawn "couldn't be priced" row,
                // and a guessed $1 for anything that looks like a stablecoin is
                // exactly the invention the core's unpriced notice exists to
                // avoid.
                "price_usd": NSNull(),
                "spam": false,
            ])
        }
        if priceFeed != nil, let last = results.last, results.count == calls.count, last.success {
            out.localPrice = Prices.chainlinkAnswer(last.returnData, decimals: 8)
        }
        return out
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

    /// The same conversion from a **decimal** raw amount — the shape
    /// `token_trust` publishes a discovered transfer's value in.
    ///
    /// Still string arithmetic, and for the same reason: a receipt of
    /// 1,234,567.891234567890123456 tokens has more significant digits than a
    /// `Double` can hold, and rounding somebody's incoming payment on the way
    /// into the store would make the wallet disagree with the chain forever.
    static func scaled(decimal digits: String, decimals: Int) -> String? {
        let trimmed = digits.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty, trimmed.allSatisfy(\.isNumber) else { return nil }
        return placeDecimalPoint(trimmed, decimals: decimals)
    }

    /// Insert the point and trim, without ever becoming a number.
    static func placeDecimalPoint(_ digits: String, decimals: Int) -> String {
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
