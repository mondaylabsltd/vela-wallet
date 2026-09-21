//
//  ClearSignerTests.swift
//  VelaWalletTests
//
//  Spec 071 on iOS: the Clear Signer's loopback channel, the spine's branch
//  and the two preferences, with nothing mocked that the core decides.
//
//  The channel is spoken to the way the page speaks to it — a TCP socket, an
//  HTTP upgrade with an `Origin`, masked frames, the token, the answer — and
//  the answer is signed by a real P-256 key standing in for the passkey, so
//  the verdicts below are the core's own, run for real.
//

import CryptoKit
import Foundation
import Network
import SafariServices
import SwiftUI
import Testing
import UIKit
import VelaCore
@testable import VelaWallet

// MARK: - A passkey and a page

/// A wallet with one real P-256 key, and what a Clear Signer page answers for
/// a digest when that key signs it.
struct ClearSignerFixture {
    let key = P256.Signing.PrivateKey()
    let credential = Data([0x11, 0x22, 0x33, 0x44])
    let signerUrl = "https://sign.getvela.app/"
    let origin = "https://sign.getvela.app"
    let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    var credentialHex: String { credential.map { String(format: "%02x", $0) }.joined() }

    var keys: [WalletKeyRecord] {
        [WalletKeyRecord(credentialId: credentialHex, publicKeyHex: Self.hex(key.publicKey.x963Representation))]
    }

    /// `{credentialId, signature (r‖s), authenticatorData, clientDataJSON}`
    /// for `digest`, user-verified — what the page's ceremony returns.
    func result(for digest: Data) -> [String: Any] {
        var authenticatorData = Data(SHA256.hash(data: Data("getvela.app".utf8)))
        authenticatorData.append(contentsOf: [0x05, 0, 0, 0, 7])
        let clientData = Data(
            #"{"type":"webauthn.get","challenge":"\#(Self.base64url(digest))","origin":"\#(origin)","crossOrigin":false}"#.utf8
        )
        var signed = authenticatorData
        signed.append(Data(SHA256.hash(data: clientData)))
        // CryptoKit hashes what it is given: the signature is over
        // sha256(authenticatorData ‖ sha256(clientDataJSON)), as WebAuthn's.
        let signature = try! key.signature(for: signed)
        return [
            "credentialId": Self.base64url(credential),
            "signature": "0x" + Self.hex(signature.rawRepresentation),
            "authenticatorData": "0x" + Self.hex(authenticatorData),
            "clientDataJSON": "0x" + Self.hex(clientData),
        ]
    }

    /// The page's `personal_sign` request, built by the core.
    @MainActor
    func messageRequest() throws -> String {
        try clearSignerRequest(
            input: UserOpSpine.clearSignerInput(
                asked: .init(method: "personal_sign",
                              paramsJson: #"["0x68656c6c6f","\#(account)"]"#,
                              origin: "https://app.example"),
                chainId: 100, account: account, accountName: "Mine", keys: keys, calls: []
            ),
            draft: nil
        )
    }

    static func hex(_ data: Data) -> String { data.map { String(format: "%02x", $0) }.joined() }

    static func base64url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

/// A page, as far as the listener can tell: a TCP socket speaking the client
/// side of a WebSocket by hand.
@MainActor
final class RawPage {
    private let connection: NWConnection
    private let queue = DispatchQueue(label: "vela.tests.raw-page")
    private var inbox = Data()

    init(port: UInt16, host: NWEndpoint.Host = "127.0.0.1") {
        connection = NWConnection(host: host, port: NWEndpoint.Port(rawValue: port)!, using: .tcp)
    }

    /// `true` once connected; `false` when refused, or nothing within `seconds`.
    func connect(within seconds: Double = 3) async -> Bool {
        let once = Once()
        return await withCheckedContinuation { continuation in
            connection.stateUpdateHandler = { state in
                switch state {
                case .ready: once.run { continuation.resume(returning: true) }
                case .failed, .waiting, .cancelled: once.run { continuation.resume(returning: false) }
                default: break
                }
            }
            connection.start(queue: queue)
            queue.asyncAfter(deadline: .now() + seconds) {
                once.run { continuation.resume(returning: false) }
            }
        }
    }

    func send(_ data: Data) async {
        await withCheckedContinuation { (continuation: CheckedContinuation<Void, Never>) in
            connection.send(content: data, completion: .contentProcessed { _ in continuation.resume() })
        }
    }

    func close() { connection.cancel() }

    /// The upgrade, from `origin`. Answers the server's status line.
    func upgrade(origin: String) async -> String? {
        await send(Data((
            "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                + "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n"
                + "Origin: \(origin)\r\n\r\n"
        ).utf8))
        let head = await read { bytes in
            bytes.range(of: Data("\r\n\r\n".utf8)).map { $0.upperBound - bytes.startIndex }
        }
        return head.flatMap { String(data: $0, encoding: .utf8)?.components(separatedBy: "\r\n").first }
    }

    /// One masked text frame — every client frame is masked (RFC 6455 §5.1).
    func sendText(_ text: String) async {
        let payload = Array(text.utf8)
        let mask: [UInt8] = [0x37, 0xfa, 0x21, 0x3d]
        var frame: [UInt8] = [0x81]
        if payload.count < 126 {
            frame.append(0x80 | UInt8(payload.count))
        } else {
            frame.append(0x80 | 126)
            frame += [UInt8(payload.count >> 8), UInt8(payload.count & 0xFF)]
        }
        frame += mask
        frame += payload.enumerated().map { $0.element ^ mask[$0.offset % 4] }
        await send(Data(frame))
    }

    func sendJSON(_ object: [String: Any]) async {
        let data = try! JSONSerialization.data(withJSONObject: object)
        await sendText(String(decoding: data, as: UTF8.self))
    }

    /// The next server frame: its opcode and payload, or `nil` when the socket
    /// ended first.
    func frame() async -> (opcode: UInt8, payload: Data)? {
        guard let frame = await read(until: Self.frameLength) else { return nil }
        let bytes = [UInt8](frame)
        let short = Int(bytes[1] & 0x7F)
        let start = short == 126 ? 4 : (short == 127 ? 10 : 2)
        return (bytes[0] & 0x0F, Data(bytes[start...]))
    }

    /// The server's next text message, parsed.
    func message() async -> [String: Any]? {
        guard let frame = await frame(), frame.opcode == 0x1 else { return nil }
        return (try? JSONSerialization.jsonObject(with: frame.payload)) as? [String: Any]
    }

    /// The upgrade and the hello: what a page does before it is given anything.
    func hello(origin: String, token: String) async -> [String: Any]? {
        guard await upgrade(origin: origin)?.hasPrefix("HTTP/1.1 101") == true else { return nil }
        await sendJSON(["v": 1, "t": "hello", "token": token])
        return await message()
    }

    private static func frameLength(_ data: Data) -> Int? {
        let bytes = [UInt8](data)
        guard bytes.count >= 2 else { return nil }
        var length = Int(bytes[1] & 0x7F)
        var at = 2
        if length == 126 {
            guard bytes.count >= 4 else { return nil }
            length = Int(bytes[2]) << 8 | Int(bytes[3])
            at = 4
        } else if length == 127 {
            guard bytes.count >= 10 else { return nil }
            length = bytes[2..<10].reduce(0) { $0 << 8 | Int($1) }
            at = 10
        }
        return bytes.count >= at + length ? at + length : nil
    }

    private func read(until enough: (Data) -> Int?) async -> Data? {
        while true {
            if let count = enough(inbox) {
                let out = Data(inbox.prefix(count))
                inbox = Data(inbox.dropFirst(count))
                return out
            }
            guard let chunk = await chunk() else { return nil }
            inbox.append(chunk)
        }
    }

    private func chunk() async -> Data? {
        await withCheckedContinuation { continuation in
            connection.receive(minimumIncompleteLength: 1, maximumLength: 65_536) { data, _, _, _ in
                continuation.resume(returning: (data?.isEmpty ?? true) ? nil : data)
            }
        }
    }
}

/// Resumes a continuation once, from whichever queue gets there first.
final class Once: @unchecked Sendable {
    private let lock = NSLock()
    private var done = false

    func run(_ body: () -> Void) {
        lock.lock()
        defer { lock.unlock() }
        guard !done else { return }
        done = true
        body()
    }
}

// MARK: - The channel

@MainActor
struct ClearSignerChannelTests {

    private func channel(
        _ fixture: ClearSignerFixture, digest: Data, timeout: TimeInterval = ClearSignerChannel.defaultTimeout
    ) throws -> ClearSignerChannel {
        ClearSignerChannel(
            signerUrl: fixture.signerUrl, requestJson: try fixture.messageRequest(),
            digest: digest, keys: fixture.keys, timeout: timeout
        )
    }

    /// The whole conversation, as the page has it: upgrade from the signer's
    /// origin, hello with the token, one intent, one answer — and an answer
    /// over this digest by this wallet's key is the core's `accepted`, which
    /// builds the same EIP-1271 envelope a passkey's assertion does.
    @Test func thePageWithTheRightOriginAndTokenIsHeardAndItsSignatureAccepted() async throws {
        let fixture = ClearSignerFixture()
        let digest = Data(repeating: 0xAB, count: 32)
        let channel = try channel(fixture, digest: digest)
        let port = try #require(await channel.open())

        let launch = try #require(channel.launchUrl?.absoluteString)
        #expect(launch.hasPrefix("https://sign.getvela.app/sign.html?ch=ws#"))
        #expect(launch.hasSuffix("p=\(port)&t=\(channel.token)"))
        #expect(Data(base64Encoded: channel.token.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/") + "==")?.count == 16, "128 bits")

        let page = RawPage(port: port)
        #expect(await page.connect())
        let intent = try #require(await page.hello(origin: fixture.origin, token: channel.token))
        #expect(intent["t"] as? String == "intent")
        #expect((intent["intent"] as? [String: Any])?["method"] as? String == "personal_sign")
        #expect((intent["intent"] as? [String: Any])?["origin"] as? String == "https://app.example")
        let id = try #require(intent["id"] as? String)
        #expect(id == channel.id)

        await page.sendJSON(["v": 1, "t": "result", "id": id, "result": fixture.result(for: digest)])
        let ending = await channel.ending()
        guard case .outcome(.accepted(let credentialIdHex, let assertion)) = ending else {
            Issue.record("expected an accepted answer, got \(ending)")
            return
        }
        #expect(credentialIdHex == fixture.credentialHex)
        #expect(assertion.signatureDer.first == 0x30, "DER, as the Safe envelope takes")
        let envelope = try eip1271Signature(assertion: assertion, credentialId: credentialIdHex, keys: fixture.keys)
        #expect(!envelope.isEmpty)
        // The conversation is over and the page is told so.
        #expect(await page.frame()?.opcode == 0x8)
    }

    /// A stranger — another page on this device that found the port, a tab
    /// holding a spent token — is turned away WITHOUT ending the request: it
    /// cannot cancel a signature somebody else is about to make.
    @Test func strangersAreTurnedAwayAndTheRequestStaysOpen() async throws {
        let fixture = ClearSignerFixture()
        let digest = Data(repeating: 0xCD, count: 32)
        let channel = try channel(fixture, digest: digest)
        let port = try #require(await channel.open())

        let foreign = RawPage(port: port)
        #expect(await foreign.connect())
        #expect(await foreign.upgrade(origin: "https://evil.example")?.hasPrefix("HTTP/1.1 403") == true)

        let wrongToken = RawPage(port: port)
        #expect(await wrongToken.connect())
        #expect(await wrongToken.upgrade(origin: fixture.origin)?.hasPrefix("HTTP/1.1 101") == true)
        await wrongToken.sendJSON(["v": 1, "t": "hello", "token": "not-the-token"])
        #expect(await wrongToken.frame()?.opcode == 0x8, "closed, and given nothing")

        // The page the wallet opened still gets the request, and its answer.
        let page = RawPage(port: port)
        #expect(await page.connect())
        let intent = try #require(await page.hello(origin: fixture.origin, token: channel.token))
        await page.sendJSON(["v": 1, "t": "result", "id": intent["id"] as? String ?? "",
                             "result": fixture.result(for: digest)])
        guard case .outcome(.accepted) = await channel.ending() else {
            Issue.record("a stranger ended the request")
            return
        }
    }

    /// A page that had the request and went away closed without signing —
    /// declined, which the sheet treats as a cancelled passkey sheet.
    @Test func aPageClosedAfterTheIntentIsDeclined() async throws {
        let fixture = ClearSignerFixture()
        let channel = try channel(fixture, digest: Data(repeating: 1, count: 32))
        let port = try #require(await channel.open())
        let page = RawPage(port: port)
        #expect(await page.connect())
        #expect(await page.hello(origin: fixture.origin, token: channel.token)?["t"] as? String == "intent")
        page.close()
        #expect(await channel.ending() == .outcome(.refused(refusal: .declined)))
    }

    /// Closing the tab: WebKit may keep a dismissed page's socket open, so
    /// the app closes its own end — and the core still decides. A page that
    /// never said hello changes nothing and the listener keeps waiting; one
    /// that had the request was closed without signing.
    @Test func closingTheTabEndsOnlyAPageThatHadTheRequest() async throws {
        let fixture = ClearSignerFixture()
        let digest = Data(repeating: 8, count: 32)
        let channel = try channel(fixture, digest: digest)
        let port = try #require(await channel.open())

        let silent = RawPage(port: port)
        #expect(await silent.connect())
        #expect(await silent.upgrade(origin: fixture.origin)?.hasPrefix("HTTP/1.1 101") == true)
        channel.pageClosed()
        // Still open: the page the wallet opens next is heard and answered.
        let page = RawPage(port: port)
        #expect(await page.connect())
        let intent = try #require(await page.hello(origin: fixture.origin, token: channel.token))
        #expect(intent["t"] as? String == "intent")

        channel.pageClosed()
        #expect(await channel.ending() == .outcome(.refused(refusal: .declined)))
    }

    /// An answer signed by a key that is not this wallet's is the core's
    /// refusal, never an assertion.
    @Test func anAnswerByAnotherKeyIsRefused() async throws {
        let fixture = ClearSignerFixture()
        let digest = Data(repeating: 2, count: 32)
        let channel = try channel(fixture, digest: digest)
        let port = try #require(await channel.open())
        let page = RawPage(port: port)
        #expect(await page.connect())
        let intent = try #require(await page.hello(origin: fixture.origin, token: channel.token))
        var stranger = ClearSignerFixture().result(for: digest)
        stranger["credentialId"] = ClearSignerFixture.base64url(Data([0x99]))
        await page.sendJSON(["v": 1, "t": "result", "id": intent["id"] as? String ?? "", "result": stranger])
        #expect(await channel.ending() == .outcome(.refused(refusal: .foreignKey)))
    }

    /// The core has no clock: five minutes are the shell's, and so is the
    /// cancel on the waiting sheet — which is a decline.
    @Test func theShellsClockAndCancelEndACeremony() async throws {
        let fixture = ClearSignerFixture()
        let slow = try channel(fixture, digest: Data(repeating: 3, count: 32), timeout: 0.2)
        #expect(await slow.open() != nil)
        #expect(await slow.ending() == .timedOut)

        let cancelled = try channel(fixture, digest: Data(repeating: 4, count: 32))
        let port = try #require(await cancelled.open())
        cancelled.cancel()
        #expect(await cancelled.ending() == .outcome(.refused(refusal: .declined)))
        // …and nothing answers on its port any more: refused, or — while the
        // listener's own close is still landing — accepted and dropped.
        let late = RawPage(port: port)
        if await late.connect(within: 1) {
            #expect(await late.upgrade(origin: fixture.origin) == nil)
        }

        // Send's cancel is a task cancel, and it ends the ceremony the same way.
        let abandoned = try channel(fixture, digest: Data(repeating: 6, count: 32))
        #expect(await abandoned.open() != nil)
        let waiting = Task { await abandoned.ending() }
        waiting.cancel()
        #expect(await waiting.value == .outcome(.refused(refusal: .declined)))
    }

    /// The listener is on 127.0.0.1 and nowhere else: from this device's own
    /// network address the port is closed.
    @Test func theListenerIsOnTheLoopbackOnly() async throws {
        let fixture = ClearSignerFixture()
        let channel = try channel(fixture, digest: Data(repeating: 5, count: 32))
        let port = try #require(await channel.open())
        #expect(await RawPage(port: port).connect())
        guard let address = Self.networkAddress() else { return }
        let stranger = RawPage(port: port, host: NWEndpoint.Host(address))
        if await stranger.connect(within: 2) {
            // Under the full parallel run another suite's wildcard listener
            // can hold the same port number on the network side, so a TCP
            // answer alone proves nothing; what answers must not be THIS
            // channel, which would switch protocols for the pinned origin.
            let status = await stranger.upgrade(origin: fixture.origin)
            #expect(status?.contains(" 101 ") != true,
                    "the channel answered on \(address): \(status ?? "")")
        }
        channel.cancel()
    }

    /// A non-loopback IPv4 address of this device, when it has one.
    private static func networkAddress() -> String? {
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return nil }
        defer { freeifaddrs(list) }
        for entry in sequence(first: first, next: { $0.pointee.ifa_next }) {
            guard let address = entry.pointee.ifa_addr, address.pointee.sa_family == UInt8(AF_INET),
                  entry.pointee.ifa_flags & UInt32(IFF_LOOPBACK) == 0,
                  entry.pointee.ifa_flags & UInt32(IFF_UP) != 0
            else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            guard getnameinfo(address, socklen_t(address.pointee.sa_len), &host, socklen_t(host.count),
                              nil, 0, NI_NUMERICHOST) == 0
            else { continue }
            return String(cString: host)
        }
        return nil
    }
}

// MARK: - The spine

/// The Clear Signer, scripted: records what it was asked, answers as told.
@MainActor
final class ScriptedClearSigner: ClearSignerPort {
    var answer: (_ digest: Data) -> ClearSignerChannel.Ending
    private(set) var asked: [(request: [String: Any], digest: Data)] = []

    init(answer: @escaping (_ digest: Data) -> ClearSignerChannel.Ending) {
        self.answer = answer
    }

    func sign(requestJson: String, digest: Data, keys: [WalletKeyRecord]) async -> ClearSignerChannel.Ending {
        let request = (try? JSONSerialization.jsonObject(with: Data(requestJson.utf8))) as? [String: Any] ?? [:]
        asked.append((request, digest))
        return answer(digest)
    }
}

@MainActor
struct ClearSignerSpineTests {
    private let fixture = ClearSignerFixture()

    private func spine(
        _ port: ScriptedRelayPort, _ signer: CountingSigner, clear: ScriptedClearSigner
    ) -> UserOpSpine {
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
        spine.signMethod = { UserOpSpine.clearSignerMethod }
        spine.clearSigner = clear
        return spine
    }

    /// The page's verified answer over what the page was sent.
    private func honestPage() -> ScriptedClearSigner {
        let fixture = self.fixture
        return ScriptedClearSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(clearSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
    }

    /// A site's `personal_sign`: the page is told the site's own method,
    /// params and origin, is asked for the SAME digest the passkey would sign
    /// (the Safe message hash), and its answer becomes the EIP-1271 envelope.
    /// The passkey is never raised.
    @Test func aMessageIsSignedByThePageOverThePasskeysDigest() async throws {
        let signer = CountingSigner()
        let page = honestPage()
        let spine = spine(ScriptedRelayPort(), signer, clear: page)
        let original = Data(repeating: 0x42, count: 32)
        let params = #"["0x68656c6c6f","\#(fixture.account)"]"#

        let signature = try await spine.signMessage(
            chainId: 100, account: fixture.account, originalHash: original,
            asked: .init(method: "personal_sign", paramsJson: params, origin: "https://app.example")
        )

        #expect(signature.hasPrefix("0x") && signature.count > 2)
        #expect(signer.calls == 0, "the Clear Signer replaces the passkey sheet")
        let asked = try #require(page.asked.first)
        let challenge = try safeMessageHash(originalHash: original, chainId: 100, safeAddress: fixture.account)
        #expect(asked.digest == challenge)
        let intent = try #require(asked.request["intent"] as? [String: Any])
        #expect(intent["method"] as? String == "personal_sign")
        #expect(intent["origin"] as? String == "https://app.example")
        let context = try #require(asked.request["context"] as? [String: Any])
        #expect(context["operation"] == nil, "a message carries no operation")
        #expect(context["allowCredentials"] as? [String] == [ClearSignerFixture.base64url(fixture.credential)])
    }

    /// The wallet's own send: the core builds `wallet_sendCalls` from the
    /// calls, the origin is empty, the page gets the ASSEMBLED operation with
    /// its fee leg's index — and an accepted answer is signed into the op and
    /// submitted like any passkey's.
    @Test func theWalletsOwnSendGoesToThePageAndIsSubmitted() async throws {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x")
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        let opHash = "0x" + String(repeating: "ab", count: 32)
        port.rpc["eth_sendUserOperation"] = .ok(opHash)
        let signer = CountingSigner()
        let page = honestPage()
        let spine = spine(port, signer, clear: page)
        let calls = [UserOpCall(to: fixture.account, value: "1000", data: "0x")]

        let hash = try await spine.submit(
            chainId: 100, account: fixture.account, calls: calls, gasFeeToken: nil,
            quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account)
        )

        #expect(hash == opHash)
        #expect(signer.calls == 0)
        let asked = try #require(page.asked.first)
        #expect(asked.digest.count == 32)
        let intent = try #require(asked.request["intent"] as? [String: Any])
        #expect(intent["method"] as? String == "wallet_sendCalls")
        #expect(intent["origin"] as? String == "")
        let operation = try #require((asked.request["context"] as? [String: Any])?["operation"] as? [String: Any])
        #expect((operation["feeLegIndex"] as? NSNumber)?.intValue == calls.count)
        #expect(operation["userOp"] != nil)
    }

    /// Nothing is signed and nothing is submitted on any other ending; the
    /// sentence that goes on the sheet is the contract's (§5).
    @Test func everyOtherEndingIsARefusalInTheContractsWords() async throws {
        let cases: [(ClearSignerChannel.Ending, ClearSignerNotice)] = [
            (.outcome(.refused(refusal: .declined)), .closed),
            (.outcome(.refused(refusal: .pageRefused(code: "unlimited_approval"))), .refused),
            (.outcome(.refused(refusal: .wrongChallenge)), .mismatch),
            (.outcome(.refused(refusal: .foreignKey)), .mismatch),
            (.outcome(.refused(refusal: .malformed(detail: "x"))), .mismatch),
            (.timedOut, .timeout),
        ]
        for (ending, notice) in cases {
            let spine = spine(ScriptedRelayPort(), CountingSigner(), clear: ScriptedClearSigner { _ in ending })
            do {
                _ = try await spine.signMessage(
                    chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
                    asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
                )
                Issue.record("\(ending) signed")
            } catch let refused as UserOpSpine.Refused {
                #expect(refused.failure == .clearSigner(notice))
            }
        }
        #expect(ClearSignerNotice.closed.key == "componentsUi.signing.clearSignerClosed")
        #expect(ClearSignerNotice.mismatch.key == "componentsUi.signing.clearSignerMismatch")
    }
}

// MARK: - The preferences and the picker

@MainActor
struct SignPrefTests {
    private var loc: Loc { Loc(overrideTag: "en", preferredLanguages: []) }

    /// Drives the machine the way `CoreStore` does — dispatch, perform each
    /// operation against the store, resolve — so the round trip is the
    /// executor's and the core's, not a copy of their rules.
    private func run(_ core: SignPrefCore, _ executor: SignPrefExecutor, _ event: [String: Any]) throws -> SignPrefViewWire {
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

    @Test func theDefaultsPersistUnderTheirKeysAndReadBack() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SignPrefExecutor(store: store)
        let core = SignPrefCore()

        var view = try run(core, executor, ["type": "refresh"])
        #expect(view.method == "auto" && !view.methodCommitted)
        #expect(view.offered == ["auto", "platform", "hybrid", "security_key", "clear_signer"])
        #expect(view.signerUrlIsDefault && view.signerUsesWalletPasskeys)

        view = try run(core, executor, ["type": "method_chosen", "method": "clear_signer"])
        #expect(view.method == "clear_signer")
        #expect(store.readString(VelaStore.Key.signMethod) == "clear_signer")

        // Refused: nothing stored, the old page stands, and the sheet says why.
        view = try run(core, executor, ["type": "signer_url_submitted", "text": "http://192.168.1.4/"])
        #expect(view.signerUrlError == "insecure")
        #expect(store.readString(VelaStore.Key.clearSignerUrl) == nil)

        view = try run(core, executor, ["type": "signer_url_submitted", "text": "http://localhost:8141/"])
        #expect(view.signerUrlError == nil && !view.signerUrlIsDefault)
        #expect(!view.signerUsesWalletPasskeys, "a page off getvela.app cannot use the wallet's passkeys")
        let stored = try #require(store.readString(VelaStore.Key.clearSignerUrl))

        // A second launch reads both back.
        let again = try run(SignPrefCore(), SignPrefExecutor(store: store), ["type": "refresh"])
        #expect(again.method == "clear_signer" && again.methodCommitted)
        #expect(again.signerUrl == stored)

        view = try run(core, executor, ["type": "signer_url_reset"])
        #expect(view.signerUrlIsDefault)
        #expect(store.readString(VelaStore.Key.clearSignerUrl) == nil)
    }

    /// Settings: the two rows say what is in force, and the page sheet carries
    /// the core's verdicts — never a rule of its own.
    @Test func settingsShowsTheMethodThePageAndWhyAPageCannotSign() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SignPrefExecutor(store: store)
        let core = SignPrefCore()
        _ = try run(core, executor, ["type": "refresh"])
        let base = SettingsFixtures.build(.st1, loc: loc)

        let official = SettingsLive.withSignPref(try run(core, executor, ["type": "refresh"]), on: base, loc: loc)
        let rows = official.sections.flatMap(\.rows)
        #expect(rows.first { $0.id == SettingsFixtures.signWithRow }?.value == "Automatic")
        #expect(rows.first { $0.id == SettingsFixtures.signerPageRow }?.value == "Official")
        #expect(official.signerPage?.reset == nil)
        #expect(official.signerPage?.foreign == nil)
        #expect(official.signWithSheet.rows.map(\.id) == ["auto", "platform", "hybrid", "security_key", "clear_signer"])

        _ = try run(core, executor, ["type": "method_chosen", "method": "clear_signer"])
        let own = SettingsLive.withSignPref(
            try run(core, executor, ["type": "signer_url_submitted", "text": "http://localhost:8141/"]),
            on: base, loc: loc
        )
        let ownRows = own.sections.flatMap(\.rows)
        #expect(ownRows.first { $0.id == SettingsFixtures.signWithRow }?.value == "Clear Signer")
        #expect(ownRows.first { $0.id == SettingsFixtures.signerPageRow }?.value == "localhost")
        #expect(own.signerPage?.reset == "Use the official page")
        #expect(own.signerPage?.foreign?.hasPrefix("Your passkeys belong to getvela.app") == true)

        let refused = SettingsLive.withSignPref(
            try run(core, executor, ["type": "signer_url_submitted", "text": "not a page"]), on: base, loc: loc
        )
        #expect(refused.signerPage?.error == "That is not a web address.")
    }

    /// The signing sheet's "Sign with": five options in the core's order and
    /// the create flow's words, the Clear Signer with its line — and each
    /// request starts at the stored default, not at the last pick.
    @Test func thePickerOffersFiveAndEachRequestStartsAtTheDefault() throws {
        let initial = try #require(SignPrefViewWire.initial)
        let suite = "vela.tests.signpref.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        var stored = "clear_signer"
        let controller = SigningController(
            wallet: (address: ClearSignerFixture().account, credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore()),
            preferredSignMethod: { stored },
            offeredSignMethods: { initial.offered },
            ports: SigningController.Ports(knownChains: { [100] })
        )
        let incoming = SigningController.Incoming(
            id: "req-1", method: "personal_sign", paramsJson: #"["0x68656c6c6f"]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        controller.open(incoming)
        #expect(controller.signMethod == "clear_signer")

        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Mine", walletAddress: ClearSignerFixture().account
        )
        context.signMethods = controller.offeredSignMethods()
        context.signMethod = controller.signMethod
        let picker = SigningLive.signWith(context: context)
        #expect(picker.options.map(\.title)
                == ["Automatic", "This device", "Phone or tablet", "USB security key", "Clear Signer"])
        #expect(picker.value == "Clear Signer")
        #expect(picker.options.last?.detail == "Check and sign on a separate page — what you see is what you sign.")
        #expect(picker.options.dropLast().allSatisfy { $0.detail == nil })

        // A pick is this request's; a name the core does not offer is not one.
        controller.signWith("platform")
        #expect(controller.signMethod == "platform")
        controller.signWith("somewhere_else")
        #expect(controller.signMethod == "platform")

        // The next request starts at the default again.
        stored = "hybrid"
        controller.open(incoming)
        #expect(controller.signMethod == "hybrid")
    }
}

// MARK: - The real page (opt-in)

/// The shipped page, served where the device pass serves it (research R5):
/// opened in the in-app tab over this app, it must reach the listener from
/// its own origin, prove itself with the token and take the request. Closing
/// the TAB then ends the ceremony as declined — which only a page that HAD the
/// request can cause; a page that never connected leaves it waiting.
///
/// Opt-in, because it needs a server:
/// `TEST_RUNNER_VELA_CLEAR_SIGNER_PAGE=http://localhost:8141/ xcodebuild test …`
/// with `python3 -m http.server 8141` in `app-web/clearsigning`.
@MainActor
struct ClearSignerPageTests {
    static let page = ProcessInfo.processInfo.environment["VELA_CLEAR_SIGNER_PAGE"]
    /// Long enough to load the page, open the socket and draw the request —
    /// and for a screenshot of it.
    static let dwell = Double(ProcessInfo.processInfo.environment["VELA_CLEAR_SIGNER_DWELL"] ?? "") ?? 12

    @Test(.enabled(if: page != nil))
    func theServedPageConnectsFromItsOriginAndTakesTheRequest() async throws {
        let page = try #require(Self.page)
        let fixture = ClearSignerFixture()
        let request = try fixture.messageRequest()
        let host = ClearSigner(loc: Loc(overrideTag: "en", preferredLanguages: []), signerUrl: { page })
        let signing = Task {
            await host.sign(requestJson: request, digest: Data(repeating: 7, count: 32), keys: fixture.keys)
        }
        print("[clear-signer-e2e] page opening: \(page)")
        try await Task.sleep(for: .seconds(Self.dwell))
        print("[clear-signer-e2e] closing the tab")

        // What the tab's close button does: the tab goes, and its delegate
        // hears that the person finished with it.
        let tab = try #require(Self.topmost as? SFSafariViewController, "the page is not on screen")
        tab.dismiss(animated: false)
        tab.delegate?.safariViewControllerDidFinish?(tab)
        var gaveUp = false
        let watchdog = Task {
            try await Task.sleep(for: .seconds(15))
            gaveUp = true
            host.cancel()
        }
        let ending = await signing.value
        watchdog.cancel()
        #expect(!gaveUp, "the page never took the request")
        #expect(ending == .outcome(.refused(refusal: .declined)))
    }

    private static var topmost: UIViewController? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .flatMap(\.windows)
            .first { $0.isKeyWindow }?
            .rootViewController?
            .topmost
    }
}
