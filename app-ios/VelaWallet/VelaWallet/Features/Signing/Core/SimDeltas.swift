//
//  SimDeltas.swift
//  VelaWallet
//
//  The transaction simulation's SHELL half (spec 055 D2).
//
//  Ported from `app-desktop/.../executor/sim.rs` and
//  `app-android/.../feature/signing/core/SimDeltas.kt`. One job since spec 082:
//  build the `eth_simulateV1` payload for a request's calls. Reading the reply
//  — a revert, a node that cannot simulate, the person's net moves — is the
//  core's `simOutcome` (RG6); the parser that lived here moved there with its
//  vectors (T039), because three shells parsed it three ways and none saw a
//  revert.
//
//  **What the deltas MEAN is the core's** (`token_trust::judge_delta`), and its
//  judgment is asymmetric on purpose: an outflow renders whenever the token's
//  metadata resolved, because the real token emits its own log and an outflow
//  cannot be understated; an INFLOW renders a confident number only for a token
//  already in the trusted set, because a log is something anybody can emit.
//
//  **Nothing here is ever written anywhere.** A simulated delta may not add a
//  token to somebody's list — that entrance belongs to the confirmed receipt
//  alone (spec 017, invariant ⑤).
//

import Foundation
import VelaCore

enum SimDeltas {

    /// A simulated balance change as the sheet writes it — the core's
    /// `formatSignedTokenAmount` (spec 082 RJ15, G49): the token ladder in
    /// the person's number preset, U+2212 for a minus, a dust amount written
    /// exactly (1000 wei is never "−0"), and `nil` for a zero change, which is
    /// not drawn at all. `delta` is the core's signed base units ("-1000").
    static func deltaText(_ delta: String, decimals: Int, preset: String? = nil) -> String? {
        formatSignedTokenAmount(
            deltaBaseUnits: delta,
            decimals: UInt32(clamping: max(0, decimals)),
            preset: preset ?? Formats.resolve(Formats.current.number).rawValue
        )
    }

    /// A clean run's deltas (the core's `TrustAssetDelta` JSON) as the fee
    /// machine reads them (spec 083 fee, issue #411; the desktop's
    /// `fee_balance_changes`): what the operation moves of each asset, native
    /// or by contract. The core derived each from the coin's OWN `Transfer`
    /// logs (the contract that emitted them) or the node's trace of native
    /// value — nothing a site's contract can emit on a coin's behalf. A token
    /// move with no contract names no coin and is dropped: never read as the
    /// native one.
    ///
    /// `nil` when a delta cannot be read (a kind this build has no name for,
    /// a missing amount): moves nobody could read are no measurement, and the
    /// machine is told nothing rather than "this coin does not move".
    static func feeBalanceChanges(_ deltas: [[String: Any]]) -> [[String: Any]]? {
        var changes: [[String: Any]] = []
        for delta in deltas {
            guard let amount = delta["delta"] as? String else { return nil }
            switch delta["kind"] as? String {
            case "native":
                changes.append(["token": NSNull(), "delta": amount])
            case "erc20":
                guard let token = delta["token"] as? String, !token.isEmpty else { continue }
                changes.append(["token": token, "delta": amount])
            default:
                return nil
            }
        }
        return changes
    }

    /// One leg of the operation being simulated.
    struct Call {
        let to: String
        let value: String?
        let data: String?
    }

    /// The `eth_simulateV1` params: ONE block-state call carrying every leg, so
    /// the legs see each other's effects — which is the whole reason to
    /// simulate a batch rather than each call alone.
    ///
    /// `nil` when there is nothing to simulate.
    static func payload(from: String, calls: [Call]) -> [Any]? {
        guard let body = body(from: from, calls: calls) else { return nil }
        return [body, "latest"]
    }

    static func body(from: String, calls: [Call]) -> [String: Any]? {
        guard let first = calls.first, !first.to.isEmpty else { return nil }
        let entries: [[String: Any]] = calls.map { call in
            var entry: [String: Any] = [
                "from": from, "to": call.to, "value": hexValue(call.value),
            ]
            let data = call.data ?? ""
            if !data.isEmpty, data != "0x" { entry["data"] = data }
            return entry
        }
        return [
            "blockStateCalls": [["calls": entries]],
            // No nonce or balance checks: the question is what the calls DO,
            // not whether this account could pay for them right now.
            "validation": false,
            // Native moves become logs, which is what makes them countable.
            "traceTransfers": true,
            "returnFullTransactions": false,
        ]
    }

    /// A value as the node wants it: `0x` hex, decimal input converted, empty
    /// and unreadable both `0x0` — a call with no value moves none.
    static func hexValue(_ value: String?) -> String {
        let raw = (value ?? "").trimmingCharacters(in: .whitespaces)
        guard !raw.isEmpty else { return "0x0" }
        if raw.hasPrefix("0x") || raw.hasPrefix("0X") {
            let digits = String(raw.dropFirst(2).drop { $0 == "0" })
            return "0x" + (digits.isEmpty ? "0" : digits)
        }
        guard let parsed = SignedDigits(raw), !parsed.negative else { return "0x0" }
        return "0x" + hexDigits(parsed)
    }

    /// A non-negative decimal string as lowercase hex, by repeated halving —
    /// the values are past `UInt64` and this is the only conversion needed.
    private static func hexDigits(_ value: SignedDigits) -> String {
        var digits = Array(value.digits).map { $0.wholeNumberValue ?? 0 }
        guard digits.contains(where: { $0 != 0 }) else { return "0" }
        var out = ""
        while digits.contains(where: { $0 != 0 }) {
            var remainder = 0
            var next: [Int] = []
            for digit in digits {
                let current = remainder * 10 + digit
                next.append(current / 16)
                remainder = current % 16
            }
            out.append(Character(String(remainder, radix: 16)))
            while next.first == 0, next.count > 1 { next.removeFirst() }
            digits = next
        }
        return String(out.reversed())
    }
}
