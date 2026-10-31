// The Swift PeerConnect peer against the protocol's conformance vectors
// (Fixtures/peerconnect-v1.json, PeerConnect's test-vectors/v1.json), the
// reference client's room records, and an in-memory relay: pairing and the
// move to the pair ID, messages both ways, large messages, resuming after
// dropped connections and restarts, replacement, closing, tampering.
//
// The simulation ports PeerConnect's own crates/peerconnect/tests/simulation.rs
// (MIT OR Apache-2.0, used here under MIT).

import CryptoKit
import Foundation
import XCTest

@testable import VelaWallet

@MainActor
final class PeerConnectTests: XCTestCase {
    // MARK: - Vectors

    private struct Vectors {
        let raw: [String: Any]

        func string(_ key: String) -> String { raw[key] as? String ?? "" }
        func bytes(_ key: String) -> [UInt8] { hexBytes(string(key)) }
        var messages: [[String: Any]] { raw["messages"] as? [[String: Any]] ?? [] }
    }

    private func vectors() throws -> Vectors {
        let url = Bundle(for: PeerConnectBundleToken.self).url(forResource: "peerconnect-v1", withExtension: "json")
            ?? URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .appendingPathComponent("Fixtures/peerconnect-v1.json")
        let data = try Data(contentsOf: url)
        let raw = try XCTUnwrap(try JSONSerialization.jsonObject(with: data) as? [String: Any])
        return Vectors(raw: raw)
    }

    /// A room record built from fixed parts, as the vectors' generator does.
    private func record(_ role: PeerRole, paired: Bool, secret: [UInt8], identity: [UInt8], peer: [UInt8]?,
                        relay: String) throws -> PeerRoom {
        var raw: [UInt8] = [2, role.rawValue, (peer == nil ? 0 : 1) | (paired ? 2 : 0)]
        raw += secret + identity + (peer ?? []) + Array(relay.utf8)
        return try PeerRoom(recordBytes: raw)
    }

    func testDerivedValuesMatchVectors() throws {
        let v = try vectors()
        let secret = v.bytes("room_secret")
        XCTAssertEqual(PeerRoom.inviteSeed(secret), v.bytes("invite_seed"))
        XCTAssertEqual(PeerConnectCrypto.x25519Public(v.bytes("inviter_identity_private")), v.bytes("inviter_identity_public"))
        XCTAssertEqual(PeerConnectCrypto.x25519Public(v.bytes("joiner_identity_private")), v.bytes("joiner_identity_public"))

        let unpaired = try record(.inviter, paired: false, secret: secret, identity: v.bytes("inviter_identity_private"),
                                  peer: nil, relay: v.string("relay_url"))
        XCTAssertEqual(unpaired.roomId(at: .invite).bytes, v.bytes("invite_id"))
        XCTAssertEqual(unpaired.id, v.string("invite_id_text"))
        XCTAssertEqual(unpaired.psk, v.bytes("psk"))
        XCTAssertEqual(unpaired.endpoint, v.string("invite_endpoint"))
        XCTAssertEqual(unpaired.invite, v.string("invite"))

        let shared = try PeerConnectCrypto.dh(v.bytes("joiner_identity_private"), v.bytes("inviter_identity_public"))
        XCTAssertEqual(shared, v.bytes("identity_dh"))
        XCTAssertEqual(try PeerConnectCrypto.dh(v.bytes("inviter_identity_private"), v.bytes("joiner_identity_public")), shared)
        XCTAssertEqual(PeerConnectCrypto.hmac(key: secret, Array("peerconnect/v1 pair".utf8), shared), v.bytes("pair_seed"))

        let inviter = try record(.inviter, paired: true, secret: secret, identity: v.bytes("inviter_identity_private"),
                                 peer: v.bytes("joiner_identity_public"), relay: v.string("relay_url"))
        let joiner = try record(.joiner, paired: true, secret: secret, identity: v.bytes("joiner_identity_private"),
                                peer: v.bytes("inviter_identity_public"), relay: v.string("relay_url"))
        XCTAssertEqual(inviter.roomId(at: .pair).bytes, v.bytes("pair_id"))
        XCTAssertEqual(inviter.id, v.string("pair_id_text"))
        XCTAssertEqual(joiner.id, v.string("pair_id_text"))
        XCTAssertEqual(inviter.roomId(at: .invite).description, v.string("invite_id_text"))
        XCTAssertNil(inviter.invite, "a paired inviter has no invite")
    }

    func testInviteFromVectorsParsesAndJoins() throws {
        let v = try vectors()
        let invite = try PeerInvite(v.string("invite"))
        XCTAssertEqual(invite.relay, v.string("relay_url"))
        XCTAssertEqual(invite.secret, v.bytes("room_secret"))
        XCTAssertEqual(invite.inviter, v.bytes("inviter_identity_public"))
        XCTAssertEqual(invite.roomId.description, v.string("invite_id_text"))
        XCTAssertEqual(invite.description, v.string("invite"))

        let joiner = try PeerRoom.join(invite: v.string("invite"))
        XCTAssertEqual(joiner.role, .joiner)
        XCTAssertEqual(joiner.id, v.string("invite_id_text"))
        XCTAssertEqual(joiner.peerKey, v.bytes("inviter_identity_public"))
        XCTAssertEqual(joiner.psk, v.bytes("psk"))
        XCTAssertNil(joiner.invite)
        XCTAssertFalse(joiner.paired)
    }

    func testHelloAndJoinFrames() throws {
        let v = try vectors()
        let nonce = v.bytes("hello_nonce")
        XCTAssertEqual(PeerWire.helloFrame(nonce: nonce), v.bytes("hello_frame"))
        let secret = v.bytes("room_secret")
        let inviter = try record(.inviter, paired: true, secret: secret, identity: v.bytes("inviter_identity_private"),
                                 peer: v.bytes("joiner_identity_public"), relay: v.string("relay_url"))
        let joiner = try record(.joiner, paired: true, secret: secret, identity: v.bytes("joiner_identity_private"),
                                peer: v.bytes("inviter_identity_public"), relay: v.string("relay_url"))
        let cases: [(PeerRoom, PeerRendezvous, String)] = [
            (inviter, .invite, "join_frame_invite_inviter"),
            (joiner, .invite, "join_frame_invite_joiner"),
            (inviter, .pair, "join_frame_pair_inviter"),
            (joiner, .pair, "join_frame_pair_joiner"),
        ]
        for (room, at, key) in cases {
            let id = room.roomId(at: at)
            // The reference's (deterministic) signature verifies under the key derived here…
            let reference = v.bytes(key)
            XCTAssertEqual(id.verifyJoin(nonce: nonce, frame: reference).role, room.role, key)
            // …and ours (CryptoKit hedges Ed25519, so the bytes differ) is the same frame shape and verifies too.
            let ours = room.joinFrame(at: at, nonce: nonce)
            XCTAssertEqual(ours.count, PeerWire.joinLength)
            XCTAssertEqual(Array(ours.prefix(2)), Array(reference.prefix(2)), key)
            XCTAssertEqual(id.verifyJoin(nonce: nonce, frame: ours).role, room.role, key)
            XCTAssertEqual(id.verifyJoin(nonce: [UInt8](repeating: 2, count: 32), frame: ours).refusal,
                           PeerWire.Close.unauthorized)
            var otherRole = ours
            otherRole[1] ^= 1
            XCTAssertEqual(id.verifyJoin(nonce: nonce, frame: otherRole).refusal, PeerWire.Close.unauthorized)
        }
    }

    /// Both handshake messages and every transport frame, byte for byte.
    func testHandshakeAndTransportMatchVectors() throws {
        let v = try vectors()
        let psk = v.bytes("psk")
        let (initiator, message1) = try PeerNoiseInitiator.start(
            s: v.bytes("joiner_identity_private"), rs: v.bytes("inviter_identity_public"), psk: psk,
            e: v.bytes("joiner_ephemeral_private"), payload: v.bytes("init_payload"))
        XCTAssertEqual([PeerWire.initFrame] + message1, v.bytes("init_frame"), "INIT")

        let responder = try PeerNoiseResponder.read(s: v.bytes("inviter_identity_private"), psk: psk, msg: message1)
        XCTAssertEqual(responder.peer, v.bytes("joiner_identity_public"), "the inviter learns the joiner's identity")
        XCTAssertEqual(responder.payload, v.bytes("init_payload"))
        // The inviter's transport: sends inviter -> joiner, receives joiner -> inviter.
        let (message2, inviterSide) = try responder.reply(
            e: v.bytes("inviter_ephemeral_private"), payload: v.bytes("reply_payload"))
        XCTAssertEqual([PeerWire.replyFrame] + message2, v.bytes("reply_frame"), "REPLY")

        // The joiner's transport: sends joiner -> inviter, receives inviter -> joiner.
        let (joinerSide, replyPayload) = try initiator.finish(message2)
        XCTAssertEqual(replyPayload, v.bytes("reply_payload"))

        // Where each side resumes (SPEC §5.3): the joiner from 5, the inviter from 2.
        let joinerResume = try XCTUnwrap(PeerConnectMachine.Resume.decode(v.bytes("init_payload")))
        let inviterResume = try XCTUnwrap(PeerConnectMachine.Resume.decode(v.bytes("reply_payload")))
        XCTAssertEqual(joinerResume.encode(), v.bytes("init_payload"))
        XCTAssertEqual(inviterResume.encode(), v.bytes("reply_payload"))
        let joinerFrom = inviterResume.seenStream == joinerResume.stream
            ? max(inviterResume.seenNext, joinerResume.base) : joinerResume.base
        let inviterFrom = joinerResume.seenStream == inviterResume.stream
            ? max(joinerResume.seenNext, inviterResume.base) : inviterResume.base
        XCTAssertEqual(joinerFrom, 5)
        XCTAssertEqual(inviterFrom, 2)

        var joiner = joinerSide
        var inviter = inviterSide
        let long = (0..<100_000).map { UInt8($0 % 251) }
        var checked = 0
        for message in v.messages {
            let fromJoiner = (message["direction"] as? String) == "joiner->inviter"
            let what = message["plaintext"] as? String ?? ""
            let plaintexts: [[UInt8]]
            if what.hasPrefix("ack "), let n = UInt64(what.dropFirst(4)) {
                plaintexts = [[0x03] + PeerConnectCrypto.u64be(n)]
            } else if what.hasSuffix(": close") {
                plaintexts = [[0x02]]
            } else if let range = what.range(of: "utf8 ") {
                plaintexts = chunked(Array(what[range.upperBound...].utf8))
            } else if what.hasSuffix("empty") {
                plaintexts = chunked([])
            } else {
                plaintexts = chunked(long)
            }
            var frames: [[UInt8]] = []
            for plaintext in plaintexts {
                let ciphertext = fromJoiner ? try joiner.encrypt(plaintext) : try inviter.encrypt(plaintext)
                frames.append([PeerWire.messageFrame] + ciphertext)
            }
            if let expected = message["frames"] as? [String] {
                XCTAssertEqual(frames.map(hex), expected, what)
            } else {
                let expected = message["frames_sha256"] as? [String] ?? []
                XCTAssertEqual(frames.map { hex(PeerConnectCrypto.sha256($0)) }, expected, what)
            }
            for (frame, plaintext) in zip(frames, plaintexts) {
                let body = Array(frame.dropFirst())
                let opened = fromJoiner ? try inviter.decrypt(body) : try joiner.decrypt(body)
                XCTAssertEqual(opened, plaintext, what)
            }
            checked += 1
        }
        XCTAssertEqual(checked, 6)
    }

    private func chunked(_ data: [UInt8]) -> [[UInt8]] {
        var out: [[UInt8]] = []
        var start = 0
        repeat {
            let end = min(start + PeerConnectMachine.chunkLength, data.count)
            out.append([end < data.count ? 0x01 : 0x00] + data[start..<end])
            start = end
        } while start < data.count
        return out
    }

    func testNoiseRejectsWrongKeysAndTampering() throws {
        let si = [UInt8](repeating: 1, count: 32), sr = [UInt8](repeating: 2, count: 32)
        let psk = [UInt8](repeating: 5, count: 32)
        let srPub = PeerConnectCrypto.x25519Public(sr)
        let e3 = [UInt8](repeating: 3, count: 32), e4 = [UInt8](repeating: 4, count: 32)
        let (_, msg1) = try PeerNoiseInitiator.start(s: si, rs: srPub, psk: psk, e: e3, payload: [])
        XCTAssertThrowsError(try PeerNoiseResponder.read(s: sr, psk: [UInt8](repeating: 6, count: 32), msg: msg1))
        XCTAssertThrowsError(try PeerNoiseResponder.read(s: [UInt8](repeating: 9, count: 32), psk: psk, msg: msg1))
        for i in 0..<msg1.count {
            var bad = msg1
            bad[i] ^= 1
            XCTAssertThrowsError(try PeerNoiseResponder.read(s: sr, psk: psk, msg: bad), "flipped byte \(i)")
        }
        XCTAssertThrowsError(try PeerNoiseResponder.read(s: sr, psk: psk, msg: Array(msg1.dropLast())))

        let (initiator, first) = try PeerNoiseInitiator.start(s: si, rs: srPub, psk: psk, e: e3, payload: [])
        let (reply, responderTransport) = try PeerNoiseResponder.read(s: sr, psk: psk, msg: first).reply(e: e4, payload: [])
        var bad = reply
        bad[40] ^= 1
        XCTAssertThrowsError(try initiator.finish(bad))
        // A failed reply leaves the handshake usable.
        var (a, _) = try initiator.finish(reply)
        var b = responderTransport

        // Within a session, reordering and replays fail.
        let m1 = try a.encrypt(Array("one".utf8))
        let m2 = try a.encrypt(Array("two".utf8))
        XCTAssertThrowsError(try b.decrypt(m2), "reordered")
        XCTAssertEqual(try b.decrypt(m1), Array("one".utf8))
        XCTAssertThrowsError(try b.decrypt(m1), "replayed")

        // A low-order ephemeral key (u = 1) is refused.
        var lowOrder = [UInt8](repeating: 0, count: PeerNoise.initLength)
        lowOrder[0] = 1
        XCTAssertThrowsError(try PeerNoiseResponder.read(s: sr, psk: psk, msg: lowOrder))
    }

    // MARK: - Rooms and records

    func testBase64URLIsStrict() {
        for bad in ["A", "AB=", "AB==", "A+", "A/", "AB C", "AR", "AAB", "-_9"] {
            XCTAssertNil(PeerBase64URL.decode(bad), bad)
        }
        XCTAssertEqual(PeerBase64URL.decode("AA"), [0])
        XCTAssertEqual(PeerBase64URL.decode("AAA"), [0, 0])
        XCTAssertEqual(PeerBase64URL.decode(""), [])
        let data = (0..<1000).map { UInt8($0 % 256) }
        for length in 0..<200 {
            let bytes = Array(data[length..<(length * 2 + 3)])
            let text = PeerBase64URL.encode(bytes)
            XCTAssertEqual(text, Data(bytes).base64EncodedString()
                .replacingOccurrences(of: "+", with: "-").replacingOccurrences(of: "/", with: "_")
                .replacingOccurrences(of: "=", with: ""))
            XCTAssertEqual(PeerBase64URL.decode(text), bytes)
        }
    }

    func testInviteRoundTripAndRoomDerivation() throws {
        let inviter = try PeerRoom.create(relay: "wss://relay.example.com/")
        XCTAssertEqual(inviter.relay, "wss://relay.example.com")
        let text = try XCTUnwrap(inviter.invite)
        XCTAssertTrue(text.hasPrefix("wss://relay.example.com#"))
        XCTAssertEqual(text.count, "wss://relay.example.com#".count + 86)
        let joiner = try PeerRoom.join(invite: text)
        XCTAssertEqual(joiner.id, inviter.id)
        XCTAssertEqual(try PeerInvite(text).roomId.description, inviter.id)
        XCTAssertEqual(joiner.psk, inviter.psk)
        XCTAssertEqual(joiner.peerKey, inviter.publicKey)
        XCTAssertEqual(joiner.endpoint(at: .invite), "wss://relay.example.com/v1/\(inviter.id)")
        XCTAssertNil(joiner.invite)
    }

    func testPairIdIsSharedByTheTwoPeersOnly() throws {
        var inviter = try PeerRoom.create(relay: "wss://r.example")
        let invite = try XCTUnwrap(inviter.invite)
        var joiner = try PeerRoom.join(invite: invite)
        inviter.setPaired(joiner.publicKey)
        joiner.setPaired(inviter.publicKey)
        XCTAssertEqual(inviter.id, joiner.id)
        XCTAssertEqual(inviter.id, inviter.roomId(at: .pair).description)
        XCTAssertNotEqual(inviter.id, try PeerInvite(invite).roomId.description)
        XCTAssertNil(inviter.invite)
        // Someone else with the invite, even knowing both public keys, gets a different pair ID.
        var stranger = try PeerRoom.join(invite: invite)
        stranger.setPaired(inviter.publicKey)
        XCTAssertNotEqual(stranger.id, inviter.id)
    }

    func testRecordRoundTrips() throws {
        var inviter = try PeerRoom.create(relay: "ws://127.0.0.1:8080/relay")
        var joiner = try PeerRoom.join(invite: try XCTUnwrap(inviter.invite))
        for step in 0..<3 {
            for room in [inviter, joiner] {
                let back = try PeerRoom.parse(room.record)
                XCTAssertEqual(back.record, room.record)
                XCTAssertEqual(back.recordBytes, room.recordBytes)
                XCTAssertEqual(back.id, room.id)
                XCTAssertEqual(back.paired, room.paired)
                XCTAssertEqual(back.closing, room.closing)
                XCTAssertEqual(back.role, room.role)
                XCTAssertEqual(back.publicKey, room.publicKey)
            }
            if step == 0 {
                inviter.setPaired(joiner.publicKey)
                joiner.setPaired(inviter.publicKey)
            } else {
                inviter.setClosing()
                joiner.setClosing()
            }
        }
        XCTAssertNil(inviter.invite)
        // An inviter record claiming a peer without being paired is inconsistent.
        var raw = try PeerRoom.create(relay: "wss://r.example").recordBytes
        raw[2] = 1
        raw.insert(contentsOf: [UInt8](repeating: 9, count: 32), at: 67)
        XCTAssertThrowsError(try PeerRoom(recordBytes: raw)) { XCTAssertEqual($0 as? PeerConnectError, .invalidRecord) }
        // Unknown version or flags are refused rather than misread.
        var newer = try PeerRoom.create(relay: "wss://r.example").recordBytes
        newer[0] = 3
        XCTAssertThrowsError(try PeerRoom(recordBytes: newer))
        newer[0] = 2
        newer[2] = 8
        XCTAssertThrowsError(try PeerRoom(recordBytes: newer))
    }

    /// Records written by the reference client (the wasm build the web wallet
    /// vendors) parse here, mean the same, and serialize back to the same text.
    func testParsesRecordsFromTheReferenceClient() throws {
        let v = try vectors()
        let joinerRecord = "AgEBAAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh9LsadChxdIT6974DEDZSRwrPRGkZnGg-Cxt-Jde4uB6DWActY2WIDRruoymt-RITg4Ue0hoo47dell0NLNFmJUd3NzOi8vcmVsYXkuZXhhbXBsZS5jb20"
        let joiner = try PeerRoom.parse(joinerRecord)
        XCTAssertEqual(joiner.role, .joiner)
        XCTAssertEqual(joiner.relay, "wss://relay.example.com")
        XCTAssertEqual(joiner.id, v.string("invite_id_text"))
        XCTAssertEqual(joiner.secret, v.bytes("room_secret"))
        XCTAssertEqual(joiner.peerKey, v.bytes("inviter_identity_public"))
        XCTAssertEqual(hex(joiner.publicKey), "3a94264bd3c0ad7652c3dd33422e5d9347a5ae18a71f8068b1a2e6780193aa65")
        XCTAssertEqual(joiner.record, joinerRecord)

        let inviterRecord = "AgAAw6SLPkaU3e4fWkpvwc1GCdxSbfjxqy2CzBeRowSsSTmW34Wy-fGCTjmo1smtRi1MWXZqL1hRWu_sohuWHplmO3dzOi8vMTI3LjAuMC4xOjgwODAvcmVsYXk"
        let inviter = try PeerRoom.parse(inviterRecord)
        XCTAssertEqual(inviter.role, .inviter)
        XCTAssertEqual(inviter.relay, "ws://127.0.0.1:8080/relay")
        XCTAssertEqual(inviter.id, "HWIY5_odw49UhYrsXQeccbupPIrfMoHxw0eEq6Aa-qk")
        XCTAssertEqual(inviter.invite, "ws://127.0.0.1:8080/relay#w6SLPkaU3e4fWkpvwc1GCdxSbfjxqy2CzBeRowSsSTnUttmuHbU-f-xBhsF7LPtd-VsvWYnUf6X3nHIQqo2-AA")
        XCTAssertEqual(hex(inviter.publicKey), "d4b6d9ae1db53e7fec4186c17b2cfb5df95b2f5989d47fa5f79c7210aa8dbe00")
        XCTAssertEqual(inviter.record, inviterRecord)
    }

    func testRejectsBadInputs() throws {
        for url in ["", "http://x", "wss://", "wss:///path", "wss://h?q=1", "wss://h#f", "wss://h h"] {
            XCTAssertThrowsError(try PeerRoom.create(relay: url), url)
        }
        let good = try XCTUnwrap(try PeerRoom.create(relay: "wss://h").invite)
        for bad in [String(good.dropLast()), "wss://h", "wss://h#", good.replacingOccurrences(of: "wss", with: "https")] {
            XCTAssertThrowsError(try PeerInvite(bad), bad)
        }
        // An inviter key of low order (u = 0, u = 1) could never complete a handshake.
        for lowOrder in [[UInt8](repeating: 0, count: 32), [1] + [UInt8](repeating: 0, count: 31)] {
            let fragment = PeerBase64URL.encode([UInt8](repeating: 7, count: 32) + lowOrder)
            XCTAssertThrowsError(try PeerInvite("wss://h#\(fragment)")) {
                XCTAssertEqual($0 as? PeerConnectError, .invalidInvite)
            }
        }
        XCTAssertThrowsError(try PeerRoomId("AAAA"))
        XCTAssertThrowsError(try PeerRoom.parse(""))
    }

    // MARK: - The machine, through an in-memory relay

    func testPairsMovesToThePairIdAndExchangesMessages() throws {
        let sim = PeerSim()
        let (a, b, inviter, joiner) = try sim.pair()
        XCTAssertEqual(sim.events(a).filter { $0 == .paired }.count, 1)
        XCTAssertEqual(sim.events(b).filter { $0 == .paired }.count, 1)
        XCTAssertEqual(inviter.peerKey, joiner.publicKey)
        XCTAssertEqual(joiner.peerKey, inviter.publicKey)

        XCTAssertEqual(try sim.send(a, "hello"), 0)
        XCTAssertEqual(try sim.send(b, ""), 0)
        XCTAssertEqual(try sim.send(b, "world"), 1)
        XCTAssertEqual(sim.messages(b), ["hello"])
        XCTAssertEqual(sim.messages(a), ["", "world"])
        sim.advance(100) // acknowledgements are batched briefly
        XCTAssertTrue(sim.events(a).contains(.delivered(1)))
        XCTAssertTrue(sim.events(b).contains(.delivered(2)))
        XCTAssertEqual(sim.nodes[a].machine.queued, 0)
    }

    func testEitherPeerMayArriveFirst() throws {
        let sim = PeerSim()
        let inviter = try PeerRoom.create(relay: "wss://relay.test")
        let b = sim.add(try PeerRoom.join(invite: try XCTUnwrap(inviter.invite)))
        XCTAssertEqual(sim.state(b), .waiting)
        sim.advance(5_000)
        let a = sim.add(inviter)
        sim.advance(25_000) // the joiner may be looking at the pair ID meanwhile
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        XCTAssertTrue(sim.nodes[a].machine.room.paired && sim.nodes[b].machine.room.paired)
    }

    func testSplitsAndReassemblesLargeMessages() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        let big = (0..<1_000_000).map { UInt8(truncatingIfNeeded: $0 &* 7) }
        let exact = [UInt8](repeating: 1, count: 65518)
        let over = [UInt8](repeating: 2, count: 65519)
        for message in [big, exact, over] { try sim.send(a, message) }
        XCTAssertEqual(sim.messageBytes(b), [big, exact, over])
        // Every frame stays within the relay's 65,536 bytes.
        XCTAssertLessThanOrEqual(sim.relay.largestFrame, PeerWire.maxFrameLength)
        XCTAssertEqual(sim.relay.largestFrame, PeerWire.maxFrameLength)

        XCTAssertThrowsError(try sim.send(a, [UInt8](repeating: 0, count: PeerWire.maxMessageLength + 1))) {
            XCTAssertEqual($0 as? PeerSendError, .tooLarge)
        }
        try sim.send(a, [UInt8](repeating: 3, count: PeerWire.maxMessageLength))
        XCTAssertEqual(sim.messageBytes(b).first?.count, PeerWire.maxMessageLength)
    }

    func testMessagesSentWhileApartArriveAfterReconnecting() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        _ = sim.events(a)
        sim.relay.down = true
        sim.restartRelay()
        XCTAssertEqual(sim.state(a), .offline)
        XCTAssertEqual(sim.state(b), .offline)
        for i in 0..<5 { try sim.send(a, [UInt8(i)]) }
        try sim.send(b, "from b")
        XCTAssertGreaterThan(sim.nodes[a].machine.queued, 0)
        sim.advance(5_000) // retries fail while the relay is down, backing off
        sim.relay.down = false
        sim.advance(10_000)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        XCTAssertEqual(sim.messageBytes(b), (0..<5).map { [UInt8($0)] })
        XCTAssertEqual(sim.messages(a), ["from b"])
        XCTAssertFalse(sim.events(a).contains(.paired), "pairing happens once")
        sim.advance(100)
        XCTAssertEqual(sim.nodes[a].machine.queued, 0)
    }

    func testNothingIsLostOrDuplicatedHoweverOftenConnectionsDrop() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        sim.relay.cutEvery = 7 // cut the receiving connection every 7 frames
        var sent: [[UInt8]] = []
        for round in 0..<40 {
            for i in 0..<10 {
                let message = PeerConnectCrypto.u64be(UInt64(round * 10 + i))
                if (try? sim.send(a, message)) != nil { sent.append(message) }
            }
            try sim.send(b, PeerConnectCrypto.u64be(UInt64(round)))
            sim.advance(2_000)
        }
        sim.relay.cutEvery = nil
        sim.advance(60_000)
        XCTAssertEqual(sim.messageBytes(b), sent)
        XCTAssertEqual(sim.messageBytes(a), (0..<40).map { PeerConnectCrypto.u64be(UInt64($0)) })
        XCTAssertEqual(sim.nodes[a].machine.queued, 0)
    }

    /// A peer restarted from its saved record (a new stream, nothing in
    /// memory) gets what was sent while it was away, exactly once.
    func testRestartFromRecordReceivesWhatWasSentMeanwhile() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        let saved = sim.nodes[b].machine.room.record
        sim.nodes[b].machine.leave()
        sim.run()
        XCTAssertEqual(sim.state(a), .waiting)
        try sim.send(a, "while you were away")
        sim.advance(1_000)
        let b2 = sim.add(try PeerRoom.parse(saved))
        sim.advance(1_000)
        XCTAssertEqual(sim.state(b2), .connected)
        XCTAssertEqual(sim.messages(b2), ["while you were away"])
        XCTAssertTrue(sim.messages(b).isEmpty)
        // Later reconnections deliver nothing twice.
        sim.restartRelay()
        sim.advance(10_000)
        XCTAssertEqual(sim.state(b2), .connected)
        XCTAssertTrue(sim.messages(b2).isEmpty)
        XCTAssertEqual(sim.nodes[a].machine.queued, 0)
    }

    func testNewerConnectionReplacesOlderOne() throws {
        let sim = PeerSim()
        let (a, b, _, joiner) = try sim.pair()
        let b2 = sim.add(joiner) // the same room opened a second time
        XCTAssertTrue(sim.events(b).contains(.replaced))
        XCTAssertEqual(sim.state(b), .offline)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b2), .connected)
        try sim.send(a, "to the new one")
        XCTAssertEqual(sim.messages(b2), ["to the new one"])
        XCTAssertTrue(sim.messages(b).isEmpty)
        // The replaced instance stays away for a minute, then takes the seat back.
        sim.advance(55_000)
        XCTAssertEqual(sim.state(b), .offline)
        sim.advance(15_000)
        XCTAssertEqual(sim.state(b), .connected)
        XCTAssertTrue(sim.events(b2).contains(.replaced))
        sim.nodes[b2].machine.leave()
        sim.run()
        sim.advance(120_000)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
    }

    func testClosingEndsTheRoomOnBothSides() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        try sim.send(a, "last words")
        sim.nodes[a].machine.close()
        XCTAssertTrue(sim.nodes[a].machine.room.closing)
        XCTAssertThrowsError(try sim.nodes[a].machine.send(Array("too late".utf8))) {
            XCTAssertEqual($0 as? PeerSendError, .closed)
        }
        sim.run()
        XCTAssertEqual(sim.messages(b), ["last words"])
        XCTAssertEqual(sim.state(a), .closed)
        XCTAssertEqual(sim.nodes[a].machine.reason, .closed)
        sim.advance(6_000) // b stays a moment in case a dropped before seeing its ACK
        XCTAssertEqual(sim.state(b), .closed)
        XCTAssertEqual(sim.nodes[b].machine.reason, .closedByPeer)
        XCTAssertTrue(sim.events(b).contains(.state(.closed, .closedByPeer)))
        sim.advance(120_000)
        XCTAssertTrue(sim.nodes.allSatisfy { $0.socket == nil && $0.machine.pollTimeout() == nil })
    }

    func testClosingReachesAPeerThatWasAway() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        sim.nodes[b].machine.leave()
        sim.run()
        sim.nodes[a].machine.close()
        sim.run()
        sim.advance(60_000)
        XCTAssertNotEqual(sim.state(a), .closed, "nobody has been told yet")
        // a stops and restarts from its saved record, which says it is closing.
        let saved = try PeerRoom.parse(sim.nodes[a].machine.room.record)
        XCTAssertTrue(saved.closing)
        sim.nodes[a].machine.leave()
        sim.run()
        let a2 = sim.add(saved)
        let b2 = sim.add(sim.nodes[b].machine.room)
        sim.advance(7_000)
        XCTAssertEqual(sim.nodes[b2].machine.reason, .closedByPeer)
        XCTAssertEqual(sim.nodes[a2].machine.reason, .closed)
    }

    func testLeavingKeepsTheRoom() throws {
        let sim = PeerSim()
        let (a, b, inviter, _) = try sim.pair()
        sim.nodes[a].machine.leave()
        sim.run()
        XCTAssertEqual(sim.state(a), .closed)
        XCTAssertEqual(sim.nodes[a].machine.reason, .left)
        XCTAssertEqual(sim.state(b), .waiting)
        let a2 = sim.add(inviter)
        XCTAssertEqual(sim.state(a2), .connected)
        XCTAssertEqual(sim.state(b), .connected)
    }

    func testALeakedInviteReachesNobodyAfterPairing() throws {
        let sim = PeerSim()
        let inviter = try PeerRoom.create(relay: "wss://relay.test")
        let invite = try XCTUnwrap(inviter.invite)
        let a = sim.add(inviter)
        let b = sim.add(try PeerRoom.join(invite: invite))
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        let stranger = sim.add(try PeerRoom.join(invite: invite))
        sim.advance(120_000)
        XCTAssertNotEqual(sim.state(stranger), .connected)
        XCTAssertFalse(sim.events(a).contains(.replaced) || sim.events(b).contains(.replaced))
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        try sim.send(a, "still private")
        XCTAssertEqual(sim.messages(b), ["still private"])
        XCTAssertTrue(sim.messages(stranger).isEmpty)
    }

    func testPairingCompletesEvenIfItsConfirmationIsLost() throws {
        let sim = PeerSim()
        sim.relay.kickAfterReply = true // the joiner's confirmation never arrives
        let inviter = try PeerRoom.create(relay: "wss://relay.test")
        let a = sim.add(inviter)
        let b = sim.add(try PeerRoom.join(invite: try XCTUnwrap(inviter.invite)))
        XCTAssertFalse(sim.nodes[a].machine.room.paired)
        // The joiner looks at both IDs in turn until it finds the inviter again.
        sim.advance(90_000)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        XCTAssertTrue(sim.nodes[a].machine.room.paired && sim.nodes[b].machine.room.paired)
    }

    func testKeepaliveDetectsADeadConnection() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        sim.advance(10 * 60_000) // pongs keep everything up
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        sim.relay.blackhole = true
        sim.advance(PeerWire.receiveTimeout + 1)
        XCTAssertNotEqual(sim.state(a), .connected)
        sim.relay.blackhole = false
        sim.advance(60_000)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
    }

    func testTamperingBreaksTheSessionButLosesNothing() throws {
        let sim = PeerSim()
        let (a, b, _, _) = try sim.pair()
        sim.relay.tamper = true
        try sim.send(a, "original")
        XCTAssertTrue(sim.messages(b).isEmpty)
        XCTAssertEqual(sim.state(b), .offline)
        try sim.send(a, "next")
        sim.advance(5_000)
        XCTAssertEqual(sim.state(a), .connected)
        XCTAssertEqual(sim.state(b), .connected)
        XCTAssertEqual(sim.messages(b), ["original", "next"])
    }

    // MARK: - The connection, over fake sockets

    /// Two `PeerConnection`s through an in-memory relay behind the socket
    /// factory, on real timers: pairing, JSON both ways, a dropped socket,
    /// and the end of the room reaching the other side.
    func testConnectionsPairExchangeResumeAndClose() async throws {
        let relay = PeerFakeSocketRelay()
        let inviterRoom = try PeerRoom.create(relay: "wss://relay.test")
        let a = PeerConnection(room: inviterRoom, socketFactory: relay)
        let b = PeerConnection(room: try PeerRoom.join(invite: try XCTUnwrap(inviterRoom.invite)), socketFactory: relay)
        var gotA: [String] = [], gotB: [String] = []
        var pairedA = 0, pairedB = 0
        a.onMessage = { gotA.append(String(decoding: $0, as: UTF8.self)) }
        b.onMessage = { gotB.append(String(decoding: $0, as: UTF8.self)) }
        a.onPaired = { pairedA += 1 }
        b.onPaired = { pairedB += 1 }
        var statesB: [PeerConnectState] = []
        b.onState = { state, _ in statesB.append(state) }

        try await waitUntil { a.state == .connected && b.state == .connected && a.room.paired && b.room.paired }
        XCTAssertEqual(pairedA, 1)
        XCTAssertEqual(pairedB, 1)
        XCTAssertEqual(a.room.id, b.room.id)
        XCTAssertNotEqual(a.room.id, inviterRoom.id)

        try a.send(#"{"type":"hello","from":"inviter"}"#)
        try b.send(#"{"type":"hello","from":"joiner"}"#)
        try await waitUntil { gotA.count == 1 && gotB.count == 1 }
        XCTAssertEqual(gotB, [#"{"type":"hello","from":"inviter"}"#])
        XCTAssertEqual(gotA, [#"{"type":"hello","from":"joiner"}"#])

        // b's socket drops; a message sent meanwhile arrives once it is back.
        relay.dropAll(of: b.room.id, role: .joiner)
        try await waitUntil { b.state == .offline }
        try a.send("sent while b was away")
        try await waitUntil { gotB.count == 2 && b.state == .connected }
        XCTAssertEqual(gotB.last, "sent while b was away")
        try await waitUntil { a.queued == 0 }

        // a ends the room: a closes once b has acknowledged, b a moment later.
        a.close()
        XCTAssertTrue(a.room.closing)
        XCTAssertThrowsError(try a.send("too late"))
        try await waitUntil { a.state == .closed }
        XCTAssertEqual(a.reason, .closed)
        try await waitUntil(timeout: 10) { b.state == .closed }
        XCTAssertEqual(b.reason, .closedByPeer)
        XCTAssertEqual(statesB.last, .closed)
        XCTAssertEqual(gotB.count, 2, "nothing delivered twice")
    }

    func testConnectionAsksTheTokenProviderAndRetriesAfterRefusal() async throws {
        let relay = PeerFakeSocketRelay()
        relay.acceptedToken = "good"
        var asked = 0
        let room = try PeerRoom.create(relay: "wss://relay.test")
        let connection = PeerConnection(room: room, tokenProvider: {
            asked += 1
            return asked == 1 ? "stale" : "good"
        }, socketFactory: relay)
        var unauthorized = 0
        connection.onUnauthorized = { unauthorized += 1 }
        try await waitUntil { connection.state == .waiting }
        XCTAssertEqual(asked, 2)
        XCTAssertEqual(unauthorized, 1)
        XCTAssertTrue(relay.urls.first?.hasSuffix("?token=stale") ?? false)
        XCTAssertTrue(relay.urls.last?.hasSuffix("?token=good") ?? false)
        connection.leave()
        XCTAssertEqual(connection.state, .closed)
        XCTAssertEqual(connection.reason, .left)
    }

    private func waitUntil(timeout: TimeInterval = 5, _ condition: () -> Bool) async throws {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() {
            if Date() > deadline {
                XCTFail("timed out")
                throw CancellationError()
            }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
    }
}

// MARK: - Helpers

private final class PeerConnectBundleToken {}

private func hexBytes(_ text: String) -> [UInt8] {
    var out: [UInt8] = []
    var index = text.startIndex
    while index < text.endIndex, let next = text.index(index, offsetBy: 2, limitedBy: text.endIndex) {
        out.append(UInt8(text[index..<next], radix: 16) ?? 0)
        index = next
    }
    return out
}

private func hex(_ bytes: [UInt8]) -> String {
    bytes.map { String(format: "%02x", $0) }.joined()
}

/// A PeerConnect relay's decisions (the reference's relay.rs), without I/O:
/// seats, presence, forwarding, close codes.
private final class PeerTestRelayCore {
    enum Msg { case text(String), binary([UInt8]) }
    enum Action {
        /// Deliver to a connection.
        case deliver(Int, Msg)
        /// The relay closes a connection with this code.
        case close(Int, UInt16)
    }

    private struct Conn {
        let room: PeerRoomId
        let nonce: [UInt8]
        var role: PeerRole?
    }

    private var conns: [Int: Conn] = [:]
    private var seats: [String: Int] = [:]

    private func seat(_ room: PeerRoomId, _ role: PeerRole) -> String { "\(room)/\(role.rawValue)" }

    /// A new connection to `url`: the HELLO to send it.
    func accept(_ id: Int, url: String) -> [UInt8] {
        let path = url.split(separator: "?").first.map(String.init) ?? url
        let room = (try? PeerRoomId(String(path.split(separator: "/").last ?? ""))) ?? PeerRoomId(bytes: [])
        let nonce = PeerConnectCrypto.random(32)
        conns[id] = Conn(room: room, nonce: nonce, role: nil)
        return PeerWire.helloFrame(nonce: nonce)
    }

    func receive(_ id: Int, _ msg: Msg) -> [Action] {
        guard let conn = conns[id] else { return [] }
        switch msg {
        case .text(let text) where text == PeerWire.ping:
            return [.deliver(id, .text(PeerWire.pong))]
        case .binary(let frame) where frame.count > PeerWire.maxFrameLength:
            return drop(id, code: PeerWire.Close.tooBig)
        case .binary(let frame) where conn.role == nil:
            let verdict = conn.room.verifyJoin(nonce: conn.nonce, frame: frame)
            guard let role = verdict.role else { return drop(id, code: verdict.refusal ?? PeerWire.Close.protocolError) }
            return join(id, role)
        case .binary(let frame) where PeerWire.isPeerFrame(frame):
            guard let role = conn.role, let partner = seats[seat(conn.room, role == .inviter ? .joiner : .inviter)]
            else { return [] }
            return [.deliver(partner, .binary(frame))]
        default:
            return drop(id, code: PeerWire.Close.protocolError)
        }
    }

    private func join(_ id: Int, _ role: PeerRole) -> [Action] {
        guard var conn = conns[id] else { return [] }
        var actions: [Action] = []
        if let old = seats[seat(conn.room, role)] {
            conns[old] = nil
            actions.append(.close(old, PeerWire.Close.replaced))
        }
        seats[seat(conn.room, role)] = id
        conn.role = role
        conns[id] = conn
        let partner = seats[seat(conn.room, role == .inviter ? .joiner : .inviter)]
        actions.append(.deliver(id, .binary(PeerWire.presenceFrame(partner != nil))))
        if let partner { actions.append(.deliver(partner, .binary(PeerWire.presenceFrame(true)))) }
        return actions
    }

    /// Removes a connection; `code` is what the relay closes it with (nil when the peer hung up).
    func drop(_ id: Int, code: UInt16?) -> [Action] {
        guard let conn = conns.removeValue(forKey: id) else { return [] }
        var actions: [Action] = []
        if let code { actions.append(.close(id, code)) }
        if let role = conn.role {
            seats[seat(conn.room, role)] = nil
            if let partner = seats[seat(conn.room, role == .inviter ? .joiner : .inviter)] {
                actions.append(.deliver(partner, .binary(PeerWire.presenceFrame(false))))
            }
        }
        return actions
    }

    func seated(_ room: String, _ role: PeerRole) -> Int? {
        seats["\(room)/\(role.rawValue)"]
    }

    func roleOf(_ id: Int) -> PeerRole? { conns[id]?.role }

    func ids() -> [Int] { Array(conns.keys) }
}

/// The deterministic simulation: machines and the relay core on a virtual clock.
@MainActor
private final class PeerSim {
    final class Node {
        let machine: PeerConnectMachine
        var socket: Int?
        var events: [PeerOutput] = []
        init(_ machine: PeerConnectMachine) { self.machine = machine }
    }

    final class Relay {
        let core = PeerTestRelayCore()
        var down = false
        var blackhole = false
        var tamper = false
        var kickAfterReply = false
        var cutEvery: Int?
        var forwarded = 0
        var largestFrame = 0
        var nextSocket = 0
        var owners: [Int: Int] = [:]
    }

    private enum Delivery {
        case toRelay(Int, PeerTestRelayCore.Msg)
        case hangup(Int)
        case toPeer(Int, Int, PeerTestRelayCore.Msg)
        case closed(Int, Int, UInt16?)
    }

    var now: UInt64 = 1_000
    var nodes: [Node] = []
    let relay = Relay()
    private var queue: [Delivery] = []

    func add(_ room: PeerRoom) -> Int {
        nodes.append(Node(PeerConnectMachine(room: room, now: now)))
        run()
        return nodes.count - 1
    }

    /// Pairs two peers; returns their nodes and their paired rooms.
    func pair() throws -> (Int, Int, PeerRoom, PeerRoom) {
        let inviter = try PeerRoom.create(relay: "wss://relay.test")
        let inviteId = inviter.id
        let a = add(inviter)
        XCTAssertEqual(state(a), .waiting)
        let b = add(try PeerRoom.join(invite: try XCTUnwrap(inviter.invite)))
        XCTAssertEqual(state(a), .connected)
        XCTAssertEqual(state(b), .connected)
        let (ra, rb) = (nodes[a].machine.room, nodes[b].machine.room)
        XCTAssertTrue(ra.paired && rb.paired)
        XCTAssertEqual(ra.id, rb.id)
        XCTAssertNotEqual(ra.id, inviteId)
        return (a, b, ra, rb)
    }

    func run() {
        while true {
            var progress = false
            for i in nodes.indices {
                while let output = nodes[i].machine.pollOutput() {
                    progress = true
                    handle(i, output)
                }
            }
            while !queue.isEmpty {
                progress = true
                deliver(queue.removeFirst())
            }
            if !progress { return }
        }
    }

    func advance(_ ms: UInt64) {
        let end = now + ms
        while let next = nodes.compactMap({ $0.machine.pollTimeout() }).min(), next <= end {
            now = max(now, next)
            for node in nodes where (node.machine.pollTimeout() ?? .max) <= now {
                node.machine.handleTimeout(now: now)
            }
            run()
        }
        now = end
    }

    func state(_ i: Int) -> PeerConnectState { nodes[i].machine.state }

    @discardableResult
    func send(_ i: Int, _ text: String) throws -> UInt64 { try send(i, Array(text.utf8)) }

    @discardableResult
    func send(_ i: Int, _ data: [UInt8]) throws -> UInt64 {
        let id = try nodes[i].machine.send(data)
        run()
        return id
    }

    func events(_ i: Int) -> [PeerOutput] {
        defer { nodes[i].events = [] }
        return nodes[i].events
    }

    func messageBytes(_ i: Int) -> [[UInt8]] {
        var out: [[UInt8]] = []
        nodes[i].events.removeAll {
            if case .message(let data) = $0 { out.append([UInt8](data)); return true }
            return false
        }
        return out
    }

    func messages(_ i: Int) -> [String] {
        messageBytes(i).map { String(decoding: $0, as: UTF8.self) }
    }

    /// The relay restarts: every connection drops, all state is lost.
    func restartRelay() {
        for id in relay.core.ids() {
            _ = relay.core.drop(id, code: nil) // no PRESENCE: everything goes at once
            if let node = relay.owners[id] { queue.append(.closed(node, id, PeerWire.Close.goingAway)) }
        }
        run()
    }

    private func handle(_ i: Int, _ output: PeerOutput) {
        let socket = nodes[i].socket
        switch output {
        case .connect(let url):
            XCTAssertNil(socket, "a peer opens one socket at a time")
            if relay.down {
                nodes[i].machine.handleClose(now: now, code: nil)
                return
            }
            let id = relay.nextSocket
            relay.nextSocket += 1
            relay.owners[id] = i
            let hello = relay.core.accept(id, url: url)
            nodes[i].socket = id
            nodes[i].machine.handleOpen(now: now)
            queue.append(.toPeer(i, id, .binary(hello)))
        case .send(let frame):
            if let socket { queue.append(.toRelay(socket, .binary([UInt8](frame)))) }
        case .ping:
            if let socket { queue.append(.toRelay(socket, .text(PeerWire.ping))) }
        case .disconnect:
            if let id = nodes[i].socket {
                nodes[i].socket = nil
                queue.append(.hangup(id))
            }
        default:
            nodes[i].events.append(output)
        }
    }

    private func deliver(_ delivery: Delivery) {
        switch delivery {
        case .toPeer(let i, let id, let msg):
            guard nodes[i].socket == id else { return }
            switch msg {
            case .text(let text): nodes[i].machine.handleText(now: now, text)
            case .binary(let frame): nodes[i].machine.handleBinary(now: now, frame)
            }
        case .hangup(let id):
            apply(relay.core.drop(id, code: nil))
        case .closed(let i, let id, let code):
            if nodes[i].socket == id {
                nodes[i].socket = nil
                nodes[i].machine.handleClose(now: now, code: code)
            }
        case .toRelay(let id, let msg):
            if case .binary(let frame) = msg { relay.largestFrame = max(relay.largestFrame, frame.count) }
            apply(relay.core.receive(id, msg), from: id)
        }
    }

    private func apply(_ actions: [PeerTestRelayCore.Action], from sender: Int? = nil) {
        for action in actions {
            switch action {
            case .close(let id, let code):
                if let node = relay.owners[id] { queue.append(.closed(node, id, code)) }
            case .deliver(let id, .text(let text)):
                if !relay.blackhole, let node = relay.owners[id] { queue.append(.toPeer(node, id, .text(text))) }
            case .deliver(let id, .binary(var frame)):
                guard let node = relay.owners[id] else { continue }
                let forwarded = sender != nil && PeerWire.isPeerFrame(frame)
                if forwarded && frame[0] == PeerWire.messageFrame && relay.tamper {
                    relay.tamper = false
                    frame[5] ^= 1
                }
                queue.append(.toPeer(node, id, .binary(frame)))
                if forwarded && frame[0] == PeerWire.replyFrame && relay.kickAfterReply {
                    relay.kickAfterReply = false
                    queue.append(.hangup(id))
                    queue.append(.closed(node, id, nil))
                }
                if forwarded && frame[0] == PeerWire.messageFrame, let n = relay.cutEvery {
                    relay.forwarded += 1
                    if relay.forwarded % n == 0 {
                        queue.append(.hangup(id))
                        queue.append(.closed(node, id, nil))
                    }
                }
            }
        }
    }
}

/// The relay core behind a `PeerSocketFactory`, delivering asynchronously
/// on the main queue as a real socket would.
@MainActor
private final class PeerFakeSocketRelay: PeerSocketFactory {
    final class Socket: PeerSocket {
        let id: Int
        weak var relay: PeerFakeSocketRelay?
        var events: (@MainActor (PeerSocketEvent) -> Bool)?
        init(id: Int, relay: PeerFakeSocketRelay, events: @escaping @MainActor (PeerSocketEvent) -> Bool) {
            self.id = id
            self.relay = relay
            self.events = events
        }
        func send(_ data: Data) { relay?.later { $0.fromSocket(self.id, .binary([UInt8](data))) } }
        func send(text: String) { relay?.later { $0.fromSocket(self.id, .text(text)) } }
        func close() {
            guard events != nil else { return }
            events = nil
            relay?.later { $0.apply($0.core.drop(self.id, code: nil)) }
        }
    }

    private let core = PeerTestRelayCore()
    private var sockets: [Int: Socket] = [:]
    private var nextId = 0
    /// When set, connections must carry `?token=` this, or are closed with 4403.
    var acceptedToken: String?
    private(set) var urls: [String] = []

    func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket {
        let socket = Socket(id: nextId, relay: self, events: events)
        nextId += 1
        sockets[socket.id] = socket
        urls.append(url.absoluteString)
        later { relay in
            relay.toSocket(socket.id, .open)
            if let token = relay.acceptedToken, !url.absoluteString.hasSuffix("?token=\(token)") {
                relay.toSocket(socket.id, .closed(code: PeerWire.Close.token))
                return
            }
            relay.toSocket(socket.id, .binary(Data(relay.core.accept(socket.id, url: url.absoluteString))))
        }
        return socket
    }

    /// Cuts the connections in a seat, as a network failure would.
    func dropAll(of room: String, role: PeerRole) {
        guard let id = core.seated(room, role) else { return }
        apply(core.drop(id, code: nil))
        toSocket(id, .closed(code: nil))
    }

    fileprivate func later(_ body: @escaping @MainActor (PeerFakeSocketRelay) -> Void) {
        DispatchQueue.main.async { [weak self] in
            MainActor.assumeIsolated {
                if let self { body(self) }
            }
        }
    }

    fileprivate func fromSocket(_ id: Int, _ msg: PeerTestRelayCore.Msg) {
        apply(core.receive(id, msg))
    }

    fileprivate func apply(_ actions: [PeerTestRelayCore.Action]) {
        for action in actions {
            switch action {
            case .deliver(let id, .text(let text)): later { $0.toSocket(id, .text(text)) }
            case .deliver(let id, .binary(let frame)): later { $0.toSocket(id, .binary(Data(frame))) }
            case .close(let id, let code): later { $0.toSocket(id, .closed(code: code)) }
            }
        }
    }

    private func toSocket(_ id: Int, _ event: PeerSocketEvent) {
        guard let socket = sockets[id], let events = socket.events else { return }
        if case .closed = event {
            socket.events = nil
            sockets[id] = nil
        }
        if !events(event) { socket.close() }
    }
}
