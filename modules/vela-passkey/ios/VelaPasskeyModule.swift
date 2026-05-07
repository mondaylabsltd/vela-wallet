import Foundation
import AuthenticationServices
import React

@objc(VelaPasskey)
class VelaPasskeyModule: NSObject {

    static let relyingParty = "getvela.app"

    /// Retained so ARC doesn't release it before the delegate callback fires.
    private var activeHandler: PasskeyRequestHandler?

    @objc static func requiresMainQueueSetup() -> Bool { true }

    // MARK: - isSupported

    @objc func isSupported(
        _ resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        if #available(iOS 16.0, *) {
            resolve(true)
        } else {
            resolve(false)
        }
    }

    // MARK: - register

    @objc func register(
        _ userName: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard #available(iOS 16.0, *) else {
            reject("PASSKEY_NOT_SUPPORTED", "Passkeys require iOS 16+", nil)
            return
        }

        let uid = Self.encodeUserID(name: userName)
        let challenge = Self.generateChallenge()

        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: Self.relyingParty
        )
        let request = provider.createCredentialRegistrationRequest(
            challenge: challenge,
            name: userName,
            userID: uid
        )

        performPasskeyRequest(request) { [weak self] result in
            self?.activeHandler = nil
            switch result {
            case .success(let authorization):
                guard let reg = authorization.credential
                        as? ASAuthorizationPlatformPublicKeyCredentialRegistration else {
                    reject("PASSKEY_NO_CREDENTIAL", "No registration credential returned", nil)
                    return
                }
                let dict: [String: Any] = [
                    "credentialId": reg.credentialID.hexString,
                    "attestationObjectHex": (reg.rawAttestationObject ?? Data()).hexString,
                    "clientDataJSONHex": reg.rawClientDataJSON.hexString,
                ]
                resolve(dict)

            case .failure(let error):
                Self.rejectWithPasskeyError(error, reject: reject)
            }
        }
    }

    // MARK: - authenticate

    @objc func authenticate(
        _ resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard #available(iOS 16.0, *) else {
            reject("PASSKEY_NOT_SUPPORTED", "Passkeys require iOS 16+", nil)
            return
        }

        let challenge = Self.generateChallenge()
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: Self.relyingParty
        )
        let request = provider.createCredentialAssertionRequest(challenge: challenge)

        performPasskeyRequest(request) { [weak self] result in
            self?.activeHandler = nil
            switch result {
            case .success(let authorization):
                guard let assertion = authorization.credential
                        as? ASAuthorizationPlatformPublicKeyCredentialAssertion else {
                    reject("PASSKEY_NO_CREDENTIAL", "No assertion credential returned", nil)
                    return
                }
                var dict: [String: Any] = [
                    "credentialId": assertion.credentialID.hexString,
                    "signatureHex": assertion.signature.hexString,
                    "authenticatorDataHex": assertion.rawAuthenticatorData.hexString,
                    "clientDataJSONHex": assertion.rawClientDataJSON.hexString,
                ]
                if let uid = assertion.userID, !uid.isEmpty {
                    dict["userIdHex"] = uid.hexString
                }
                resolve(dict)

            case .failure(let error):
                Self.rejectWithPasskeyError(error, reject: reject)
            }
        }
    }

    // MARK: - sign

    @objc func sign(
        _ challengeHex: String,
        credentialId: String?,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard #available(iOS 16.0, *) else {
            reject("PASSKEY_NOT_SUPPORTED", "Passkeys require iOS 16+", nil)
            return
        }

        guard let challengeData = Data(hexString: challengeHex) else {
            reject("PASSKEY_FAILED", "Invalid challenge hex", nil)
            return
        }

        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: Self.relyingParty
        )
        let request = provider.createCredentialAssertionRequest(challenge: challengeData)

        if let credHex = credentialId, !credHex.isEmpty {
            guard let credData = Data(hexString: credHex) else {
                reject("PASSKEY_FAILED", "Invalid credentialId hex", nil)
                return
            }
            request.allowedCredentials = [
                ASAuthorizationPlatformPublicKeyCredentialDescriptor(credentialID: credData)
            ]
        }

        performPasskeyRequest(request) { [weak self] result in
            self?.activeHandler = nil
            switch result {
            case .success(let authorization):
                guard let assertion = authorization.credential
                        as? ASAuthorizationPlatformPublicKeyCredentialAssertion else {
                    reject("PASSKEY_NO_CREDENTIAL", "No assertion credential returned", nil)
                    return
                }
                var dict: [String: Any] = [
                    "credentialId": assertion.credentialID.hexString,
                    "signatureHex": assertion.signature.hexString,
                    "authenticatorDataHex": assertion.rawAuthenticatorData.hexString,
                    "clientDataJSONHex": assertion.rawClientDataJSON.hexString,
                ]
                if let uid = assertion.userID, !uid.isEmpty {
                    dict["userIdHex"] = uid.hexString
                }
                resolve(dict)

            case .failure(let error):
                Self.rejectWithPasskeyError(error, reject: reject)
            }
        }
    }

    // MARK: - Private

    private func performPasskeyRequest(
        _ request: ASAuthorizationRequest,
        completion: @escaping (Result<ASAuthorization, Error>) -> Void
    ) {
        let handler = PasskeyRequestHandler()
        self.activeHandler = handler  // prevent ARC from releasing before callback
        handler.performRequest(request, completion: completion)
    }

    private static func rejectWithPasskeyError(
        _ error: Error,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        if let authError = error as? ASAuthorizationError,
           authError.code == .canceled {
            reject("PASSKEY_CANCELLED", "Passkey operation was cancelled", error)
        } else {
            reject("PASSKEY_FAILED", error.localizedDescription, error)
        }
    }

    private static func encodeUserID(name: String) -> Data {
        let combined = "\(name)\0\(UUID().uuidString)"
        return Data(combined.utf8)
    }

    private static func generateChallenge() -> Data {
        var bytes = [UInt8](repeating: 0, count: 32)
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes)
    }
}

// MARK: - PasskeyRequestHandler

private class PasskeyRequestHandler: NSObject,
    ASAuthorizationControllerDelegate,
    ASAuthorizationControllerPresentationContextProviding {

    private var completion: ((Result<ASAuthorization, Error>) -> Void)?

    func performRequest(
        _ request: ASAuthorizationRequest,
        completion: @escaping (Result<ASAuthorization, Error>) -> Void
    ) {
        self.completion = completion
        DispatchQueue.main.async {
            let controller = ASAuthorizationController(authorizationRequests: [request])
            controller.delegate = self
            controller.presentationContextProvider = self
            controller.performRequests()
        }
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        completion?(.success(authorization))
        completion = nil
    }

    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        completion?(.failure(error))
        completion = nil
    }

    func presentationAnchor(for controller: ASAuthorizationController) -> ASPresentationAnchor {
        guard let scene = UIApplication.shared.connectedScenes.first as? UIWindowScene,
              let window = scene.windows.first else {
            return ASPresentationAnchor()
        }
        return window
    }
}

// MARK: - Data ↔ Hex

private extension Data {
    var hexString: String {
        map { String(format: "%02x", $0) }.joined()
    }

    init?(hexString: String) {
        let clean = hexString.hasPrefix("0x") ? String(hexString.dropFirst(2)) : hexString
        guard clean.count % 2 == 0 else { return nil }
        var data = Data(capacity: clean.count / 2)
        var index = clean.startIndex
        while index < clean.endIndex {
            let nextIndex = clean.index(index, offsetBy: 2)
            guard let byte = UInt8(clean[index..<nextIndex], radix: 16) else { return nil }
            data.append(byte)
            index = nextIndex
        }
        self = data
    }
}
