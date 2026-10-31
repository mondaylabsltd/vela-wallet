// PeerConnect v1 for Swift: the wire constants, the Noise IKpsk1 handshake,
// and the sans-I/O peer state machine (SPEC §4–§7). A port of the reference
// crate's wire.rs, noise.rs and peer.rs; see PeerConnectRoom.swift for the
// credit and license (PeerConnect is MIT OR Apache-2.0, used here under MIT).
//
// The machine owns no socket and no timer. A driver (PeerConnection) feeds
// it everything that happens to its one WebSocket, with a monotonic `now` in
// milliseconds, drains `pollOutput()` after every input, and calls
// `handleTimeout(now:)` once `pollTimeout()` is reached.

import CryptoKit
import Foundation

// MARK: - Wire

/// Frame layout, protocol constants and close codes.
nonisolated enum PeerWire {
    typealias CloseCode = UInt16

    /// Peer → peer: Noise handshake message 1 (joiner → inviter).
    static let initFrame: UInt8 = 0x01
    /// Peer → peer: Noise handshake message 2 (inviter → joiner).
    static let replyFrame: UInt8 = 0x02
    /// Peer → peer: Noise transport message.
    static let messageFrame: UInt8 = 0x03
    /// Relay → peer: `nonce[32]`, the challenge for JOIN. Always the first frame.
    static let hello: UInt8 = 0x80
    /// Peer → relay: `role[1] signature[64]`.
    static let join: UInt8 = 0x81
    /// Relay → peer: `present[1]`, whether a partner connection holds the other seat.
    static let presence: UInt8 = 0x82

    static let ping = "ping"
    static let pong = "pong"

    static let nonceLength = 32
    static let helloLength = 1 + nonceLength
    static let joinLength = 1 + 1 + 64
    static let presenceLength = 2

    /// Largest frame: a type byte plus the largest Noise message.
    static let maxFrameLength = 1 + PeerNoise.maxMessageLength
    /// Largest application message (it travels in as many frames as needed).
    static let maxMessageLength = 16 << 20
    /// Bytes a sender keeps in flight, sent but not acknowledged, at most.
    static let window = 4 << 20

    static let joinContext = Array("peerconnect/v1 join".utf8)

    /// Timers, in milliseconds.
    static let pingInterval: UInt64 = 25_000
    static let receiveTimeout: UInt64 = 60_000
    static let idleTimeout: UInt64 = 90_000
    static let joinTimeout: UInt64 = 10_000
    static let handshakeTimeout: UInt64 = 10_000

    /// WebSocket close codes used by the protocol (SPEC §4.7).
    enum Close {
        static let normal: CloseCode = 1000
        static let goingAway: CloseCode = 1001
        static let protocolError: CloseCode = 1002
        static let tooBig: CloseCode = 1009
        static let unauthorized: CloseCode = 4401
        static let token: CloseCode = 4403
        static let timeout: CloseCode = 4408
        static let replaced: CloseCode = 4409
        static let rateLimited: CloseCode = 4429
    }

    static func helloFrame(nonce: [UInt8]) -> [UInt8] { [hello] + nonce }

    static func presenceFrame(_ present: Bool) -> [UInt8] { [presence, present ? 1 : 0] }

    /// The message a peer signs to answer a HELLO.
    static func joinMessage(nonce: [UInt8], role: PeerRole) -> [UInt8] {
        joinContext + nonce + [role.rawValue]
    }

    static func isPeerFrame(_ frame: [UInt8]) -> Bool { (frame.first ?? 0xFF) < 0x80 }
}

// MARK: - Noise_IKpsk1_25519_ChaChaPoly_SHA256

/// `Noise_IKpsk1_25519_ChaChaPoly_SHA256`, the only handshake PeerConnect
/// uses, written out straight for exactly that pattern (Noise revision 34):
///
///     <- s
///     ...
///     -> e, es, s, ss, psk
///     <- e, ee, se
///
/// The joiner is the initiator, the inviter the responder.
nonisolated enum PeerNoise {
    static let protocolName = Array("Noise_IKpsk1_25519_ChaChaPoly_SHA256".utf8)
    static let prologue = Array("peerconnect/v1".utf8)

    static let keyLength = 32
    static let tagLength = 16
    /// Largest Noise message, handshake or transport.
    static let maxMessageLength = 65535
    /// Largest plaintext a single transport message can carry.
    static let maxPlaintextLength = maxMessageLength - tagLength
    /// Length of handshake messages 1 and 2 with an empty payload.
    static let initLength = keyLength + (keyLength + tagLength) + tagLength
    static let replyLength = keyLength + tagLength

    /// A handshake or transport message failed to authenticate or was malformed.
    struct Failure: Error {}

    /// The Noise HKDF: HMAC-SHA256 extract-then-expand producing `count` outputs.
    static func hkdf(_ ck: [UInt8], _ ikm: [UInt8], count: Int) -> [[UInt8]] {
        let prk = PeerConnectCrypto.hmac(key: ck, ikm)
        var out: [[UInt8]] = []
        var previous: [UInt8] = []
        for i in 0..<count {
            previous = PeerConnectCrypto.hmac(key: prk, previous, [UInt8(i + 1)])
            out.append(previous)
        }
        return out
    }

    static func dh(_ secret: [UInt8], _ publicKey: [UInt8]) throws -> [UInt8] {
        do { return try PeerConnectCrypto.dh(secret, publicKey) } catch { throw Failure() }
    }
}

nonisolated struct PeerNoiseCipherState {
    let key: SymmetricKey
    var n: UInt64 = 0

    init(key: [UInt8]) {
        self.key = SymmetricKey(data: key)
    }

    private func nonce() throws -> ChaChaPoly.Nonce {
        // 2^64-1 is reserved by Noise; a session never gets anywhere close.
        guard n != UInt64.max else { throw PeerNoise.Failure() }
        var bytes = [UInt8](repeating: 0, count: 12)
        for i in 0..<8 { bytes[4 + i] = UInt8(truncatingIfNeeded: n >> (8 * UInt64(i))) }
        return try ChaChaPoly.Nonce(data: bytes)
    }

    mutating func encrypt(ad: [UInt8], _ plaintext: [UInt8]) throws -> [UInt8] {
        let box = try ChaChaPoly.seal(plaintext, using: key, nonce: nonce(), authenticating: ad)
        n += 1
        return Array(box.ciphertext) + Array(box.tag)
    }

    mutating func decrypt(ad: [UInt8], _ ciphertext: [UInt8]) throws -> [UInt8] {
        guard ciphertext.count >= PeerNoise.tagLength else { throw PeerNoise.Failure() }
        let length = ciphertext.count - PeerNoise.tagLength
        do {
            let box = try ChaChaPoly.SealedBox(
                nonce: nonce(), ciphertext: ciphertext[..<length], tag: ciphertext[length...])
            let plaintext = try ChaChaPoly.open(box, using: key, authenticating: ad)
            n += 1
            return Array(plaintext)
        } catch {
            throw PeerNoise.Failure()
        }
    }
}

nonisolated struct PeerNoiseSymmetricState {
    var ck: [UInt8]
    var h: [UInt8]
    var cipher: PeerNoiseCipherState?

    init() {
        // The protocol name is longer than HASHLEN, so h = HASH(name).
        h = PeerConnectCrypto.sha256(PeerNoise.protocolName)
        ck = h
        mixHash(PeerNoise.prologue)
    }

    mutating func mixHash(_ data: [UInt8]) {
        h = PeerConnectCrypto.sha256(h, data)
    }

    mutating func mixKey(_ ikm: [UInt8]) {
        let out = PeerNoise.hkdf(ck, ikm, count: 2)
        ck = out[0]
        cipher = PeerNoiseCipherState(key: out[1])
    }

    mutating func mixKeyAndHash(_ ikm: [UInt8]) {
        let out = PeerNoise.hkdf(ck, ikm, count: 3)
        ck = out[0]
        mixHash(out[1])
        cipher = PeerNoiseCipherState(key: out[2])
    }

    mutating func encryptAndHash(_ plaintext: [UInt8]) throws -> [UInt8] {
        // In IKpsk1 a key is always established before the first encryption.
        guard var cipher else { throw PeerNoise.Failure() }
        let ciphertext = try cipher.encrypt(ad: h, plaintext)
        self.cipher = cipher
        mixHash(ciphertext)
        return ciphertext
    }

    mutating func decryptAndHash(_ ciphertext: [UInt8]) throws -> [UInt8] {
        guard var cipher else { throw PeerNoise.Failure() }
        let plaintext = try cipher.decrypt(ad: h, ciphertext)
        self.cipher = cipher
        mixHash(ciphertext)
        return plaintext
    }

    /// (initiator → responder, responder → initiator) cipher states.
    func split() -> (PeerNoiseCipherState, PeerNoiseCipherState) {
        let out = PeerNoise.hkdf(ck, [], count: 2)
        return (PeerNoiseCipherState(key: out[0]), PeerNoiseCipherState(key: out[1]))
    }
}

/// Initiator side (the joiner), waiting for the responder's reply.
nonisolated struct PeerNoiseInitiator {
    private let state: PeerNoiseSymmetricState
    private let e: [UInt8]
    private let s: [UInt8]

    /// Starts a handshake towards `rs`, returning the state and message 1,
    /// which carries `payload`. `e` is the ephemeral private key.
    static func start(s: [UInt8], rs: [UInt8], psk: [UInt8], e: [UInt8], payload: [UInt8]) throws
        -> (PeerNoiseInitiator, [UInt8])
    {
        var st = PeerNoiseSymmetricState()
        st.mixHash(rs)
        let ePub = PeerConnectCrypto.x25519Public(e)
        var msg = ePub
        st.mixHash(ePub)
        st.mixKey(ePub)
        st.mixKey(try PeerNoise.dh(e, rs))
        msg += try st.encryptAndHash(PeerConnectCrypto.x25519Public(s))
        st.mixKey(try PeerNoise.dh(s, rs))
        st.mixKeyAndHash(psk)
        msg += try st.encryptAndHash(payload)
        return (PeerNoiseInitiator(state: st, e: e, s: s), msg)
    }

    private init(state: PeerNoiseSymmetricState, e: [UInt8], s: [UInt8]) {
        self.state = state
        self.e = e
        self.s = s
    }

    /// Processes the responder's reply, returning the session and the
    /// reply's payload. On failure the state is untouched, so a stray or
    /// forged reply does not spoil a handshake in progress.
    func finish(_ msg: [UInt8]) throws -> (PeerNoiseTransport, [UInt8]) {
        guard msg.count >= PeerNoise.replyLength else { throw PeerNoise.Failure() }
        var st = state
        let re = Array(msg[..<PeerNoise.keyLength])
        st.mixHash(re)
        st.mixKey(re)
        st.mixKey(try PeerNoise.dh(e, re))
        st.mixKey(try PeerNoise.dh(s, re))
        let payload = try st.decryptAndHash(Array(msg[PeerNoise.keyLength...]))
        let (send, recv) = st.split()
        return (PeerNoiseTransport(send: send, recv: recv), payload)
    }
}

/// Responder side (the inviter), having read the initiator's message 1.
nonisolated struct PeerNoiseResponder {
    private let state: PeerNoiseSymmetricState
    private let re: [UInt8]
    /// The initiator's static public key, authenticated by the handshake.
    let peer: [UInt8]
    /// The payload of message 1.
    let payload: [UInt8]

    static func read(s: [UInt8], psk: [UInt8], msg: [UInt8]) throws -> PeerNoiseResponder {
        guard msg.count >= PeerNoise.initLength else { throw PeerNoise.Failure() }
        var st = PeerNoiseSymmetricState()
        st.mixHash(PeerConnectCrypto.x25519Public(s))
        let k = PeerNoise.keyLength
        let re = Array(msg[..<k])
        st.mixHash(re)
        st.mixKey(re)
        st.mixKey(try PeerNoise.dh(s, re))
        let rs = try st.decryptAndHash(Array(msg[k..<(k * 2 + PeerNoise.tagLength)]))
        st.mixKey(try PeerNoise.dh(s, rs))
        st.mixKeyAndHash(psk)
        let payload = try st.decryptAndHash(Array(msg[(k * 2 + PeerNoise.tagLength)...]))
        return PeerNoiseResponder(state: st, re: re, peer: rs, payload: payload)
    }

    private init(state: PeerNoiseSymmetricState, re: [UInt8], peer: [UInt8], payload: [UInt8]) {
        self.state = state
        self.re = re
        self.peer = peer
        self.payload = payload
    }

    /// Writes message 2, which carries `payload`, completing the handshake.
    func reply(e: [UInt8], payload: [UInt8]) throws -> ([UInt8], PeerNoiseTransport) {
        var st = state
        let ePub = PeerConnectCrypto.x25519Public(e)
        var reply = ePub
        st.mixHash(ePub)
        st.mixKey(ePub)
        st.mixKey(try PeerNoise.dh(e, re))
        st.mixKey(try PeerNoise.dh(e, peer))
        reply += try st.encryptAndHash(payload)
        let (recv, send) = st.split()
        return (reply, PeerNoiseTransport(send: send, recv: recv))
    }
}

/// The pair of cipher states a completed handshake produced.
nonisolated struct PeerNoiseTransport {
    var send: PeerNoiseCipherState
    var recv: PeerNoiseCipherState

    /// Encrypts at most `PeerNoise.maxPlaintextLength` bytes.
    mutating func encrypt(_ plaintext: [UInt8]) throws -> [UInt8] {
        guard plaintext.count <= PeerNoise.maxPlaintextLength else { throw PeerNoise.Failure() }
        return try send.encrypt(ad: [], plaintext)
    }

    mutating func decrypt(_ ciphertext: [UInt8]) throws -> [UInt8] {
        guard ciphertext.count <= PeerNoise.maxMessageLength else { throw PeerNoise.Failure() }
        return try recv.decrypt(ad: [], ciphertext)
    }
}

// MARK: - Machine types

/// `offline`: not on the relay (connecting or waiting to retry);
/// `waiting`: on the relay, peer not (yet) connected;
/// `connected`: end-to-end session established;
/// `closed`: stopped for good, see the close reason.
nonisolated enum PeerConnectState: String, Equatable, Sendable {
    case offline
    case waiting
    case connected
    case closed
}

nonisolated enum PeerCloseReason: String, Equatable, Sendable {
    /// `leave()` was called. The room remains valid.
    case left
    /// `close()` was called and the peer has been told. Delete the room.
    case closed
    /// The peer ended the room. Delete it.
    case closedByPeer = "closed-by-peer"
}

nonisolated enum PeerSendError: Error, Equatable, Sendable {
    /// Over 16 MiB.
    case tooLarge
    /// 64 MiB already await acknowledgement; try again after a delivery.
    case full
    /// The room is closed or being closed.
    case closed
}

/// What the machine asks of its driver.
nonisolated enum PeerOutput: Equatable, Sendable {
    /// Open a WebSocket to this URL (any previous one is already closed).
    case connect(String)
    /// Send a binary message.
    case send(Data)
    /// Send the text message `ping`, ahead of anything queued if possible.
    case ping
    /// Close the WebSocket (code 1000) after flushing what was sent, then
    /// forget it: report nothing more about that socket.
    case disconnect
    /// The connection state changed (`reason` only with `.closed`).
    case state(PeerConnectState, PeerCloseReason?)
    /// A message from the peer, authenticated, decrypted and in order.
    case message(Data)
    /// The peer has received every message with an id below this one.
    case delivered(UInt64)
    /// Pairing is complete: the room now meets at its pair ID, which the
    /// invite does not reach. Persist the room again.
    case paired
    /// Another connection took this peer's seat (the same room open
    /// elsewhere). The peer waits a minute before taking it back; `leave()`
    /// lets the other instance keep it.
    case replaced
    /// The relay requires a token and refused ours (or we have none). The
    /// peer stays offline until `setToken`.
    case unauthorized
}

/// A FIFO with O(1) pops and random access by position.
nonisolated struct PeerDeque<Element> {
    private var items: [Element] = []
    private var head = 0

    var count: Int { items.count - head }
    var isEmpty: Bool { count == 0 }

    subscript(position: Int) -> Element { items[head + position] }

    mutating func append(_ element: Element) { items.append(element) }

    mutating func popFirst() -> Element? {
        guard head < items.count else { return nil }
        let element = items[head]
        head += 1
        if head == items.count {
            items.removeAll(keepingCapacity: true)
            head = 0
        } else if head >= 1024, head * 2 >= items.count {
            items.removeFirst(head)
            head = 0
        }
        return element
    }

    mutating func removeAll(where predicate: (Element) -> Bool) {
        items = Array(items[head...]).filter { !predicate($0) }
        head = 0
    }
}

// MARK: - The peer state machine

/// The peer state machine (SPEC §5, §6), free of I/O: connects, joins,
/// handshakes whenever a partner connection appears, and carries a reliable,
/// ordered stream of messages across any number of connections. It moves a
/// new room from its invite ID to its pair ID, keeps the connection alive,
/// and reconnects with backoff when anything fails. Times are milliseconds
/// on any monotonic clock.
nonisolated final class PeerConnectMachine {
    // Transport plaintext kinds (the first plaintext byte, SPEC §5.3).
    private enum Kind {
        static let final: UInt8 = 0x00
        static let more: UInt8 = 0x01
        static let close: UInt8 = 0x02
        static let ack: UInt8 = 0x03
    }

    static let chunkLength = PeerNoise.maxPlaintextLength - 1
    /// The handshake payload: `stream base seen_stream seen_next`, each a u64.
    static let resumeLength = 32
    /// Streams of the peer whose position a receiver remembers.
    static let rememberedStreams = 16
    static let connectTimeout: UInt64 = 15_000
    static let backoffBase: UInt64 = 500
    static let backoffMax: UInt64 = 30_000
    /// A connection that stayed joined this long counts as a success for backoff.
    static let stable: UInt64 = 30_000
    /// How long to wait before reclaiming a seat another connection took.
    static let replacedDelay: UInt64 = 60_000
    /// How long to wait at least after the relay said we connect too often.
    static let rateLimitedDelay: UInt64 = 60_000
    /// A joiner alone this long at the invite ID, or unpaired at the pair ID, tries the other.
    static let seekInterval: UInt64 = 20_000
    /// A paired joiner alone this long at the pair ID looks at the invite ID.
    static let pairedSeekInterval: UInt64 = 300_000
    /// After acknowledging the peer's CLOSE, how long to stay for it to see that.
    static let closeLinger: UInt64 = 30_000
    /// Once the closing peer has left, how long to stay in case it comes straight back.
    static let closeGrace: UInt64 = 5_000
    /// Bytes of messages a peer holds until they are acknowledged.
    static let outboxLimit = 64 << 20
    /// A receiver acknowledges after this many messages, this many bytes, or
    /// this long after an unacknowledged delivery, whichever comes first.
    static let ackMessages: UInt32 = 32
    static let ackBytes = 1 << 20
    static let ackDelay: UInt64 = 50
    /// A receiver in the middle of a message acknowledges at least this often.
    static let progressAck: UInt64 = 10_000
    /// A sender with data in flight that hears no acknowledgement this long reconnects.
    static let ackTimeout: UInt64 = 30_000

    /// The state each side announces in its handshake message.
    struct Resume: Equatable {
        var stream: UInt64
        var base: UInt64
        var seenStream: UInt64
        var seenNext: UInt64

        func encode() -> [UInt8] {
            [stream, base, seenStream, seenNext].flatMap(PeerConnectCrypto.u64be)
        }

        /// Reads the first 32 bytes; anything after is for future revisions.
        static func decode(_ payload: [UInt8]) -> Resume? {
            guard let a = PeerConnectCrypto.readU64be(payload, at: 0),
                  let b = PeerConnectCrypto.readU64be(payload, at: 8),
                  let c = PeerConnectCrypto.readU64be(payload, at: 16),
                  let d = PeerConnectCrypto.readU64be(payload, at: 24)
            else { return nil }
            return Resume(stream: a, base: b, seenStream: c, seenNext: d)
        }
    }

    /// Messages we have sent, kept until the peer acknowledges them.
    private struct Outbox {
        /// Our stream's id, fresh for every machine.
        var stream: UInt64
        /// The id of `messages[0]`; every lower id has been acknowledged.
        var base: UInt64 = 0
        var messages = PeerDeque<(kind: UInt8, data: [UInt8])>()
        var bytes = 0
        /// The id of our CLOSE, once queued.
        var close: UInt64?

        var nextId: UInt64 { base + UInt64(messages.count) }
    }

    /// Where we are in the peer's streams.
    private struct Inbound {
        /// For the streams that most recently delivered something (last =
        /// latest): the id of the next message to deliver.
        var streams: [(stream: UInt64, next: UInt64)] = []
        /// In the current session: the peer's stream, the id of the next
        /// message to arrive, and of the next one to deliver.
        var current: UInt64 = 0
        var arriving: UInt64 = 0
        var next: UInt64 = 0
        /// Delivered since we last acknowledged.
        var unacked: UInt32 = 0
        var unackedBytes = 0
        var ackAt: UInt64?
        /// When we last sent an acknowledgement.
        var ackedAt: UInt64 = 0

        func position(_ stream: UInt64) -> UInt64? {
            streams.first { $0.stream == stream }?.next
        }

        /// Records the position in `stream`, making it the most recent.
        mutating func setPosition(_ stream: UInt64, _ next: UInt64) {
            streams.removeAll { $0.stream == stream }
            streams.append((stream, next))
            if streams.count > PeerConnectMachine.rememberedStreams { streams.removeFirst() }
        }

        /// What to announce in a handshake: our position in `stream`, or in
        /// the latest stream if the peer's is not known yet; (0, 0) for none.
        func seen(_ stream: UInt64?) -> (UInt64, UInt64) {
            if let stream {
                return position(stream).map { (stream, $0) } ?? (0, 0)
            }
            return streams.last.map { ($0.stream, $0.next) } ?? (0, 0)
        }
    }

    private final class Established {
        var transport: PeerNoiseTransport
        /// A message being reassembled.
        var partial: [UInt8] = []
        /// The id of the next message to transmit in this session.
        var sent: UInt64
        /// Bytes transmitted in this session and not yet acknowledged.
        var inFlight = 0
        /// When the peer last acknowledged anything (or the session began).
        var heardAt: UInt64

        init(transport: PeerNoiseTransport, sent: UInt64, heardAt: UInt64) {
            self.transport = transport
            self.sent = sent
            self.heardAt = heardAt
        }
    }

    private enum Session {
        /// No partner connection in the other seat, since then.
        case absent(since: UInt64)
        /// Inviter, waiting for the joiner's handshake.
        case awaitingInit
        /// Joiner, waiting for the inviter's reply to the handshake it sent.
        case awaitingReply(PeerNoiseInitiator, sent: Resume, deadline: UInt64)
        case established(Established)
    }

    private enum Phase {
        /// Waiting for HELLO, then (after sending JOIN) for PRESENCE.
        case joining(deadline: UInt64, joined: Bool)
        case joined(Session)
    }

    private final class Open {
        let at: PeerRendezvous
        var phase: Phase
        /// When the relay accepted our JOIN.
        var joinedAt: UInt64?
        /// Reconnect if nothing arrives by then.
        var receiveDeadline: UInt64
        var nextPing: UInt64

        init(at: PeerRendezvous, now: UInt64) {
            self.at = at
            phase = .joining(deadline: now + PeerWire.joinTimeout, joined: false)
            receiveDeadline = now + PeerWire.receiveTimeout
            nextPing = now + PeerWire.pingInterval
        }
    }

    private enum Link {
        /// No socket; connect at `retryAt`.
        case idle(retryAt: UInt64)
        /// Waiting for the socket to open.
        case connecting(at: PeerRendezvous, deadline: UInt64)
        case open(Open)
        /// The relay refused our token: wait for another.
        case unauthorized
        case done
    }

    // MARK: State

    private(set) var room: PeerRoom
    /// The relay token, appended to every endpoint (SPEC §4.1).
    private var token: String?
    private var link: Link
    /// The time of the latest input; `send` has none of its own.
    private var clock: UInt64
    private(set) var state: PeerConnectState = .offline
    private(set) var reason: PeerCloseReason?
    private var output = PeerDeque<PeerOutput>()
    /// Connection attempts since the last stable connection.
    private var failures: UInt32 = 0
    /// Where to connect next.
    private var nextAt: PeerRendezvous
    /// Inviter, until pairing completes: the joiner it is pairing with.
    private var tentative: [UInt8]?
    /// The peer ended the room and we acknowledged; stop when it leaves, or then.
    private var endedByPeer: UInt64?
    private var out: Outbox
    private var inbound = Inbound()

    /// Starts the machine; the first output asks the driver to connect. A
    /// room that is closing resumes telling the peer.
    init(room: PeerRoom, token: String? = nil, now: UInt64) {
        var stream: UInt64 = 0
        while stream == 0 { stream = PeerConnectCrypto.randomU64() }
        self.room = room
        self.token = token.flatMap { $0.isEmpty ? nil : $0 }
        link = .idle(retryAt: now)
        clock = now
        nextAt = room.paired ? .pair : .invite
        out = Outbox(stream: stream)
        if room.closing { queueClose() }
        connect(now)
    }

    /// For a relay that requires a token the application is still fetching:
    /// the peer stays offline until `setToken`.
    static func awaitingToken(room: PeerRoom, now: UInt64) -> PeerConnectMachine {
        let machine = PeerConnectMachine(room: room, token: nil, now: now)
        machine.output.removeAll { if case .connect = $0 { return true } else { return false } }
        machine.link = .unauthorized
        return machine
    }

    /// Replaces the relay token. Used from the next connection on; a peer
    /// waiting after `.unauthorized` connects again at once.
    func setToken(now: UInt64, _ token: String?) {
        clock = now
        self.token = token.flatMap { $0.isEmpty ? nil : $0 }
        if case .unauthorized = link {
            failures = 0
            connect(now)
        }
    }

    /// Bytes of messages waiting to be acknowledged by the peer.
    var queued: Int { out.bytes }

    /// The id the next queued message will get.
    var nextMessageId: UInt64 { out.nextId }

    func pollOutput() -> PeerOutput? {
        output.popFirst()
    }

    /// When the driver must next call `handleTimeout(now:)`.
    func pollTimeout() -> UInt64? {
        var linkTime: UInt64?
        switch link {
        case .idle(let retryAt):
            linkTime = retryAt
        case .connecting(_, let deadline):
            linkTime = deadline
        case .open(let open):
            var phase: UInt64?
            switch open.phase {
            case .joining(let deadline, _):
                phase = deadline
            case .joined(.awaitingReply(_, _, let deadline)):
                phase = deadline
            case .joined(.absent(let since)):
                phase = seekAfter(open.at).map { since + $0 }
            case .joined(.established(let est)):
                let silent = est.inFlight > 0 ? est.heardAt + Self.ackTimeout : nil
                phase = [inbound.ackAt, silent].compactMap { $0 }.min()
            case .joined(.awaitingInit):
                phase = nil
            }
            linkTime = [open.receiveDeadline, open.nextPing, phase].compactMap { $0 }.min()
        case .unauthorized:
            linkTime = nil
        case .done:
            return nil
        }
        return [linkTime, endedByPeer].compactMap { $0 }.min()
    }

    func handleTimeout(now: UInt64) {
        clock = now
        if let at = endedByPeer, now >= at {
            return finish(.closedByPeer)
        }
        var seek = false
        if case .open(let open) = link, case .joined(.absent(let since)) = open.phase {
            seek = seekAfter(open.at).map { now >= since + $0 } ?? false
        }
        switch link {
        case .idle(let retryAt) where now >= retryAt:
            connect(now)
        case .connecting(_, let deadline) where now >= deadline:
            reconnect(now)
        case .open(let open):
            let expired: Bool
            switch open.phase {
            case .joining(let deadline, _):
                expired = now >= deadline
            case .joined(.awaitingReply(_, _, let deadline)):
                expired = now >= deadline
            case .joined(.established(let est)):
                // Data in flight and no acknowledgement for long: the session
                // is broken. Reconnecting resumes the stream.
                expired = est.inFlight > 0 && now >= est.heardAt + Self.ackTimeout
            case .joined:
                expired = false
            }
            if expired || now >= open.receiveDeadline {
                reconnect(now)
            } else if seek {
                // Nobody here: try the other room ID straight away.
                let at = open.at
                dropSocket(now, close: true)
                nextAt = at.other
                connect(now)
            } else {
                if now >= open.nextPing {
                    open.nextPing = now + PeerWire.pingInterval
                    output.append(.ping)
                }
                if let ackAt = inbound.ackAt, now >= ackAt {
                    sendAck(now)
                }
            }
        default:
            break
        }
    }

    /// The socket requested by `.connect` is open.
    func handleOpen(now: UInt64) {
        clock = now
        if case .connecting(let at, _) = link {
            link = .open(Open(at: at, now: now))
        }
    }

    /// The socket closed or failed, with the close code if one was received.
    func handleClose(now: UInt64, code: UInt16?) {
        clock = now
        let tried: PeerRendezvous
        switch link {
        case .connecting(let at, _): tried = at
        case .open(let open): tried = open.at
        default: return
        }
        dropSocket(now, close: false)
        if code == PeerWire.Close.token || code == PeerWire.Close.rateLimited {
            // The relay never let us look: a joiner tries the same ID again.
            nextAt = tried
        }
        if code == PeerWire.Close.replaced {
            output.append(.replaced)
            let delay = Self.replacedDelay
            retryAt(now + delay + Self.randomBelow(delay / 6))
        } else if code == PeerWire.Close.token {
            // Never again with the same token (SPEC §4.7).
            link = .unauthorized
            setState(.offline)
            output.append(.unauthorized)
        } else if code == PeerWire.Close.rateLimited {
            let delay = max(backoff(), Self.rateLimitedDelay)
            retryAt(now + delay + Self.randomBelow(delay / 6))
        } else {
            let delay = backoff()
            retryAt(now + delay)
        }
    }

    func handleText(now: UInt64, _ text: String) {
        clock = now
        // `pong` is the only text a relay sends; all that matters is that it arrived.
        if case .open(let open) = link {
            open.receiveDeadline = now + PeerWire.receiveTimeout
        }
    }

    func handleBinary(now: UInt64, _ frame: [UInt8]) {
        clock = now
        guard case .open(let open) = link else { return }
        open.receiveDeadline = now + PeerWire.receiveTimeout
        guard let kind = frame.first else { return }
        let body = Array(frame.dropFirst())

        switch open.phase {
        case .joining(_, false) where kind == PeerWire.hello:
            guard body.count == PeerWire.nonceLength else { return reconnect(now) }
            output.append(.send(Data(room.joinFrame(at: open.at, nonce: body))))
            open.phase = .joining(deadline: now + PeerWire.joinTimeout, joined: true)
        case .joining(_, true) where kind == PeerWire.presence:
            open.phase = .joined(.absent(since: now))
            open.joinedAt = now
            setState(.waiting)
            presence(now, body == [1])
        case .joining:
            reconnect(now)
        case .joined:
            switch kind {
            case PeerWire.presence: presence(now, body == [1])
            case PeerWire.initFrame: handleInit(now, body)
            case PeerWire.replyFrame: handleReply(now, body)
            case PeerWire.messageFrame: handleMessage(now, body)
            // Anything else is from a newer protocol revision or stale: ignore.
            default: break
            }
        }
    }

    /// Queues a message for the peer and returns its id. It is sent as soon
    /// as there is a session, kept until the peer acknowledges it, and resent
    /// after a reconnection if need be; `.delivered` reports progress.
    @discardableResult
    func send(_ data: [UInt8]) throws -> UInt64 {
        if room.closing || endedByPeer != nil || state == .closed { throw PeerSendError.closed }
        if data.count > PeerWire.maxMessageLength { throw PeerSendError.tooLarge }
        if out.bytes + data.count > Self.outboxLimit { throw PeerSendError.full }
        let id = push(Kind.final, data)
        pump()
        return id
    }

    /// Disconnects and stops, keeping the room valid for later. Messages not
    /// yet acknowledged are dropped.
    func leave() {
        finish(.left)
    }

    /// Ends the room: queued messages go first, then the peer is told, and
    /// once it has acknowledged that the state becomes closed (`.closed`).
    /// The machine keeps reconnecting until then; the room record now says
    /// it is closing (persist it), so a peer restarted from it carries on.
    func close() {
        if !room.closing && state != .closed {
            room.setClosing()
            queueClose()
        }
    }

    // MARK: Internals

    private func queueClose() {
        out.close = push(Kind.close, [])
        pump()
    }

    private func push(_ kind: UInt8, _ data: [UInt8]) -> UInt64 {
        let id = out.nextId
        out.bytes += data.count
        out.messages.append((kind, data))
        return id
    }

    /// How long a joiner waits alone at `at` before trying the other ID.
    private func seekAfter(_ at: PeerRendezvous) -> UInt64? {
        guard room.role == .joiner else { return nil }
        return at == .pair && room.paired ? Self.pairedSeekInterval : Self.seekInterval
    }

    private func connect(_ now: UInt64) {
        let at: PeerRendezvous
        switch room.role {
        case .inviter: at = room.paired ? .pair : .invite
        case .joiner: at = nextAt
        }
        link = .connecting(at: at, deadline: now + Self.connectTimeout)
        var url = room.endpoint(at: at)
        if let token { url += "?token=\(token)" }
        output.append(.connect(url))
    }

    /// Abandons the current socket and schedules another attempt.
    private func reconnect(_ now: UInt64) {
        dropSocket(now, close: true)
        let delay = backoff()
        retryAt(now + delay)
    }

    /// Forgets the current socket, asking the driver to close it if it is
    /// still open. A connection that stayed joined long enough proves the
    /// path works, so the backoff starts over. Decides where a joiner goes
    /// next: an unpaired one alternates, a paired one goes home.
    private func dropSocket(_ now: UInt64, close: Bool) {
        let old = link
        link = .idle(retryAt: now)
        var at: PeerRendezvous?
        switch old {
        case .connecting(let a, _): at = a
        case .open(let open):
            at = open.at
            if let joined = open.joinedAt, now >= joined, now - joined >= Self.stable { failures = 0 }
        default: at = nil
        }
        if close, at != nil { output.append(.disconnect) }
        if let at { nextAt = room.paired ? .pair : at.other }
        inbound.ackAt = nil
    }

    /// The delay before the next attempt: exponential with jitter, between
    /// half and all of `min(30 s, 0.5 s × 2^failures)`.
    private func backoff() -> UInt64 {
        let ceiling = min(Self.backoffBase << UInt64(min(failures, 16)), Self.backoffMax)
        failures += 1
        return ceiling / 2 + Self.randomBelow(ceiling / 2 + 1)
    }

    private func retryAt(_ at: UInt64) {
        link = .idle(retryAt: at)
        setState(.offline)
    }

    private func finish(_ reason: PeerCloseReason) {
        switch link {
        case .done: return
        case .connecting, .open: output.append(.disconnect)
        default: break
        }
        link = .done
        setState(.closed, reason)
    }

    private func setState(_ state: PeerConnectState, _ reason: PeerCloseReason? = nil) {
        if self.state != state || self.reason != reason {
            self.state = state
            self.reason = reason
            output.append(.state(state, reason))
        }
    }

    private var session: Session? {
        if case .open(let open) = link, case .joined(let session) = open.phase { return session }
        return nil
    }

    private func setSession(_ session: Session) {
        if case .open(let open) = link, case .joined = open.phase { open.phase = .joined(session) }
    }

    private var established: Established? {
        if case .established(let est)? = session { return est }
        return nil
    }

    /// Our resume state for a handshake with the peer's stream `theirs`, if known.
    private func resume(_ theirs: UInt64?) -> Resume {
        let (seenStream, seenNext) = inbound.seen(theirs)
        return Resume(stream: out.stream, base: out.base, seenStream: seenStream, seenNext: seenNext)
    }

    /// A partner connection arrived (`present`) or left. Either way any
    /// session with the previous one is over; a new arrival gets a fresh
    /// handshake, which the joiner starts.
    private func presence(_ now: UInt64, _ present: Bool) {
        if !present, let until = endedByPeer {
            // The peer has most likely seen our acknowledgement of its CLOSE
            // and gone. Stay a few seconds more in case it dropped before that.
            endedByPeer = min(until, now + Self.closeGrace)
        }
        var atInviteInSession = false
        if case .open(let open) = link, open.at == .invite, case .joined(.established) = open.phase {
            atInviteInSession = true
        }
        if !present && atInviteInSession && room.role == .joiner {
            // An inviter that leaves a working session at the invite ID has
            // most likely just completed pairing and moved: follow it.
            dropSocket(now, close: true)
            nextAt = .pair
            return connect(now)
        }
        let next: Session
        switch (present, room.role) {
        case (false, _):
            next = .absent(since: now)
        case (true, .inviter):
            next = .awaitingInit
        case (true, .joiner):
            guard let inviter = room.peer else { return reconnect(now) }
            let sent = resume(nil)
            guard let started = try? PeerNoiseInitiator.start(
                s: room.identity, rs: inviter, psk: room.psk, e: PeerConnectCrypto.random(32),
                payload: sent.encode())
            else { return reconnect(now) }
            let (initiator, msg) = started
            output.append(.send(Data([PeerWire.initFrame] + msg)))
            next = .awaitingReply(initiator, sent: sent, deadline: now + PeerWire.handshakeTimeout)
        }
        setSession(next)
        inbound.ackAt = nil
        setState(.waiting)
    }

    private func handleInit(_ now: UInt64, _ body: [UInt8]) {
        guard room.role == .inviter else { return }
        switch session {
        case .awaitingInit?, .established?: break
        default: return
        }
        guard let responder = try? PeerNoiseResponder.read(s: room.identity, psk: room.psk, msg: body),
              let theirs = Resume.decode(responder.payload)
        else { return }
        // Only the joiner this inviter is pairing, or has paired, with may connect.
        let known = room.paired ? room.peer : tentative
        if let known {
            if known != responder.peer { return }
        } else {
            tentative = responder.peer
        }
        // Our position in the joiner's stream: we know which one it is now.
        let ours = resume(theirs.stream)
        guard let replied = try? responder.reply(e: PeerConnectCrypto.random(32), payload: ours.encode())
        else { return }
        let (reply, transport) = replied
        output.append(.send(Data([PeerWire.replyFrame] + reply)))
        establish(now, transport, ours: ours, theirs: theirs)
    }

    private func handleReply(_ now: UInt64, _ body: [UInt8]) {
        guard case .awaitingReply(let initiator, let sent, _)? = session else { return }
        // A reply that does not verify is stale or forged; keep waiting.
        guard let finished = try? initiator.finish(body),
              let theirs = Resume.decode(finished.1)
        else { return }
        let transport = finished.0
        establish(now, transport, ours: sent, theirs: theirs)
    }

    /// Starts a session and picks both streams up where they were left, by
    /// the rules both sides apply to the same two payloads.
    private func establish(_ now: UInt64, _ transport: PeerNoiseTransport, ours: Resume, theirs: Resume) {
        guard case .open(let open) = link else { return }
        let at = open.at
        // Outgoing: resend from the first message the peer has not seen;
        // everything before it is acknowledged.
        let from = theirs.seenStream == out.stream
            ? min(max(theirs.seenNext, out.base), out.nextId)
            : out.base
        // Incoming: the peer resends from the point it computes the same way;
        // what arrives below our delivery position for its stream is a duplicate.
        let arriving = ours.seenStream == theirs.stream ? max(ours.seenNext, theirs.base) : theirs.base
        // (The remembered position is updated only once a message from this
        // stream arrives: a replayed handshake never gets that far.)
        inbound.next = inbound.position(theirs.stream).map { max($0, theirs.base) } ?? theirs.base
        inbound.current = theirs.stream
        inbound.arriving = arriving
        inbound.unacked = 0
        inbound.unackedBytes = 0
        inbound.ackAt = nil
        inbound.ackedAt = now

        acknowledged(from)
        if case .done = link { return } // that acknowledged our CLOSE: the room is over
        setSession(.established(Established(transport: transport, sent: from, heardAt: now)))
        setState(.connected)
        if room.role == .joiner {
            if at == .pair && !room.paired, let inviter = room.peer {
                room.setPaired(inviter)
                output.append(.paired)
            }
            // Tells the inviter at once that the session works, and so that
            // this joiner holds the identity it is pinning.
            sendAck(now)
        }
        pump()
    }

    /// The peer has everything below `next`.
    private func acknowledged(_ next: UInt64) {
        let next = min(next, out.nextId)
        guard next > out.base else { return }
        let sent = established?.sent
        var freedInFlight = 0
        while out.base < next {
            guard let message = out.messages.popFirst() else { break }
            out.bytes -= message.data.count
            if let sent, out.base < sent { freedInFlight += message.data.count }
            out.base += 1
        }
        if let est = established {
            est.inFlight = max(0, est.inFlight - freedInFlight)
            // The peer may acknowledge what this session has not sent yet
            // (it had it already): nothing below `next` needs sending.
            est.sent = max(est.sent, next)
        }
        output.append(.delivered(next))
        if let close = out.close, close < next {
            finish(.closed)
        }
    }

    /// Transmits queued messages while the window allows.
    private func pump() {
        guard let est = established else { return }
        while est.inFlight < PeerWire.window {
            let index = Int(est.sent - out.base)
            guard index < out.messages.count else { break }
            let message = out.messages[index]
            if est.inFlight == 0 { est.heardAt = clock } // the wait for an acknowledgement starts now
            if message.kind == Kind.close {
                output.append(.send(seal(est, Kind.close, [])))
            } else {
                let data = message.data
                var start = 0
                repeat {
                    let end = min(start + Self.chunkLength, data.count)
                    let more = end < data.count
                    output.append(.send(seal(est, more ? Kind.more : Kind.final, Array(data[start..<end]))))
                    start = end
                } while start < data.count
            }
            est.inFlight += message.data.count
            est.sent += 1
        }
    }

    private func sendAck(_ now: UInt64) {
        guard let est = established else { return }
        output.append(.send(seal(est, Kind.ack, PeerConnectCrypto.u64be(inbound.next))))
        inbound.unacked = 0
        inbound.unackedBytes = 0
        inbound.ackAt = nil
        inbound.ackedAt = now
    }

    private func handleMessage(_ now: UInt64, _ body: [UInt8]) {
        // Messages outside a session are left over from an earlier one.
        guard let est = established else { return }
        guard let plaintext = try? est.transport.decrypt(body) else {
            // Within a session the relay must deliver every message, in order,
            // untouched. Anything else breaks the session: start over.
            return reconnect(now)
        }
        guard let kind = plaintext.first else { return }
        switch kind {
        case Kind.more, Kind.final:
            if est.partial.count + plaintext.count - 1 > PeerWire.maxMessageLength {
                return reconnect(now)
            }
            if kind == Kind.more {
                est.partial += plaintext[1...]
                if now >= inbound.ackedAt + Self.progressAck {
                    sendAck(now) // still receiving: tell the sender
                }
            } else {
                let message: [UInt8]
                if est.partial.isEmpty {
                    message = Array(plaintext[1...])
                } else {
                    est.partial += plaintext[1...]
                    message = est.partial
                    est.partial = []
                }
                if arrived() { deliver(now, message) }
            }
        case Kind.close:
            if arrived() {
                // Acknowledge, then stay until the peer has seen that.
                sendAck(now)
                endedByPeer = now + Self.closeLinger
            }
            return
        case Kind.ack:
            if let next = PeerConnectCrypto.readU64be(plaintext, at: 1) {
                est.heardAt = now
                acknowledged(next)
                pump()
            }
        default:
            break
        }
        confirm(now)
    }

    /// Numbers the message (or CLOSE) that just arrived, and says whether it
    /// is new: one below our position in its stream is a duplicate.
    private func arrived() -> Bool {
        let id = inbound.arriving
        inbound.arriving += 1
        if id < inbound.next { return false }
        inbound.next = id + 1
        inbound.setPosition(inbound.current, id + 1)
        return true
    }

    private func deliver(_ now: UInt64, _ message: [UInt8]) {
        inbound.unacked += 1
        inbound.unackedBytes += message.count
        output.append(.message(Data(message)))
        if inbound.unacked >= Self.ackMessages || inbound.unackedBytes >= Self.ackBytes {
            sendAck(now)
        } else if inbound.ackAt == nil {
            inbound.ackAt = now + Self.ackDelay
        }
    }

    /// Inviter: an authenticated message from the joiner proves it holds the
    /// session, and so that it knows it was pinned. Pairing is complete: move
    /// to the pair ID at once, where the joiner follows. A room being closed
    /// has no use for pairing, and stays put until its CLOSE is acknowledged.
    private func confirm(_ now: UInt64) {
        guard room.role == .inviter, !room.paired, !room.closing, let joiner = tentative else { return }
        guard case .open = link else { return }
        room.setPaired(joiner)
        output.append(.paired)
        dropSocket(now, close: true)
        setState(.offline)
        connect(now)
    }

    private func seal(_ est: Established, _ kind: UInt8, _ data: [UInt8]) -> Data {
        // Chunks fit and nonces never run out, so this cannot fail.
        let ciphertext = (try? est.transport.encrypt([kind] + data)) ?? []
        return Data([PeerWire.messageFrame] + ciphertext)
    }

    private static func randomBelow(_ bound: UInt64) -> UInt64 {
        PeerConnectCrypto.randomU64() % max(bound, 1)
    }
}
