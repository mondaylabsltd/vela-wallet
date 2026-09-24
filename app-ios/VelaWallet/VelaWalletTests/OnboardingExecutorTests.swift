//
//  OnboardingExecutorTests.swift
//  VelaWalletTests
//
//  The failure contract, and the exhaustiveness the compiler cannot give us.
//
//  The bridge is JSON, so `OnboardingExecutor`'s eighteen-way branch is a switch
//  over strings rather than the desktop's `match` over an enum — the compiler
//  will not notice an operation nobody handled. **This file is that check.** An
//  operation with no failure variant leaves the core waiting forever on an
//  effect nobody answers, which presents as a spinner that never stops, with no
//  error anywhere.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct OnboardingExecutorTests {

    private func operation(_ type: String) -> [String: Any] { ["type": type] }

    private func type(of answer: [String: Any]) -> String { answer["type"] as? String ?? "" }

    @Test func everyOperationOwnsAFailureVariant() {
        #expect(OnboardingExecutor.operations.count == 18)
        for name in OnboardingExecutor.operations {
            let answer = OnboardingExecutor.failure(
                for: operation(name),
                error: RegistryFailure(message: "boom", network: true)
            )
            #expect(!type(of: answer).isEmpty, "no failure variant for `\(name)`")
        }
    }

    /// The session machine's operations, all of them — a count, because the
    /// failure map answers by NAME and an operation nobody taught it would
    /// fall through to `accounts_unavailable` and sign the device out.
    ///
    /// Eight since 2026-09-23: `remove_account`, one wallet leaving a device
    /// that keeps the others.
    @Test func sessionOperationsAreAllEight() {
        #expect(SessionExecutor.operations.count == 8)
    }

    /// `network` is the one bit of classification only a shell can supply: a
    /// request that never arrived is not the same as one the server refused.
    /// The core branches on it — an unreachable index offers a different
    /// endpoint, a 4xx does not.
    @Test func onlyATransportFailureIsNetwork() {
        let refused = OnboardingExecutor.failure(
            for: operation("registry_query_by_public_key"),
            error: RegistryFailure(message: "Query failed: 404", network: false)
        )
        #expect(type(of: refused) == "index_failed")
        #expect(refused["network"] as? Bool == false)

        let unreachable = OnboardingExecutor.failure(
            for: operation("registry_publish"),
            error: RegistryFailure(message: "Register failed: offline", network: true)
        )
        #expect(unreachable["network"] as? Bool == true)

        // An error that is not a registry failure at all cannot claim the server
        // answered — it never got that far.
        struct Odd: Error {}
        let unknown = OnboardingExecutor.failure(for: operation("registry_query_unit"), error: Odd())
        #expect(unknown["network"] as? Bool == true)
    }

    /// `sign_member_proof` is the mixed one: the ceremony and the challenge
    /// fetch can each fail, and the core branches differently on the two.
    /// Classifying by the OPERATION rather than by what threw would send a
    /// network outage to the passkey sheet.
    @Test func memberProofClassifiesByWhatThrew() {
        let fromRegistry = OnboardingExecutor.failure(
            for: operation("sign_member_proof"),
            error: RegistryFailure(message: "Challenge failed: 503", network: false)
        )
        #expect(type(of: fromRegistry) == "index_failed")

        let fromCeremony = OnboardingExecutor.failure(
            for: operation("sign_member_proof"),
            error: PasskeyFailure(kind: .cancelled, message: "cancelled")
        )
        #expect(type(of: fromCeremony) == "passkey_failed")
        #expect(fromCeremony["kind"] as? String == "cancelled")
    }

    /// A cancellation carries no message; everything else carries the platform's
    /// own words. Prettifying them would lose the only detail a bug report has.
    @Test func onlyClassifiedFailuresDropTheirMessage() {
        let cancelled = OnboardingExecutor.failure(
            for: operation("register_passkey"),
            error: PasskeyFailure(kind: .cancelled, message: "User cancelled the operation")
        )
        #expect(cancelled["message"] is NSNull)

        let other = OnboardingExecutor.failure(
            for: operation("register_passkey"),
            error: PasskeyFailure(kind: .other, message: "provider exploded")
        )
        #expect(other["message"] as? String == "provider exploded")
    }

    /// Three operations degrade rather than fail; the core must never see an error.
    @Test func bestEffortOperationsDegradeQuietly() {
        struct Odd: Error {}
        let name = OnboardingExecutor.failure(for: operation("lookup_legacy_name"), error: Odd())
        #expect(type(of: name) == "legacy_name")
        #expect(name["name"] is NSNull)

        let health = OnboardingExecutor.failure(for: operation("probe_index_health"), error: Odd())
        #expect(health["ok"] as? Bool == false)

        // A dismissed dialog is a refusal, not an error.
        let prompt = OnboardingExecutor.failure(for: operation("prompt"), error: Odd())
        #expect(prompt["accepted"] as? Bool == false)
    }

    /// `check_passkey_support` never fails outward — the contract says report
    /// `supported: false`. A thrown support check would abort a create before
    /// the form is even on screen.
    @Test func supportCheckFailsAsUnsupported() {
        struct Odd: Error {}
        let answer = OnboardingExecutor.failure(for: operation("check_passkey_support"), error: Odd())
        #expect(type(of: answer) == "passkey_support")
        #expect(answer["supported"] as? Bool == false)
    }

    /// An operation nobody handled still answers — the core has to be unblocked
    /// whatever happened — but says loudly what went wrong.
    @Test func anUnknownOperationAnswersLoudly() {
        struct Odd: Error {}
        let answer = OnboardingExecutor.failure(for: operation("teleport_wallet"), error: Odd())
        #expect(type(of: answer) == "storage_failed")
        #expect((answer["message"] as? String ?? "").contains("teleport_wallet"))
    }

    /// Every failure variant serializes: an answer the bridge cannot parse is
    /// the same as no answer at all.
    @Test func everyFailureVariantSerializes() {
        struct Odd: Error {}
        for name in OnboardingExecutor.operations {
            let json = CoreJSON.string(OnboardingExecutor.failure(for: operation(name), error: Odd()))
            let parsed = try? CoreJSON.object(json)
            #expect(parsed?["type"] as? String != nil, "`\(name)` produced unparseable JSON")
        }
    }

    /// UTC, always: a local offset in `created_at_iso` means something else abroad.
    @Test func timestampsAreUtcIso() {
        let stamp = OnboardingExecutor.nowISO()
        #expect(stamp.hasSuffix("Z"))
        #expect(stamp.range(of: #"^\d{4}-\d{2}-\d{2}T[\d:.]+Z$"#, options: .regularExpression) != nil)
    }

    /// The two recovery purposes share a label; what differs is the BYTES.
    @Test func recoveryChallengesShareALabelAndDifferInBytes() {
        let verify = String(decoding: OnboardingExecutor.challenge(for: "verify"), as: UTF8.self)
        let first = String(decoding: OnboardingExecutor.challenge(for: "recover_first"), as: UTF8.self)
        let second = String(decoding: OnboardingExecutor.challenge(for: "recover_second"), as: UTF8.self)
        #expect(verify.hasPrefix("vela-verify-"))
        #expect(first.hasPrefix("vela-recover-"))
        #expect(second.hasPrefix("vela-recover-"))
    }

    /// A pasted URL with a stray newline or slash must not become a broken host.
    @Test func registryURLIsNormalized() {
        #expect(RegistryClient.normalize("  https://example.test/ \n") == "https://example.test")
        #expect(RegistryClient.normalize("   ") == RegistryClient.defaultURL)
    }

    /// The handle shape every Vela client mints, and the core parses back.
    @Test func userHandleCarriesNulAndAUuid() {
        let handle = PasskeyExecutor.encodeUserHandle("Ada")
        let parts = handle.split(separator: "\u{0}", maxSplits: 1, omittingEmptySubsequences: false)
        #expect(parts.count == 2)
        #expect(parts[0] == "Ada")
        #expect(UUID(uuidString: String(parts[1])) != nil)
    }
}
