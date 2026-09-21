//
//  ClearSignerChannel.swift
//  VelaWallet
//
//  The phone's end of the Clear Signer's loopback WebSocket (spec 071,
//  contract §2): a TCP listener on 127.0.0.1, and nothing else.
//
//  Every byte a socket delivers goes to the core's `ClearSignerConnection`,
//  which speaks RFC 6455, admits only the signer page's `Origin`, checks the
//  one-time token, hands the intent over once and verifies the answer against
//  this request's digest and this wallet's keys. This file writes what it is
//  told to write, closes what it is told to close and stops at the first
//  outcome. The security boundary has one implementation, tested on both
//  sides (research R7), and none of it is here.
//
//  ## The loopback only
//
//  The listener is REQUIRED to bind 127.0.0.1. A port on the Wi-Fi interface
//  would let anybody on the network knock, and the page connects to
//  `ws://127.0.0.1:<port>` — never `localhost` — so there is no IPv6 twin to
//  open either.
//
//  ## One ceremony, then nothing
//
//  A fresh listener per request and a fresh conversation per accepted socket.
//  A socket that never proves itself (another page found the port, a stale
//  tab) is closed and the listener keeps waiting, which is also what makes
//  "open the page again" work: a new socket, the same port, the same token.
//  At the first outcome — or a cancel, or five minutes — the listener and
//  every socket close.
//
//  Runs on the main queue: per message the core frames a few bytes and checks
//  at most one signature, and one queue is what keeps this bookkeeping free of
//  locks under the target's MainActor default.
//

import Foundation
import Network
import VelaCore

final class ClearSignerChannel {

    /// How one ceremony ended.
    enum Ending: Equatable {
        /// The core's verdict: an answer it verified, or why there is none —
        /// `declined` for a page that went away holding the request, and for
        /// a cancel.
        case outcome(ClearSignerOutcome)
        /// Nobody answered in time. The core has no clock; this is the shell's.
        case timedOut
        /// The loopback listener could not be opened, or broke under us.
        case unavailable
    }

    /// The contract's five minutes.
    static let defaultTimeout: TimeInterval = 5 * 60

    let signerUrl: String
    /// 128 random bits, base64url: the page's proof that this wallet opened
    /// it for this request. It travels in the URL fragment, never to a server.
    let token: String
    /// The request's id on the wire.
    let id: String

    private let requestJson: String
    private let digest: Data
    private let keys: [WalletKeyRecord]
    private let timeout: TimeInterval

    private var listener: NWListener?
    private(set) var port: UInt16?
    /// Every open socket and the conversation it is having.
    private var sockets: [ObjectIdentifier: (socket: NWConnection, conversation: ClearSignerConnection)] = [:]
    private var ended: Ending?
    private var waiting: [CheckedContinuation<Ending, Never>] = []
    /// `open()`'s caller, until the listener is ready or failed.
    private var binding: CheckedContinuation<UInt16?, Never>?
    private var deadline: DispatchWorkItem?

    /// `requestJson` is `clearSignerRequest`'s; `digest` what the passkey
    /// must sign; `keys` the account's founding keys.
    init(
        signerUrl: String,
        requestJson: String,
        digest: Data,
        keys: [WalletKeyRecord],
        timeout: TimeInterval = ClearSignerChannel.defaultTimeout
    ) {
        self.signerUrl = signerUrl
        self.requestJson = requestJson
        self.digest = digest
        self.keys = keys
        self.timeout = timeout
        self.token = Self.freshToken()
        self.id = UUID().uuidString.lowercased()
    }

    /// The page, told where to connect and how to prove itself. `nil` until
    /// the listener is open.
    var launchUrl: URL? {
        port.flatMap { URL(string: clearSignerWsLaunch(base: signerUrl, port: $0, token: token)) }
    }

    // MARK: - The listener

    /// Binds `127.0.0.1:0` and answers the port the system chose; `nil` when
    /// no listener could be opened (and the ceremony has then ended
    /// `unavailable`). The five minutes start here.
    func open() async -> UInt16? {
        if let port { return port }
        guard ended == nil else { return nil }
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: .ipv4(.loopback), port: .any)
        guard let listener = try? NWListener(using: parameters) else {
            finish(.unavailable)
            return nil
        }
        self.listener = listener
        // Every handler runs on the main queue, where this class lives.
        listener.newConnectionHandler = { [weak self] socket in
            MainActor.assumeIsolated { self?.accept(socket) }
        }
        listener.stateUpdateHandler = { [weak self] state in
            MainActor.assumeIsolated { self?.listenerChanged(state) }
        }
        let bound: UInt16? = await withCheckedContinuation { continuation in
            binding = continuation
            listener.start(queue: .main)
        }
        guard let bound, ended == nil else {
            finish(.unavailable)
            return nil
        }
        port = bound
        let deadline = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.finish(.timedOut) }
        }
        self.deadline = deadline
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: deadline)
        return bound
    }

    private func listenerChanged(_ state: NWListener.State) {
        switch state {
        case .ready:
            binding?.resume(returning: listener?.port?.rawValue)
            binding = nil
        case .failed, .cancelled:
            binding?.resume(returning: nil)
            binding = nil
            // After our own `finish` this is the echo of its cancel, and
            // `finish` ignores a second ending.
            finish(.unavailable)
        default:
            break
        }
    }

    /// Waits for the ceremony's end. A cancelled task cancels the ceremony —
    /// the send screen's cancel is a `Task` cancel, and it must not leave a
    /// listener behind.
    func ending() async -> Ending {
        if let ended { return ended }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                if let ended {
                    continuation.resume(returning: ended)
                } else {
                    waiting.append(continuation)
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.cancel() }
        }
    }

    /// The waiting sheet's cancel: the person declined (contract §2).
    func cancel() {
        finish(.outcome(.refused(refusal: .declined)))
    }

    // MARK: - Sockets

    private func accept(_ socket: NWConnection) {
        guard ended == nil,
              let conversation = try? ClearSignerConnection(
                signerUrl: signerUrl, token: token, id: id,
                requestJson: requestJson, digest: digest, keys: keys
              )
        else {
            socket.cancel()
            return
        }
        let key = ObjectIdentifier(socket)
        sockets[key] = (socket, conversation)
        socket.stateUpdateHandler = { [weak self] state in
            switch state {
            case .failed, .cancelled: MainActor.assumeIsolated { self?.socketEnded(key) }
            default: break
            }
        }
        socket.start(queue: .main)
        receive(on: socket, key: key)
    }

    private func receive(on socket: NWConnection, key: ObjectIdentifier) {
        socket.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] data, _, isComplete, error in
            MainActor.assumeIsolated {
                self?.received(data, isComplete: isComplete || error != nil, on: socket, key: key)
            }
        }
    }

    private func received(_ data: Data?, isComplete: Bool, on socket: NWConnection, key: ObjectIdentifier) {
        guard ended == nil, let conversation = sockets[key]?.conversation else { return }
        if let data, !data.isEmpty {
            let step = conversation.feed(bytes: data)
            if step.close {
                // Its last word is said: the socket closes once the write
                // lands, and its end is no longer news to anybody.
                sockets[key] = nil
            }
            write(step.write, to: socket, thenClose: step.close)
            if let outcome = step.outcome {
                finish(.outcome(outcome))
                return
            }
            if step.close { return }
        }
        if isComplete {
            socketEnded(key)
            return
        }
        receive(on: socket, key: key)
    }

    private func write(_ bytes: Data, to socket: NWConnection, thenClose close: Bool) {
        guard !bytes.isEmpty else {
            if close { socket.cancel() }
            return
        }
        socket.send(content: bytes, completion: .contentProcessed { _ in
            if close { socket.cancel() }
        })
    }

    /// The socket closed under its conversation. A page that had the request
    /// and went away was closed without signing; one that never proved itself
    /// changes nothing, and the listener keeps waiting.
    private func socketEnded(_ key: ObjectIdentifier) {
        guard let (socket, conversation) = sockets.removeValue(forKey: key) else { return }
        socket.cancel()
        if let refusal = conversation.closed() {
            finish(.outcome(.refused(refusal: refusal)))
        }
    }

    /// The first ending wins; the listener and every socket close with it.
    private func finish(_ ending: Ending) {
        guard ended == nil else { return }
        ended = ending
        deadline?.cancel()
        listener?.cancel()
        sockets.values.forEach { $0.socket.cancel() }
        sockets.removeAll()
        let waiters = waiting
        waiting.removeAll()
        waiters.forEach { $0.resume(returning: ending) }
    }

    // MARK: - The token

    /// 16 bytes from the system's CSPRNG, base64url without padding.
    static func freshToken() -> String {
        var generator = SystemRandomNumberGenerator()
        let bytes = (0..<16).map { _ in UInt8.random(in: .min ... .max, using: &generator) }
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
