//
//  TokenTrustExecutor.swift
//  VelaWallet
//
//  The only place the `token_trust` core touches the outside world.
//
//  Ported from `app-web/vela-wallet/src/lib/wallet/core/token-trust-executor.ts`.
//  Seven operations, one service call each. **No branching on business
//  meaning**: the allowlist, the one capped retry, the local `topics[2]`
//  re-verification, the metadata gate and the admission filter are all decided
//  in Rust — this file is transport and wire translation.
//
//  That division is not stylistic here. `token_trust` is the wallet's anti-scam
//  core: a shell that pre-filtered logs, or invented a symbol for a token that
//  would not answer, would have made the security decision the machine exists
//  to make.
//
//  ## Three contracts, or the core stalls
//
//  ① **`eth_getLogs` failures are classified once.** A provider that capped the
//    block span is a different fact from a provider that failed, and the core
//    acts on the difference (probe + exactly one capped retry, invariant ④).
//    On web that classification is a wording match in the shell; here it is
//    already made — `rpc_pool` parses the cap and hands back
//    `RpcOutcome.rangeCap`, which is why that case exists.
//  ② **`MulticallErc20Meta` answers EVERY requested address**, resolved or not.
//    An omitted address leaves the metadata gate permanently unmet and the scan
//    chain never finishes. `meta: null` is a fact; silence is a wedge.
//  ③ **`BlockTimestamp` carries `now_ms`.** The core never reads a clock, so
//    the "fall back to now" rule is fed to it rather than computed by it.
//

import Foundation
import VelaCore

@MainActor
final class TokenTrustExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "rpc_block_number",
        "rpc_get_logs",
        "rpc_get_block_by_number",
        "multicall_erc20_meta",
        "read_custom_tokens",
        "write_custom_token",
        "invalidate_token_cache",
    ]

    /// `keccak256("Transfer(address,address,uint256)")` — transport
    /// vocabulary, not a decision. The core pins the *recipient* topic and the
    /// contract allowlist and carries the same constant for its own local
    /// re-verification.
    private static let transferTopic =
        "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    private let store: VelaStore
    private let pool: RpcPool
    private let metadata: TokenMetadata

    init(store: VelaStore, pool: RpcPool, metadata: TokenMetadata) {
        self.store = store
        self.pool = pool
        self.metadata = metadata
    }

    func perform(_ operation: [String: Any]) async -> String {
        let address = operation["address"] as? String ?? ""
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0

        switch operation["type"] as? String ?? "" {

        case "rpc_block_number":
            let outcome = await pool.call(chainId: chainId, method: "eth_blockNumber")
            var hex: Any = NSNull()
            if case .ok(let value) = outcome, let text = value as? String { hex = text }
            return CoreJSON.string([
                "type": "block_number", "address": address, "chain_id": chainId,
                "block_hex": hex,
            ])

        case "rpc_get_logs":
            return await getLogs(operation, address: address, chainId: chainId)

        case "rpc_get_block_by_number":
            let block = operation["block"] as? String ?? ""
            let outcome = await pool.call(
                chainId: chainId, method: "eth_getBlockByNumber", params: [block, false]
            )
            var timestamp: Any = NSNull()
            if case .ok(let value) = outcome,
               let header = value as? [String: Any],
               let hex = header["timestamp"] as? String,
               let seconds = Self.hexToNumber(hex) {
                timestamp = seconds
            }
            return CoreJSON.string([
                "type": "block_timestamp", "address": address, "chain_id": chainId,
                // The core answers by block, so a block hex the shell cannot
                // read back is reported as 0 — the core drops it and the
                // transfer takes the `now` fallback.
                "block_number": Self.hexToNumber(block) ?? 0,
                "timestamp_sec": timestamp,
                // ③ the clock, carried.
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])

        case "multicall_erc20_meta":
            let addrs = operation["addrs"] as? [String] ?? []
            let resolved = await metadata.resolve(chainId: chainId, addresses: addrs)
            return CoreJSON.string([
                "type": "erc_meta", "chain_id": chainId,
                // ② one entry per requested address.
                "entries": addrs.map { addr -> [String: Any] in
                    guard let meta = resolved[addr.lowercased()] else {
                        return ["addr": addr, "meta": NSNull()]
                    }
                    return ["addr": addr,
                            "meta": ["symbol": meta.symbol, "decimals": meta.decimals]]
                },
            ])

        case "read_custom_tokens":
            return CoreJSON.string([
                "type": "custom_tokens",
                "tokens": CustomTokens.load(store: store).compactMap(CustomTokens.toWire),
            ])

        case "write_custom_token":
            let written = write(token: operation["token"] as? [String: Any] ?? [:])
            return CoreJSON.string(["type": "token_written", "ok": written])

        case "invalidate_token_cache":
            // There is no `fetchTokens` TTL to invalidate on this client — the
            // balance executor reads through every time — so this is an
            // acknowledged no-op rather than a skipped one. The core waits for
            // the ack before finishing an admission.
            return CoreJSON.string(["type": "cache_invalidated"])

        default:
            // See `ContactsExecutor`: logged, not trapped. `custom_tokens` with
            // an empty list leaves the machine able to scan rather than stalled
            // waiting for an answer that will never come.
            print("[vela-wallet] token_trust: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "custom_tokens", "tokens": []])
        }
    }

    // MARK: - The scan's one RPC that has three answers

    private func getLogs(
        _ operation: [String: Any], address: String, chainId: Int
    ) async -> String {
        var filter: [String: Any] = [
            "fromBlock": operation["from_block"] as? String ?? "0x0",
            "toBlock": operation["to_block"] as? String ?? "latest",
            "topics": [Self.transferTopic, NSNull(),
                       operation["recipient_topic"] as? String ?? ""],
        ]
        // An empty allowlist means "no address restriction" in a JSON-RPC
        // filter, so it is omitted rather than sent as `[]` — which every
        // endpoint reads as "match nothing".
        let contracts = operation["contracts"] as? [String] ?? []
        if !contracts.isEmpty { filter["address"] = contracts }

        // `kind` is the pool's endpoint CLASS — `rpc` or `bundler`, the two
        // tiers it scores separately. It is not a label for the method, and
        // passing one ("logs") faults the core, which is how this line was
        // first written and how the live suite caught it.
        let outcome = await pool.call(
            chainId: chainId, method: "eth_getLogs", params: [filter]
        )
        let result: [String: Any]
        switch outcome {
        case .ok(let value):
            result = ["type": "ok", "logs": (value as? [Any] ?? []).map(Self.logToWire)]
        case .rangeCap(_, let maxSpan):
            // ① the range verdict, made by `rpc_pool` rather than re-parsed
            // here. `0` means "capped, but no number could be read" and the
            // core stays conservative.
            result = ["type": "range_capped", "cap": max(0, Int(maxSpan))]
        case .failed:
            result = ["type": "failed"]
        }
        return CoreJSON.string([
            "type": "logs", "address": address, "chain_id": chainId, "outcome": result,
        ])
    }

    // MARK: - Wire translation

    /// One raw log, carried through untouched.
    ///
    /// A codec, not a policy: whether a log means anything is the core's call —
    /// it re-verifies `topics[2]` locally, so a buggy or malicious endpoint
    /// cannot surface somebody else's transfer as a fake "received"
    /// (invariant ①). Only the shape is normalised, because a missing field
    /// would be rejected by serde and a rejected result strands the scan.
    static func logToWire(_ raw: Any) -> [String: Any] {
        let record = raw as? [String: Any] ?? [:]
        return [
            "address": record["address"] as? String ?? "",
            "topics": (record["topics"] as? [Any] ?? []).map { $0 as? String ?? "" },
            "data": record["data"] as? String ?? "",
            "transaction_hash": record["transactionHash"] as? String ?? "",
            // `?? '0x0'` is applied in the core; `null` means "absent".
            "block_number": (record["blockNumber"] as? String).map { $0 as Any } ?? NSNull(),
            "log_index": (record["logIndex"] as? String).map { $0 as Any } ?? NSNull(),
        ]
    }

    /// The core's token in the shape every client persists — **replacing by
    /// id, never duplicating** (invariant ⑧), through the one writer both
    /// token machines share.
    private func write(token: [String: Any]) -> Bool {
        CustomTokens.save(CustomTokens.fromWire(token), store: store)
    }

    /// `parseInt(hex, 16)`. Unusable input reads as `nil`, never as a zero the
    /// core would compare against.
    static func hexToNumber(_ hex: String) -> Double? {
        let digits = hex.hasPrefix("0x") || hex.hasPrefix("0X") ? String(hex.dropFirst(2)) : hex
        guard !digits.isEmpty, let value = UInt64(digits, radix: 16) else { return nil }
        return Double(value)
    }
}
