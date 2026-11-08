//
//  R21DoorsTests.swift
//  VelaWalletTests
//
//  Round 21 "closing the doors" (§35), the iOS shell's half — each rule is
//  the core's, and these prove the shell asks it and renders its answer:
//
//    1. every SafeOp expires (review 1-6): the spine stamps the FINAL
//       operation with its window just before the hash, each attempt anew;
//       the core refuses to hash an unstamped one; the relay's AA22 is
//       `expired` — "Not sent", calmly, on Send, the signing sheet and the
//       multisig's execution;
//    2. a relay's settlementGas past the cap (`settlement_gas_too_high`) is a
//       quote Vela won't accept, said in its own sentence;
//    3. a site asking a multisig to change who controls it is said in its
//       own sentence (the vector replay through the real machine is in
//       `MultisigFlowTests`, beside the §31 ones);
//    4. the relying party a build's passkeys live under is the core's rule
//       (`relying_party_id`, the shared vector `rules/rp-id.json`), and the
//       shell hands it this build's facts.
//

import AuthenticationServices
import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - 1. Every SafeOp expires

/// Signs where a passkey would, with a real P-256 key: a WebAuthn assertion
/// the core's envelope accepts — so the spine runs all the way to the POST.
@MainActor
final class P256AssertionSigner: UserOpSigner {
    let fixture: TrustedSignerFixture
    private(set) var challenges: [Data] = []

    init(_ fixture: TrustedSignerFixture) { self.fixture = fixture }

    func sign(
        challenge: Data, credentialIdHex: String?, transports: String, method: KeyMethod
    ) async throws -> Assertion {
        challenges.append(challenge)
        var authenticatorData = Data(SHA256.hash(data: Data("getvela.app".utf8)))
        authenticatorData.append(contentsOf: [0x05, 0, 0, 0, 7])
        let clientData = Data(
            #"{"type":"webauthn.get","challenge":"\#(TrustedSignerFixture.base64url(challenge))","origin":"https://getvela.app","crossOrigin":false}"#.utf8
        )
        var signed = authenticatorData
        signed.append(Data(SHA256.hash(data: clientData)))
        let signature = try fixture.key.signature(for: signed)
        return Assertion(
            credentialIdHex: fixture.credentialHex,
            signatureDerHex: TrustedSignerFixture.hex(signature.derRepresentation),
            authenticatorDataHex: TrustedSignerFixture.hex(authenticatorData),
            clientDataJsonHex: TrustedSignerFixture.hex(clientData),
            userIdHex: nil,
            authenticatorAttachment: "platform"
        )
    }
}

/// A relay that records what is posted and answers it as told.
@MainActor
final class WindowCapture: RelayPort {
    let inner = ScriptedRelayPort()
    private(set) var posted: [[String: Any]] = []
    var sendAnswer = RpcCallResult(outcome: .ok("0x" + String(repeating: "ab", count: 32)), maybeDelivered: false, heldErrorJson: nil)

    init() {
        inner.rpc["eth_getCode"] = .ok("0x")
        inner.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        inner.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
    }

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        await callDetailed(chainId: chainId, method: method, params: params, kind: kind).outcome
    }

    func callDetailed(chainId: Int, method: String, params: [Any], kind: String) async -> RpcCallResult {
        if method == "eth_sendUserOperation" {
            if let op = params.first as? [String: Any] { posted.append(op) }
            return sendAnswer
        }
        return await inner.callDetailed(chainId: chainId, method: method, params: params, kind: kind)
    }

    func bundlerBase(chainId: Int) async -> String? { "https://relay.test" }
    func bestRpcUrl(chainId: Int) async -> String? { "https://rpc.test" }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer { .failed }
}

@MainActor
@Suite(.hangLimit)
struct SafeOpWindowTests {

    private let fixture = TrustedSignerFixture()
    private let gnosis = 100
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    /// 2026-10-26T…Z, in ms — any instant; the window is read from it.
    private static let t0: Double = 1_793_000_000_000

    private func spine(_ port: WindowCapture, now: @escaping () -> Double) -> (UserOpSpine, P256AssertionSigner) {
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        let signer = P256AssertionSigner(fixture)
        let spine = UserOpSpine(
            relay: RelayClient(port: port, now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }, now: now
        )
        return (spine, signer)
    }

    private func submit(_ spine: UserOpSpine) async throws -> UserOpSpine.Submitted {
        try await spine.submit(
            chainId: gnosis, account: fixture.account,
            calls: [UserOpCall(to: payee, value: "1000", data: "0x")],
            gasFeeToken: nil, quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: payee),
            writeAhead: { _, _ in true }
        )
    }

    /// `validAfter(6) ‖ validUntil(6)`: the signature's first 12 bytes.
    private func window(_ op: [String: Any]?) throws -> (after: UInt64, until: UInt64) {
        let signature = try #require(op?["signature"] as? String)
        let hex = signature.hasPrefix("0x") ? String(signature.dropFirst(2)) : signature
        try #require(hex.count >= 24)
        return (try #require(UInt64(hex.prefix(12), radix: 16)), try #require(UInt64(hex.dropFirst(12).prefix(12), radix: 16)))
    }

    private func draft() throws -> UserOpDraft {
        try userOpDraft(
            sender: fixture.account, nonce: "0x0", deployed: false,
            keyHexes: fixture.keys.map(\.publicKeyHex),
            calls: [UserOpCall(to: payee, value: "1000", data: "0x")],
            fee: .inBand(gasFeeToken: nil, amount: "1000", recipient: payee),
            floors: userOpFloors(chainId: 100, deployed: false, subCalls: 2)
        )
    }

    /// The core refuses to hash an operation that never expires: a spine
    /// that forgot to stamp it would sign nothing. Stamped, the window is
    /// open now and closes 30 minutes from the shell's clock.
    @Test func anUnstampedOperationIsRefusedAndAStampedOneClosesInThirtyMinutes() throws {
        let unstamped = try draft()
        #expect(unstamped.validAfter == 0 && unstamped.validUntil == 0, "a draft carries no window")
        #expect(throws: (any Error).self) { try userOpSafeOpHash(draft: unstamped, chainId: 100) }

        let stamped = try userOpStampValidity(draft: unstamped, nowMs: Self.t0 + 999)
        #expect(stamped.validAfter == 0)
        #expect(stamped.validUntil == UInt64(Self.t0 / 1000) + 1800)
        #expect(try userOpSafeOpHash(draft: stamped, chainId: 100).count == 32)
        // The window is in the hash: another window, another challenge.
        let later = try userOpStampValidity(draft: unstamped, nowMs: Self.t0 + 60_000)
        #expect(try userOpSafeOpHash(draft: later, chainId: 100) != userOpSafeOpHash(draft: stamped, chainId: 100))
    }

    /// The spine stamps the final operation from ITS clock just before the
    /// hash, and every submit is a new attempt with a window of its own.
    @Test func theSpineSignsEachAttemptWithItsOwnWindow() async throws {
        var clock = Self.t0
        let port = WindowCapture()
        let (spine, signer) = spine(port, now: { clock })

        _ = try await submit(spine)
        let first = try window(port.posted.first)
        #expect(first.after == 0)
        #expect(first.until == UInt64(Self.t0 / 1000) + 1800)

        clock += 120_000
        _ = try await submit(spine)
        let second = try window(port.posted.last)
        #expect(port.posted.count == 2)
        #expect(second.until == first.until + 120, "a second attempt stamps anew")
        #expect(signer.challenges.count == 2 && signer.challenges[0] != signer.challenges[1],
                "the window is in what the passkey signs")
    }

    /// The relay's AA22 — the window closed before the EntryPoint saw it —
    /// is the core's `Expired`, and the spine's `expired`: nothing was sent.
    @Test func theRelaysAA22IsExpiredAndNothingWasSent() async throws {
        let relay = ScriptedRelayPort()
        relay.detailed["eth_sendUserOperation"] = [RpcCallResult(
            outcome: .rpcError(code: -32500, message: "AA22 expired or not due"), maybeDelivered: false, heldErrorJson: nil
        )]
        let verdict = await RelayClient(port: relay, now: { 0 }, retryDelayMs: 0)
            .sendUserOp(chainId: 100, opJson: "{}", localHash: "0x" + String(repeating: "11", count: 32))
        #expect(verdict == .notSent(rejection: .expired))

        let port = WindowCapture()
        port.sendAnswer = RpcCallResult(
            outcome: .rpcError(code: -32500, message: "AA22 expired or not due"), maybeDelivered: false, heldErrorJson: nil
        )
        let (spine, _) = spine(port, now: { Self.t0 })
        var caught: UserOpSpine.Failure?
        do { _ = try await submit(spine) } catch let refused as UserOpSpine.Refused { caught = refused.failure }
        #expect(caught == .expired)
    }

    /// Where `expired` goes: Send hears `{"type":"expired"}`; a page is
    /// answered the core's fixed sentence as a relay's refusal (the core
    /// then says it calmly and offers Try again); a multisig's execution
    /// says it honestly.
    @Test func expiredIsSaidByEverySurfaceThatSubmits() throws {
        #expect(SendExecutor.failureWire(.expired)["type"] as? String == "expired")

        let outcome = SignExecutor.refusedOutcome(.expired, method: "eth_sendTransaction", chainId: 100)
        #expect(outcome["type"] as? String == "failed")
        #expect(outcome["message"] as? String == userOpExpiredDetail())
        #expect(outcome["refused"] as? Bool == true)
        #expect(!userOpExpiredDetail().isEmpty)

        #expect(MultisigLive.shellFailure(.expired)
                == .unknown("The transaction expired before it was submitted; nothing was sent."))
    }

    /// The receipt of an operation that expired offers Review again, as one
    /// whose nonce another send used does; its sentence resolves.
    @Test func anExpiredReceiptIsReviewedAgain() {
        #expect(SendLive.reviewsAgain("componentsUi.signing.expired"))
        #expect(SendLive.reviewsAgain("componentsUi.signing.wentFirst"))
        #expect(!SendLive.reviewsAgain(nil))
        #expect(!SendLive.reviewsAgain("componentsUi.signing.refused"))
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        #expect(en.t(I18nKeys.CoreRound.expired) == "It expired before it was submitted — nothing was sent.")
        #expect(I18nKeys.CoreRound.all.contains(I18nKeys.CoreRound.expired))
    }
}

// MARK: - 2. The relay's settlementGas past the cap

@MainActor
struct SettlementGasCapTests {
    private let en = Loc(overrideTag: "en", preferredLanguages: [])

    /// `settlement_gas_too_high` is a fee failure the core knows: its own
    /// reason, re-asked on the relay's schedule (as a gas quote too high
    /// is), and Continue's alert says the stopped sentence.
    @Test func aBillPastTheCapIsAQuoteVelaWontAccept() throws {
        let failure = "settlement_gas_too_high"
        #expect(feeFailureReasonKey(failure: failure) == I18nKeys.CoreRound.reasonSettlementHigh)
        #expect(sendEstimateFailureBodyKey(failure: failure) == I18nKeys.CoreRound.reasonSettlementHighStopped)
        #expect(feeRequoteDelayMs(failure: failure, attempt: 1) != nil, "the core asks again by itself")
        #expect(feeRequoteDelayMs(failure: failure, attempt: 1) == feeRequoteDelayMs(failure: "gas_quote_too_high", attempt: 1))
        #expect(en.t(I18nKeys.CoreRound.reasonSettlementHigh) != I18nKeys.CoreRound.reasonSettlementHigh)
        #expect(en.t(I18nKeys.CoreRound.reasonSettlementHighStopped).contains("won’t accept"))

        // The fee view decodes it, and Send hands it to its machine as it is.
        let view = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": false, "failed": failure, "fee": NSNull(),
            "stale": false, "fee_token": NSNull(), "options": [], "confirm_fee_ready": false,
        ])
        #expect(view.failed == failure)
        #expect(SendExecutor.estimateFailure(view.failed) as? String == failure)
        let alert = SendLive.alertText(["type": "estimate_failed", "kind": failure], loc: en, chain: "Gnosis")
        #expect(alert.title == en.t("send.alertEstimateFailedTitle"))
        #expect(alert.body == en.t(I18nKeys.CoreRound.reasonSettlementHighStopped))
    }
}

// MARK: - 3. A site asking a multisig to change who controls it

@MainActor
struct MultisigSelfCallWordsTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    /// The refusal's own words: its title, and its sentence with the
    /// function the core names — never the parent-Safe sentence, never the
    /// core's English message.
    @Test func aSelfCallRefusalIsSaidInItsOwnSentence() {
        let refusal = MsJSON(any: [
            "code": -32603, "message": "core english",
            "data": [
                "reason": "self_call", "safe": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                "function": "addOwnerWithThreshold", "leg": 1,
                "key": "componentsUi.signing.multisigSelfCallBlocked",
            ] as [String: Any],
        ] as [String: Any])
        let words = MsWords(loc: loc, hub: nil)
        let text = MultisigDappSheet.refusalText(refusal, words: words)
        #expect(text == loc.t("componentsUi.signing.multisigSelfCallBlocked", vars: ["function": "addOwnerWithThreshold"]))
        #expect(text.contains("addOwnerWithThreshold"))
        #expect(text.contains("Nothing was proposed"))
        #expect(MultisigDappSheet.refusalTitle(refusal, words: words) == loc.t("componentsUi.signing.selfCallBlockedTitle"))

        // Any other refusal keeps the proposal's title and the core's words.
        let other = MsJSON(any: ["code": -32603, "message": "core english", "data": NSNull()] as [String: Any])
        #expect(MultisigDappSheet.refusalTitle(other, words: words) == words.t("proposal.title"))
        #expect(MultisigDappSheet.refusalText(other, words: words) == "core english")
        for key in [I18nKeys.CoreRound.multisigSelfCallBlocked, I18nKeys.CoreRound.selfCallBlockedTitle] {
            #expect(loc.t(key) != key, "\(key) resolves")
        }
    }
}

// MARK: - 4. The relying party is the core's rule

@MainActor
struct RelyingPartyTests {

    private static let vectorURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/rules/rp-id.json")

    private func vector() throws -> [String: Any] {
        try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: Self.vectorURL)) as? [String: Any])
    }

    /// The shared vector's native cases, through this shell's door to the
    /// core's rule (`relying_party_json`) — and through the typed door the
    /// app uses (`relying_party_id`), which must agree.
    @Test func theVectorsNativeCasesReplayThroughTheCore() throws {
        let cases = try #require(try vector()["native"] as? [[String: Any]])
        #expect(cases.count == 14)
        for item in cases {
            let name = item["name"] as? String ?? "?"
            let ask = try #require(item["ask"] as? [String: Any])
            let answer = try CoreJSON.object(relyingPartyJson(askJson: CoreJSON.string(ask)))
            let testBuild = ask["testBuild"] as? Bool ?? false
            let simulator = ask["simulator"] as? Bool ?? false
            let domain = ask["testDomain"] as? String
            let typed = PasskeyRelyingParty.resolve(testBuild: testBuild, simulator: simulator, testDomain: domain)
            if let rpId = item["rpId"] as? String {
                #expect(answer["rpId"] as? String == rpId, "\(name): \(answer)")
                #expect(typed == .success(rpId), "\(name)")
            } else {
                let kind = (answer["error"] as? [String: Any])?["kind"] as? String
                #expect(kind == item["error"] as? String, "\(name): \(answer)")
                guard case .failure(let unavailable) = typed else {
                    Issue.record("\(name): the typed door answered \(typed)")
                    continue
                }
                let message = (answer["error"] as? [String: Any])?["message"] as? String
                #expect(unavailable.message == message, "\(name): the core's own sentence")
            }
        }
    }

    /// This build's facts: a test build (every Debug build) on the
    /// simulator is `localhost`; release is `getvela.app` whatever the
    /// domain; a device test with no domain is the core's sentence saying
    /// how to set one — never `getvela.app`.
    @Test func theShellHandsTheCoreThisBuildsFacts() throws {
        #expect(PasskeyRelyingParty.testBuild, "the tests run a Debug build")
        #expect(PasskeyRelyingParty.simulator, "the tests run on a simulator")
        #expect(PasskeyRelyingParty.thisBuild == .success("localhost"))
        #expect(PasskeyExecutor.relyingParty == "localhost")
        #expect(PasskeyExecutor().relyingPartyId == "localhost")

        #expect(PasskeyRelyingParty.resolve(testBuild: false, simulator: false, testDomain: "vela-test.example")
                == .success("getvela.app"))
        #expect(PasskeyRelyingParty.resolve(testBuild: true, simulator: false, testDomain: "vela-test.example")
                == .success("vela-test.example"))
        guard case .failure(let unset) = PasskeyRelyingParty.resolve(testBuild: true, simulator: false, testDomain: nil) else {
            Issue.record("a device test with no domain must refuse")
            return
        }
        #expect(unset.message.contains("TEST_RP_DOMAIN"))
        #expect(unset.message.hasPrefix("This test build has no passkey domain."))

        // The build setting reaches the app through the Info.plist, empty by
        // default; an empty one is no domain.
        #expect(Bundle.main.object(forInfoDictionaryKey: PasskeyRelyingParty.infoKey) as? String == "")
        #expect(PasskeyRelyingParty.configuredDomain([PasskeyRelyingParty.infoKey: ""]) == nil)
        #expect(PasskeyRelyingParty.configuredDomain(nil) == nil)
        #expect(PasskeyRelyingParty.configuredDomain([PasskeyRelyingParty.infoKey: "vela-test.example"]) == "vela-test.example")
    }

    /// The system sheet's requests are made for the core's relying party;
    /// with none, the ceremony fails with the core's sentence before any
    /// request is made.
    @Test func theCeremonyAsksForTheCoresPartyOrFailsInItsWords() async throws {
        let challenge = Data(repeating: 7, count: 32)
        let device = PasskeyExecutor(relyingParty: .success("vela-test.example"))
        let requests = try device.systemRequests(challenge: challenge, credentialIdHex: nil, transports: "")
        #expect(!requests.isEmpty)
        for request in requests {
            if let platform = request as? ASAuthorizationPlatformPublicKeyCredentialAssertionRequest {
                #expect(platform.relyingPartyIdentifier == "vela-test.example")
            } else if let key = request as? ASAuthorizationSecurityKeyPublicKeyCredentialAssertionRequest {
                #expect(key.relyingPartyIdentifier == "vela-test.example")
            }
        }

        let sentence = "This test build has no passkey domain."
        let unset = PasskeyExecutor(relyingParty: .failure(.init(message: sentence)))
        #expect(unset.relyingPartyId == "")
        do {
            _ = try unset.systemRequests(challenge: challenge, credentialIdHex: nil, transports: "")
            Issue.record("no request may be made without a relying party")
        } catch let failure as PasskeyFailure {
            #expect(failure.message == sentence)
            #expect(failure.kind == .notSupported)
        }
        do {
            _ = try await unset.register(name: "Mine", excludeCredentialIds: [], method: .platform)
            Issue.record("no registration may be asked without a relying party")
        } catch let failure as PasskeyFailure {
            #expect(failure.message == sentence)
        }
    }
}
