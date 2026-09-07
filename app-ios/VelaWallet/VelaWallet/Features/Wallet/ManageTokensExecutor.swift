//
//  ManageTokensExecutor.swift
//  VelaWallet
//
//  The only place the `manage_tokens` core touches the outside world.
//
//  Ported from `app-web/vela-wallet/src/lib/wallet/core/manage-tokens-executor.ts`.
//  Five operations: one probe, two storage reads/writes, one removal and one
//  cache acknowledgement. Validity, admission, de-dupe and the decision to
//  invalidate are all the core's.
//
//  ## The probe answers `null` for every unresolved path
//
//  An RPC failure, a decode failure, a sub-call that reverted, a contract with
//  no name or no symbol — all the same `meta: null`. That is not laziness: the
//  core re-checks admission regardless, and a shell that distinguished them
//  would be deciding which failures are worth listing (invariant ②).
//
//  ## The address echo
//
//  Every probe answer carries the address it probed. The core drops an answer
//  for a superseded input — which is the fix for a real TypeScript defect: the
//  old closure landed token A's metadata under address B, and a save could then
//  write the wrong contract's symbol.
//

import Foundation
import VelaCore

@MainActor
final class ManageTokensExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "multicall_erc20_meta",
        "read_custom_tokens",
        "write_custom_token",
        "remove_custom_token",
        "invalidate_token_cache",
    ]

    private let store: VelaStore
    private let pool: RpcPool
    /// What "invalidate the token cache" means on this client.
    ///
    /// There is no `fetchTokens` TTL here — the balance executor reads through
    /// every time — so the honest translation of the core's invalidation is a
    /// re-read: a token somebody just added should appear in their balances
    /// now, not on the next refresh they happen to trigger.
    private let onInvalidate: () -> Void

    init(store: VelaStore, pool: RpcPool, onInvalidate: @escaping () -> Void = {}) {
        self.store = store
        self.pool = pool
        self.onInvalidate = onInvalidate
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "multicall_erc20_meta":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let address = operation["address"] as? String ?? ""
            return CoreJSON.string([
                "type": "chain_meta_resolved",
                "chain_id": chainId,
                // Echoed verbatim — the correlation key that retires an answer
                // for an address somebody has since edited.
                "address": address,
                "meta": await probe(chainId: chainId, address: address)
                    .map { $0 as Any } ?? NSNull(),
            ])

        case "read_custom_tokens":
            return CoreJSON.string([
                "type": "custom_tokens_loaded",
                "tokens": CustomTokens.load(store: store).compactMap(Self.toWire),
            ])

        case "write_custom_token":
            let token = CustomTokens.fromWire(operation["token"] as? [String: Any] ?? [:])
            return CoreJSON.string([
                "type": CustomTokens.save(token, store: store) ? "saved" : "save_failed",
            ])

        case "remove_custom_token":
            let id = operation["id"] as? String ?? ""
            // "It was already gone" is not a failure — the row leaves either
            // way, and reporting a failure would put it back.
            CustomTokens.remove(id: id, store: store)
            return CoreJSON.string(["type": "removed", "id": id])

        case "invalidate_token_cache":
            onInvalidate()
            return CoreJSON.string(["type": "cache_invalidated"])

        default:
            // See `ContactsExecutor`: logged, not trapped. An empty load leaves
            // the panel usable rather than stalled on its mount effect.
            print("[vela-wallet] manage_tokens: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "custom_tokens_loaded", "tokens": []])
        }
    }

    // MARK: - The probe

    /// `name()`, `symbol()` and `decimals()` in one `aggregate3`.
    ///
    /// The address goes to the chain **exactly as typed** — an `eth_call` is
    /// case-insensitive about it, and only the save lowercases. One batch, so a
    /// chain that is unreachable costs one round trip rather than three.
    private func probe(chainId: Int, address: String) async -> [String: Any]? {
        guard let nameCall = Multicall.selector("name()"),
              let symbolCall = Multicall.selector("symbol()"),
              let decimalsCall = Multicall.selector("decimals()")
        else { return nil }

        let outcome = await Multicall.aggregate3(
            chainId: chainId,
            calls: [
                Multicall.call(address, nameCall),
                Multicall.call(address, symbolCall),
                Multicall.call(address, decimalsCall),
            ],
            pool: pool
        )
        guard case .ok(let results) = outcome, results.count == 3,
              results[0].success, results[1].success, results[2].success
        else { return nil }

        let name = TokenMetadata.decodeString(results[0].returnData)
        let symbol = TokenMetadata.decodeString(results[1].returnData)
        // No name or no symbol is not a token this wallet will list. The core
        // owns that rule; this is the read that lets it apply it.
        guard !name.isEmpty, !symbol.isEmpty,
              let decimals = TokenMetadata.decodeDecimals(results[2].returnData)
        else { return nil }
        return ["name": name, "symbol": symbol, "decimals": decimals]
    }

    /// A stored token in `manage_tokens`' shape, which — unlike
    /// `token_trust`'s — carries `network_name`, because this machine freezes
    /// it into the record it saves.
    static func toWire(_ stored: [String: Any]) -> [String: Any]? {
        guard var wire = CustomTokens.toWire(stored) else { return nil }
        let chainId = (wire["chain_id"] as? Int) ?? 0
        wire["network_name"] = stored["networkName"] as? String
            ?? ChainCatalog.meta(chainId)?.displayName ?? ""
        return wire
    }
}
