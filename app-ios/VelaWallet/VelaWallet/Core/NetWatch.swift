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
//    `netHealthStep`: a give-up that no rate limit explains is a miss
//    (timeouts included — the pool gives up after its passes), any answer is
//    a reach. Three misses in a row are "offline", and the first answer after
//    them is the edge.
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

    /// Calls in a row that never reached a server — the core's state.
    private(set) var misses: UInt32 = 0
    private(set) var online = true
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

    /// One routed call's outcome. Only a give-up no rate limit explains is a
    /// miss; a refusal, a value, a 429 — the server answered.
    func observe(_ outcome: RpcOutcome) {
        if case .failed(let rateLimited) = outcome, !rateLimited {
            step(reached: false)
        } else {
            step(reached: true)
        }
    }

    /// The core's `netHealthStep`, and its edge.
    func step(reached: Bool) {
        let next = netHealthStep(misses: misses, online: online, reached: reached)
        misses = next.misses
        online = next.online
        switch next.edge {
        case "came_back":
            cameBack(source: "rpc")
        case "went_offline":
            VelaLog.failure(.net, kind: "offline", "misses=\(next.misses)")
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
            self.misses = 0
            self.online = true
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
