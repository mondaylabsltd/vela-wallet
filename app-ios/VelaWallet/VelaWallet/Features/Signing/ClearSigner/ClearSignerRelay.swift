//
//  ClearSignerRelay.swift
//  VelaWallet
//
//  The wallet's end of the Clear Signer relay (spec 075,
//  `contracts/relay.md`): the channel to a signer page on ANOTHER device.
//
//  The relay is dumb and blind. It pairs two sockets in a room and forwards
//  their frames; it never sees a key, an intent or a signature. What makes
//  that safe is the session both ends run over it — P-256 ECDH, HKDF-SHA256,
//  AES-GCM — and that session is the CORE's (`ClearSignerHandshake` /
//  `ClearSignerSession`), byte-for-byte the page's `lib/transport/secure.js`.
//  Nothing in this file derives a key, a code or an IV.
//
//  What is here is the part only a shell can do:
//
//  - a WebSocket to `<relay>/v1/rooms/<room>?role=requester`;
//  - the order of the handshake (wait for `joined`, then the page's hello,
//    then ours) and the frames the sealed messages ride in;
//  - the pairing link the QR carries, with the `rk` that lets the PAGE refuse
//    a stand-in wallet;
//  - the six digits the person compares, and the rule that **nothing is sent
//    until they say the two screens agree** — that is what stops a stand-in
//    PAGE, which matters most for a create, where a substituted page would
//    hand this wallet somebody else's key.
//
//  Every answer is judged by the core, exactly as on the loopback channel:
//  `clearSignerVerifyCeremony` for a ceremony, `clearSignerVerify` for a
//  signature.
//

import Foundation
import VelaCore

/// A frame on a relay socket. Text carries the relay's own words and the two
/// hellos; everything after the handshake is binary and sealed.
enum ClearSignerFrame: Equatable {
    case text(String)
    case binary(Data)
}

/// The socket under the relay conversation — a seam, so the pairing can be
/// tested against a fake relay without a server.
protocol ClearSignerSocket: AnyObject {
    func send(_ frame: ClearSignerFrame) async -> Bool
    /// The next frame, or `nil` once the socket has ended.
    func receive() async -> ClearSignerFrame?
    func close()
}

/// `URLSessionWebSocketTask`, as a `ClearSignerSocket`.
final class WebSocketRelaySocket: ClearSignerSocket {
    private let task: URLSessionWebSocketTask
    private var done = false

    init?(url: String) {
        guard let url = URL(string: url) else { return nil }
        task = URLSession.shared.webSocketTask(with: url)
        task.resume()
    }

    func send(_ frame: ClearSignerFrame) async -> Bool {
        guard !done else { return false }
        let message: URLSessionWebSocketTask.Message = switch frame {
        case .text(let text): .string(text)
        case .binary(let data): .data(data)
        }
        return await withCheckedContinuation { continuation in
            task.send(message) { error in continuation.resume(returning: error == nil) }
        }
    }

    func receive() async -> ClearSignerFrame? {
        guard !done else { return nil }
        let frame: ClearSignerFrame? = await withCheckedContinuation { continuation in
            task.receive { result in
                switch result {
                case .success(.string(let text)): continuation.resume(returning: .text(text))
                case .success(.data(let data)): continuation.resume(returning: .binary(data))
                case .success: continuation.resume(returning: nil)
                case .failure: continuation.resume(returning: nil)
                }
            }
        }
        if frame == nil { done = true }
        return frame
    }

    func close() {
        done = true
        task.cancel(with: .normalClosure, reason: nil)
    }
}

/// One pairing with a signer page on another device, and the session of
/// requests that follows it.
final class ClearSignerRelayConversation: ClearSignerConversation {

    /// The page the link points at — and what every answer's origin is
    /// checked against.
    let signerUrl: String
    /// The QR's payload, and what "copy link" copies.
    let link: String
    /// `<relay>/v1/rooms/<room>?role=requester`.
    let roomUrl: String

    private let app: String
    private let openSocket: (String) -> ClearSignerSocket?
    /// Each request's id on the wire — a seam, so a test can replay the
    /// shared vectors' own ids.
    private let nextId: () -> String
    private let handshake: ClearSignerHandshake
    private var socket: ClearSignerSocket?
    private var session: ClearSignerSession?
    /// The session's `n`: the larger of what we sent and what we saw, plus
    /// one (PROTOCOL.md §4).
    private var counter: UInt64 = 0
    private var ended = false

    /// `nil` when the room id or the handshake key could not be drawn — a
    /// ~2⁻³² event the caller retries, or falls back to this device.
    init?(
        signerUrl: String,
        relay: String,
        app: String = ClearSignerRelayConversation.appName,
        random: (Int) -> Data = ClearSignerRelayConversation.randomBytes,
        openSocket: @escaping (String) -> ClearSignerSocket? = { WebSocketRelaySocket(url: $0) },
        nextId: @escaping () -> String = { UUID().uuidString.lowercased() }
    ) {
        guard let room = clearSignerRelayRoom(random: random(16)),
              let handshake = try? ClearSignerHandshake(secret: random(32), nonce: random(16))
        else { return nil }
        self.signerUrl = signerUrl
        self.app = app
        self.openSocket = openSocket
        self.nextId = nextId
        self.handshake = handshake
        self.roomUrl = clearSignerRelayRoomUrl(relay: relay, room: room)
        self.link = clearSignerRelayLink(
            signerUrl: signerUrl, relay: relay, room: room,
            rk: clearSignerKeyFingerprint(publicKey: handshake.publicKey())
        )
    }

    /// `vela-ios/<version>` — what the page shows as "who is asking".
    static var appName: String { "vela-ios/\(BuildInfo.version)" }

    /// The system CSPRNG. `nonisolated` because it is this type's DEFAULT
    /// argument, and a default argument is evaluated outside any actor —
    /// `PasskeyExecutor.random` cannot be, under this target's MainActor
    /// default.
    nonisolated static func randomBytes(_ count: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: count)
        // A session key from predictable bytes is a session somebody else can
        // stand in the middle of; there is no weaker fallback worth having.
        let status = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        precondition(status == errSecSuccess, "the system CSPRNG refused: \(status)")
        return Data(bytes)
    }

    // MARK: - Pairing

    /// Connect, wait for the page, and derive the session. Answers the six
    /// digits both screens must show, or `nil` when the relay or the page
    /// never came through.
    func pair() async -> String? {
        guard let socket = openSocket(roomUrl) else { return nil }
        self.socket = socket
        // The relay says `joined` when both roles are present; the page then
        // speaks first. A hello that arrives before we read `joined` is
        // taken as it comes — the relay never buffers, so it cannot be early.
        while !ended {
            guard let frame = await socket.receive() else { return nil }
            guard case .text(let text) = frame, let message = Self.json(text) else { continue }
            if message["t"] as? String == "hello" { return await complete(with: text) }
            // `{"v":1,"relay":"joined"|"left"}` — neither is ours to answer.
            guard message["relay"] == nil else { continue }
        }
        return nil
    }

    /// Our hello, then the derivation. Ours goes out FIRST: the page needs it
    /// to derive the same key, and it checks our key against the link's `rk`
    /// before it will talk to us at all.
    private func complete(with pageHello: String) async -> String? {
        guard let socket, await socket.send(.text(handshake.hello(app: app))),
              let session = try? handshake.complete(peerHello: pageHello, relay: true)
        else { return nil }
        self.session = session
        return session.code()
    }

    // MARK: - Requests

    /// The first request of the session. The person must already have
    /// confirmed the code — nothing reaches a page nobody vouched for.
    func begin(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending {
        await send(ask)
    }

    func send(_ ask: ClearSignerAsk) async -> ClearSignerChannel.Ending {
        guard !ended, let socket, let session else { return .unavailable }
        let id = nextId()
        guard let intent = Self.intent(ask, id: id, n: next()),
              await socket.send(.binary(session.seal(plaintext: intent, msgId: nil)))
        else { return .unavailable }
        return await answer(to: ask, id: id)
    }

    /// The flow is over: a sealed `bye`, then the socket closes.
    func end() {
        guard !ended else { return }
        ended = true
        if let socket, let session {
            let bye = Self.body(["v": 1, "t": "bye", "n": next(), "reason": "done"])
            let sealed = session.seal(plaintext: bye ?? Data(), msgId: nil)
            Task { [socket] in
                _ = await socket.send(.binary(sealed))
                socket.close()
            }
        } else {
            socket?.close()
        }
        socket = nil
        session = nil
    }

    /// The person closed the pairing sheet, or left the flow.
    func cancel() { end() }

    // MARK: - The wire

    private func next() -> UInt64 {
        counter += 1
        return counter
    }

    /// `{v,t:"intent",n,id,intent,context}` — the request the core built,
    /// unwrapped into the session's envelope.
    private static func intent(_ ask: ClearSignerAsk, id: String, n: UInt64) -> Data? {
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

    /// Wait for the page's answer to `id`, opened and judged by the core.
    private func answer(to ask: ClearSignerAsk, id: String) async -> ClearSignerChannel.Ending {
        guard let socket, let session else { return .unavailable }
        while !ended {
            guard let frame = await socket.receive() else {
                // The socket died under the request; nothing was signed.
                return .unavailable
            }
            switch frame {
            case .text:
                // The relay's own words (`joined`, `left`). A page that left
                // may come back in the same room, so this waits rather than
                // giving up on it.
                continue
            case .binary(let sealed):
                guard let plain = try? session.open(sealed: sealed, msgId: nil),
                      let message = Self.json(String(decoding: plain, as: UTF8.self))
                else { continue }
                if let n = (message["n"] as? NSNumber)?.uint64Value, n > counter { counter = n }
                guard message["id"] as? String == id || message["t"] as? String == "bye" else { continue }
                if let verdict = judge(message, ask: ask) { return verdict }
            }
        }
        return .unavailable
    }

    /// The core's verdict on one answer; `nil` for a message that is not one.
    private func judge(_ message: [String: Any], ask: ClearSignerAsk) -> ClearSignerChannel.Ending? {
        switch message["t"] as? String {
        case "bye":
            // The page hung up holding the request: closed without signing.
            return refusal(.declined, for: ask)
        case "error":
            switch ask {
            case .ceremony(_, let operationJson, let memberChallenge):
                // The core reads an `error` envelope itself, so the page's
                // own code reaches the machine unedited.
                return .ceremony(clearSignerVerifyCeremony(
                    operationJson: operationJson, answerJson: Self.body(message).map(Self.text) ?? "{}",
                    signerOrigin: signerUrl, expectedMemberChallenge: memberChallenge
                ))
            case .signature:
                return refusal(Self.refusal(code: message["code"] as? String ?? ""), for: ask)
            }
        case "result":
            switch ask {
            case .ceremony(_, let operationJson, let memberChallenge):
                return .ceremony(clearSignerVerifyCeremony(
                    operationJson: operationJson, answerJson: Self.body(message).map(Self.text) ?? "{}",
                    signerOrigin: signerUrl, expectedMemberChallenge: memberChallenge
                ))
            case .signature(_, let digest, let keys):
                guard let result = message["result"] as? [String: Any],
                      let json = Self.body(result).map(Self.text)
                else { return refusal(.malformed(detail: "no result"), for: ask) }
                return .outcome(clearSignerVerify(resultJson: json, digest: digest, keys: keys))
            }
        default:
            return nil
        }
    }

    private func refusal(_ refusal: ClearSignerRefusal, for ask: ClearSignerAsk) -> ClearSignerChannel.Ending {
        switch ask {
        case .ceremony: .ceremony(.refused(refusal: refusal))
        case .signature: .outcome(.refused(refusal: refusal))
        }
    }

    /// The page's `code`, in the core's vocabulary — `clear_signer::refusal`,
    /// which the bindings do not export on its own. A signing answer is the
    /// only place this shell needs it: a ceremony's `error` envelope goes to
    /// `clearSignerVerifyCeremony`, which reads the code itself.
    static func refusal(code: String) -> ClearSignerRefusal {
        code.isEmpty || code == "user_rejected" ? .declined : .pageRefused(code: code)
    }

    private static func json(_ text: String) -> [String: Any]? {
        (try? JSONSerialization.jsonObject(with: Data(text.utf8))) as? [String: Any]
    }

    private static func body(_ object: Any) -> Data? {
        try? JSONSerialization.data(withJSONObject: object)
    }

    private static func text(_ data: Data) -> String { String(decoding: data, as: UTF8.self) }
}
