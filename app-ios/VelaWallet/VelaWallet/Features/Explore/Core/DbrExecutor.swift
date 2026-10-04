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
//  ## Every read answers by its deadline (spec 099 FR-008)
//
//  The core puts `deadline_ms` on every `read`. The pool's own sweep has no
//  bound a page can rely on — a read waiting behind slow endpoints waited as
//  long as they did — so the read races the deadline here: when it passes
//  first the core hears `timed_out` and the late body is dropped. When the
//  pool gave up because every endpoint said to slow down, the core hears
//  `rate_limited`; any other read with no answer is `no_endpoint`. Which
//  layer and reason that makes, and the words, are the core's.
//

import Foundation

@MainActor
final class DbrExecutor {

    static let operations = [
        "list_sites", "write_grant", "remove_grant", "write_site_chain", "deliver",
        "read", "resolve_user_op", "forward_to_signing", "cancel_signing",
        "save_connection_record", "log", "forward_to_add_network", "cancel_add_network",
    ]

    /// What one read through the pool came back as.
    enum Read {
        /// The JSON-RPC body: `["result": …]` or `["error": ["code", "message"]]`.
        case answered([String: Any])
        /// No endpoint answered. `rateLimited`: the pool gave up because every
        /// one of them said to slow down.
        case unanswered(rateLimited: Bool)
    }

    /// What the app owns: the web views, the pool, the relay, the sheet, the feed.
    struct Ports {
        /// Hand one message to `window.__velaDeliver` in THAT tab's page. A tab
        /// with no web view drops it — there is no front-tab fallback.
        var deliver: (_ tab: String, _ messageJson: String) -> Void = { _, _ in }
        /// One call through the wallet's own pool.
        var poolCall: @MainActor (_ chainId: Int, _ method: String, _ params: [Any], _ bundler: Bool) async -> Read
        = { _, _, _, _ in .unanswered(rateLimited: false) }
        /// The transaction a user operation landed in, if it has.
        var resolveUserOp: @MainActor (_ chainId: Int, _ userOpHash: String) async -> String? = { _, _ in nil }
        /// Open the signing sheet. Answered later, exactly once, through
        /// `signing_answered`.
        var forwardToSigning: (DbrForward) -> Void = { _ in }
        /// The page that asked is gone: close its sheet without answering.
        var cancelSigning: (_ tab: String, _ id: String) -> Void = { _, _ in }
        /// Write the "connected to" row.
        var saveConnectionRecord: (_ row: [String: Any]) -> Void = { _ in }
        /// Spec 100: hand a page's add-network request to `network_admin`
        /// (`dapp_add_requested`) — the operation as the core sent it, `ask`
        /// untouched. Answered later, exactly once, through
        /// `add_network_answered`.
        var forwardToAddNetwork: (_ operation: [String: Any]) -> Void = { _ in }
        /// Spec 100: the page that asked to add a network is gone.
        var cancelAddNetwork: (_ tab: String, _ id: String) -> Void = { _, _ in }
    }

    private let store: VelaStore
    private let now: () -> Double
    var ports: Ports
    #if DEBUG
    /// A test's deadline in place of the core's 30 s, so "the read that never
    /// answers" is a test that takes milliseconds.
    var deadlineForTesting: Double?
    #endif

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
            var deadlineMs = (operation["deadline_ms"] as? NSNumber)?.doubleValue ?? 0
            #if DEBUG
            if let deadlineForTesting { deadlineMs = deadlineForTesting }
            #endif
            let call = ports.poolCall
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let method = operation["method"] as? String ?? ""
            let params = Self.array(operation["params_json"] as? String ?? "[]")
            let bundler = operation["bundler"] as? Bool ?? false
            let read = await Self.byDeadline(deadlineMs) {
                await call(chainId, method, params, bundler)
            }
            return Self.readAnswered(read, nowMs: now())

        case "resolve_user_op":
            let txHash = await ports.resolveUserOp(
                (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                operation["user_op_hash"] as? String ?? ""
            )
            return CoreJSON.string([
                "type": "user_op_resolved",
                "tx_hash": txHash.flatMap { $0.isEmpty ? nil : $0 } ?? NSNull(),
                "now_ms": now(),
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

        case "forward_to_add_network":
            ports.forwardToAddNetwork(operation)
            return Self.ack

        case "cancel_add_network":
            ports.cancelAddNetwork(
                operation["tab"] as? String ?? "",
                operation["id"] as? String ?? ""
            )
            return Self.ack

        // Spec 099 FR-015: the core's line, one per request end and tab
        // state change, written as it is.
        case "log":
            VelaLog.notice(.browser, operation["line"] as? String ?? "")
            return Self.ack

        default:
            VelaLog.failure(.browser, kind: "unhandled_operation", "dapp_browser \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    /// The contract's answer for an operation that could not be performed.
    static func neutralAnswer(_ operation: [String: Any]) -> String {
        let nowMs = Date().timeIntervalSince1970 * 1000
        switch operation["type"] as? String ?? "" {
        case "list_sites":
            return CoreJSON.string(["type": "sites_listed", "sites": [Any]()])
        case "read":
            return CoreJSON.string([
                "type": "read_answered", "body_json": NSNull(), "now_ms": nowMs, "failure": NSNull(),
            ])
        case "resolve_user_op":
            return CoreJSON.string(["type": "user_op_resolved", "tx_hash": NSNull(), "now_ms": nowMs])
        default:
            return ack
        }
    }

    /// The read's answer to the core: the body, or `nil` with why there is
    /// none (`DbrReadFailure`). `read` is `nil` when the deadline passed first.
    static func readAnswered(_ read: Read?, nowMs: Double) -> String {
        let body: String?
        let failure: String?
        switch read {
        case .answered(let answer):
            body = json(answer)
            failure = body == nil ? "no_endpoint" : nil
        case .unanswered(let rateLimited):
            body = nil
            failure = rateLimited ? "rate_limited" : "no_endpoint"
        case nil:
            body = nil
            failure = "timed_out"
        }
        return CoreJSON.string([
            "type": "read_answered",
            "body_json": body.map { $0 as Any } ?? NSNull(),
            "now_ms": nowMs,
            "failure": failure.map { $0 as Any } ?? NSNull(),
        ])
    }

    /// `work`'s answer, or `nil` when `deadlineMs` passed first — and then
    /// whatever it answers later is dropped. The pool's I/O is not cancelled
    /// (it settles its own bans and cooldowns); only this answer stops
    /// waiting for it. `deadlineMs <= 0` waits as long as `work` takes.
    static func byDeadline<T>(
        _ deadlineMs: Double, _ work: @escaping @MainActor () async -> T
    ) async -> T? {
        guard deadlineMs > 0 else { return await work() }
        let once = Once<T>()
        return await withCheckedContinuation { continuation in
            once.continuation = continuation
            let worker = Task { @MainActor in once.finish(await work()) }
            once.timer = Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(deadlineMs * 1_000_000))
                guard !Task.isCancelled else { return }
                once.finish(nil)
            }
            _ = worker
        }
    }

    /// One answer, whichever comes first.
    @MainActor
    private final class Once<T> {
        var continuation: CheckedContinuation<T?, Never>?
        var timer: Task<Void, Never>?

        func finish(_ value: T?) {
            guard let continuation else { return }
            self.continuation = nil
            timer?.cancel()
            timer = nil
            continuation.resume(returning: value)
        }

        nonisolated deinit {}
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
