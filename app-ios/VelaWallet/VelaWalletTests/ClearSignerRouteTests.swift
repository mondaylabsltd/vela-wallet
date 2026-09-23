//
//  ClearSignerRouteTests.swift
//  VelaWalletTests
//
//  Spec 075 on iOS: the Clear Signer as a fourth PASSKEY ROUTE.
//
//  071's tests (`ClearSignerTests`) cover it as a way to sign. These cover
//  what it became:
//
//  - a session of several requests on one page visit — create, then that
//    key's member proof, then `bye` — spoken to the way the page speaks to it;
//  - the tunnel requester, against the shared vectors
//    (`rust/crates/vela-core/tests/clear-signer/secure-session.json`) and
//    against a fake tunnel;
//  - the create and sign-in choosers listing four routes, and the signing
//    sheet five;
//  - `signer_origin` round-tripping through the account record and taking the
//    ceremony back to the page the key lives behind;
//  - the onboarding executor reporting the core's verdict as the machine
//    result, and a closed page as a cancel rather than an error.
//
//  Nothing here fakes a verdict. Every answer goes through the core
//  (`clearSignerVerifyCeremony`, `clearSignerVerify`, `signRoute`), and the
//  attestation is the conformance vectors' real one.
//

import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - A page that runs ceremonies

/// What a Clear Signer page answers for the four ceremonies, built from the
/// repository's own conformance vectors so the core judges real bytes.
struct ClearSignerCeremonyFixture {
    let key = P256.Signing.PrivateKey()
    let signerUrl = "https://sign.getvela.app/"
    let origin = "https://sign.getvela.app"
    /// The credential the vector's attestation object carries.
    let credential = Data(repeating: 0xCD, count: 16)

    var credentialHex: String { ClearSignerFixture.hex(credential) }
    var credentialB64: String { ClearSignerFixture.base64url(credential) }

    /// `extractPublicKey/real-key` from `tests/vectors/webauthn.json` — a
    /// genuine attestation object whose COSE key is a P-256 point, which is
    /// what `verify_registration` insists on.
    static let attestationObjectHex: String = {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
            .appendingPathComponent("rust/crates/vela-core/tests/vectors/webauthn.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: root))) as? [String: Any]
        let cases = json?["cases"] as? [[String: Any]] ?? []
        let hex = cases
            .first { $0["name"] as? String == "extractPublicKey/real-key" }
            .flatMap { ($0["input"] as? [String: Any])?["attestation_object"] as? String } ?? ""
        return hex.hasPrefix("0x") ? String(hex.dropFirst(2)) : hex
    }()

    /// `{"type":"webauthn.create",…}` in the byte order the core's prefix
    /// check demands, over a challenge the page drew itself.
    func createAnswer(challenge: Data = Data(repeating: 0x5A, count: 32)) -> [String: Any] {
        let clientData = Data(
            #"{"type":"webauthn.create","challenge":"\#(ClearSignerFixture.base64url(challenge))","origin":"\#(origin)","crossOrigin":false}"#.utf8
        )
        return [
            "registration": [
                "credentialId": credentialB64,
                "attestationObject": Self.attestationObjectHex,
                "clientDataJSON": ClearSignerFixture.hex(clientData),
                "authenticatorAttachment": "platform",
                "transports": "internal,hybrid",
            ],
            "origin": origin,
        ]
    }

    /// An assertion over `challenge`, user-verified, by this fixture's key.
    func assertionAnswer(challenge: Data) -> [String: Any] {
        var authenticatorData = Data(SHA256.hash(data: Data("getvela.app".utf8)))
        authenticatorData.append(contentsOf: [0x05, 0, 0, 0, 7])
        let clientData = Data(
            #"{"type":"webauthn.get","challenge":"\#(ClearSignerFixture.base64url(challenge))","origin":"\#(origin)","crossOrigin":false}"#.utf8
        )
        var signed = authenticatorData
        signed.append(Data(SHA256.hash(data: clientData)))
        let signature = try! key.signature(for: signed)
        return [
            "assertion": [
                "credentialId": credentialB64,
                "signatureDer": ClearSignerFixture.hex(signature.derRepresentation),
                "authenticatorData": ClearSignerFixture.hex(authenticatorData),
                "clientDataJSON": ClearSignerFixture.hex(clientData),
                "userHandle": NSNull(),
                "authenticatorAttachment": "platform",
            ],
            "origin": origin,
        ]
    }

    /// A `register_passkey` operation with `method = clear_signer`, as the
    /// create machine sends it.
    func registerOperation(name: String = "Mine") -> [String: Any] {
        [
            "type": "register_passkey", "name": name,
            "exclude_credential_ids": [] as [String], "method": "clear_signer",
        ]
    }

    /// The member proof for the key just minted.
    func memberProofOperation(groupPublicKey: String = "04" + String(repeating: "ab", count: 64))
        -> [String: Any] {
        [
            "type": "sign_member_proof", "credential_id": credentialHex,
            "public_key_hex": "04" + String(repeating: "cd", count: 64),
            "attestation_hex": "", "transports": "internal",
            "method": "clear_signer", "group_public_key_hex": groupPublicKey,
        ]
    }

    static func json(_ object: [String: Any]) -> String {
        String(decoding: (try? JSONSerialization.data(withJSONObject: object)) ?? Data(), as: UTF8.self)
    }
}

// MARK: - One session, several requests

@MainActor
struct ClearSignerSessionTests {

    /// A create and its member proof on ONE page visit (contract §1.5): the
    /// page says hello once, takes two intents in turn, and the flow's end is
    /// a `bye` rather than a socket that vanished.
    ///
    /// This is the shape of the whole 075 create flow. Before it, every
    /// ceremony opened its own tab — which for a create meant two tabs and two
    /// "allow this page to reach other apps" prompts for one key.
    @Test func aCreateAndItsMemberProofRunOnOnePageVisit() async throws {
        let page = ClearSignerCeremonyFixture()
        let registry = "https://p256-index-v2.getvela.app"
        let register = page.registerOperation()
        let registerRequest = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(register),
            id: "ignored", walletName: "Mine", registry: registry
        ))
        let channel = ClearSignerChannel(
            signerUrl: page.signerUrl,
            first: .ceremony(
                request: registerRequest,
                operationJson: ClearSignerCeremonyFixture.json(register),
                memberChallenge: nil
            )
        )
        let port = try #require(await channel.open())
        let socket = RawPage(port: port)
        #expect(await socket.connect())

        // 1. Create. The page is asked for `vela_createPasskey` and draws its
        //    own challenge; the wallet only checks what comes back.
        let first = try #require(await socket.hello(origin: page.origin, token: channel.token))
        let firstIntent = try #require(first["intent"] as? [String: Any])
        #expect(firstIntent["method"] as? String == "vela_createPasskey")
        #expect((first["context"] as? [String: Any])?["walletName"] as? String == "Mine")
        await socket.sendJSON(
            ["v": 1, "t": "result", "id": first["id"] as? String ?? ""]
                .merging(page.createAnswer()) { _, new in new }
        )
        guard case .ceremony(.registered(let registrationJson)) = await channel.ending() else {
            Issue.record("the page's registration was not accepted")
            return
        }
        let registration = try CoreJSON.object(registrationJson)
        #expect(registration["credential_id"] as? String == page.credentialHex)
        // Where the key lives from now on — stamped by the core, from the page
        // the wallet opened, never from anything the page said about itself.
        #expect(registration["signer_origin"] as? String == page.origin)

        // 2. The member proof, on the SAME connection. The wallet's own
        //    registry challenge rides along and the core demands equality.
        let challenge = Data(repeating: 0x42, count: 32)
        let member = page.memberProofOperation()
        let memberRequest = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(member),
            id: "ignored", walletName: "Mine", registry: registry
        ))
        let second = Task {
            await channel.send(.ceremony(
                request: memberRequest,
                operationJson: ClearSignerCeremonyFixture.json(member),
                memberChallenge: challenge
            ))
        }
        let secondIntent = try #require(await socket.message())
        #expect(secondIntent["t"] as? String == "intent")
        let asked = try #require(secondIntent["intent"] as? [String: Any])
        #expect(asked["method"] as? String == "vela_memberProof")
        let params = try #require((asked["params"] as? [[String: Any]])?.first)
        #expect(params["registry"] as? String == registry, "the page fetches from the wallet's registry")
        #expect(params["credentialId"] as? String == page.credentialB64)
        await socket.sendJSON(
            ["v": 1, "t": "result", "id": secondIntent["id"] as? String ?? ""]
                .merging(page.assertionAnswer(challenge: challenge)) { _, new in new }
        )
        guard case .ceremony(.asserted(let assertionJson)) = await second.value else {
            Issue.record("the member proof was not accepted")
            return
        }
        let assertion = try CoreJSON.object(assertionJson)
        #expect(assertion["credential_id"] as? String == page.credentialHex)
        // …and it assembles into the registry proof the machine reports.
        let proof = try CoreJSON.object(try registryBuildMemberProof(
            authenticatorDataHex: assertion["authenticator_data_hex"] as? String ?? "",
            clientDataJsonHex: assertion["client_data_json_hex"] as? String ?? "",
            signatureDerHex: assertion["signature_der_hex"] as? String ?? ""
        ))
        #expect(!proof.isEmpty)

        // 3. The flow ends: `bye`, then the close.
        channel.end()
        let bye = try #require(await socket.frame())
        #expect(((try? JSONSerialization.jsonObject(with: bye.payload)) as? [String: Any])?["t"] as? String == "bye")
        #expect(await socket.frame()?.opcode == 0x8)
    }

    /// A page that answered the first request and then went away has closed
    /// without signing the SECOND one — a decline, not a channel that could
    /// not be opened. The two get different sentences on screen, and telling
    /// somebody "the answer does not match" when they closed a tab is a lie
    /// about what they did.
    @Test func aPageThatLeavesBetweenRequestsIsADecline() async throws {
        let page = ClearSignerCeremonyFixture()
        let register = page.registerOperation()
        let request = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(register),
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = ClearSignerChannel(
            signerUrl: page.signerUrl,
            first: .ceremony(request: request,
                             operationJson: ClearSignerCeremonyFixture.json(register),
                             memberChallenge: nil)
        )
        let port = try #require(await channel.open())
        let socket = RawPage(port: port)
        #expect(await socket.connect())
        let intent = try #require(await socket.hello(origin: page.origin, token: channel.token))
        await socket.sendJSON(
            ["v": 1, "t": "result", "id": intent["id"] as? String ?? ""]
                .merging(page.createAnswer()) { _, new in new }
        )
        guard case .ceremony(.registered) = await channel.ending() else {
            Issue.record("the registration was not accepted")
            return
        }
        socket.close()
        // Give the closed socket a turn to be noticed.
        try await Task.sleep(for: .milliseconds(200))
        let member = page.memberProofOperation()
        let memberRequest = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(member),
            id: "y", walletName: "Mine", registry: "https://r.test"
        ))
        let ending = await channel.send(.ceremony(
            request: memberRequest,
            operationJson: ClearSignerCeremonyFixture.json(member),
            memberChallenge: Data(repeating: 1, count: 32)
        ))
        #expect(ending == .ceremony(.refused(refusal: .declined)))
        #expect(ClearSignerNotice(ending: ending) == .closed)
        channel.end()
    }

    /// A member proof over a challenge the page did NOT fetch for these
    /// inputs is refused — the one check that keeps "confirm this key joins
    /// your wallet" from being a signature over something else.
    @Test func aMemberProofOverAnotherChallengeIsRefused() async throws {
        let page = ClearSignerCeremonyFixture()
        let member = page.memberProofOperation()
        let request = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(member),
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = ClearSignerChannel(
            signerUrl: page.signerUrl,
            first: .ceremony(
                request: request,
                operationJson: ClearSignerCeremonyFixture.json(member),
                memberChallenge: Data(repeating: 0x01, count: 32)
            )
        )
        let port = try #require(await channel.open())
        let socket = RawPage(port: port)
        #expect(await socket.connect())
        let intent = try #require(await socket.hello(origin: page.origin, token: channel.token))
        await socket.sendJSON(
            ["v": 1, "t": "result", "id": intent["id"] as? String ?? ""]
                .merging(page.assertionAnswer(challenge: Data(repeating: 0x02, count: 32))) { _, new in new }
        )
        #expect(await channel.ending() == .ceremony(.refused(refusal: .wrongChallenge)))
        channel.end()
    }

    /// A page that answers from another origin is refused before the answer is
    /// used — the wallet checks the origin the AUTHENTICATOR signed, not the
    /// one the page writes beside it.
    @Test func anAnswerFromAnotherOriginIsRefused() async throws {
        let page = ClearSignerCeremonyFixture()
        let register = page.registerOperation()
        let request = try #require(clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(register),
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = ClearSignerChannel(
            signerUrl: page.signerUrl,
            first: .ceremony(request: request,
                             operationJson: ClearSignerCeremonyFixture.json(register),
                             memberChallenge: nil)
        )
        let port = try #require(await channel.open())
        let socket = RawPage(port: port)
        #expect(await socket.connect())
        let intent = try #require(await socket.hello(origin: page.origin, token: channel.token))
        var answer = page.createAnswer()
        // The signed clientDataJSON says somewhere else.
        let clientData = Data(
            #"{"type":"webauthn.create","challenge":"AAAA","origin":"https://evil.example","crossOrigin":false}"#.utf8
        )
        var registration = answer["registration"] as? [String: Any] ?? [:]
        registration["clientDataJSON"] = ClearSignerFixture.hex(clientData)
        answer["registration"] = registration
        await socket.sendJSON(
            ["v": 1, "t": "result", "id": intent["id"] as? String ?? ""].merging(answer) { _, new in new }
        )
        guard case .ceremony(.refused(.malformed)) = await channel.ending() else {
            Issue.record("an answer from another origin was accepted")
            return
        }
        channel.end()
    }
}

// MARK: - The tunnel requester

/// A tunnel socket that plays a script: what it hands the wallet, and what the
/// wallet handed it.
final class FakeTunnelSocket: ClearSignerSocket {
    private var inbound: [ClearSignerFrame]
    private(set) var sent: [ClearSignerFrame] = []
    private(set) var closed = false

    init(_ inbound: [ClearSignerFrame]) {
        self.inbound = inbound
    }

    func send(_ frame: ClearSignerFrame) async -> Bool {
        sent.append(frame)
        return true
    }

    /// Runs out rather than hanging: a conversation that could not open a
    /// frame must say `unavailable`, not wait forever inside a test.
    func receive() async -> ClearSignerFrame? {
        inbound.isEmpty ? nil : inbound.removeFirst()
    }

    func close() { closed = true }
}

@MainActor
struct ClearSignerTunnelTests {

    /// One case of the shared session vectors.
    struct Vector {
        let code: String
        let rk: String
        let signerHello: String
        let requesterHello: String
        let requesterSecret: Data
        let requesterNonce: Data
        let requesterPublicKey: Data
        let app: String
        /// `(fromRequester, plaintext, sealed)` in session order.
        let messages: [(fromRequester: Bool, plaintext: Data, sealed: Data)]
    }

    static let vectors: [Vector] = {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("rust/crates/vela-core/tests/clear-signer/secure-session.json")
        let json = (try? JSONSerialization.jsonObject(with: Data(contentsOf: url))) as? [String: Any]
        let cases = (json?["cases"] as? [[String: Any]] ?? []).filter {
            $0["label"] as? String == "vela-tunnel/1"
        }
        return cases.map { item in
            let requester = item["requester"] as? [String: Any] ?? [:]
            let signer = item["signer"] as? [String: Any] ?? [:]
            return Vector(
                code: item["code"] as? String ?? "",
                rk: item["rk"] as? String ?? "",
                signerHello: signer["hello"] as? String ?? "",
                requesterHello: requester["hello"] as? String ?? "",
                requesterSecret: unhex(requester["secretHex"] as? String ?? ""),
                requesterNonce: unhex(requester["nonceHex"] as? String ?? ""),
                requesterPublicKey: unhex(requester["publicKeyHex"] as? String ?? ""),
                app: requester["app"] as? String ?? "",
                messages: (item["messages"] as? [[String: Any]] ?? []).map { message in
                    (
                        fromRequester: message["from"] as? String == "requester",
                        plaintext: Data((message["plaintext"] as? String ?? "").utf8),
                        sealed: unhex(message["sealedHex"] as? String ?? "")
                    )
                }
            )
        }
    }()

    static func unhex(_ text: String) -> Data { UserOpSpine.unhex(text) }

    /// The wallet's side of the tunnel session, byte for byte against the
    /// vectors the page is pinned to: the requester's key, the `rk` its link
    /// carries, the six digits both screens show, and every message sealed and
    /// opened in order.
    ///
    /// The session itself is the core's — which is exactly why this is worth
    /// pinning here: the shell drives it, and a shell that drove it in the
    /// wrong ROLE, with the wrong label, or without the counter would still
    /// compile and would quietly produce a channel the page cannot read.
    @Test func theRequesterSideMatchesTheSharedVectors() throws {
        #expect(!Self.vectors.isEmpty, "the shared session vectors did not load")
        for vector in Self.vectors {
            let handshake = try ClearSignerHandshake(
                secret: vector.requesterSecret, nonce: vector.requesterNonce
            )
            #expect(handshake.publicKey() == vector.requesterPublicKey)
            #expect(clearSignerKeyFingerprint(publicKey: handshake.publicKey()) == vector.rk)
            // The hello the page reads, field for field.
            // The vectors predate the peer's mark (075): no icon, no field.
            let ours = try CoreJSON.object(handshake.hello(app: vector.app, icon: nil))
            let theirs = try CoreJSON.object(vector.requesterHello)
            #expect(ours["pk"] as? String == theirs["pk"] as? String)
            #expect(ours["nonce"] as? String == theirs["nonce"] as? String)
            #expect(ours["role"] as? String == "requester")
            #expect(ours["app"] as? String == vector.app)

            let session = try handshake.complete(peerHello: vector.signerHello, tunnel: true)
            #expect(session.code() == vector.code, "the two screens would show different digits")
            for message in vector.messages {
                if message.fromRequester {
                    #expect(session.seal(plaintext: message.plaintext, msgId: nil) == message.sealed)
                } else {
                    #expect(try session.open(sealed: message.sealed, msgId: nil) == message.plaintext)
                }
            }
        }
    }

    /// The pairing link the QR carries: the page's own address, the tunnel, the
    /// room, and the `rk` that lets the PAGE refuse a stand-in wallet.
    @Test func thePairingLinkCarriesTheRoomAndTheRequestersFingerprint() throws {
        let vector = try #require(Self.vectors.first)
        var draws = [Data(repeating: 0x07, count: 16), vector.requesterSecret, vector.requesterNonce]
        let conversation = try #require(ClearSignerTunnelConversation(
            signerUrl: "https://sign.getvela.app/",
            tunnel: "wss://tunnel.getvela.app",
            app: vector.app,
            random: { _ in draws.removeFirst() },
            openSocket: { _ in nil }
        ))
        #expect(conversation.link.hasPrefix("https://sign.getvela.app/"))
        #expect(conversation.link.contains("rk=\(vector.rk)"))
        #expect(conversation.link.contains("&v=1"))
        let room = try #require(clearSignerTunnelRoom(random: Data(repeating: 0x07, count: 16)))
        #expect(conversation.link.contains("room=\(room)"))
        #expect(conversation.roomUrl == "wss://tunnel.getvela.app/v1/rooms/\(room)?role=requester")
    }

    /// Against a tunnel that only forwards frames: the wallet waits for
    /// `joined`, answers the page's hello with its own, shows the code the
    /// vectors pin — and then seals its request as a BINARY frame in the
    /// requester's direction, so nothing readable ever reaches the tunnel.
    @Test func theWalletPairsThroughATunnelAndSealsEverythingAfterTheHandshake() async throws {
        let vector = try #require(Self.vectors.first)
        var draws = [Data(repeating: 0x07, count: 16), vector.requesterSecret, vector.requesterNonce]
        let socket = FakeTunnelSocket([
            .text(#"{"v":1,"relay":"joined"}"#),
            .text(vector.signerHello),
            // The vectors' first signer message, under the same key: opening
            // it is the proof that this end derived the session correctly.
            .binary(vector.messages.first { !$0.fromRequester }?.sealed ?? Data()),
        ])
        let conversation = try #require(ClearSignerTunnelConversation(
            signerUrl: "https://sign.getvela.app/",
            tunnel: "wss://tunnel.getvela.app",
            app: vector.app,
            random: { _ in draws.removeFirst() },
            openSocket: { _ in socket },
            // The vectors' own request id, so the vectors' own answer is the
            // one this request is waiting for.
            nextId: { "e6f3" }
        ))

        #expect(await conversation.pair() == vector.code)
        // Our hello went out as TEXT, and it is the one the vectors pin.
        guard case .text(let hello) = try #require(socket.sent.first) else {
            Issue.record("the wallet's hello was not a text frame")
            return
        }
        let ourKey = try CoreJSON.object(hello)["pk"] as? String
        let vectorKey = try CoreJSON.object(vector.requesterHello)["pk"] as? String
        #expect(ourKey == vectorKey)

        // The request. The tunnel sees a binary frame whose IV says "requester
        // → signer, message 1" and nothing else.
        let ending = await conversation.send(.signature(
            request: #"{"id":"e6f3","intent":{"method":"personal_sign","params":[],"origin":""},"context":{}}"#,
            digest: Data(repeating: 9, count: 32),
            keys: [WalletKeyRecord(credentialId: "aa", publicKeyHex: "04" + String(repeating: "11", count: 64))]
        ))
        guard case .binary(let sealed) = try #require(socket.sent.last) else {
            Issue.record("the intent was not sealed into a binary frame")
            return
        }
        #expect(sealed.prefix(4) == Data("P2C.".utf8))
        #expect(sealed.count > 12 + 16, "IV, ciphertext and tag")
        #expect(!ClearSignerFixture.hex(sealed).contains("706572736f6e616c5f7369676e"),
                "the tunnel must never see `personal_sign` in the clear")

        // The answer opened — and then the core refused it, because the
        // vectors' `result` carries no signature. What matters here is that it
        // was OPENED at all: a wrong key, label, direction or counter would
        // have left the conversation waiting and answered `unavailable`.
        guard case .outcome(.refused(let refusal)) = ending else {
            Issue.record("the page's sealed answer did not reach the core: \(ending)")
            return
        }
        #expect(ClearSignerNotice(refusal) == .mismatch)

        // `bye` is sealed too, in the same direction and the next counter.
        conversation.end()
        await Task.yield()
        guard case .binary(let farewell) = try #require(socket.sent.last) else {
            Issue.record("the bye was not sealed")
            return
        }
        #expect(farewell.prefix(4) == Data("P2C.".utf8))
    }

    /// A tunnel that never pairs the two ends: no code, and the wallet says so
    /// rather than sitting on a spinner — the sheet offers "this device" again.
    @Test func aTunnelThatNeverPairsAnswersNothing() async throws {
        let vector = try #require(Self.vectors.first)
        var draws = [Data(repeating: 0x07, count: 16), vector.requesterSecret, vector.requesterNonce]
        let conversation = try #require(ClearSignerTunnelConversation(
            signerUrl: "https://sign.getvela.app/",
            tunnel: "wss://tunnel.getvela.app",
            app: vector.app,
            random: { _ in draws.removeFirst() },
            openSocket: { _ in FakeTunnelSocket([.text(#"{"v":1,"relay":"joined"}"#)]) }
        ))
        #expect(await conversation.pair() == nil)
    }

    /// The page's refusal codes, in the core's own vocabulary: a person who
    /// did not slide is a decline (never an error), and everything else keeps
    /// the page's reason.
    @Test func thePagesRefusalCodesKeepTheirMeaning() {
        #expect(ClearSignerTunnelConversation.refusal(code: "user_rejected") == .declined)
        #expect(ClearSignerTunnelConversation.refusal(code: "") == .declined)
        #expect(ClearSignerTunnelConversation.refusal(code: "refused") == .pageRefused(code: "refused"))
        #expect(ClearSignerNotice(ClearSignerTunnelConversation.refusal(code: "refused")) == .refused)
        #expect(ClearSignerNotice(ClearSignerTunnelConversation.refusal(code: "")) == .closed)
    }
}

// MARK: - The choosers

@MainActor
struct ClearSignerChooserTests {
    private var loc: Loc { Loc(overrideTag: "zh", preferredLanguages: []) }

    /// The create key-method picker and the sign-in method sheet both render
    /// `KeyMethod.allCases`, so this is what puts the fourth route on both of
    /// them (spec 075 SC-001): four routes, no `auto`, each with a title and a
    /// line in the person's language.
    @Test func theCreateAndSignInChoosersListFourRoutes() {
        #expect(KeyMethod.allCases == [.platform, .hybrid, .securityKey, .clearSigner])
        let titles = KeyMethod.allCases.map { loc.t(methodCopy($0).title) }
        #expect(titles.last == "清晰签名器")
        #expect(Set(titles).count == 4, "two routes share a title")
        #expect(loc.t(methodCopy(.clearSigner).body) == "在独立的页面上核对并签名——所见即所签。")
        // A method that arrives from the core by name resolves to the route.
        #expect(KeyMethod(rawValue: "clear_signer") == .clearSigner)
    }

    /// The signing sheet's "Sign with" has an `auto`, so it lists FIVE — and
    /// the Clear Signer is the only one with a line under it, because the
    /// other four say where a key is and this one says what it does.
    @Test func theSigningSheetListsFiveWithTheClearSignersLine() throws {
        let offered = try #require(SignPrefViewWire.initial).offered
        #expect(offered == ["auto", "platform", "hybrid", "security_key", "clear_signer"])
        let titles = offered.compactMap { SigningLive.signMethodTitle($0, loc: loc) }
        #expect(titles.count == 5)
        #expect(titles.last == "清晰签名器")
        #expect(SigningLive.signMethodDetail("clear_signer", loc: loc) != nil)
        #expect(offered.dropLast().allSatisfy { SigningLive.signMethodDetail($0, loc: loc) == nil })
    }

    /// Settings' tunnel row, beside the page row: the value is "official" or
    /// the host, and every verdict under the field is the core's.
    @Test func settingsShowsTheTunnelBesideTheSignerPage() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SignPrefExecutor(store: store)
        let core = SignPrefCore()
        let base = SettingsFixtures.build(.st1, loc: loc)

        var view = try Self.run(core, executor, ["type": "refresh"])
        #expect(view.tunnelUrlIsDefault && view.tunnelUrl == clearSignerDefaultTunnel())
        var model = SettingsLive.withSignPref(view, on: base, loc: loc)
        #expect(model.sections.flatMap(\.rows).first { $0.id == SettingsFixtures.tunnelRow }?.value == "官方")
        #expect(model.tunnel?.reset == nil, "nothing to put back while it is the official one")

        // Refused: nothing stored, and the sheet says why.
        view = try Self.run(core, executor, ["type": "tunnel_url_submitted", "text": "ws://192.168.1.4/"])
        #expect(view.tunnelUrlError == "insecure")
        #expect(store.readString(VelaStore.Key.clearSignerTunnel) == nil)
        model = SettingsLive.withSignPref(view, on: base, loc: loc)
        #expect(model.tunnel?.error == "请使用 wss:// 地址，或本机回环地址上的 ws://。")

        view = try Self.run(core, executor, ["type": "tunnel_url_submitted", "text": "wss://tunnel.example/"])
        #expect(view.tunnelUrlError == nil && !view.tunnelUrlIsDefault)
        #expect(store.readString(VelaStore.Key.clearSignerTunnel) == "wss://tunnel.example")
        model = SettingsLive.withSignPref(view, on: base, loc: loc)
        #expect(model.sections.flatMap(\.rows)
            .first { $0.id == SettingsFixtures.tunnelRow }?.value == "tunnel.example")
        #expect(model.tunnel?.reset == "使用官方隧道")

        // A second launch reads it back under its own key.
        let again = try Self.run(SignPrefCore(), SignPrefExecutor(store: store), ["type": "refresh"])
        #expect(again.tunnelUrl == "wss://tunnel.example" && !again.tunnelUrlIsDefault)

        view = try Self.run(core, executor, ["type": "tunnel_url_reset"])
        #expect(view.tunnelUrlIsDefault)
        #expect(store.readString(VelaStore.Key.clearSignerTunnel) == nil)
    }

    /// Drives the machine the way `CoreStore` does — dispatch, perform each
    /// operation against the store, resolve.
    static func run(
        _ core: SignPrefCore, _ executor: SignPrefExecutor, _ event: [String: Any]
    ) throws -> SignPrefViewWire {
        var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
        var pending = result["effects"] as? [[String: Any]] ?? []
        while !pending.isEmpty {
            let effect = pending.removeFirst()
            let id = (effect["id"] as? NSNumber)?.uint64Value ?? 0
            let answer = executor.perform(effect["operation"] as? [String: Any] ?? [:])
            result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: answer))
            pending += result["effects"] as? [[String: Any]] ?? []
        }
        return try CoreJSON.decode(SignPrefViewWire.self, from: CoreJSON.object(core.view()))
    }
}

// MARK: - `signer_origin`: where a key lives

@MainActor
struct SignerOriginTests {

    /// The account record carries it, whole, through a write and a read — and
    /// it reaches `sign_route` in the shape the core reads.
    ///
    /// **This is the one that loses a wallet if it drifts.** A key minted
    /// behind somebody's own signer page is reachable nowhere else; a mapper
    /// that dropped `signer_origin` would send its ceremony to a platform
    /// sheet that cannot see the key, and the person would be told their own
    /// passkey does not exist.
    @Test func anAccountRecordRoundTripsSignerOriginAndItReachesTheRoute() async throws {
        let store = AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let origin = "https://sign.example"
        await store.saveAccount([
            "id": "cred-1", "address": address, "name": "Mine",
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "keys": [[
                "credential_id": "cred-1",
                "public_key_hex": "04" + String(repeating: "11", count: 64),
                "name": "Mine", "transports": "internal", "signer_origin": origin,
            ]],
        ])
        let reloaded = try #require(await store.loadAccounts().first)
        let key = try #require((reloaded["keys"] as? [[String: Any]])?.first)
        #expect(key["signer_origin"] as? String == origin, "the record forgot where its key lives")

        let port = SendAccountPort(accounts: store)
        let routes = await port.keyRoutesJson(of: address)
        let decoded = (try? JSONSerialization.jsonObject(with: Data(routes.utf8))) as? [[String: Any]]
        #expect(decoded?.first?["signer_origin"] as? String == origin,
                "the route the core reads lost where the key lives: \(routes)")

        // `auto` follows the key to its page — the core's ruling, read here
        // through the same door the spine uses.
        let auto = try #require(signRoute(deviceKeysJson: routes, method: "auto"))
        let route = try CoreJSON.decoder.decode(SignRouteWire.self, from: Data(auto.utf8))
        #expect(route.method == "clear_signer")
        #expect(route.signerOrigin == origin)
        #expect(route.credentialId == "cred-1")
    }

    /// A wallet whose key is NOT behind a page routes as it always did:
    /// `auto` answers nothing, and the ceremony is the platform sheet's.
    @Test func anOrdinaryKeyIsNotRoutedToAPage() async throws {
        let routes = #"[{"credential_id":"cred-1","transports":"internal"}]"#
        #expect(signRoute(deviceKeysJson: routes, method: "auto") == nil)
        let named = try #require(signRoute(deviceKeysJson: routes, method: "platform"))
        let route = try CoreJSON.decoder.decode(SignRouteWire.self, from: Data(named.utf8))
        #expect(route.method == "platform")
        #expect(route.signerOrigin.isEmpty, "absent on the wire, empty here")
    }

    /// The spine opens the page the KEY lives behind, not the one Settings
    /// names — for a send nobody asked about "Sign with" at all.
    @Test func theSpineOpensThePageTheKeyLivesBehind() async throws {
        let fixture = ClearSignerFixture()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.routesJson = ClearSignerCeremonyFixture.json([
            "credential_id": fixture.credentialHex,
            "transports": "internal",
            "signer_origin": "https://sign.example",
        ]).withSquareBrackets
        let signer = CountingSigner()
        let page = ScriptedClearSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(clearSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let relay = RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { signer })
        // "Automatic" — nobody chose the Clear Signer, the KEY did.
        spine.signMethod = { "auto" }
        spine.clearSigner = page

        _ = try await spine.signMessage(
            chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
            asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
        )
        #expect(signer.calls == 0, "a key behind a page must never reach the platform sheet")
        #expect(page.asked.first?.page == "https://sign.example")
    }
}

private extension String {
    /// The one-element array `sign_route` reads.
    var withSquareBrackets: String { "[\(self)]" }
}

// MARK: - The executor

/// The Clear Signer as the onboarding executor reaches it, scripted.
@MainActor
final class ScriptedCeremonyPort: ClearSignerCeremonyPort {
    var answer: (_ operationJson: String) -> ClearSignerCeremonyStep
    private(set) var asked: [(operationJson: String, registry: String, walletName: String, challenge: Data?, page: String?)] = []
    private(set) var ended = 0

    init(answer: @escaping (_ operationJson: String) -> ClearSignerCeremonyStep) {
        self.answer = answer
    }

    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String?
    ) async -> ClearSignerCeremonyStep {
        asked.append((operationJson, registry, walletName, expectedMemberChallenge, page))
        return answer(operationJson)
    }

    func endFlow() { ended += 1 }
}

@MainActor
struct ClearSignerExecutorTests {

    private func executor(_ port: ScriptedCeremonyPort) -> OnboardingExecutor {
        OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test"),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            clearSigner: port,
            walletName: { "Mine" }
        )
    }

    /// A `register_passkey` with `method = clear_signer` goes to the page, and
    /// the core's `Registration` — `signer_origin` and all — is reported as
    /// the result a platform ceremony would have given.
    @Test func aCreateOnTheClearSignerIsReportedAsPasskeyRegistered() async throws {
        let page = ClearSignerCeremonyFixture()
        let registration = ClearSignerCeremonyFixture.json([
            "credential_id": page.credentialHex,
            "attestation_object_hex": ClearSignerCeremonyFixture.attestationObjectHex,
            "client_data_json_hex": "00", "authenticator_attachment": "platform",
            "transports": "internal", "signer_origin": page.origin,
        ])
        let port = ScriptedCeremonyPort { _ in .registered(registration) }
        let answer = try CoreJSON.object(await executor(port).perform(page.registerOperation()))
        #expect(answer["type"] as? String == "passkey_registered")
        let reported = try #require(answer["registration"] as? [String: Any])
        #expect(reported["signer_origin"] as? String == page.origin)
        #expect(reported["credential_id"] as? String == page.credentialHex)
        // The page is told the wallet's own name and its own registry.
        #expect(port.asked.first?.walletName == "Mine")
        #expect(port.asked.first?.registry == "https://r.test")
        // The operation goes over verbatim, so nothing is copied wrongly.
        #expect(try CoreJSON.object(port.asked.first?.operationJson ?? "{}")["name"] as? String == "Mine")
    }

    /// A page that was closed is a CANCEL — the same as a dismissed system
    /// sheet, and never an error alert (contract §5).
    @Test func aClosedPageIsACancelAndARefusalCarriesItsSentence() async throws {
        let page = ClearSignerCeremonyFixture()
        let closed = ScriptedCeremonyPort { _ in .failed(kind: .cancelled, message: nil) }
        let answer = try CoreJSON.object(await executor(closed).perform(page.registerOperation()))
        #expect(answer["type"] as? String == "passkey_failed")
        #expect(answer["kind"] as? String == "cancelled")
        #expect(answer["message"] is NSNull, "a cancel carries no words to alert with")

        let refused = ScriptedCeremonyPort { _ in .failed(kind: .other, message: "the page said no") }
        let other = try CoreJSON.object(await executor(refused).perform(page.registerOperation()))
        #expect(other["kind"] as? String == "other")
        #expect(other["message"] as? String == "the page said no")
    }

    /// Chosen where no page can be opened — a preview, the gallery — fails
    /// closed rather than silently signing with the platform authenticator.
    @Test func theRouteWithNoPageFailsClosed() async throws {
        let page = ClearSignerCeremonyFixture()
        let executor = OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test"),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            clearSigner: nil
        )
        let answer = try CoreJSON.object(await executor.perform(page.registerOperation()))
        #expect(answer["type"] as? String == "passkey_failed")
        #expect(answer["kind"] as? String == "not_supported")
    }

    /// Every other route is untouched — and this is checked on the DECISION
    /// rather than by running the other route, because running it raises the
    /// system passkey sheet and a unit test has nobody to answer it (this test
    /// hung the whole suite when it was written the other way round).
    @Test func anotherRouteNeverReachesThePage() throws {
        var operation = ClearSignerCeremonyFixture().registerOperation()
        #expect(OnboardingExecutor.routesToTheClearSigner(operation))
        for other in ["platform", "hybrid", "security_key", "", "something_else"] {
            operation["method"] = other
            #expect(!OnboardingExecutor.routesToTheClearSigner(operation),
                    "\(other) would have opened a page")
        }
        operation.removeValue(forKey: "method")
        #expect(!OnboardingExecutor.routesToTheClearSigner(operation),
                "an operation with no method must route as it always did")
    }

    /// The re-publish's LIVE member proof (recovery's third signature) goes to
    /// the page the CORE named for that member — `RegistryPublishMember`'s own
    /// `signer_origin`, never a lookup here and never the page Settings holds.
    /// A member with no page is not routed to one at all.
    @Test func aPublishMemberSignsOnThePageTheCoreNamedForIt() throws {
        let group = "04" + String(repeating: "ab", count: 64)
        let behindAPage = PublishMember(json: [
            "credential_id": "cred-1", "public_key_hex": "04" + String(repeating: "11", count: 64),
            "attestation_hex": "beef", "transports": "internal",
            "signer_origin": "https://sign.example",
        ])
        #expect(behindAPage.signerOrigin == "https://sign.example",
                "the member's own page was dropped on the way in")
        let routed = OnboardingExecutor.memberProofOperation(behindAPage, groupPublicKey: group)
        #expect(routed["method"] as? String == "clear_signer")
        #expect(routed["signer_origin"] as? String == "https://sign.example")
        #expect(routed["credential_id"] as? String == "cred-1")
        #expect(routed["group_public_key_hex"] as? String == group)
        // The core reads it as the ceremony it is, from this very JSON.
        let request = clearSignerCeremonyRequest(
            operationJson: ClearSignerCeremonyFixture.json(routed),
            id: "x", walletName: "Mine", registry: "https://r.test"
        )
        #expect(request != nil)

        let ordinary = PublishMember(json: [
            "credential_id": "cred-2", "public_key_hex": "04" + String(repeating: "22", count: 64),
        ])
        #expect(ordinary.signerOrigin.isEmpty)
        let unrouted = OnboardingExecutor.memberProofOperation(ordinary, groupPublicKey: group)
        #expect(unrouted["method"] as? String == "", "a key that is not behind a page must not be sent to one")
    }
}

/// The two operations whose outside world is the UI — neither is reached here.
@MainActor
private final class NoDeps: OnboardingExecutorDeps {
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool { false }
    func complete(mode: [String: Any]) async {}
}
