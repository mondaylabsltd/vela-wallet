//
//  GuardExecutor.swift
//  VelaWallet
//
//  The `approval_guard` machine's three reads. Nothing here judges an
//  approval.
//
//  The core detects every approval-granting shape off the raw calldata and
//  decides what may leave. This supplies a token's symbol and decimals, an
//  allowance and a balance — three facts, and the honest absence of each.
//
//  `metas: null` (the whole read failed) and a token **missing from** a
//  returned list are different facts with different fallbacks in the core. An
//  empty token list answers `[]`, never `null`: nothing to ask about is not a
//  failure to find out.
//

import Foundation
import VelaCore

@MainActor
final class GuardExecutor {

    static let operations = ["read_token_metadata", "read_erc20_allowance", "read_erc20_balance"]

    private let pool: RpcPool

    init(pool: RpcPool) {
        self.pool = pool
    }

    func perform(_ operation: [String: Any]) async -> String {
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0

        switch operation["type"] as? String ?? "" {

        case "read_token_metadata":
            let tokens = operation["tokens"] as? [String] ?? []
            return CoreJSON.string([
                "type": "meta_resolved",
                "metas": await metas(chainId: chainId, tokens: tokens) as Any? ?? NSNull(),
            ])

        case "read_erc20_allowance":
            let value = await uint256(
                chainId: chainId,
                to: operation["token"] as? String ?? "",
                data: Self.allowanceCalldata(
                    owner: operation["owner"] as? String ?? "",
                    spender: operation["spender"] as? String ?? ""
                )
            )
            return CoreJSON.string(["type": "allowance_read", "allowance": value as Any? ?? NSNull()])

        case "read_erc20_balance":
            let owner = operation["owner"] as? String ?? ""
            let value = await uint256(
                chainId: chainId,
                to: operation["token"] as? String ?? "",
                data: try? erc20EncodeBalanceOf(ownerHex: owner)
            )
            return CoreJSON.string(["type": "balance_read", "balance": value as Any? ?? NSNull()])

        default:
            print("[vela-wallet] approval_guard: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "read_token_metadata": return CoreJSON.string(["type": "meta_resolved", "metas": NSNull()])
        case "read_erc20_allowance": return CoreJSON.string(["type": "allowance_read", "allowance": NSNull()])
        default: return CoreJSON.string(["type": "balance_read", "balance": NSNull()])
        }
    }

    /// `allowance(address,address)` — three existing exports, no new Rust.
    static func allowanceCalldata(owner: String, spender: String) -> Data? {
        guard let selector = try? functionSelector(signature: "allowance(address,address)"),
              let ownerWord = try? abiEncodeAddress(addressHex: owner),
              let spenderWord = try? abiEncodeAddress(addressHex: spender)
        else { return nil }
        return selector + ownerWord + spenderWord
    }

    /// One `eth_call` whose answer is a single word, as a decimal string.
    private func uint256(chainId: Int, to: String, data: Data?) async -> String? {
        guard let data, !to.isEmpty else { return nil }
        let hex = "0x" + data.map { String(format: "%02x", $0) }.joined()
        let outcome = await pool.call(
            chainId: chainId,
            method: "eth_call",
            params: [["to": to, "data": hex], "latest"]
        )
        guard case .ok(let body) = outcome,
              let result = body as? String
        else { return nil }
        return Self.decimal(fromWordHex: result)
    }

    /// One token list, one Multicall3 round trip.
    ///
    /// `allowFailure` is set on every call: one reverting token in a batch of
    /// twelve must not cost the other eleven their symbol.
    private func metas(chainId: Int, tokens: [String]) async -> [[String: Any]]? {
        guard !tokens.isEmpty else { return [] }
        guard let symbolSelector = try? functionSelector(signature: "symbol()"),
              let decimalsSelector = try? functionSelector(signature: "decimals()")
        else { return nil }

        let calls = tokens.flatMap { token in
            [Multicall.call(token, symbolSelector), Multicall.call(token, decimalsSelector)]
        }
        guard case .ok(let results) = await Multicall.aggregate3(chainId: chainId, calls: calls, pool: pool),
              results.count >= calls.count
        else { return nil }

        return tokens.enumerated().compactMap { index, token in
            let symbolSlot = results[index * 2]
            let decimalsSlot = results[index * 2 + 1]
            guard symbolSlot.success, decimalsSlot.success,
                  let symbol = Self.string(fromReturnData: symbolSlot.returnData),
                  let decimals = Self.decimal(fromWordHex: Self.hex(decimalsSlot.returnData)),
                  let value = Int(decimals), (0...36).contains(value)
            else { return nil }
            return ["token": token, "symbol": symbol, "decimals": value]
        }
    }

    // MARK: - The three decoders

    private static func hex(_ data: Data) -> String {
        "0x" + data.map { String(format: "%02x", $0) }.joined()
    }

    /// One 32-byte word as a decimal string, arbitrary precision.
    ///
    /// Not `UInt64`: an unlimited allowance is 2^256 - 1 and every integer
    /// type in Swift overflows on it. The core wants a decimal string anyway.
    static func decimal(fromWordHex raw: String) -> String? {
        var digits = raw.hasPrefix("0x") ? String(raw.dropFirst(2)) : raw
        guard !digits.isEmpty, digits.count % 2 == 0,
              digits.allSatisfy({ $0.isHexDigit })
        else { return nil }
        if digits.count > 64 { digits = String(digits.suffix(64)) }

        var decimal = [0]
        for character in digits {
            guard let nibble = character.hexDigitValue else { return nil }
            var carry = nibble
            for position in decimal.indices {
                let product = decimal[position] * 16 + carry
                decimal[position] = product % 10
                carry = product / 10
            }
            while carry > 0 {
                decimal.append(carry % 10)
                carry /= 10
            }
        }
        return decimal.reversed().map(String.init).joined()
    }

    /// An ABI-encoded `string` return, or a `bytes32` symbol from the older
    /// tokens that predate the standard's own return type.
    static func string(fromReturnData data: Data) -> String? {
        if data.count >= 64 {
            let offset = Int(decimal(fromWordHex: hex(data.prefix(32))) ?? "") ?? -1
            if offset == 32, data.count >= 64 {
                let length = Int(decimal(fromWordHex: hex(data.subdata(in: 32..<64))) ?? "") ?? 0
                let start = 64
                let end = start + length
                if length > 0, data.count >= end,
                   let text = String(data: data.subdata(in: start..<end), encoding: .utf8) {
                    return text
                }
            }
        }
        // `bytes32`: right-padded with zeros.
        if data.count == 32 {
            let trimmed = data.prefix { $0 != 0 }
            if !trimmed.isEmpty, let text = String(data: trimmed, encoding: .utf8) { return text }
        }
        return nil
    }
}
