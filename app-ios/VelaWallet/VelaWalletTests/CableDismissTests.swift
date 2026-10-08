//
//  CableDismissTests.swift
//  VelaWalletTests
//
//  Issue #459: dismissing the "Scan a code" sheet held the person for the
//  whole 90 s scan and then said "your other device didn't connect". Now a
//  dismissal ends THAT ceremony as `PasskeyFailure(.cancelled)` — the answer
//  the core takes quietly (sign-in goes idle, create returns to its keys) —
//  and never the next one: recovery's second signature, or a retry.
//
//  No Bluetooth is needed: a dismissal that lands before the scan has begun
//  must still end it, and that is the deterministic case. Where a test lets a
//  scan run, it holds whatever the radio says (a simulator may end the scan
//  on its own) and asserts only what is true either way.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A phone ceremony asks none of these before its channel is up.
private final class SilentPrompts: SmartCardCtapCeremony.Prompts, @unchecked Sendable {
    func askPin(product: String, retries: Int, isRetry: Bool) -> String? { nil }
    func askWhichWallet(_ choices: [CtapCredentialChoice]) -> Int? { nil }
    func touchWaiting(kind: String?, product: String) {}
    func awaitKeyInsertion(probe: @escaping () async -> Bool) async -> Bool { false }
}

/// What the QR sheet was told, in order, and — when set — a dismissal the
/// moment a code appears.
@MainActor
private final class CodeScreen {
    var shown: [String?] = []
    var dismissOnShow = false
    weak var ceremony: HybridCeremony?

    func show(_ payload: String?) {
        shown.append(payload)
        if payload != nil, dismissOnShow { ceremony?.cancel() }
    }
}

@MainActor
struct CableDismissTests {
    private let challenge = Data(repeating: 7, count: 32)

    private func ceremony(_ screen: CodeScreen) -> HybridCeremony {
        let ceremony = HybridCeremony(prompts: SilentPrompts()) { payload, _ in screen.show(payload) }
        screen.ceremony = ceremony
        return ceremony
    }

    /// How a ceremony ended: `nil` for an assertion, else the failure's kind.
    private func outcome(_ ceremony: HybridCeremony) async -> FailureKind? {
        do {
            _ = try await ceremony.assert(challenge: challenge, credentialIdHex: nil)
            return nil
        } catch let failure as PasskeyFailure {
            return failure.kind
        } catch {
            Issue.record("not a PasskeyFailure: \(error)")
            return .other
        }
    }

    /// The code is dismissed as it appears: the ceremony ends as a cancel —
    /// never "no phone answered" — and the code comes down.
    @Test(.timeLimit(.minutes(5)))
    func aDismissedCodeEndsItsCeremonyAsACancel() async {
        let screen = CodeScreen()
        screen.dismissOnShow = true
        let kind = await outcome(ceremony(screen))
        #expect(kind == .cancelled)
        #expect(screen.shown.count == 2)
        #expect(screen.shown.first.map { $0 != nil } == true, "a code went up")
        #expect(screen.shown.last.map { $0 == nil } == true, "and came down")
    }

    /// The same for a registration: the create flow's "add a key on your
    /// phone" is the same sheet.
    @Test(.timeLimit(.minutes(5)))
    func aDismissedCodeEndsARegistrationAsACancel() async {
        let screen = CodeScreen()
        screen.dismissOnShow = true
        do {
            _ = try await ceremony(screen).register(name: "Vela", excludeCredentialIds: [])
            Issue.record("a dismissed registration returned a key")
        } catch let failure as PasskeyFailure {
            #expect(failure.kind == .cancelled)
        } catch {
            Issue.record("not a PasskeyFailure: \(error)")
        }
    }

    /// A cancel reaches only the ceremony in flight: after a dismissed one,
    /// and after a cancel with nothing in flight, the next ceremony puts up a
    /// fresh code and is NOT born cancelled.
    @Test(.timeLimit(.minutes(5)))
    func aCancelNeverReachesTheNextCeremony() async throws {
        let screen = CodeScreen()
        let ceremony = ceremony(screen)

        screen.dismissOnShow = true
        #expect(await outcome(ceremony) == .cancelled)
        // Nothing in flight now: this must reach nobody.
        ceremony.cancel()

        screen.dismissOnShow = false
        screen.shown = []
        let next = Task { await outcome(ceremony) }
        await Wait.until { screen.shown.first != nil }
        #expect(screen.shown.first.map { $0 != nil } == true, "the next ceremony shows its own code")
        // A leaked cancel would end it at once. Give one the chance to show;
        // whatever the radio does meanwhile, the assertions below hold.
        try await Task.sleep(nanoseconds: 300_000_000)
        let endedByItself = screen.shown.count > 1
        ceremony.cancel()
        let kind = await next.value
        if endedByItself {
            #expect(kind != .cancelled, "an earlier cancel ended a ceremony that started after it")
        } else {
            #expect(kind == .cancelled, "dismissed mid-scan, it ends as a cancel")
        }
        #expect(screen.shown.last.map { $0 == nil } == true, "the code came down")
    }
}
