//
//  CoreRoundScenes.swift
//  VelaWallet
//
//  Views written by the REAL cores, driven through their bridges with nothing
//  read and nothing sent, for PR 2's integration round:
//
//  - `FeeCoreScene` — the `fee_policy` machine to a failure, and to its own
//    re-ask after it (note 1): the row's figure and reason, whether it is
//    retrying, and the line under the held confirm;
//  - `BalanceCoreScene` — the `balance_dashboard` machine over a round whose
//    Ethereum read never left the app (note 11): the home's line.
//
//  The boards draw them and the tests read them, so both show what the
//  machine itself says — never a hand-written guess at its JSON.
//
//  DEBUG only: fixture data, for `CorrectnessGalleryScreen` and the tests.
//

#if DEBUG

import Foundation
import VelaCore

enum FeeCoreScene: String, CaseIterable {
    /// The chain's nodes did not answer the account read: the core asks
    /// again by itself (3 s, 6 s, then every 8 s) — "Retrying…".
    case chainDown = "chain-down"
    /// The read never left the app (issue 483): Vela's own fault, retried
    /// the same way, never "Can't reach <chain>".
    case internalFault = "internal"
    /// An account with no key to estimate it with: no retry fixes it, so
    /// only a tap asks again — "Tap to retry" / "Tap it to retry".
    case missingKey = "missing-key"

    /// The core's views for this scene, as it wrote them: the failure on
    /// screen and, for a failure the core retries by itself, the moment its
    /// re-ask is out (`retrying`). `nil` if the bridge refused a step.
    struct Views {
        let failed: String
        let retrying: String?
    }

    static let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    @MainActor
    func views(chainId: Int = 8453) -> Views? {
        let core = FeePolicyCore()
        var event: [String: Any] = [
            "type": "quote_requested",
            "chain_id": chainId,
            "account": Self.account,
            "deployed": false,
            "public_key_available": self != .missingKey,
            "tier": "standard",
            "calls": [[
                "to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141",
                "value": "1000000000000000", "data": "0x",
            ]],
            "fee_token": NSNull(),
            "auto_fee_token": true,
            "number": "comma_dot",
        ]
        // The account read is the fee machine's own (issue 483): it fails here.
        if self != .missingKey { event["read_deployment"] = true }
        guard let asked = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event))) else {
            return nil
        }
        if self == .missingKey {
            return Views(failed: Self.view(asked), retrying: nil)
        }
        let read: [String: Any] = self == .chainDown
            ? ["type": "unreachable", "rate_limited": false]
            : ["type": "internal", "kind": "board: pool_fault"]
        guard let readId = Self.effect("read_deployment", in: asked),
              let failed = try? CoreJSON.object(core.resolveEffect(
                  effectId: readId, resultJson: CoreJSON.string(["type": "deployment", "read": read])
              ))
        else { return nil }
        // The core's own re-ask: the timer it set on the failure runs out.
        guard let ttlId = Self.effect("start_ttl", in: failed),
              let retrying = try? CoreJSON.object(core.resolveEffect(
                  effectId: ttlId, resultJson: CoreJSON.string(["type": "ttl_elapsed"])
              ))
        else { return Views(failed: Self.view(failed), retrying: nil) }
        return Views(failed: Self.view(failed), retrying: Self.view(retrying))
    }

    fileprivate static func view(_ step: [String: Any]) -> String {
        CoreJSON.string(step["view"] as? [String: Any] ?? [:])
    }

    fileprivate static func effect(_ type: String, in step: [String: Any]) -> UInt64? {
        for effect in step["effects"] as? [[String: Any]] ?? [] {
            guard let operation = effect["operation"] as? [String: Any],
                  operation["type"] as? String == type
            else { continue }
            return (effect["id"] as? NSNumber)?.uint64Value
        }
        return nil
    }
}

/// The home's balance over a round in which one chain's read never left the
/// app (PR 2 note 11) — or, `internalFault: false`, the same chain simply out
/// of reach — as the `balance_dashboard` core writes it.
enum BalanceCoreScene {
    static let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// `failedChain` failed this round: inside Vela (`internal_chain_ids`), or
    /// its nodes did not answer. Gnosis and Base answered with holdings —
    /// unless `everyChain`: then every chain asked failed the same way, with
    /// nothing cached, and nothing at all is known.
    @MainActor
    static func view(failedChain: Int = 1, internalFault: Bool, everyChain: Bool = false) -> BalanceViewWire? {
        let core = BalanceDashboardCore()
        guard let opened = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_changed", "address": address,
        ]))), let fetchId = FeeCoreScene.effect("fetch_tokens", in: opened)
        else { return nil }
        let tokens: [[String: Any]] = [
            [
                "chain_id": 100, "symbol": "xDAI", "name": "Gnosis", "balance": "412.5", "decimals": 18,
                "token_address": NSNull(), "price_usd": 1.0, "spam": false,
            ],
            [
                "chain_id": 8453, "symbol": "USDC", "name": "USD Coin", "balance": "250", "decimals": 6,
                "token_address": "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", "price_usd": 1.0,
                "spam": false,
            ],
        ]
        let read = [failedChain, 100, 8453]
        let failed = everyChain ? read : [failedChain]
        guard let settled = try? CoreJSON.object(core.resolveEffect(effectId: fetchId, resultJson: CoreJSON.string([
            "type": "fetch_settled", "address": address, "pull": false,
            "tokens": everyChain ? [] : tokens,
            "failed_chain_ids": failed,
            "rate_limited_chain_ids": [Int](),
            "read_chain_ids": read,
            "internal_chain_ids": internalFault ? failed : [Int](),
            "now_ms": Date().timeIntervalSince1970 * 1000,
        ]))), let view = settled["view"] as? [String: Any]
        else { return nil }
        return try? CoreJSON.decode(BalanceViewWire.self, from: view)
    }
}

#endif
