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

    /// The relying party. A passkey is bound to it: change it and every existing
    /// wallet becomes unreachable from this app. It must match the
    /// `webcredentials:` entitlement and the apple-app-site-association file.
    static let relyingParty = "getvela.app"

    let relyingPartyId: String

    private var continuation: CheckedContinuation<ASAuthorization, Error>?
    private var controller: ASAuthorizationController?

    /// The app-owned CTAP2-over-CCID path (`vela_core::ctap`, the same protocol
    /// the desktop and Android run), for a USB-C security key reached with NO
    /// Apple service and NO domain association.
    ///
    /// Preferred over the system `ASAuthorizationSecurityKeyProvider` for the
    /// security-key method when a card is present: it is the escape hatch a
    /// lapsed or merely-down relying-party association cannot padlock
    /// (FR-009c), and it uses the KEY's own PIN/fingerprint, never the phone's.
    /// `nil` on surfaces with no UI to prompt through (previews, the gallery).
    var smartCard: SmartCardCtapCeremony?

    /// The caBLE "sign in with your phone" path (spec 019), for the scan
    /// method — OUR initiator, not the system sheet's: it shows a QR, the
    /// phone that holds the passkey scans it, and the ceremony runs over the
    /// channel that phone opens — a direct L2CAP CoC when its advert offers a
    /// PSM (CTAP 2.3, no internet), the WebSocket tunnel otherwise. This is
    /// what reaches an Android/securitykeys authenticator, and what still
    /// works when Apple's own cross-device flow cannot (no internet). `nil`
    /// on surfaces with no UI (previews, the gallery).
    var hybrid: HybridCeremony?

    init(relyingPartyId: String = PasskeyExecutor.relyingParty) {
        self.relyingPartyId = relyingPartyId
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
        // The app-owned CCID path IS the security-key route, ALWAYS — never the
        // system's ASAuthorization security-key provider. The person chose "USB
        // security key", and our implementation is the whole point of that
        // choice: the key's own PIN/fingerprint, no Apple service, no domain
        // association. When no card is plugged in it prompts to plug one in,
        // rather than handing off to a system sheet that consults the RP's
        // association (founder direction 2026-08-27).
        if method == .securityKey, let smartCard {
            return try await smartCard.register(name: name, excludeCredentialIds: excludeCredentialIds)
        }
        // The scan method mints the key on the OTHER phone over OUR caBLE —
        // BLE-only capable, unlike the QR inside the system sheet.
        if method == .hybrid, let hybrid {
            return try await hybrid.register(name: name, excludeCredentialIds: excludeCredentialIds)
        }

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
        // ever presented, takes the app-owned CCID path, never the system's
        // security-key provider (founder direction 2026-08-27); when no card is
        // present it prompts to plug one in.
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
        method: KeyMethod = .platform
    ) async throws -> Assertion {
        switch Self.assertionPath(transports: transports, method: method) {
        case .hybrid:
            if let hybrid {
                return try await hybrid.assert(challenge: challenge, credentialIdHex: credentialIdHex)
            }
        case .securityKey:
            if let smartCard {
                return try await smartCard.assert(challenge: challenge, credentialIdHex: credentialIdHex)
            }
        case .system:
            break
        }
        let requests = try systemRequests(
            challenge: challenge, credentialIdHex: credentialIdHex, transports: transports
        )
        let authorization = try await perform(requests)

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
    func systemRequests(
        challenge: Data, credentialIdHex: String?, transports: String
    ) throws -> [ASAuthorizationRequest] {
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
        var requests: [ASAuthorizationRequest] = [request]
        guard Self.offersSecurityKey(transports) else { return requests }
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
                    transports: Self.securityKeyTransports(transports)
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

    private func perform(_ requests: [ASAuthorizationRequest]) async throws -> ASAuthorization {
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

    /// The platform's error, in the core's vocabulary.
    static func classify(_ error: Error) -> PasskeyFailure {
        if let failure = error as? PasskeyFailure { return failure }
        guard let authError = error as? ASAuthorizationError else {
            return PasskeyFailure(kind: .other, message: error.localizedDescription)
        }
        switch authError.code {
        case .canceled:
            return PasskeyFailure(kind: .cancelled, message: "User cancelled the operation")
        case .notHandled, .notInteractive:
            // The system declined to show anything. On a device with no
            // credential and no way to make one, that is "this device will not
            // do it", which is what `not_supported` is for.
            return PasskeyFailure(kind: .notSupported, message: authError.localizedDescription)
        case .failed, .invalidResponse, .unknown:
            return PasskeyFailure(kind: .other, message: authError.localizedDescription)
        @unknown default:
            return PasskeyFailure(kind: .other, message: authError.localizedDescription)
        }
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
