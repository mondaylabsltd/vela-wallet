//
//  SignExecutor.swift
//  VelaWallet
//
//  The `sign_request` machine's seven arms, and the shell decides none of
//  them.
//
//  Single-flight, "a rejected pipeline may not submit", the record-then-respond
//  order and the account sequencing all live in the core. This answers the
//  transport, writes the record, asks the relay, and runs the ceremony —
//  through `UserOpSpine`, the **same** pipeline a person's own transfer runs.
//  Deliberately: a second signing path is a second set of rules about the
//  same Safe.
//
//  Ported from `app-android/.../feature/signing/core/SignExecutor.kt` (spec
//  044 T029).
//
//  ## It reports twice
//
//  The accepted user-op hash goes back mid-flight through `op_submitted`, so
//  the durable record precedes anything the dApp could poll. Then the final
//  outcome. The page is answered with a **transaction** hash once the receipt
//  arrives; the op hash only when it is late, and the tracker patches the row
//  when it lands.
//
//  Blocking the page on a receipt is what spec 028 found made
//  `eth_sendTransaction` time out with -32603 on a real dApp.
//

import Foundation
import VelaCore

@MainActor
final class SignExecutor {

    static let operations = [
        "send_response", "check_bundler_funding", "attempt_sponsorship",
        "sign_and_submit", "persist_record", "update_record", "switch_active_account",
    ]

    struct Ports {
        /// The answer, to the transport (tab) that owns the request.
        var respond: (_ transportId: String, _ id: String, _ json: [String: Any]) -> Void = { _, _, _ in }
        /// The relay accepted. The core must hear this **before** the submit
        /// resolves.
        var opSubmitted: (_ id: String, _ userOpHash: String) -> Void = { _, _ in }
        /// A ceremony is about to be raised.
        var signingStarted: () -> Void = {}
        /// The feed re-reads its store.
        var recordsPersisted: () -> Void = {}
        /// The session's active-account switch.
        var switchAccount: (_ index: Int) async -> Bool = { _ in false }
        /// The chain's native symbol, for the record row.
        var nativeSymbol: (_ chainId: Int) -> String = { _ in "" }
    }

    private let spine: UserOpSpine
    private let relay: RelayClient
    private let store: VelaStore
    var ports: Ports

    /// How long the final answer waits for a receipt before handing back the
    /// op hash — the desktop's `await_receipt`.
    private let receiptWaitMs: Double
    private let receiptPollMs: Double

    /// User-op hashes whose pending record is on disk. The response never
    /// precedes the record.
    private var persisted: Set<String> = []

    init(
        spine: UserOpSpine,
        relay: RelayClient,
        store: VelaStore,
        ports: Ports = Ports(),
        receiptWaitMs: Double = 120_000,
        receiptPollMs: Double = 3_000
    ) {
        self.spine = spine
        self.relay = relay
        self.store = store
        self.ports = ports
        self.receiptWaitMs = receiptWaitMs
        self.receiptPollMs = receiptPollMs
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "send_response":
            ports.respond(
                operation["transport_id"] as? String ?? "",
                operation["id"] as? String ?? "",
                Self.responseJson(
                    id: operation["id"] as? String ?? "",
                    payload: operation["payload"] as? [String: Any] ?? [:]
                )
            )
            return CoreJSON.string(["type": "responded"])

        // `null` means "proceed to submit" — including when the check itself
        // failed. The core's own doc is explicit that a timed-out or errored
        // pre-check is not a refusal, and the submit's underfunded answer is
        // the authority. Desktop and Android answer the same.
        case "check_bundler_funding":
            return CoreJSON.string(["type": "pre_check", "funding": NSNull()])

        // Denied with no reason rather than invented: sponsorship is a relay
        // feature this shell does not reach, and `funded` would be a claim
        // that somebody else paid.
        case "attempt_sponsorship":
            return CoreJSON.string([
                "type": "sponsorship",
                "outcome": ["type": "denied", "reason": NSNull()],
            ])

        case "sign_and_submit":
            return CoreJSON.string([
                "type": "submit",
                "outcome": await signAndSubmit(operation),
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])

        case "persist_record":
            let record = operation["record"] as? [String: Any] ?? [:]
            let row = Self.recordRow(
                record,
                nativeSymbol: ports.nativeSymbol((record["chain_id"] as? NSNumber)?.intValue ?? 0)
            )
            TxRecords.writeRecords([row], store: store)
            if let hash = record["user_op_hash"] as? String, !hash.isEmpty {
                persisted.insert(hash.lowercased())
            }
            ports.recordsPersisted()
            return CoreJSON.string(["type": "record_persisted"])

        case "update_record":
            let id = operation["record_id"] as? String ?? ""
            let close = operation["close"] as? [String: Any] ?? [:]
            var fields: [String: Any] = [:]
            if close["type"] as? String == "confirmed" {
                fields["status"] = "confirmed"
                if let txHash = close["tx_hash"] as? String, !txHash.isEmpty {
                    fields["txHash"] = txHash
                }
            } else {
                fields["status"] = "failed"
            }
            TxRecords.patch(ids: [id], fields: fields, store: store)
            ports.recordsPersisted()
            return CoreJSON.string(["type": "record_updated"])

        // Best effort, and the core is told either way: it sequences "switch
        // first, then the approval surface may act" off this acknowledgement,
        // so withholding it would strand the grant.
        case "switch_active_account":
            _ = await ports.switchAccount((operation["index"] as? NSNumber)?.intValue ?? 0)
            return CoreJSON.string(["type": "account_switched"])

        default:
            print("[vela-wallet] sign_request: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "send_response": return CoreJSON.string(["type": "responded"])
        case "check_bundler_funding": return CoreJSON.string(["type": "pre_check", "funding": NSNull()])
        case "attempt_sponsorship":
            return CoreJSON.string([
                "type": "sponsorship", "outcome": ["type": "denied", "reason": NSNull()],
            ])
        case "sign_and_submit":
            return CoreJSON.string([
                "type": "submit",
                "outcome": ["type": "failed", "message": "Signing failed"],
                "now_ms": Date().timeIntervalSince1970 * 1000,
            ])
        case "persist_record": return CoreJSON.string(["type": "record_persisted"])
        case "update_record": return CoreJSON.string(["type": "record_updated"])
        default: return CoreJSON.string(["type": "account_switched"])
        }
    }

    // MARK: - The ceremony

    private func signAndSubmit(_ operation: [String: Any]) async -> [String: Any] {
        let method = operation["method"] as? String ?? ""
        let paramsJson = operation["params_json"] as? String ?? "[]"
        let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
        let address = operation["address"] as? String ?? ""

        if method == "personal_sign" || method == "eth_sign" || method.contains("signTypedData") {
            return await signMessage(method: method, paramsJson: paramsJson, chainId: chainId, address: address)
        }

        guard let calls = Self.callsOf(method: method, paramsJson: paramsJson) else {
            return ["type": "failed", "message": "\(method) carried no transaction this wallet could read"]
        }

        let quoted = (operation["quoted_fee"] as? [String: Any]).map {
            UserOpSpine.Quoted(
                amount: $0["amount"] as? String ?? "0",
                recipient: $0["recipient"] as? String ?? ""
            )
        }

        do {
            let hash = try await spine.submit(
                chainId: chainId,
                account: address,
                calls: calls,
                gasFeeToken: operation["gas_fee_token"] as? String,
                quotedFee: quoted,
                signingStarted: { [ports] in ports.signingStarted() }
            )
            ports.opSubmitted(operation["id"] as? String ?? "", hash)
            // The durable record precedes anything the dApp could poll: the
            // core persists it on `op_submitted`, and the answer waits for it.
            await waitForRecord(of: hash)
            return ["type": "succeeded", "result": await awaitReceipt(chainId: chainId, userOpHash: hash) ?? hash]
        } catch let refused as UserOpSpine.Refused {
            switch refused.failure {
            case .passkeyCancelled:
                return ["type": "passkey_cancelled"]
            case .bundlerUnderfunded:
                return [
                    "type": "underfunded",
                    "message": "The relay's gas account is underfunded",
                    "funding": NSNull(),
                ]
            case .relayerUnavailable:
                return ["type": "failed", "message": "The gas relayer is unavailable right now"]
            case .other(let message):
                return ["type": "failed", "message": message ?? "Signing failed"]
            }
        } catch {
            return ["type": "failed", "message": "Signing failed"]
        }
    }

    /// A message: hashed the way the page's verifier hashes it, signed once,
    /// answered as the EIP-1271 envelope.
    private func signMessage(
        method: String, paramsJson: String, chainId: Int, address: String
    ) async -> [String: Any] {
        guard let original = Self.messageHash(method: method, paramsJson: paramsJson) else {
            return ["type": "failed", "message": "\(method) carried nothing this wallet could sign"]
        }
        do {
            let signature = try await spine.signMessage(
                chainId: chainId,
                account: address,
                originalHash: original,
                signingStarted: { [ports] in ports.signingStarted() }
            )
            return ["type": "succeeded", "result": signature]
        } catch let refused as UserOpSpine.Refused {
            if case .passkeyCancelled = refused.failure { return ["type": "passkey_cancelled"] }
            if case .other(let message) = refused.failure {
                return ["type": "failed", "message": message ?? "Signing failed"]
            }
            return ["type": "failed", "message": "Signing failed"]
        } catch {
            return ["type": "failed", "message": "Signing failed"]
        }
    }

    /// Bounded: five seconds, then answer anyway. The record is on its way and
    /// a page held open forever is worse than a page answered a moment early.
    private func waitForRecord(of hash: String) async {
        let key = hash.lowercased()
        let deadline = Date().addingTimeInterval(5)
        while !persisted.contains(key), Date() < deadline {
            try? await Task.sleep(nanoseconds: 50_000_000)
        }
    }

    private func awaitReceipt(chainId: Int, userOpHash: String) async -> String? {
        let deadline = Date().addingTimeInterval(receiptWaitMs / 1000)
        while Date() < deadline {
            if case .resolved(_, let txHash, _, _) = await relay.userOpReceipt(
                chainId: chainId, userOpHash: userOpHash
            ), !txHash.isEmpty {
                return txHash
            }
            try? await Task.sleep(nanoseconds: UInt64(receiptPollMs) * 1_000_000)
        }
        return nil
    }

    // MARK: - The pure parts

    /// What the page's verifier will hash.
    ///
    /// `personal_sign` is the EIP-191 prefix over the bytes; `eth_sign` is the
    /// same envelope over `params[1]` (EIP-1474's rule, the params swapped);
    /// typed data is its EIP-712 digest, computed by the core.
    static func messageHash(method: String, paramsJson: String) -> Data? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else { return nil }

        if method == "personal_sign" || method == "eth_sign" {
            let index = method == "eth_sign" ? 1 : 0
            guard params.count > index, let payload = params[index] as? String, !payload.isEmpty
            else { return nil }
            let bytes: Data = isHexPayload(payload)
                ? ((try? fromHex(s: payload)) ?? Data())
                : Data(payload.utf8)
            let prefix = Data("\u{19}Ethereum Signed Message:\n\(bytes.count)".utf8)
            return keccak256(data: prefix + bytes)
        }

        guard params.count > 1 else { return nil }
        let typed: String
        if let text = params[1] as? String {
            typed = text
        } else if let object = try? JSONSerialization.data(withJSONObject: params[1]),
                  let text = String(data: object, encoding: .utf8) {
            typed = text
        } else {
            return nil
        }
        return try? hashTypedData(typedDataJson: typed)
    }

    static func isHexPayload(_ payload: String) -> Bool {
        payload.hasPrefix("0x") && payload.count % 2 == 0
            && payload.dropFirst(2).allSatisfy { $0.isHexDigit }
    }

    /// The page's answer in the wire's shape; the core chose `ok`/`err` and
    /// the code.
    static func responseJson(id: String, payload: [String: Any]) -> [String: Any] {
        switch payload["type"] as? String ?? "" {
        case "ok":
            return BrowserExecutor.resultJson(id: id, result: payload["result"] as? String)
        case "err":
            let kind = payload["kind"] as? String ?? ""
            return BrowserExecutor.errorJson(
                id: id,
                code: (payload["code"] as? NSNumber)?.intValue ?? -32603,
                message: (payload["message"] as? String) ?? defaultMessage(kind)
            )
        default:
            return BrowserExecutor.errorJson(id: id, code: -32603, message: "The wallet produced no answer")
        }
    }

    static func defaultMessage(_ kind: String) -> String {
        switch kind {
        case "user_rejected": return "User rejected the request"
        case "wallet_switched_chains": return "The wallet switched chains"
        case "unsupported_chain": return "Unsupported chain"
        case "unauthorized_account": return "Unauthorized account"
        case "invalid_params": return "Invalid params"
        case "unsupported_capability": return "Unsupported capability"
        case "unlimited_approval": return "Unlimited approvals are disabled"
        case "funding_cancelled": return "Funding cancelled"
        case "submit_failed": return "The transaction could not be submitted"
        case "stale_fee_quote": return "The fee quote expired"
        default: return "The request was refused"
        }
    }

    /// The calls a request carries: one for `eth_sendTransaction`, many for
    /// `wallet_sendCalls` — and **an empty batch is not a batch**. Hex value
    /// on the wire, decimal to the core.
    static func callsOf(method: String, paramsJson: String) -> [UserOpCall]? {
        guard let data = paramsJson.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any],
              let first = params.first as? [String: Any]
        else { return nil }

        if method == "wallet_sendCalls" {
            guard let raw = first["calls"] as? [[String: Any]] else { return nil }
            let calls = raw.compactMap(call(from:))
            return calls.isEmpty ? nil : calls
        }
        return call(from: first).map { [$0] }
    }

    private static func call(from raw: [String: Any]) -> UserOpCall? {
        guard let to = raw["to"] as? String, !to.isEmpty else { return nil }
        let valueHex = (raw["value"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x0"
        guard let value = GuardExecutor.decimal(fromWordHex: padded(valueHex)) else { return nil }
        let dataHex = (raw["data"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x"
        return UserOpCall(to: to, value: value, data: dataHex)
    }

    /// The decimal converter wants whole bytes; a dApp writes `0x1`.
    private static func padded(_ hex: String) -> String {
        let digits = hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
        return digits.count % 2 == 0 ? "0x" + digits : "0x0" + digits
    }

    /// The feed's row for a dApp's request.
    ///
    /// A signature carries **no** value and **no** symbol: it moves nothing,
    /// and a row that claimed a value would show up in the feed as money.
    static func recordRow(_ record: [String: Any], nativeSymbol: String) -> [String: Any] {
        let paramsJson = record["params_json"] as? String ?? "[]"
        let first = (try? JSONSerialization.jsonObject(
            with: Data(paramsJson.utf8)
        )).flatMap { ($0 as? [Any])?.first as? [String: Any] }

        let kind: String
        var to = ""
        var value = "0"
        var symbol = ""
        var decimals = 0
        switch record["kind"] as? String ?? "" {
        case "dapp_tx":
            kind = "dapp_tx"
            to = first?["to"] as? String ?? ""
            value = (first?["value"] as? String).flatMap { $0.isEmpty ? nil : $0 } ?? "0x0"
            symbol = nativeSymbol
            decimals = 18
        case "sign_typed_data":
            kind = "sign_typed_data"
        default:
            kind = "sign_message"
        }

        let clipped = paramsJson.utf8.count > 4096
        var row: [String: Any] = [
            "id": record["record_id"] as? String ?? "",
            "userOpHash": record["user_op_hash"] as? String ?? "",
            "txHash": record["result"] as? String ?? "",
            "from": record["from"] as? String ?? "",
            "to": to,
            "value": value,
            "symbol": symbol,
            "decimals": decimals,
            "chainId": (record["chain_id"] as? NSNumber)?.intValue ?? 0,
            // SECONDS.
            "timestamp": Int(((record["now_ms"] as? NSNumber)?.doubleValue ?? 0) / 1000),
            "status": (record["status"] as? String) == "confirmed" ? "confirmed" : "pending",
            "type": kind,
            "dappOrigin": record["dapp_origin"] as? String ?? "",
            "signedRequest": clipped ? String(paramsJson.prefix(4096)) : paramsJson,
            "requestTruncated": clipped,
        ]
        if let intent = record["intent"] as? String, !intent.isEmpty { row["intent"] = intent }
        return row
    }
}
