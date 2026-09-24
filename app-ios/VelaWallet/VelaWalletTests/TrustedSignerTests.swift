//
//  TrustedSignerTests.swift
//  VelaWalletTests
//
//  Spec 071 on iOS: the Trusted Signer's loopback channel, the spine's branch
//  and the two preferences, with nothing mocked that the core decides.
//
//  The channel is spoken to the way the page speaks to it — a TCP socket, an
//  HTTP upgrade with an `Origin`, masked frames, the token, the answer — and
//  the answer is signed by a real P-256 key standing in for the passkey, so
//  the verdicts below are the core's own, run for real.
//

import Compression
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

/// A wallet with one real P-256 key, and what a Trusted Signer page answers for
/// a digest when that key signs it.
struct TrustedSignerFixture {
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
        try trustedSignerRequest(
            input: UserOpSpine.trustedSignerInput(
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

/// The browser's side of one visit (spec 076): the request read out of the
/// launch URL's fragment exactly as `intake.js` reads it, and the answer
/// handed back the way a navigation to `velawallet://sign-result` reaches this
/// app.
@MainActor
struct PageVisit {
    let url: URL
    let token: String
    let callbackUrl: String
    let request: [String: Any]

    /// Plain `throws`, not `#require`: a visit is also read inside the
    /// channel's `openPage` closure, where there is no test context.
    struct NotALaunchUrl: Error { let url: URL }

    init(_ url: URL) throws {
        self.url = url
        let fragment = url.absoluteString.components(separatedBy: "#").last ?? ""
        var fields: [String: String] = [:]
        for pair in fragment.components(separatedBy: "&") {
            let parts = pair.components(separatedBy: "=")
            guard parts.count == 2 else { continue }
            fields[parts[0]] = parts[1]
        }
        guard let token = fields["t"]?.removingPercentEncoding,
              let callback = Self.b64url(fields["cb"]),
              let payload = Self.b64url(fields["i"]),
              let json = fields["z"] == "1" ? Self.inflateRaw(payload) : payload,
              let request = (try? JSONSerialization.jsonObject(with: json)) as? [String: Any]
        else { throw NotALaunchUrl(url: url) }
        self.token = token
        self.callbackUrl = String(decoding: callback, as: UTF8.self)
        self.request = request
    }

    /// The intent the page would draw.
    var intent: [String: Any] { request["intent"] as? [String: Any] ?? [:] }
    var context: [String: Any] { request["context"] as? [String: Any] ?? [:] }

    func callback(_ body: [String: Any]) -> URL {
        let json = (try? JSONSerialization.data(withJSONObject: body)) ?? Data("{}".utf8)
        return URL(string: callbackUrl + "?t=" + Self.percent(token)
            + "&result=" + TrustedSignerFixture.base64url(json))!
    }

    /// The page answered: it navigates to the callback.
    @discardableResult
    func answer(_ body: [String: Any]) -> Bool {
        TrustedSignerCallbacks.deliver(callback(body))
    }

    /// The page (or the person) said no.
    @discardableResult
    func refuse(_ code: String) -> Bool {
        TrustedSignerCallbacks.deliver(
            URL(string: callbackUrl + "?t=" + Self.percent(token) + "&error=" + code)!
        )
    }

    private static func percent(_ text: String) -> String {
        text.addingPercentEncoding(withAllowedCharacters: .alphanumerics) ?? text
    }

    private static func b64url(_ text: String?) -> Data? {
        guard var text else { return nil }
        text = text.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        while text.count % 4 != 0 { text += "=" }
        return Data(base64Encoded: text)
    }

    /// `DecompressionStream('deflate-raw')`, which is what the page uses —
    /// and what Apple calls `COMPRESSION_ZLIB` (raw DEFLATE, no header).
    private static func inflateRaw(_ data: Data) -> Data? {
        let capacity = max(data.count * 12, 64 * 1024)
        var out = Data(count: capacity)
        let written = out.withUnsafeMutableBytes { destination -> Int in
            data.withUnsafeBytes { source -> Int in
                compression_decode_buffer(
                    destination.bindMemory(to: UInt8.self).baseAddress!, capacity,
                    source.bindMemory(to: UInt8.self).baseAddress!, data.count,
                    nil, COMPRESSION_ZLIB
                )
            }
        }
        guard written > 0 else { return nil }
        return out.prefix(written)
    }
}

struct TrustedSignerChannelTests {

    @MainActor
    private func channel(
        _ fixture: TrustedSignerFixture, digest: Data, timeout: TimeInterval = TrustedSignerChannel.defaultTimeout
    ) throws -> TrustedSignerChannel {
        TrustedSignerChannel(
            signerUrl: fixture.signerUrl, requestJson: try fixture.messageRequest(),
            digest: digest, keys: fixture.keys, timeout: timeout
        )
    }

    /// The whole conversation, as the page has it: the request out of the
    /// launch URL's fragment, one answer back through the scheme — and an
    /// answer over this digest by this wallet's key is the core's `accepted`,
    /// which builds the same EIP-1271 envelope a passkey's assertion does.
    @Test @MainActor func thePageReadsTheRequestFromTheUrlAndItsSignatureIsAccepted() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xAB, count: 32)
        let channel = try channel(fixture, digest: digest)
        let url = try #require(channel.start())

        let launch = url.absoluteString
        #expect(launch.hasPrefix("https://sign.getvela.app/sign.html?ch=url#"))
        // The request is in the FRAGMENT, which no server is sent. A query
        // would be in the log of the server that serves the page.
        #expect(launch.contains("#i="))
        #expect(!launch.components(separatedBy: "#")[0].contains("i="))
        #expect(Data(base64Encoded: channel.token.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/") + "==")?.count == 16, "128 bits")

        let page = try PageVisit(url)
        #expect(page.intent["method"] as? String == "personal_sign")
        #expect(page.intent["origin"] as? String == "https://app.example")
        // The page is told where to answer, and it is this wallet's scheme.
        #expect(page.callbackUrl == trustedSignerCallbackUrl())

        #expect(page.answer(fixture.result(for: digest)))
        let ending = await channel.ending()
        guard case .outcome(.accepted(let credentialIdHex, let assertion)) = ending else {
            Issue.record("expected an accepted answer, got \(ending)")
            return
        }
        #expect(credentialIdHex == fixture.credentialHex)
        #expect(assertion.signatureDer.first == 0x30, "DER, as the Safe envelope takes")
        let envelope = try eip1271Signature(assertion: assertion, credentialId: credentialIdHex, keys: fixture.keys)
        #expect(!envelope.isEmpty)
        channel.end()
    }

    /// A callback naming some other attempt — another app that registered the
    /// scheme, a replay, a stale tab — reaches nothing, and the request it
    /// was aimed at is still waiting.
    @Test @MainActor func aCallbackForAnotherAttemptReachesNothing() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xCD, count: 32)
        let channel = try channel(fixture, digest: digest)
        let url = try #require(channel.start())
        let page = try PageVisit(url)

        let stranger = URL(string: trustedSignerCallbackUrl() + "?t=not-this-attempt&result=e30")!
        #expect(!TrustedSignerCallbacks.deliver(stranger), "nobody awaits that token")

        // And the real page still gets through.
        #expect(page.answer(fixture.result(for: digest)))
        guard case .outcome(.accepted) = await channel.ending() else {
            Issue.record("the real answer was not accepted")
            return
        }
        channel.end()
    }

    /// The page, or the person on it, said no: the request stays open to be
    /// signed another way (contract §5).
    @Test @MainActor func aRefusalIsToldAsTheRefusalItWas() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xCD, count: 32)
        let declined = try channel(fixture, digest: digest)
        _ = try #require(declined.start())
        #expect(try PageVisit(#require(declined.launchUrl)).refuse("user_rejected"))
        #expect(await declined.ending() == .outcome(.refused(refusal: .declined)))

        let refused = try channel(fixture, digest: digest)
        _ = try #require(refused.start())
        #expect(try PageVisit(#require(refused.launchUrl)).refuse("origin_not_allowed"))
        #expect(await refused.ending() == .outcome(.refused(refusal: .pageRefused(code: "origin_not_allowed"))))
    }

    /// Closing the tab is NOT an answer on this transport. The page often
    /// outlives its own navigation, so a dismissal read as "closed without
    /// signing" would turn a signature the person just approved into a
    /// refusal — measured on a real phone, the tab is still up when the
    /// callback arrives.
    @Test @MainActor func closingTheTabDoesNotRefuseAnAnswerThatIsOnItsWay() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xAB, count: 32)
        let channel = try channel(fixture, digest: digest)
        let url = try #require(channel.start())
        let page = try PageVisit(url)

        channel.pageClosed()
        #expect(page.answer(fixture.result(for: digest)), "the request is still waiting")
        guard case .outcome(.accepted) = await channel.ending() else {
            Issue.record("a dismissed tab lost an answer the person had already given")
            return
        }
        channel.end()
    }

    /// An answer by a key this wallet does not hold is refused, whoever sent
    /// it. This is what survives an intercepted or forged callback.
    @Test @MainActor func anAnswerByAnotherKeyIsRefused() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xAB, count: 32)
        let channel = try channel(fixture, digest: digest)
        let url = try #require(channel.start())
        let stranger = TrustedSignerFixture()
        #expect(try PageVisit(url).answer(stranger.result(for: digest)))
        guard case .outcome(.refused) = await channel.ending() else {
            Issue.record("a signature by a key the wallet does not hold was accepted")
            return
        }
        channel.end()
    }

    /// And an answer over some OTHER digest, by the right key.
    @Test @MainActor func anAnswerOverAnotherDigestIsRefused() async throws {
        let fixture = TrustedSignerFixture()
        let channel = try channel(fixture, digest: Data(repeating: 0xAB, count: 32))
        let url = try #require(channel.start())
        #expect(try PageVisit(url).answer(fixture.result(for: Data(repeating: 0x01, count: 32))))
        guard case .outcome(.refused) = await channel.ending() else {
            Issue.record("a signature over another digest was accepted")
            return
        }
        channel.end()
    }

    /// The clock and the cancel, which are the shell's and not the core's.
    @Test @MainActor func theShellsClockAndCancelEndACeremony() async throws {
        let fixture = TrustedSignerFixture()
        let digest = Data(repeating: 0xAB, count: 32)
        let ticking = try channel(fixture, digest: digest, timeout: 0.2)
        _ = try #require(ticking.start())
        #expect(await ticking.ending() == .timedOut)

        let cancelled = try channel(fixture, digest: digest)
        _ = try #require(cancelled.start())
        cancelled.cancel()
        #expect(await cancelled.ending() == .outcome(.refused(refusal: .declined)))
    }

    /// Spec 075 over this transport: a flow's second request is its own
    /// visit, with its own one-time token, and the page is opened again.
    @Test @MainActor func aFlowsNextRequestIsANewVisitWithANewToken() async throws {
        let fixture = TrustedSignerFixture()
        let first = Data(repeating: 0xAB, count: 32)
        let second = Data(repeating: 0xCD, count: 32)
        let channel = try channel(fixture, digest: first)
        var opened: [URL] = []
        let url = try #require(channel.start())
        #expect(try PageVisit(url).answer(fixture.result(for: first)))
        guard case .outcome(.accepted) = await channel.ending() else {
            Issue.record("the first request was not answered")
            return
        }

        let firstToken = channel.token
        // The page answers the second visit as it is opened — no sleeping on a
        // guess about when the channel gets there. A test that waits a fixed
        // 50ms passes alone and fails in a parallel run, which is how this one
        // announced itself.
        channel.openPage = { url in
            opened.append(url)
            _ = try? PageVisit(url).answer(fixture.result(for: second))
        }
        let ending = await channel.send(.signature(
            request: try fixture.messageRequest(), digest: second, keys: fixture.keys
        ))
        #expect(opened == [try #require(channel.launchUrl)], "the next request opens the page again")
        #expect(channel.token != firstToken, "one-time means one request")
        guard case .outcome(.accepted) = ending else {
            Issue.record("the flow's second request was not answered")
            return
        }
        channel.end()
    }
}

final class ScriptedTrustedSigner: TrustedSignerPort {
    var answer: (_ digest: Data) -> TrustedSignerChannel.Ending
    private(set) var asked: [(request: [String: Any], digest: Data, page: String?)] = []

    init(answer: @escaping (_ digest: Data) -> TrustedSignerChannel.Ending) {
        self.answer = answer
    }

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> TrustedSignerChannel.Ending {
        let request = (try? JSONSerialization.jsonObject(with: Data(requestJson.utf8))) as? [String: Any] ?? [:]
        asked.append((request, digest, signerOrigin))
        return answer(digest)
    }
}

@MainActor
struct TrustedSignerSpineTests {
    private let fixture = TrustedSignerFixture()

    private func spine(
        _ port: ScriptedRelayPort, _ signer: CountingSigner, clear: ScriptedTrustedSigner
    ) -> UserOpSpine {
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
        spine.signMethod = { UserOpSpine.trustedSignerMethod }
        spine.trustedSigner = clear
        return spine
    }

    /// The page's verified answer over what the page was sent.
    private func honestPage() -> ScriptedTrustedSigner {
        let fixture = self.fixture
        return ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
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
        #expect(signer.calls == 0, "the Trusted Signer replaces the passkey sheet")
        let asked = try #require(page.asked.first)
        let challenge = try safeMessageHash(originalHash: original, chainId: 100, safeAddress: fixture.account)
        #expect(asked.digest == challenge)
        let intent = try #require(asked.request["intent"] as? [String: Any])
        #expect(intent["method"] as? String == "personal_sign")
        #expect(intent["origin"] as? String == "https://app.example")
        let context = try #require(asked.request["context"] as? [String: Any])
        #expect(context["operation"] == nil, "a message carries no operation")
        #expect(context["allowCredentials"] as? [String] == [TrustedSignerFixture.base64url(fixture.credential)])
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
        let cases: [(TrustedSignerChannel.Ending, TrustedSignerNotice)] = [
            (.outcome(.refused(refusal: .declined)), .closed),
            (.outcome(.refused(refusal: .pageRefused(code: "unlimited_approval"))), .refused),
            (.outcome(.refused(refusal: .wrongChallenge)), .mismatch),
            (.outcome(.refused(refusal: .foreignKey)), .mismatch),
            (.outcome(.refused(refusal: .malformed(detail: "x"))), .mismatch),
            (.timedOut, .timeout),
        ]
        for (ending, notice) in cases {
            let spine = spine(ScriptedRelayPort(), CountingSigner(), clear: ScriptedTrustedSigner { _ in ending })
            do {
                _ = try await spine.signMessage(
                    chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
                    asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
                )
                Issue.record("\(ending) signed")
            } catch let refused as UserOpSpine.Refused {
                #expect(refused.failure == .trustedSigner(notice))
            }
        }
        #expect(TrustedSignerNotice.closed.key == "componentsUi.signing.trustedSignerClosed")
        #expect(TrustedSignerNotice.mismatch.key == "componentsUi.signing.trustedSignerMismatch")
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
        #expect(view.offered == ["auto", "platform", "hybrid", "security_key", "trusted_signer"])
        #expect(view.signerUrlIsDefault && view.signerUsesWalletPasskeys)

        view = try run(core, executor, ["type": "method_chosen", "method": "trusted_signer"])
        #expect(view.method == "trusted_signer")
        #expect(store.readString(VelaStore.Key.signMethod) == "trusted_signer")

        // Refused: nothing stored, the old page stands, and the sheet says why.
        view = try run(core, executor, ["type": "signer_url_submitted", "text": "http://192.168.1.4/"])
        #expect(view.signerUrlError == "insecure")
        #expect(store.readString(VelaStore.Key.trustedSignerUrl) == nil)

        view = try run(core, executor, ["type": "signer_url_submitted", "text": "http://localhost:8141/"])
        #expect(view.signerUrlError == nil && !view.signerUrlIsDefault)
        #expect(!view.signerUsesWalletPasskeys, "a page off getvela.app cannot use the wallet's passkeys")
        let stored = try #require(store.readString(VelaStore.Key.trustedSignerUrl))

        // A second launch reads both back.
        let again = try run(SignPrefCore(), SignPrefExecutor(store: store), ["type": "refresh"])
        #expect(again.method == "trusted_signer" && again.methodCommitted)
        #expect(again.signerUrl == stored)

        view = try run(core, executor, ["type": "signer_url_reset"])
        #expect(view.signerUrlIsDefault)
        #expect(store.readString(VelaStore.Key.trustedSignerUrl) == nil)
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
        #expect(official.signWithSheet.rows.map(\.id) == ["auto", "platform", "hybrid", "security_key", "trusted_signer"])

        _ = try run(core, executor, ["type": "method_chosen", "method": "trusted_signer"])
        let own = SettingsLive.withSignPref(
            try run(core, executor, ["type": "signer_url_submitted", "text": "http://localhost:8141/"]),
            on: base, loc: loc
        )
        let ownRows = own.sections.flatMap(\.rows)
        #expect(ownRows.first { $0.id == SettingsFixtures.signWithRow }?.value == "Trusted Signer")
        #expect(ownRows.first { $0.id == SettingsFixtures.signerPageRow }?.value == "localhost")
        #expect(own.signerPage?.reset == "Use the official page")
        #expect(own.signerPage?.foreign?.hasPrefix("Your passkeys belong to getvela.app") == true)

        let refused = SettingsLive.withSignPref(
            try run(core, executor, ["type": "signer_url_submitted", "text": "not a page"]), on: base, loc: loc
        )
        #expect(refused.signerPage?.error == "That is not a web address.")
    }

    /// The signing sheet's "Sign with": five options in the core's order and
    /// the create flow's words, the Trusted Signer with its line — and each
    /// request starts at the stored default, not at the last pick.
    @Test func thePickerOffersFiveAndEachRequestStartsAtTheDefault() throws {
        let initial = try #require(SignPrefViewWire.initial)
        let suite = "vela.tests.signpref.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        var stored = "trusted_signer"
        let controller = SigningController(
            wallet: (address: TrustedSignerFixture().account, credentialId: "cred-1"),
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
        #expect(controller.signMethod == "trusted_signer")

        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Mine", walletAddress: TrustedSignerFixture().account
        )
        context.signMethods = controller.offeredSignMethods()
        context.signMethod = controller.signMethod
        let picker = SigningLive.signWith(context: context)
        #expect(picker.options.map(\.title)
                == ["Automatic", "This device", "Phone or tablet", "USB security key", "Trusted Signer"])
        #expect(picker.value == "Trusted Signer")
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
/// `TEST_RUNNER_VELA_TRUSTED_SIGNER_PAGE=http://localhost:8141/ xcodebuild test …`
/// with `python3 -m http.server 8141` in `app-web/trusted-signer`.
@MainActor
struct TrustedSignerPageTests {
    static let page = ProcessInfo.processInfo.environment["VELA_TRUSTED_SIGNER_PAGE"]
    /// Long enough to load the page, open the socket and draw the request —
    /// and for a screenshot of it.
    static let dwell = Double(ProcessInfo.processInfo.environment["VELA_TRUSTED_SIGNER_DWELL"] ?? "") ?? 12

    @Test(.enabled(if: page != nil))
    func theServedPageConnectsFromItsOriginAndTakesTheRequest() async throws {
        let page = try #require(Self.page)
        let fixture = TrustedSignerFixture()
        let request = try fixture.messageRequest()
        let host = TrustedSigner(loc: Loc(overrideTag: "en", preferredLanguages: []), signerUrl: { page })
        let signing = Task {
            await host.sign(requestJson: request, digest: Data(repeating: 7, count: 32), keys: fixture.keys)
        }
        print("[trusted-signer-e2e] page opening: \(page)")
        try await Task.sleep(for: .seconds(Self.dwell))
        print("[trusted-signer-e2e] closing the tab")

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
