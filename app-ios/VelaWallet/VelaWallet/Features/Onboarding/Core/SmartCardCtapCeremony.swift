import CryptoTokenKit
import Foundation
import VelaCore

/// The app-owned CTAP2 ceremony over CCID — a USB-C security key reached
/// through CryptoTokenKit, no system passkey service involved.
///
/// iOS has no public USB-HID host API, but it DOES have a smart-card interface,
/// and a FIDO key answers CTAP2 over ISO 7816 APDUs on it (the same binding it
/// uses over NFC). The whole protocol — applet SELECT, the PIN/UV dance, the
/// keepalive poll, `getNextAssertion` enumeration — lives in `vela_core`'s
/// `ApduCable` and runs in Rust, the same code the desktop and Android run.
/// This class is only iOS's side of its two seams:
///
///  * the transport ([SmartCardCtapPort], a `TKSmartCard` as the core's
///    `CcidPort`), and
///  * the person and the platform ([CtapCeremonyHost] — PIN, wallet picker,
///    randomness, the touch prompt).
///
/// **Why, when `ASAuthorizationSecurityKeyProvider` already exists.** The system
/// provider requires the `webcredentials:` entitlement and a live
/// apple-app-site-association — so a lapsed or merely-DOWN domain padlocks it.
/// This path consults no association and no Apple service; it is the escape
/// hatch (FR-009c), the same one the desktop and Android give.
///
/// Requires the `com.apple.security.smartcard` entitlement and a YubiKey with
/// firmware 5.8+ (FIDO over CCID). Founder-proven on device in the demo.
final class SmartCardCtapCeremony {

    /// The UI seam. The model implements it; every method BLOCKS the calling
    /// (background) thread until the person answers on the main actor — a
    /// synchronous CTAP host callback cannot suspend, so it waits on a
    /// semaphore the UI signals.
    protocol Prompts: AnyObject, Sendable {
        /// The key's PIN. `nil` is a dismissal. `retries` is -1 when the key
        /// would not say how many attempts are left.
        func askPin(product: String, retries: Int, isRetry: Bool) -> String?
        /// One key holds several wallets — which? `nil` is a dismissal.
        func askWhichWallet(_ choices: [CtapCredentialChoice]) -> Int?
        /// The key is blinking. `kind` is "presence" / "fingerprint" /
        /// "select"; `nil` clears the prompt.
        func touchWaiting(kind: String?, product: String)
        /// No key present: "insert your security key" until `probe` finds one
        /// (`true`) or the person closes the sheet (`false`).
        func awaitKeyInsertion(probe: @escaping () async -> Bool) async -> Bool
    }

    private let prompts: Prompts

    init(prompts: Prompts) {
        self.prompts = prompts
    }

    /// Is a card/reader with a valid card present for this path to use?
    func deviceAvailable() async -> Bool {
        guard let manager = TKSmartCardSlotManager.default else { return false }
        return await Self.firstValidCard(manager) != nil
    }

    func register(name: String, excludeCredentialIds: [String]) async throws -> Registration {
        try await run { port, host in
            try ctapRegisterCcid(
                port: port,
                host: host,
                name: name,
                excludeCredentialIds: excludeCredentialIds
            ).toRegistration()
        }
    }

    func assert(challenge: Data, credentialIdHex: String?) async throws -> Assertion {
        try await run { port, host in
            try ctapAssertCcid(
                port: port,
                host: host,
                challenge: challenge,
                credentialIdHex: credentialIdHex ?? ""
            ).toAssertion()
        }
    }

    /// Connect the first slot holding a valid card, begin a session, and run
    /// [body] on a background queue — the ceremony BLOCKS (it is the Rust
    /// protocol driving the host callbacks), so it must not run on the main
    /// actor. `CtapError` is mapped onto the shell's `PasskeyFailure` so the
    /// core sees the same vocabulary every other path produces.
    private func run<T>(
        _ body: @escaping (SmartCardCtapPort, CtapCeremonyHost) throws -> T
    ) async throws -> T {
        guard let manager = TKSmartCardSlotManager.default else {
            throw PasskeyFailure(
                kind: .notSupported,
                message: "Smart-card access is unavailable (missing entitlement or unsupported device)."
            )
        }
        var found = await Self.firstValidCard(manager)
        if found == nil {
            // No key is a WAITABLE state, not a diagnosis: the person is
            // holding the key they are about to plug in. Failing here said
            // "Biometric authentication is not available on this device" — a
            // sentence about the wrong subject entirely (issue #450, iPad; the
            // same thing Android fixed on 2026-08-28). The sheet polls;
            // plugging the key in continues the ceremony by itself, closing
            // the sheet is a cancel.
            let inserted = await prompts.awaitKeyInsertion {
                await Self.firstValidCard(manager) != nil
            }
            guard inserted else {
                throw PasskeyFailure(kind: .cancelled, message: "No key was inserted")
            }
            found = await Self.firstValidCard(manager)
        }
        guard let found else {
            throw PasskeyFailure(
                kind: .notSupported,
                message: "No security key is present. Plug in a USB-C security key and try again."
            )
        }
        let (card, slotName) = (found.card, found.slot)
        guard try await card.beginSession() else {
            throw PasskeyFailure(kind: .other, message: "Could not open a session with the security key.")
        }

        let port = SmartCardCtapPort(card: card, slotName: slotName)
        let host = HostBridge(prompts: prompts)
        defer {
            prompts.touchWaiting(kind: nil, product: "")
            card.endSession()
        }

        return try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global(qos: .userInitiated).async {
                do {
                    continuation.resume(returning: try body(port, host))
                } catch let error as CtapError {
                    continuation.resume(throwing: error.toPasskeyFailure())
                } catch {
                    continuation.resume(
                        throwing: PasskeyFailure(kind: .other, message: error.localizedDescription)
                    )
                }
            }
        }
    }

    /// The first slot holding a card that answers, or `nil` when no key is
    /// plugged in.
    private static func firstValidCard(
        _ manager: TKSmartCardSlotManager
    ) async -> (card: TKSmartCard, slot: String)? {
        for name in manager.slotNames {
            guard let slot = await manager.getSlot(withName: name), slot.state == .validCard,
                  let card = slot.makeSmartCard()
            else { continue }
            return (card, name)
        }
        return nil
    }
}

/// A `TKSmartCard` as the core cable's `CcidPort`.
///
/// Raw transmit only: the applet SELECT, the `0x9100` keepalive poll and the
/// `61xx` GET RESPONSE chaining are the core `ApduCable`'s. `TKSmartCard.transmit`
/// is async and the core calls synchronously (on a background thread), so a
/// semaphore bridges the two — exactly what the background context is for.
final class SmartCardCtapPort: CcidPort, @unchecked Sendable {
    private let card: TKSmartCard
    private let slotName: String

    init(card: TKSmartCard, slotName: String) {
        self.card = card
        self.slotName = slotName
    }

    func transmit(apdu: Data) -> ApduOutcome {
        let semaphore = DispatchSemaphore(value: 0)
        var outcome: ApduOutcome = .failed(detail: "no reply from the security key")
        card.transmit(apdu) { data, error in
            if let data {
                outcome = .response(bytes: data)
            } else {
                outcome = .failed(detail: error?.localizedDescription ?? "APDU transmit failed")
            }
            semaphore.signal()
        }
        semaphore.wait()
        return outcome
    }

    func pollDelay() {
        Thread.sleep(forTimeInterval: 0.1)
    }

    func product() -> String { slotName }
    func path() -> String { slotName }
}

/// The Kotlin/Swift `CtapCeremonyHost`, bridging the Rust ceremony's
/// synchronous callbacks to the UI.
private final class HostBridge: CtapCeremonyHost, @unchecked Sendable {
    private let prompts: SmartCardCtapCeremony.Prompts

    init(prompts: SmartCardCtapCeremony.Prompts) {
        self.prompts = prompts
    }

    func pin(request: CtapPinRequest) -> String? {
        prompts.askPin(
            product: request.product,
            retries: Int(request.retries),
            isRetry: request.retry
        )
    }

    func pick(choices: [CtapCredentialChoice]) -> UInt32? {
        prompts.askWhichWallet(choices).map(UInt32.init)
    }

    func random(len: UInt32) -> Data {
        var bytes = [UInt8](repeating: 0, count: Int(len))
        _ = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        return Data(bytes)
    }

    func note(line: String) {
        print("[vela-wallet] ccid.ctap: \(line)")
    }

    func touch(kind: String, product: String) {
        prompts.touchWaiting(kind: kind, product: product)
    }
}

extension CtapRegistration {
    func toRegistration() -> Registration {
        Registration(
            credentialIdHex: credentialIdHex,
            attestationObjectHex: attestationObjectHex,
            clientDataJsonHex: clientDataJsonHex,
            authenticatorAttachment: authenticatorAttachment,
            transports: transports
        )
    }
}

extension CtapAssertion {
    func toAssertion() -> Assertion {
        Assertion(
            credentialIdHex: credentialIdHex,
            signatureDerHex: signatureDerHex,
            authenticatorDataHex: authenticatorDataHex,
            clientDataJsonHex: clientDataJsonHex,
            userIdHex: userIdHex.isEmpty ? nil : userIdHex,
            authenticatorAttachment: authenticatorAttachment
        )
    }
}

extension CtapError {
    func toPasskeyFailure() -> PasskeyFailure {
        switch self {
        case .Cancelled:
            return PasskeyFailure(kind: .cancelled, message: "User cancelled the operation")
        case let .NotSupported(detail):
            return PasskeyFailure(kind: .notSupported, message: detail)
        case let .NotDiscoverable(detail):
            return PasskeyFailure(kind: .notDiscoverable, message: detail)
        case let .Other(detail):
            return PasskeyFailure(kind: .other, message: detail)
        }
    }
}
