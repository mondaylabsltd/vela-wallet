// M4: the PeerConnect rooms behind the multisig machines' `peer_*`
// operations — PeerConnect's own peer in the core (uniffi `PeerConnectPeer`)
// over a net this suite hands in, as Android's `MultisigPeerConnectTest`.
// `MemoryRelay` keeps the relay's side of SPEC §5 (HELLO, JOIN seats,
// PRESENCE, forwarding peer frames, `ping`/`pong`), so two Vela transports
// pair through it, Noise and all, exactly as through `relay.peerconnect.dev`
// — and the public relay's token is asked for as the core's `relay.*` rules
// say.

import Foundation
import XCTest

@testable import VelaWallet

@MainActor
final class MultisigPeerRoomsTests: XCTestCase {
    /// A relay in memory: seats per room, frames forwarded byte for byte, in
    /// order, each on a later turn of the main queue (never inside a send).
    private final class MemoryRelay: MultisigPeerNet {
        var urls: [String] = []
        var posts: [(url: String, body: String?)] = []
        var answer: (String) -> (Int?, String) = { _ in (nil, "unreachable") }
        /// Tokens the relay refuses (SPEC §4.7: closed with 4403).
        var refused: Set<String> = []
        private var seats: [String: [UInt8: Conn]] = [:]

        final class Conn: PeerSocket {
            let room: String
            let events: @MainActor (PeerSocketEvent) -> Bool
            weak var relay: MemoryRelay?
            var role: UInt8?
            var open = true

            init(room: String, relay: MemoryRelay, events: @escaping @MainActor (PeerSocketEvent) -> Bool) {
                self.room = room
                self.relay = relay
                self.events = events
            }

            /// Said to this connection's owner, later, in order.
            func say(_ event: PeerSocketEvent) {
                DispatchQueue.main.async {
                    MainActor.assumeIsolated { _ = self.events(event) }
                }
            }

            func send(_ data: Data) {
                guard open, let relay, let type = data.first else { return }
                let bytes = [UInt8](data)
                if type == 0x81, bytes.count > 1 {
                    let role = bytes[1]
                    self.role = role
                    var room = relay.seats[self.room] ?? [:]
                    if let previous = room[role], previous !== self {
                        previous.open = false
                        previous.say(.closed(code: 4001))
                    }
                    room[role] = self
                    relay.seats[self.room] = room
                    let other = room[1 - role]
                    say(.binary(Data([0x82, other != nil ? 1 : 0])))
                    other?.say(.binary(Data([0x82, 1])))
                } else if type < 0x80, let role {
                    relay.seats[room]?[1 - role]?.say(.binary(data))
                }
            }

            func send(text: String) {
                if open, text == "ping" { say(.text("pong")) }
            }

            func close() {
                guard open else { return }
                open = false
                guard let relay, let role, relay.seats[room]?[role] === self else { return }
                relay.seats[room]?[role] = nil
                relay.seats[room]?[1 - role]?.say(.binary(Data([0x82, 0])))
            }
        }

        func open(url: URL, events: @escaping @MainActor (PeerSocketEvent) -> Bool) -> any PeerSocket {
            let text = url.absoluteString
            urls.append(text)
            let conn = Conn(room: String(text.split(separator: "?", maxSplits: 1).first ?? ""), relay: self, events: events)
            conn.say(.open)
            if let token = URLComponents(string: text)?.queryItems?.first(where: { $0.name == "token" })?.value,
               refused.contains(token) {
                conn.open = false
                conn.say(.closed(code: 4403))
                return conn
            }
            conn.say(.binary(Data([0x80] + (0..<32).map { UInt8($0) })))
            return conn
        }

        func post(url: String, body: String?) async -> (status: Int?, body: String) {
            posts.append((url, body))
            let (status, text) = answer(url)
            return (status, text)
        }
    }

    private final class Told {
        var events: [[String: Any]] = []
        func of(_ type: String) -> [[String: Any]] { events.filter { $0["type"] as? String == type } }
    }

    private func defaults() -> UserDefaults {
        let name = "peer-rooms-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: name)!
        defaults.removePersistentDomain(forName: name)
        return defaults
    }

    private func rooms(_ relay: MemoryRelay, _ store: UserDefaults? = nil, told: Told? = nil) -> MultisigPeerRooms {
        let rooms = MultisigPeerRooms(defaultRelay: MultisigRelayTokens.defaultRelay, net: relay, defaults: store ?? defaults())
        if let told { rooms.tell = { told.events.append($0) } }
        return rooms
    }

    private func until(_ what: String, seconds: Double = 10, _ check: () -> Bool) async {
        let deadline = Date().addingTimeInterval(seconds)
        while !check(), Date() < deadline {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
        XCTAssertTrue(check(), what)
    }

    private func receive(_ rooms: MultisigPeerRooms, _ room: String) async -> MultisigPeerArrival {
        await rooms.receive(room: room)
    }

    private func status(_ event: [String: Any]) -> [String: Any] { event["status"] as? [String: Any] ?? [:] }

    func testTwoTransportsPairThroughTheRelayAndCarryJson() async throws {
        let relay = MemoryRelay()
        let velaTold = Told(), walletTold = Told()
        let vela = rooms(relay, told: velaTold)
        let wallet = rooms(relay, told: walletTold)

        let created = try await vela.open(relay: "ws://relay.test", invite: nil, pairing: nil)
        XCTAssertEqual(created.room, "room-1")
        let invite = try XCTUnwrap(created.invite)
        XCTAssertTrue(invite.hasPrefix("ws://relay.test#"), invite)
        let joined = try await wallet.open(relay: nil, invite: invite, pairing: nil)
        XCTAssertNil(joined.invite, "a joined room hands out no invite")

        await until("both sides connected") {
            velaTold.of("peer_state").contains { $0["state"] as? String == "connected" }
                && walletTold.of("peer_state").contains { $0["state"] as? String == "connected" }
        }
        await until("pairing told with the record that reopens the room") { !velaTold.of("paired").isEmpty }
        XCTAssertEqual(velaTold.of("paired").first?["room"] as? String, "room-1")
        let pairing = try XCTUnwrap(velaTold.of("paired").first?["pairing"] as? String)
        XCTAssertFalse(pairing.isEmpty)
        XCTAssertNotEqual(pairing, created.pairing, "paired, the record moves on")
        // On the relay: said up, as the core reads the state.
        XCTAssertTrue(velaTold.of("relay").contains { status($0)["kind"] as? String == "up" })
        // A local relay asks no token.
        XCTAssertTrue(relay.posts.isEmpty)
        XCTAssertTrue(relay.urls.allSatisfy { $0.hasPrefix("ws://relay.test/v1/") }, "\(relay.urls)")

        await vela.send(room: created.room, message: ["method": "walletpair_connect", "id": 1])
        guard case .message(let arrived) = await receive(wallet, joined.room) else { return XCTFail("nothing arrived") }
        XCTAssertEqual((arrived as? [String: Any])?["method"] as? String, "walletpair_connect")
        XCTAssertEqual((arrived as? [String: Any])?["id"] as? Int, 1)
        await wallet.send(room: joined.room, message: "plain words")
        guard case .message(let back) = await receive(vela, created.room) else { return XCTFail("nothing came back") }
        XCTAssertEqual(back as? String, "plain words")

        // Many, in order.
        for n in 0..<20 { await vela.send(room: created.room, message: ["n": n]) }
        for n in 0..<20 {
            guard case .message(let got) = await receive(wallet, joined.room) else { return XCTFail("lost \(n)") }
            XCTAssertEqual((got as? [String: Any])?["n"] as? Int, n)
        }

        // Ending the room ends it for both; a receive then says how.
        await vela.close(room: created.room, cancel: false, end: true)
        guard case .closed(let mine) = await receive(vela, created.room) else { return XCTFail("still open") }
        XCTAssertEqual(mine, "closed")
        guard case .closed(let theirs) = await receive(wallet, joined.room) else { return XCTFail("the other side still open") }
        XCTAssertEqual(theirs, "closed-by-peer")
    }

    func testAPairedRoomReopensFromItsRecordAndResumes() async throws {
        let relay = MemoryRelay()
        let velaTold = Told(), walletTold = Told()
        let vela = rooms(relay, told: velaTold)
        let wallet = rooms(relay, told: walletTold)
        let created = try await vela.open(relay: "ws://relay.test", invite: nil, pairing: nil)
        let joined = try await wallet.open(relay: nil, invite: created.invite, pairing: nil)
        await until("paired on both sides") { !velaTold.of("paired").isEmpty && !walletTold.of("paired").isEmpty }
        let velaRecord = try XCTUnwrap(velaTold.of("paired").last?["pairing"] as? String)
        let walletRecord = try XCTUnwrap(walletTold.of("paired").last?["pairing"] as? String)

        // Both leave (the screen went away), then come back from the records: no invite, the same room.
        vela.leaveAll()
        wallet.leaveAll()
        guard case .closed(let left) = await receive(vela, created.room) else { return XCTFail("still open") }
        XCTAssertEqual(left, "closed", "a room no longer here reads as closed")
        _ = joined

        let veloAgain = rooms(relay, told: velaTold)
        let walletAgain = rooms(relay, told: walletTold)
        let reopened = try await veloAgain.open(relay: nil, invite: nil, pairing: velaRecord)
        XCTAssertNil(reopened.invite, "a kept pairing hands out no invite")
        XCTAssertEqual(reopened.pairing, velaRecord)
        let rejoined = try await walletAgain.open(relay: nil, invite: nil, pairing: walletRecord)
        await veloAgain.send(room: reopened.room, message: ["again": true])
        guard case .message(let got) = await receive(walletAgain, rejoined.room) else { return XCTFail("not resumed") }
        XCTAssertEqual((got as? [String: Any])?["again"] as? Bool, true)
        veloAgain.leaveAll()
        walletAgain.leaveAll()
    }

    func testUnknownRoomsReadAsClosedAndBadInputsAreRefused() async throws {
        let vela = rooms(MemoryRelay())
        guard case .closed(let reason) = await receive(vela, "room-99") else { return XCTFail("an unknown room hangs") }
        XCTAssertEqual(reason, "closed")
        do {
            _ = try await vela.open(relay: nil, invite: "not an invite", pairing: nil)
            XCTFail("a bad invite opened a room")
        } catch {}
        do {
            _ = try await vela.open(relay: nil, invite: nil, pairing: "nonsense")
            XCTFail("a bad record opened a room")
        } catch {}
    }

    func testThePublicRelayWaitsForASiteTokenThenConnects() async throws {
        let relay = MemoryRelay()
        let expires = Int(Date().timeIntervalSince1970) + 86_400
        relay.answer = { url in
            url.contains("getvela.app") ? (200, #"{"token":"site-token","expires_at":\#(expires)}"#) : (500, "no")
        }
        let store = defaults()
        let vela = rooms(relay, store)
        _ = try await vela.open(relay: nil, invite: nil, pairing: nil)
        await until("connected after the token") { !relay.urls.isEmpty }
        XCTAssertTrue(relay.urls.first?.hasPrefix(MultisigRelayTokens.defaultRelay + "/v1/") == true, "\(relay.urls)")
        XCTAssertTrue(relay.urls.first?.contains("site-token") == true, "the token rides on the URL: \(relay.urls)")
        let post = try XCTUnwrap(relay.posts.first)
        XCTAssertEqual(post.url, MultisigRelayTokens.tokenEndpoint)
        XCTAssertTrue(post.body?.contains("\"device\"") == true, "asked under this device's ID")
        XCTAssertNotNil(store.string(forKey: MultisigRelayTokens.deviceKey))
        let kept = try XCTUnwrap(store.string(forKey: MultisigRelayTokens.tokenKey))
        XCTAssertTrue(kept.contains("site-token") && kept.contains("\"source\":\"site\""), kept)

        // A second room uses the kept token: no new request.
        _ = try await vela.open(relay: nil, invite: nil, pairing: nil)
        await until("second room connects") { relay.urls.count >= 2 }
        XCTAssertEqual(relay.posts.count, 1)
        vela.leaveAll()
    }

    func testNoSiteTokenFallsBackToTheTrialsAndNoneIsSaidHonestly() async throws {
        let relay = MemoryRelay()
        let expires = Int(Date().timeIntervalSince1970) + 3_600
        relay.answer = { url in
            url.contains("trial") ? (200, #"{"token":"trial-token","expires_at":\#(expires)}"#) : (429, "slow down")
        }
        let store = defaults()
        let vela = rooms(relay, store)
        _ = try await vela.open(relay: nil, invite: nil, pairing: nil)
        await until("connected on the trial's token") { !relay.urls.isEmpty }
        XCTAssertTrue(store.string(forKey: MultisigRelayTokens.tokenKey)?.contains("\"source\":\"trial\"") == true)
        vela.leaveAll()

        // Nowhere answers: the relay status says why, retrying; nothing is connected to.
        let none = MemoryRelay()
        let told = Told()
        let lonely = rooms(none, told: told)
        _ = try await lonely.open(relay: nil, invite: nil, pairing: nil)
        await until("said retrying for want of a token") {
            told.of("relay").contains { status($0)["kind"] as? String == "retrying" && status($0)["why"] as? String == "noToken" }
        }
        XCTAssertTrue(none.urls.isEmpty)
        // Retry cuts the wait short: asked again at once.
        let asked = none.posts.count
        lonely.retryAll()
        await until("asked again on Retry") { none.posts.count > asked }
        lonely.leaveAll()
    }

    func testARefusedTokenIsForgottenAndANewOneAskedFor() async throws {
        let relay = MemoryRelay()
        let expires = Int(Date().timeIntervalSince1970) + 86_400
        relay.refused = ["old-token"]
        relay.answer = { url in
            url.contains("getvela.app") ? (200, #"{"token":"new-token","expires_at":\#(expires)}"#) : (500, "no")
        }
        let store = defaults()
        // A kept token the core finds good — but the relay no longer takes it.
        store.set(#"{"token":"old-token","expiresAt":\#(expires),"source":"site"}"#, forKey: MultisigRelayTokens.tokenKey)
        let vela = rooms(relay, store)
        _ = try await vela.open(relay: nil, invite: nil, pairing: nil)
        await until("tried with the new token") { relay.urls.contains { $0.contains("token=new-token") } }
        XCTAssertTrue(relay.urls.first?.contains("token=old-token") == true, "the kept one first: \(relay.urls)")
        XCTAssertEqual(relay.posts.count, 1, "one new token asked for")
        let kept = try XCTUnwrap(store.string(forKey: MultisigRelayTokens.tokenKey))
        XCTAssertTrue(kept.contains("new-token"), "the refused one forgotten, the new one kept: \(kept)")
        vela.leaveAll()
    }
}
