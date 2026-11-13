//
//  MultisigPeerNet.swift
//  VelaWallet
//
//  The I/O a PeerConnect room needs, and nothing of the protocol: one
//  WebSocket at a time to the relay (`URLSessionWebSocketTask`) and one POST
//  for the relay's token. The protocol — the Noise handshake, the relay's
//  JOIN, the framing, the ordered, acknowledged, resumable messages — is
//  PeerConnect's own crate inside the core (`PeerConnectPeer`, the Rust the
//  web runs as wasm and the desktop and Android link); `MultisigPeerRooms`
//  drives it with what happens here. Tests hand in their own net (an
//  in-memory relay).
//

import Foundation

/// What happens to a socket. A socket reports `.open` at most once, then
/// messages, then `.closed` exactly once (unless its owner closed it),
/// always asynchronously: never from inside `MultisigPeerNet.open`.
nonisolated enum PeerSocketEvent: Equatable, Sendable {
    case open
    case text(String)
    case binary(Data)
    /// The close code, when the other side sent one.
    case closed(code: UInt16?)
}

/// One WebSocket, as a room uses it.
@MainActor
protocol PeerSocket: AnyObject {
    func send(_ data: Data)
    func send(text: String)
    /// Closes with code 1000 once what was sent has gone; reports nothing more.
    func close()
}

/// The relay and the token server, as a room reaches them.
@MainActor
protocol MultisigPeerNet: AnyObject {
    /// Open a WebSocket to `url`. `events` returns false once nobody is
    /// listening any more, and the socket then closes itself.
    func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket
    /// POST `body` (JSON, or nothing): the status (`nil`: no answer) and the text.
    func post(url: String, body: String?) async -> (status: Int?, body: String)
}

/// The app's: `URLSessionWebSocketTask` and `URLSession.shared`, every
/// callback on the main queue.
@MainActor
final class URLSessionPeerNet: MultisigPeerNet {
    static let shared = URLSessionPeerNet()

    func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket {
        // A relay off this device over plain `ws://` is refused as a node over
        // plain `http://` is (`CoreHTTP.allowsTransport`): a failed connection.
        guard CoreHTTP.allowsTransport(url.absoluteString) else { return RefusedPeerSocket(events: events) }
        return URLSessionPeerSocket(url: url, events: events)
    }

    func post(url: String, body: String?) async -> (status: Int?, body: String) {
        guard let target = URL(string: url) else { return (nil, "not a URL") }
        // The app's one transport rule (`CoreHTTP.allowsTransport`).
        guard CoreHTTP.allowsTransport(url) else { return (nil, "not https: \(url)") }
        var request = URLRequest(url: target)
        request.httpMethod = "POST"
        request.timeoutInterval = 15
        if let body {
            request.httpBody = Data(body.utf8)
            request.setValue("application/json", forHTTPHeaderField: "content-type")
        }
        do {
            let (data, response) = try await URLSession.shared.data(for: request)
            return ((response as? HTTPURLResponse)?.statusCode, String(decoding: data, as: UTF8.self))
        } catch {
            return (nil, error.localizedDescription)
        }
    }
}

/// A connection the transport rule refused: it closes at once, as a relay
/// that never answered does.
@MainActor
final class RefusedPeerSocket: PeerSocket {
    init(events: @escaping @MainActor (PeerSocketEvent) -> Bool) {
        Task { @MainActor in _ = events(.closed(code: nil)) }
    }

    func send(_ data: Data) {}
    func send(text: String) {}
    func close() {}
}

@MainActor
final class URLSessionPeerSocket: PeerSocket {
    private let session: URLSession
    private let task: URLSessionWebSocketTask
    private var events: (@MainActor (PeerSocketEvent) -> Bool)?
    private var opened = false
    /// Sends handed to the task and not yet gone: a close waits for them.
    private var sending = 0
    private var closing = false

    init(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) {
        let delegate = URLSessionPeerSocketDelegate()
        let configuration = URLSessionConfiguration.ephemeral
        configuration.waitsForConnectivity = false
        session = URLSession(configuration: configuration, delegate: delegate, delegateQueue: .main)
        task = session.webSocketTask(with: url)
        self.events = events
        delegate.owner = self
        task.resume()
        receive()
    }

    func send(_ data: Data) { write(.data(data)) }

    func send(text: String) { write(.string(text)) }

    private func write(_ message: URLSessionWebSocketTask.Message) {
        guard events != nil, !closing else { return }
        sending += 1
        // A failed send ends the receive loop too, which reports the close.
        task.send(message) { [weak self] _ in
            let owner = self
            URLSessionPeerSocket.onMain { owner?.sent() }
        }
    }

    private func sent() {
        sending -= 1
        if closing, sending <= 0 { finish() }
    }

    /// What was sent goes first (the room's last words: a close for both
    /// peers), then the close; never longer than a few seconds.
    func close() {
        guard events != nil else { return }
        events = nil
        closing = true
        if sending <= 0 {
            finish()
        } else {
            Task { @MainActor [weak self] in
                try? await Task.sleep(nanoseconds: 3_000_000_000)
                self?.finish()
            }
        }
    }

    private func finish() {
        if task.state == .running || task.state == .suspended {
            task.cancel(with: .normalClosure, reason: nil)
        }
        session.finishTasksAndInvalidate()
    }

    private func receive() {
        task.receive { [weak self] result in
            let owner = self
            URLSessionPeerSocket.onMain { owner?.received(result) }
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

    /// The task ended, one way or another: said once, with the code the relay
    /// closed it with, if any (4409 replaced, 4403 token, …).
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

    /// Runs `body` on the main actor: at once when already on the main thread
    /// (URLSession calls back on its delegate queue, `.main`), else queued.
    nonisolated static func onMain(_ body: @escaping @MainActor () -> Void) {
        if Thread.isMainThread {
            MainActor.assumeIsolated(body)
        } else {
            DispatchQueue.main.async { MainActor.assumeIsolated(body) }
        }
    }
}

nonisolated private final class URLSessionPeerSocketDelegate: NSObject, URLSessionWebSocketDelegate, @unchecked Sendable {
    /// Set once on the main thread, read on the main queue only.
    weak var owner: URLSessionPeerSocket?

    func urlSession(_ session: URLSession, webSocketTask: URLSessionWebSocketTask, didOpenWithProtocol protocol: String?) {
        URLSessionPeerSocket.onMain { [weak self] in self?.owner?.didOpen() }
    }

    func urlSession(
        _ session: URLSession, webSocketTask: URLSessionWebSocketTask,
        didCloseWith closeCode: URLSessionWebSocketTask.CloseCode, reason: Data?
    ) {
        let code = closeCode.rawValue
        URLSessionPeerSocket.onMain { [weak self] in self?.owner?.ended(code: code) }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: (any Error)?) {
        URLSessionPeerSocket.onMain { [weak self] in self?.owner?.ended() }
    }
}
