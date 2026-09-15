//
//  Multicall.swift
//  VelaWallet
//
//  One batched `eth_call`, routed.
//
//  Ported from `app-web/vela-wallet/src/lib/services/abi.ts`' `MULTICALL3` +
//  `encAggregate3`/`decAggregate3` pairing, which every reader on web reaches
//  through. Three callers share it here — balances, native prices and the fiat
//  feeds — and they share it for the reason the pool exists: one shape for
//  "ask a chain several questions at once", so a ban, a rate limit and a
//  garbled reply mean the same thing to all of them.
//
//  The ENCODING itself is not here. `multicall3_encode_aggregate3` and its
//  decoder are `vela-core-uniffi`'s (spec 051 research D1): `aggregate3(
//  (address,bool,bytes)[])` is a dynamic array of tuples with a dynamic member,
//  and a hand-rolled Swift encoder is correct right up until the first tuple
//  with two dynamic members.
//

import Foundation
import VelaCore

enum Multicall {

    /// Multicall3 — the same address on every chain that has it.
    static let address = "0xcA11bde05977b3631167028862bE2a173976CA11"

    /// What a batch came back as.
    ///
    /// A garbled reply is `failed`, not an empty list: "no results" is a claim
    /// about the chain, and bytes this build cannot read are not evidence for
    /// it.
    enum Outcome {
        case ok([Multicall3Result])
        /// `rateLimited` travels because the two get different screens —
        /// invariant ④ forbids offering "swap in your own RPC" to somebody the
        /// chain is merely throttling.
        case failed(rateLimited: Bool)
    }

    /// Encode, route and decode one `aggregate3`.
    ///
    /// `allowFailure` is the caller's per-call choice, but every caller here
    /// sets it: one reverting contract in a batch of twelve must not cost the
    /// other eleven their answer, and a failed entry **keeps its slot** so
    /// results still match calls by index.
    static func aggregate3(
        chainId: Int,
        calls: [Multicall3Call],
        pool: RpcPool
    ) async -> Outcome {
        guard !calls.isEmpty else { return .ok([]) }
        guard let calldata = try? multicall3EncodeAggregate3(calls: calls) else {
            return .failed(rateLimited: false)
        }

        let outcome = await pool.call(
            chainId: chainId,
            method: "eth_call",
            params: [["to": address, "data": "0x" + calldata.hexString], "latest"]
        )
        guard case .ok(let value) = outcome else {
            if case .failed(let throttled) = outcome {
                return .failed(rateLimited: throttled)
            }
            return .failed(rateLimited: false)
        }
        guard let hex = value as? String,
              let data = Data(hexString: hex),
              let results = try? multicall3DecodeAggregate3(data: data)
        else {
            return .failed(rateLimited: false)
        }
        return .ok(results)
    }

    /// `f(...)` → its four-byte selector, from the bridge rather than a pasted
    /// constant: a signature typo is then a `nil` here instead of a call that
    /// reverts on-chain (FR-006).
    static func selector(_ signature: String) -> Data? {
        try? functionSelector(signature: signature)
    }

    /// One call in a batch, with the failure allowance every caller here wants.
    static func call(_ target: String, _ callData: Data) -> Multicall3Call {
        Multicall3Call(target: target, allowFailure: true, callData: callData)
    }
}
