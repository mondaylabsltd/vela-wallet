//
//  MultisigPeerRooms.swift
//  VelaWallet
//
//  The PeerConnect transport behind the multisig machines' `peer_*`
//  operations (the web's `shell/peer.ts` + `external/relay.ts`): rooms
//  (Noise end to end; the relay reads nothing) over the Swift PeerConnect
//  client, and the relay's token. Nothing of WalletPair: what is sent and
//  what an arrival means are the core's (`multisig_outside`).
//
//  A room is known to the core by a handle this transport hands out. What
//  the connection says of itself goes back to the machine that opened it as
//  its own events: `peer_state` (offline / waiting / connected / closed),
//  `paired` (the record that reopens it now) and `relay` (where getting onto
//  the relay stands).
//
//  Every decision about the relay's token is the core's
//  (`multisig_rules::relay`, asked through the link door): which relay needs
//  one, whether the kept one is still good, what an endpoint answered, which
//  token a refusal forgets, how long to wait and when to stop, where the
//  connection's way onto the relay stands. This keeps the I/O only.
//

import Foundation

@MainActor
final class MultisigPeerRooms: MultisigPeerTransport {
    /// What a connection tells the machine that opened it (an event).
    var tell: (([String: Any]) -> Void)?

    private struct Open {
        let connection: PeerConnection
        let relay: MultisigRelayWatch
        var inbox: [Any] = []
        var waiting: CheckedContinuation<MultisigPeerArrival, Never>?
        var closed: String?
    }

    private var rooms: [String: Open] = [:]
    private var next = 0
    private let defaultRelay: String
    private let socketFactory: (any PeerSocketFactory)?

    init(defaultRelay: String, socketFactory: (any PeerSocketFactory)? = nil) {
        self.defaultRelay = defaultRelay
        self.socketFactory = socketFactory
    }

    /// The app's rooms: the relay a profile names, else PeerConnect's own.
    static func live(relay: String?) -> MultisigPeerRooms {
        MultisigPeerRooms(defaultRelay: relay.flatMap { $0.isEmpty ? nil : $0 } ?? MultisigRelay.defaultRelay)
    }

    // MARK: - MultisigPeerTransport

    func open(relay: String?, invite: String?, pairing: String?) async throws
        -> (room: String, invite: String?, pairing: String?) {
        let room: PeerRoom
        if let pairing { room = try PeerRoom.parse(pairing) }
        else if let invite { room = try PeerRoom.join(invite: invite) }
        else { room = try PeerRoom.create(relay: relay ?? defaultRelay) }
        let fresh = pairing == nil && invite == nil
        next += 1
        let handle = "room-\(next)"
        let watch = MultisigRelayWatch()
        let connection = MultisigRelay.connect(room: room, watch: watch, socketFactory: socketFactory)
        rooms[handle] = Open(connection: connection, relay: watch)
        connection.onMessage = { [weak self] data in
            self?.deliver(handle, .message(Self.read(data)))
        }
        connection.onPaired = { [weak self, weak connection] in
            guard let connection else { return }
            self?.tell?(["type": "paired", "room": handle, "pairing": connection.room.record])
        }
        let previous = connection.onState
        connection.onState = { [weak self] state, reason in
            previous?(state, reason)
            guard let self else { return }
            self.tell?(["type": "peer_state", "room": handle, "state": state.rawValue])
            if state == .closed {
                let why = reason?.rawValue ?? "closed"
                self.rooms[handle]?.closed = why
                self.deliver(handle, .closed(why))
            }
        }
        watch.onChange = { [weak self] status in
            self?.tell?(["type": "relay", "room": handle, "status": status])
        }
        return (handle, fresh ? room.invite : nil, room.record)
    }

    func send(room handle: String, message: Any) async {
        guard let open = rooms[handle], open.closed == nil else { return }
        let text: String
        if let string = message as? String { text = string }
        else if let data = try? JSONSerialization.data(withJSONObject: message, options: [.fragmentsAllowed]) {
            text = String(decoding: data, as: UTF8.self)
        } else { return }
        _ = try? open.connection.send(text)
    }

    func receive(room handle: String) async -> MultisigPeerArrival {
        guard var open = rooms[handle] else { return .closed("closed") }
        if !open.inbox.isEmpty {
            let first = open.inbox.removeFirst()
            rooms[handle] = open
            return .message(first)
        }
        if let closed = open.closed { return .closed(closed) }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                if var current = rooms[handle] {
                    current.waiting?.resume(returning: .closed("left"))
                    current.waiting = continuation
                    rooms[handle] = current
                } else {
                    continuation.resume(returning: .closed("closed"))
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.deliver(handle, .closed("left"), dropMessage: true) }
        }
    }

    func close(room handle: String, cancel: Bool, end: Bool) async {
        // `walletpair_cancel` is the core's message, sent before this.
        guard let open = rooms[handle] else { return }
        if end { open.connection.close() } else { open.connection.leave() }
        rooms[handle]?.closed = end ? "closed" : "left"
        deliver(handle, .closed(end ? "closed" : "left"))
        rooms.removeValue(forKey: handle)
    }

    // MARK: - The person's controls

    /// Try the relay again now (a wait cut short, a stop undone).
    func retryAll() {
        for open in rooms.values {
            open.relay.retry()
            open.connection.reconnectNow()
        }
    }

    /// Leave every room (the screen went away): pairings are kept.
    func leaveAll() {
        for (handle, open) in rooms {
            open.connection.leave()
            deliver(handle, .closed("left"))
        }
        rooms.removeAll()
    }

    /// The network came back: a connection waiting out its backoff tries now.
    func reconnectNow() {
        for open in rooms.values { open.connection.reconnectNow() }
    }

    // MARK: - Delivery

    private func deliver(_ handle: String, _ arrival: MultisigPeerArrival, dropMessage: Bool = false) {
        guard var open = rooms[handle] else { return }
        if let waiter = open.waiting {
            open.waiting = nil
            rooms[handle] = open
            waiter.resume(returning: arrival)
        } else if case .message(let message) = arrival, !dropMessage {
            open.inbox.append(message)
            rooms[handle] = open
        }
    }

    /// A message as it arrived: JSON when it reads as JSON, else its text.
    nonisolated static func read(_ data: Data) -> Any {
        if let object = try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) {
            return object
        }
        return String(decoding: data, as: UTF8.self)
    }
}

/// One connection's way onto the relay, as the screen says it (the web's
/// `RelayWatch`): a wait it counts down is cut short, a stop undone, by
/// `retry`.
@MainActor
final class MultisigRelayWatch {
    private(set) var status: [String: Any] = ["kind": "connecting"]
    var onChange: (([String: Any]) -> Void)?
    private var wake: (() -> Void)?

    func set(_ status: [String: Any]) {
        self.status = status
        onChange?(status)
    }

    /// Wait `ms`, said as retrying for `why` — less, if Retry is tapped.
    func wait(_ why: String, detail: String, ms: Int) async {
        set(["kind": "retrying", "why": why, "detail": detail, "at": Date().timeIntervalSince1970 * 1000 + Double(ms)])
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            var done = false
            let finish = { [weak self] in
                guard !done else { return }
                done = true
                self?.wake = nil
                continuation.resume()
            }
            wake = finish
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(max(0, ms)) * 1_000_000)
                finish()
            }
        }
    }

    /// Stop: said as failed for `why`, until Retry is tapped.
    func stop(_ why: String, detail: String) async {
        set(["kind": "failed", "why": why, "detail": detail])
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            wake = { [weak self] in
                self?.wake = nil
                self?.set(["kind": "connecting"])
                continuation.resume()
            }
        }
    }

    func retry() { wake?() }
}

/// The PeerConnect relay rooms go through, and the token it asks of each
/// peer (the web's `external/relay.ts`). Vela asks its own site for a token
/// under a random device id and keeps it until it expires; if the site
/// cannot issue one, PeerConnect's public trial gives a short one.
@MainActor
enum MultisigRelay {
    static let defaultRelay = "wss://relay.peerconnect.dev"
    static let tokenEndpoint = "https://getvela.app/api/peerconnect-token"
    static let trialEndpoint = "https://peerconnect.dev/api/trial"

    static let endpointKey = "vela.peerconnectTokenEndpoint"
    static let deviceKey = "vela.peerconnectDevice"
    static let tokenKey = "vela.peerconnectToken.v1"

    /// One of the core's relay rules.
    static func rule(_ name: String, _ args: [String: Any] = [:]) -> Any? {
        MsRules.call("relay.\(name)", args)
    }

    static func needsToken(_ relay: String) -> Bool {
        rule("needsToken", ["relay": relay]) as? Bool ?? false
    }

    private static var defaults: UserDefaults { .standard }

    private static func read(_ key: String) -> Any? {
        guard let raw = defaults.string(forKey: key), let data = raw.data(using: .utf8) else { return nil }
        return try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
    }

    private static func write(_ key: String, _ value: Any?) {
        guard let value, let data = try? JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed]) else {
            defaults.removeObject(forKey: key)
            return
        }
        defaults.set(String(decoding: data, as: UTF8.self), forKey: key)
    }

    static var endpoint: String {
        (defaults.string(forKey: endpointKey)).flatMap { $0.isEmpty ? nil : $0 } ?? tokenEndpoint
    }

    /// This device's id at Vela's site: 32 random hex characters, made once.
    static func deviceId() -> String {
        if let kept = read(deviceKey) as? String, kept.range(of: "^[a-z0-9]{26,64}$", options: .regularExpression) != nil {
            return kept
        }
        let id = (0..<16).map { _ in String(format: "%02x", UInt8.random(in: 0...255)) }.joined()
        write(deviceKey, id)
        return id
    }

    /// One endpoint asked; what it answered is read by the core: a token, or what to say.
    private static func ask(_ url: String, body: [String: Any]?) async -> Any {
        var status: Any = NSNull()
        var text = ""
        if let target = URL(string: url) {
            var request = URLRequest(url: target)
            request.httpMethod = "POST"
            request.timeoutInterval = 15
            if let body, let data = try? JSONSerialization.data(withJSONObject: body) {
                request.httpBody = data
                request.setValue("application/json", forHTTPHeaderField: "content-type")
            }
            if let (data, response) = try? await URLSession.shared.data(for: request) {
                status = (response as? HTTPURLResponse)?.statusCode ?? NSNull()
                text = String(decoding: data, as: UTF8.self)
            }
        }
        let answer = rule("tokenAnswer", ["status": status, "body": text]) as? [String: Any] ?? [:]
        if let ok = answer["ok"] { return ok }
        return answer["err"] as? String ?? "no answer"
    }

    struct NoToken: Error { let detail: String }

    /// A token for the public relay: the one kept if the core finds it still
    /// good, else a new one from Vela's site, else a trial one.
    static func token(trial: Bool) async throws -> (token: String, source: String) {
        if let usable = rule("usable", [
            "kept": read(tokenKey) ?? NSNull(), "nowS": Date().timeIntervalSince1970, "trial": trial,
        ]) as? [String: Any], let token = usable["token"] as? String {
            return (token, usable["source"] as? String ?? "site")
        }
        let site: Any? = trial ? nil : await ask(endpoint, body: ["device": deviceId()])
        var fresh: [String: Any]
        if let site = site as? [String: Any] {
            fresh = site
            fresh["source"] = "site"
        } else {
            let fallback = await ask(trialEndpoint, body: nil)
            guard let kept = fallback as? [String: Any] else {
                let detail = rule("noTokenDetail", [
                    "endpoint": endpoint, "site": site ?? NSNull(), "trial": fallback,
                ]) as? String ?? "no token"
                throw NoToken(detail: detail)
            }
            fresh = kept
            fresh["source"] = "trial"
        }
        write(tokenKey, fresh)
        return (fresh["token"] as? String ?? "", fresh["source"] as? String ?? "site")
    }

    static func forget(refused: String?) {
        if rule("forgets", ["kept": read(tokenKey) ?? NSNull(), "refused": refused ?? NSNull()]) as? Bool == true {
            write(tokenKey, nil)
        }
    }

    /// Connect to `room`, with a token when its relay asks for one; `watch`
    /// says where getting onto the relay stands.
    static func connect(room: PeerRoom, watch: MultisigRelayWatch, socketFactory: (any PeerSocketFactory)?) -> PeerConnection {
        guard needsToken(room.relay) else {
            let plain = PeerConnection(room: room, socketFactory: socketFactory)
            follow(plain, watch: watch, accepted: nil)
            return plain
        }
        var last: String?
        var refusals: Any = NSNull()
        func refused(_ on: String, _ source: String? = nil) -> [String: Any]? {
            let answer = rule("refusals", ["state": refusals, "on": on, "source": source ?? NSNull()]) as? [String: Any]
            refusals = answer?["state"] ?? NSNull()
            return answer?["before"] as? [String: Any]
        }
        let provider: @MainActor () async -> String? = {
            var before = refused("beforeToken")
            while let step = before, step["kind"] as? String != "go" {
                if step["kind"] as? String == "wait" {
                    await watch.wait("refused", detail: "HTTP 4403", ms: (step["ms"] as? NSNumber)?.intValue ?? 2_000)
                } else {
                    await watch.stop("refused", detail: "HTTP 4403")
                    _ = refused("retried")
                }
                before = refused("beforeToken")
            }
            let trial = before?["trial"] as? Bool ?? false
            var tries = 0
            while true {
                watch.set(["kind": "connecting"])
                do {
                    let issued = try await token(trial: trial)
                    last = issued.token
                    _ = refused("issued", issued.source)
                    return issued.token
                } catch {
                    let detail = (error as? NoToken)?.detail ?? "\(error)"
                    let step = rule("noTokenStep", ["tries": tries]) as? [String: Any] ?? ["kind": "stop"]
                    if step["kind"] as? String == "wait" {
                        await watch.wait("noToken", detail: detail, ms: (step["ms"] as? NSNumber)?.intValue ?? 2_000)
                        tries += 1
                    } else {
                        await watch.stop("noToken", detail: detail)
                        tries = 0
                    }
                }
            }
        }
        let connection = PeerConnection(room: room, tokenProvider: provider, socketFactory: socketFactory)
        connection.onUnauthorized = {
            forget(refused: last)
            _ = refused("unauthorized")
        }
        follow(connection, watch: watch, accepted: { _ = refused("accepted") })
        return connection
    }

    /// Where the connection's way onto the relay stands, as the core reads
    /// its state: up once on the relay; offline for the core's while, said
    /// as the relay not answering.
    private static func follow(_ connection: PeerConnection, watch: MultisigRelayWatch, accepted: (() -> Void)?) {
        var timer: Task<Void, Never>?
        let unreachableAfter = ((rule("constants") as? [String: Any])?["unreachableAfterMs"] as? NSNumber)?.intValue ?? 15_000
        func said(_ offlineMs: Int) -> Bool {
            guard let next = rule("onState", [
                "status": watch.status, "state": connection.state.rawValue,
                "offlineMs": offlineMs, "relay": connection.room.relay,
            ]) as? [String: Any] else { return false }
            let isAccepted = next["accepted"] as? Bool ?? false
            if isAccepted { accepted?() }
            if let status = next["status"] as? [String: Any],
               CoreJSON.string(status) != CoreJSON.string(watch.status) {
                watch.set(status)
            }
            return isAccepted
        }
        connection.onState = { [weak connection] state, _ in
            timer?.cancel()
            guard let connection else { return }
            if said(0) || state == .closed { return }
            timer = Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(unreachableAfter) * 1_000_000)
                guard !Task.isCancelled, connection.state != .closed else { return }
                _ = said(unreachableAfter)
            }
        }
    }
}
