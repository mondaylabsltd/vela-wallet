//
//  TrustedSignerRouteTests.swift
//  VelaWalletTests
//
//  Spec 102 on iOS: the trusted page is WHERE a person reviews and signs, not
//  a fourth place a key lives.
//
//  071's tests (`TrustedSignerTests`) cover signing there. These cover the
//  rest of the model:
//
//  - a wallet on its own domain runs its key ceremonies on its page (R3) — a
//    session of several requests, create then that key's member proof, spoken
//    to the way the page speaks to it;
//  - the create and sign-in choosers listing THREE places, and the venue's
//    words;
//  - the account's signing plan — key, domain, venue — taking each signature
//    where it belongs, old records migrated by the core;
//  - the onboarding executor routing by the operation's `page`, reporting the
//    core's verdict as the machine result, a closed page as a cancel.
//
//  Nothing here fakes a verdict. Every answer goes through the core
//  (`trustedSignerVerifyCeremony`, `trustedSignerVerify`, `signingPlan`), and
//  the attestation is the conformance vectors' real one.
//

import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - A page that runs ceremonies

/// What a wallet's own signing page answers for the four ceremonies, built from
/// the repository's own conformance vectors so the core judges real bytes.
/// The page is on the person's own domain: only such a wallet runs its
/// ceremonies on a page (R3).
struct TrustedSignerCeremonyFixture {
    let key = P256.Signing.PrivateKey()
    let signerUrl = "https://sign.example.com/"
    let origin = "https://sign.example.com"
    /// The credential the vector's attestation object carries.
    let credential = Data(repeating: 0xCD, count: 16)

    var credentialHex: String { TrustedSignerFixture.hex(credential) }
    var credentialB64: String { TrustedSignerFixture.base64url(credential) }

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
            #"{"type":"webauthn.create","challenge":"\#(TrustedSignerFixture.base64url(challenge))","origin":"\#(origin)","crossOrigin":false}"#.utf8
        )
        return [
            "registration": [
                "credentialId": credentialB64,
                "attestationObject": Self.attestationObjectHex,
                "clientDataJSON": TrustedSignerFixture.hex(clientData),
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
            #"{"type":"webauthn.get","challenge":"\#(TrustedSignerFixture.base64url(challenge))","origin":"\#(origin)","crossOrigin":false}"#.utf8
        )
        var signed = authenticatorData
        signed.append(Data(SHA256.hash(data: clientData)))
        let signature = try! key.signature(for: signed)
        return [
            "assertion": [
                "credentialId": credentialB64,
                "signatureDer": TrustedSignerFixture.hex(signature.derRepresentation),
                "authenticatorData": TrustedSignerFixture.hex(authenticatorData),
                "clientDataJSON": TrustedSignerFixture.hex(clientData),
                "userHandle": NSNull(),
                "authenticatorAttachment": "platform",
            ],
            "origin": origin,
        ]
    }

    /// A `register_passkey` on this wallet's page, as the create machine
    /// sends it: the place is the method, the page is `page` (spec 102).
    func registerOperation(name: String = "Mine", method: String = "platform") -> [String: Any] {
        [
            "type": "register_passkey", "name": name,
            "exclude_credential_ids": [] as [String], "method": method, "page": signerUrl,
        ]
    }

    /// The member proof for the key just minted.
    func memberProofOperation(groupPublicKey: String = "04" + String(repeating: "ab", count: 64))
        -> [String: Any] {
        [
            "type": "sign_member_proof", "credential_id": credentialHex,
            "public_key_hex": "04" + String(repeating: "cd", count: 64),
            "attestation_hex": "", "transports": "internal",
            "method": "platform", "group_public_key_hex": groupPublicKey, "page": signerUrl,
        ]
    }

    static func json(_ object: [String: Any]) -> String {
        String(decoding: (try? JSONSerialization.data(withJSONObject: object)) ?? Data(), as: UTF8.self)
    }
}

// MARK: - One session, several requests

/// The deployment a member proof is bound to, as `/api/health` names it.
let TEST_DEPLOYMENT = SignerRegistryDeployment(
    chainId: 100,
    contract: "0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf"
)

@MainActor
struct TrustedSignerSessionTests {

    /// A create and then its member proof (contract §1.5). On the custom
    /// scheme that is TWO visits — a URL carries one request — so what makes
    /// them one flow is this side: the same channel, the same page, one
    /// `end()`, and a fresh one-time token for each.
    ///
    /// This is the shape of a create on the person's own signing page.
    @Test func aCreateAndItsMemberProofRunOnOneFlow() async throws {
        let page = TrustedSignerCeremonyFixture()
        let registry = "https://p256-index-v2.getvela.app"
        let register = page.registerOperation()
        let registerRequest = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(register),
            id: "ignored", walletName: "Mine", registry: registry,
            deployment: TEST_DEPLOYMENT
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl, launch: TestAdmissions.launcher(page.signerUrl),
            first: .ceremony(
                request: registerRequest,
                operationJson: TrustedSignerCeremonyFixture.json(register),
                memberChallenge: nil
            )
        )
        var opened: [URL] = []
        let first = try PageVisit(try #require(channel.start()))

        // 1. Create. The page is asked for `vela_createPasskey` and draws its
        //    own challenge; the wallet only checks what comes back.
        #expect(first.intent["method"] as? String == "vela_createPasskey")
        #expect(first.context["walletName"] as? String == "Mine")
        // The page the wallet CHECKED is the page that opened (R6), on the
        // wallet's own domain — not the official one.
        #expect(first.url.absoluteString.hasPrefix(page.signerUrl + "b/"))
        // R5: told where the key is to live, so the browser goes straight there.
        let createParams = try #require((first.intent["params"] as? [[String: Any]])?.first)
        #expect(createParams["place"] as? String == "platform")
        #expect(createParams["hints"] as? [String] == ["client-device"])
        #expect(first.answer(page.createAnswer()))
        guard case .ceremony(.registered(let registrationJson)) = await channel.ending() else {
            Issue.record("the page's registration was not accepted")
            return
        }
        let registration = try CoreJSON.object(registrationJson)
        #expect(registration["credential_id"] as? String == page.credentialHex)
        // Where the key lives from now on — stamped by the core, from the page
        // the wallet opened, never from anything the page said about itself.
        #expect(registration["signer_origin"] as? String == page.origin)

        // 2. The member proof, a second visit of the same flow. The wallet's
        //    own registry challenge rides along and the core demands equality.
        let challenge = Data(repeating: 0x42, count: 32)
        let member = page.memberProofOperation()
        let memberRequest = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(member),
            id: "ignored", walletName: "Mine", registry: registry,
            deployment: TEST_DEPLOYMENT
        ))
        let firstToken = channel.token
        // The page answers the second visit as it is opened, so nothing here
        // waits on a guess about when the channel gets there.
        var asked: [String: Any] = [:]
        channel.openPage = { url in
            opened.append(url)
            guard let visit = try? PageVisit(url) else { return }
            asked = visit.intent
            _ = visit.answer(page.assertionAnswer(challenge: challenge))
        }
        let ending = await channel.send(.ceremony(
            request: memberRequest,
            operationJson: TrustedSignerCeremonyFixture.json(member),
            memberChallenge: challenge
        ))
        #expect(opened == [try #require(channel.launchUrl)], "the next request opens the page again")
        #expect(channel.token != firstToken, "a one-time token per request")
        #expect(asked["method"] as? String == "vela_memberProof")
        let params = try #require((asked["params"] as? [[String: Any]])?.first)
        #expect(params["registry"] as? String == registry, "the page fetches from the wallet's registry")
        #expect(params["credentialId"] as? String == page.credentialB64)
        guard case .ceremony(.asserted(let assertionJson)) = ending else {
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
        channel.end()
    }

    /// A person who closes the page instead of answering the flow's SECOND
    /// request has closed without signing it — a decline, not a channel that
    /// could not be opened. The two get different sentences on screen, and
    /// telling somebody "the answer does not match" when they closed a tab is
    /// a lie about what they did.
    @Test func aRequestTheSecondVisitRefusesIsADecline() async throws {
        let page = TrustedSignerCeremonyFixture()
        let register = page.registerOperation()
        let request = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(register),
            id: "x", walletName: "Mine", registry: "https://r.test",
            deployment: TEST_DEPLOYMENT
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl, launch: TestAdmissions.launcher(page.signerUrl),
            first: .ceremony(request: request,
                             operationJson: TrustedSignerCeremonyFixture.json(register),
                             memberChallenge: nil)
        )
        let first = try PageVisit(try #require(channel.start()))
        #expect(first.answer(page.createAnswer()))
        guard case .ceremony(.registered) = await channel.ending() else {
            Issue.record("the registration was not accepted")
            return
        }
        let member = page.memberProofOperation()
        let memberRequest = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(member),
            id: "y", walletName: "Mine", registry: "https://r.test",
            deployment: TEST_DEPLOYMENT
        ))
        channel.openPage = { url in
            _ = try? PageVisit(url).refuse("user_rejected")
        }
        let ending = await channel.send(.ceremony(
            request: memberRequest,
            operationJson: TrustedSignerCeremonyFixture.json(member),
            memberChallenge: Data(repeating: 1, count: 32)
        ))
        #expect(ending == .ceremony(.refused(refusal: .declined)))
        #expect(TrustedSignerNotice(ending: ending) == .closed)
        channel.end()
    }

    /// A member proof over a challenge the page did NOT fetch for these
    /// inputs is refused — the one check that keeps "confirm this key joins
    /// your wallet" from being a signature over something else.
    @Test func aMemberProofOverAnotherChallengeIsRefused() async throws {
        let page = TrustedSignerCeremonyFixture()
        let member = page.memberProofOperation()
        let request = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(member),
            id: "x", walletName: "Mine", registry: "https://r.test",
            deployment: TEST_DEPLOYMENT
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl, launch: TestAdmissions.launcher(page.signerUrl),
            first: .ceremony(
                request: request,
                operationJson: TrustedSignerCeremonyFixture.json(member),
                memberChallenge: Data(repeating: 0x01, count: 32)
            )
        )
        let visit = try PageVisit(try #require(channel.start()))
        #expect(visit.answer(page.assertionAnswer(challenge: Data(repeating: 0x02, count: 32))))
        #expect(await channel.ending() == .ceremony(.refused(refusal: .wrongChallenge)))
        channel.end()
    }

    /// A page that answers from another origin is refused before the answer is
    /// used — the wallet checks the origin the AUTHENTICATOR signed, not the
    /// one the page writes beside it.
    @Test func anAnswerFromAnotherOriginIsRefused() async throws {
        let page = TrustedSignerCeremonyFixture()
        let register = page.registerOperation()
        let request = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(register),
            id: "x", walletName: "Mine", registry: "https://r.test",
            deployment: TEST_DEPLOYMENT
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl, launch: TestAdmissions.launcher(page.signerUrl),
            first: .ceremony(request: request,
                             operationJson: TrustedSignerCeremonyFixture.json(register),
                             memberChallenge: nil)
        )
        let visit = try PageVisit(try #require(channel.start()))
        var answer = page.createAnswer()
        // The signed clientDataJSON says somewhere else.
        let clientData = Data(
            #"{"type":"webauthn.create","challenge":"AAAA","origin":"https://evil.example","crossOrigin":false}"#.utf8
        )
        var registration = answer["registration"] as? [String: Any] ?? [:]
        registration["clientDataJSON"] = TrustedSignerFixture.hex(clientData)
        answer["registration"] = registration
        #expect(visit.answer(answer))
        guard case .ceremony(.refused(.malformed)) = await channel.ending() else {
            Issue.record("an answer from another origin was accepted")
            return
        }
        channel.end()
    }
}

// MARK: - The choosers

@MainActor
struct TrustedSignerChooserTests {
    private var loc: Loc { Loc(overrideTag: "zh", preferredLanguages: []) }

    /// The create key-method picker and the sign-in method sheet both render
    /// `KeyMethod.allCases`: three places, no fourth (spec 102), each with a
    /// title and a line in the person's language. The core no longer knows a
    /// `trusted_signer` method either.
    @Test func theCreateAndSignInChoosersListThreePlaces() {
        #expect(KeyMethod.allCases == [.platform, .hybrid, .securityKey])
        for chooser in [KeyChooser.create, .signIn] {
            let titles = KeyMethod.allCases.map { methodCopy($0, chooser: chooser, loc: loc).title }
            #expect(!titles.contains("可信签名器"))
            #expect(Set(titles).count == 3, "two places share a title")
        }
        #expect(KeyMethod(rawValue: "trusted_signer") == nil)
        #expect(keyMethodWords(method: "trusted_signer", chooser: "create", unlock: "face_id") == nil)
    }

    /// "Use my own signing page" is the venue's words, not a fourth place —
    /// and so are the two rows of "Where you review and sign".
    @Test func theVenueHasItsOwnWords() throws {
        let own = try #require(venueWords(row: "own_page"))
        #expect(loc.t(own.titleKey) == "使用我自己的签名页")
        #expect(own.lineKey.map { loc.t($0) } == "高级：钥匙属于你页面的域名，只能在那里签名")
        let inVela = try #require(venueWords(row: "in_vela"))
        let onAPage = try #require(venueWords(row: "page"))
        #expect(loc.t(inVela.titleKey) == "在 Vela 里")
        #expect(loc.t(onAPage.titleKey) == "在可信签名页")
        #expect(venueWords(row: "trusted_signer") == nil)
    }

    /// Settings has no pairing-service row at all: the channel went on
    /// 2026-09-23 and its address went with it. A row left behind would be a
    /// setting for something the wallet no longer opens.
    @Test func settingsHasNoPairingServiceRow() {
        let ids = SettingsFixtures.build(.st1, loc: loc).sections.flatMap(\.rows).map(\.id)
        #expect(!ids.contains { $0.localizedCaseInsensitiveContains("tunnel") })
        #expect(!ids.contains { $0.localizedCaseInsensitiveContains("relay") })
    }
}

// MARK: - The signing plan: key, domain, venue

@MainActor
struct SigningPlanTests {
    private let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func record(_ extra: [String: Any], keyOrigin: String? = nil) -> String {
        var key: [String: Any] = [
            "credential_id": "cred-1", "public_key_hex": "04" + String(repeating: "11", count: 64),
            "name": "Mine", "transports": "internal",
        ]
        if let keyOrigin { key["signer_origin"] = keyOrigin }
        var record: [String: Any] = [
            "id": "cred-1", "address": address, "name": "Mine",
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "created_at_iso": "2026-09-01T00:00:00.000Z", "keys": [key],
        ]
        record.merge(extra) { _, new in new }
        return String(decoding: try! JSONSerialization.data(withJSONObject: record), as: UTF8.self)
    }

    /// A new record: its key, its domain and its venue, as written.
    @Test func aNewRecordIsReadAsWritten() throws {
        let plan = try #require(SigningPlanWire.of(accountJson: record([
            "sign_in_key": ["credential_id": "cred-1", "method": "security_key", "transports": "usb,nfc"],
            "signing_domain": "getvela.app",
            "signing_venue": ["type": "page", "url": "https://sign.getvela.app/"],
        ])))
        #expect(plan.onAppDomain)
        #expect(plan.venue == .page(url: "https://sign.getvela.app/"))
        #expect(plan.blocked == nil)
        let key = try #require(plan.key)
        #expect(key.credentialId == "cred-1" && key.place == .securityKey)
        #expect(key.hints == ["security-key"])
    }

    /// Spec 102's migration, as the core reads ≤ 0.9.7 records: a Trusted
    /// Signer sign-in on the official page (or none named) reviews on the
    /// official page; one on somebody's own page is locked to it; a place
    /// sign-in signs in Vela. The key's place comes from its transports — the
    /// fourth "method" is gone.
    @Test func oldRecordsMigrateToAVenue() throws {
        let official = try #require(SigningPlanWire.of(accountJson: record([
            "signed_in_with": ["credential_id": "cred-1", "method": "trusted_signer",
                               "signer_origin": "https://sign.getvela.app"],
        ])))
        #expect(official.onAppDomain && official.venue == .page(url: "https://sign.getvela.app/"))
        #expect(official.key?.place == .platform)

        let unnamed = try #require(SigningPlanWire.of(accountJson: record([
            "signed_in_with": ["credential_id": "cred-1", "method": "trusted_signer"],
        ])))
        #expect(unnamed.venue == .page(url: "https://sign.getvela.app/"))

        let own = try #require(SigningPlanWire.of(accountJson: record([
            "signed_in_with": ["credential_id": "cred-1", "method": "trusted_signer",
                               "signer_origin": "https://sign.example.com"],
        ], keyOrigin: "https://sign.example.com")))
        #expect(own.domain == "sign.example.com" && !own.onAppDomain)
        #expect(own.venue.pageUrl?.hasPrefix("https://sign.example.com") == true)

        let app = try #require(SigningPlanWire.of(accountJson: record([
            "signed_in_with": ["credential_id": "cred-1", "method": "platform"],
        ])))
        #expect(app.venue == .inVela && app.onAppDomain)

        let before = try #require(SigningPlanWire.of(accountJson: record([:])))
        #expect(before.key == nil, "a record from before the sign-in key names none")
        #expect(before.venue == .inVela)
    }

    /// The spine opens the page the PLAN names — the account's own page, for
    /// an account locked to its domain — and offers it every founding key
    /// when the record names no sign-in key.
    @Test func theSpineOpensThePageThePlanNames() async throws {
        let fixture = TrustedSignerFixture()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        var stored = try CoreJSON.object(record([:], keyOrigin: "https://sign.example.com"))
        stored["keys"] = [[
            "credential_id": fixture.credentialHex, "public_key_hex": fixture.keys[0].publicKeyHex,
            "name": "Mine", "transports": "internal", "signer_origin": "https://sign.example.com",
        ]]
        stored["address"] = fixture.account
        accounts.recordJson = String(decoding: try JSONSerialization.data(withJSONObject: stored), as: UTF8.self)
        let signer = CountingSigner()
        let page = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let relay = RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { signer })
        spine.trustedSigner = page

        _ = try await spine.signMessage(
            chainId: 100, account: fixture.account, originalHash: Data(repeating: 1, count: 32),
            asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
        )
        #expect(signer.calls == 0, "a key on its own domain must never reach the platform sheet")
        #expect(page.asked.first?.page.hasPrefix("https://sign.example.com") == true)
        #expect(page.asked.first?.place == nil, "a record with no sign-in key names no place")
    }
}

// MARK: - The executor

/// The wallet's own page as the onboarding executor reaches it, scripted.
@MainActor
final class ScriptedCeremonyPort: TrustedSignerCeremonyPort {
    var answer: (_ operationJson: String) -> TrustedSignerCeremonyStep
    private(set) var asked: [(operationJson: String, registry: String, walletName: String, challenge: Data?, page: String)] = []
    private(set) var ended = 0

    init(answer: @escaping (_ operationJson: String) -> TrustedSignerCeremonyStep) {
        self.answer = answer
    }

    /// What the last ceremony was told the deployment was, so a test can check
    /// a member proof carries one and nothing else does.
    var deployments: [SignerRegistryDeployment?] = []

    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String,
        deployment: SignerRegistryDeployment?
    ) async -> TrustedSignerCeremonyStep {
        asked.append((operationJson, registry, walletName, expectedMemberChallenge, page))
        deployments.append(deployment)
        return answer(operationJson)
    }

    func endFlow() { ended += 1 }
}

@MainActor
struct TrustedSignerExecutorTests {

    private func executor(_ port: ScriptedCeremonyPort) -> OnboardingExecutor {
        OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test", transport: IndexScript.unreachable),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            trustedSigner: port,
            walletName: { "Mine" }
        )
    }

    /// A `register_passkey` that names a page goes to THAT page (R3), and the
    /// core's `Registration` — `signer_origin` and all — is reported as the
    /// result a platform ceremony would have given.
    @Test func aCreateOnItsOwnPageIsReportedAsPasskeyRegistered() async throws {
        let page = TrustedSignerCeremonyFixture()
        let registration = TrustedSignerCeremonyFixture.json([
            "credential_id": page.credentialHex,
            "attestation_object_hex": TrustedSignerCeremonyFixture.attestationObjectHex,
            "client_data_json_hex": "00", "authenticator_attachment": "platform",
            "transports": "internal", "signer_origin": page.origin,
        ])
        let port = ScriptedCeremonyPort { _ in .registered(registration) }
        let answer = try CoreJSON.object(await executor(port).perform(page.registerOperation()))
        #expect(answer["type"] as? String == "passkey_registered")
        let reported = try #require(answer["registration"] as? [String: Any])
        #expect(reported["signer_origin"] as? String == page.origin)
        #expect(reported["credential_id"] as? String == page.credentialHex)
        // The page is the operation's own, and told the wallet's name and
        // registry.
        #expect(port.asked.first?.page == page.signerUrl)
        #expect(port.asked.first?.walletName == "Mine")
        #expect(port.asked.first?.registry == "https://r.test")
        // The operation goes over verbatim, so nothing is copied wrongly.
        #expect(try CoreJSON.object(port.asked.first?.operationJson ?? "{}")["name"] as? String == "Mine")
    }

    /// A page that was closed is a CANCEL — the same as a dismissed system
    /// sheet, and never an error alert (contract §5).
    @Test func aClosedPageIsACancelAndARefusalCarriesItsSentence() async throws {
        let page = TrustedSignerCeremonyFixture()
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

    /// A page named where none can be opened — a preview, the gallery — fails
    /// closed rather than silently signing with the platform authenticator,
    /// in the corpus's words.
    @Test func aPageWithNoWayToOpenItFailsClosed() async throws {
        let page = TrustedSignerCeremonyFixture()
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let executor = OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test", transport: IndexScript.unreachable),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            trustedSigner: nil,
            words: { loc.t($0) }
        )
        let answer = try CoreJSON.object(await executor.perform(page.registerOperation()))
        #expect(answer["type"] as? String == "passkey_failed")
        #expect(answer["kind"] as? String == "not_supported")
        #expect(answer["message"] as? String == loc.t("componentsUi.signing.signerDown"))
    }

    /// Only an operation that NAMES a page goes to one — checked on the
    /// decision rather than by running the other route, because running it
    /// raises the system passkey sheet and a unit test has nobody to answer
    /// it. The method is the place, never a route: no value of it opens a
    /// page.
    @Test func onlyAnOperationThatNamesAPageReachesIt() throws {
        var operation = TrustedSignerCeremonyFixture().registerOperation()
        #expect(OnboardingExecutor.page(of: operation) == "https://sign.example.com/")
        for method in ["platform", "hybrid", "security_key", "trusted_signer", ""] {
            operation["method"] = method
            #expect(OnboardingExecutor.page(of: operation) != nil)
        }
        operation["page"] = ""
        #expect(OnboardingExecutor.page(of: operation) == nil, "an empty page is no page")
        operation.removeValue(forKey: "page")
        operation["method"] = "trusted_signer"
        #expect(OnboardingExecutor.page(of: operation) == nil,
                "an old method name must not open a page on its own")
    }

    /// The re-publish's LIVE member proof (recovery's third signature) goes to
    /// the page the CORE named on the publish — the operation's `page`, never
    /// a lookup here — over the publish's place; with no page, to none.
    @Test func aPublishMemberSignsOnThePageThePublishNames() throws {
        let group = "04" + String(repeating: "ab", count: 64)
        let member = PublishMember(json: [
            "credential_id": "cred-1", "public_key_hex": "04" + String(repeating: "11", count: 64),
            "attestation_hex": "beef", "transports": "internal",
        ])
        let routed = OnboardingExecutor.memberProofOperation(
            member, groupPublicKey: group, method: .hybrid, page: "https://sign.example.com/"
        )
        #expect(routed["method"] as? String == "hybrid")
        #expect(routed["page"] as? String == "https://sign.example.com/")
        #expect(routed["credential_id"] as? String == "cred-1")
        #expect(routed["group_public_key_hex"] as? String == group)
        #expect(OnboardingExecutor.page(of: routed) == "https://sign.example.com/")
        // The core reads it as the ceremony it is, from this very JSON.
        let request = trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(routed),
            id: "x", walletName: "Mine", registry: "https://r.test",
            deployment: TEST_DEPLOYMENT
        )
        // The deployment reaches the page's params, which is what lets it
        // compute the member challenge without asking the network (076) — and
        // the place, so the browser asks for the key where it lives (R5).
        let built = try CoreJSON.object(try #require(request))
        let intent = try #require(built["intent"] as? [String: Any])
        let params = try #require((intent["params"] as? [[String: Any]])?.first)
        #expect((params["chainId"] as? NSNumber)?.uint64Value == TEST_DEPLOYMENT.chainId)
        #expect(params["registryContract"] as? String == TEST_DEPLOYMENT.contract)
        #expect(params["place"] as? String == "hybrid")

        let unrouted = OnboardingExecutor.memberProofOperation(member, groupPublicKey: group, method: .platform, page: nil)
        #expect(unrouted["page"] == nil, "a wallet in the app must not be sent to a page")
        #expect(OnboardingExecutor.page(of: unrouted) == nil)
    }
}

/// The two operations whose outside world is the UI — neither is reached here.
@MainActor
private final class NoDeps: OnboardingExecutorDeps {
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool { false }
    func complete(mode: [String: Any]) async {}
}
