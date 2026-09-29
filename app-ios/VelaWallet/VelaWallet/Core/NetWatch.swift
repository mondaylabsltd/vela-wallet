//
//  NetWatch.swift
//  VelaWallet
//
//  Whether the network came back — so nothing waits for a person's tap once
//  it has (spec 082 RE3, W5).
//
//  Two witnesses, because either alone misses the case this exists for:
//
//  - **The pool's own calls.** A hanging proxy node (the China case) leaves
//    the system's path "satisfied" for the whole outage, so the path monitor
//    never fires; only the calls know. Every routed call is fed to the core's
//    `netHealthStep` with the chain it read: a give-up that no rate limit
//    explains is a miss (timeouts included — the pool gives up after its
//    passes), any answer is a reach. "Offline" is the core's rule (spec 082
//    RJ14): three misses in a row from at least two chains AND nothing
//    reached for 10 s — one faulted chain while the rest answer is that
//    chain's notice, never "offline" (G53: the desktop flapped every ~20 s
//    with one chain down). The first answer after it is the edge.
//  - **The system path**, for the plain case: Wi-Fi drops and returns, and
//    nothing in the app is calling right then. An unsatisfied → satisfied
//    edge, debounced 500 ms (a path flaps while it settles).
//
//  What "came back" makes happen is the root's (the failed pages, the logo
//  misses, a balance read); this only says when. The counting rule is the
//  core's, never re-derived here.
//

import Foundation
import Network
import Observation
import VelaCore

@MainActor
@Observable
final class NetWatch {

    /// The core's count, carried from call to call and handed back whole —
    /// misses, the chains they came from, and when anything last answered.
    private(set) var state: NetHealthState = netHealthFresh()
    /// Calls in a row that never reached a server.
    var misses: UInt32 { state.misses }
    var online: Bool { state.online }
    /// How many times the network came back (tests read it).
    private(set) var cameBackCount = 0

    /// The edge. Set once, by the root.
    var onCameBack: () -> Void = {}

    private let debounceMs: UInt64
    private var path: NetPathSource?
    private var lastSatisfied: Bool?
    private var debounce: Task<Void, Never>?

    init(debounceMs: UInt64 = 500) {
        self.debounceMs = debounceMs
    }

    // MARK: - The pool's calls

    /// One routed call's outcome, on the chain it read. Only a give-up no
    /// rate limit explains is a miss; a refusal, a value, a 429 — the server
    /// answered.
    func observe(_ outcome: RpcOutcome, chainId: Int?, nowMs: Double = Date().timeIntervalSince1970 * 1000) {
        if case .failed(let rateLimited) = outcome, !rateLimited {
            step(reached: false, source: chainId, nowMs: nowMs)
        } else {
            step(reached: true, source: chainId, nowMs: nowMs)
        }
    }

    /// The core's `netHealthStep`, and its edge. `source` is the chain the
    /// call read (`nil`: none — each such miss is its own source).
    func step(reached: Bool, source: Int?, nowMs: Double = Date().timeIntervalSince1970 * 1000) {
        let next = netHealthStep(
            state: state, reached: reached,
            source: source.map { UInt32(clamping: $0) }, nowMs: nowMs
        )
        state = next.state
        switch next.edge {
        case "came_back":
            cameBack(source: "rpc")
        case "went_offline":
            VelaLog.failure(
                .net, kind: "offline",
                "misses=\(next.state.misses) chains=\(next.state.sources.count + Int(next.state.unsourced))"
            )
        default:
            break
        }
    }

    // MARK: - The system path

    /// Start watching the system path (the real `NWPathMonitor`, or a test's).
    func watchPath(_ source: NetPathSource = SystemPath()) {
        path?.cancel()
        path = source
        source.start { [weak self] satisfied in self?.pathChanged(satisfied: satisfied) }
    }

    /// The path's state now. Unsatisfied → satisfied, held for the debounce,
    /// is the network coming back.
    func pathChanged(satisfied: Bool) {
        defer { lastSatisfied = satisfied }
        guard satisfied else {
            debounce?.cancel()
            debounce = nil
            return
        }
        guard lastSatisfied == false else { return }
        debounce?.cancel()
        let wait = debounceMs
        debounce = Task { @MainActor [weak self] in
            if wait > 0 { try? await Task.sleep(nanoseconds: wait * 1_000_000) }
            guard let self, !Task.isCancelled, self.lastSatisfied == true else { return }
            self.debounce = nil
            // The path is back: whatever the calls counted is over.
            self.state = netHealthFresh()
            self.cameBack(source: "path")
        }
    }

    private func cameBack(source: String) {
        cameBackCount += 1
        VelaLog.notice(.net, "net came back (\(source))")
        onCameBack()
    }
}

/// The system path, as the seam sees it — `true` = satisfied.
protocol NetPathSource: AnyObject {
    func start(_ onChange: @escaping @MainActor (Bool) -> Void)
    func cancel()
}

/// `NWPathMonitor`, delivered on the main actor.
final class SystemPath: NetPathSource {
    private let monitor = NWPathMonitor()
    private let queue = DispatchQueue(label: "app.getvela.netwatch")

    func start(_ onChange: @escaping @MainActor (Bool) -> Void) {
        monitor.pathUpdateHandler = { path in
            let satisfied = path.status == .satisfied
            Task { @MainActor in onChange(satisfied) }
        }
        monitor.start(queue: queue)
    }

    func cancel() { monitor.cancel() }

    deinit { monitor.cancel() }
}
