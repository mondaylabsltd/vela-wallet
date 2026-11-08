//
//  PasskeyExecutor.swift
//  VelaWallet
//
//  Every WebAuthn ceremony this app performs, and nothing else.
//
//  iOS reaches WebAuthn through AuthenticationServices rather than through a
//  JSON request document, so unlike the web and Android paths this file BUILDS
//  the ceremony from typed requests. What must not differ is the result: the
//  same ES256-only credential, the same discoverability, the same exclude list.
//  A difference here produces a credential the other three clients cannot use,
//  and a wallet IS its key set.
//
//  Nothing here decides what a failure means. It classifies the platform's error
//  into the vocabulary the core branches on (`FailureKind`) and stops.
//

import AuthenticationServices
import Foundation
import VelaCore

/// The core's `FailureKind` vocabulary.
enum FailureKind: String {
    case cancelled
    case notSupported = "not_supported"
    case notDiscoverable = "not_discoverable"
    case other
}

/// A ceremony that produced no credential, already classified.
///
/// Classification is the ONE judgement call a shell makes, and it is
/// deliberately narrow: everything unrecognised becomes `other` carrying the
/// platform's own words, which the core forwards verbatim into the bug report.
struct PasskeyFailure: Error {
    let kind: FailureKind
    let message: String
    /// Real-device S3: the platform error's facts (its domain and code),
    /// from which the core names the sentence a sheet draws
    /// (`passkey_error_words`) — never the system's English in a translated
    /// sheet. `nil` for a failure this app wrote itself.
    var facts: PasskeyErrorFacts? = nil
}

/// A platform passkey error's facts, as the core reads them
/// (`passkey_error::PasskeyErrorFacts`).
struct PasskeyErrorFacts: Equatable {
    var domain: String
    var code: Int?
    var message: String

    static let platform = "ios"

    /// The facts of any error: an `NSError`'s domain and code.
    static func of(_ error: Error) -> PasskeyErrorFacts {
        if let failure = error as? PasskeyFailure {
            return failure.facts ?? PasskeyErrorFacts(domain: "", code: nil, message: failure.message)
        }
        let ns = error as NSError
        return PasskeyErrorFacts(domain: ns.domain, code: ns.code, message: ns.localizedDescription)
    }

    /// The wire object `passkey_failed.error` carries.
    var wire: [String: Any] {
        ["platform": Self.platform, "domain": domain, "code": code.map { $0 as Any } ?? NSNull(), "message": message]
    }

    /// The core's words for these facts: the kind, the corpus key a sheet
    /// draws, and the failure the machines branch on.
    var words: PasskeyErrorWords {
        passkeyErrorWords(platform: Self.platform, domain: domain, code: code.map { Int32(clamping: $0) }, message: message)
    }
}

/// §35 (review 0-1): the relying party this build's platform passkeys live
/// under — the core's rule (`relying_party_id`), never a shell's. A release
/// build is `getvela.app` (and never reads the test domain); a test build —
/// Debug, which the "Vela Test" device-test build is too — on the simulator
/// is `localhost`; a test build on a device is the build's `TEST_RP_DOMAIN`
/// (the build setting, through the `VelaTestRpDomain` Info.plist key), and
/// without one it is the core's error, said as the ceremony's failure —
/// never a quiet fall-back to `getvela.app`, whose association file lists
/// the store app only.
nonisolated enum PasskeyRelyingParty {
    /// Why this build has no relying party: the core's sentence.
    struct Unavailable: Error, Equatable, Sendable {
        let message: String
    }

    /// The Info.plist key the `TEST_RP_DOMAIN` build setting reaches the
    /// app through (empty when the setting is).
    static let infoKey = "VelaTestRpDomain"

    /// A test build, by the core's notion (`is_test_build`): every Debug
    /// build — the simulator's, the parallel space's and the device-test
    /// build alike. Release is not.
    static var testBuild: Bool {
        #if DEBUG
        true
        #else
        false
        #endif
    }

    /// Running on the simulator, whose platform authenticator answers no
    /// association: it signs with the parallel space's fixture keys.
    static var simulator: Bool {
        #if targetEnvironment(simulator)
        true
        #else
        false
        #endif
    }

    /// The build's `TEST_RP_DOMAIN`, `nil` when it is empty. A release build
    /// never reads it — the core would not either.
    static func configuredDomain(_ info: [String: Any]?) -> String? {
        guard testBuild else { return nil }
        guard let domain = info?[infoKey] as? String, !domain.isEmpty else { return nil }
        return domain
    }

    /// The core's answer for these facts.
    static func resolve(testBuild: Bool, simulator: Bool, testDomain: String?) -> Result<String, Unavailable> {
        do {
            return .success(try relyingPartyId(testBuild: testBuild, simulator: simulator, testDomain: testDomain))
        } catch {
            return .failure(Unavailable(message: coreMessage(error)))
        }
    }

    /// This build's: read once, at first use.
    static let thisBuild: Result<String, Unavailable> = resolve(
        testBuild: testBuild, simulator: simulator,
        testDomain: configuredDomain(Bundle.main.infoDictionary)
    )

    /// The core's sentence out of its error — `CoreError::Internal`'s text
    /// as it is, never a Swift rendering of the enum.
    private static func coreMessage(_ error: Error) -> String {
        if case CoreError.Internal(let message) = error { return message }
        return "\(error)"
    }
}

/// A completed registration, in the core's hex vocabulary.
struct Registration {
    let credentialIdHex: String
    let attestationObjectHex: String
    let clientDataJsonHex: String
    let authenticatorAttachment: String
    let transports: String
}

/// A completed assertion.
struct Assertion {
    let credentialIdHex: String
    let signatureDerHex: String
    let authenticatorDataHex: String
    let clientDataJsonHex: String
    let userIdHex: String?
    let authenticatorAttachment: String
}

@MainActor
final class PasskeyExecutor: NSObject {

    /// This build's relying party, by name (§35, `PasskeyRelyingParty`): a
    /// passkey is bound to it — change it and every existing wallet becomes
    /// unreachable from this app — and the release build's must match the
    /// `webcredentials:` entitlement and getvela.app's
    /// apple-app-site-association file. Empty only for a test build on a
    /// device with no `TEST_RP_DOMAIN`, whose every ceremony fails with the
    /// core's sentence before a request is made.
    nonisolated static var relyingParty: String { (try? PasskeyRelyingParty.thisBuild.get()) ?? "" }

    /// The core's answer for this executor: the relying party, or why there
    /// is none.
    let relyingPartyAnswer: Result<String, PasskeyRelyingParty.Unavailable>

    /// The relying party a request is made for — empty when there is none
    /// (no request is then made: `platformRelyingParty()` throws first).
    var relyingPartyId: String { (try? relyingPartyAnswer.get()) ?? "" }

    /// The relying party for a system-sheet request, or — a test build on a
    /// device with no test domain — the ceremony's failure in the core's
    /// words. The app-owned CTAP paths (USB CCID, caBLE) name their own
    /// party in the core and are not gated by an association file.
    private func platformRelyingParty() throws -> String {
        switch relyingPartyAnswer {
        case .success(let rpId): return rpId
        case .failure(let unavailable):
            throw PasskeyFailure(kind: .notSupported, message: unavailable.message)
        }
    }

    private var continuation: CheckedContinuation<ASAuthorization, Error>?
    private var controller: ASAuthorizationController?

    /// The app-owned CTAP2-over-CCID path (`vela_core::ctap`, the same protocol
    /// the desktop and Android run), for a USB-C security key reached with NO
    /// Apple service and NO domain association.
    ///
    /// FIRST for the security-key method: it is the escape hatch a lapsed or
    /// merely-down relying-party association cannot padlock (FR-009c), and it
    /// uses the KEY's own PIN/fingerprint, never the phone's. When it cannot
    /// reach the key — a Lightning iPhone, an NFC key, firmware below 5.8, or
    /// the person asking for it on "Insert your security key" — it says so
    /// (`SystemSheetFallback`) and the same request goes to the system's
    /// security-key provider. `nil` on surfaces with no UI to prompt through
    /// (previews, the gallery).
    var smartCard: SmartCardCtapCeremony?

    /// The person picked a key method afresh (a row on the keys screen, a
    /// row on the sign-in sheet): a choice of Apple's sheet made on an
    /// earlier insert prompt does not carry over to it.
    func methodPickedAfresh() {
        smartCard?.forgetSystemSheetChoice()
    }

    /// The caBLE "sign in with your phone" path (spec 019), for the scan
    /// method — OUR initiator, not the system sheet's: it shows a QR, the
    /// phone that holds the passkey scans it, and the ceremony runs over the
    /// channel that phone opens — a direct L2CAP CoC when its advert offers a
    /// PSM (CTAP 2.3, no internet), the WebSocket tunnel otherwise. This is
    /// what reaches an Android/securitykeys authenticator, and what still
    /// works when Apple's own cross-device flow cannot (no internet). `nil`
    /// on surfaces with no UI (previews, the gallery).
    var hybrid: HybridCeremony?

    init(relyingParty: Result<String, PasskeyRelyingParty.Unavailable> = PasskeyRelyingParty.thisBuild) {
        self.relyingPartyAnswer = relyingParty
        super.init()
    }

    /// Whether a passkey ceremony can be attempted at all.
    ///
    /// The question is whether the SERVICE exists, not whether a credential does
    /// or whether a biometric is enrolled. Answering `false` for "no passkeys
    /// saved yet" would make the core raise "this device cannot create a wallet"
    /// on a device that can — and leave no way back, because a person cannot
    /// enrol a credential from inside a flow that refuses to start.
    ///
    /// The deployment target is 17.4 (research D6), so the provider is always
    /// present; the check exists so the operation is performed rather than
    /// assumed.
    func supported() -> Bool {
        ASAuthorizationPlatformPublicKeyCredentialProvider(relyingPartyIdentifier: relyingPartyId)
            .createCredentialRegistrationRequest(
                challenge: Data(count: 32),
                name: "probe",
                userID: Data(count: 1)
            ) != nil
    }

    /// Registration.
    ///
    /// `method` NARROWS the sheet; it does not define it. Asking for a security
    /// key issues only the security-key request, because the person said which
    /// object they were holding. Every other method offers the platform request
    /// AND the security-key request together, so the system sheet lists Face ID,
    /// another device, and a security key, and the person picks.
    ///
    /// That last part is a fix, not a flourish. The FIRST founding key is minted
    /// from the name screen, before a key list exists, so it carries the core's
    /// default method — platform — and iOS was therefore offering exactly two
    /// ways in: this device, or a QR to another phone. Somebody holding a
    /// YubiKey could not start a wallet with it at all, on a wallet whose whole
    /// design is that a key set can mix (founder-found 2026-08-26). Android had
    /// no such gap: its default request carries no attachment constraint, so
    /// Credential Manager listed security keys from the start.
    ///
    /// `Hybrid` never arrives as a method — the key screen offers it as
    /// present-and-unavailable rather than issuing a ceremony no transport can
    /// run, because on iOS the QR flow is the SYSTEM's to offer inside the
    /// platform sheet, not ours to request.
    func register(
        name: String,
        excludeCredentialIds: [String],
        method: KeyMethod
    ) async throws -> Registration {
        // The app-owned CCID path is the security-key route FIRST. The person
        // chose "USB security key", and our implementation is the point of
        // that choice: the key's own PIN/fingerprint, no Apple service, no
        // domain association (founder direction 2026-08-27). When no card is
        // plugged in it prompts to plug one in.
        //
        // It hands over to the system's security-key provider only where it
        // cannot work: a Lightning iPhone (that sheet waited for ever), a key
        // that does not answer FIDO over CCID, or the person's own "Use
        // Apple's security-key sheet". The request below is then the
        // security-key one alone — same ES256, resident key, verification.
        if method == .securityKey, let smartCard {
            do {
                return try await smartCard.register(name: name, excludeCredentialIds: excludeCredentialIds)
            } catch is SmartCardCtapCeremony.SystemSheetFallback {
                // Fall through to the system sheet.
            }
        }
        // The scan method mints the key on the OTHER phone over OUR caBLE —
        // BLE-only capable, unlike the QR inside the system sheet.
        if method == .hybrid, let hybrid {
            return try await hybrid.register(name: name, excludeCredentialIds: excludeCredentialIds)
        }

        let relyingPartyId = try platformRelyingParty()
        let challenge = Self.random(32)
        let userId = Data(Self.encodeUserHandle(name).utf8)
        let excluded = try excludeCredentialIds.map { hex in
            ASAuthorizationPlatformPublicKeyCredentialDescriptor(credentialID: try fromHex(s: hex))
        }

        var requests: [ASAuthorizationRequest] = []

        if method != .securityKey {
            let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
                relyingPartyIdentifier: relyingPartyId
            )
            let platformRequest = provider.createCredentialRegistrationRequest(
                challenge: challenge,
                name: name,
                userID: userId
            )
            // The reason this feature raised the deployment target to 17.4
            // (research D6): a multi-key wallet registers each founding key
            // separately, and the provider must refuse to silently REPLACE an
            // earlier one — the Safe address depends on every key in the set.
            platformRequest.excludedCredentials = excluded
            requests.append(platformRequest)
        }

        do {
            let provider = ASAuthorizationSecurityKeyPublicKeyCredentialProvider(
                relyingPartyIdentifier: relyingPartyId
            )
            let securityKeyRequest = provider.createCredentialRegistrationRequest(
                challenge: challenge,
                displayName: name,
                name: name,
                userID: userId
            )
            // ES256 (P-256) ONLY, deliberately without an RS256 fallback. The
            // on-chain verifier is the RIP-7212 P-256 precompile and
            // two-signature recovery is ECDSA math, so an RSA credential can
            // never become a working wallet: it would pass creation and then die
            // during key extraction — after minting an orphan passkey on the
            // person's key.
            securityKeyRequest.credentialParameters = [
                ASAuthorizationPublicKeyCredentialParameters(algorithm: .ES256)
            ]
            // Discoverable, or the credential never appears in the picker and
            // the wallet dies with this device (issue #1).
            securityKeyRequest.residentKeyPreference = .required
            securityKeyRequest.userVerificationPreference = .required
            securityKeyRequest.attestationPreference = .direct
            securityKeyRequest.excludedCredentials = try excludeCredentialIds.map { hex in
                ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor(
                    credentialID: try fromHex(s: hex),
                    transports: ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported
                )
            }
            requests.append(securityKeyRequest)
        }

        let authorization = try await perform(requests)

        guard let credential = authorization.credential
            as? ASAuthorizationPublicKeyCredentialRegistration
        else {
            throw PasskeyFailure(kind: .other, message: "No credential returned")
        }
        guard let attestation = credential.rawAttestationObject else {
            throw PasskeyFailure(kind: .other, message: "The authenticator returned no attestation")
        }

        // What the person actually used, not what was asked for. With both
        // requests on the sheet the two can differ — that is the point of
        // offering both — and a row labelled from the REQUEST would call a
        // YubiKey "this device".
        let usedSecurityKey =
            credential is ASAuthorizationSecurityKeyPublicKeyCredentialRegistration
        return Registration(
            credentialIdHex: toHex(data: credential.credentialID, prefixed: false),
            attestationObjectHex: toHex(data: attestation, prefixed: false),
            clientDataJsonHex: toHex(data: credential.rawClientDataJSON, prefixed: false),
            authenticatorAttachment: usedSecurityKey ? "cross-platform" : "platform",
            transports: usedSecurityKey ? "usb,nfc" : "internal,hybrid"
        )
    }

    /// Which way an assertion goes. **The METHOD decides; transports are only
    /// the allow-list's hint** — the rule Android keeps (2026-09-26), so the two
    /// phones send the same route down the same path.
    enum AssertionPath: Equatable {
        /// OUR caBLE initiator — the scan method.
        case hybrid
        /// The app-owned CCID path to a security key.
        case securityKey
        /// The system sheet: the platform request, and the security-key
        /// request beside it unless the key is known to live on a phone.
        case system(offersSecurityKey: Bool)
    }

    private static let removableTransports: Set<String> = ["usb", "nfc", "ble"]

    /// The route's transports as tokens, in the order given.
    private static func tokens(_ transports: String) -> [String] {
        transports.split(separator: ",")
            .map { $0.trimmingCharacters(in: .whitespaces) }
            .filter { !$0.isEmpty }
    }

    /// Does the credential live ONLY on something the person has to present —
    /// a USB stick, an NFC card — and nowhere a vault can be consulted?
    ///
    /// A list that also names `internal` or `hybrid` does not: a sign-in route
    /// names the method's own transports AND where the sign-in found the key,
    /// so "This device" answered by a key on the desk reads
    /// `internal,usb,nfc,ble,hybrid`. The system sheet reaches that key with
    /// every hint and offers the security key itself (NFC included); the
    /// app-owned USB path could only ask for it to be plugged in.
    static func onlyPresented(_ transports: String) -> Bool {
        let hints = Set(tokens(transports))
        return !hints.isDisjoint(with: removableTransports) && hints.isDisjoint(with: ["internal", "hybrid"])
    }

    /// Whether the system sheet offers the security-key request beside the
    /// platform one. Only a key known NOT to be removable goes without it — an
    /// unknown set still offers both, which is what a mixed founding set needs.
    private static func offersSecurityKey(_ transports: String) -> Bool {
        let hints = Set(tokens(transports))
        return hints.isEmpty || !hints.isDisjoint(with: removableTransports)
    }

    static func assertionPath(transports: String, method: KeyMethod) -> AssertionPath {
        // THE METHOD OUTRANKS THE TRANSPORT HINTS. A caBLE credential is
        // cross-platform, so its recovery proof arrives carrying the WIDE hint
        // set ("usb,nfc,ble,hybrid") — with the removable branch first, a person
        // who had just signed in by scanning a QR was told to plug in a USB
        // security key they never owned (device-found 2026-08-28). The scan
        // method reaches the credential on the OTHER phone over OUR caBLE.
        if method == .hybrid { return .hybrid }
        // A person who chose the hardware key, or a credential that is only
        // ever presented, takes the app-owned CCID path first (founder
        // direction 2026-08-27); when no card is present it prompts to plug
        // one in, and that path hands over to the system's security-key
        // provider where it cannot reach the key (`assert`).
        if method == .securityKey || onlyPresented(transports) { return .securityKey }
        return .system(offersSecurityKey: offersSecurityKey(transports))
    }

    /// An assertion. `credentialIdHex` pins it to one credential; `nil` is the
    /// "who are you?" ceremony sign-in starts with.
    /// - Parameter transports: WHERE the credential lives — the route's list,
    ///   or what its authenticator reported at registration (`hybrid,internal`,
    ///   `usb,nfc`, …), or empty when unknown. It is the allow-list's hint; the
    ///   method picks the path (`assertionPath`).
    func assert(
        challenge: Data,
        credentialIdHex: String?,
        transports: String = "",
        method: KeyMethod = .platform,
        deadlineMs: Double? = nil
    ) async throws -> Assertion {
        var securityKeyOnly = false
        switch Self.assertionPath(transports: transports, method: method) {
        case .hybrid:
            if let hybrid {
                return try await hybrid.assert(challenge: challenge, credentialIdHex: credentialIdHex)
            }
        case .securityKey:
            if let smartCard {
                do {
                    return try await smartCard.assert(challenge: challenge, credentialIdHex: credentialIdHex)
                } catch is SmartCardCtapCeremony.SystemSheetFallback {
                    // The app-owned path cannot reach this key (a Lightning
                    // iPhone, NFC, firmware below 5.8, or the person asked):
                    // Apple's security-key sheet, and only it — the person
                    // said which object they are holding.
                    securityKeyOnly = true
                }
            }
        case .system:
            break
        }
        let requests = try systemRequests(
            challenge: challenge, credentialIdHex: credentialIdHex, transports: transports,
            securityKeyOnly: securityKeyOnly
        )
        let authorization = try await perform(requests, deadlineMs: deadlineMs)

        guard let credential = authorization.credential
            as? ASAuthorizationPublicKeyCredentialAssertion
        else {
            throw PasskeyFailure(kind: .other, message: "No credential returned")
        }

        return Assertion(
            credentialIdHex: toHex(data: credential.credentialID, prefixed: false),
            // `signatureDerHex`, not `signatureHex`: the platform hands back a
            // DER signature and the core normalises it (including low-S) itself.
            // Naming it otherwise would invite a shell to "helpfully" convert.
            signatureDerHex: toHex(data: credential.signature, prefixed: false),
            authenticatorDataHex: toHex(data: credential.rawAuthenticatorData, prefixed: false),
            clientDataJsonHex: toHex(data: credential.rawClientDataJSON, prefixed: false),
            // Absent, not empty: no user handle is a different fact from an
            // empty one, and the core's name resolution branches on it.
            userIdHex: credential.userID.flatMap { $0.isEmpty ? nil : toHex(data: $0, prefixed: false) },
            authenticatorAttachment: credential is ASAuthorizationSecurityKeyPublicKeyCredentialAssertion
                ? "cross-platform"
                : "platform"
        )
    }

    /// The system sheet's requests, pinned to `credentialIdHex` when there is
    /// one: the platform request (this phone, or a nearby one over the system's
    /// own hybrid), and the security-key request ALONGSIDE it — not instead of
    /// it — naming every removable cable the route lists. A wallet whose
    /// founding set mixes a phone passkey and a hardware key has to be able to
    /// sign with whichever is at hand, and the person picks in the sheet.
    ///
    /// `securityKeyOnly` is the security-key method handed over from the
    /// app-owned path: the security-key request alone, whatever the hints say.
    func systemRequests(
        challenge: Data, credentialIdHex: String?, transports: String, securityKeyOnly: Bool = false
    ) throws -> [ASAuthorizationRequest] {
        let relyingPartyId = try platformRelyingParty()
        var requests: [ASAuthorizationRequest] = []
        if !securityKeyOnly {
            let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
                relyingPartyIdentifier: relyingPartyId
            )
            let request = provider.createCredentialAssertionRequest(challenge: challenge)
            request.userVerificationPreference = .required
            if let credentialIdHex {
                request.allowedCredentials = [
                    ASAuthorizationPlatformPublicKeyCredentialDescriptor(
                        credentialID: try fromHex(s: credentialIdHex)
                    )
                ]
            }
            requests.append(request)
            guard Self.offersSecurityKey(transports) else { return requests }
        }
        let securityKeyProvider = ASAuthorizationSecurityKeyPublicKeyCredentialProvider(
            relyingPartyIdentifier: relyingPartyId
        )
        let securityKeyRequest = securityKeyProvider.createCredentialAssertionRequest(
            challenge: challenge
        )
        securityKeyRequest.userVerificationPreference = .required
        if let credentialIdHex {
            securityKeyRequest.allowedCredentials = [
                ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor(
                    credentialID: try fromHex(s: credentialIdHex),
                    // Handed over because USB did not reach it: every cable
                    // the system knows, NFC and Lightning among them.
                    transports: securityKeyOnly
                        ? ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported
                        : Self.securityKeyTransports(transports)
                )
            ]
        }
        requests.append(securityKeyRequest)
        return requests
    }

    /// The cables a security-key entry names: the route's, in its order, or
    /// every supported one when it names none. A credential known to live on a
    /// security key should say which cable.
    private static func securityKeyTransports(
        _ transports: String
    ) -> [ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport] {
        let mapped: [ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport] =
            tokens(transports).compactMap { hint in
                switch hint {
                case "usb": return .usb
                case "nfc": return .nfc
                case "ble": return .bluetooth
                default: return nil
                }
            }
        return mapped.isEmpty
            ? ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported
            : mapped
    }

    static func random(_ count: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: count)
        // `SecRandomCopyBytes` is the system CSPRNG. A failure here is not
        // recoverable by trying again with something weaker: a wallet key
        // derived from predictable bytes is a wallet somebody else owns.
        let status = SecRandomCopyBytes(kSecRandomDefault, count, &bytes)
        precondition(status == errSecSuccess, "the system CSPRNG refused: \(status)")
        return Data(bytes)
    }

    /// `name` + NUL + `uuid` — the handle shape every Vela client mints and
    /// `Assertion::user_name` in the core parses back. A handle without the NUL
    /// separator, or without a uuid tail, is read as a credential this app did
    /// not create, and its name is discarded rather than shown.
    ///
    /// `UUID().uuidString` is UPPERCASE on Apple platforms and lowercase on the
    /// web; the core's shape check is case-insensitive for exactly this reason.
    static func encodeUserHandle(_ name: String) -> String {
        "\(name)\u{0}\(UUID().uuidString)"
    }

    // MARK: - The delegate bridge

    /// - Parameter deadlineMs: the prompt's own deadline (send-11) — the
    ///   signing prompt's is the core's (`passkey_sign_timeout_ms`). The
    ///   system sheet takes no timeout on iOS, so the prompt is cancelled
    ///   here when it runs out, and ends as a cancel AT the deadline: what
    ///   `passkey_ceremony_end` reads as timed out. `nil`: no deadline (the
    ///   sign-in and create ceremonies, as before).
    private func perform(
        _ requests: [ASAuthorizationRequest], deadlineMs: Double? = nil
    ) async throws -> ASAuthorization {
        try await withCheckedThrowingContinuation { continuation in
            // A previous request is abandoned rather than left dangling: the
            // system refuses a second concurrent ceremony, which would surface
            // as a spurious failure on a flow the person did not cancel.
            self.continuation?.resume(
                throwing: PasskeyFailure(kind: .cancelled, message: "Superseded by a newer request")
            )
            self.continuation = continuation

            let controller = ASAuthorizationController(authorizationRequests: requests)
            controller.delegate = self
            controller.presentationContextProvider = self
            self.controller = controller
            controller.performRequests()
            if let deadlineMs, deadlineMs.isFinite, deadlineMs > 0 {
                DispatchQueue.main.asyncAfter(deadline: .now() + deadlineMs / 1000) { [weak self, weak controller] in
                    MainActor.assumeIsolated {
                        // Only the prompt this deadline was set for: one that
                        // ended, or a newer one, is not touched.
                        guard let self, let controller, self.controller === controller else { return }
                        controller.cancel()
                        self.finish(.failure(
                            PasskeyFailure(kind: .cancelled, message: "The passkey prompt timed out")
                        ))
                    }
                }
            }
        }
    }

    private func finish(_ result: Result<ASAuthorization, Error>) {
        guard let continuation else { return }
        self.continuation = nil
        self.controller = nil
        continuation.resume(with: result)
    }
}

extension PasskeyExecutor: ASAuthorizationControllerDelegate {
    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        finish(.success(authorization))
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        finish(.failure(Self.classify(error)))
    }

    /// The platform's error, in the core's vocabulary: its kind is the
    /// core's reading of the error's facts (`passkey_error_words`, real-device
    /// S3 — one rule on every platform), and the facts ride along so the
    /// sheet says the core's sentence, never the system's English.
    static func classify(_ error: Error) -> PasskeyFailure {
        if let failure = error as? PasskeyFailure { return failure }
        let facts = PasskeyErrorFacts.of(error)
        let kind = FailureKind(rawValue: facts.words.failure) ?? .other
        let message = kind == .cancelled ? "User cancelled the operation" : facts.message
        return PasskeyFailure(kind: kind, message: message, facts: facts)
    }
}

extension PasskeyExecutor: ASAuthorizationControllerPresentationContextProviding {
    /// The anchor the system sheet attaches to.
    ///
    /// The KEY WINDOW of the foreground active scene, resolved at call time
    /// rather than captured at construction: a window held from `init` is the
    /// wrong one after a scene change, and the sheet then presents over nothing
    /// or not at all.
    func presentationAnchor(for session: ASAuthorizationController) -> ASPresentationAnchor {
        let scenes = UIApplication.shared.connectedScenes
        let window = scenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }?
            .windows
            .first { $0.isKeyWindow }
        return window ?? ASPresentationAnchor()
    }
}
