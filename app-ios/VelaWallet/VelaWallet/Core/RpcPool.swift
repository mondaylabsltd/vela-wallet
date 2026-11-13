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
    /// R22: a corroborated call's two answers (`callCorroborated`); `nil` for
    /// every other call.
    var corroborated: RpcCorroborated? = nil
}

/// R22 (docs/multisig-machines.md §36, review 1-2): a read a signing verdict
/// stands on, as two independent nodes answered it — the pool chose both
/// (`rpc_pool`'s `Respond { witness_url, own_node }`); what they MEAN, and
/// whether they agree, is the core's. Bodies are the nodes' JSON-RPC
/// members as they came: `{"result": …}` or `{"error": {code, message, data?}}`.
struct RpcCorroborated {
    /// The first node's answer; `nil`: the pool gave up — no node answered.
    let primary: [String: Any]?
    /// The second, independent node's answer; `nil`: nobody independent
    /// answered (the core then believes no plain verdict on it).
    let witness: [String: Any]?
    /// The pool holds the person's own node alone ("Only use my node") and it
    /// answered — trusted alone, by their choice, and said so.
    let ownNode: Bool
    /// The pool gave up because every node was rate limiting.
    var rateLimited: Bool = false
    /// §39 (r22-2): nobody independent answered BECAUSE the pool holds no
    /// other provider for this chain (the verdict's `sole_provider`) —
    /// configuration, not failure. Passed into the envelope as it came.
    var soleProvider: Bool = false

    /// The one envelope the core reads (`rpc_corroborated_envelope`) — built
    /// by the core's helper, never by hand.
    var envelopeJson: String {
        rpcCorroboratedEnvelope(
            primaryJson: primary.map { CoreJSON.string($0) },
            witnessJson: witness.map { CoreJSON.string($0) },
            ownNode: ownNode,
            soleProvider: soleProvider
        )
    }

    /// The envelope as an object (the multisig's `rpc_corroborated`).
    var envelope: [String: Any] {
        (try? CoreJSON.object(envelopeJson)) ?? ["unreachable": true]
    }

    /// One node's body as the pool's outcome — what a reader of a single
    /// answer (`safe_owner_read`'s `answer` / `witness`) is handed.
    static func outcome(_ body: [String: Any]?, rateLimited: Bool = false) -> RpcOutcome {
        guard let body else { return .failed(rateLimited: rateLimited) }
        if let error = body["error"] as? [String: Any] {
            return .rpcError(code: (error["code"] as? NSNumber)?.intValue, message: error["message"] as? String ?? "")
        }
        let result = body["result"]
        return .ok(result is NSNull ? nil : result)
    }

    /// The first node's answer as an outcome.
    var primaryOutcome: RpcOutcome { Self.outcome(primary, rateLimited: rateLimited) }
    /// The witness's answer as an outcome; `nil` without one.
    var witnessOutcome: RpcOutcome? { witness.map { Self.outcome($0) } }

    /// A node's body for one outcome (a test's, or a seam's).
    static func body(_ outcome: RpcOutcome) -> [String: Any]? {
        switch outcome {
        case .ok(let value): return ["result": value ?? NSNull()]
        case .rpcError(let code, let message):
            return ["error": ["code": code.map { $0 as Any } ?? NSNull(), "message": message] as [String: Any]]
        case .failed, .rangeCap: return nil
        }
    }

    /// Two independent nodes answered `outcome` alike — a test's world, or a
    /// seam standing for a pool whose nodes agreed.
    static func agreed(_ outcome: RpcOutcome) -> RpcCorroborated {
        let body = body(outcome)
        return RpcCorroborated(primary: body, witness: body, ownNode: false)
    }

    /// One node answered `outcome` and nobody independent did.
    static func single(_ outcome: RpcOutcome) -> RpcCorroborated {
        RpcCorroborated(primary: body(outcome), witness: nil, ownNode: false)
    }

    /// No node answered.
    static let unreachable = RpcCorroborated(primary: nil, witness: nil, ownNode: false)
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
    /// "Only use my node": chains public nodes are serving because the
    /// person's own is not answering — read, but said (the balance's
    /// `fallback_chain_ids`).
    private(set) var fallbackChains: [Int] = []
    /// Chains read through the person's own node alone that cannot be read
    /// now — the balance's `own_node_chain_ids` (of its failed ones).
    private(set) var ownNodeUnreachedChains: [Int] = []
    /// The pool's view as the core wrote it (`RpcPoolView`) — what the core's
    /// `rpc_node_standing` reads a chain's standing from, so Settings' rows
    /// and the balance read the person's node by one rule. `nil` before the
    /// first view.
    ///
    /// Written out when read (native parity Phase 4): `viewRevision` is what
    /// a reader observes, so it is still redrawn when the view moves.
    var viewJson: String? {
        _ = viewRevision
        return core.json
    }
    /// Bumped at every view that changed — the observable half of `viewJson`.
    private(set) var viewRevision = 0

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
        /// R22: a corroborated read — concluded with both nodes' answers.
        var corroborate = false

        /// One URL's answer as its JSON-RPC member (`{result}` / `{error}`),
        /// `nil` when that URL answered nothing this call holds.
        func body(_ url: String) -> [String: Any]? {
            if let value = bodies[url] { return ["result": value ?? NSNull()] }
            if let error = errors[url] { return ["error": error] }
            return nil
        }
    }

    /// Every routed call's outcome, once concluded, with the chain it read —
    /// the network's health is read from these (`NetWatch`, spec 082 RE3):
    /// the calls are the only witness of a proxy node that hangs while the
    /// system path stays up, and the chain is what tells one faulted chain
    /// from a network that is gone (RJ14).
    var onOutcome: ((_ outcome: RpcOutcome, _ chainId: Int) -> Void)?

    /// r12-3: the chains out of reach now (`RpcPoolView.out_of_reach_chains`),
    /// whenever that set changes — the balance machine is told
    /// (`pool_unreached`) and re-reads, forced, a network the account is read
    /// on that has just gone out of reach. A warm outage aged "Updated 2m"
    /// for minutes before: the home's poll is ten.
    ///
    /// r13-4: with them, the SILENT ones (`RpcPoolView.silent_chains`: three
    /// different nodes said nothing at all for 5 s, nothing answered since)
    /// — the core names those at once, as the web's `onPoolOutOfReach` has
    /// it say them; told whenever either set changes.
    var onOutOfReach: ((_ chainIds: [Int], _ silent: [Int]) -> Void)?
    /// The last sets said through `onOutOfReach`.
    private(set) var outOfReachChains: [Int] = []
    private(set) var silentChains: [Int] = []

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

    /// What leaves the device, as seams — each `nil` (the app): the real
    /// network and the stored endpoints. A test's pool answers from a
    /// script, so the routing core runs for real over a network that is not
    /// there.
    struct Transport {
        /// A chain's RPC endpoints (`load_pool_config`) and "Only use my node".
        var endpoints: (@MainActor (_ chainId: Int) async -> (rpcs: [CollectedEndpoint], ownOnly: Bool))?
        /// One JSON-RPC POST.
        var post: (@MainActor (_ url: String, _ method: String, _ params: [Any]) async -> CoreHTTP.RpcReply)?
        /// `eth_chainId` of one endpoint (`probe_chain_id`); `nil` = no answer.
        var chainId: (@MainActor (_ url: String) async -> Int?)?
    }
    private let transport: Transport

    init(store: VelaStore, accounts: AccountStore, offline: Bool = false, transport: Transport = Transport()) {
        self.store = store
        self.accounts = accounts
        self.offline = offline
        self.transport = transport
        self.core = CoreStore(
            bridge: RpcPoolCore(),
            perform: { [weak self] operation in
                await self?.perform(operation) ?? CoreJSON.string(["type": "concluded"])
            },
            onView: { [weak self] view in self?.viewArrived(view) },
            onFault: { VelaLog.failure(.rpc, kind: "pool_fault", VelaLog.error($0)) },
            keepsJson: true,
            // Every call settles through this machine, and most leave its
            // view as it was: the core sends none for those, and nobody
            // watching the pool is woken (native parity Phase 4). Its
            // volatile `pending_urls` are the hedging shells' (desktop, web):
            // not asked for (`CoreStore` opts in without volatile fields), so
            // a step of a call no longer reads as a changed view.
            skipsUnchanged: true
        )
    }

    /// The pool's view, mirrored — each field written only when it moved, so
    /// a screen that reads one list is not redrawn for another's change.
    func viewArrived(_ view: RpcPoolViewWire) {
        if failedChains != view.failedChains { failedChains = view.failedChains }
        if rateLimitedChains != view.rateLimitedChains { rateLimitedChains = view.rateLimitedChains }
        if unreachedChains != view.unreachedChains { unreachedChains = view.unreachedChains }
        if fallbackChains != view.fallbackChains { fallbackChains = view.fallbackChains }
        if ownNodeUnreachedChains != view.ownNodeUnreachedChains {
            ownNodeUnreachedChains = view.ownNodeUnreachedChains
        }
        let outOfReach = view.outOfReachChains.sorted()
        let silent = view.silentChains.sorted()
        if outOfReachChains != outOfReach || silentChains != silent {
            outOfReachChains = outOfReach
            silentChains = silent
            onOutOfReach?(outOfReach, silent)
        }
        viewRevision &+= 1
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
        kind: String = "rpc",
        corroborate: Bool = false
    ) async -> RpcCallResult {
        let corroborate = corroborate && kind == "rpc"
        if offline {
            return RpcCallResult(
                outcome: .failed(rateLimited: false), maybeDelivered: false, heldErrorJson: nil,
                corroborated: corroborate ? .unreachable : nil
            )
        }
        // A pool that cannot send this chain's calls answers at once, and
        // nothing left: no network was asked, so the network's health hears
        // nothing of it either (`onOutcome`).
        if let kind = unsendable(chainId: chainId) {
            VelaLog.failure(.rpc, kind: kind, "chain=\(chainId) method=\(method)")
            return RpcCallResult(
                outcome: .failed(rateLimited: false), maybeDelivered: false, heldErrorJson: nil,
                corroborated: corroborate ? .unreachable : nil
            )
        }
        // Never refused for want of a boot (issue #483): boot, then route.
        bootForCall("method=\(method) chain=\(chainId)")
        let callId = mintCallId()
        let result = await withCheckedContinuation { continuation in
            var entry = Pending(method: method, params: params)
            entry.corroborate = corroborate
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
                // r17-10's person's-own-ask flag is the web's; this shell
                // does not say it (as on Android), so it is said as false.
                "person": false,
                // R22: a read a signing verdict stands on — two independent
                // nodes, each checked for this chain first, never hedged.
                "corroborate": corroborate,
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

    /// R22 (review 1-2): a read a signing verdict stands on — the
    /// simulation, a multisig guard's reads, an unlisted Safe's `isOwner` —
    /// answered by two independent nodes (or the person's own node alone, by
    /// their choice). The pool picks both; the core compares them. Never
    /// hedged and never answered early: this shell answers a call only on
    /// the core's `Conclude`.
    func callCorroborated(chainId: Int, method: String, params: [Any] = []) async -> RpcCorroborated {
        let routed = await callDetailed(chainId: chainId, method: method, params: params, kind: "rpc", corroborate: true)
        if let corroborated = routed.corroborated { return corroborated }
        if case .failed(let rateLimited) = routed.outcome {
            return RpcCorroborated(primary: nil, witness: nil, ownNode: false, rateLimited: rateLimited)
        }
        return .single(routed.outcome)
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

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "load_pool_config":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            if let endpoints = transport.endpoints {
                let scripted = await endpoints(chainId)
                return CoreJSON.string([
                    "type": "pool_config",
                    "chain_id": chainId,
                    "rpc_endpoints": scripted.rpcs.map { ["url": $0.url, "source": $0.source] },
                    "bundler_endpoints": [] as [Any],
                    "own_only": scripted.ownOnly,
                    "now_ms": Self.nowMs,
                ])
            }
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
                // The person's "Only use my node": what it means — and that it
                // means nothing without an endpoint of theirs — is the core's.
                "own_only": RpcEndpoints.ownNodeOnly(chainId: chainId, store: store),
                "now_ms": Self.nowMs,
            ])

        case "json_rpc_post":
            return await post(operation)

        // ALWAYS answered, race or none (R22): a corroborated read asks it of
        // each endpoint before its POST and waits on the answer.
        case "probe_chain_id":
            let chainId = (operation["chain_id"] as? NSNumber)?.intValue ?? 0
            let url = operation["url"] as? String ?? ""
            let started = Date()
            let reported: Int?
            if let probe = transport.chainId {
                reported = await probe(url)
            } else {
                let result = await CoreHTTP.rpc(
                    url, method: "eth_chainId", params: [],
                    timeout: timeout(operation)
                )
                reported = NetworkAdminExecutor.parseChainId(result)
            }
            return CoreJSON.string([
                "type": "chain_id_probed",
                "chain_id": chainId,
                "url": url,
                "reported": reported.map { $0 as Any } ?? NSNull(),
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
        let reply: CoreHTTP.RpcReply
        if let post = transport.post {
            reply = await post(url, entry.method, entry.params)
        } else {
            reply = await CoreHTTP.rpcEnvelope(
                url, method: entry.method, params: entry.params, timeout: timeout(operation),
                headers: Self.postHeaders(operation)
            )
        }
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
        // R22: a corroborated read goes back with both nodes' answers — the
        // one the core named (`url`) and its independent witness
        // (`witness_url`), or the person's own node alone (`own_node`). The
        // pool giving up is no answer at all (`primary` nil → unreachable).
        let corroborated: RpcCorroborated? = entry.corroborate ? Self.corroborated(entry, verdict: verdict) : nil
        func answer(_ outcome: RpcOutcome, held: String? = nil) {
            entry.resume?(RpcCallResult(
                outcome: outcome, maybeDelivered: maybeDelivered, heldErrorJson: held,
                corroborated: corroborated
            ))
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

    /// A corroborated call's conclusion, as both nodes answered it.
    private static func corroborated(_ entry: Pending, verdict: [String: Any]) -> RpcCorroborated {
        guard verdict["type"] as? String == "respond" else {
            return RpcCorroborated(
                primary: nil, witness: nil, ownNode: false,
                rateLimited: verdict["rate_limited"] as? Bool ?? false
            )
        }
        let url = verdict["url"] as? String ?? ""
        return RpcCorroborated(
            primary: entry.body(url) ?? ["result": NSNull()],
            witness: (verdict["witness_url"] as? String).flatMap { entry.body($0) },
            ownNode: verdict["own_node"] as? Bool ?? false,
            soleProvider: verdict["sole_provider"] as? Bool ?? false
        )
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
    /// "Only use my node" (owner decision 2026-10-28): the chains whose own
    /// node is not answering and whose reads public nodes served for now.
    var fallbackChains: [Int] = []
    /// The chains read through the person's own node alone.
    var ownOnlyChains: [Int] = []
    /// Of those, the ones that cannot be read now — "Can't reach your node".
    var ownNodeUnreachedChains: [Int] = []
    /// r12-3: the chains out of reach now, by the core's one rule (failed
    /// or unreached, never only rate limited) — what the balance machine
    /// hears as `pool_unreached`.
    var outOfReachChains: [Int] = []
    /// r13-4: of them, the SILENT ones (several nodes said nothing at all
    /// for seconds) — handed on beside them, named at once.
    var silentChains: [Int] = []

    private enum CodingKeys: String, CodingKey {
        case failedChains, rateLimitedChains, banned, unreachedChains
        case fallbackChains, ownOnlyChains, ownNodeUnreachedChains, outOfReachChains, silentChains
    }

    init(
        failedChains: [Int], rateLimitedChains: [Int], banned: [RpcBanEntryWire],
        unreachedChains: [Int] = [], fallbackChains: [Int] = [], ownOnlyChains: [Int] = [],
        ownNodeUnreachedChains: [Int] = [], outOfReachChains: [Int] = [], silentChains: [Int] = []
    ) {
        self.failedChains = failedChains
        self.rateLimitedChains = rateLimitedChains
        self.banned = banned
        self.unreachedChains = unreachedChains
        self.fallbackChains = fallbackChains
        self.ownOnlyChains = ownOnlyChains
        self.ownNodeUnreachedChains = ownNodeUnreachedChains
        self.outOfReachChains = outOfReachChains
        self.silentChains = silentChains
    }

    /// The newer lists are read when present: a view from an older core reads
    /// as none — never a view this app cannot decode.
    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        failedChains = try c.decode([Int].self, forKey: .failedChains)
        rateLimitedChains = try c.decode([Int].self, forKey: .rateLimitedChains)
        banned = try c.decode([RpcBanEntryWire].self, forKey: .banned)
        unreachedChains = try c.decodeIfPresent([Int].self, forKey: .unreachedChains) ?? []
        fallbackChains = try c.decodeIfPresent([Int].self, forKey: .fallbackChains) ?? []
        ownOnlyChains = try c.decodeIfPresent([Int].self, forKey: .ownOnlyChains) ?? []
        ownNodeUnreachedChains = try c.decodeIfPresent([Int].self, forKey: .ownNodeUnreachedChains) ?? []
        outOfReachChains = try c.decodeIfPresent([Int].self, forKey: .outOfReachChains) ?? []
        silentChains = try c.decodeIfPresent([Int].self, forKey: .silentChains) ?? []
    }
}

struct RpcBanEntryWire: Decodable, Equatable {
    let url: String
    let bannedAtMs: Double
    let permanent: Bool
}
