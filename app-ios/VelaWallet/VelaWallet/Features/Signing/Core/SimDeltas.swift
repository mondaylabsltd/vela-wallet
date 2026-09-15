//
//  SimDeltas.swift
//  VelaWallet
//
//  The transaction simulation's SHELL half (spec 055 D2).
//
//  Ported from `app-desktop/.../executor/sim.rs` and
//  `app-android/.../feature/signing/core/SimDeltas.kt`. Two jobs and no third:
//  build the `eth_simulateV1` payload for a request's calls, and net the
//  `Transfer` logs of the calls that succeeded into per-token deltas.
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

enum SimDeltas {

    /// `keccak256("Transfer(address,address,uint256)")`.
    static let transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    /// The sender a node reports for a NATIVE value move under
    /// `traceTransfers` — not a contract, a sentinel.
    static let nativeSentinel = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"

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

    /// The logs of every SUCCEEDED call in every simulated block.
    ///
    /// `nil` when the node answered an error or nothing at all — which the
    /// sheet must show as "could not check", never as "nothing moves".
    static func logsOf(_ result: [String: Any]) -> [[String: Any]]? {
        if let error = result["error"], !(error is NSNull) { return nil }
        guard let blocks = result["result"] as? [[String: Any]], !blocks.isEmpty else { return nil }
        var logs: [[String: Any]] = []
        for block in blocks {
            guard let calls = block["calls"] as? [[String: Any]] else { continue }
            for call in calls where succeeded(call) {
                logs += (call["logs"] as? [[String: Any]]) ?? []
            }
        }
        return logs
    }

    /// A call is a success unless the node says otherwise. Two spellings of the
    /// status are in the wild (`"0x1"` and a number), and a call with neither
    /// and no error is one that ran.
    private static func succeeded(_ call: [String: Any]) -> Bool {
        if let status = call["status"] as? String { return status == "0x1" }
        if let status = call["status"] as? NSNumber { return status.intValue == 1 }
        let error = call["error"]
        return error == nil || error is NSNull
    }

    /// Net `Transfer` deltas for one address, in first-seen order, zeros
    /// dropped.
    ///
    /// In-and-out of the same token nets to nothing, and printing a pair of
    /// moves that cancel is how a swap looks like a theft.
    static func deriveDeltas(logs: [[String: Any]], user: String) -> [[String: Any]] {
        let me = user.lowercased()
        var order: [String] = []
        var totals: [String: SignedDigits] = [:]

        for log in logs {
            guard let topics = log["topics"] as? [String], topics.count == 3,
                  topics[0].caseInsensitiveCompare(transferTopic) == .orderedSame
            else { continue }
            let from = topicAddress(topics[1])
            let to = topicAddress(topics[2])
            guard from == me || to == me else { continue }
            guard let value = SignedDigits.firstWord(hex: log["data"] as? String ?? ""),
                  !value.isZero
            else { continue }
            let address = (log["address"] as? String ?? "").lowercased()
            guard !address.isEmpty else { continue }
            let key = address == nativeSentinel ? "native" : address
            if totals[key] == nil { order.append(key) }
            var total = totals[key] ?? .zero
            if to == me { total = total.adding(value) }
            if from == me { total = total.subtracting(value) }
            totals[key] = total
        }

        return order.compactMap { key in
            guard let total = totals[key], !total.isZero else { return nil }
            return key == "native"
                ? ["kind": "native", "token": NSNull(), "delta": total.text]
                : ["kind": "erc20", "token": key, "delta": total.text]
        }
    }

    /// A 32-byte topic → the address in its low 20 bytes, lowercase.
    static func topicAddress(_ topic: String) -> String {
        let hex = (topic.hasPrefix("0x") ? String(topic.dropFirst(2)) : topic).lowercased()
        return hex.count >= 40 ? "0x" + String(hex.suffix(40)) : ""
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
