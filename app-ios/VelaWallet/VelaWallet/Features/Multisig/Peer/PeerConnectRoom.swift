// PeerConnect v1 for Swift: room credentials (SPEC §3, §4).
//
// A pure-Swift (CryptoKit only) port of the PeerConnect protocol core, the
// peer side only: rooms, invites and records here; the Noise handshake and
// the sans-I/O peer state machine in PeerConnectProtocol.swift; a
// URLSessionWebSocketTask driver in PeerConnectConnection.swift. It is
// byte-compatible with the reference (the Rust crate `peerconnect`, which
// the web wallet runs as WebAssembly through its vendored JS client): a room
// record saved by either parses in the other, and the two talk to each other
// through any PeerConnect v1 relay.
//
// Ported from PeerConnect (https://peerconnect.dev), commit f3148cb:
// crates/peerconnect/src/{room,noise,peer,wire,base64}.rs and SPEC.md.
// PeerConnect is dual-licensed MIT OR Apache-2.0; this port is used under
// the MIT license:
//
//   MIT License
//
//   Copyright (c) 2026 The PeerConnect authors
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions:
//
//   The above copyright notice and this permission notice shall be included
//   in all copies or substantial portions of the Software.
//
//   THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//   OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//   MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
//   NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//   DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//   OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//   USE OR OTHER DEALINGS IN THE SOFTWARE.
//
// One deliberate difference: CryptoKit's Ed25519 signatures are randomized
// (hedged), so a JOIN frame is not byte-identical to the reference's
// deterministic one. Both are valid signatures under the same key, which is
// all a relay checks (SPEC §4.3); the tests verify the reference's JOIN
// vectors against the keys derived here instead of comparing bytes.

import CryptoKit
import Foundation

// MARK: - Errors and roles

nonisolated enum PeerConnectError: Error, Equatable, Sendable {
    /// Not a `ws://` or `wss://` URL without query or fragment.
    case invalidRelayUrl
    case invalidInvite
    case invalidRoomId
    /// A persisted room record that does not parse.
    case invalidRecord
}

/// The two seats of a room.
nonisolated enum PeerRole: UInt8, Equatable, Sendable {
    /// Created the room and handed out the invite. Noise responder.
    case inviter = 0
    /// Joined with the invite. Noise initiator.
    case joiner = 1

    /// `"inviter"` or `"joiner"`, as the JavaScript client names them.
    var name: String { self == .inviter ? "inviter" : "joiner" }
}

/// Where on the relay the two peers meet (SPEC §3.1).
nonisolated enum PeerRendezvous: Equatable, Sendable {
    /// Derived from the room secret alone; used only until the room is paired.
    case invite
    /// Derived from the room secret and both identities; used once paired.
    case pair

    var other: PeerRendezvous { self == .invite ? .pair : .invite }
}

// MARK: - base64url

/// Unpadded base64url (RFC 4648 §5), strict: padding, whitespace, foreign
/// characters and non-zero trailing bits are rejected, so every byte string
/// has exactly one accepted encoding.
nonisolated enum PeerBase64URL {
    private static let alphabet = Array("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_".utf8)

    static func encode<Bytes: Collection>(_ bytes: Bytes) -> String where Bytes.Element == UInt8 {
        let input = Array(bytes)
        var out = [UInt8]()
        out.reserveCapacity((input.count + 2) / 3 * 4)
        var i = 0
        while i < input.count {
            let chunk = input[i..<min(i + 3, input.count)]
            var n: UInt32 = 0
            for (k, b) in chunk.enumerated() { n |= UInt32(b) << (16 - 8 * UInt32(k)) }
            for k in 0...chunk.count { out.append(alphabet[Int((n >> (18 - 6 * UInt32(k))) & 63)]) }
            i += 3
        }
        return String(decoding: out, as: UTF8.self)
    }

    static func decode(_ text: String) -> [UInt8]? {
        let text = Array(text.utf8)
        if text.count % 4 == 1 { return nil }
        var out = [UInt8]()
        out.reserveCapacity(text.count * 3 / 4)
        var i = 0
        while i < text.count {
            let chunk = text[i..<min(i + 4, text.count)]
            var n: UInt32 = 0
            for (k, c) in chunk.enumerated() {
                guard let v = value(c) else { return nil }
                n |= UInt32(v) << (18 - 6 * UInt32(k))
            }
            let len = chunk.count - 1
            // Bits below the last whole byte must be zero (canonical encoding).
            if n & (0xFF_FFFF >> (8 * UInt32(len))) != 0 { return nil }
            for k in 0..<len { out.append(UInt8((n >> (16 - 8 * UInt32(k))) & 0xFF)) }
            i += 4
        }
        return out
    }

    private static func value(_ c: UInt8) -> UInt8? {
        switch c {
        case UInt8(ascii: "A")...UInt8(ascii: "Z"): return c - UInt8(ascii: "A")
        case UInt8(ascii: "a")...UInt8(ascii: "z"): return c - UInt8(ascii: "a") + 26
        case UInt8(ascii: "0")...UInt8(ascii: "9"): return c - UInt8(ascii: "0") + 52
        case UInt8(ascii: "-"): return 62
        case UInt8(ascii: "_"): return 63
        default: return nil
        }
    }
}

// MARK: - Primitives

/// The few primitives PeerConnect uses, on CryptoKit.
nonisolated enum PeerConnectCrypto {
    struct Failure: Error {}

    static func random(_ count: Int) -> [UInt8] {
        // The system generator is cryptographically secure on Apple platforms.
        var rng = SystemRandomNumberGenerator()
        return (0..<count).map { _ in UInt8.random(in: .min ... .max, using: &rng) }
    }

    static func randomU64() -> UInt64 {
        var rng = SystemRandomNumberGenerator()
        return rng.next()
    }

    static func sha256(_ parts: [UInt8]...) -> [UInt8] {
        var hash = SHA256()
        for part in parts { hash.update(data: part) }
        return Array(hash.finalize())
    }

    static func hmac(key: [UInt8], _ parts: [UInt8]...) -> [UInt8] {
        hmac(key: key, parts: parts)
    }

    static func hmac(key: [UInt8], parts: [[UInt8]]) -> [UInt8] {
        var mac = HMAC<SHA256>(key: SymmetricKey(data: key))
        for part in parts { mac.update(data: part) }
        return Array(mac.finalize())
    }

    /// The X25519 public key of a private key, used as-is (X25519 clamps).
    static func x25519Public(_ secret: [UInt8]) -> [UInt8] {
        // Only the length can make this throw, and secrets are always 32 bytes.
        guard let key = try? Curve25519.KeyAgreement.PrivateKey(rawRepresentation: secret) else { return [] }
        return Array(key.publicKey.rawRepresentation)
    }

    /// X25519, refusing a non-contributory (all-zero) result: low-order
    /// public keys cannot take part (SPEC §2).
    static func dh(_ secret: [UInt8], _ publicKey: [UInt8]) throws -> [UInt8] {
        do {
            let key = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: secret)
            let other = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: publicKey)
            let shared = try key.sharedSecretFromKeyAgreement(with: other).withUnsafeBytes { Array($0) }
            guard shared.count == 32, shared.contains(where: { $0 != 0 }) else { throw Failure() }
            return shared
        } catch {
            throw Failure()
        }
    }

    /// Whether an X25519 public key can take part in a handshake.
    static func usable(_ publicKey: [UInt8]) -> Bool {
        publicKey.count == 32 && (try? dh([UInt8](repeating: 1, count: 32), publicKey)) != nil
    }

    /// The Ed25519 public key for a 32-byte seed.
    static func ed25519Public(seed: [UInt8]) -> [UInt8] {
        guard let key = try? Curve25519.Signing.PrivateKey(rawRepresentation: seed) else { return [] }
        return Array(key.publicKey.rawRepresentation)
    }

    static func ed25519Sign(seed: [UInt8], _ message: [UInt8]) -> [UInt8] {
        guard let key = try? Curve25519.Signing.PrivateKey(rawRepresentation: seed),
              let signature = try? key.signature(for: message)
        else { return [UInt8](repeating: 0, count: 64) }
        return Array(signature)
    }

    static func ed25519Verify(publicKey: [UInt8], signature: [UInt8], _ message: [UInt8]) -> Bool {
        guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: publicKey) else { return false }
        return key.isValidSignature(signature, for: message)
    }

    static func u64be(_ value: UInt64) -> [UInt8] {
        (0..<8).map { UInt8(truncatingIfNeeded: value >> (56 - 8 * UInt64($0))) }
    }

    static func readU64be(_ bytes: [UInt8], at offset: Int) -> UInt64? {
        guard offset >= 0, bytes.count >= offset + 8 else { return nil }
        return (0..<8).reduce(UInt64(0)) { $0 << 8 | UInt64(bytes[offset + $1]) }
    }
}

// MARK: - Room IDs

/// Identifies a room on a relay: the room's Ed25519 public key, which locates
/// the room without granting access to it. Its text form is base64url.
nonisolated struct PeerRoomId: Equatable, Hashable, Sendable, CustomStringConvertible {
    let bytes: [UInt8]

    var description: String { PeerBase64URL.encode(bytes) }

    /// Parses the base64url form (32 bytes). Unlike the reference this does
    /// not check that the bytes are a valid Ed25519 point: a peer only ever
    /// derives IDs, and only a relay needs to refuse bad ones.
    init(_ text: String) throws {
        guard let bytes = PeerBase64URL.decode(text), bytes.count == 32 else { throw PeerConnectError.invalidRoomId }
        self.bytes = bytes
    }

    init(bytes: [UInt8]) {
        self.bytes = bytes
    }

    /// Relay side (used by the tests' in-memory relay): checks a JOIN frame
    /// against the HELLO nonce; returns the seat it claims, or the close code
    /// to refuse it with.
    func verifyJoin(nonce: [UInt8], frame: [UInt8]) -> (role: PeerRole?, refusal: PeerWire.CloseCode?) {
        guard frame.count == PeerWire.joinLength, frame[0] == PeerWire.join,
              let role = PeerRole(rawValue: frame[1])
        else { return (nil, PeerWire.Close.protocolError) }
        let ok = PeerConnectCrypto.ed25519Verify(
            publicKey: bytes, signature: Array(frame[2...]), PeerWire.joinMessage(nonce: nonce, role: role))
        return ok ? (role, nil) : (nil, PeerWire.Close.unauthorized)
    }
}

// MARK: - Invite

/// An invitation to a room: `<relay URL>#<base64url(secret ‖ inviter key)>`.
///
/// A bearer credential until the room is paired: whoever presents it first
/// pairs with the inviter. It must travel over a channel that keeps it
/// confidential and authentic. After pairing it is useless.
nonisolated struct PeerInvite: Sendable, CustomStringConvertible {
    let relay: String
    let secret: [UInt8]
    let inviter: [UInt8]

    init(relay: String, secret: [UInt8], inviter: [UInt8]) {
        self.relay = relay
        self.secret = secret
        self.inviter = inviter
    }

    init(_ text: String) throws {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard let hash = text.firstIndex(of: "#") else { throw PeerConnectError.invalidInvite }
        let relay = String(text[..<hash])
        let fragment = String(text[text.index(after: hash)...])
        guard let bytes = PeerBase64URL.decode(fragment), bytes.count == 64 else { throw PeerConnectError.invalidInvite }
        let inviter = Array(bytes[32...])
        guard PeerConnectCrypto.usable(inviter) else { throw PeerConnectError.invalidInvite }
        self.relay = try PeerRoom.relayURL(relay)
        self.secret = Array(bytes[..<32])
        self.inviter = inviter
    }

    var roomId: PeerRoomId {
        PeerRoomId(bytes: PeerConnectCrypto.ed25519Public(seed: PeerRoom.inviteSeed(secret)))
    }

    var description: String { "\(relay)#\(PeerBase64URL.encode(secret + inviter))" }
}

// MARK: - Room

/// One peer's side of a room: everything it needs to reconnect, at any time,
/// to any relay that speaks the protocol. Persist `record` (a secret), again
/// whenever the connection reports pairing, and after `close()`.
nonisolated struct PeerRoom: Sendable, CustomStringConvertible {
    let relay: String
    let role: PeerRole
    /// The room secret `S`.
    let secret: [UInt8]
    /// This peer's X25519 identity private key, as generated (unclamped).
    let identity: [UInt8]
    /// The other peer's identity: from the invite (joiner), or once paired (inviter).
    private(set) var peer: [UInt8]?
    private(set) var paired: Bool = false
    private(set) var closing: Bool = false

    /// Derived from the secret: the invite-ID signing seed and the psk.
    let inviteSeed: [UInt8]
    let psk: [UInt8]
    /// The pair-ID signing seed; known whenever `peer` is.
    private(set) var pairSeed: [UInt8]?

    /// Bumped whenever the layout changes, so an old record is refused rather than misread.
    static let recordVersion: UInt8 = 2
    private static let hasPeerFlag: UInt8 = 1
    private static let pairedFlag: UInt8 = 2
    private static let closingFlag: UInt8 = 4

    // MARK: Creating

    /// Creates a new room on `relay` (a `ws://` or `wss://` URL). The caller
    /// becomes the inviter; hand `invite` to the other peer.
    static func create(relay: String) throws -> PeerRoom {
        PeerRoom(relay: try relayURL(relay), role: .inviter, secret: PeerConnectCrypto.random(32),
                 identity: PeerConnectCrypto.random(32), peer: nil)
    }

    /// Accepts an invite. The caller becomes the joiner, with a new identity
    /// that the inviter will pin: persist `record` before connecting, since
    /// joining again from the same invite yields a different identity, which
    /// the inviter refuses once paired.
    static func join(invite: String) throws -> PeerRoom {
        join(try PeerInvite(invite))
    }

    static func join(_ invite: PeerInvite) -> PeerRoom {
        PeerRoom(relay: invite.relay, role: .joiner, secret: invite.secret,
                 identity: PeerConnectCrypto.random(32), peer: invite.inviter)
    }

    /// Restores a room saved with `record`.
    static func parse(_ record: String) throws -> PeerRoom {
        guard let bytes = PeerBase64URL.decode(record.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            throw PeerConnectError.invalidRecord
        }
        return try PeerRoom(recordBytes: bytes)
    }

    init(relay: String, role: PeerRole, secret: [UInt8], identity: [UInt8], peer: [UInt8]?) {
        self.relay = relay
        self.role = role
        self.secret = secret
        self.identity = identity
        self.inviteSeed = PeerRoom.inviteSeed(secret)
        self.psk = PeerConnectCrypto.hmac(key: secret, Array("peerconnect/v1 psk".utf8))
        if let peer { setPeer(peer) }
    }

    static func inviteSeed(_ secret: [UInt8]) -> [UInt8] {
        PeerConnectCrypto.hmac(key: secret, Array("peerconnect/v1 room".utf8))
    }

    // MARK: Reading

    /// The record to persist in order to reconnect later (base64url of
    /// `version role flags secret[32] identity[32] [peer[32]] relay`), the
    /// same text as the reference's `Room::to_string` / JS `toString()`.
    var record: String { PeerBase64URL.encode(recordBytes) }

    var description: String {
        "PeerRoom(id: \(id), role: \(role.name), relay: \(relay), paired: \(paired), closing: \(closing))"
    }

    /// The invite for this room, while the inviter has not yet paired.
    var invite: String? {
        guard role == .inviter, !paired else { return nil }
        return PeerInvite(relay: relay, secret: secret, inviter: publicKey).description
    }

    /// The room's ID on the relay: the pair ID once paired, the invite ID before.
    var id: String { roomId(at: paired ? .pair : .invite).description }

    /// This peer's identity public key.
    var publicKey: [UInt8] { PeerConnectCrypto.x25519Public(identity) }

    /// The other peer's identity public key: known to a joiner from the
    /// start, and to an inviter once paired.
    var peerKey: [UInt8]? { peer }

    /// The WebSocket URL of `id` on the room's relay.
    var endpoint: String { endpoint(at: paired ? .pair : .invite) }

    func endpoint(at: PeerRendezvous) -> String {
        "\(relay)/v1/\(roomId(at: at))"
    }

    func roomId(at: PeerRendezvous) -> PeerRoomId {
        PeerRoomId(bytes: PeerConnectCrypto.ed25519Public(seed: signer(at)))
    }

    /// The JOIN frame answering a relay's HELLO nonce at `at`.
    func joinFrame(at: PeerRendezvous, nonce: [UInt8]) -> [UInt8] {
        let signature = PeerConnectCrypto.ed25519Sign(
            seed: signer(at), PeerWire.joinMessage(nonce: nonce, role: role))
        return [PeerWire.join, role.rawValue] + signature
    }

    private func signer(_ at: PeerRendezvous) -> [UInt8] {
        switch at {
        case .invite: return inviteSeed
        // Only a joiner, or a paired inviter, ever goes to the pair ID.
        case .pair: return pairSeed ?? inviteSeed
        }
    }

    // MARK: Changing

    /// Records that pairing is complete; an inviter learns the joiner's key here.
    mutating func setPaired(_ peer: [UInt8]) {
        setPeer(peer)
        paired = true
    }

    mutating func setClosing() {
        closing = true
    }

    private mutating func setPeer(_ peer: [UInt8]) {
        // Both peers compute the same secret from their identities; anyone
        // else, invite or not, cannot.
        let shared = (try? PeerConnectCrypto.dh(identity, peer)) ?? []
        pairSeed = PeerConnectCrypto.hmac(key: secret, Array("peerconnect/v1 pair".utf8), shared)
        self.peer = peer
    }

    // MARK: Record

    var recordBytes: [UInt8] {
        var flags: UInt8 = 0
        if peer != nil { flags |= PeerRoom.hasPeerFlag }
        if paired { flags |= PeerRoom.pairedFlag }
        if closing { flags |= PeerRoom.closingFlag }
        var out: [UInt8] = [PeerRoom.recordVersion, role.rawValue, flags]
        out += secret
        out += identity
        if let peer { out += peer }
        out += Array(relay.utf8)
        return out
    }

    init(recordBytes bytes: [UInt8]) throws {
        let bad = PeerConnectError.invalidRecord
        let known = PeerRoom.hasPeerFlag | PeerRoom.pairedFlag | PeerRoom.closingFlag
        guard bytes.count >= 67, bytes[0] == PeerRoom.recordVersion, bytes[2] & ~known == 0,
              let role = PeerRole(rawValue: bytes[1])
        else { throw bad }
        let flags = bytes[2]
        var peer: [UInt8]?
        var rest = 67
        if flags & PeerRoom.hasPeerFlag != 0 {
            guard bytes.count >= 99 else { throw bad }
            peer = Array(bytes[67..<99])
            rest = 99
        }
        // A joiner always knows the inviter; an inviter knows the joiner exactly when paired.
        let consistent = role == .joiner ? peer != nil : (peer != nil) == (flags & PeerRoom.pairedFlag != 0)
        guard consistent else { throw bad }
        if let peer, !PeerConnectCrypto.usable(peer) { throw bad }
        guard let relay = String(bytes: bytes[rest...], encoding: .utf8) else { throw bad }
        self.init(relay: try PeerRoom.relayURL(relay), role: role, secret: Array(bytes[3..<35]),
                  identity: Array(bytes[35..<67]), peer: peer)
        paired = flags & PeerRoom.pairedFlag != 0
        closing = flags & PeerRoom.closingFlag != 0
    }

    /// Validates a relay base URL and strips any trailing slash.
    static func relayURL(_ url: String) throws -> String {
        let rest: Substring
        if url.hasPrefix("wss://") {
            rest = url.dropFirst(6)
        } else if url.hasPrefix("ws://") {
            rest = url.dropFirst(5)
        } else {
            throw PeerConnectError.invalidRelayUrl
        }
        let host = rest.split(separator: "/", maxSplits: 1, omittingEmptySubsequences: false).first ?? ""
        let graphic = rest.utf8.allSatisfy { $0 >= 0x21 && $0 <= 0x7E && $0 != UInt8(ascii: "?") && $0 != UInt8(ascii: "#") }
        guard !host.isEmpty, graphic else { throw PeerConnectError.invalidRelayUrl }
        var out = Substring(url)
        while out.hasSuffix("/") { out = out.dropLast() }
        return String(out)
    }
}
