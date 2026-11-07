//
//  MultisigPeerRooms.swift
//  VelaWallet
//
//  The PeerConnect transport behind the multisig machines' `peer_*`
//  operations (native multisig M4) — the web's `shell/peer.ts`, the
//  desktop's `multisig/peer.rs`, Android's `MultisigPeerConnect.kt`: rooms
//  (Noise end to end; the relay reads nothing) and the public relay's token.
//
//  The protocol is PeerConnect's own sans-I/O peer in the core
//  (`PeerConnectPeer` over uniffi, the vendored crate the web runs as wasm
//  and the desktop links): rooms, invites and records, the handshake, the
//  framing, the resume. This file supplies the socket (`MultisigPeerNet`),
//  the clock and the timers. What is sent and what an arrival means are the
//  core's (`multisig_outside`); every relay judgement too (`relay.*` through
//  the link door: `needsToken`, `usable`, `tokenAnswer`, `noTokenDetail`,
//  `refusals`, `noTokenStep`, `onState`, `forgets`, `constants`).
//
//  A room is known to the core by a handle handed out here (`room-N`); what
//  the connection says of itself goes back to the machine that opened it as
//  its own events: `peer_state` (offline / waiting / connected / closed),
//  `paired` (the record that reopens it now) and `relay` (where getting onto
//  the relay stands).
//

import Foundation
import VelaCore

@MainActor
final class MultisigPeerRooms: MultisigPeerTransport {
    /// What a connection tells the machine that opened it (an event).
    var tell: (([String: Any]) -> Void)?

    private struct Open {
        let room: MultisigPeerRoom
        var inbox: [Any] = []
        var waiting: CheckedContinuation<MultisigPeerArrival, Never>?
        var closed: String?
    }

    private var rooms: [String: Open] = [:]
    private var next = 0
    private let defaultRelay: String
    private let net: any MultisigPeerNet
    private let tokens: MultisigRelayTokens

    init(defaultRelay: String, net: (any MultisigPeerNet)? = nil, defaults: UserDefaults = .standard) {
        self.defaultRelay = defaultRelay
        let net = net ?? URLSessionPeerNet.shared
        self.net = net
        tokens = MultisigRelayTokens(net: net, defaults: defaults)
    }

    /// The app's rooms: the relay a profile names, else PeerConnect's own.
    static func live(relay: String?) -> MultisigPeerRooms {
        MultisigPeerRooms(defaultRelay: relay.flatMap { $0.isEmpty ? nil : $0 } ?? MultisigRelayTokens.defaultRelay)
    }

    // MARK: - MultisigPeerTransport

    func open(relay: String?, invite: String?, pairing: String?) async throws
        -> (room: String, invite: String?, pairing: String?) {
        let made: PeerRoomRecord
        if let pairing {
            made = PeerRoomRecord(record: pairing, invite: nil, relay: try peerconnectRoomRelay(record: pairing))
        } else if let invite {
            made = try peerconnectJoinRoom(invite: invite)
        } else {
            made = try peerconnectCreateRoom(relay: relay ?? defaultRelay)
        }
        next += 1
        let handle = "room-\(next)"
        let room = try MultisigPeerRoom(
            record: made.record, relay: made.relay,
            needsToken: MultisigRelayTokens.needsToken(made.relay), net: net, tokens: tokens
        )
        rooms[handle] = Open(room: room)
        room.onMessage = { [weak self] bytes in
            self?.deliver(handle, .message(Self.read(bytes)))
        }
        room.onPaired = { [weak self] record in
            self?.tell?(["type": "paired", "room": handle, "pairing": record])
        }
        room.onState = { [weak self] state, reason in
            guard let self else { return }
            self.tell?(["type": "peer_state", "room": handle, "state": state])
            if state == "closed" {
                let why = reason ?? "closed"
                self.rooms[handle]?.closed = why
                self.deliver(handle, .closed(why))
            }
        }
        room.watch.onChange = { [weak self] status in
            self?.tell?(["type": "relay", "room": handle, "status": status])
        }
        room.start()
        let fresh = pairing == nil && invite == nil
        return (handle, fresh ? made.invite : nil, made.record)
    }

    func send(room handle: String, message: Any) async {
        guard let open = rooms[handle], open.closed == nil else { return }
        let text: String
        if let string = message as? String { text = string }
        else if let data = try? JSONSerialization.data(withJSONObject: message, options: [.fragmentsAllowed]) {
            text = String(decoding: data, as: UTF8.self)
        } else { return }
        open.room.send(Data(text.utf8))
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

    /// `walletpair_cancel` is the core's message, sent before this (`cancel`).
    func close(room handle: String, cancel: Bool, end: Bool) async {
        guard let open = rooms[handle] else { return }
        open.room.stop(end: end)
        rooms[handle]?.closed = end ? "closed" : "left"
        deliver(handle, .closed(end ? "closed" : "left"))
        rooms.removeValue(forKey: handle)
    }

    // MARK: - The person's controls

    /// Try the relay again now: a wait cut short, a stop undone.
    func retryAll() {
        for open in rooms.values { open.room.retry() }
    }

    /// Leave every room (the screen went away): pairings are kept.
    func leaveAll() {
        for (handle, open) in rooms {
            open.room.stop(end: false)
            deliver(handle, .closed("left"))
        }
        rooms.removeAll()
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

// MARK: - One room

/// One room: the core's peer, its socket, its timers, its way onto the relay.
/// Everything the peer asks is done in order (`pump`); every socket event,
/// timeout and token goes back in with the time it happened.
@MainActor
final class MultisigPeerRoom {
    let relay: String
    let needsToken: Bool
    let watch = MultisigRelayWatch()

    var onState: ((String, String?) -> Void)?
    var onMessage: ((Data) -> Void)?
    var onPaired: ((String) -> Void)?

    private let peer: PeerConnectPeer
    private let net: any MultisigPeerNet
    private let tokens: MultisigRelayTokens
    private let epoch = ContinuousClock.now
    /// The clock moved on by a Retry (the JS client's `reconnectNow`).
    private var ahead: UInt64 = 0
    private var socket: (any PeerSocket)?
    /// Identifies the current socket, so a stale one's news is ignored.
    private var attempt = 0
    private var connecting = false
    private(set) var state = "offline"
    private var timer: Task<Void, Never>?
    private var unreachable: Task<Void, Never>?
    private var fetching = false
    private var over = false

    init(record: String, relay: String, needsToken: Bool, net: any MultisigPeerNet, tokens: MultisigRelayTokens) throws {
        self.relay = relay
        self.needsToken = needsToken
        self.net = net
        self.tokens = tokens
        peer = try PeerConnectPeer(record: record, awaitingToken: needsToken, nowMs: 0)
    }

    /// Monotonic ms since this room began (the peer's clock).
    private func now() -> UInt64 {
        let elapsed = epoch.duration(to: .now).components
        // 1 ms = 10^15 attoseconds.
        return UInt64(max(0, elapsed.seconds)) * 1000 + UInt64(max(0, elapsed.attoseconds) / 1_000_000_000_000_000) + ahead
    }

    /// Begin: whatever the peer asks first (the relay), and the token when its relay wants one.
    func start() {
        pump()
        if needsToken { fetchToken() }
    }

    func send(_ bytes: Data) {
        guard !over else { return }
        _ = try? peer.send(bytes: bytes)
        pump()
    }

    /// The person's Retry: a wait cut short, a stop undone, the relay tried now.
    func retry() {
        watch.wake()
        reconnectNow()
    }

    /// Leave (the room is kept) or end it for both peers; nothing more is said.
    func stop(end: Bool) {
        guard !over else { return }
        if end { peer.end() } else { peer.leave() }
        pump()
        over = true
        watch.end()
        socket?.close()
        socket = nil
        timer?.cancel()
        timer = nil
        unreachable?.cancel()
        unreachable = nil
    }

    // MARK: Driving

    /// Everything the peer asks, done in order.
    private func pump() {
        while let output = peer.pollOutput() {
            switch output {
            case .connect(let url):
                connect(url)
            case .send(let bytes):
                socket?.send(bytes)
            case .ping:
                socket?.send(text: "ping")
            case .disconnect:
                socket?.close()
                socket = nil
                connecting = false
                attempt += 1
            case .state(let next, let reason):
                state = next
                onState?(next, reason)
                follow()
            case .message(let bytes):
                onMessage?(bytes)
            case .delivered, .replaced:
                break
            case .paired(let record):
                onPaired?(record)
            case .unauthorized:
                guard needsToken else { break }
                // Counted before the next token is asked for: that one must not be the one just refused.
                tokens.forget(refused: watch.unauthorized())
                fetchToken()
            }
        }
        schedule()
    }

    private func schedule() {
        timer?.cancel()
        timer = nil
        guard !over, let at = peer.pollTimeout() else { return }
        let now = self.now()
        let delay = at > now ? at - now : 0
        timer = Task { @MainActor [weak self] in
            if delay > 0 { try? await Task.sleep(nanoseconds: delay * 1_000_000) }
            guard !Task.isCancelled, let self, !self.over else { return }
            self.peer.handleTimeout(nowMs: self.now())
            self.pump()
        }
    }

    private func connect(_ url: String) {
        attempt += 1
        let mine = attempt
        connecting = true
        guard let target = URL(string: url) else {
            // Cannot happen with a relay the core accepted: a failed connection.
            Task { @MainActor [weak self] in self?.event(.closed(code: nil), attempt: mine) }
            return
        }
        socket = net.open(url: target) { [weak self] event in
            guard let self, !self.over else { return false }
            self.event(event, attempt: mine)
            return true
        }
    }

    private func event(_ event: PeerSocketEvent, attempt mine: Int) {
        guard mine == attempt, !over else { return }
        let now = self.now()
        switch event {
        case .open:
            guard connecting else { return }
            connecting = false
            peer.handleOpen(nowMs: now)
        case .text(let text):
            peer.handleText(nowMs: now, text: text)
        case .binary(let frame):
            peer.handleBinary(nowMs: now, frame: frame)
        case .closed(let code):
            guard connecting || socket != nil else { return }
            connecting = false
            socket = nil
            peer.handleClose(nowMs: now, code: code)
        }
        pump()
    }

    /// Offline and waiting to try the relay again: tried now, the clock moved on to that moment.
    private func reconnectNow() {
        guard !over, state == "offline", socket == nil, !connecting, let at = peer.pollTimeout() else { return }
        let now = self.now()
        if at > now { ahead += at - now }
        peer.handleTimeout(nowMs: self.now())
        pump()
    }

    /// Where the way onto the relay stands, as the core reads the state;
    /// offline for the core's while, said as the relay not answering.
    private func follow() {
        unreachable?.cancel()
        unreachable = nil
        let accepted = watch.onState(state, offlineMs: 0, relay: relay, token: needsToken)
        guard !accepted, state != "closed", !over else { return }
        let after = MultisigRelayTokens.unreachableAfterMs
        unreachable = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(after) * 1_000_000)
            guard !Task.isCancelled, let self, !self.over else { return }
            _ = self.watch.onState(self.state, offlineMs: after, relay: self.relay, token: self.needsToken)
        }
    }

    // MARK: The token

    /// A token for the peer, on a task of its own (one at a time).
    private func fetchToken() {
        guard !fetching, !over else { return }
        fetching = true
        Task { @MainActor [weak self] in
            guard let self else { return }
            let token = await self.tokenFor()
            self.fetching = false
            guard let token, !self.over else { return }
            self.peer.setToken(nowMs: self.now(), token: token)
            self.pump()
        }
    }

    /// The token, as the core schedules it: a refusal waited out (or stopped,
    /// until Retry), then the kept one or a new one, with the waits and the
    /// stop when none comes. `nil`: the room is gone.
    private func tokenFor() async -> String? {
        var before = watch.refusals("beforeToken")
        while let step = before, step["kind"] as? String != "go" {
            if step["kind"] as? String == "wait" {
                guard await watch.waitOut("refused", detail: "HTTP 4403", ms: Self.int(step["ms"]) ?? 2_000) else { return nil }
            } else {
                guard await watch.stopUntilRetry("refused", detail: "HTTP 4403") else { return nil }
                _ = watch.refusals("retried")
            }
            before = watch.refusals("beforeToken")
        }
        // The last chance before stopping is the trial's, once.
        let trial = before?["trial"] as? Bool ?? false
        var tries = 0
        while !over {
            watch.set(["kind": "connecting"])
            switch await tokens.token(trial: trial) {
            case .token(let token, let source):
                watch.issued(token, source: source)
                return token
            case .nothing(let detail):
                let step = MsRules.call("relay.noTokenStep", ["tries": tries]) as? [String: Any] ?? ["kind": "stop"]
                if step["kind"] as? String == "wait" {
                    guard await watch.waitOut("noToken", detail: detail, ms: Self.int(step["ms"]) ?? 2_000) else { return nil }
                    tries += 1
                } else {
                    guard await watch.stopUntilRetry("noToken", detail: detail) else { return nil }
                    tries = 0
                }
            }
        }
        return nil
    }

    nonisolated static func int(_ value: Any?) -> Int? {
        (value as? NSNumber)?.intValue
    }
}

// MARK: - The relay's word

/// One room's way onto the relay, as the screen says it (the web's
/// `RelayWatch`), with the refusals counted as the core counts them; a wait
/// it counts down is cut short, a stop undone, by `wake`.
@MainActor
final class MultisigRelayWatch {
    private(set) var status: [String: Any] = ["kind": "connecting"]
    var onChange: (([String: Any]) -> Void)?
    private var refusalState: Any = NSNull()
    /// The token last handed to the peer.
    private var last: String?
    private var wakeUp: (() -> Void)?
    private(set) var over = false

    func set(_ next: [String: Any]) {
        if (next as NSDictionary).isEqual(to: status) { return }
        status = next
        onChange?(next)
    }

    /// The core's refusal count, moved on by `on`: what to do before a token, when asked.
    func refusals(_ on: String, source: String? = nil) -> [String: Any]? {
        let answer = MsRules.call("relay.refusals", [
            "state": refusalState, "on": on, "source": source ?? NSNull(),
        ]) as? [String: Any]
        if let state = answer?["state"] { refusalState = state }
        return answer?["before"] as? [String: Any]
    }

    /// The relay refused the token in use: counted; that token handed back.
    func unauthorized() -> String? {
        _ = refusals("unauthorized")
        return last
    }

    func issued(_ token: String, source: String) {
        last = token
        _ = refusals("issued", source: source)
    }

    /// The core's reading of the connection's state: a new status said; on
    /// the relay, the refusals forgotten. Whether it is on the relay.
    func onState(_ state: String, offlineMs: Int, relay: String, token: Bool) -> Bool {
        guard let answer = MsRules.call("relay.onState", [
            "status": status, "state": state, "offlineMs": offlineMs, "relay": relay,
        ]) as? [String: Any] else { return false }
        let accepted = answer["accepted"] as? Bool ?? false
        if accepted, token { _ = refusals("accepted") }
        if let next = answer["status"] as? [String: Any] { set(next) }
        return accepted
    }

    /// Wait `ms`, said as retrying for `why` — less, if Retry is tapped.
    /// `false`: the room is gone.
    func waitOut(_ why: String, detail: String, ms: Int) async -> Bool {
        guard !over else { return false }
        set(["kind": "retrying", "why": why, "detail": detail, "at": Date().timeIntervalSince1970 * 1000 + Double(ms)])
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            var done = false
            let finish = { [weak self] in
                guard !done else { return }
                done = true
                self?.wakeUp = nil
                continuation.resume()
            }
            wakeUp = finish
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(max(0, ms)) * 1_000_000)
                finish()
            }
        }
        return !over
    }

    /// Stopped, said as failed for `why`, until Retry. `false`: the room is gone.
    func stopUntilRetry(_ why: String, detail: String) async -> Bool {
        guard !over else { return false }
        set(["kind": "failed", "why": why, "detail": detail])
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            wakeUp = { [weak self] in
                self?.wakeUp = nil
                continuation.resume()
            }
        }
        return !over
    }

    func wake() { wakeUp?() }

    /// The room is gone: any wait ends, nothing more is said.
    func end() {
        over = true
        onChange = nil
        wakeUp?()
    }
}

// MARK: - The public relay's token

/// The token PeerConnect's public relay asks of each peer (the web's
/// `external/relay.ts`): Vela asks its own site under a random device ID and
/// keeps it until the core finds it spent; if the site cannot issue one,
/// PeerConnect's public trial gives a short one. Every reading is the core's.
@MainActor
final class MultisigRelayTokens {
    static let endpointKey = "vela.peerconnectTokenEndpoint"
    static let deviceKey = "vela.peerconnectDevice"
    static let tokenKey = "vela.peerconnectToken.v1"

    private static var constants: [String: Any] {
        MsRules.call("relay.constants") as? [String: Any] ?? [:]
    }
    static var defaultRelay: String { constants["defaultRelay"] as? String ?? CoreServiceDefaults.value.peerconnectRelayUrl }
    static var tokenEndpoint: String { constants["tokenEndpoint"] as? String ?? CoreServiceDefaults.value.peerconnectTokenUrl }
    static var trialEndpoint: String { constants["trialEndpoint"] as? String ?? CoreServiceDefaults.value.peerconnectTrialUrl }
    static var unreachableAfterMs: Int { MultisigPeerRoom.int(constants["unreachableAfterMs"]) ?? 10_000 }

    static func needsToken(_ relay: String) -> Bool {
        MsRules.call("relay.needsToken", ["relay": relay]) as? Bool ?? false
    }

    nonisolated enum Got: Sendable {
        case token(String, source: String)
        /// What each place answered, in the core's words.
        case nothing(String)
    }

    private let net: any MultisigPeerNet
    private let defaults: UserDefaults
    /// Two rooms opening at once share one request.
    private var asking: Task<Got, Never>?

    init(net: any MultisigPeerNet, defaults: UserDefaults) {
        self.net = net
        self.defaults = defaults
    }

    /// A token for the public relay: the one kept, if the core finds it still
    /// good, else a new one from Vela's site under this device's ID, else a
    /// trial one (`trial`: the trial's, unless the kept one is the trial's).
    func token(trial: Bool) async -> Got {
        if let usable = usable(trial: trial) { return usable }
        if let asking { return await asking.value }
        let task = Task { @MainActor in await self.fresh(trial: trial) }
        asking = task
        let got = await task.value
        asking = nil
        return got
    }

    private func fresh(trial: Bool) async -> Got {
        let endpoint = defaults.string(forKey: Self.endpointKey).flatMap { $0.isEmpty ? nil : $0 } ?? Self.tokenEndpoint
        let site = trial ? nil : await ask(endpoint, body: CoreJSON.string(["device": deviceId()]))
        var kept: [String: Any]
        if let ok = site?["ok"] as? [String: Any] {
            kept = ok
            kept["source"] = "site"
        } else {
            let tried = await ask(Self.trialEndpoint, body: nil)
            guard let ok = tried["ok"] as? [String: Any] else {
                let detail = MsRules.call("relay.noTokenDetail", [
                    "endpoint": endpoint, "site": site?["err"] ?? NSNull(), "trial": tried["err"] as? String ?? "no answer",
                ]) as? String
                return .nothing(detail ?? "no token")
            }
            kept = ok
            kept["source"] = "trial"
        }
        defaults.set(CoreJSON.string(kept), forKey: Self.tokenKey)
        guard let token = kept["token"] as? String else { return .nothing("no token") }
        return .token(token, source: kept["source"] as? String ?? "site")
    }

    private func usable(trial: Bool) -> Got? {
        let answer = MsRules.call("relay.usable", [
            "kept": keptToken() ?? NSNull(), "nowS": Date().timeIntervalSince1970, "trial": trial,
        ]) as? [String: Any]
        guard let token = answer?["token"] as? String else { return nil }
        return .token(token, source: answer?["source"] as? String ?? "site")
    }

    /// One endpoint asked; what it answered read by the core (`{ok}` or `{err}`).
    private func ask(_ url: String, body: String?) async -> [String: Any] {
        let (status, text) = await net.post(url: url, body: body)
        return MsRules.call("relay.tokenAnswer", ["status": status ?? NSNull(), "body": text]) as? [String: Any]
            ?? ["err": "no answer"]
    }

    private func keptToken() -> Any? {
        guard let raw = defaults.string(forKey: Self.tokenKey) else { return nil }
        return try? JSONSerialization.jsonObject(with: Data(raw.utf8), options: [.fragmentsAllowed])
    }

    /// The relay refused `refused`: the kept token goes when it is that one (the core's `forgets`).
    func forget(refused: String?) {
        let forgets = MsRules.call("relay.forgets", [
            "kept": keptToken() ?? NSNull(), "refused": refused ?? NSNull(),
        ]) as? Bool ?? false
        if forgets { defaults.removeObject(forKey: Self.tokenKey) }
    }

    /// This device's ID at Vela's site: 32 random hex characters, made once.
    func deviceId() -> String {
        // Kept plain, or as the JSON string earlier builds wrote.
        let raw = defaults.string(forKey: Self.deviceKey)
        let kept = raw.flatMap { try? JSONSerialization.jsonObject(with: Data($0.utf8), options: [.fragmentsAllowed]) as? String } ?? raw
        if let kept, kept.range(of: "^[a-z0-9]{26,64}$", options: .regularExpression) != nil {
            return kept
        }
        var rng = SystemRandomNumberGenerator()
        let id = (0..<16).map { _ in String(format: "%02x", UInt8.random(in: 0...255, using: &rng)) }.joined()
        defaults.set(id, forKey: Self.deviceKey)
        return id
    }
}
