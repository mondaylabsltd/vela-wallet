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
import VelaCore
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

    /// Issue #460: an error badge is a "!" in the error colour, never the
    /// close glyph. The sheet's column is leading-aligned, so a red × in a
    /// disc sat top-left — where a close button goes — and did nothing.
    @Test func anErrorBadgeIsAnExclamationNeverAClose() {
        #expect(StatusBadge.glyphName(.error) == "exclamationmark")
        for variant: BadgeVariant in [.success, .warning, .neutral, .error, .timeout, .info] {
            #expect(StatusBadge.glyphName(variant) != "xmark", "\(variant) draws the close glyph")
        }
        #expect(sheets.contains { FlowSheet.badge(for: $0.1.type) == .error },
                "the failure sheets wear the error badge this pins")
    }

    @Test func everyFixtureResolvesToTheScreenItsNameClaims() {
        let expected: [String: FlowScreen] = [
            "name · empty": .name,
            "name · filled": .name,
            "name · too long": .name,
            "name · draft waiting": .name,
            "keys · one, needs a second": .keys,
            "keys · two, ready": .keys,
            "keys · signing page offered": .keys,
            "keys · on a self-hosted page": .keys,
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
            "registry_unreachable",
        ])
    }

    /// Only the recovery offer and the registry's free retry are confirmable —
    /// their answers are the ones that branch.
    @Test func onlyTheRecoveryOfferAndTheRegistryRetryAreConfirmable() {
        for (_, kind, confirmable) in sheets {
            #expect(confirmable == (kind.type == "recover_offer" || kind.type == "registry_unreachable"))
        }
    }

    /// PR 2: "can't look up your wallet" — its own words, Try again and
    /// Cancel, a warning (not the offer's info), and the network's words when
    /// nothing left the device. Never the rebuild offer's.
    @Test func theRegistryUnreachablePromptSaysWhatHappenedAndOffersAFreeRetry() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let remote = promptCopy(PromptKind(type: "registry_unreachable"), loc: loc)
        #expect(remote.title == loc.t("onboarding.login.registryUnreachableTitle"))
        #expect(remote.title == "Can't Look Up Your Wallet")
        #expect(remote.message == loc.t("onboarding.login.registryUnreachableBody"))
        #expect(remote.confirmLabel == loc.t("common.tryAgain"))
        #expect(remote.cancelLabel == loc.t("common.cancel"))
        #expect(remote.title != loc.t("onboarding.login.recoverOfferTitle"))
        let local = promptCopy(PromptKind(type: "registry_unreachable", local: true), loc: loc)
        #expect(local.title == loc.t("onboarding.common.networkTitle"))
        #expect(local.message == loc.t("onboarding.common.networkBody"))
        #expect(local.confirmLabel == loc.t("common.tryAgain"))
        #expect(FlowSheet.badge(for: "registry_unreachable") == .warning)
        #expect(FlowSheet.badge(for: "recover_offer") == .info)
        // The wire's `local` reaches the kind.
        #expect(PromptKind(json: ["type": "registry_unreachable", "local": true]).local)
        #expect(!PromptKind(json: ["type": "registry_unreachable", "local": false]).local)
        for key in [
            "onboarding.login.registryUnreachableTitle", "onboarding.login.registryUnreachableBody",
            "onboarding.common.networkTitle", "onboarding.common.networkBody",
        ] {
            #expect(loc.t(key) != key, "\(key) resolves")
            #expect(I18nKeys.all.contains(key))
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
    @Test func everySemanticVariantHasCopy() throws {
        for status in StatusKey.allCases {
            #expect(statusKeyToI18n(status).hasPrefix("onboarding."))
        }
        for label in SubmitLabel.allCases {
            #expect(submitLabelToI18n(label).hasPrefix("onboarding."))
        }
        // Both namespaces are corpus paths, which is what this checks.
        let namespaces = ["onboarding.", "componentsUi.signing."]
        let corpus = { (key: String) in namespaces.contains { key.hasPrefix($0) } }
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        for method in KeyMethod.allCases {
            #expect(corpus(providerLineFor(method)))
            // The core's words, resolved: never a bare key, never empty.
            for chooser in [KeyChooser.create, .signIn] {
                for unlock in ["face_id", "touch_id", "other"] {
                    let copy = methodCopy(method, chooser: chooser, loc: loc, unlock: unlock)
                    for line in [copy.title, copy.body] {
                        #expect(!line.isEmpty, "\(method) \(chooser) \(unlock)")
                        #expect(!corpus(line), "\(method) \(chooser) \(unlock) drew its key: \(line)")
                    }
                }
            }
        }
        // Spec 102: three places — the trusted page is a venue, with its own
        // words ("Use a trusted signing page", D6), never a fourth row here.
        #expect(KeyMethod.allCases.count == 3)
        let entry = try #require(venueWords(row: "signing_page"))
        #expect(entry.titleKey == "onboarding.create.signingPageTitle")
        #expect(!corpus(loc.t(entry.titleKey)))
        // A shell built before D6 asks by the old name and gets the same words.
        #expect(venueWords(row: "own_page") == entry)
    }

    /// 087 F01: 这台设备 names what unlocks a passkey on THIS device — an
    /// iPhone 11 read "Touch ID 或 Windows Hello". Face ID and Touch ID are
    /// product names, drawn as they are; a device with neither reads the
    /// corpus's family line, never another platform's product.
    @Test func thisDeviceNamesItsOwnAuthenticator() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        for chooser in [KeyChooser.create, .signIn] {
            #expect(methodCopy(.platform, chooser: chooser, loc: loc, unlock: "face_id").body == "Face ID")
            #expect(methodCopy(.platform, chooser: chooser, loc: loc, unlock: "touch_id").body == "Touch ID")
            let other = methodCopy(.platform, chooser: chooser, loc: loc, unlock: "other").body
            #expect(other == loc.t("onboarding.create.methodPlatformBody"))
            #expect(!other.contains("Windows Hello"))
        }
        #expect(["face_id", "touch_id", "other"].contains(ThisDevice.unlock))
    }

    /// 087 F02: the sign-in sheet's 手机或平板 scans — it never says create;
    /// the create picker keeps "create it on a nearby device".
    @Test func theSignInPhoneRowNeverSaysCreate() {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(methodCopy(.hybrid, chooser: .signIn, loc: loc).body == loc.t("explore.scan"))
        #expect(methodCopy(.hybrid, chooser: .create, loc: loc).body == loc.t("onboarding.create.methodHybridBody"))
        #expect(!methodCopy(.hybrid, chooser: .signIn, loc: loc).body.contains("创建"))
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
