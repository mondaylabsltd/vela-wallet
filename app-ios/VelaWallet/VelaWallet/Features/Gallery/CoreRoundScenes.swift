//
//  CoreRoundScenes.swift
//  VelaWallet
//
//  Views written by the REAL cores, driven through their bridges with nothing
//  read and nothing sent, for PR 2's integration round:
//
//  - `FeeCoreScene` — the `fee_policy` machine to a failure, and to its own
//    re-ask after it (note 1): the row's figure and reason, whether it is
//    retrying, and the line under the held confirm; and (PR 2 polish) to a
//    relay that answered the operation would fail — what a tap on the row
//    does then (`wouldFail`);
//  - `BalanceCoreScene` — the `balance_dashboard` machine over a round whose
//    Ethereum read never left the app (note 11): the home's line.
//  - `TrustCoreScene` — the `token_trust` machine's judged view of a
//    simulation (PR 3 device round): the view the signing sheet draws its
//    verdict from, with the core's own "No asset changes" key on a check
//    that moves nothing, and a verdict of four rows and a warning.
//
//  - `SimWaitScene` — the `sign_request` machine holding a transaction's
//    confirm for its simulation's verdict, and past its deadline (PR 3, fix
//    C), under the core's own gate (`signConfirmState`) over the request's
//    real reading and guard.
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

    /// PR 2 polish: the relay answered that this operation fails with the
    /// coin in force (`would_fail`) — the person chose USDC on `chainId` and
    /// the simulation of a swap was refused. With the chain's own coin also
    /// on offer (`anotherCoin`) a tap opens the coins, "Pay with another
    /// coin" (`tap` `choose_coin`); with nothing in it to pay from there is
    /// nothing left to try (`nothing`, the dash). Every read the run asks is answered as
    /// a healthy chain and relay would; the view is the core's own.
    @MainActor
    static func wouldFail(anotherCoin: Bool, chainId: Int = 8453) -> String? {
        let core = FeePolicyCore()
        let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
        let request: [String: Any] = [
            "type": "quote_requested",
            "chain_id": chainId,
            "account": account,
            "deployed": true,
            "public_key_available": true,
            "tier": "standard",
            // A contract call — a swap through a router: the relay's
            // simulation of it is what answers "this would fail" (a plain
            // transfer the relay refuses is priced on fallback limits).
            "calls": [[
                "to": "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
                "value": "0", "data": "0x3593564c" + String(repeating: "ab", count: 1_200),
            ]],
            "fee_token": usdc,
            "auto_fee_token": false,
            "number": "comma_dot",
        ]
        // With no other coin on offer the chain's own coin is still listed —
        // the relay always quotes it — but holds nothing to pay from.
        let native: [String: Any] = [
            "recipient": "0x00000000000000000000000000000000000000aa", "asset": "native",
            "fee_token": NSNull(), "balance": anotherCoin ? "1000000000000000000" : "0", "decimals": 18,
            "symbol": "ETH", "usd_balance": anotherCoin ? "1868.70" : "0", "usd_price": "1868.70000000",
        ]
        let usdcRow: [String: Any] = [
            "recipient": "0x00000000000000000000000000000000000000bb", "asset": "erc20",
            "fee_token": usdc, "balance": "5000000", "decimals": 6, "symbol": "USDC",
            "usd_balance": "5.00", "usd_price": "1",
        ]
        func answer(_ operation: [String: Any]) -> [String: Any]? {
            switch operation["type"] as? String {
            case "fetch_gas_price":
                return ["type": "gas_price", "eth_gas_price": "1000000000", "base_fee": "0", "priority_fee": "0"]
            case "fetch_bundler_quote":
                return ["type": "bundler_quote", "quote": [
                    "max_fee_per_gas": "2000000000", "max_priority_fee_per_gas": NSNull(),
                    "network_fee_per_gas": "1000000000", "relayer_fee_per_gas": "1000000000",
                    "in_band_fee_per_gas": NSNull(),
                ] as [String: Any]]
            case "fetch_in_band_quotes":
                return ["type": "in_band_quotes", "quotes": [native, usdcRow]]
            case "read_deployment":
                return ["type": "deployment", "read": ["type": "read", "deployed": true]]
            case "estimate_user_op_gas":
                // The relay answered: this exact operation fails.
                return ["type": "user_op_gas", "outcome": ["type": "refused"]]
            default:
                // Timers and the inner-call measure: never answered here.
                return nil
            }
        }
        guard var step = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(request))) else {
            return nil
        }
        var queue = step["effects"] as? [[String: Any]] ?? []
        var steps = 0
        while !queue.isEmpty, steps < 24 {
            steps += 1
            let effect = queue.removeFirst()
            guard let operation = effect["operation"] as? [String: Any],
                  let reply = answer(operation),
                  let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let next = try? CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(reply)))
            else { continue }
            if next["view"] is [String: Any] { step = next }
            queue += next["effects"] as? [[String: Any]] ?? []
        }
        return view(step)
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

    /// `failedChain` failed this round: inside Vela (`internal_chain_ids`),
    /// for want of its token list (`tokenListFault` → `registry_chain_ids`,
    /// PR 3 note 4), or its nodes did not answer. Gnosis and Base answered
    /// with holdings — unless `everyChain`: then every chain asked failed the
    /// same way, with nothing cached, and nothing at all is known.
    ///
    /// `alsoDown` are further chains whose nodes did not answer, beside
    /// `failedChain`; `healthy` is the same round with every chain answering.
    @MainActor
    static func view(
        failedChain: Int = 1, internalFault: Bool, everyChain: Bool = false, tokenListFault: Bool = false,
        alsoDown: [Int] = [], healthy: Bool = false
    ) -> BalanceViewWire? {
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
        let read = [failedChain, 100, 8453] + alsoDown
        let named = everyChain ? [failedChain, 100, 8453] : [failedChain]
        let failed = healthy ? [] : named + alsoDown
        guard let settled = try? CoreJSON.object(core.resolveEffect(effectId: fetchId, resultJson: CoreJSON.string([
            "type": "fetch_settled", "address": address, "pull": false,
            "tokens": everyChain ? [] : tokens,
            "failed_chain_ids": failed,
            "rate_limited_chain_ids": [Int](),
            "read_chain_ids": read,
            "internal_chain_ids": internalFault && !healthy ? named : [Int](),
            "registry_chain_ids": tokenListFault && !healthy ? named : [Int](),
            "now_ms": Date().timeIntervalSince1970 * 1000,
        ]))), let view = settled["view"] as? [String: Any]
        else { return nil }
        return try? CoreJSON.decode(BalanceViewWire.self, from: view)
    }

    /// Final note F19 — a wallet that held nothing last session, through its
    /// first round, as the real core writes it: `checking` is the view once
    /// the cached total (0) has landed and the read is still out; `settled`
    /// the view after that round ended — with every chain answering, with
    /// `missing` out of reach, or (`threw`) with a read that threw.
    @MainActor
    static func zeroWallet(
        missing: [Int] = [], threw: Bool = false
    ) -> (checking: BalanceViewWire, settled: BalanceViewWire)? {
        let core = BalanceDashboardCore()
        guard let opened = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_changed", "address": address,
        ]))), let cacheId = FeeCoreScene.effect("read_balance_cache", in: opened),
            let fetchId = FeeCoreScene.effect("fetch_tokens", in: opened),
            let cached = try? CoreJSON.object(core.resolveEffect(effectId: cacheId, resultJson: CoreJSON.string([
                "type": "cached_total_loaded", "address": address, "usd": 0.0,
            ]))), let checking = cached["view"] as? [String: Any]
        else { return nil }
        let result: [String: Any] = threw
            ? ["type": "fetch_errored", "address": address, "pull": false, "internal": false]
            : [
                "type": "fetch_settled", "address": address, "pull": false,
                "tokens": [[String: Any]](),
                "failed_chain_ids": missing,
                "rate_limited_chain_ids": [Int](),
                "read_chain_ids": [1, 56, 100, 8453] + missing,
                "internal_chain_ids": [Int](),
                "registry_chain_ids": [Int](),
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ]
        guard let ended = try? CoreJSON.object(core.resolveEffect(
            effectId: fetchId, resultJson: CoreJSON.string(result)
        )), let settled = ended["view"] as? [String: Any],
            let first = try? CoreJSON.decode(BalanceViewWire.self, from: checking),
            let last = try? CoreJSON.decode(BalanceViewWire.self, from: settled)
        else { return nil }
        return (first, last)
    }
}

/// The signing sheet's simulation verdict as the `token_trust` core judges
/// it (PR 3 device round): the deltas go in as a simulation's would, the
/// metadata the core then asks for is answered from `named`, and the view it
/// writes — judgments, `ready`, and `noChangeKey` when nothing moves — comes
/// back as it is.
enum TrustCoreScene {
    /// A token's `symbol()` and `decimals()`, as the chain would answer.
    typealias Named = [String: (symbol: String, decimals: Int)]

    @MainActor
    static func sim(
        _ deltas: [[String: Any]] = [], chainId: Int = 8453, named: Named = [:]
    ) -> TrustSimViewWire? {
        let core = TokenTrustCore()
        guard var step = try? CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "sim_deltas_computed", "address": FeeCoreScene.account.lowercased(),
            "chain_id": chainId, "deltas": deltas,
        ]))) else { return nil }
        // A token the core cannot name yet: it asks, and holds the verdict
        // until the answer. One nobody names stays unverified.
        for effect in step["effects"] as? [[String: Any]] ?? [] {
            guard let operation = effect["operation"] as? [String: Any],
                  operation["type"] as? String == "multicall_erc20_meta",
                  let id = (effect["id"] as? NSNumber)?.uint64Value
            else { continue }
            let entries = (operation["addrs"] as? [String] ?? []).map { addr -> [String: Any] in
                guard let meta = named[addr.lowercased()] else { return ["addr": addr, "meta": NSNull()] }
                return ["addr": addr, "meta": ["symbol": meta.symbol, "decimals": meta.decimals]]
            }
            guard let answered = try? CoreJSON.object(core.resolveEffect(
                effectId: id,
                resultJson: CoreJSON.string(["type": "erc_meta", "chain_id": chainId, "entries": entries])
            )) else { return nil }
            step = answered
        }
        guard let view = step["view"] as? [String: Any],
              let trust = try? CoreJSON.decode(TrustViewWire.self, from: view)
        else { return nil }
        return trust.sim
    }

    /// A check under which nothing of the person's moves.
    @MainActor
    static func nothing(chainId: Int = 8453) -> TrustSimViewWire? { sim([], chainId: chainId) }

    static let weth = "0x4200000000000000000000000000000000000006"
    static let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"
    static let unknown = "0x5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"

    /// A verdict taller than the place the sheet keeps: three coins leave
    /// (the chain's own, WETH, USDC — outflows, so each is written) and a
    /// token nobody vouches for arrives — four rows, and the warning under
    /// an unverified token.
    @MainActor
    static func tall(chainId: Int = 8453) -> TrustSimViewWire? {
        sim([
            ["kind": "native", "token": NSNull(), "delta": "-1500000000000000000"],
            ["kind": "erc20", "token": weth, "delta": "-40000000000000000"],
            ["kind": "erc20", "token": usdc, "delta": "-25000000"],
            ["kind": "erc20", "token": unknown, "delta": "123456789"],
        ], chainId: chainId, named: [weth: ("WETH", 18), usdc: ("USDC", 6)])
    }
}

/// A site's transaction whose confirm waits for the simulation's verdict
/// (PR 3, fix C), as the real cores write it: the `sign_request` machine is
/// told the request and that its simulation is out (`sim_started`), the
/// `clear_signing` and `approval_guard` machines read the same request, and
/// the gate is the core's own `signConfirmState` over those views and a
/// quoted fee — nothing here builds a confirm state by hand.
///
/// No effect is run: the deadline the core asks for (`sim_verdict_timer`) is
/// answered only when `waitedOut` says it passed, so the held scene stays
/// held for as long as it is looked at.
enum SimWaitScene {
    struct Views {
        let request: SigningController.Incoming
        let sign: SignViewWire
        let clear: ClearSigningViewWire
        let guardView: GuardViewWire
        /// `signConfirmState` over the three views above and `fee`.
        let gate: SignConfirmStateWire
        /// The deadline the core asked for, as it asked.
        let timer: [String: Any]
    }

    static let recipient = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"

    /// `waitedOut`: the core's deadline passed with no verdict — the confirm
    /// is open and the view names the could-not-check line. Otherwise the
    /// simulation is still out and the confirm is held. `fee` is the fee
    /// view the gate reads beside them (a quoted one: only the verdict is
    /// missing).
    @MainActor
    static func views(
        waitedOut: Bool, chainId: Int = 8453, fee: String = HandoffFeeFixtures.feeJson
    ) -> Views? {
        let account = FeeCoreScene.account.lowercased()
        let call: [String: Any] = ["from": account, "to": recipient, "value": "0x14d1120d7b160000", "data": "0x"]
        guard let data = try? JSONSerialization.data(withJSONObject: [call]) else { return nil }
        let request = SigningController.Incoming(
            id: "pr3c-sim-wait", method: "eth_sendTransaction", paramsJson: String(decoding: data, as: UTF8.self),
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: chainId
        )
        let nowMs = Date().timeIntervalSince1970 * 1000

        // The request, as `SigningController.open` tells the machine of it.
        let signCore = SignRequestCore()
        let events: [[String: Any]] = [
            ["type": "networks_changed", "chain_ids": [chainId]],
            ["type": "accounts_changed",
             "accounts": [["address": account, "credential_id": "cred-1"]], "active_index": 0],
            ["type": "request_arrived", "id": request.id, "method": request.method,
             "params_json": request.paramsJson, "origin": request.origin, "transport_id": request.transportId,
             "first_party": false, "dedicated_transport": true, "per_request_chain": chainId,
             "dapp": NSNull(), "granted_address": account, "requested_address": NSNull(),
             "request_ts_ms": NSNull(), "now_ms": nowMs],
        ]
        for event in events {
            guard (try? signCore.dispatch(eventJson: CoreJSON.string(event))) != nil else { return nil }
        }
        // Its simulation is out: the core holds the confirm and asks for its
        // deadline.
        guard var step = try? CoreJSON.object(signCore.dispatch(eventJson: CoreJSON.string([
            "type": "sim_started", "id": request.id,
        ]))), let timerId = FeeCoreScene.effect("sim_verdict_timer", in: step),
            let timer = (step["effects"] as? [[String: Any]] ?? [])
                .compactMap({ $0["operation"] as? [String: Any] })
                .first(where: { $0["type"] as? String == "sim_verdict_timer" })
        else { return nil }
        if waitedOut {
            // The deadline passed: `SignExecutor`'s own answer to the timer.
            guard let fired = try? CoreJSON.object(signCore.resolveEffect(
                effectId: timerId, resultJson: SignExecutor.simVerdictTimerFired(timer)
            )) else { return nil }
            step = fired
        }

        // What it does, and what an approval in it may be edited to.
        guard let read = try? CoreJSON.object(ClearSigningCore().dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": recipient, "data": "0x", "value": call["value"] as Any,
            "chain_id": chainId, "locale": SigningController.defaultLocale,
        ]))), let guarded = try? CoreJSON.object(ApprovalGuardCore().dispatch(eventJson: CoreJSON.string([
            "type": "approval_detected", "method": request.method, "params_json": request.paramsJson,
            "chain_id": chainId, "wallet_address": account, "read_only": false, "now_ms": nowMs,
        ]))), let signView = step["view"] as? [String: Any],
            let clearView = read["view"] as? [String: Any],
            let guardView = guarded["view"] as? [String: Any],
            let sign = try? CoreJSON.decode(SignViewWire.self, from: signView),
            let clear = try? CoreJSON.decode(ClearSigningViewWire.self, from: clearView),
            let guardWire = try? CoreJSON.decode(GuardViewWire.self, from: guardView)
        else { return nil }
        return Views(
            request: request, sign: sign, clear: clear, guardView: guardWire,
            gate: SignConfirmStateWire.of(
                sign: CoreJSON.string(signView), guard: CoreJSON.string(guardView),
                clear: CoreJSON.string(clearView), fee: fee, speedTier: nil
            ),
            timer: timer
        )
    }
}

#endif
