// PeerConnect v1 for Swift: the connection, driving the sans-I/O machine
// (PeerConnectProtocol.swift) over one WebSocket at a time, with its timers.
// It mirrors the JavaScript client the web wallet vendors
// (app-web/vela-wallet/src/lib/vendor/peerconnect/index.js): the same
// states, reasons and events, the token provider asked before the first
// connection and again whenever the relay refuses the token, and Vela's
// local `reconnectNow()`. Ported from PeerConnect (MIT OR Apache-2.0, used
// here under MIT); see PeerConnectRoom.swift for the credit and license.

import CryptoKit
import Foundation

// MARK: - Sockets

/// What happens to a socket. A socket reports `.open` at most once, then
/// messages, then `.closed` exactly once (unless it was closed by its owner),
/// always asynchronously: never from inside `PeerSocketFactory.open`.
nonisolated enum PeerSocketEvent: Equatable, Sendable {
    case open
    case text(String)
    case binary(Data)
    /// The close code, if the other side sent one.
    case closed(code: UInt16?)
}

/// One WebSocket, as a connection uses it.
@MainActor
protocol PeerSocket: AnyObject {
    func send(_ data: Data)
    func send(text: String)
    /// Closes with code 1000 after what was sent; reports nothing more.
    func close()
}

/// Opens sockets; injectable so tests can use an in-memory relay.
@MainActor
protocol PeerSocketFactory: AnyObject {
    /// Opens a WebSocket to `url`. `events` returns false once nobody is
    /// listening any more (the connection was released without `leave()`),
    /// and the socket then closes itself.
    func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket
}

/// The production socket: `URLSessionWebSocketTask`, every callback on the
/// main queue.
@MainActor
final class PeerURLSessionSocketFactory: PeerSocketFactory {
    init() {}

    func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket {
        PeerURLSessionSocket(url: url, events: events)
    }
}

@MainActor
final class PeerURLSessionSocket: PeerSocket {
    private let session: URLSession
    private let task: URLSessionWebSocketTask
    private var events: (@MainActor (PeerSocketEvent) -> Bool)?
    private var opened = false

    init(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) {
        let delegate = PeerURLSessionSocketDelegate()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = false
        session = URLSession(configuration: configuration, delegate: delegate, delegateQueue: .main)
        task = session.webSocketTask(with: url)
        self.events = events
        delegate.owner = self
        task.resume()
        receive()
    }

    func send(_ data: Data) {
        guard events != nil else { return }
        task.send(.data(data)) { _ in } // a failed send ends the receive loop too
    }

    func send(text: String) {
        guard events != nil else { return }
        task.send(.string(text)) { _ in }
    }

    func close() {
        guard events != nil else { return }
        events = nil
        task.cancel(with: .normalClosure, reason: nil)
        session.finishTasksAndInvalidate()
    }

    private func receive() {
        task.receive { [weak self] result in
            let owner = self
            PeerURLSessionSocket.onMain { owner?.received(result) }
        }
    }

    private func received(_ result: Result<URLSessionWebSocketTask.Message, any Error>) {
        guard events != nil else { return }
        switch result {
        case .success(.data(let data)):
            report(.binary(data))
            receive()
        case .success(.string(let text)):
            report(.text(text))
            receive()
        case .success:
            receive()
        case .failure:
            ended()
        }
    }

    fileprivate func didOpen() {
        guard !opened else { return }
        opened = true
        report(.open)
    }

    /// The task ended, one way or another: report it once, with the code the
    /// relay closed it with, if any (4409 replaced, 4403 token, …).
    fileprivate func ended(code: Int? = nil) {
        guard events != nil else { return }
        let raw = code ?? task.closeCode.rawValue
        let code = raw > 0 && raw <= Int(UInt16.max) ? UInt16(raw) : nil
        report(.closed(code: code))
        events = nil
        session.finishTasksAndInvalidate()
    }

    private func report(_ event: PeerSocketEvent) {
        guard let events else { return }
        if !events(event) { close() }
    }

    /// Runs `body` on the main actor: at once if already on the main thread
    /// (URLSession calls back on its delegate queue, `.main`), else queued.
    nonisolated static func onMain(_ body: @escaping @MainActor () -> Void) {
        if Thread.isMainThread {
            MainActor.assumeIsolated(body)
        } else {
            DispatchQueue.main.async { MainActor.assumeIsolated(body) }
        }
    }
}

nonisolated private final class PeerURLSessionSocketDelegate: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    /// Set once on the main thread, read on the main queue only.
    weak var owner: PeerURLSessionSocket?

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        PeerURLSessionSocket.onMain { [weak self] in self?.owner?.didOpen() }
    }

    func urlSession(
        _ session: URLSession, webSocketTask: URLSessionWebSocketTask,
        didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?
    ) {
        let code = closeCode.rawValue
        PeerURLSessionSocket.onMain { [weak self] in self?.owner?.ended(code: code) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: (any Error)?) {
        PeerURLSessionSocket.onMain { [weak self] in self?.owner?.ended() }
    }
}

// MARK: - Connection

/// What a connection reports, as an `AsyncStream` element.
nonisolated enum PeerConnectionEvent: Equatable, Sendable {
    /// `state` (and `reason`) changed.
    case state(PeerConnectState, PeerCloseReason?)
    /// A message from the peer, authenticated, decrypted and in order.
    case message(Data)
    /// The peer has received every message whose id is below this one.
    case delivered(UInt64)
    /// Pairing is complete and the room has moved to its pair ID: save `room.record` again.
    case paired
    /// Another connection took this peer's seat, most likely the same room
    /// opened elsewhere. The connection retries in a minute; `leave()` lets
    /// the other one keep it.
    case replaced
    /// The relay requires a token and refused this connection's. The
    /// connection stays offline until it gets another (token provider, or `setToken`).
    case unauthorized
}

/// A live room connection. It reconnects on its own until left or closed.
///
/// Like the JavaScript client, it starts on the next turn of the main actor,
/// so the caller can set its callbacks first.
@MainActor
final class PeerConnection {
    /// `offline`: not on the relay (connecting or waiting to retry);
    /// `waiting`: on the relay, peer not (yet) connected;
    /// `connected`: end-to-end session established;
    /// `closed`: stopped for good, see `reason`.
    private(set) var state: PeerConnectState = .offline
    private(set) var reason: PeerCloseReason?

    /// The room as it stands now; save `room.record` after `onPaired` and after `close()`.
    var room: PeerRoom { machine.room }
    /// Bytes of messages waiting to be acknowledged by the peer.
    var queued: Int { machine.queued }

    var onState: ((PeerConnectState, PeerCloseReason?) -> Void)?
    var onMessage: ((Data) -> Void)?
    var onDelivered: ((UInt64) -> Void)?
    var onPaired: (() -> Void)?
    var onReplaced: (() -> Void)?
    var onUnauthorized: (() -> Void)?

    /// For a relay that requires one: called before the first connection and
    /// again whenever the relay refuses the token. A nil or empty answer is
    /// asked again in 30 s.
    let tokenProvider: (@MainActor () async -> String?)?

    private let machine: PeerConnectMachine
    private let factory: any PeerSocketFactory
    private let clock: () -> UInt64
    /// Added to `clock`: `reconnectNow()` moves time on to the next attempt.
    private var skipped: UInt64 = 0
    private var socket: (any PeerSocket)?
    /// Identifies the current socket, so a stale one's events are ignored.
    private var generation = 0
    private var timer: Task<Void, Never>?
    private var tokenRetry: Task<Void, Never>?
    private var fetching = false
    private var subscribers: [UUID: AsyncStream<PeerConnectionEvent>.Continuation] = [:]

    /// Connects to a room. `token`: for a relay that requires one; or pass
    /// `tokenProvider` to fetch it (it wins over `token`).
    init(
        room: PeerRoom,
        token: String? = nil,
        tokenProvider: (@MainActor () async -> String?)? = nil,
        socketFactory: (any PeerSocketFactory)? = nil,
        clock: (() -> UInt64)? = nil
    ) {
        let epoch = ContinuousClock.now
        self.clock = clock ?? {
            let elapsed = epoch.duration(to: .now).components
            // 1 ms = 10^15 attoseconds.
            return UInt64(max(0, elapsed.seconds)) * 1000 + UInt64(max(0, elapsed.attoseconds) / 1_000_000_000_000_000)
        }
        factory = socketFactory ?? PeerURLSessionSocketFactory()
        self.tokenProvider = tokenProvider
        let now = self.clock()
        if tokenProvider != nil {
            // Offline until the first token arrives: the relay would refuse us.
            machine = PeerConnectMachine.awaitingToken(room: room, now: now)
        } else {
            machine = PeerConnectMachine(room: room, token: token, now: now)
        }
        // Let the caller attach callbacks before anything happens.
        Task { @MainActor [weak self] in
            guard let self else { return }
            if self.tokenProvider != nil { self.fetchToken() } else { self.pump() }
        }
    }

    /// Connects to a room saved with `room.record`.
    convenience init(
        record: String,
        token: String? = nil,
        tokenProvider: (@MainActor () async -> String?)? = nil,
        socketFactory: (any PeerSocketFactory)? = nil
    ) throws {
        self.init(room: try PeerRoom.parse(record), token: token, tokenProvider: tokenProvider,
                  socketFactory: socketFactory)
    }

    /// Queues a message (UTF-8), up to 16 MiB, and returns its id. It is
    /// delivered as soon as the peer is reachable. Throws `PeerSendError`
    /// when 64 MiB already await acknowledgement, or once the room is closed
    /// or closing.
    @discardableResult
    func send(_ text: String) throws -> UInt64 {
        try send(Data(text.utf8))
    }

    @discardableResult
    func send(_ data: Data) throws -> UInt64 {
        let id = try machine.send([UInt8](data))
        pump()
        return id
    }

    /// Disconnects, keeping the room for later. Unacknowledged messages are dropped.
    func leave() {
        machine.leave()
        pump()
    }

    /// Ends the room for both peers: the connection carries on until the peer
    /// has been told, then closes with reason `.closed`. Save `room` (it is
    /// now closing) so that connecting with it later finishes the job; delete
    /// it once closed.
    func close() {
        machine.close()
        pump()
    }

    /// Vela: offline and waiting out its backoff (the network came back) —
    /// try the relay now instead of at the end of the wait (up to 30 s). The
    /// clock moves on to that moment, as if the wait had passed. Does nothing
    /// while connecting, connected or closed.
    func reconnectNow() {
        guard state == .offline, socket == nil, let at = machine.pollTimeout() else { return }
        let now = self.now()
        if at > now { skipped += at - now }
        machine.handleTimeout(now: self.now())
        pump()
    }

    /// Replaces the relay token: used from the next connection on, at once
    /// after `unauthorized`.
    func setToken(_ token: String?) {
        machine.setToken(now: now(), token)
        pump()
    }

    /// Every event from now on, until the connection closes or the stream is dropped.
    func events() -> AsyncStream<PeerConnectionEvent> {
        let (stream, continuation) = AsyncStream.makeStream(of: PeerConnectionEvent.self)
        if state == .closed {
            continuation.finish()
            return stream
        }
        let id = UUID()
        subscribers[id] = continuation
        continuation.onTermination = { [weak self] _ in
            let owner = self
            Task { @MainActor in owner?.subscribers[id] = nil }
        }
        return stream
    }

    // MARK: Driving

    private func now() -> UInt64 {
        clock() + skipped
    }

    private func pump() {
        while let output = machine.pollOutput() {
            switch output {
            case .connect(let url):
                open(url)
            case .send(let data):
                socket?.send(data)
            case .ping:
                socket?.send(text: PeerWire.ping)
            case .disconnect:
                disconnect()
            case .state(let state, let reason):
                self.state = state
                self.reason = reason
                onState?(state, reason)
                emit(.state(state, reason))
            case .message(let data):
                onMessage?(data)
                emit(.message(data))
            case .delivered(let id):
                onDelivered?(id)
                emit(.delivered(id))
            case .paired:
                onPaired?()
                emit(.paired)
            case .replaced:
                onReplaced?()
                emit(.replaced)
            case .unauthorized:
                onUnauthorized?()
                emit(.unauthorized)
                if tokenProvider != nil { fetchToken() }
            }
        }
        schedule()
        if state == .closed { stop() }
    }

    private func schedule() {
        timer?.cancel()
        timer = nil
        guard let at = machine.pollTimeout() else { return }
        let now = self.now()
        let delay = at > now ? at - now : 0
        timer = Task { @MainActor [weak self] in
            if delay > 0 { try? await Task.sleep(nanoseconds: delay * 1_000_000) }
            guard !Task.isCancelled, let self else { return }
            self.machine.handleTimeout(now: self.now())
            self.pump()
        }
    }

    private func open(_ url: String) {
        generation += 1
        let mine = generation
        guard let target = URL(string: url) else {
            // Cannot happen with a validated relay URL; treat it as a failed connection.
            Task { @MainActor [weak self] in
                guard let self, self.generation == mine else { return }
                self.machine.handleClose(now: self.now(), code: nil)
                self.pump()
            }
            return
        }
        socket = factory.open(url: target) { [weak self] event in
            guard let self else { return false }
            guard self.generation == mine, self.socket != nil else { return true }
            let now = self.now()
            switch event {
            case .open: self.machine.handleOpen(now: now)
            case .text(let text): self.machine.handleText(now: now, text)
            case .binary(let data): self.machine.handleBinary(now: now, [UInt8](data))
            case .closed(let code):
                self.socket = nil
                self.machine.handleClose(now: now, code: code)
            }
            self.pump()
            return true
        }
    }

    private func disconnect() {
        let socket = self.socket
        self.socket = nil
        generation += 1
        socket?.close()
    }

    private func fetchToken() {
        guard !fetching, state != .closed, let provider = tokenProvider else { return }
        fetching = true
        tokenRetry?.cancel()
        tokenRetry = nil
        Task { @MainActor [weak self] in
            let token = await provider()
            guard let self else { return }
            self.fetching = false
            guard self.state != .closed else { return }
            if let token, !token.isEmpty {
                self.setToken(token)
            } else {
                self.tokenRetry = Task { @MainActor [weak self] in
                    try? await Task.sleep(nanoseconds: 30_000_000_000)
                    guard !Task.isCancelled, let self, self.state != .closed else { return }
                    self.fetchToken()
                }
            }
        }
    }

    private func emit(_ event: PeerConnectionEvent) {
        for continuation in subscribers.values { continuation.yield(event) }
    }

    /// Closed for good: nothing more will happen.
    private func stop() {
        timer?.cancel()
        timer = nil
        tokenRetry?.cancel()
        tokenRetry = nil
        for continuation in subscribers.values { continuation.finish() }
        subscribers = [:]
    }
}
