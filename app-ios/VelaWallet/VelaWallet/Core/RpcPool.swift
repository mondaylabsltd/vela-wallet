//
//  RpcPool.swift
//  VelaWallet
//
//  Every chain read in the app, routed by one machine.
//
//  `rpc_pool.rs` is 1,975 lines of routing decisions — six-tier source scoring,
//  EMA latency, cooldowns, temporary and permanent bans, four-way error
//  classification, a three-pass sweep with jittered backoff, and the all-banned
//  self-rescue. None of it is here. What is here is the half its module doc
//  reserves for the shell:
//
//      The shell keeps the fetch execution: it holds the request payloads keyed
//      by `call_id`, performs `JsonRpcPost`, and reports transport outcomes
//      back — this core only ever decides *which URL next and why*.
//
//  ## Why this one machine gets a facade
//
//  The other six are fire-and-forget: dispatch an event, render the view that
//  comes back. A caller of the pool wants an **answer** — "give me the result
//  of this call" — and has to wait for it. So this wraps `CoreStore` in one
//  `await`, and it is the only new plumbing shape in spec 051.
//
//  The correlation is `call_id`: the shell mints it, holds the payload and the
//  response bodies under it, and the core hands it back on every operation and
//  finally on `Conclude`. A response body never travels through the core —
//  routing does not need it, and a core that carried megabytes of `eth_getLogs`
//  output through its model would be paying for the privilege of ignoring it.
//
//  ## One session, app-wide
//
//  FR-002. A ban is a fact about the network, not about a screen. Two callers
//  with their own endpoint lists is how the Expo client got a ban map that
//  disagreed with itself.
//

import Foundation
import Observation
import VelaCore

extension RpcPoolCore: CoreBridge {}

/// What a routed call came back as.
enum RpcOutcome {
    /// The core accepted this URL's answer. `result` is the JSON-RPC `result`
    /// field — `nil` is a legitimate answer (`eth_getCode` on an EOA is `0x`,
    /// but a null result happens too), so callers must not read it as failure.
    case ok(Any?)
    /// Every endpoint was swept and none answered. `rateLimited` separates
    /// "the chain refused us" from "the chain is down", and the two get
    /// different screens: invariant ④ forbids offering "swap in your own RPC"
    /// for a chain that is merely throttling us.
    case failed(rateLimited: Bool)
    /// The provider capped the block span of an `eth_getLogs`. The caller
    /// narrows and asks again; the core has already recorded the cap.
    case rangeCap(url: String, maxSpan: Double)
    /// The endpoint answered, and the answer was a JSON-RPC error.
    ///
    /// **Not a failure of the endpoint** — `classify_response_error` routes an
    /// error that is neither permanent nor transient to `Route::Success`,
    /// because a revert, an "out of gas" and a relay's refusal of a user
    /// operation are all valid responses to a question. They are just not
    /// values. Until spec 052 this case did not exist and those answers
    /// arrived as `.ok(nil)`, which conflated "the chain said no" with "the
    /// method returned null" — and dropped the relay's own sentence, which is
    /// the one thing `classify_relay_rejection` needs to do its job.
    case rpcError(code: Int?, message: String)
}

/// A routed call with what the submit path and the tracker need beside the
/// answer (spec 082).
struct RpcCallResult {
    let outcome: RpcOutcome
    /// The core's OR over every POST of this call of `may_have_delivered`:
    /// `false` only when no POST can have been acted on (contract §2). What
    /// lets a submit say "not sent" — and only then.
    let maybeDelivered: Bool
    /// The JSON-RPC `error` member the endpoint answered, as it came, when the
    /// core ruled it a range cap (T180): the tracker's find-event judges it
    /// itself, so it must arrive untouched rather than as a span.
    let heldErrorJson: String?
}

@MainActor
@Observable
final class RpcPool {

    /// Every operation this facade is required to handle. Named for the same
    /// reason the executors name theirs: a JSON tag has no exhaustiveness
    /// check, so a drift test asserts the core asks for nothing else.
    static let operations = [
        "load_pool_config",
        "json_rpc_post",
        "probe_chain_id",
        "draw_jitter",
        "start_backoff",
        "persist_bans",
        "conclude",
    ]

    /// Chains whose whole pool failed on the last attempt — the stale-balance
    /// notice's source.
    private(set) var failedChains: [Int] = []
    /// The transient subset. A chain here keeps its cached balance and must
    /// **never** show the "swap in your own RPC" banner.
    private(set) var rateLimitedChains: [Int] = []
    /// Chains one call's first pass could not reach at all — every endpoint
    /// failed on transport, no rate limit (spec 082 RF1). The browser's chain
    /// notice reads `failed ∪ unreached ∖ rate-limited`; the home banner
    /// keeps reading `failedChains` alone.
    private(set) var unreachedChains: [Int] = []

    /// One in-flight call: what to send, what came back from where, and who is
    /// waiting.
    private struct Pending {
        let method: String
        let params: [Any]
        /// Bodies by URL. `Conclude { Respond { url } }` names which one won.
        var bodies: [String: Any?] = [:]
        /// The JSON-RPC `error` members, by the URL that answered one. Kept
        /// beside the bodies rather than inside them so `.ok`'s payload stays
        /// exactly the `result` field every caller since 051 reads.
        var errors: [String: [String: Any]] = [:]
        var resume: ((RpcCallResult) -> Void)?
    }

    /// Every routed call's outcome, once concluded, with the chain it read —
    /// the network's health is read from these (`NetWatch`, spec 082 RE3):
    /// the calls are the only witness of a proxy node that hangs while the
    /// system path stays up, and the chain is what tells one faulted chain
    /// from a network that is gone (RJ14).
    var onOutcome: ((_ outcome: RpcOutcome, _ chainId: Int) -> Void)?

    private let store: VelaStore
    private let accounts: AccountStore
    private var core: CoreStore<RpcPoolViewWire>!
    private var pending: [String: Pending] = [:]
    private var nextCallId = 0

    /// A pool with no network: every call is answered `.failed` at once and
    /// nothing leaves the machine — what a phone with no network answers.
    /// For tests of the callers' fail-closed paths, which used to get this
    /// shape from a pool nobody booted (issue #483 removed that refusal).
    /// Chosen at construction; never a state a pool falls into.
    private let offline: Bool

    init(store: VelaStore, accounts: AccountStore, offline: Bool = false) {
        self.store = store
        self.accounts = accounts
        self.offline = offline
        self.core = CoreStore(
            bridge: RpcPoolCore(),
            perform: { [weak self] operation in
                await self?.perform(operation) ?? CoreJSON.string(["type": "concluded"])
            },
            onView: { [weak self] view in
                self?.failedChains = view.failedChains
                self?.rateLimitedChains = view.rateLimitedChains
                self?.unreachedChains = view.unreachedChains
            },
            onFault: { VelaLog.failure(.rpc, kind: "pool_fault", VelaLog.error($0)) }
        )
    }

    /// Whether the routing core has been given its ban map.
    ///
    /// A call before that would be dropped by `CoreStore` (events before boot
    /// are, on purpose) and its continuation would never resume — an `await`
    /// that hangs for the life of the process. So every entry point boots
    /// first (`bootForCall`).
    private(set) var booted = false

    /// Read the persisted ban map and hand it to the core. Once: a second
    /// call would hand the core a stale copy of bans it has since changed.
    /// Synchronous and needing only the store, so a pool can never be "not
    /// ready" for longer than this call.
    func boot() {
        guard !booted else { return }
        booted = true
        core.boot(CoreJSON.string([
            "type": "bans_loaded",
            "entries": RpcEndpoints.loadBans(store: store),
        ]))
    }

    /// A call reached a pool nobody booted: boot it now and answer the call,
    /// instead of refusing it (issue #483).
    ///
    /// It used to refuse — `before_boot`, a `.failed` with nothing sent —
    /// and that refusal was read by every caller as "the chain did not
    /// answer". A pool built a second time by a re-run `RootView.init` was
    /// never booted, so the dApp sheet's fee said "Can't reach Polygon" and
    /// its retry asked the same dead pool again until the app was killed.
    /// The app's own pool is booted as the graph is built (`AppGraph`), so
    /// this is a wiring fault when it happens: logged as `late_boot`, the
    /// report's ring keeps it, and the call is answered all the same — as
    /// Android, the web and the desktop always have (their cores route before
    /// `BansLoaded`).
    private func bootForCall(_ what: String) {
        guard !booted else { return }
        VelaLog.failure(.rpc, kind: "late_boot", what)
        boot()
    }

    // MARK: - A pool that cannot send (PR 2 note 11)

    /// Chains whose calls cannot leave the app: a fault inside Vela, never
    /// the chain's. Only a DEBUG build ever holds any — injected with the
    /// launch argument `-vela.faultPool 1,137` (the web's
    /// `vela.faultPool(chain)`) or `faultChain(_:)` — so a screenshot and a
    /// test can show what the wallet says when the fault is its own (home
    /// with an internal fault must never read "Can't reach Ethereum"). A
    /// release pool sends every call.
    private var faulted: Set<Int> = RpcPool.injectedFaults()

    /// Why a call on `chainId` cannot leave the app now — diagnostics for the
    /// report, never shown — or `nil` when it can. A read that fails while
    /// this says so failed inside Vela: it is told as that, never as the
    /// network out of reach (the balance's `internal_chain_ids`, the fee's
    /// `internal`).
    func unsendable(chainId: Int) -> String? {
        faulted.contains(chainId) ? "pool_fault" : nil
    }

    #if DEBUG
    /// Fault (or heal) one chain's calls — a test's and a board's hook.
    func faultChain(_ chainId: Int, _ faulty: Bool = true) {
        if faulty { faulted.insert(chainId) } else { faulted.remove(chainId) }
    }
    #endif

    private static func injectedFaults() -> Set<Int> {
        #if DEBUG
        let raw = UserDefaults.standard.string(forKey: "vela.faultPool") ?? ""
        return Set(raw.split(separator: ",").compactMap {
            Int($0.trimmingCharacters(in: .whitespaces))
        })
        #else
        return []
        #endif
    }

    // MARK: - The one thing callers want

    /// Route one JSON-RPC call and wait for the core's verdict.
    ///
    /// The caller never names a URL. That is the entire point: which endpoint,
    /// after which failure, under which ban is one decision and the core owns
    /// it.
    func call(
        chainId: Int,
        method: String,
        params: [Any] = [],
        kind: String = "rpc"
    ) async -> RpcOutcome {
        await callDetailed(chainId: chainId, method: method, params: params, kind: kind).outcome
    }

    /// `call`, with whether any POST may have been acted on and the held
    /// error of a range cap (spec 082: the submit's verdict and the tracker's
    /// find-event read these; every other caller wants `call`).
    func callDetailed(
        chainId: Int,
        method: String,
        params: [Any] = [],
        kind: String = "rpc"
    ) async -> RpcCallResult {
        if offline {
            return RpcCallResult(outcome: .failed(rateLimited: false), maybeDelivered: false, heldErrorJson: nil)
        }
        // A pool that cannot send this chain's calls answers at once, and
        // nothing left: no network was asked, so the network's health hears
        // nothing of it either (`onOutcome`).
        if let kind = unsendable(chainId: chainId) {
            VelaLog.failure(.rpc, kind: kind, "chain=\(chainId) method=\(method)")
            return RpcCallResult(outcome: .failed(rateLimited: false), maybeDelivered: false, heldErrorJson: nil)
        }
        // Never refused for want of a boot (issue #483): boot, then route.
        bootForCall("method=\(method) chain=\(chainId)")
        let callId = mintCallId()
        let result = await withCheckedContinuation { continuation in
            var entry = Pending(method: method, params: params)
            var resumed = false
            entry.resume = { result in
                // The core concludes a call exactly once, but a continuation
                // resumed twice is a crash rather than a bug report — so the
                // guard is here rather than in a comment.
                guard !resumed else { return }
                resumed = true
                continuation.resume(returning: result)
            }
            pending[callId] = entry
            core.dispatch(CoreJSON.string([
                "type": "call_requested",
                "call_id": callId,
                "chain_id": chainId,
                "kind": kind,
                "method": method,
                "now_ms": Self.nowMs,
            ]))
        }
        if case .failed(let rateLimited) = result.outcome {
            VelaLog.failure(
                .rpc, kind: rateLimited ? "rate_limited" : "gave_up",
                "chain=\(chainId) method=\(method) kind=\(kind)"
            )
        }
        onOutcome?(result.outcome, chainId)
        return result
    }

    /// Somebody changed an endpoint or a provider key.
    ///
    /// `nil` invalidates every chain; a chain id reloads that one and drops its
    /// cached winner — which matters more than it sounds: the winner is handed
    /// to the relay as `x-vela-rpc-url` for up to an hour, so a stale one keeps
    /// sending traffic to the endpoint the person just replaced.
    func invalidate(chainId: Int?) {
        bootForCall("invalidate")
        core.dispatch(CoreJSON.string(
            chainId.map { ["type": "refresh_chain", "chain_id": $0] }
                ?? ["type": "invalidate_all"]
        ))
    }

    /// The best endpoint for a chain, as the core scores it — for the callers
    /// that need a URL rather than an answer (a WebSocket, a link).
    func bestRpcUrl(chainId: Int) async -> String? {
        if offline { return nil }
        bootForCall("best_rpc_url chain=\(chainId)")
        let callId = mintCallId()
        return await withCheckedContinuation { continuation in
            var entry = Pending(method: "", params: [])
            var resumed = false
            entry.resume = { result in
                guard !resumed else { return }
                resumed = true
                if case .ok(let value) = result.outcome { continuation.resume(returning: value as? String) }
                else { continuation.resume(returning: nil) }
            }
            pending[callId] = entry
            core.dispatch(CoreJSON.string([
                "type": "best_rpc_url_requested",
                "call_id": callId,
                "chain_id": chainId,
                "now_ms": Self.nowMs,
            ]))
        }
    }

    /// Which bundler REST base this chain's `/v1/…` calls must use.
    ///
    /// The core's invariant ③: account-info and sponsor reads must resolve to
    /// the SAME bundler the pool would submit to. Tempo's gas reimbursement is
    /// paid to that bundler's per-Safe EOA, so reading it from a different one
    /// reimburses the wrong address and the operation is rejected on chain.
    /// `nil` means every bundler endpoint is banned or the pool is empty — the
    /// caller falls back to the built-in base.
    func bundlerBase(chainId: Int) async -> String? {
        if offline { return nil }
        bootForCall("bundler_base chain=\(chainId)")
        let callId = mintCallId()
        return await withCheckedContinuation { continuation in
            var entry = Pending(method: "", params: [])
            var resumed = false
            entry.resume = { result in
                guard !resumed else { return }
                resumed = true
                if case .ok(let value) = result.outcome { continuation.resume(returning: value as? String) }
                else { continuation.resume(returning: nil) }
            }
            pending[callId] = entry
            core.dispatch(CoreJSON.string([
                "type": "bundler_base_requested",
                "call_id": callId,
                "chain_id": chainId,
                "now_ms": Self.nowMs,
            ]))
        }
    }

    /// Every pool is stale — after a network is added, edited or removed.
    func invalidateAll() {
        bootForCall("invalidate_all")
        core.dispatch(CoreJSON.string(["type": "invalidate_all"]))
    }

    func refresh(chainId: Int) {
        bootForCall("refresh chain=\(chainId)")
        core.dispatch(CoreJSON.string(["type": "refresh_chain", "chain_id": chainId]))
    }

    // MARK: - The seven operations

    private func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "load_pool_config":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            // Bans are NOT filtered here: the core excludes them at selection,
            // and hiding them at collection would blind its self-rescue.
            let rpcs = await RpcEndpoints.collect(
                chainId: chainId, store: store, accounts: accounts
            )
            let bundlers = await RpcEndpoints.collectBundlers(chainId: chainId, accounts: accounts)
            return CoreJSON.string([
                "type": "pool_config",
                "chain_id": chainId,
                "rpc_endpoints": rpcs.map { ["url": $0.url, "source": $0.source] },
                "bundler_endpoints": bundlers.map { ["url": $0.url, "source": $0.source] },
                "now_ms": Self.nowMs,
            ])

        case "json_rpc_post":
            return await post(operation)

        case "probe_chain_id":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let url = operation["url"] as? String ?? ""
            let started = Date()
            let result = await CoreHTTP.rpc(
                url, method: "eth_chainId", params: [],
                timeout: timeout(operation)
            )
            return CoreJSON.string([
                "type": "chain_id_probed",
                "chain_id": chainId,
                "url": url,
                "reported": NetworkAdminExecutor.parseChainId(result) ?? NSNull(),
                "latency_ms": Date().timeIntervalSince(started) * 1000,
                "now_ms": Self.nowMs,
            ])

        // The jitter is the shell's because randomness is not a decision the
        // core can make and still be testable — it asks for a number in
        // [0, 1) and multiplies it by a delay it chose itself.
        case "draw_jitter":
            return CoreJSON.string([
                "type": "jitter",
                "call_id": operation["call_id"] as? String ?? "",
                "value": Double.random(in: 0..<1),
            ])

        case "start_backoff":
            let ms = (operation["delay_ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string([
                "type": "backoff_elapsed",
                "call_id": operation["call_id"] as? String ?? "",
                "now_ms": Self.nowMs,
            ])

        case "persist_bans":
            RpcEndpoints.saveBans(operation["entries"] as? [[String: Any]] ?? [], store: store)
            return CoreJSON.string(["type": "persisted"])

        case "conclude":
            conclude(operation)
            return CoreJSON.string(["type": "concluded"])

        default:
            // A pool operation this build does not know cannot be answered with
            // anything meaningful — but it must be answered, or the machine
            // waits forever holding every caller behind it.
            VelaLog.failure(.rpc, kind: "unhandled_operation", "\(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "concluded"])
        }
    }

    /// POST the payload the shell is holding for this call.
    ///
    /// The outcome the core receives is a **classification**, never the body:
    /// did it answer, with what JSON-RPC error, or did the transport fail and
    /// how. The body stays here under the URL that produced it, because the
    /// core's `Conclude` names a URL rather than carrying a result.
    /// The headers of one `json_rpc_post`: the relay's RPC header exactly when
    /// the core set `x_rpc_url` — which it does on bundler calls only.
    static func postHeaders(_ operation: [String: Any]) -> [String: String] {
        CoreHTTP.relayRpcHeaders(operation["x_rpc_url"] as? String)
    }

    private func post(_ operation: [String: Any]) async -> String {
        let callId = operation["call_id"] as? String ?? ""
        let url = operation["url"] as? String ?? ""
        let started = Date()

        guard let entry = pending[callId] else {
            // The caller went away. Answer so the core can finish the call
            // rather than hold the endpoint sweep open on a question nobody
            // is waiting for.
            return CoreJSON.string([
                "type": "post_outcome", "call_id": callId, "url": url,
                "outcome": ["type": "network"],
                "latency_ms": 0, "now_ms": Self.nowMs,
            ])
        }

        // Spec 098 §5: a bundler call names the chain's RPC to the relay. The
        // core sets `x_rpc_url` on bundler calls only — never on a call to an
        // RPC provider.
        let reply = await CoreHTTP.rpcEnvelope(
            url, method: entry.method, params: entry.params, timeout: timeout(operation),
            headers: Self.postHeaders(operation)
        )
        let latency = Date().timeIntervalSince(started) * 1000

        var outcome: [String: Any]
        switch reply {
        case .response(let body):
            if let error = body["error"] as? [String: Any] {
                // Built as `[String: Any]` explicitly: a literal mixing Int,
                // String and NSNull has no single element type Swift will
                // infer, and the diagnostic points at the dictionary rather
                // than at the mix.
                var info: [String: Any] = [:]
                info["code"] = (error["code"] as? NSNumber)?.intValue ?? NSNull()
                info["message"] = (error["message"] as? String) ?? NSNull()
                outcome = ["type": "response", "error": info]
                // Held for `conclude`: when the core rules this answer a
                // success (an execution error is a valid response), the
                // caller must receive the sentence rather than a null.
                pending[callId]?.errors[url] = error
            } else {
                outcome = ["type": "response", "error": NSNull()]
                pending[callId]?.bodies[url] = body["result"] ?? nil
            }
        case .httpError(let status):
            outcome = ["type": "http_error", "status": status]
        case .nonJSON:
            outcome = ["type": "non_json"]
        case .timeout:
            outcome = ["type": "timeout"]
        case .network:
            outcome = ["type": "network"]
        case .notConnected:
            outcome = ["type": "not_connected"]
        }

        return CoreJSON.string([
            "type": "post_outcome", "call_id": callId, "url": url,
            "outcome": outcome, "latency_ms": latency, "now_ms": Self.nowMs,
        ])
    }

    /// The core has decided. Hand the waiting caller its answer and forget the
    /// call — the bodies it was holding are the largest thing in this class.
    private func conclude(_ operation: [String: Any]) {
        let callId = operation["call_id"] as? String ?? ""
        guard let entry = pending.removeValue(forKey: callId) else { return }
        let verdict = operation["verdict"] as? [String: Any] ?? [:]
        // Absent on the verdicts that carry none (a URL lookup): no POST.
        let maybeDelivered = verdict["maybe_delivered"] as? Bool ?? false
        func answer(_ outcome: RpcOutcome, held: String? = nil) {
            entry.resume?(RpcCallResult(outcome: outcome, maybeDelivered: maybeDelivered, heldErrorJson: held))
        }

        switch verdict["type"] as? String ?? "" {
        case "respond":
            let url = verdict["url"] as? String ?? ""
            if let body = entry.bodies[url] {
                answer(.ok(body))
            } else if let error = entry.errors[url] {
                answer(.rpcError(
                    code: (error["code"] as? NSNumber)?.intValue,
                    message: (error["message"] as? String) ?? ""
                ), held: CoreJSON.string(error))
            } else {
                answer(.ok(nil))
            }
        case "range_cap":
            let url = verdict["url"] as? String ?? ""
            // The error the endpoint answered, as it came: the find-event
            // judges a range error itself (T180), and a span alone would make
            // it retry the same too-wide window forever.
            answer(.rangeCap(
                url: url,
                maxSpan: (verdict["max_span"] as? NSNumber)?.doubleValue ?? 0
            ), held: entry.errors[url].map { CoreJSON.string($0) })
        case "best_rpc_url":
            answer(.ok(verdict["url"] as? String))
        case "bundler_base":
            answer(.ok(verdict["base_url"] as? String))
        default:
            answer(.failed(rateLimited: verdict["rate_limited"] as? Bool ?? false))
        }
    }

    private func timeout(_ operation: [String: Any]) -> TimeInterval {
        let ms = (operation["timeout_ms"] as? NSNumber)?.doubleValue ?? 0
        return ms > 0 ? ms / 1000 : CoreHTTP.Timeout.rpcRead
    }

    private func mintCallId() -> String {
        nextCallId += 1
        return "ios-\(nextCallId)"
    }

    private static var nowMs: Double { Date().timeIntervalSince1970 * 1000 }
}

/// The pool's view: what the home screen needs to know about the network's
/// health, and the ban map as persisted.
struct RpcPoolViewWire: Decodable, Equatable {
    let failedChains: [Int]
    let rateLimitedChains: [Int]
    let banned: [RpcBanEntryWire]
    /// Spec 082 RF1: chains one call's first pass could not reach at all.
    var unreachedChains: [Int] = []
}

struct RpcBanEntryWire: Decodable, Equatable {
    let url: String
    let bannedAtMs: Double
    let permanent: Bool
}
