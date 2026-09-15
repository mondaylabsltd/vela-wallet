//
//  BrowserExecutor.swift
//  VelaWallet
//
//  The `dapp_permissions` machine's eight operations, and every envelope this
//  app ever puts on the page channel.
//
//  Ported from `app-android/.../feature/browser/core/BrowserExecutor.kt`
//  (spec 044), which is the desktop's `executor/dapp_permissions.rs`. The
//  envelope shapes are in `contracts/page-envelope.md` and are the same on all
//  four clients — a fifth spelling would be a fifth set of answers to
//  questions about somebody's money.
//
//  ## Two delivery rules live here, and they differ on purpose
//
//  `respond` goes to the tab that **asked** — the owner keeps request id → tab
//  id. `emit_event` goes to the **current** tab only: a page that is not in
//  front of the person is not told about an account switch it never asked
//  about. Collapsing the two into "send to the browser" is an easy refactor
//  and a real bug.
//
//  ## A revoked grant is stored as an empty string
//
//  `remove_grant` writes `""` rather than deleting the key, ported verbatim.
//  Whatever reads it must treat an empty string as **absent**, or a revoked
//  origin comes back granted.
//

import Foundation

@MainActor
final class BrowserExecutor {

    static let operations = [
        "read_grant", "write_grant", "remove_grant", "respond", "emit_event",
        "settle_forwarded", "save_connection_record", "forward_to_signing",
    ]

    /// What the app owns: the page channel, the router, the feed.
    struct Ports {
        /// Deliver an answer to the tab that asked.
        var respond: (_ id: String, _ json: [String: Any]) -> Void = { _, _ in }
        /// Push an EIP-1193 event to the page in front of the person.
        var emit: (_ json: [String: Any]) -> Void = { _ in }
        /// Error every request still open on the transport.
        var settleForwarded: (_ code: Int, _ message: String) -> Void = { _, _ in }
        /// Write the "connected to" row.
        var saveConnectionRecord: (_ row: [String: Any]) -> Void = { _ in }
        /// Hand a non-consent request to the router.
        var forward: (_ id: String, _ method: String, _ paramsJson: String, _ origin: String) -> Void
            = { _, _, _, _ in }
    }

    private let store: VelaStore
    var ports: Ports

    init(store: VelaStore, ports: Ports = Ports()) {
        self.store = store
        self.ports = ports
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "read_grant":
            let origin = operation["origin"] as? String ?? ""
            return CoreJSON.string([
                "type": "grant_read",
                "origin": origin,
                "grant": readGrant(origin: origin) ?? NSNull(),
            ])

        case "write_grant":
            if let grant = operation["grant"] as? [String: Any],
               let origin = grant["origin"] as? String, !origin.isEmpty,
               let text = try? JSONSerialization.data(withJSONObject: grant),
               let json = String(data: text, encoding: .utf8) {
                store.writeString(Self.grantKey(origin), json)
            }
            return CoreJSON.string(["type": "ack"])

        case "remove_grant":
            let origin = operation["origin"] as? String ?? ""
            if !origin.isEmpty { store.writeString(Self.grantKey(origin), "") }
            return CoreJSON.string(["type": "ack"])

        case "respond":
            let id = operation["id"] as? String ?? ""
            let payload = operation["payload"] as? [String: Any] ?? [:]
            ports.respond(id, Self.responseJson(id: id, payload: payload))
            return CoreJSON.string(["type": "ack"])

        case "emit_event":
            if let event = operation["event"] as? [String: Any] {
                ports.emit(Self.eventJson(event))
            }
            return CoreJSON.string(["type": "ack"])

        case "settle_forwarded":
            let code = (operation["code"] as? NSNumber)?.intValue ?? 4900
            let reason = operation["reason"] as? String ?? ""
            ports.settleForwarded(code, Self.rejectMessage(reason))
            return CoreJSON.string(["type": "ack"])

        case "save_connection_record":
            ports.saveConnectionRecord(Self.connectionRow(
                address: operation["address"] as? String ?? "",
                chainId: (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                origin: operation["origin"] as? String ?? "",
                nowMs: Date().timeIntervalSince1970 * 1000
            ))
            return CoreJSON.string(["type": "ack"])

        case "forward_to_signing":
            ports.forward(
                operation["id"] as? String ?? "",
                operation["method"] as? String ?? "",
                operation["params_json"] as? String ?? "[]",
                operation["origin"] as? String ?? ""
            )
            return CoreJSON.string(["type": "ack"])

        default:
            print("[vela-wallet] dapp_permissions: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "read_grant":
            return CoreJSON.string([
                "type": "grant_read",
                "origin": operation["origin"] as? String ?? "",
                "grant": NSNull(),
            ])
        default:
            return CoreJSON.string(["type": "ack"])
        }
    }

    /// A stored grant, or `nil` for absent, empty (revoked) or unreadable.
    private func readGrant(origin: String) -> [String: Any]? {
        guard !origin.isEmpty,
              let text = store.readString(Self.grantKey(origin)),
              !text.isEmpty,
              let data = text.data(using: .utf8),
              let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }
        return object
    }

    // MARK: - The envelopes

    /// One document per origin, the desktop's key.
    static func grantKey(_ origin: String) -> String { "vela.perm.\(origin)" }

    /// The answer to one request. The core chose `ok`/`err` and the code.
    static func responseJson(id: String, payload: [String: Any]) -> [String: Any] {
        switch payload["type"] as? String ?? "" {
        case "accounts":
            return resultJson(id: id, result: payload["addresses"] as? [String] ?? [])
        case "permissions":
            // EIP-2255: granted is one capability, refused is an empty list.
            let granted = payload["granted"] as? Bool ?? false
            return resultJson(id: id, result: granted ? [["parentCapability": "eth_accounts"]] : [])
        case "error":
            return errorJson(
                id: id,
                code: (payload["code"] as? NSNumber)?.intValue ?? 4001,
                message: rejectMessage(payload["reason"] as? String ?? "")
            )
        default:
            return errorJson(id: id, code: -32603, message: "The wallet produced no answer")
        }
    }

    static func resultJson(id: String, result: Any?) -> [String: Any] {
        ["dir": "res", "id": id, "result": result ?? NSNull()]
    }

    static func errorJson(id: String, code: Int, message: String) -> [String: Any] {
        ["dir": "res", "id": id, "error": ["code": code, "message": message]]
    }

    static func eventJson(_ event: [String: Any]) -> [String: Any] {
        switch event["type"] as? String ?? "" {
        case "accounts_changed":
            return ["dir": "evt", "event": "accountsChanged", "data": event["addresses"] as? [String] ?? []]
        case "chain_changed":
            return ["dir": "evt", "event": "chainChanged", "data": event["chain_id_hex"] as? String ?? "0x0"]
        default:
            return ["dir": "evt", "event": "disconnect"]
        }
    }

    /// Nine reasons, nine sentences. The core owns which reason; these are the
    /// shell's words for it.
    static func rejectMessage(_ reason: String) -> String {
        switch reason {
        case "unauthorized_frame": return "Unauthorized frame"
        case "no_account_available": return "No wallet account available"
        case "consent_busy": return "Another connection request is open"
        case "insecure_origin": return "Signing requires a secure origin"
        case "user_rejected": return "User rejected the request"
        case "navigated_away": return "The page navigated away"
        case "browser_closed": return "The browser was closed"
        case "not_connected": return "This site is not connected"
        case "stale_authorized_address": return "The authorized address changed"
        default: return "The request was refused"
        }
    }

    /// The feed's "connected to" row.
    ///
    /// No hashes and no value, because nothing moved — and `type: "connect"`
    /// so 052's `load_send_history`, which narrows to `send`, can never mistake
    /// it for somebody the person has paid.
    static func connectionRow(address: String, chainId: Int, origin: String, nowMs: Double) -> [String: Any] {
        [
            "id": "dapp-\(Int(nowMs))-connect",
            "userOpHash": "",
            "txHash": "",
            "from": address,
            "to": "",
            "value": "0",
            "symbol": "",
            "decimals": 0,
            "chainId": chainId,
            // SECONDS. Milliseconds here puts every connection in the year
            // 57000 and silently breaks the feed's day grouping.
            "timestamp": Int(nowMs / 1000),
            "status": "confirmed",
            "type": "connect",
            "dappOrigin": origin,
        ]
    }
}
