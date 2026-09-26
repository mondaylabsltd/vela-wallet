//
//  SignInRouteTests.swift
//  VelaWalletTests
//
//  Founder, 2026-09-26: a person says where their passkey is when they create
//  the wallet or sign in, and never again — every later signature reuses the
//  key the account signed in with, over the same route. The account record
//  names it (`signed_in_with`), the core reads it (`signInRoute`), and nothing
//  on this side offers a choice.
//
//  Everything here runs the real core, the real account store and the real
//  spine; only the passkey is a recorder, and every ceremony stops at it —
//  WHERE a ceremony was sent is the whole claim.
//

import AuthenticationServices
import CryptoKit
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// Remembers where each ceremony was sent, then declines.
@MainActor
final class RoutingSigner: UserOpSigner {
    struct Asked: Equatable {
        let credentialIdHex: String?
        let transports: String
        let method: KeyMethod
    }

    private(set) var asked: [Asked] = []

    func sign(
        challenge: Data,
        credentialIdHex: String?,
        transports: String,
        method: KeyMethod
    ) async throws -> Assertion {
        asked.append(Asked(credentialIdHex: credentialIdHex, transports: transports, method: method))
        throw PasskeyFailure(kind: .cancelled, message: "cancelled")
    }
}

@MainActor
struct SignInRouteTests {
    private let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    /// An Apple passkey approved from a phone (registered as caBLE), and a
    /// YubiKey — the two ways a key most often is somewhere else.
    private let first = "a1a1a1a1"
    private let second = "b2b2b2b2"
    private let firstKey = Self.publicKeyHex()
    private let secondKey = Self.publicKeyHex()

    // MARK: - Where a signature goes

    /// (a) The account signed in with its SECOND key, over a security key: a
    /// message and a send are both pinned to that key and reach it over USB,
    /// NFC or BLE — not the first key, and not the route either key reported
    /// at registration.
    @Test func theSecondKeySignedInOverASecurityKeySignsThereAndOnlyThere() async throws {
        let store = await Self.store(record(signedInWith: [
            "credential_id": second, "method": "security_key",
        ]))
        let signer = RoutingSigner()
        let relay = ScriptedRelayPort()
        relay.rpc["eth_getCode"] = .ok("0x")
        relay.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        let spine = spine(SendAccountPort(accounts: store), signer, relay: relay)

        await expectCancelled { try await signMessage(spine) }
        await expectCancelled {
            _ = try await spine.submit(
                chainId: 100, account: address,
                calls: [UserOpCall(to: address, value: "1000", data: "0x")],
                gasFeeToken: nil, quotedFee: UserOpSpine.Quoted(amount: "1000", recipient: address)
            )
        }

        let pinned = RoutingSigner.Asked(credentialIdHex: second, transports: "usb,nfc,ble", method: .securityKey)
        #expect(signer.asked == [pinned, pinned], "every signature reuses the sign-in key and its route")
    }

    /// (b) A record written before the sign-in key existed signs exactly as it
    /// did: the first founding key, its own stored transports, and the method
    /// those transports describe (`SendAccountPort.routing`) — the `auto` it
    /// always signed with. A stored default "Sign with" no longer counts.
    @Test func aRecordFromBeforeTheSignInKeySignsExactlyAsItDid() async throws {
        let legacy = record()
        #expect(signInRoute(accountJson: try Self.json(legacy)) == nil, "the core names no route for it")
        let store = await Self.store(legacy)
        let port = SendAccountPort(accounts: store)
        let signer = RoutingSigner()

        await expectCancelled { try await signMessage(spine(port, signer)) }

        let before = await port.routing(of: address)
        #expect(before.transports == "hybrid" && before.method == .hybrid)
        #expect(signer.asked == [.init(credentialIdHex: first, transports: before.transports, method: before.method)])
    }

    /// "This device" was chosen at sign-in and a key off the phone answered it
    /// (a cross-platform attachment): the route names the choice AND where the
    /// key was found, and the ceremony gets both, untouched — the method the
    /// person chose, the transports that reach the key.
    @Test func aPlatformSignInAnsweredOffThePhoneReachesTheKeyWhereItWasFound() async throws {
        let store = await Self.store(record(signedInWith: [
            "credential_id": second, "method": "platform", "transports": "usb,nfc,ble,hybrid",
        ]))
        let signer = RoutingSigner()

        await expectCancelled { try await signMessage(spine(SendAccountPort(accounts: store), signer)) }

        #expect(signer.asked == [
            .init(credentialIdHex: second, transports: "internal,usb,nfc,ble,hybrid", method: .platform),
        ])
    }

    /// …and that ceremony goes to the SYSTEM sheet, not the app-owned USB path:
    /// the method decides, and transports are only the allow-list's hint. The
    /// sheet is pinned to the key on both of its requests, and the security-key
    /// request names every removable cable the route lists — so the system can
    /// still offer the key over NFC, or a nearby device.
    @Test func aPlatformRouteWithEveryTransportGoesToTheSystemSheetWithThemAll() throws {
        let transports = "internal,usb,nfc,ble,hybrid"
        #expect(PasskeyExecutor.assertionPath(transports: transports, method: .platform)
                == .system(offersSecurityKey: true))

        let requests = try PasskeyExecutor().systemRequests(
            challenge: Data(repeating: 7, count: 32), credentialIdHex: second, transports: transports
        )
        #expect(requests.count == 2)
        let platform = try #require(requests.first as? ASAuthorizationPlatformPublicKeyCredentialAssertionRequest)
        #expect(platform.allowedCredentials.map(\.credentialID) == [Data([0xb2, 0xb2, 0xb2, 0xb2])])
        let securityKey = try #require(requests.last as? ASAuthorizationSecurityKeyPublicKeyCredentialAssertionRequest)
        #expect(securityKey.allowedCredentials.map(\.credentialID) == [Data([0xb2, 0xb2, 0xb2, 0xb2])])
        #expect(securityKey.allowedCredentials.first?.transports == [.usb, .nfc, .bluetooth])

        // A security-key sign-in is the app-owned path whatever else it lists.
        #expect(PasskeyExecutor.assertionPath(transports: "usb,nfc,ble,hybrid", method: .securityKey) == .securityKey)
    }

    /// A record from before the sign-in key takes the path it always took,
    /// whatever its first key reported. The old rule is written out here as the
    /// reference: the method from `usb`/`nfc`, then caBLE for `hybrid`, the
    /// app-owned USB path for any removable token or a security-key method,
    /// and otherwise the system sheet offering the security key unless the key
    /// is known to live on a phone.
    @Test func aRecordFromBeforeTheSignInKeyTakesThePathItAlwaysTook() async throws {
        func oldPath(_ transports: String) -> PasskeyExecutor.AssertionPath {
            let hints = Set(transports.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) })
            let method: KeyMethod = hints.contains("hybrid") && !hints.contains("internal") ? .hybrid
                : hints.contains("usb") || hints.contains("nfc") ? .securityKey : .platform
            if method == .hybrid { return .hybrid }
            let removable = !hints.isDisjoint(with: ["usb", "nfc", "ble"])
            if removable || method == .securityKey { return .securityKey }
            let platformOnly = !hints.isEmpty && hints.isDisjoint(with: ["usb", "nfc", "ble"])
            return .system(offersSecurityKey: !platformOnly)
        }
        let lists = [
            "", "internal", "hybrid,internal", "internal,hybrid", "hybrid", "usb,nfc", "usb", "nfc", "ble",
            "ble,nfc,usb", "internal,ble", "hybrid,internal,ble", "internal,usb", "cable,internal",
            "usb,nfc,ble,hybrid",
        ]
        for transports in lists {
            var legacy = record()
            var keys = try #require(legacy["keys"] as? [[String: Any]])
            keys[0]["transports"] = transports
            legacy["keys"] = keys
            let (routed, method) = await SendAccountPort(accounts: await Self.store(legacy)).routing(of: address)
            #expect(routed == transports)
            #expect(PasskeyExecutor.assertionPath(transports: routed, method: method) == oldPath(transports),
                    "path for \"\(transports)\"")
        }
    }

    /// A record naming a key this wallet does not hold is read as naming none:
    /// a signature from a stranger's key could not be verified anyway.
    @Test func aSignInKeyTheWalletDoesNotHoldSignsAsBefore() async throws {
        let store = await Self.store(record(signedInWith: ["credential_id": "c0ffee", "method": "platform"]))
        let signer = RoutingSigner()

        await expectCancelled { try await signMessage(spine(SendAccountPort(accounts: store), signer)) }

        #expect(signer.asked == [.init(credentialIdHex: first, transports: "hybrid", method: .hybrid)])
    }

    /// A Trusted Signer sign-in: every signature goes to the page the sign-in
    /// ran on, and no passkey sheet is ever raised. With no page named, the
    /// page from Settings (the empty origin).
    @Test func aTrustedSignerSignInSignsOnItsPageNeverOnAPasskeySheet() async throws {
        let pages: [(named: String?, opened: String)] = [("https://my.signer", "https://my.signer"), (nil, "")]
        for (named, opened) in pages {
            var key: [String: Any] = ["credential_id": second, "method": "trusted_signer"]
            if let named { key["signer_origin"] = named }
            let store = await Self.store(record(signedInWith: key))
            let signer = RoutingSigner()
            let page = ScriptedTrustedSigner { _ in .timedOut }
            let spine = spine(SendAccountPort(accounts: store), signer)
            spine.trustedSigner = page

            do {
                try await signMessage(spine)
                Issue.record("a timed-out page signed")
            } catch let refused as UserOpSpine.Refused {
                #expect(refused.failure == .trustedSigner(.timeout))
            }
            #expect(signer.asked.isEmpty, "a Trusted Signer sign-in raised a passkey sheet")
            #expect(page.asked.map { $0.page } == [opened])
        }
    }

    // MARK: - The record keeps it

    /// Every door a record is written through keeps `signed_in_with`: the
    /// onboarding machines' `save_account`, the session's write-back, and a
    /// save of another account beside it.
    @Test func theSignInKeySurvivesEveryWriteOfTheRecord() async throws {
        let store = AccountStore(defaults: UserDefaults(suiteName: "vela.tests.signin.\(UUID().uuidString)")!)
        let deps = CompletionRecorder()
        let onboarding = OnboardingExecutor(
            passkey: PasskeyExecutor(), registry: RegistryClient(baseURL: "https://r.test"),
            store: store, deps: deps
        )
        let session = SessionExecutor(store: store)
        let signedIn = record(signedInWith: ["credential_id": second, "method": "security_key"])

        _ = await onboarding.perform(["type": "save_account", "account": signedIn])
        try await expectSignedInWithSecond(store)

        _ = await session.perform(["type": "save_account", "account": signedIn])
        try await expectSignedInWithSecond(store)

        var stranger = record()
        stranger["id"] = "other"
        stranger["address"] = "0x2222222222222222222222222222222222222222"
        await store.saveAccount(stranger)
        try await expectSignedInWithSecond(store)
        #expect(await store.loadAccounts().count == 2)
    }

    /// The whole journey through the REAL login machine: a record written
    /// before this change is held here, the person signs in with its second
    /// key over a security key, and the machine re-saves the held record to
    /// name it — whole, both founding keys still there — and the next
    /// signature goes to that key over that route, and to wherever the
    /// sign-in found it (a cross-platform key: USB, NFC, BLE or a phone).
    @Test func signingInWithTheSecondKeyReSavesTheHeldRecordAndSignsWithIt() async throws {
        let store = await Self.store(record())
        let (answered, mode) = try await signIn(store, method: "security_key", assertion: assertion(credentialId: second))
        #expect(answered == [
            "check_passkey_support", "authenticate_passkey", "load_accounts", "save_account", "complete_onboarding",
        ])

        let saved = try #require(await store.loadAccounts().first)
        #expect(await store.loadAccounts().count == 1, "the held record was re-saved, not added")
        #expect(saved["name"] as? String == "Mine" && saved["address"] as? String == address)
        let keys = try #require(saved["keys"] as? [[String: Any]])
        #expect(keys.compactMap { $0["public_key_hex"] as? String } == [firstKey, secondKey],
                "the re-save dropped a founding key — a different Safe")
        try await expectSignedInWithSecond(store)
        #expect((saved["signed_in_with"] as? [String: Any])?["transports"] as? String == "usb,nfc,ble,hybrid",
                "the record forgot where the sign-in found the key")
        #expect(mode?["type"] as? String == "set_wallet")
        let handed = try #require((mode?["accounts"] as? [[String: Any]])?.first)
        #expect((handed["signed_in_with"] as? [String: Any])?["credential_id"] as? String == second,
                "the session was handed the record without its sign-in key")

        let signer = RoutingSigner()
        await expectCancelled { try await signMessage(spine(SendAccountPort(accounts: store), signer)) }
        #expect(signer.asked == [
            .init(credentialIdHex: second, transports: "usb,nfc,ble,hybrid", method: .securityKey),
        ])
    }

    // MARK: - The Trusted Signer

    /// An account signed in through the Trusted Signer signs there with its
    /// sign-in key ALONE: the page is offered that one key, and an answer by
    /// another of the wallet's own keys is refused like any mismatch — nothing
    /// signed, the request left open. The sign-in key's answer is taken.
    @Test func aTrustedSignerSignInOffersThePageItsKeyAloneAndTakesNoOtherAnswer() async throws {
        let founding = TrustedSignerFixture(credential: Data([0xa1, 0xa1, 0xa1, 0xa1]))
        let signedIn = TrustedSignerFixture(credential: Data([0xb2, 0xb2, 0xb2, 0xb2]))
        let accounts = ScriptedAccounts()
        accounts.keyList = founding.keys + signedIn.keys
        accounts.recordJson = try Self.json(Self.record(founding, signedIn, signedInWith: [
            "credential_id": signedIn.credentialHex, "method": "trusted_signer",
            "signer_origin": "https://my.signer",
        ]))
        let passkey = CountingSigner()
        let page = OfferedKeysPage(answering: founding)
        let spine = UserOpSpine(
            relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { passkey }
        )
        spine.trustedSigner = page

        do {
            try await signMessage(spine, account: founding.account)
            Issue.record("another founding key's answer was taken")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .trustedSigner(.mismatch))
        }
        #expect(page.offered == [[TrustedSignerFixture.base64url(signedIn.credential)]],
                "the page was offered more than the sign-in key")
        #expect(page.pages == ["https://my.signer"])

        page.answering = signedIn
        try await signMessage(spine, account: founding.account)
        #expect(page.offered.count == 2)
        #expect(passkey.calls == 0, "a Trusted Signer sign-in raised a passkey sheet")
    }

    /// A record from before the sign-in key, whose key lives behind a page,
    /// still offers the page EVERY founding key and takes any of their answers.
    @Test func aRecordFromBeforeTheSignInKeyStillOffersThePageEveryKey() async throws {
        let founding = TrustedSignerFixture(credential: Data([0xa1, 0xa1, 0xa1, 0xa1]))
        let other = TrustedSignerFixture(credential: Data([0xb2, 0xb2, 0xb2, 0xb2]))
        let accounts = ScriptedAccounts()
        accounts.keyList = founding.keys + other.keys
        accounts.routesJson = try Self.json([
            ["credential_id": founding.credentialHex, "transports": "internal", "signer_origin": "https://sign.example"],
            ["credential_id": other.credentialHex, "transports": "internal"],
        ])
        let page = OfferedKeysPage(answering: other)
        let spine = UserOpSpine(
            relay: RelayClient(port: ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { CountingSigner() }
        )
        spine.trustedSigner = page

        try await signMessage(spine, account: founding.account)

        #expect(page.offered == [[founding.credential, other.credential].map(TrustedSignerFixture.base64url)])
        #expect(page.pages == ["https://sign.example"])
    }

    /// A sign-in THROUGH the Trusted Signer, through the real login machine:
    /// the record names the page the sign-in ran on and the method chosen, and
    /// the next signature goes to that page with the sign-in key alone.
    @Test func aSignInOnTheTrustedSignerKeepsItsPageAndSignsThere() async throws {
        let store = await Self.store(record())
        var answer = assertion(credentialId: second)
        answer["authenticator_attachment"] = "platform"
        answer["signer_origin"] = "https://my.signer"
        let (answered, _) = try await signIn(store, method: "trusted_signer", assertion: answer)
        #expect(answered.contains("save_account"), "the held record was not re-saved: \(answered)")

        let saved = try #require(await store.loadAccounts().first)
        let key = try #require(saved["signed_in_with"] as? [String: Any], "the record lost its sign-in key")
        #expect(key["credential_id"] as? String == second)
        #expect(key["method"] as? String == "trusted_signer")
        #expect(key["signer_origin"] as? String == "https://my.signer")

        let passkey = RoutingSigner()
        let page = ScriptedTrustedSigner { _ in .timedOut }
        let spine = spine(SendAccountPort(accounts: store), passkey)
        spine.trustedSigner = page
        do {
            try await signMessage(spine)
            Issue.record("a timed-out page signed")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .trustedSigner(.timeout))
        }
        #expect(passkey.asked.isEmpty)
        let asked = try #require(page.asked.first)
        #expect(asked.page == "https://my.signer")
        #expect((asked.request["context"] as? [String: Any])?["allowCredentials"] as? [String]
                == [TrustedSignerFixture.base64url(Data([0xb2, 0xb2, 0xb2, 0xb2]))])
    }

    // MARK: - Plumbing

    /// The record as the core writes it (`app::Account`, snake_case).
    private func record(signedInWith: [String: Any]? = nil) -> [String: Any] {
        var record: [String: Any] = [
            "id": first, "name": "Mine", "address": address,
            "public_key_hex": firstKey, "created_at_iso": "2026-09-01T00:00:00.000Z",
            "keys": [
                ["credential_id": first, "public_key_hex": firstKey, "name": "Mine", "transports": "hybrid"],
                ["credential_id": second, "public_key_hex": secondKey, "name": "YubiKey", "transports": "usb,nfc"],
            ],
        ]
        if let signedInWith { record["signed_in_with"] = signedInWith }
        return record
    }

    private func expectSignedInWithSecond(_ store: AccountStore) async throws {
        let json = try #require(await SendAccountPort(accounts: store).accountJson(of: address))
        let route = try #require(signInRoute(accountJson: json), "the record lost its sign-in key: \(json)")
        let decoded = try CoreJSON.decoder.decode(SignRouteWire.self, from: Data(route.utf8))
        #expect(decoded.credentialId == second && decoded.method == "security_key")
    }

    private func spine(
        _ accounts: UserOpSpine.AccountPort, _ signer: RoutingSigner, relay: ScriptedRelayPort? = nil
    ) -> UserOpSpine {
        UserOpSpine(
            relay: RelayClient(port: relay ?? ScriptedRelayPort(), now: { 0 }, retryDelayMs: 0),
            accounts: accounts, signer: { signer }
        )
    }

    private func signMessage(_ spine: UserOpSpine, account: String? = nil) async throws {
        _ = try await spine.signMessage(
            chainId: 100, account: account ?? address, originalHash: Data(repeating: 7, count: 32),
            asked: .init(method: "personal_sign", paramsJson: #"["0x00"]"#, origin: "https://a.example")
        )
    }

    /// Signs in through the REAL login machine with the person's `method`,
    /// answering the ceremony with `assertion` and everything else through the
    /// real onboarding executor over `store`. Answers the operations asked for,
    /// in order, and what the rest of the app was handed.
    private func signIn(
        _ store: AccountStore, method: String, assertion: [String: Any]
    ) async throws -> (answered: [String], mode: [String: Any]?) {
        let deps = CompletionRecorder()
        let executor = OnboardingExecutor(
            passkey: PasskeyExecutor(), registry: RegistryClient(baseURL: "https://r.test"),
            store: store, deps: deps
        )
        let login = LoginCore()
        var pending = try Self.effects(login.dispatch(eventJson: CoreJSON.string([
            "type": "sign_in", "method": method,
        ])))
        var answered: [String] = []
        while !pending.isEmpty {
            let (id, operation) = pending.removeFirst()
            let type = operation["type"] as? String ?? ""
            answered.append(type)
            let answer: String
            switch type {
            // The two answers only a person and their authenticator can give.
            case "check_passkey_support":
                answer = CoreJSON.string(["type": "passkey_support", "supported": true])
            case "authenticate_passkey":
                #expect(operation["method"] as? String == method)
                answer = CoreJSON.string([
                    "type": "passkey_authenticated", "assertion": assertion,
                    "now_iso": "2026-09-26T08:00:00.000Z",
                ])
            default:
                answer = await executor.perform(operation)
            }
            pending += try Self.effects(login.resolveEffect(effectId: id, resultJson: answer))
            guard answered.count < 20 else {
                Issue.record("the sign-in never finished: \(answered)")
                break
            }
        }
        return (answered, deps.mode)
    }

    /// A two-key record for the Trusted Signer's fixtures, as the core writes it.
    private static func record(
        _ first: TrustedSignerFixture, _ second: TrustedSignerFixture, signedInWith: [String: Any]
    ) -> [String: Any] {
        let key = { (fixture: TrustedSignerFixture) -> [String: Any] in
            ["credential_id": fixture.credentialHex, "public_key_hex": fixture.keys[0].publicKeyHex,
             "name": "Mine", "transports": "internal"]
        }
        return [
            "id": first.credentialHex, "name": "Mine", "address": first.account,
            "public_key_hex": first.keys[0].publicKeyHex, "created_at_iso": "2026-09-01T00:00:00.000Z",
            "keys": [key(first), key(second)], "signed_in_with": signedInWith,
        ]
    }

    private func expectCancelled(_ body: () async throws -> Void) async {
        do {
            try await body()
            Issue.record("a declined ceremony signed")
        } catch let refused as UserOpSpine.Refused {
            #expect(refused.failure == .passkeyCancelled)
        } catch {
            Issue.record("refused before the ceremony: \(error)")
        }
    }

    /// A user-verified `webauthn.get` from `credentialId` — the shape the login
    /// machine checks before it looks the credential up.
    private func assertion(credentialId: String) -> [String: Any] {
        var authenticatorData = Data(SHA256.hash(data: Data("getvela.app".utf8)))
        authenticatorData.append(contentsOf: [0x05, 0, 0, 0, 1])
        let clientData = Data(#"{"type":"webauthn.get","challenge":"AAAA","origin":"https://getvela.app"}"#.utf8)
        return [
            "credential_id": credentialId, "signature_der_hex": "3006020101020101",
            "authenticator_data_hex": TrustedSignerFixture.hex(authenticatorData),
            "client_data_json_hex": TrustedSignerFixture.hex(clientData),
            "user_id_hex": NSNull(), "authenticator_attachment": "cross-platform",
        ]
    }

    private static func store(_ record: [String: Any]) async -> AccountStore {
        let store = AccountStore(defaults: UserDefaults(suiteName: "vela.tests.signin.\(UUID().uuidString)")!)
        await store.saveAccount(record)
        return store
    }

    private static func effects(_ json: String) throws -> [(UInt64, [String: Any])] {
        (try CoreJSON.object(json)["effects"] as? [[String: Any]] ?? []).compactMap { effect in
            guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                  let operation = effect["operation"] as? [String: Any]
            else { return nil }
            return (id, operation)
        }
    }

    private static func json(_ object: Any) throws -> String {
        String(decoding: try JSONSerialization.data(withJSONObject: object), as: UTF8.self)
    }

    nonisolated private static func publicKeyHex() -> String {
        TrustedSignerFixture.hex(P256.Signing.PrivateKey().publicKey.x963Representation)
    }
}

/// What the onboarding executor hands the rest of the app when a flow ends.
@MainActor
private final class CompletionRecorder: OnboardingExecutorDeps {
    private(set) var mode: [String: Any]?
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool { false }
    func complete(mode: [String: Any]) async { self.mode = mode }
}

/// A Trusted Signer page that answers as `answering`, verified — as the real
/// channel verifies — against the keys it was offered.
final class OfferedKeysPage: TrustedSignerPort {
    var answering: TrustedSignerFixture
    /// Each request's `allowCredentials`, in order.
    private(set) var offered: [[String]] = []
    private(set) var pages: [String?] = []

    init(answering: TrustedSignerFixture) {
        self.answering = answering
    }

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> TrustedSignerChannel.Ending {
        let request = (try? JSONSerialization.jsonObject(with: Data(requestJson.utf8))) as? [String: Any] ?? [:]
        offered.append((request["context"] as? [String: Any])?["allowCredentials"] as? [String] ?? [])
        pages.append(signerOrigin)
        let result = (try? JSONSerialization.data(withJSONObject: answering.result(for: digest))) ?? Data()
        return .outcome(trustedSignerVerify(
            resultJson: String(decoding: result, as: UTF8.self), digest: digest, keys: keys
        ))
    }
}
