//
//  TrustedSignerRouteTests.swift
//  VelaWalletTests
//
//  Spec 075 on iOS: the Trusted Signer as a fourth PASSKEY ROUTE.
//
//  071's tests (`TrustedSignerTests`) cover it as a way to sign. These cover
//  what it became:
//
//  - a session of several requests on one page visit — create, then that
//    key's member proof, then `bye` — spoken to the way the page speaks to it;
//  - the create and sign-in choosers listing four routes, and the signing
//    sheet five;
//  - `signer_origin` round-tripping through the account record and taking the
//    ceremony back to the page the key lives behind;
//  - the onboarding executor reporting the core's verdict as the machine
//    result, and a closed page as a cancel rather than an error.
//
//  Nothing here fakes a verdict. Every answer goes through the core
//  (`trustedSignerVerifyCeremony`, `trustedSignerVerify`, `signRoute`), and the
//  attestation is the conformance vectors' real one.
//

import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - A page that runs ceremonies

/// What a Trusted Signer page answers for the four ceremonies, built from the
/// repository's own conformance vectors so the core judges real bytes.
struct TrustedSignerCeremonyFixture {
    let key = P256.Signing.PrivateKey()
    let signerUrl = "https://sign.getvela.app/"
    let origin = "https://sign.getvela.app"
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

    /// A `register_passkey` operation with `method = trusted_signer`, as the
    /// create machine sends it.
    func registerOperation(name: String = "Mine") -> [String: Any] {
        [
            "type": "register_passkey", "name": name,
            "exclude_credential_ids": [] as [String], "method": "trusted_signer",
        ]
    }

    /// The member proof for the key just minted.
    func memberProofOperation(groupPublicKey: String = "04" + String(repeating: "ab", count: 64))
        -> [String: Any] {
        [
            "type": "sign_member_proof", "credential_id": credentialHex,
            "public_key_hex": "04" + String(repeating: "cd", count: 64),
            "attestation_hex": "", "transports": "internal",
            "method": "trusted_signer", "group_public_key_hex": groupPublicKey,
        ]
    }

    static func json(_ object: [String: Any]) -> String {
        String(decoding: (try? JSONSerialization.data(withJSONObject: object)) ?? Data(), as: UTF8.self)
    }
}

// MARK: - One session, several requests

@MainActor
struct TrustedSignerSessionTests {

    /// A create and then its member proof (contract §1.5). On the custom
    /// scheme that is TWO visits — a URL carries one request — so what makes
    /// them one flow is this side: the same channel, the same page, one
    /// `end()`, and a fresh one-time token for each.
    ///
    /// This is the shape of the whole 075 create flow.
    @Test func aCreateAndItsMemberProofRunOnOneFlow() async throws {
        let page = TrustedSignerCeremonyFixture()
        let registry = "https://p256-index-v2.getvela.app"
        let register = page.registerOperation()
        let registerRequest = try #require(trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(register),
            id: "ignored", walletName: "Mine", registry: registry
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl,
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
            id: "ignored", walletName: "Mine", registry: registry
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
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl,
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
            id: "y", walletName: "Mine", registry: "https://r.test"
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
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl,
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
            id: "x", walletName: "Mine", registry: "https://r.test"
        ))
        let channel = TrustedSignerChannel(
            signerUrl: page.signerUrl,
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
    /// `KeyMethod.allCases`, so this is what puts the fourth route on both of
    /// them (spec 075 SC-001): four routes, no `auto`, each with a title and a
    /// line in the person's language.
    @Test func theCreateAndSignInChoosersListFourRoutes() {
        #expect(KeyMethod.allCases == [.platform, .hybrid, .securityKey, .trustedSigner])
        let titles = KeyMethod.allCases.map { loc.t(methodCopy($0).title) }
        #expect(titles.last == "可信签名器")
        #expect(Set(titles).count == 4, "two routes share a title")
        #expect(loc.t(methodCopy(.trustedSigner).body) == "在独立的页面上核对并签名——所见即所签。")
        // A method that arrives from the core by name resolves to the route.
        #expect(KeyMethod(rawValue: "trusted_signer") == .trustedSigner)
    }

    /// The signing sheet's "Sign with" has an `auto`, so it lists FIVE — and
    /// the Trusted Signer is the only one with a line under it, because the
    /// other four say where a key is and this one says what it does.
    @Test func theSigningSheetListsFiveWithTheTrustedSignersLine() throws {
        let offered = try #require(SignPrefViewWire.initial).offered
        #expect(offered == ["auto", "platform", "hybrid", "security_key", "trusted_signer"])
        let titles = offered.compactMap { SigningLive.signMethodTitle($0, loc: loc) }
        #expect(titles.count == 5)
        #expect(titles.last == "可信签名器")
        #expect(SigningLive.signMethodDetail("trusted_signer", loc: loc) != nil)
        #expect(offered.dropLast().allSatisfy { SigningLive.signMethodDetail($0, loc: loc) == nil })
    }

    /// Settings has no pairing-service row at all: the channel went on
    /// 2026-09-23 and its address went with it. A row left behind would be a
    /// setting for something the wallet no longer opens.
    @Test func settingsHasNoPairingServiceRow() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        let executor = SignPrefExecutor(store: store)
        let view = try Self.run(SignPrefCore(), executor, ["type": "refresh"])
        let model = SettingsLive.withSignPref(view, on: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        let ids = model.sections.flatMap(\.rows).map(\.id)
        #expect(!ids.contains { $0.localizedCaseInsensitiveContains("tunnel") })
        #expect(!ids.contains { $0.localizedCaseInsensitiveContains("relay") })
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
        #expect(route.method == "trusted_signer")
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
        let fixture = TrustedSignerFixture()
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.routesJson = TrustedSignerCeremonyFixture.json([
            "credential_id": fixture.credentialHex,
            "transports": "internal",
            "signer_origin": "https://sign.example",
        ]).withSquareBrackets
        let signer = CountingSigner()
        let page = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let relay = RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0)
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { signer })
        // "Automatic" — nobody chose the Trusted Signer, the KEY did.
        spine.signMethod = { "auto" }
        spine.trustedSigner = page

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

/// The Trusted Signer as the onboarding executor reaches it, scripted.
@MainActor
final class ScriptedCeremonyPort: TrustedSignerCeremonyPort {
    var answer: (_ operationJson: String) -> TrustedSignerCeremonyStep
    private(set) var asked: [(operationJson: String, registry: String, walletName: String, challenge: Data?, page: String?)] = []
    private(set) var ended = 0

    init(answer: @escaping (_ operationJson: String) -> TrustedSignerCeremonyStep) {
        self.answer = answer
    }

    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String?
    ) async -> TrustedSignerCeremonyStep {
        asked.append((operationJson, registry, walletName, expectedMemberChallenge, page))
        return answer(operationJson)
    }

    func endFlow() { ended += 1 }
}

@MainActor
struct TrustedSignerExecutorTests {

    private func executor(_ port: ScriptedCeremonyPort) -> OnboardingExecutor {
        OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test"),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            trustedSigner: port,
            walletName: { "Mine" }
        )
    }

    /// A `register_passkey` with `method = trusted_signer` goes to the page, and
    /// the core's `Registration` — `signer_origin` and all — is reported as
    /// the result a platform ceremony would have given.
    @Test func aCreateOnTheTrustedSignerIsReportedAsPasskeyRegistered() async throws {
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
        // The page is told the wallet's own name and its own registry.
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

    /// Chosen where no page can be opened — a preview, the gallery — fails
    /// closed rather than silently signing with the platform authenticator.
    @Test func theRouteWithNoPageFailsClosed() async throws {
        let page = TrustedSignerCeremonyFixture()
        let executor = OnboardingExecutor(
            passkey: PasskeyExecutor(),
            registry: RegistryClient(baseURL: "https://r.test"),
            store: AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!),
            deps: NoDeps(),
            trustedSigner: nil
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
        var operation = TrustedSignerCeremonyFixture().registerOperation()
        #expect(OnboardingExecutor.routesToTheTrustedSigner(operation))
        for other in ["platform", "hybrid", "security_key", "", "something_else"] {
            operation["method"] = other
            #expect(!OnboardingExecutor.routesToTheTrustedSigner(operation),
                    "\(other) would have opened a page")
        }
        operation.removeValue(forKey: "method")
        #expect(!OnboardingExecutor.routesToTheTrustedSigner(operation),
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
        #expect(routed["method"] as? String == "trusted_signer")
        #expect(routed["signer_origin"] as? String == "https://sign.example")
        #expect(routed["credential_id"] as? String == "cred-1")
        #expect(routed["group_public_key_hex"] as? String == group)
        // The core reads it as the ceremony it is, from this very JSON.
        let request = trustedSignerCeremonyRequest(
            operationJson: TrustedSignerCeremonyFixture.json(routed),
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
