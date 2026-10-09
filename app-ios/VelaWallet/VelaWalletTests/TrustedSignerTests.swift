//
//  TrustedSignerTests.swift
//  VelaWalletTests
//
//  Specs 071 and 102 on iOS: the trusted page's channel, the spine's branch
//  for an account whose venue is a page, and Settings → Signing pages, with
//  nothing mocked that the core decides.
//
//  The channel is spoken to the way the page speaks to it — the request out of
//  the launch URL the CHECKED page builds (`SignerPageAdmission.urlLaunch`),
//  the answer back through the scheme — and the answer is signed by a real
//  P-256 key standing in for the passkey, so the verdicts below are the
//  core's own, run for real.
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
    var key = P256.Signing.PrivateKey()
    var credential = Data([0x11, 0x22, 0x33, 0x44])
    let signerUrl = "https://sign.getvela.app/"
    let origin = "https://sign.getvela.app"
    let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    var credentialHex: String { credential.map { String(format: "%02x", $0) }.joined() }

    var keys: [WalletKeyRecord] {
        [WalletKeyRecord(credentialId: credentialHex, publicKeyHex: Self.hex(key.publicKey.x963Representation))]
    }

    /// This wallet's stored record (`app::Account`, spec 102): signed in with
    /// its key, which lives in `place`, reviewing and signing at `venue` —
    /// the official page unless said otherwise.
    func recordJson(
        place: String = "platform",
        venue: [String: Any] = ["type": "page", "url": "https://sign.getvela.app/"]
    ) -> String {
        let publicKeyHex = Self.hex(key.publicKey.x963Representation)
        let record: [String: Any] = [
            "id": credentialHex, "name": "Mine", "address": account,
            "public_key_hex": publicKeyHex, "created_at_iso": "2026-09-26T00:00:00.000Z",
            "keys": [[
                "credential_id": credentialHex, "public_key_hex": publicKeyHex,
                "name": "Mine", "transports": "internal",
            ]],
            "sign_in_key": ["credential_id": credentialHex, "method": place],
            "signing_domain": "getvela.app",
            "signing_venue": venue,
        ]
        return String(decoding: try! JSONSerialization.data(withJSONObject: record), as: UTF8.self)
    }

    /// A record whose venue is the official page — every signature goes there.
    var pageRecordJson: String { recordJson() }

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

/// A page this device "checked": the core's own target for `base`, admitted
/// by naming the bytes as the version the target asks for — which is what a
/// fetch of those very bytes would hash to. Hermetic: no network, and the
/// launch URL is still the core's (spec 102 R6).
@MainActor
enum TestAdmissions {
    static let official = "https://sign.getvela.app/"

    static func admitted(_ base: String = official) -> SignerPageAdmission {
        let target = SignerPageTarget.choose(base: base, index: nil, trusted: [], blocked: [])
        return signerPageAdmit(
            target: target, observedHash: target.version(), failure: .notChecked,
            trusted: [], blocked: [], verificationOff: false,
            checkedAtMs: UInt64(Date().timeIntervalSince1970 * 1000)
        )
    }

    static func launcher(_ base: String = official) -> TrustedSignerChannel.Launcher {
        TrustedSignerChannel.launcher(admitted(base))
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
            signerUrl: fixture.signerUrl, launch: TestAdmissions.launcher(fixture.signerUrl),
            requestJson: try fixture.messageRequest(),
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
        // Spec 102 R6: the phones open the CHECKED, content-addressed version
        // (`/b/<sha256>/sign`) — the very URL the check fetched — never the
        // root page.
        #expect(launch.hasPrefix("https://sign.getvela.app/b/"))
        #expect(launch.components(separatedBy: "#")[0].hasSuffix("/sign?ch=url"))
        let version = launch.dropFirst("https://sign.getvela.app/b/".count).prefix { $0 != "/" }
        #expect(version.count == 64 && version.allSatisfy(\.isHexDigit), "a sha-256 names the version")
        let checked = SignerPageTarget.choose(base: fixture.signerUrl, index: nil, trusted: [], blocked: [])
        let checkedUrl = try #require(checked.url())
        #expect(launch.hasPrefix(checkedUrl), "the URL that opens is the URL that was checked")
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
    private(set) var asked: [(request: [String: Any], digest: Data, page: String, place: KeyMethod?, keyName: String)] = []

    init(answer: @escaping (_ digest: Data) -> TrustedSignerChannel.Ending) {
        self.answer = answer
    }

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], page: String,
        keyName: String, place: KeyMethod?
    ) async -> TrustedSignerChannel.Ending {
        let request = (try? JSONSerialization.jsonObject(with: Data(requestJson.utf8))) as? [String: Any] ?? [:]
        asked.append((request, digest, page, place, keyName))
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
        // The account reviews and signs on the official page (spec 102), so
        // every signature goes there.
        accounts.recordJson = fixture.pageRecordJson
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
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
        // R5: the page is told which key, and where it lives, so the browser
        // asks for that key only — no generic "where is your passkey?".
        let route = try #require(context["keyRoute"] as? [String: Any])
        #expect(route["credentialId"] as? String == TrustedSignerFixture.base64url(fixture.credential))
        #expect(route["place"] as? String == "platform")
        #expect(route["hints"] as? [String] == ["client-device"])
        #expect(asked.page == "https://sign.getvela.app/")
        #expect(asked.place == .platform, "the card says which key the person confirms with")
        #expect(asked.keyName == "Mine", "…by its own name first")
    }

    /// The same account, its venue In Vela: the page is never asked, and the
    /// passkey is pinned to the sign-in key over its place.
    @Test func anAccountThatSignsInVelaNeverReachesThePage() async throws {
        let signer = CountingSigner()
        let page = honestPage()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.recordJson(venue: ["type": "in_vela"])
        let spine = UserOpSpine(
            relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
        spine.trustedSigner = page
        do {
            _ = try await spine.signMessage(
                chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
                asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
            )
            Issue.record("the counting signer signed")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .passkeyCancelled)
        }
        #expect(signer.calls == 1, "In Vela is the app's own sheet")
        #expect(page.asked.isEmpty, "an account that signs in Vela opened a page")
    }

    /// R1: an account on a domain nothing here can reach signs NOTHING — not
    /// the page, not the passkey — and says why, in the core's words.
    @Test func anAccountNothingHereCanReachIsRefusedBeforeAnyPrompt() async throws {
        let signer = CountingSigner()
        let page = honestPage()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        var record = try CoreJSON.object(fixture.recordJson(venue: ["type": "in_vela"]))
        record["signing_domain"] = "sign.example.com"
        accounts.recordJson = String(decoding: try JSONSerialization.data(withJSONObject: record), as: UTF8.self)
        let stored = try #require(accounts.recordJson)
        let plan = try #require(SigningPlanWire.of(accountJson: stored))
        let blocked = try #require(plan.blocked, "the core lets an unreachable account sign")
        let spine = UserOpSpine(
            relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
        spine.trustedSigner = page
        do {
            _ = try await spine.signMessage(
                chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
                asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
            )
            Issue.record("an account nothing can reach signed")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .trustedSigner(.blocked(blocked)))
        }
        #expect(signer.calls == 0 && page.asked.isEmpty)
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(TrustedSignerNotice.blocked(blocked).text(loc).contains("sign.example.com"))
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
            quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: fixture.account),
            writeAhead: { _, _ in true }
        )

        #expect(hash.userOpHash == opHash)
        #expect(!hash.maybeSent, "the relay answered: accepted, not may-have-been-sent")
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
        let mismatchLine = SignerIntegrityLine(
            state: .mismatch, version: "deadbeef", checkedAtMs: nil,
            key: "componentsUi.signing.integrity.mismatch", opens: false
        )
        let cases: [(TrustedSignerChannel.Ending, TrustedSignerNotice)] = [
            (.outcome(.refused(refusal: .declined)), .closed),
            (.outcome(.refused(refusal: .pageRefused(code: "unlimited_approval"))), .refused),
            (.outcome(.refused(refusal: .wrongChallenge)), .mismatch),
            (.outcome(.refused(refusal: .foreignKey)), .mismatch),
            (.outcome(.refused(refusal: .malformed(detail: "x"))), .mismatch),
            (.timedOut, .timeout),
            // A ceremony answer to a signature is a shell bug — never English
            // on the sheet, but the contract's own sentence.
            (.ceremony(.refused(refusal: .declined)), .mismatch),
            (.unavailable, .unavailable),
            // R6: the check refused the page; it was never opened.
            (.notOpened(mismatchLine), .notOpened(mismatchLine)),
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
        #expect(TrustedSignerNotice.unavailable.key == "componentsUi.signing.signerDown")
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(TrustedSignerNotice.notOpened(mismatchLine).text(loc)
                == "Version deadbeef isn't on Vela's published build list. Not opened.")
        // Every sentence is the corpus's: none of them is English the shell
        // wrote (research §4's hard-coded errors).
        for notice in cases.map(\.1) {
            #expect(notice.text(loc) != notice.key, "\(notice) has no sentence in the corpus")
        }

        // Left on the hand-off card before anything opened: a cancelled sheet.
        let spine = spine(ScriptedRelayPort(), CountingSigner(), clear: ScriptedTrustedSigner { _ in .cancelled })
        do {
            _ = try await spine.signMessage(
                chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
                asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
            )
            Issue.record("a cancelled card signed")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .passkeyCancelled)
        }
    }
}

// MARK: - Settings → Signing pages (spec 102)

@MainActor
struct SigningPagesTests {
    private var loc: Loc { Loc(overrideTag: "en", preferredLanguages: []) }

    /// Drives the machine the way `CoreStore` does — dispatch, perform each
    /// operation against the store, resolve — so the round trip is the
    /// executor's and the core's, not a copy of their rules.
    static func run(
        _ core: SigningPagesCore, _ executor: SigningPagesExecutor, _ event: [String: Any]
    ) throws -> SigningPagesViewWire {
        var result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(event)))
        var pending = result["effects"] as? [[String: Any]] ?? []
        while !pending.isEmpty {
            let effect = pending.removeFirst()
            let id = (effect["id"] as? NSNumber)?.uint64Value ?? 0
            let answer = executor.perform(effect["operation"] as? [String: Any] ?? [:])
            result = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: answer))
            pending += result["effects"] as? [[String: Any]] ?? []
        }
        return try CoreJSON.decode(SigningPagesViewWire.self, from: CoreJSON.object(core.view()))
    }

    /// The list persists under its own key and reads back: the official page
    /// first and never stored, refusals store nothing, and a page can be
    /// renamed and removed.
    @Test func pagesPersistUnderTheirKeyAndReadBack() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SigningPagesExecutor(store: store)
        let core = SigningPagesCore()

        var view = try Self.run(core, executor, ["type": "refresh"])
        #expect(view.loaded)
        #expect(view.pages.map(\.official) == [true], "the official page alone, first")
        #expect(view.pages.first?.domain == "getvela.app")
        #expect(view.saved.isEmpty)

        // Refused: nothing stored, and the sheet can say why.
        view = try Self.run(core, executor, ["type": "page_added", "url": "http://192.168.1.4/", "name": ""])
        #expect(view.addError == "insecure")
        #expect(SigningPagesViewWire.addErrorKey(view.addError) == "settings.signing.pageInsecure")
        #expect(store.rawValue(VelaStore.Key.signingPages) == nil)

        view = try Self.run(core, executor, ["type": "page_added", "url": "https://sign.example.com", "name": "Mine"])
        #expect(view.addError == nil)
        let mine = try #require(view.pages.last)
        #expect(!mine.official && mine.domain == "sign.example.com" && mine.name == "Mine")
        #expect(store.rawValue(VelaStore.Key.signingPages)?.contains("sign.example.com") == true)

        view = try Self.run(core, executor, ["type": "page_added", "url": "https://sign.example.com/", "name": ""])
        #expect(view.addError == "duplicate")

        // A second launch reads it back.
        let again = try Self.run(SigningPagesCore(), SigningPagesExecutor(store: store), ["type": "refresh"])
        #expect(again.saved.map(\.url) == [mine.url])

        view = try Self.run(core, executor, ["type": "page_renamed", "url": mine.url, "name": "Work"])
        #expect(view.pages.last?.name == "Work")
        view = try Self.run(core, executor, ["type": "page_removed", "url": mine.url])
        #expect(view.saved.isEmpty)
    }

    /// The 071 field's page is imported once, as a saved page, and the old
    /// key removed — so removing it later does not bring it back.
    @Test func theOldTrustedSignerPageIsImportedOnce() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        store.writeString(VelaStore.Key.trustedSignerUrl, "https://sign.example.com/")
        let view = try Self.run(SigningPagesCore(), SigningPagesExecutor(store: store), ["type": "refresh"])
        #expect(view.saved.map(\.url) == ["https://sign.example.com/"])
        #expect(store.readString(VelaStore.Key.trustedSignerUrl) == nil, "the old key was left to import again")
    }

    /// Settings has no free-text "Trusted Signer page" row any more, and no
    /// "Sign with" chosen per signature (founder, 2026-09-26).
    @Test func settingsHasNoTrustedSignerUrlField() {
        let rows = SettingsFixtures.build(.st1, loc: loc).sections.flatMap(\.rows)
        #expect(!rows.contains { $0.id == "signer-page" })
        #expect(!rows.contains { $0.title == "Sign with" }, "a default \"Sign with\" is back in Settings")
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
        let host = TrustedSigner(loc: Loc(overrideTag: "en", preferredLanguages: []), checks: .shared)
        let signing = Task {
            await host.sign(
                requestJson: request, digest: Data(repeating: 7, count: 32), keys: fixture.keys,
                page: page, keyName: "", place: .platform
            )
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
