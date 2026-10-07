//
//  CableWatchdogTests.swift
//  VelaWalletTests
//
//  Issue #446: an iPad's sign-in with a phone failed on every attempt with
//  NSURLErrorCancelled (-999). The WebSocket tunnel's read watchdog, cancelled
//  the moment a frame arrived, fell out of its sleep and cancelled the socket —
//  the BLE channel's 2026-08-28 bug, shipped a second time without its guard.
//  And the sheet then told the person to set up Face ID.
//

import Testing
@testable import VelaWallet

@MainActor
private final class Fired {
    var count = 0
}

@MainActor
struct CableWatchdogTests {
    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    @Test func aCancelledWatchdogNeverFires() async throws {
        let fired = Fired()
        let watchdog = CableWatchdog.start(after: 50_000_000) { fired.count += 1 }
        // The read it guards finished: the deferred cancel.
        watchdog.cancel()
        try await Task.sleep(nanoseconds: 300_000_000)
        #expect(fired.count == 0)
    }

    @Test func aWatchdogLeftRunningFiresOnce() async throws {
        let fired = Fired()
        CableWatchdog.start(after: 20_000_000) { fired.count += 1 }
        try await Task.sleep(nanoseconds: 400_000_000)
        #expect(fired.count == 1)
    }

    @Test func theBudgetIsTheCeremonysNotTheConnects() {
        // 130 s: the person approving on the other phone, not a 15 s connect.
        #expect(CableWatchdog.ceremonyBudget == 130_000_000_000)
    }

    @Test func aFailedPhoneLinkIsNotBlamedOnBiometrics() {
        let link = promptCopy(
            PromptKind(
                type: "sign_in_failed",
                detail: "caBLE transport: Error Domain=NSURLErrorDomain Code=-999 \"cancelled\"",
                phoneLink: true
            ),
            loc: loc
        )
        #expect(link.message == loc.t(I18nKeys.Flow.phoneLinkFailed))
        #expect(!link.message.contains("Face ID"))
        #expect(!link.message.contains("NSURLErrorDomain"))

        // An authenticator's own failure keeps the old sheet.
        let other = promptCopy(PromptKind(type: "sign_in_failed", detail: "boom"), loc: loc)
        #expect(other.message.contains("boom"))

        // A create over the phone link says the same.
        let create = promptCopy(
            PromptKind(type: "create_failed", detail: "caBLE transport: closed", phoneLink: true),
            loc: loc
        )
        #expect(create.message == loc.t(I18nKeys.Flow.phoneLinkFailed))
    }

    @Test func theFlagIsReadFromTheCoresJSON() {
        let kind = PromptKind(json: ["type": "sign_in_failed", "detail": "x", "phone_link": true])
        #expect(kind.phoneLink)
        #expect(!PromptKind(json: ["type": "sign_in_failed", "detail": "x"]).phoneLink)
    }
}
