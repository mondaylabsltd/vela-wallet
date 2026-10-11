//
//  NetworkAdminStub.swift
//  VelaWalletTests
//
//  The `network_admin` machine's world, with no network in it: every
//  operation that would reach one is answered here, at once; storage, the
//  debounce, the pool and bundler flushes go to the REAL executor, so what is
//  saved is what is on disk.
//
//  SettingsEndpointsTests used to send its blur's probe wave to the real
//  services ("nothing is asserted about them"). Nothing was — but a test that
//  waits for its machine to finish then waits on four real requests and their
//  timeouts, and a CI runner's network is not this test's business.
//

import Foundation
@testable import VelaWallet

@MainActor
enum NetworkAdminStub {
    /// `reported` is what an RPC answers to `eth_chainId`, per URL; `chains`
    /// what the chain-data endpoint knows (raw, as the executor sends it);
    /// `asked` hears each operation's type and URL as it is performed.
    /// `code` is what `eth_getCode` answers for a contract's address and
    /// `p256` what the P-256 precompile's test call returns — both `nil`
    /// (nothing deployed, no verifier) unless a test says otherwise.
    static func perform(
        executor: NetworkAdminExecutor,
        reported: @escaping (String) -> Int? = { _ in nil },
        chains: [Int: [String: Any]] = [:],
        code: @escaping (_ address: String) -> String? = { _ in nil },
        p256: String? = nil,
        asked: @escaping (_ type: String, _ url: String) -> Void = { _, _ in }
    ) -> @MainActor ([String: Any]) async -> String {
        { operation in
            let type = operation["type"] as? String ?? ""
            let url = operation["url"] as? String ?? ""
            asked(type, url)
            switch type {
            case "probe_rpc":
                return CoreJSON.string([
                    "type": "probed", "url": url,
                    "reported_chain_id": reported(url) ?? NSNull(), "latency_ms": 12,
                ])
            case "probe_reachable":
                return CoreJSON.string(["type": "reachable", "url": url, "ok": true, "latency_ms": 9])
            case "fetch_service_health":
                return CoreJSON.string([
                    "type": "service_health", "field": operation["field"] ?? "",
                    "body": ["type": "failed"], "latency_ms": 0,
                ])
            case "fetch_fiat_rates":
                return CoreJSON.string(["type": "fiat_rates", "body": ["type": "failed"], "latency_ms": 0])
            case "fetch_chain_info":
                let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
                return CoreJSON.string([
                    "type": "chain_info", "chain_id": chainId,
                    "data": chains[chainId] ?? NSNull(),
                ])
            case "fetch_search_index":
                return CoreJSON.string(["type": "search_index", "chains": []])
            case "rpc_get_code":
                let address = operation["address"] as? String ?? ""
                return CoreJSON.string([
                    "type": "code", "url": url, "address": address,
                    "code": code(address).map { $0 as Any } ?? NSNull(),
                ])
            case "rpc_call_p256":
                return CoreJSON.string([
                    "type": "p256_call", "url": url, "result": p256.map { $0 as Any } ?? NSNull(),
                ])
            default:
                return await executor.perform(operation)
            }
        }
    }
}
