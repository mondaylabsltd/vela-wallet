//
//  MultisigOutsideSession.swift
//  VelaWallet
//
//  Vela's side of a live session with an outside owner's wallet (M4), run by
//  the core's session machine (`multisig_outside::session::OutsideSession`,
//  uniffi `MultisigOutsideSessionCore`): the room (or a pairing kept
//  before), `walletpair_connect`, the owner among the wallet's accounts, the
//  network switched, the SafeTx signed — and the answer verified by the core
//  before it is handed back — or the transaction sent. The machine decides;
//  this holds its view for the sheet, hands it the PeerConnect rooms, the
//  store and the mailbox (the web's `outside-session.svelte.ts`).
//
//  The outside owner opens Vela's connect page in a browser with its own
//  wallet (`connectLink`); that page and this session meet in the room.
//

import Foundation
import Observation
import VelaCore

@MainActor
@Observable
final class MultisigOutsideSessionModel: Identifiable {
    let id = UUID()
    let request: [String: Any]
    /// The session machine's view, raw.
    private(set) var view: MsJSON = .null
    /// How the ask ended, once it did (`approved`, `sent`, `cancelled`).
    @ObservationIgnored var onOutcome: (([String: Any]) -> Void)?

    @ObservationIgnored private var machine: MultisigMachine!
    @ObservationIgnored private let rooms: MultisigPeerRooms
    @ObservationIgnored private let base: MultisigPorts
    @ObservationIgnored private let relayUrl: String?
    @ObservationIgnored private var mailboxTimer: Task<Void, Never>?
    @ObservationIgnored private var answered = false
    @ObservationIgnored private var serviceUrl: String = ""

    /// How often the mailbox is looked at for an approval made from the link
    /// alone (r4C-2, the web's `MAILBOX_EVERY_MS`).
    static let mailboxEveryMs: UInt64 = 8_000

    init(request: [String: Any], ports: MultisigPorts, relay: String?, rooms: MultisigPeerRooms? = nil, serviceUrl: String = "") {
        self.request = request
        self.base = ports
        self.relayUrl = relay
        self.serviceUrl = serviceUrl
        self.rooms = rooms ?? MultisigPeerRooms.live(relay: relay)
        let scoped = MultisigScopedPorts(base: ports, peer: self.rooms)
        machine = MultisigMachine(core: MultisigOutsideSessionCore(), ports: scoped, name: "outside session")
        self.rooms.tell = { [weak self] event in self?.machine.dispatch(event) }
        _ = machine.listen { [weak self] view in self?.moved(view) }
    }

    func start() {
        machine.dispatch([
            "type": "start",
            "request": request,
            "service_url": serviceUrl.isEmpty ? MultisigServiceURL.current() : serviceUrl,
            "relay": relayUrl ?? NSNull(),
            "origin": NSNull(),
        ])
        mailboxTimer = Task { @MainActor [weak self] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: Self.mailboxEveryMs * 1_000_000)
                guard let self, !Task.isCancelled else { return }
                self.machine.dispatch(["type": "mailbox_tick"])
            }
        }
    }

    private func moved(_ raw: [String: Any]) {
        view = MsJSON(any: raw)
        if !answered, let outcome = raw["outcome"] as? [String: Any] {
            answered = true
            mailboxTimer?.cancel()
            onOutcome?(outcome)
        }
    }

    /// The page to open in a browser: with the invite for a new room, none for
    /// one paired before — the members who may sign and the transaction, all
    /// in the fragment (the core's `connectLink`).
    var link: String? {
        let invite = view["invite"]?.string
        let members = view["members"]?.array.compactMap(\.string) ?? []
        let tx = view["tx"].flatMap { tx -> (Int, String, String)? in
            guard let chain = tx["chainId"]?.int, let safe = tx["safe"]?.string, let hash = tx["safeTxHash"]?.string else { return nil }
            return (chain, safe, hash)
        }
        return MsRules.connectLink(
            page: MultisigServiceURL.connectPage(),
            invite: invite, members: members,
            tx: tx.map { (chainId: $0.0, multisig: $0.1, safeTxHash: $0.2) }
        )
    }

    var phase: String { view["phase"]?.string ?? "waiting" }
    var failure: MsJSON? { view["failure"].flatMap { $0.isNull ? nil : $0 } }
    var relay: MsJSON? { view["relay"] }

    func askAgain() { machine.dispatch(["type": "ask_again"]) }
    func stopWaiting() { machine.dispatch(["type": "stop_waiting"]) }
    func anotherWallet(unpair: Bool) { machine.dispatch(["type": "another_wallet", "unpair": unpair]) }
    func retryRelay() { rooms.retryAll() }

    /// The person closed the sheet: the machine says how it ended (cancelled,
    /// unless it was answered), the rooms are left (the pairing kept).
    func close() {
        machine.dispatch(["type": "close"])
        mailboxTimer?.cancel()
        if !answered {
            answered = true
            onOutcome?(["kind": "cancelled"])
        }
        rooms.leaveAll()
    }
}

/// The app's ports, with a session's own rooms.
@MainActor
final class MultisigScopedPorts: MultisigPorts {
    private let base: MultisigPorts
    let peer: MultisigPeerTransport

    init(base: MultisigPorts, peer: MultisigPeerTransport) {
        self.base = base
        self.peer = peer
    }

    func http(method: String, url: String, body: String?, timeoutMs: Int) async throws -> (status: Int, retryAfter: String?, body: String) {
        try await base.http(method: method, url: url, body: body, timeoutMs: timeoutMs)
    }
    func rpc(chainId: Int, method: String, params: Any) async -> MultisigRpcReply {
        await base.rpc(chainId: chainId, method: method, params: params)
    }
    func storeRead(_ key: String) -> String?? { base.storeRead(key) }
    func storeWrite(_ key: String, _ value: String?) -> Bool { base.storeWrite(key, value) }
    func nowMs() -> Double { base.nowMs() }
    func random(_ bytes: Int) -> Data { base.random(bytes) }
    func passkey(_ op: [String: Any]) async -> [String: Any] { await base.passkey(op) }
    func askRisk(_ op: [String: Any]) async -> Bool { await base.askRisk(op) }
    func sendCalls(_ op: [String: Any]) async -> [String: Any] { await base.sendCalls(op) }
    func quoteCalls(_ op: [String: Any]) async -> [String: Any] { await base.quoteCalls(op) }
    func simulate(_ op: [String: Any]) async -> String? { await base.simulate(op) }
    func recipientFacts(_ op: [String: Any]) async -> (isContract: Bool?, firstTime: Bool?) { await base.recipientFacts(op) }
    func userOpStatus(_ op: [String: Any]) async -> (status: String?, txHash: String?)? { await base.userOpStatus(op) }
    func wallet(method: String, params: Any) async throws -> Any { try await base.wallet(method: method, params: params) }
    /// A session routes nothing: its outcome is its view's.
    func notice(_ notice: [String: Any]) async {}
}

/// The multisig service in use, as the machines are told it.
enum MultisigServiceURL {
    /// The core's default (`DEFAULT_GENESIS_SERVICE_URL`).
    static let defaultURL = "https://safe-genesis.getvela.app"

    static func current(defaults: UserDefaults = .standard) -> String {
        guard let raw = defaults.string(forKey: VelaStore.Key.serviceEndpoints),
              let object = try? JSONSerialization.jsonObject(with: Data(raw.utf8)) as? [String: Any],
              let url = object["genesisServiceURL"] as? String,
              !url.trimmingCharacters(in: .whitespaces).isEmpty
        else { return defaultURL }
        var clean = url.trimmingCharacters(in: .whitespaces)
        while clean.hasSuffix("/") { clean.removeLast() }
        return clean
    }

    /// Vela's connect page, at the wallet's public web address, in the
    /// person's language (the page an outside owner opens).
    static func connectPage(locale: String = MultisigServiceURL.locale) -> String {
        let origin = MsRules.string("shareOrigin", ["origin": "app://ios"]) ?? "https://getvela.app"
        return "\(origin)/\(locale)/connect"
    }

    /// The page's locale segment for the app's language.
    static var locale: String {
        let lang = UserDefaults.standard.string(forKey: VelaStore.Key.language) ?? "auto"
        return Loc.supported.contains(lang) ? lang : "en"
    }
}
