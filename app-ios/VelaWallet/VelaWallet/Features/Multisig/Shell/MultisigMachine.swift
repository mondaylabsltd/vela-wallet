//
//  MultisigMachine.swift
//  VelaWallet
//
//  One of the core's multisig machines, running in this app: its view, its
//  events in, its operations out through `MultisigExecutor`. The machine
//  decides; this holds the view the screens draw (the web's
//  `shell/machine.svelte.ts`).
//
//  It runs on `CoreDriver`, the loop every machine of this app shares — so
//  the multisig machines get the same three properties the session and send
//  machines have: bridge calls serialised on the main actor, nothing thrown
//  reaching the loop, a cancelled effect never answered. An answer the core
//  refuses is answered once more with the operation's own failure, so no
//  machine is ever left waiting.
//

import Foundation
import Observation
import VelaCore

/// The four calls every multisig machine has (`multisig_bridge.rs`).
protocol MultisigCore: AnyObject {
    func dispatch(eventJson: String) throws -> String
    func resolveEffect(effectId: UInt64, resultJson: String) throws -> String
    func view() throws -> String
}

extension MultisigAccountsCore: MultisigCore {}
extension MultisigProposalsCore: MultisigCore {}
extension MultisigMembersCore: MultisigCore {}
extension MultisigOutsidePageCore: MultisigCore {}
extension MultisigOutsideSessionCore: MultisigCore {}

/// `CoreDriver` reads a `CoreBridge`. The multisig machines answer every call
/// with their whole view (they have no `skip_unchanged_views`), which the
/// driver reads the same way.
private final class MultisigBridge: CoreBridge {
    let core: MultisigCore
    init(_ core: MultisigCore) { self.core = core }
    func dispatch(eventJson: String) throws -> String { try core.dispatch(eventJson: eventJson) }
    func resolveEffect(effectId: UInt64, resultJson: String) throws -> String {
        try core.resolveEffect(effectId: effectId, resultJson: resultJson)
    }
    func view() throws -> String { try core.view() }
    func skipUnchangedViews(includeVolatile: Bool) throws {}
}

@MainActor
@Observable
final class MultisigMachine {
    /// The machine's view, as last committed (the raw JSON object).
    private(set) var view: [String: Any] = [:]
    /// Bumped on every committed view: what a SwiftUI body keys on.
    private(set) var version = 0
    let name: String

    @ObservationIgnored private var driver: CoreDriver!
    @ObservationIgnored private var listeners: [UUID: ([String: Any]) -> Void] = [:]
    @ObservationIgnored private let executor: MultisigExecutor

    init(core: MultisigCore, ports: MultisigPorts, name: String) {
        self.name = name
        let executor = MultisigExecutor(ports: ports)
        self.executor = executor
        if let text = try? core.view(), let object = try? CoreJSON.object(text) {
            view = object
        }
        // Built once `self` exists: its views come back here.
        let driver = CoreDriver(
            bridge: MultisigBridge(core),
            perform: { operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.commit(view) },
            onFault: { error in
                VelaLog.failure(.multisig, kind: "multisig_\(name)_fault", "\(error)")
            }
        )
        driver.neutralAnswer = { operation in
            MultisigExecutor.failureJson(for: operation, detail: "the core refused the answer")
        }
        self.driver = driver
    }

    private func commit(_ view: [String: Any]) {
        self.view = view
        version &+= 1
        for listener in listeners.values { listener(view) }
    }

    /// Send one event (a dictionary the core's `Event` reads).
    func dispatch(_ event: [String: Any]) {
        driver.dispatch(CoreJSON.string(event))
    }

    /// Told every committed view until the returned stop.
    @discardableResult
    func listen(_ onView: @escaping ([String: Any]) -> Void) -> () -> Void {
        let id = UUID()
        listeners[id] = onView
        return { [weak self] in self?.listeners.removeValue(forKey: id) }
    }

    /// The first view `pick` finds something in (now, or a later one), or
    /// `nil` past `timeout`.
    func until<T>(timeout: TimeInterval = 600, _ pick: @escaping ([String: Any]) -> T?) async -> T? {
        if let now = pick(view) { return now }
        return await withCheckedContinuation { continuation in
            var stop: (() -> Void)?
            var finished = false
            let finish: (T?) -> Void = { value in
                guard !finished else { return }
                finished = true
                stop?()
                continuation.resume(returning: value)
            }
            stop = listen { view in
                if let found = pick(view) { finish(found) }
            }
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(timeout * 1_000_000_000))
                finish(nil)
            }
        }
    }

    /// No effect in flight: the machine waits on nothing but its next event.
    var isIdle: Bool { driver.isIdle }

    func dispose() {
        listeners.removeAll()
        driver.dispose()
    }

    /// Typed: the view decoded as `T` (`nil` when it does not read).
    ///
    /// Read with the wire's own key names — no snake-case conversion: a view
    /// carries records the core reads back whole (a stored multisig's
    /// genesis is `salt_nonce`), and maps keyed by address or by hash.
    func decoded<T: Decodable>(_ type: T.Type) -> T? {
        Self.decode(type, from: view)
    }

    static let decoder = JSONDecoder()

    static func decode<T: Decodable>(_ type: T.Type, from object: Any) -> T? {
        guard let data = try? JSONSerialization.data(withJSONObject: object, options: [.fragmentsAllowed]) else { return nil }
        return try? decoder.decode(type, from: data)
    }
}
