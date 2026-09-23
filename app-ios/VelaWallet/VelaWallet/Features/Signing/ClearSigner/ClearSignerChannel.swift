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
//  ## One flow, then nothing
//
//  A fresh listener per FLOW, and a fresh conversation per accepted socket
//  until one of them answers. A socket that never proves itself (another page
//  found the port, a stale tab) is closed and the listener keeps waiting,
//  which is also what makes "open the page again" work: a new socket, the same
//  port, the same token. Once a socket has answered it holds the session and
//  every other one is dropped — a second conversation replaying the opening
//  request would put the same ceremony twice. At `end()` — or a cancel, or
//  five idle minutes — the listener and every socket close.
//
//  Runs on the main queue: per message the core frames a few bytes and checks
//  at most one signature, and one queue is what keeps this bookkeeping free of
//  locks under the target's MainActor default.
//
//  ## A session, not a ceremony (spec 075)
//
//  A flow opens the page ONCE and puts its requests to it in turn — create
//  then the member proof, a sign-in then recovery's two proofs. After an
//  answer the core's conversation stays open for `send(_:)`, and `end()`
//  says `bye` and closes. That is why an answer no longer tears the listener
//  down: only a timeout, a cancel, or `end()` do.
//

import Foundation
import Network
import VelaCore

/// One request of a session: a passkey ceremony (spec 075) or a signature
/// (071). Both ride the same channels; only who judges the answer differs,
/// and that judge is the core's.
enum ClearSignerAsk {
    /// `request` is `clearSignerCeremonyRequest`'s, `operationJson` the
    /// machine operation's own wire JSON, `memberChallenge` the registry
    /// challenge the WALLET fetched (a member proof only).
    case ceremony(request: String, operationJson: String, memberChallenge: Data?)
    /// `request` is `clearSignerRequest`'s; the answer must sign `digest`
    /// with one of `keys`.
    case signature(request: String, digest: Data, keys: [WalletKeyRecord])
}

/// A page that is holding this flow's session: the loopback tab on this
/// device (`ClearSignerChannel`), or the tunnel to another device
/// (`ClearSignerTunnelConversation`).
protocol ClearSignerConversation: AnyObject {
    /// Opens whatever has to be opened and puts the FIRST request. `nil` when
    /// nothing could be opened.
    func begin(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending
    /// The session's next request, once the last one has its answer.
    func send(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending
    /// The flow is over — `bye`, then close.
    func end()
}

/// The envelope work the SEALED channels share (PROTOCOL.md §4): the request
/// that goes out, the verdict that comes back, and the page's refusal codes in
/// the core's vocabulary.
///
/// The tunnel (`ClearSignerTunnelConversation`) and the BLE peripheral
/// (`ClearSignerBleConversation`) run the same core session; the only thing
/// that differs between them is what carries the bytes — a WebSocket frame or
/// a GATT notification. So the JSON lives here once rather than twice, where
/// one copy could start shaping `n` or `id` differently from the other and
/// only one of the two channels would drift from the page.
///
/// The loopback channel does NOT use this: there the core's
/// `ClearSignerConnection` writes the whole envelope itself.
enum ClearSignerAnswer {

    /// `{v,t:"intent",n,id,intent,context}` — the request the core built,
    /// unwrapped into the session's envelope.
    static func intent(_ ask: ClearSignerAsk, id: String, n: UInt64) -> Data? {
        let requestJson = switch ask {
        case .ceremony(let request, _, _): request
        case .signature(let request, _, _): request
        }
        guard let request = json(requestJson) else { return nil }
        var message: [String: Any] = ["v": 1, "t": "intent", "n": n, "id": id]
        message["intent"] = request["intent"] ?? NSNull()
        message["context"] = request["context"] ?? NSNull()
        return body(message)
    }

    /// The core's verdict on one answer; `nil` for a message that is not one.
    static func verdict(
        _ message: [String: Any], ask: ClearSignerAsk, signerUrl: String
    ) -> ClearSignerChannel.Ending? {
        switch message["t"] as? String {
        case "bye":
            // The page hung up holding the request: closed without signing.
            return ClearSignerChannel.declined(for: ask)
        case "error":
            switch ask {
            case .ceremony(_, let operationJson, let memberChallenge):
                // The core reads an `error` envelope itself, so the page's own
                // code reaches the machine unedited.
                return .ceremony(clearSignerVerifyCeremony(
                    operationJson: operationJson, answerJson: body(message).map(text) ?? "{}",
                    signerOrigin: signerUrl, expectedMemberChallenge: memberChallenge
                ))
            case .signature:
                return ClearSignerChannel.refused(
                    refusal(code: message["code"] as? String ?? ""), for: ask
                )
            }
        case "result":
            switch ask {
            case .ceremony(_, let operationJson, let memberChallenge):
                return .ceremony(clearSignerVerifyCeremony(
                    operationJson: operationJson, answerJson: body(message).map(text) ?? "{}",
                    signerOrigin: signerUrl, expectedMemberChallenge: memberChallenge
                ))
            case .signature(_, let digest, let keys):
                guard let result = message["result"] as? [String: Any],
                      let json = body(result).map(text)
                else { return ClearSignerChannel.refused(.malformed(detail: "no result"), for: ask) }
                return .outcome(clearSignerVerify(resultJson: json, digest: digest, keys: keys))
            }
        default:
            return nil
        }
    }

    /// The page's `code`, in the core's vocabulary — `clear_signer::refusal`,
    /// which the bindings do not export on its own. A signing answer is the
    /// only place a shell needs it: a ceremony's `error` envelope goes to
    /// `clearSignerVerifyCeremony`, which reads the code itself.
    static func refusal(code: String) -> ClearSignerRefusal {
        code.isEmpty || code == "user_rejected" ? .declined : .pageRefused(code: code)
    }

    static func json(_ text: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any]
    }

    static func body(_ object: Any) -> Data? {
        try? JSONSerialization.data(withJSONObject: object)
    }

    static func text(_ data: Data) -> String { String(decoding: data, as: UTF8.self) }
}

final class ClearSignerChannel: ClearSignerConversation {

    /// How one REQUEST of a session ended.
    enum Ending: Equatable {
        /// The core's verdict on a signature: an answer it verified, or why
        /// there is none — `declined` for a page that went away holding the
        /// request, and for a cancel.
        case outcome(ClearSignerOutcome)
        /// Spec 075: the core's verdict on a passkey ceremony.
        case ceremony(ClearSignerCeremonyOutcome)
        /// Nobody answered in time. The core has no clock; this is the shell's.
        case timedOut
        /// The loopback listener could not be opened, or broke under us.
        case unavailable
    }

    /// The contract's five minutes — idle, so a session of several requests
    /// is not cut off halfway (contract §1.5).
    static let defaultTimeout: TimeInterval = 5 * 60

    let signerUrl: String
    /// 128 random bits, base64url: the page's proof that this wallet opened
    /// it for this request. It travels in the URL fragment, never to a server.
    let token: String
    /// The CURRENT request's id on the wire.
    private(set) var id: String

    /// The session's opening request — replayed to a page that reconnects
    /// before anything has been answered ("open the page again").
    private let first: ClearSignerAsk
    /// The request in flight. A refusal is shaped like the request that earned
    /// it, so a ceremony's decline never arrives as a signing verdict (and the
    /// spine never reads one as a shell bug).
    private var current: ClearSignerAsk
    private let timeout: TimeInterval

    private var listener: NWListener?
    private(set) var port: UInt16?
    /// Every open socket and the conversation it is having.
    private var sockets: [ObjectIdentifier: (socket: NWConnection, conversation: ClearSignerConnection)] = [:]
    /// The socket that answered: the session lives on it, and a later
    /// request goes there and nowhere else.
    private var active: ObjectIdentifier?
    /// Terminal: a timeout, a cancel, or a listener that broke. Answered to
    /// every later caller.
    private var terminal: Ending?
    /// The channel has been shut down (`end()`), with no terminal verdict.
    private var closed = false
    /// Verdicts nobody was waiting for yet — a page that closed while the
    /// flow was between requests.
    private var pendingEndings: [Ending] = []
    private var waiting: [CheckedContinuation<Ending, Never>] = []
    /// `open()`'s caller, until the listener is ready or failed.
    private var binding: CheckedContinuation<UInt16?, Never>?
    private var deadline: DispatchWorkItem?

    init(
        signerUrl: String,
        first: ClearSignerAsk,
        timeout: TimeInterval = ClearSignerChannel.defaultTimeout
    ) {
        self.signerUrl = signerUrl
        self.first = first
        self.current = first
        self.timeout = timeout
        self.token = Self.freshToken()
        self.id = UUID().uuidString.lowercased()
    }

    /// The 071 door: one signature, one page visit.
    convenience init(
        signerUrl: String,
        requestJson: String,
        digest: Data,
        keys: [WalletKeyRecord],
        timeout: TimeInterval = ClearSignerChannel.defaultTimeout
    ) {
        self.init(
            signerUrl: signerUrl,
            first: .signature(request: requestJson, digest: digest, keys: keys),
            timeout: timeout
        )
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
        guard !stopped else { return nil }
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
        guard let bound, !stopped else {
            finish(.unavailable)
            return nil
        }
        port = bound
        armDeadline()
        return bound
    }

    /// Whether anything more can happen here.
    private var stopped: Bool { closed || terminal != nil }

    /// The idle clock, restarted. A session of several requests is cut off
    /// only when nothing has moved for the whole window (contract §1.5).
    private func armDeadline() {
        deadline?.cancel()
        guard !stopped else { return }
        let deadline = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.finish(.timedOut) }
        }
        self.deadline = deadline
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: deadline)
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

    /// Waits for the CURRENT request's verdict. A cancelled task cancels the
    /// ceremony — the send screen's cancel is a `Task` cancel, and it must
    /// not leave a listener behind.
    func ending() async -> Ending {
        if let next = take() { return next }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                if let next = take() {
                    continuation.resume(returning: next)
                } else {
                    waiting.append(continuation)
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.cancel() }
        }
    }

    /// The verdict already in hand, if there is one.
    private func take() -> Ending? {
        if !pendingEndings.isEmpty { return pendingEndings.removeFirst() }
        if let terminal { return terminal }
        return closed ? .unavailable : nil
    }

    // MARK: - A session of several requests (spec 075)

    /// Opens the listener and waits for the first request's verdict. The page
    /// itself is opened by whoever owns the tab — this only has to be running
    /// before it is.
    func begin(_ ask: ClearSignerAsk) async -> Ending {
        guard await open() != nil else { return .unavailable }
        return await ending()
    }

    /// The session's next request, on the socket that answered the last one.
    func send(_ ask: ClearSignerAsk) async -> Ending {
        guard !stopped else { return .unavailable }
        // The page went away BETWEEN requests — it answered the last one and
        // then closed. That is a page closed without signing this one, not a
        // channel that could not be opened, and the two get different
        // sentences on screen.
        guard let key = active, let held = sockets[key] else {
            return Self.declined(for: ask)
        }
        let next = UUID().uuidString.lowercased()
        let step: ClearSignerStep
        do {
            switch ask {
            case .ceremony(let request, let operationJson, let memberChallenge):
                step = try held.conversation.sendCeremony(
                    id: next, requestJson: request, operationJson: operationJson,
                    expectedMemberChallenge: memberChallenge
                )
            case .signature(let request, let digest, let keys):
                step = try held.conversation.sendSignature(
                    id: next, requestJson: request, digest: digest, keys: keys
                )
            }
        } catch {
            return .unavailable
        }
        // An empty step is the core saying "not now" — the last request has
        // no answer yet, or the conversation is over.
        guard !step.write.isEmpty else { return .unavailable }
        id = next
        current = ask
        write(step.write, to: held.socket, thenClose: step.close)
        armDeadline()
        return await ending()
    }

    /// A refusal shaped like the request that earned it.
    static func declined(for ask: ClearSignerAsk) -> Ending {
        refused(.declined, for: ask)
    }

    static func refused(_ refusal: ClearSignerRefusal, for ask: ClearSignerAsk) -> Ending {
        switch ask {
        case .ceremony: .ceremony(.refused(refusal: refusal))
        case .signature: .outcome(.refused(refusal: refusal))
        }
    }

    /// The same, for the request in flight.
    private func refused(_ refusal: ClearSignerRefusal) -> Ending {
        Self.refused(refusal, for: current)
    }

    /// The flow is done: `bye` on the wire, then everything closes. The page
    /// leaves its waiting screen for its done screen.
    func end() {
        guard !stopped else { return }
        if let key = active, let held = sockets.removeValue(forKey: key) {
            let step = held.conversation.end()
            // Written and THEN closed by the completion, so the `bye` lands.
            write(step.write, to: held.socket, thenClose: true)
        }
        active = nil
        closed = true
        shutDown()
        let waiters = waiting
        waiting.removeAll()
        waiters.forEach { $0.resume(returning: .unavailable) }
    }

    /// The waiting sheet's cancel: the person declined (contract §2).
    func cancel() {
        finish(refused(.declined))
    }

    /// The person closed the tab. WebKit does not always close a dismissed
    /// page's socket, so this end closes every one it holds and the core says
    /// what that means: declined for a page that had the request, nothing
    /// for one that never proved itself — which leaves the listener, and
    /// "open the page again", waiting.
    func pageClosed() {
        sockets.keys.forEach(socketEnded)
    }

    // MARK: - Sockets

    /// A fresh conversation holding the session's opening request.
    ///
    /// Only while nothing has been answered: once a socket has the session,
    /// a second one replaying the first request would put the same ceremony
    /// twice. "Open the page again" is exactly the before case.
    private func opening() -> ClearSignerConnection? {
        guard active == nil else { return nil }
        switch first {
        case .signature(let request, let digest, let keys):
            return try? ClearSignerConnection(
                signerUrl: signerUrl, token: token, id: id,
                requestJson: request, digest: digest, keys: keys
            )
        case .ceremony(let request, let operationJson, let memberChallenge):
            return try? ClearSignerConnection.newCeremony(
                signerUrl: signerUrl, token: token, id: id,
                requestJson: request, operationJson: operationJson,
                expectedMemberChallenge: memberChallenge
            )
        }
    }

    private func accept(_ socket: NWConnection) {
        guard !stopped, let conversation = opening() else {
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
        guard !stopped, let conversation = sockets[key]?.conversation else { return }
        if let data, !data.isEmpty {
            let step = conversation.feed(bytes: data)
            if step.close {
                // Its last word is said: the socket closes once the write
                // lands, and its end is no longer news to anybody.
                sockets[key] = nil
                if active == key { active = nil }
            }
            write(step.write, to: socket, thenClose: step.close)
            // An answer no longer ends the channel (spec 075): the session
            // stays on this socket for the flow's next request — which means
            // this end must KEEP READING it. Returning here is what left the
            // second request of a session unanswered until the idle clock.
            if let verdict = step.outcome.map(Ending.outcome) ?? step.ceremony.map(Ending.ceremony) {
                if !step.close {
                    active = key
                    // Every other socket is a stranger now, and a stranger
                    // holding the port would be given the opening request.
                    sockets.keys.filter { $0 != key }.forEach(drop)
                }
                deliver(verdict)
                if step.close || sockets[key] == nil { return }
                receive(on: socket, key: key)
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

    /// Close a socket without asking its conversation what that means — it is
    /// not the one holding the session.
    private func drop(_ key: ObjectIdentifier) {
        sockets.removeValue(forKey: key)?.socket.cancel()
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
        if active == key { active = nil }
        if let refusal = conversation.closed() {
            deliver(refused(refusal))
        }
    }

    /// One request's verdict, to whoever is waiting for it — or kept for the
    /// caller that has not asked yet.
    private func deliver(_ ending: Ending) {
        guard !stopped else { return }
        armDeadline()
        if waiting.isEmpty {
            pendingEndings.append(ending)
        } else {
            waiting.removeFirst().resume(returning: ending)
        }
    }

    /// A terminal ending: the first one wins, and the listener and every
    /// socket close with it.
    private func finish(_ ending: Ending) {
        guard !stopped else { return }
        terminal = ending
        shutDown()
        let waiters = waiting
        waiting.removeAll()
        waiters.forEach { $0.resume(returning: ending) }
    }

    private func shutDown() {
        deadline?.cancel()
        deadline = nil
        listener?.cancel()
        sockets.values.forEach { $0.socket.cancel() }
        sockets.removeAll()
        active = nil
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
