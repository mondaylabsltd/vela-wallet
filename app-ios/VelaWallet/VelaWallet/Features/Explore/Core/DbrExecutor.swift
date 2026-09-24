//
//  DbrExecutor.swift
//  VelaWallet
//
//  The `dapp_browser` machine's ten operations (spec 070), and nothing that
//  decides.
//
//  Before 070 this file was `BrowserExecutor` + `RequestRouter` + `DappRpc`:
//  a routing table, a request router, a request→tab map and a set of open ids,
//  each a Swift copy of a rule three other clients also copied — and each
//  drifted (`eth_sign` answered "disconnected", `wallet_addEthereumChain`
//  answered `null` and switched nothing, a background tab's navigation settled
//  the front tab's requests). The core owns all of it now. What is left here
//  is I/O: the grant and chain keys, one string posted into one tab's page, a
//  read through the person's own endpoints, a receipt lookup, and the hand-off
//  to the signing sheet.
//
//  ## Every operation is answered, whatever happens
//
//  A JSON tag has no exhaustiveness check, and an operation nobody answers is
//  a page promise that never settles. So every branch returns, an unknown tag
//  gets `neutralAnswer`, and the controller hands the same function to the
//  driver for an answer the core refuses (`CoreStore`'s `neutralAnswer`). The
//  neutral answers are the contract's: an empty site list, `ack`, a read
//  nobody answered (-32603 to the page), a receipt not landed yet (`null`).
//

import Foundation

@MainActor
final class DbrExecutor {

    static let operations = [
        "list_sites", "write_grant", "remove_grant", "write_site_chain", "deliver",
        "read", "resolve_user_op", "forward_to_signing", "cancel_signing",
        "save_connection_record",
    ]

    /// What the app owns: the web views, the pool, the relay, the sheet, the feed.
    struct Ports {
        /// Hand one message to `window.__velaDeliver` in THAT tab's page. A tab
        /// with no web view drops it — there is no front-tab fallback.
        var deliver: (_ tab: String, _ messageJson: String) -> Void = { _, _ in }
        /// One call through the wallet's own pool: `["result": …]`,
        /// `["error": ["code", "message"]]`, or `nil` when no endpoint answered.
        var poolCall: (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> [String: Any]?
        = { _, _, _, _ in nil }
        /// The transaction a user operation landed in, if it has.
        var resolveUserOp: (_ chainId: Int, _ userOpHash: String) async -> String? = { _, _ in nil }
        /// Open the signing sheet. Answered later, exactly once, through
        /// `signing_answered`.
        var forwardToSigning: (DbrForward) -> Void = { _ in }
        /// The page that asked is gone: close its sheet without answering.
        var cancelSigning: (_ tab: String, _ id: String) -> Void = { _, _ in }
        /// Write the "connected to" row.
        var saveConnectionRecord: (_ row: [String: Any]) -> Void = { _ in }
    }

    private let store: VelaStore
    private let now: () -> Double
    var ports: Ports

    init(
        store: VelaStore,
        ports: Ports = Ports(),
        now: @escaping () -> Double = { Date().timeIntervalSince1970 * 1000 }
    ) {
        self.store = store
        self.ports = ports
        self.now = now
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "list_sites":
            return CoreJSON.string(["type": "sites_listed", "sites": listSites()])

        case "write_grant":
            if let grant = operation["grant"] as? [String: Any],
               let origin = grant["origin"] as? String, !origin.isEmpty,
               let data = try? JSONSerialization.data(withJSONObject: grant),
               let text = String(data: data, encoding: .utf8) {
                store.writeString(Self.grantKey(origin), text)
            }
            return Self.ack

        case "remove_grant":
            let origin = operation["origin"] as? String ?? ""
            if !origin.isEmpty { store.remove(Self.grantKey(origin)) }
            return Self.ack

        case "write_site_chain":
            let origin = operation["origin"] as? String ?? ""
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            if !origin.isEmpty, chainId > 0 {
                store.writeString(Self.chainKey(origin), String(chainId))
            }
            return Self.ack

        case "deliver":
            ports.deliver(
                operation["tab"] as? String ?? "",
                operation["message_json"] as? String ?? ""
            )
            return Self.ack

        case "read":
            let body = await ports.poolCall(
                (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                operation["method"] as? String ?? "",
                Self.array(operation["params_json"] as? String ?? "[]"),
                operation["bundler"] as? Bool ?? false
            )
            return CoreJSON.string([
                "type": "read_answered",
                "body_json": body.flatMap(Self.json) ?? NSNull(),
            ])

        case "resolve_user_op":
            let txHash = await ports.resolveUserOp(
                (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                operation["user_op_hash"] as? String ?? ""
            )
            return CoreJSON.string([
                "type": "user_op_resolved",
                "tx_hash": txHash.flatMap { $0.isEmpty ? nil : $0 } ?? NSNull(),
            ])

        case "forward_to_signing":
            ports.forwardToSigning(DbrForward(operation: operation))
            return Self.ack

        case "cancel_signing":
            ports.cancelSigning(
                operation["tab"] as? String ?? "",
                operation["id"] as? String ?? ""
            )
            return Self.ack

        case "save_connection_record":
            ports.saveConnectionRecord(Self.connectionRow(
                address: operation["address"] as? String ?? "",
                chainId: (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                origin: operation["origin"] as? String ?? "",
                nowMs: now()
            ))
            return Self.ack

        default:
            print("[vela-wallet] dapp_browser: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    /// The contract's answer for an operation that could not be performed.
    static func neutralAnswer(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        case "list_sites":
            return CoreJSON.string(["type": "sites_listed", "sites": [Any]()])
        case "read":
            return CoreJSON.string(["type": "read_answered", "body_json": NSNull()])
        case "resolve_user_op":
            return CoreJSON.string(["type": "user_op_resolved", "tx_hash": NSNull()])
        default:
            return ack
        }
    }

    static let ack = CoreJSON.string(["type": "ack"])

    // MARK: - The store

    /// One document per origin, the desktop's and the extension's key.
    static func grantKey(_ origin: String) -> String { "vela.perm.\(origin)" }
    /// The site's chain, a bare number — the extension's key (spec 070 FR-003).
    static func chainKey(_ origin: String) -> String { "vela.chain.\(origin)" }

    /// Every stored site: its grant and its chain, either of which may be
    /// absent. A grant that does not decode is left out rather than sent on,
    /// so one damaged key cannot make the core refuse the whole list.
    func listSites() -> [[String: Any]] {
        var sites: [String: (grant: DpermGrantWire?, chain: Int?)] = [:]
        for key in store.allKeys() {
            if key.hasPrefix("vela.perm.") {
                let origin = String(key.dropFirst("vela.perm.".count))
                guard !origin.isEmpty, let grant = DpermGrantWire.read(store.readString(key))
                else { continue }
                sites[origin, default: (nil, nil)].grant = grant
            } else if key.hasPrefix("vela.chain.") {
                let origin = String(key.dropFirst("vela.chain.".count))
                guard !origin.isEmpty,
                      let text = store.readString(key)?.trimmingCharacters(in: .whitespaces),
                      let chain = Int(text), chain > 0, chain <= Int(UInt32.max)
                else { continue }
                sites[origin, default: (nil, nil)].chain = chain
            }
        }
        return sites.keys.sorted().map { origin in
            let site = sites[origin]!
            return [
                "origin": origin,
                "grant": site.grant?.stored ?? NSNull(),
                "chain_id": site.chain.map { $0 as Any } ?? NSNull(),
            ]
        }
    }

    // MARK: - Shapes

    static func array(_ json: String) -> [Any] {
        guard let data = json.data(using: .utf8),
              let params = try? JSONSerialization.jsonObject(with: data) as? [Any]
        else { return [] }
        return params
    }

    /// A JSON-RPC body as the text the core parses.
    static func json(_ body: [String: Any]) -> String? {
        guard JSONSerialization.isValidJSONObject(body),
              let data = try? JSONSerialization.data(withJSONObject: body, options: [.fragmentsAllowed])
        else { return nil }
        return String(data: data, encoding: .utf8)
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
