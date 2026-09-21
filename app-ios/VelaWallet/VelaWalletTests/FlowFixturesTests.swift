//
//  FlowFixturesTests.swift
//  VelaWalletTests
//
//  Mechanical coverage of the v2 state set (spec 019 T136/T141).
//
//  The load-bearing test is `everyFixtureResolvesToTheScreenItsNameClaims`: the
//  gallery and the app share one `screenFor`, so a fixture that renders the
//  wrong step here renders the wrong step in production too. Spec 014's version
//  of this file pinned design codes against a presentation type this app owned;
//  that type is gone, and pinning a code list against fixtures nobody ships
//  would only check the fixtures against themselves.
//

import Testing
@testable import VelaWallet

@MainActor
struct FlowFixturesTests {

    private var flows: [(String, CreateView)] {
        FlowFixtures.all.compactMap { entry in
            if case .flow(let view) = entry.fixture { (entry.code, view) } else { nil }
        }
    }

    private var sheets: [(String, PromptKind, Bool)] {
        FlowFixtures.all.compactMap { entry in
            if case .sheet(let kind, let confirmable) = entry.fixture {
                (entry.code, kind, confirmable)
            } else {
                nil
            }
        }
    }

    @Test func everyFixtureResolvesToTheScreenItsNameClaims() {
        let expected: [String: FlowScreen] = [
            "name · empty": .name,
            "name · filled": .name,
            "name · too long": .name,
            "name · draft waiting": .name,
            "keys · one, needs a second": .keys,
            "keys · two, ready": .keys,
            "keys · unconfirmed row": .keys,
            "keys · at the cap": .keys,
            "progress · verify": .progress,
            "progress · derive": .progress,
            "progress · publish": .progress,
            "retry · publish failed": .retry,
            "done": .done,
        ]
        #expect(flows.count == expected.count)
        for (code, view) in flows {
            #expect(screenFor(view) == expected[code], "fixture `\(code)` renders the wrong screen")
        }
    }

    @Test func fixtureCodesAreUnique() {
        let codes = FlowFixtures.all.map(\.code)
        #expect(codes.count == Set(codes).count)
    }

    /// The nine prompt kinds the core can raise, all present.
    ///
    /// Spec 014's eighteen `OutcomeKind` values were not reduced so much as
    /// relocated: eight of them are screens in v2 rather than sheets. What is
    /// left is what a sheet is for.
    @Test func everyPromptKindHasASheetFixture() {
        #expect(Set(sheets.map(\.1.type)) == [
            "not_supported_create",
            "not_supported_login",
            "not_discoverable",
            "incompatible_create",
            "incompatible_login",
            "recover_offer",
            "recover_failed",
            "create_failed",
            "sign_in_failed",
        ])
    }

    /// Only the recovery offer is confirmable — its answer is the one that branches.
    @Test func onlyTheRecoveryOfferIsConfirmable() {
        for (_, kind, confirmable) in sheets {
            #expect(confirmable == (kind.type == "recover_offer"))
        }
    }

    /// The two prompts that carry the platform's own words must actually carry them.
    @Test func detailBearingPromptsHaveDetail() {
        for (_, kind, _) in sheets where kind.type == "create_failed" || kind.type == "sign_in_failed" {
            #expect(!(kind.detail ?? "").isEmpty)
        }
    }

    /// `settingUpIdentity` is NOT a progress-screen status.
    ///
    /// It happens before the key list exists, so it belongs to the Name screen's
    /// status line. A mapping that promoted it would send the person to a
    /// progress screen with a zero-key subtitle.
    @Test func settingUpIdentityStaysOnTheNameScreen() {
        #expect(progressFor(.settingUpIdentity) == nil)
        #expect(progressFor(.setupCancelled) == nil)
        #expect(progressFor(.verifyCancelled) == nil)
        #expect(progressFor(.verifyingIdentity) != nil)
        #expect(progressFor(.extractingKey) != nil)
        #expect(progressFor(.computingAddress) != nil)
        #expect(progressFor(.syncingKey) != nil)
    }

    /// Every progress position points at a real task row.
    @Test func progressPositionsStayInsideTheTaskList() {
        for status in StatusKey.allCases {
            guard let position = progressFor(status) else { continue }
            #expect(progressTasks.indices.contains(position.activeTask))
            #expect((1...100).contains(position.percent))
        }
    }

    /// Every semantic variant the core emits has copy. Exhaustive by enum.
    @Test func everySemanticVariantHasCopy() {
        for status in StatusKey.allCases {
            #expect(statusKeyToI18n(status).hasPrefix("onboarding."))
        }
        for label in SubmitLabel.allCases {
            #expect(submitLabelToI18n(label).hasPrefix("onboarding."))
        }
        for method in KeyMethod.allCases {
            #expect(providerLineFor(method).hasPrefix("onboarding."))
            let copy = methodCopy(method)
            #expect(copy.title.hasPrefix("onboarding."))
            #expect(copy.body.hasPrefix("onboarding."))
        }
    }

    /// Issue #207: a key row's badge claims only what somebody verified, and
    /// its caption says where the key LIVES (the authenticator's report), not
    /// what was tapped — the web's `keyBadge` / `providerLineFor`.
    @Test func aKeyRowBadgesOnlyWhatItCanVouchForAndNamesWhereItLives() throws {
        func row(synced: Bool, known: Bool, method: String = "hybrid", kind: String = "security_key")
            throws -> CreateKeyRow {
            try CoreJSON.decode(CreateKeyRow.self, from: [
                "name": "Key", "authenticator_attachment": "cross-platform", "transports": "usb",
                "confirmed": true, "synced": synced, "synced_known": known, "aaguid": "",
                "provider_name": "", "method": method, "kind": kind,
            ])
        }
        // Unreadable attestation: `synced` fails open to true for the GATE, and
        // no green "Cloud-synced" is drawn from that guess.
        #expect(keyBadge(try row(synced: true, known: false)) == nil)
        #expect(keyBadge(try row(synced: true, known: true))?.text == I18nKeys.Create.keySyncedBadge)
        #expect(keyBadge(try row(synced: false, known: true))?.synced == false)
        #expect(keyBadge(try row(synced: false, known: true))?.text == I18nKeys.Create.keyDeviceOnlyBadge)

        // A "Phone or tablet" tap answered by a USB key is a security key.
        let fob = try row(synced: false, known: true)
        #expect(fob.method == .hybrid && fob.kind == .securityKey)
        #expect(providerLineFor(fob.kind) == I18nKeys.Create.providerSecurityKey)
        #expect(providerLineFor(.platform) == I18nKeys.Create.methodPlatformTitle)
        #expect(providerLineFor(.hybrid) == I18nKeys.Create.methodHybridTitle)
    }

    /// The cap fixture sits exactly at the core's `MAX_MULTI_KEYS`, not near it.
    @Test func theCapFixtureIsAtTheCap() {
        let view = flows.first { $0.0 == "keys · at the cap" }!.1
        #expect(view.keys.count == maxKeys)
        #expect(!view.canAddKey, "a full list must not offer another key")
    }

    /// An address exists on the Done fixture and nowhere else.
    ///
    /// The core withholds `address` until the group has landed and the account
    /// is saved — an address shown earlier is one somebody can fund before the
    /// wallet is reachable. A fixture that leaked it would make that ordering
    /// look optional.
    @Test func onlyTheDoneFixtureCarriesAnAddress() {
        for (code, view) in flows {
            if view.stage == .created {
                #expect(view.address == FlowFixtures.fixtureAddress)
            } else {
                #expect(view.address == nil, "fixture `\(code)` shows an address before there is one")
            }
        }
    }
}
