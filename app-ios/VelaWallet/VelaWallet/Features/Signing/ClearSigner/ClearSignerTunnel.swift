//
//  ClearSignerTunnel.swift
//  VelaWallet
//
//  The wallet's end of the Clear Signer tunnel (spec 075,
//  `contracts/tunnel.md`): the channel to a signer page on ANOTHER device.
//
//  The tunnel is dumb and blind. It pairs two sockets in a room and forwards
//  their frames; it never sees a key, an intent or a signature. What makes
//  that safe is the session both ends run over it — P-256 ECDH, HKDF-SHA256,
//  AES-GCM — and that session is the CORE's (`ClearSignerHandshake` /
//  `ClearSignerSession`), byte-for-byte the page's `lib/transport/secure.js`.
//  Nothing in this file derives a key, a code or an IV.
//
//  What is here is the part only a shell can do:
//
//  - a WebSocket to `<tunnel>/v1/rooms/<room>?role=requester`;
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
import UIKit
import VelaCore

/// A frame on a tunnel socket. Text carries the tunnel's own words and the two
/// hellos; everything after the handshake is binary and sealed.
enum ClearSignerFrame: Equatable {
    case text(String)
    case binary(Data)
}

/// The socket under the tunnel conversation — a seam, so the pairing can be
/// tested against a fake tunnel without a server.
protocol ClearSignerSocket: AnyObject {
    func send(_ frame: ClearSignerFrame) async -> Bool
    /// The next frame, or `nil` once the socket has ended.
    func receive() async -> ClearSignerFrame?
    func close()
}

/// `URLSessionWebSocketTask`, as a `ClearSignerSocket`.
final class WebSocketTunnelSocket: ClearSignerSocket {
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
final class ClearSignerTunnelConversation: ClearSignerConversation {

    /// The page the link points at — and what every answer's origin is
    /// checked against.
    let signerUrl: String
    /// The QR's payload, and what "copy link" copies.
    let link: String
    /// `<tunnel>/v1/rooms/<room>?role=requester`.
    let roomUrl: String

    private let app: String
    private let icon: String
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
        tunnel: String,
        app: String = ClearSignerTunnelConversation.appName,
        icon: String = ClearSignerTunnelConversation.appIcon,
        random: (Int) -> Data = ClearSignerTunnelConversation.randomBytes,
        openSocket: @escaping (String) -> ClearSignerSocket? = { WebSocketTunnelSocket(url: $0) },
        nextId: @escaping () -> String = { UUID().uuidString.lowercased() }
    ) {
        guard let room = clearSignerTunnelRoom(random: random(16)),
              let handshake = try? ClearSignerHandshake(secret: random(32), nonce: random(16))
        else { return nil }
        self.signerUrl = signerUrl
        self.app = app
        self.icon = icon
        self.openSocket = openSocket
        self.nextId = nextId
        self.handshake = handshake
        self.roomUrl = clearSignerTunnelRoomUrl(tunnel: tunnel, room: room)
        self.link = clearSignerTunnelLink(
            signerUrl: signerUrl, tunnel: tunnel, room: room,
            rk: clearSignerKeyFingerprint(publicKey: handshake.publicKey())
        )
    }

    /// What the page shows as "who is asking" — a product name a person
    /// recognises, not a user-agent string. The page says in as many words
    /// that this is self-reported, so it may as well be readable.
    static var appName: String { "Vela Wallet \(BuildInfo.version)" }

    /// The app icon this bundle actually ships, at its largest listed size.
    ///
    /// `UIImage(named:)` on an icon file name is the only way to reach it: an
    /// app cannot load its own icon from the asset catalog by symbol.
    private static var bundleIcon: UIImage? {
        guard let icons = Bundle.main.infoDictionary?["CFBundleIcons"] as? [String: Any],
              let primary = icons["CFBundlePrimaryIcon"] as? [String: Any],
              let files = primary["CFBundleIconFiles"] as? [String],
              let name = files.last
        else { return nil }
        return UIImage(named: name)
    }

    /// This app's own mark, as the small inline PNG the page accepts.
    ///
    /// Rendered once and kept: it cannot change while the app is installed,
    /// and a handshake travels in the clear in 244-byte frames, so the size
    /// matters more than the fidelity — 64 px is what the page draws. The core
    /// refuses anything over its cap, so an oversized string is not built.
    static let appIcon: String = {
        guard let icon = bundleIcon,
              let data = UIGraphicsImageRenderer(size: CGSize(width: 64, height: 64))
                  .image(actions: { _ in icon.draw(in: CGRect(x: 0, y: 0, width: 64, height: 64)) })
                  .pngData()
        else { return "" }
        let uri = "data:image/png;base64,\(data.base64EncodedString())"
        return uri.count <= 6144 ? uri : ""
    }()

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
    /// digits both screens must show, or `nil` when the tunnel or the page
    /// never came through.
    func pair() async -> String? {
        guard let socket = openSocket(roomUrl) else { return nil }
        self.socket = socket
        // The tunnel says `joined` when both roles are present; the page then
        // speaks first. A hello that arrives before we read `joined` is
        // taken as it comes — the tunnel never buffers, so it cannot be early.
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
        guard let socket, await socket.send(.text(handshake.hello(app: app, icon: icon))),
              let session = try? handshake.complete(peerHello: pageHello, tunnel: true)
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
        guard let intent = ClearSignerAnswer.intent(ask, id: id, n: next()),
              await socket.send(.binary(session.seal(plaintext: intent, msgId: nil)))
        else { return .unavailable }
        return await answer(to: ask, id: id)
    }

    /// The flow is over: a sealed `bye`, then the socket closes.
    func end() {
        guard !ended else { return }
        ended = true
        if let socket, let session {
            let bye = ClearSignerAnswer.body(["v": 1, "t": "bye", "n": next(), "reason": "done"])
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
                // The tunnel's own words (`joined`, `left`). A page that left
                // may come back in the same room, so this waits rather than
                // giving up on it.
                continue
            case .binary(let sealed):
                guard let plain = try? session.open(sealed: sealed, msgId: nil),
                      let message = Self.json(String(decoding: plain, as: UTF8.self))
                else { continue }
                if let n = (message["n"] as? NSNumber)?.uint64Value, n > counter { counter = n }
                guard message["id"] as? String == id || message["t"] as? String == "bye" else { continue }
                if let verdict = ClearSignerAnswer.verdict(message, ask: ask, signerUrl: signerUrl) {
                    return verdict
                }
            }
        }
        return .unavailable
    }

    /// The page's `code`, in the core's vocabulary. Kept as a name here
    /// because it is the tunnel's own vocabulary too; the one implementation
    /// is `ClearSignerAnswer`'s, shared with the BLE peripheral.
    static func refusal(code: String) -> ClearSignerRefusal {
        ClearSignerAnswer.refusal(code: code)
    }

    private static func json(_ text: String) -> [String: Any]? {
        ClearSignerAnswer.json(text)
    }
}
