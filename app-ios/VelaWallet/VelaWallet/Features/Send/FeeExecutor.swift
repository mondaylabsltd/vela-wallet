//
//  FeeExecutor.swift
//  VelaWallet
//
//  The only place the `fee_policy` core touches the outside world.
//
//  Seven operations, every one of them a read. Ported from
//  `app-android/.../feature/send/core/FeeExecutor.kt` (spec 043), which is the
//  desktop's `executor/fee.rs`.
//
//  ## Nothing here is a number this shell chose
//
//  A `nil` from any of these is a FACT the core degrades on — only a failed
//  `eth_gasPrice` triggers its five-gwei default, and it is the core that
//  decides that, not this file. A shell that substituted its own figure would
//  be a second opinion nobody diffed, and the figure in question is what
//  somebody pays.
//

import Foundation
import VelaCore

@MainActor
final class FeeExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "fetch_gas_price",
        "fetch_bundler_quote",
        "fetch_in_band_quotes",
        "fetch_fee_recipient",
        "estimate_user_op_gas",
        "measure_inner_calls",
        "start_ttl",
    ]

    /// `eth_estimateGas({from, to, value, data})` for one call on one chain —
    /// the gas hex, or `nil` when nobody answered. The same read the spine's
    /// `measureCall` makes at submit.
    typealias MeasureCall = (_ chainId: Int, _ from: String, _ to: String, _ valueHex: String, _ data: String) async -> String?

    /// The pool's `eth_estimateGas`, as both the spine and this executor read it.
    static func measuring(with pool: RpcPool) -> MeasureCall {
        { chainId, from, to, valueHex, data in
            let outcome = await pool.call(
                chainId: chainId, method: "eth_estimateGas",
                params: [["from": from, "to": to, "value": valueHex, "data": data]]
            )
            guard case .ok(let value) = outcome, let hex = value as? String, hex.hasPrefix("0x") else { return nil }
            return hex
        }
    }

    private let relay: RelayClient
    /// How a draft is assembled for the estimate. The spine owns the order;
    /// this only needs the JSON the bundler is asked about.
    private let accounts: UserOpSpine.AccountPort
    /// The inner calls' own gas (issue #262 follow-up): the quote measures what
    /// the submit measures, so the fee is priced on the `callGasLimit` the op
    /// will carry. Unwired = nothing measured = the relay's figure.
    private let measureCall: MeasureCall

    init(
        relay: RelayClient,
        accounts: UserOpSpine.AccountPort,
        measureCall: @escaping MeasureCall = { _, _, _, _, _ in nil }
    ) {
        self.relay = relay
        self.accounts = accounts
        self.measureCall = measureCall
    }

    func perform(_ operation: [String: Any]) async -> String {
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        switch operation["type"] as? String ?? "" {

        case "fetch_gas_price":
            // `want_tip` is false on Tempo: `eth_maxPriorityFeePerGas` is
            // meaningless there and asking corrupts the stablecoin
            // reimbursement. The core decides; this obeys.
            let signals = await relay.gasSignals(
                chainId: chainId, wantTip: operation["want_tip"] as? Bool ?? true
            )
            return CoreJSON.string([
                "type": "gas_price",
                "eth_gas_price": signals.ethGasPrice.map { $0 as Any } ?? NSNull(),
                "base_fee": signals.baseFee.map { $0 as Any } ?? NSNull(),
                "priority_fee": signals.priorityFee.map { $0 as Any } ?? NSNull(),
            ])

        case "fetch_bundler_quote":
            let tier = operation["tier"] as? String ?? "fast"
            return CoreJSON.string([
                "type": "bundler_quote",
                "quote": await relay.bundlerQuote(chainId: chainId, tier: tier)
                    .map { $0 as Any } ?? NSNull(),
            ])

        case "fetch_in_band_quotes":
            let account = operation["account"] as? String ?? ""
            return CoreJSON.string([
                "type": "in_band_quotes",
                "quotes": await relay.inBandQuotes(chainId: chainId, safe: account)
                    .map { $0 as Any } ?? NSNull(),
            ])

        case "fetch_fee_recipient":
            let account = operation["account"] as? String ?? ""
            return CoreJSON.string([
                "type": "fee_recipient",
                "recipient": await relay.accountInfo(chainId: chainId, safe: account)?
                    .feeRecipient.map { $0 as Any } ?? NSNull(),
            ])

        case "estimate_user_op_gas":
            return await estimate(operation, chainId: chainId)

        case "measure_inner_calls":
            return await measure(operation, chainId: chainId)

        case "start_ttl":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string(["type": "ttl_elapsed"])

        default:
            // Answered, loudly. An unanswered operation leaves the machine
            // waiting and the confirm screen's slide disabled forever, which
            // looks exactly like a wallet that refuses to send.
            print("[vela-wallet] fee_policy: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "context_unavailable"]])
        }
    }

    /// Simulate the REAL batch — the person's calls plus the fee leg.
    ///
    /// The draft is built with a dummy signature, which is what makes the
    /// estimate honest: a verification cost measured without one is a cost the
    /// submitted operation will exceed.
    ///
    /// **One simulation per operation** (spec 078): the speed in force and
    /// every tier preview ask this for the same calls at the same instant —
    /// nothing in it depends on speed — so they share one flight and, for the
    /// fee-signals window, one held answer (`RelayClient.sharedSimulation`).
    /// Only an `estimated` outcome is held; a refusal or an unreachable relay
    /// is shared with whoever asked at that moment and asked again after.
    private func estimate(_ operation: [String: Any], chainId: Int) async -> String {
        let account = operation["account"] as? String ?? ""
        let deployed = operation["deployed"] as? Bool ?? false
        let key = Self.operationKey(
            account: account, deployed: deployed,
            calls: operation["calls"] as? [[String: Any]] ?? []
        )
        return await relay.sharedSimulation(
            chainId: chainId,
            key: key,
            cacheable: { $0.contains("\"estimated\"") },
            run: { await self.simulate(operation, chainId: chainId, account: account, deployed: deployed) }
        )
    }

    /// The operation a simulation is about, as one string: the account, the
    /// deployment and the calls byte for byte, keys sorted so the same
    /// operation always spells the same key.
    static func operationKey(account: String, deployed: Bool, calls: [[String: Any]]) -> String {
        let object: [String: Any] = [
            "account": account.lowercased(), "deployed": deployed, "calls": calls,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        else { return "\(account.lowercased()):\(deployed):\(calls.count)" }
        return String(decoding: data, as: UTF8.self)
    }

    private func simulate(
        _ operation: [String: Any], chainId: Int, account: String, deployed: Bool
    ) async -> String {
        let calls = (operation["calls"] as? [[String: Any]] ?? []).map { call in
            UserOpCall(
                to: call["to"] as? String ?? "",
                value: call["value"] as? String ?? "0",
                data: call["data"] as? String ?? "0x"
            )
        }
        let keys = await accounts.keys(of: account)
        guard !keys.isEmpty else {
            // No key means no initCode and no signature shape, so there is
            // nothing to simulate. `context_unavailable` is the core's own word
            // for it (invariant ⑤).
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "context_unavailable"]])
        }
        let nonce = deployed
            ? (await relay.nonce(chainId: chainId, sender: account) ?? "0x0")
            : "0x0"
        let floors = userOpFloors(
            chainId: UInt32(chainId), deployed: deployed, subCalls: UInt32(calls.count + 1)
        )
        guard let draft = try? userOpDraft(
            sender: account, nonce: nonce, deployed: deployed,
            keyHexes: keys.map(\.publicKeyHex), calls: calls,
            fee: .estimateOnly, floors: floors
        ), let json = try? userOpRelayJson(draft: draft, feeToken: nil) else {
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "context_unavailable"]])
        }
        switch await relay.estimateUserOpGas(chainId: chainId, opJson: json) {
        case .estimated(let verification, let call, let preVerification):
            return CoreJSON.string([
                "type": "user_op_gas",
                "outcome": [
                    "type": "estimated",
                    "verification_gas_limit": verification,
                    "call_gas_limit": call,
                    "pre_verification_gas": preVerification,
                ],
            ])
        case .refused:
            // The relay looked at THIS operation and said no. The core reads
            // that as a simulation failure and falls back to its own floors.
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "simulation_failed"]])
        case .unreachable:
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "context_unavailable"]])
        }
    }

    /// Each call on its own, from the Safe's address — one entry per call, in
    /// order, a decimal string or `null`. What an unmeasured call means (no
    /// floor, the relay's figure) is the core's.
    private func measure(_ operation: [String: Any], chainId: Int) async -> String {
        let from = operation["from"] as? String ?? ""
        let calls = operation["calls"] as? [[String: Any]] ?? []
        var gas: [Any] = []
        for call in calls {
            let value = call["value"] as? String ?? "0"
            let data = call["data"] as? String ?? "0x"
            let to = call["to"] as? String ?? ""
            guard let valueHex = UserOpSpine.decimalToHex(value).map({ "0x" + $0 }),
                  let hex = await measureCall(chainId, from, to, valueHex, data),
                  let measured = UInt64(hex.dropFirst(hex.hasPrefix("0x") ? 2 : 0), radix: 16)
            else {
                gas.append(NSNull())
                continue
            }
            gas.append(String(measured))
        }
        return CoreJSON.string(["type": "inner_calls_measured", "gas": gas])
    }

    /// What the core hears when an arm threw: nothing was done.
    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "fetch_gas_price":
            return CoreJSON.string([
                "type": "gas_price", "eth_gas_price": NSNull(),
                "base_fee": NSNull(), "priority_fee": NSNull(),
            ])
        case "fetch_bundler_quote":
            return CoreJSON.string(["type": "bundler_quote", "quote": NSNull()])
        case "fetch_in_band_quotes":
            return CoreJSON.string(["type": "in_band_quotes", "quotes": NSNull()])
        case "fetch_fee_recipient":
            return CoreJSON.string(["type": "fee_recipient", "recipient": NSNull()])
        case "measure_inner_calls":
            let count = (operation["calls"] as? [Any])?.count ?? 0
            return CoreJSON.string([
                "type": "inner_calls_measured",
                "gas": Array(repeating: NSNull(), count: count) as [Any],
            ])
        case "start_ttl":
            return CoreJSON.string(["type": "ttl_elapsed"])
        default:
            return CoreJSON.string(["type": "user_op_gas", "outcome": ["type": "context_unavailable"]])
        }
    }
}
