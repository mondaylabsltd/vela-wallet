//
//  MultisigExecutor.swift
//  VelaWallet
//
//  The iOS shell for the core's multisig machines (`vela_core::app::multisig_*`,
//  over uniffi: `MultisigAccountsCore`, `MultisigProposalsCore`,
//  `MultisigMembersCore`, `MultisigOutsidePageCore`, `MultisigOutsideSessionCore`).
//  Every `MultisigOperation` any of the five asks is answered here by exactly
//  ONE `MultisigShellResult` — the web's `shell/handler.ts`, line for line.
//
//  This file draws nothing and decides nothing. It performs I/O and reports
//  what happened in the vocabulary's own words: an HTTP answer as it came, a
//  JSON-RPC reply as the node gave it, a store key as it was. What to ask,
//  when, how often and what an answer means is the core's (the same core the
//  web, desktop and Android drive). The product's own pipelines — the passkey
//  and its trusted page, the risk question, the send pipeline, simulation,
//  recipient facts, the relay's word on a user operation, the PeerConnect
//  transport, the injected wallet, and where a notice goes — come in as
//  `MultisigPorts`, so a test answers them from a simulated world and the app
//  from `MultisigLive`.
//
//  The contract the effect loop holds every shell to: `perform` never fails.
//  A port that throws is answered with that operation's own failure
//  (`MultisigExecutor.failure(for:detail:)`, the web's `operationFailure`).
//

import Foundation

/// A JSON-RPC reply through the wallet's own pool: its `result`, the node's
/// `error`, or nobody answered.
enum MultisigRpcReply {
    case result(Any?)
    case error(code: Int?, message: String)
    case unanswered(String)
}

/// The PeerConnect rooms a machine opens (`peer_*`), as the web's
/// `PeerTransport`: what a room carries is the core's (`multisig_outside`).
@MainActor
protocol MultisigPeerTransport: AnyObject {
    /// Create (`invite` and `pairing` absent), join (`invite`) or reopen
    /// (`pairing`) a room: its handle, and the invite / pairing record.
    func open(relay: String?, invite: String?, pairing: String?) async throws
        -> (room: String, invite: String?, pairing: String?)
    func send(room: String, message: Any) async
    /// The next message, or the room's close (its reason).
    func receive(room: String) async -> MultisigPeerArrival
    func close(room: String, cancel: Bool, end: Bool) async
}

enum MultisigPeerArrival {
    case message(Any)
    case closed(String)
}

/// The product pipelines a multisig operation reaches (the web's
/// `MultisigShellDeps`). Each answers in the vocabulary; a throw is answered
/// with the operation's failure.
@MainActor
protocol MultisigPorts: AnyObject {
    /// One HTTP request, as it came back: status, `Retry-After`, body.
    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws
        -> (status: Int, retryAfter: String?, body: String)
    /// One call through the RPC pool.
    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply
    /// The key-value store (`nil`: none — answered `store_failed`).
    func storeRead(_ key: String) -> String??
    func storeWrite(_ key: String, _ value: String?) -> Bool
    func nowMs() -> Double
    func random(_ bytes: Int) -> Data
    /// The member's passkey, in Vela or on its trusted page: the result object.
    func passkey(_ op: [String: Any]) async -> [String: Any]
    /// The high-risk question: `false` when nobody can be asked.
    func askRisk(_ op: [String: Any]) async -> Bool
    /// The send pipeline: `calls_sent` / `calls_failed` / `calls_cancelled`.
    func sendCalls(_ op: [String: Any]) async -> [String: Any]
    /// The fee pipeline: `quoted` / `quote_failed`.
    func quoteCalls(_ op: [String: Any]) async -> [String: Any]
    /// The signing sheet's simulation envelope, or `nil`.
    func simulate(_ op: [String: Any]) async -> String?
    /// What the send flow knows of a recipient.
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?)
    /// The relay's word on a user operation, or `nil`.
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)?
    var peer: MultisigPeerTransport { get }
    /// One EIP-1193 request to an injected wallet. iOS has none: a refusal.
    func wallet(method: String, params: Any) async throws -> Any
    /// Route a notice (another machine, the activity, a screen).
    func notice(_ notice: [String: Any]) async
}

/// Who answered first, between a read and its bound (main actor only).
final class MultisigOnce: @unchecked Sendable {
    var done = false
}

/// An EIP-1193 refusal (`{ code, message }`).
struct MultisigWalletRefusal: Error {
    let code: Int?
    let message: String
}

@MainActor
struct MultisigExecutor {
    let ports: MultisigPorts

    /// Perform one operation; the result JSON the core resolves it with.
    func perform(_ op: [String: Any]) async -> String {
        CoreJSON.string(await answer(op))
    }

    /// Perform one operation; the result object.
    func answer(_ op: [String: Any]) async -> [String: Any] {
        let type = op["type"] as? String ?? ""
        switch type {
        case "http":
            let timeout = Self.int(op["timeout_ms"]) ?? 15_000
            do {
                let answer = try await ports.http(
                    method: op["method"] as? String ?? "GET",
                    url: op["url"] as? String ?? "",
                    body: op["body"] as? String,
                    timeoutMs: timeout
                )
                return [
                    "type": "http_answered",
                    "status": answer.status,
                    "retry_after": answer.retryAfter ?? NSNull(),
                    "body": answer.body,
                ]
            } catch {
                return ["type": "http_unreachable", "detail": Self.describe(error)]
            }
        case "rpc":
            let chainId = Self.int(op["chain_id"]) ?? 0
            let method = op["method"] as? String ?? ""
            let params = op["params"] ?? []
            let bound = Self.int(op["bound_ms"])
            let reply: MultisigRpcReply
            if let bound {
                reply = await Self.within(ms: bound) { [ports] in
                    await ports.rpc(chainId: chainId, method: method, params: params)
                } ?? .unanswered("no answer in \(bound) ms")
            } else {
                reply = await ports.rpc(chainId: chainId, method: method, params: params)
            }
            switch reply {
            case .result(let value):
                return ["type": "rpc_result", "result": value ?? NSNull()]
            case .error(let code, let message):
                return ["type": "rpc_error", "code": code.map { $0 as Any } ?? NSNull(), "message": message]
            case .unanswered(let detail):
                return ["type": "rpc_unanswered", "detail": detail]
            }
        case "load_store":
            var entries: [String: Any] = [:]
            for key in op["keys"] as? [String] ?? [] {
                guard let read = ports.storeRead(key) else {
                    return ["type": "store_failed", "detail": "no storage"]
                }
                entries[key] = read ?? NSNull()
            }
            return ["type": "store_loaded", "entries": entries]
        case "write_store":
            let key = op["key"] as? String ?? ""
            let value = op["value"] as? String
            guard ports.storeWrite(key, value) else {
                return ["type": "store_failed", "detail": "the store did not take \(key)"]
            }
            return ["type": "store_written"]
        case "wait":
            let ms = max(0, Self.int(op["ms"]) ?? 0)
            // A wait the machine stopped waiting for is cancelled with its
            // task; the answer is dropped by the loop either way.
            try? await Task.sleep(nanoseconds: UInt64(ms) * 1_000_000)
            return ["type": "waited"]
        case "now":
            return ["type": "now", "now_ms": ports.nowMs()]
        case "random":
            let bytes = ports.random(max(0, Self.int(op["bytes"]) ?? 0))
            return ["type": "random", "hex": "0x" + bytes.map { String(format: "%02x", $0) }.joined()]
        case "passkey":
            return await ports.passkey(op)
        case "ask_risk":
            return ["type": "risk_answered", "signs": await ports.askRisk(op)]
        case "send_calls":
            return await ports.sendCalls(op)
        case "quote_calls":
            return await ports.quoteCalls(op)
        case "simulate":
            return ["type": "simulated", "reply": await ports.simulate(op) ?? NSNull()]
        case "recipient_risk":
            let facts = await ports.recipientFacts(op)
            return [
                "type": "recipient_facts",
                "is_contract": facts.isContract.map { $0 as Any } ?? NSNull(),
                "first_time": facts.firstTime.map { $0 as Any } ?? NSNull(),
            ]
        case "user_op_status":
            let answer = await ports.userOpStatus(op)
            return [
                "type": "user_op_status",
                "status": answer?.status ?? NSNull(),
                "tx_hash": answer?.txHash ?? NSNull(),
            ]
        case "peer_open":
            do {
                let opened = try await ports.peer.open(
                    relay: op["relay"] as? String,
                    invite: op["invite"] as? String,
                    pairing: op["pairing"] as? String
                )
                return [
                    "type": "peer_opened",
                    "room": opened.room,
                    "invite": opened.invite ?? NSNull(),
                    "pairing": opened.pairing ?? NSNull(),
                ]
            } catch {
                return ["type": "peer_closed", "reason": Self.describe(error)]
            }
        case "peer_send":
            await ports.peer.send(room: op["room"] as? String ?? "", message: op["message"] ?? NSNull())
            return ["type": "ack"]
        case "peer_receive":
            switch await ports.peer.receive(room: op["room"] as? String ?? "") {
            case .message(let message): return ["type": "peer_message", "message": message]
            case .closed(let reason): return ["type": "peer_closed", "reason": reason]
            }
        case "peer_close":
            await ports.peer.close(
                room: op["room"] as? String ?? "",
                cancel: op["cancel"] as? Bool ?? false,
                end: op["end"] as? Bool ?? false
            )
            return ["type": "ack"]
        case "wallet_request":
            do {
                let result = try await ports.wallet(method: op["method"] as? String ?? "", params: op["params"] ?? NSNull())
                return ["type": "wallet_answered", "result": result]
            } catch let refusal as MultisigWalletRefusal {
                return ["type": "wallet_refused", "code": refusal.code.map { $0 as Any } ?? NSNull(), "message": refusal.message]
            } catch {
                return ["type": "wallet_refused", "code": NSNull(), "message": Self.describe(error)]
            }
        case "notice":
            await ports.notice(op["notice"] as? [String: Any] ?? [:])
            return ["type": "ack"]
        default:
            // An operation this build does not know: its neutral answer, so the
            // machine is never left waiting (a newer core than this shell).
            return Self.failure(for: op, detail: "this app does not perform \(type)")
        }
    }

    /// The failure each operation answers when its port could not — the
    /// result the machine waiting on it reads as "it did not happen", never
    /// a hang (the web's `operationFailure`).
    static func failure(for op: [String: Any], detail: String) -> [String: Any] {
        switch op["type"] as? String ?? "" {
        case "http": return ["type": "http_unreachable", "detail": detail]
        case "rpc": return ["type": "rpc_unanswered", "detail": detail]
        case "load_store", "write_store": return ["type": "store_failed", "detail": detail]
        case "wait": return ["type": "waited"]
        case "now": return ["type": "now", "now_ms": Date().timeIntervalSince1970 * 1000]
        case "random": return ["type": "random", "hex": "0x"]
        case "passkey": return ["type": "passkey_failed", "detail": detail]
        case "ask_risk": return ["type": "risk_answered", "signs": false]
        case "send_calls": return ["type": "calls_failed", "detail": detail, "maybe_sent": false]
        case "quote_calls": return ["type": "quote_failed", "detail": detail]
        case "simulate": return ["type": "simulated", "reply": NSNull()]
        case "recipient_risk": return ["type": "recipient_facts", "is_contract": NSNull(), "first_time": NSNull()]
        case "user_op_status": return ["type": "user_op_status", "status": NSNull(), "tx_hash": NSNull()]
        case "peer_open", "peer_receive": return ["type": "peer_closed", "reason": detail]
        case "peer_send", "peer_close", "notice": return ["type": "ack"]
        case "wallet_request": return ["type": "wallet_refused", "code": NSNull(), "message": detail]
        default: return ["type": "ack"]
        }
    }

    /// The same, as the JSON the loop resolves a refused answer with.
    static func failureJson(for op: [String: Any], detail: String) -> String {
        CoreJSON.string(failure(for: op, detail: detail))
    }

    // MARK: - Helpers

    static func int(_ value: Any?) -> Int? {
        switch value {
        case let n as NSNumber: return n.intValue
        case let s as String: return Int(s)
        default: return nil
        }
    }

    static func describe(_ error: Error) -> String {
        if let refusal = error as? MultisigWalletRefusal { return refusal.message }
        let ns = error as NSError
        return "\(ns.domain) \(ns.code): \(ns.localizedDescription)"
    }

    /// `work`, or `nil` past `ms` — a read that never answers never holds
    /// the machine (the web's `within`).
    ///
    /// Not a task group: a group waits for every child, and a read that
    /// ignores cancellation would hold it past the bound. The late answer is
    /// dropped instead.
    static func within<T>(ms: Int, _ work: @escaping @MainActor () async -> T) async -> T? {
        let once = MultisigOnce()
        return await withCheckedContinuation { (continuation: CheckedContinuation<T?, Never>) in
            let worker = Task { @MainActor in
                let value = await work()
                guard !once.done else { return }
                once.done = true
                continuation.resume(returning: value)
            }
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(max(0, ms)) * 1_000_000)
                guard !once.done else { return }
                once.done = true
                worker.cancel()
                continuation.resume(returning: nil)
            }
        }
    }
}
