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
/// **What it needs** is physical, not an entitlement: a USB-C port and a key
/// that offers FIDO over its CCID (smart-card) interface — a YubiKey on
/// firmware 5.8 or later. `TKSmartCardSlotManager.default` is always there on
/// iOS 16+ (the iOS SDK's `TKSmartCard.h`: "iOS: The defaultManager instance
/// is always accessible"); `com.apple.security.smartcard` gates it on macOS
/// only. Device-verified 2026-08-28 on an iPhone 15 Pro with a YubiKey 5C on
/// firmware 5.8, in a build signed without that key.
///
/// **What it cannot reach** goes through Apple's own security-key sheet
/// (`SystemSheetFallback`, which `PasskeyExecutor` catches): a Lightning key
/// (an MFi accessory, never a CCID reader), an NFC key, and a key below 5.8,
/// whose FIDO applet does not answer here. This route stays FIRST on USB-C —
/// Apple's sheet is the one a down association padlocks.
final class SmartCardCtapCeremony {

    /// How "insert your security key" ended.
    enum KeyInsertion: Sendable, Equatable {
        /// A card answered: the ceremony goes on over CCID.
        case inserted
        /// The sheet was closed: a cancel.
        case cancelled
        /// "Use Apple's security-key sheet" — the way out for a key this
        /// route cannot reach (NFC, Lightning, firmware below 5.8).
        case useSystemSheet
    }

    /// This route cannot run the ceremony — no key answers over CCID — and
    /// Apple's security-key sheet should. Not a failure the core hears:
    /// `PasskeyExecutor` catches it and issues the same request (ES256,
    /// resident key, user verification required) to the system provider.
    struct SystemSheetFallback: Error, Equatable {}

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
        /// No key present: "insert your security key" until `probe` finds
        /// one, the person closes the sheet, or they ask for Apple's sheet.
        ///
        /// `@MainActor` on the probe is load-bearing: a bare `() async -> Bool`
        /// is `nonisolated(nonsending)` under this target's settings, and its
        /// metadata needs a runtime entry point iOS 17 does not have (087 F33).
        func awaitKeyInsertion(probe: @escaping @MainActor () async -> Bool) async -> KeyInsertion
    }

    private let prompts: Prompts

    /// The person asked for Apple's sheet on the insert prompt. The
    /// ceremonies that FOLLOW in the same flow — a new key's proof of
    /// signing, a recovery's second signature — go there too, without asking
    /// again: the key that could not be reached here a moment ago has not
    /// changed. Forgotten the moment a card answers, and whenever the person
    /// picks a method afresh (`forgetSystemSheetChoice`).
    private var prefersSystemSheet = false

    init(prompts: Prompts) {
        self.prompts = prompts
    }

    /// The person picked a method again: the insert prompt is theirs to see.
    func forgetSystemSheetChoice() {
        prefersSystemSheet = false
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
        // Always there on iOS 16+ (see the type's doc); `nil` only on a system
        // with no smart-card service at all, where Apple's sheet is the route.
        guard let manager = TKSmartCardSlotManager.default else { throw SystemSheetFallback() }
        var found = await Self.firstValidCard(manager)
        if found == nil {
            // A Lightning iPhone has no USB-C port for a CCID key: unless a
            // card already answers (through an adapter — taken above), none
            // will, and "Insert your security key" waited for ever. Its keys —
            // Lightning, NFC — are Apple's sheet's.
            if KeyPort.lightningPhone || prefersSystemSheet { throw SystemSheetFallback() }
            // No key is a WAITABLE state, not a diagnosis: the person is
            // holding the key they are about to plug in. Failing here said
            // "Biometric authentication is not available on this device" — a
            // sentence about the wrong subject entirely (issue #450, iPad; the
            // same thing Android fixed on 2026-08-28). The sheet polls;
            // plugging the key in continues the ceremony by itself, closing
            // the sheet is a cancel. Never a timeout: a person who has not
            // plugged the key in YET is not a key that cannot answer.
            switch await prompts.awaitKeyInsertion(probe: { await Self.firstValidCard(manager) != nil }) {
            case .inserted:
                found = await Self.firstValidCard(manager)
            case .useSystemSheet:
                prefersSystemSheet = true
                throw SystemSheetFallback()
            case .cancelled:
                throw PasskeyFailure(kind: .cancelled, message: "No key was inserted")
            }
        }
        guard let found else {
            throw PasskeyFailure(
                kind: .notSupported,
                message: "No security key is present. Plug in a USB-C security key and try again."
            )
        }
        // A card answers: this route is the one, whatever was chosen before.
        prefersSystemSheet = false
        let (card, slotName) = (found.card, found.slot)
        // A card that will not open a session is one this route cannot talk
        // to; Apple's sheet may (it has its own transports).
        guard (try? await card.beginSession()) == true else { throw SystemSheetFallback() }

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
                    // The key never answered one command here: no FIDO applet
                    // on its CCID interface (firmware below 5.8), or an
                    // exchange that went nowhere. Not this key's failure to
                    // report — Apple's sheet reaches it another way.
                    if Self.fallsBackToSystem(error, answered: port.answered) {
                        continuation.resume(throwing: SystemSheetFallback())
                    } else {
                        continuation.resume(throwing: error.toPasskeyFailure())
                    }
                } catch {
                    continuation.resume(
                        throwing: PasskeyFailure(kind: .other, message: error.localizedDescription)
                    )
                }
            }
        }
    }

    /// A ceremony that failed before the card answered ANYTHING goes to
    /// Apple's sheet; one that failed after it (a wrong PIN, a full key, a
    /// refusal) is that key's own answer and is reported. A cancel is always a
    /// cancel.
    static func fallsBackToSystem(_ error: CtapError, answered: Bool) -> Bool {
        if case .Cancelled = error { return false }
        return !answered
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
    private let lock = NSLock()
    private var completed = false

    /// Has the card completed one command — a reply ending `90 00`? The first
    /// command is the core's SELECT of the FIDO applet, so `false` after a
    /// failed ceremony means the key does not speak FIDO on this interface
    /// (or did not reply at all). A transport fact; what the bytes mean is
    /// the core's.
    var answered: Bool {
        lock.lock()
        defer { lock.unlock() }
        return completed
    }

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
        if case .response(let bytes) = outcome, Self.completes(bytes) {
            lock.lock()
            completed = true
            lock.unlock()
        }
        return outcome
    }

    /// ISO 7816-4: a reply's last two bytes are its status, and `90 00` is
    /// "done, no error".
    static func completes(_ reply: Data) -> Bool {
        reply.count >= 2 && reply[reply.endIndex - 2] == 0x90 && reply[reply.endIndex - 1] == 0x00
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

/// Which port this device's keys plug into — what decides whether waiting for
/// a CCID key makes sense at all.
///
/// The shell's fact, not the core's: only the device knows its own connector.
/// iPhones are the clean case — every model before the iPhone 15 has a
/// Lightning port, every one since has USB-C. An iPad (the iPhone layout runs
/// there too) is not guessed: its line mixes both, so it keeps the "insert
/// your key" screen, which offers Apple's sheet by hand.
enum KeyPort {
    /// This device is an iPhone with a Lightning port.
    static let lightningPhone = lightningPhone(model: model)

    /// The hardware identifier — `iPhone12,1`. A simulator reports the Mac's
    /// machine, and names the model it plays in its environment.
    static var model: String {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] {
            return simulated
        }
        var info = utsname()
        uname(&info)
        return withUnsafeBytes(of: &info.machine) { raw in
            String(decoding: raw.prefix { $0 != 0 }, as: UTF8.self)
        }
    }

    /// `iPhone15,4` (iPhone 15) is the first with USB-C; `iPhone15,2` and
    /// `iPhone15,3` are the 14 Pro pair, Lightning. Anything that is not an
    /// iPhone identifier, or does not parse, is not called Lightning.
    static func lightningPhone(model: String) -> Bool {
        guard model.hasPrefix("iPhone") else { return false }
        let parts = model.dropFirst("iPhone".count).split(separator: ",").map { Int($0) }
        guard parts.count == 2, let major = parts[0], let minor = parts[1] else { return false }
        return major < 15 || (major == 15 && minor < 4)
    }
}
