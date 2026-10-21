//
//  AgentStore.swift
//  VelaWallet
//
//  The AI agent, as the phone's screens see it (spec 103, spec 104 phase 7).
//
//  The agent itself is the core's — `vela-agent`, the one copy the desktop
//  runs too: the runner (check → examine → approve → perform → observe), the
//  transfers, the ClientMCP tools and the bridge connection, on a thread of
//  its own (`AgentCore`). What is this app's is what it does for it
//  (`AgentShellBridge`): its views, its stores' calls (`AgentPerform`), its
//  screens (`AgentPage`), its send.
//
//  One way in for the person and the agent: the person's own taps on the
//  commit points go through the same intents (`person`), so an agent's request
//  and a tap reach the store that owns the change by one road (spec 103 D3).
//
//  Off unless the person turns it on, and answering only while the app is in
//  front: in the background the connection is hung up, and a request waiting
//  for the person is withdrawn.
//

import Foundation
import Observation
import VelaCore

/// Settings → AI agent: `vela.agent`, `{ enabled, url }` — and the client
/// token, which is the Keychain's (`AgentTokenVault`), never the record's.
struct AgentConfig: Equatable {
    var enabled = false
    /// The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`.
    var url = ""
    /// A ClientMCP client token for this wallet.
    var token = ""

    init(enabled: Bool = false, url: String = "", token: String = "") {
        self.enabled = enabled
        self.url = url
        self.token = token
    }

    /// What is stored; anything unreadable reads as off. A `token` in the
    /// record is an older build's, to be moved to the Keychain.
    init(_ object: [String: Any]) {
        enabled = object["enabled"] as? Bool ?? false
        url = object["url"] as? String ?? ""
        token = object["token"] as? String ?? ""
    }

    /// The record as stored: no token.
    var record: [String: Any] { ["enabled": enabled, "url": url] }

    /// Whether the bridge can be dialled with what is entered.
    var ready: Bool {
        enabled && agentBridgeUrlProblem(url: url) == nil
            && !token.trimmingCharacters(in: .whitespaces).isEmpty
    }
}

/// One of the agent's requests waiting for the person.
struct AgentRequest: Identifiable, Equatable {
    let id: UInt64
    /// The action's `type`.
    let kind: String
    /// The action, as JSON.
    let actionJson: String

    var action: [String: Any] { (try? CoreJSON.object(actionJson)) ?? [:] }
}

/// One of the agent's recent calls.
struct AgentCall: Identifiable, Equatable {
    let id: UInt64
    /// The action's `type`.
    let kind: String
    /// `approving`, `performing`, `observing` while it runs.
    let stage: String?
    /// `completed`, `accepted`, `rejected` once it ended.
    let status: String?
    /// Why it was rejected.
    let code: String?
}

/// How a request ended.
struct AgentOutcome: Equatable {
    /// `completed`, `accepted` or `rejected`.
    let status: String
    let code: String?
    let detail: String?

    init(json: String) {
        let object = (try? CoreJSON.object(json)) ?? [:]
        let reason = object["reason"] as? [String: Any]
        status = object["status"] as? String ?? "rejected"
        code = reason?["code"] as? String
        detail = reason?["detail"] as? String
    }

    var done: Bool { status != "rejected" }
}

/// Hears how a person's request ended, on the agent's thread.
private final class AgentDoneHandler: AgentDone, @unchecked Sendable {
    private let body: @Sendable (String) -> Void
    init(_ body: @escaping @Sendable (String) -> Void) { self.body = body }
    func done(outcome: String) { body(outcome) }
}

@MainActor
@Observable
final class AgentStore {

    private(set) var config: AgentConfig
    private(set) var link: AgentLink = .off
    /// The agent's requests waiting for the person, oldest first.
    private(set) var approvals: [AgentRequest] = []
    /// The agent's recent calls, newest first.
    private(set) var activity: [AgentCall] = []

    /// The app's own stores, read and changed for the agent. Weak: the graph
    /// owns this store.
    @ObservationIgnored weak var graph: AppGraph?
    /// What only the drawn root can do — open a screen, erase, clear a store
    /// (`AgentPage`). Registered by `RootView`.
    @ObservationIgnored var page: AgentPage?

    @ObservationIgnored private let shelf: VelaStore
    /// The client token's place: the Keychain.
    @ObservationIgnored private let vault: AgentTokenVault
    @ObservationIgnored private var core: AgentCore!
    /// Whether the app is in front: the agent answers only then.
    @ObservationIgnored private var active = true
    /// What the core was last configured with, so a scene change that changes
    /// nothing is not a reconnect.
    @ObservationIgnored private var dialled: AgentConfig?

    init(shelf: VelaStore, vault: AgentTokenVault = KeychainTokenVault()) {
        self.shelf = shelf
        self.vault = vault
        var config = AgentConfig(shelf.readObject(VelaStore.Key.agent))
        if config.token.isEmpty {
            config.token = vault.read() ?? ""
        } else if vault.write(config.token) {
            // An older build kept it in the record: moved to the Keychain, and
            // the record written again without it.
            shelf.writeObject(VelaStore.Key.agent, config.record)
        }
        self.config = config
        let bridge = AgentShellBridge()
        self.core = AgentCore(shell: bridge, device: "phone")
        bridge.store = self
    }

    /// Connect as set up, once the graph is in place.
    func start() { connect() }

    /// The app came to the front, or left it.
    func sceneActive(_ active: Bool) {
        guard self.active != active else { return }
        self.active = active
        connect()
    }

    /// The graph is going away (an erase builds a new one): hang up for good.
    func shutdown() {
        active = false
        connect()
    }

    /// Erase This Device: the token goes too — it is not one of the `vela.*`
    /// values the sweep finds.
    func forgetToken() {
        vault.delete()
        config.token = ""
    }

    private func connect() {
        let wanted = AgentConfig(enabled: config.enabled && active, url: config.url, token: config.token)
        guard wanted != dialled else { return }
        dialled = wanted
        core.configure(enabled: wanted.enabled, bridgeUrl: wanted.url, clientToken: wanted.token)
        refresh()
    }

    /// The `configure_agent` intent, performed: what is `nil` stays as it is.
    func apply(enabled: Bool?, url: String?, token: String?) {
        var next = config
        if let enabled { next.enabled = enabled }
        if let url { next.url = url.trimmingCharacters(in: .whitespaces) }
        if let token {
            next.token = token.trimmingCharacters(in: .whitespaces)
            // The Keychain's; a token it would not keep is kept for this run only.
            if next.token.isEmpty { vault.delete() } else { vault.write(next.token) }
        }
        config = next
        shelf.writeObject(VelaStore.Key.agent, next.record)
        connect()
    }

    /// Read again what the core holds — on its `changed`.
    func refresh() {
        let link = core.link()
        if link != self.link { self.link = link }
        let approvals = Self.requests(core.approvals())
        if approvals != self.approvals { self.approvals = approvals }
        let activity = Self.calls(core.activity())
        if activity != self.activity { self.activity = activity }
    }

    /// The person answered the agent's request.
    func answer(_ id: UInt64, approved: Bool) { core.answer(id: id, approved: approved) }

    /// The person stopped the agent: nothing of its still waits, and it is
    /// switched off.
    func stop() { core.stop() }

    /// The person's own tap, through the same intents an agent's request
    /// takes (spec 103 D3). `then` hears how it ended.
    func person(_ action: [String: Any], then: (@MainActor (AgentOutcome) -> Void)? = nil) {
        let done = then.map { then in
            AgentDoneHandler { json in
                DispatchQueue.main.async { MainActor.assumeIsolated { then(AgentOutcome(json: json)) } }
            }
        }
        if !core.invokePerson(actionJson: CoreJSON.string(action), done: done) {
            print("[vela-wallet] agent: not an action: \(action["type"] ?? "?")")
        }
    }

    /// `person`, waited for.
    @discardableResult
    func person(_ action: [String: Any]) async -> AgentOutcome {
        await withCheckedContinuation { continuation in
            person(action) { outcome in continuation.resume(returning: outcome) }
        }
    }

    /// The passkey's ceremony moved: `started` (its prompt is up) or `signed`.
    func ceremony(_ event: String) { core.ceremony(event: event) }

    /// The transfer on the open send's review, by its operation id — adopted
    /// if the person filled the form themselves, so their confirm goes
    /// through `submit_transfer` as an agent's would.
    func reviewing() -> String? {
        guard let json = core.currentReview() else { return nil }
        return (try? CoreJSON.object(json))?["id"] as? String
    }

    /// The MCP endpoint an agent adds, derived from the bridge URL.
    var endpoint: String? { agentMcpEndpoint(url: config.url) }

    /// Why the bridge URL cannot be used — `invalid`, `insecure` — once one
    /// is entered.
    var urlProblem: String? {
        config.url.isEmpty ? nil : agentBridgeUrlProblem(url: config.url)
    }

    /// The agent's request on show: the oldest still waiting.
    var pending: AgentRequest? { approvals.first }

    /// What the capsule shows: set up and switched on, or a request waiting.
    var showsCapsule: Bool { config.ready || pending != nil }

    private static func list(_ json: String) -> [[String: Any]] {
        guard let data = json.data(using: .utf8),
              let list = try? JSONSerialization.jsonObject(with: data) as? [[String: Any]]
        else { return [] }
        return list
    }

    private static func requests(_ json: String) -> [AgentRequest] {
        list(json).compactMap { row in
            guard let id = (row["id"] as? NSNumber)?.uint64Value,
                  let action = row["action"] as? [String: Any]
            else { return nil }
            return AgentRequest(
                id: id,
                kind: action["type"] as? String ?? "",
                actionJson: CoreJSON.string(action)
            )
        }
    }

    private static func calls(_ json: String) -> [AgentCall] {
        list(json).compactMap { row in
            guard let id = (row["id"] as? NSNumber)?.uint64Value else { return nil }
            let outcome = row["outcome"] as? [String: Any]
            return AgentCall(
                id: id,
                kind: row["action"] as? String ?? "",
                stage: row["stage"] as? String,
                status: outcome?["status"] as? String,
                code: (outcome?["reason"] as? [String: Any])?["code"] as? String
            )
        }
    }
}
