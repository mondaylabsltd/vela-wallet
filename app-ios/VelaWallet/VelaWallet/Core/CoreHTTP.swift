//
//  CoreHTTP.swift
//  VelaWallet
//
//  The two request shapes an executor needs, and nothing else.
//
//  `URLSession`, a per-call budget, and one rule about what a failure means.
//  It is deliberately not a service layer: an executor owns the meaning of its
//  own answer, and the core owns every decision that follows from it.
//
//  ## A failure is never an exception here
//
//  Every method returns an optional. A refused request, a timeout, a body that
//  is not JSON and a 500 all come back as `nil`, because the caller's next move
//  is always the same — answer the core with the variant it models for "I could
//  not find out". Throwing would only make each of the sixteen call sites write
//  the same `catch`.
//
//  What the caller must NOT do is turn `nil` into a value. `nil` is *unknown*,
//  and the difference between "unknown" and "absent" is the difference between
//  a warning shown honestly and a warning shown to everybody.
//
//  ## Timeouts are the web client's, on purpose
//
//  A probe that gives up sooner here than on the web would make the same chain
//  compatible on one client and not on another, from the same sofa.
//  (`app-web/.../services/net.ts` NET_TIMEOUTS.)
//

import Foundation

enum CoreHTTP {

    /// Per-call budgets, matching web's `NET_TIMEOUTS`.
    enum Timeout {
        /// ethereum-data chain info, token lists, the search index.
        static let ethereumData: TimeInterval = 5
        /// Custom-network RPC validation probe.
        static let networkCheck: TimeInterval = 10
        /// Fiat FX rates.
        static let fiatRates: TimeInterval = 8
        /// Read-only JSON-RPC (eth_call / getBalance / getLogs) — fail over
        /// fast, because the pool has other endpoints to try.
        static let rpcRead: TimeInterval = 8
    }

    /// One session for the app, so connection reuse and the system proxy
    /// configuration are shared. iOS resolves the device's proxy settings
    /// itself, which is the platform answer to the desktop's `proxy.rs`.
    private static let session: URLSession = {
        let config = URLSessionConfiguration.ephemeral
        config.waitsForConnectivity = false
        config.httpAdditionalHeaders = ["Accept": "application/json"]
        return URLSession(configuration: config)
    }()

    /// `GET url` → parsed JSON, or `nil` for anything that is not a 2xx JSON
    /// body.
    static func getJSON(_ url: String, timeout: TimeInterval) async -> Any? {
        guard let request = request(url, timeout: timeout) else { return nil }
        return await perform(request).body
    }

    /// One JSON-RPC call → the `result` field, or `nil`.
    ///
    /// A JSON-RPC `error` object is `nil` too: the endpoint answered, but not
    /// with an answer. Both are "I could not find out" as far as the core is
    /// concerned, and neither is a value to pass on.
    static func rpc(
        _ url: String,
        method: String,
        params: [Any],
        timeout: TimeInterval = Timeout.networkCheck
    ) async -> Any? {
        guard var request = request(url, timeout: timeout) else { return nil }
        let payload: [String: Any] = [
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: payload) else { return nil }
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        guard let object = await perform(request).body as? [String: Any],
              object["error"] == nil
        else { return nil }
        let result = object["result"]
        return result is NSNull ? nil : result
    }

    /// What a health probe needs and a plain fetch does not: the status code
    /// and how long it took, even when the body is useless.
    struct Probe {
        let body: Any?
        /// `nil` when the request never reached a server at all — a DNS
        /// failure, a refused connection, a timeout. Distinct from a 500,
        /// which is a server saying no.
        let status: Int?
        let latencyMs: Double
    }

    static func probeJSON(_ url: String, timeout: TimeInterval) async -> Probe {
        guard let request = request(url, timeout: timeout) else {
            return Probe(body: nil, status: nil, latencyMs: 0)
        }
        return await perform(request)
    }

    /// A JSON-RPC reply, **classified rather than interpreted**.
    ///
    /// `rpc_pool` bans on the difference between these, so the distinctions are
    /// the core's vocabulary and not this file's convenience: a 429 is not a
    /// 500, a timeout is not a refused connection, and a JSON-RPC error is a
    /// server that answered.
    enum RpcReply {
        /// A 2xx with a JSON object body — which may still carry an `error`.
        case response([String: Any])
        case httpError(status: Int)
        /// 2xx, but the body was not JSON. Some proxies answer HTML.
        case nonJSON
        case timeout
        /// Never reached a server: DNS, refused, offline, TLS.
        case network
    }

    /// One JSON-RPC call, with the envelope preserved.
    ///
    /// Distinct from `rpc(_:method:params:)`, which returns just the result and
    /// flattens every failure to `nil`. The pool needs the failure *kind*,
    /// because that is what it bans on.
    static func rpcEnvelope(
        _ url: String,
        method: String,
        params: [Any],
        timeout: TimeInterval = Timeout.rpcRead
    ) async -> RpcReply {
        guard var request = request(url, timeout: timeout) else { return .network }
        let payload: [String: Any] = [
            "jsonrpc": "2.0", "id": 1, "method": method, "params": params,
        ]
        guard let body = try? JSONSerialization.data(withJSONObject: payload) else {
            return .network
        }
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = body

        do {
            let (data, response) = try await session.data(for: request)
            let status = (response as? HTTPURLResponse)?.statusCode ?? 0
            guard (200..<300).contains(status) else { return .httpError(status: status) }
            guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
            else { return .nonJSON }
            return .response(object)
        } catch let error as URLError where error.code == .timedOut {
            return .timeout
        } catch {
            return .network
        }
    }

    // MARK: - The one request, the one send

    /// Refuses anything that is not `https`.
    ///
    /// Not a policy invented here: the core models `not_https` as its own
    /// endpoint state, and a wallet that would talk to a plaintext RPC is a
    /// wallet whose balances anyone on the network can rewrite.
    private static func request(_ url: String, timeout: TimeInterval) -> URLRequest? {
        guard let parsed = URL(string: url),
              parsed.scheme?.lowercased() == "https"
        else { return nil }
        var request = URLRequest(url: parsed)
        request.timeoutInterval = timeout
        return request
    }

    private static func perform(_ request: URLRequest) async -> Probe {
        let started = Date()
        do {
            let (data, response) = try await session.data(for: request)
            let elapsed = Date().timeIntervalSince(started) * 1000
            let status = (response as? HTTPURLResponse)?.statusCode
            guard let status, (200..<300).contains(status) else {
                return Probe(body: nil, status: status, latencyMs: elapsed)
            }
            return Probe(
                body: try? JSONSerialization.jsonObject(with: data),
                status: status,
                latencyMs: elapsed
            )
        } catch {
            return Probe(
                body: nil,
                status: nil,
                latencyMs: Date().timeIntervalSince(started) * 1000
            )
        }
    }
}
