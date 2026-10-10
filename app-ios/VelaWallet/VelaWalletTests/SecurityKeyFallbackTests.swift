//
//  SecurityKeyFallbackTests.swift
//  VelaWalletTests
//
//  PR 3: a security key the app's own USB route cannot reach goes through
//  Apple's security-key sheet.
//
//  The app-owned CCID route (`SmartCardCtapCeremony`) reaches a USB-C key that
//  offers FIDO over its smart-card interface — and nothing else. A Lightning
//  iPhone, an NFC key and a key on firmware below 5.8 used to wait on "Insert
//  your security key" for ever. Now:
//
//  - the insert sheet ends three ways — a key answered, it was closed, or the
//    person asked for Apple's sheet;
//  - a Lightning iPhone is not made to wait at all;
//  - a ceremony the card never answered falls back; one it answered and then
//    refused is the key's own answer;
//  - the hand-over issues the security-key request alone.
//
//  No key and no radio are needed: these are the rules around the ceremony,
//  not the ceremony.
//

import AuthenticationServices
import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// The insert prompt, answered at once with one outcome, and counted.
private final class CountingPrompts: SmartCardCtapCeremony.Prompts, @unchecked Sendable {
    private let lock = NSLock()
    private var count = 0
    let outcome: SmartCardCtapCeremony.KeyInsertion

    init(_ outcome: SmartCardCtapCeremony.KeyInsertion) { self.outcome = outcome }

    var asked: Int {
        lock.lock()
        defer { lock.unlock() }
        return count
    }

    func askPin(product: String, retries: Int, isRetry: Bool) -> String? { nil }
    func askWhichWallet(_ choices: [CtapCredentialChoice]) -> Int? { nil }
    func touchWaiting(kind: String?, product: String) {}
    func awaitKeyInsertion(
        probe: @escaping @MainActor () async -> Bool
    ) async -> SmartCardCtapCeremony.KeyInsertion {
        lock.lock()
        count += 1
        lock.unlock()
        return outcome
    }
}

@MainActor
struct SecurityKeyFallbackTests {

    private func makeModel() -> OnboardingModel {
        let defaults = UserDefaults(suiteName: "vela.tests.securityKeyFallback.\(UUID().uuidString)")!
        let accounts = AccountStore(defaults: defaults)
        return OnboardingModel(session: SessionController(store: accounts), store: accounts)
    }

    // MARK: - The insert sheet

    /// "Use Apple's security-key sheet" answers the waiting ceremony with its
    /// own outcome — not a cancel, which the core would take as the person
    /// giving up.
    @Test func askingForApplesSheetIsItsOwnAnswer() async {
        let model = makeModel()
        async let outcome = model.awaitKeyInsertion(probe: { false })
        await Wait.until { model.pendingInsertKey != nil }
        #expect(model.onboardingSheetPresented, "the insert sheet is not up")
        model.answerInsertKey(.useSystemSheet)
        #expect(await outcome == .useSystemSheet)
        #expect(model.pendingInsertKey == nil)
        // The shared sheet stays up under Apple's: whatever that ceremony
        // ends in swaps this sheet's content, and is never presented into
        // its dismissal.
        #expect(model.systemSheetHold)
        #expect(model.onboardingSheetPresented, "the shared sheet dropped between the two")
    }

    /// Closing the sheet — the button, a swipe, a tap outside — is a cancel.
    @Test func closingTheInsertSheetIsACancel() async {
        let model = makeModel()
        async let outcome = model.awaitKeyInsertion(probe: { false })
        await Wait.until { model.pendingInsertKey != nil }
        model.dismissOnboardingSheet()
        #expect(await outcome == .cancelled)
        #expect(!model.systemSheetHold, "a cancel holds nothing up")
        #expect(!model.onboardingSheetPresented)
    }

    /// A key that answers continues by itself: plugging it in is the gesture.
    @Test func aKeyThatAnswersContinuesByItself() async {
        let model = makeModel()
        #expect(await model.awaitKeyInsertion(probe: { true }) == .inserted)
        #expect(model.pendingInsertKey == nil)
    }

    /// The option and its hint are in the corpus, in the reader's words.
    @Test func theOptionIsWordedByTheCorpus() {
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(en.t(I18nKeys.Flow.insertKeyAppleSheet) == "Use Apple's security-key sheet")
        #expect(en.t(I18nKeys.Flow.insertKeyAppleSheetHint) == "For NFC, Lightning or older keys")
        #expect(zh.t(I18nKeys.Flow.insertKeyAppleSheet) == "改用 Apple 的安全密钥面板")
        #expect(zh.t(I18nKeys.Flow.insertKeyAppleSheetHint) == "适用于 NFC、Lightning 或较旧的密钥")
    }

    /// With no key anywhere (a simulator has none), the ceremony asks ONCE.
    /// "Use Apple's security-key sheet" hands it over — and the ceremonies
    /// that follow in the same flow (a new key's proof of signing, a
    /// recovery's second signature) go the same way without asking again.
    /// A method picked afresh puts the question back.
    @Test func theChoiceOfApplesSheetCarriesThroughTheFlow() async {
        let prompts = CountingPrompts(.useSystemSheet)
        let ceremony = SmartCardCtapCeremony(prompts: prompts)
        func handsOver() async -> Bool {
            do {
                _ = try await ceremony.assert(challenge: Data(repeating: 7, count: 32), credentialIdHex: nil)
                return false
            } catch is SmartCardCtapCeremony.SystemSheetFallback {
                return true
            } catch {
                return false
            }
        }
        #expect(await handsOver(), "the option did not hand the ceremony over")
        let first = prompts.asked
        // Asked once here; not at all where nothing would ever answer (a
        // Lightning model, or no smart-card service).
        #expect(first <= 1)
        #expect(await handsOver(), "the ceremony that follows did not go the same way")
        #expect(prompts.asked == first, "the person was asked a second time in one flow")

        ceremony.forgetSystemSheetChoice()
        #expect(await handsOver())
        #expect(prompts.asked == first * 2, "a method picked afresh did not put the question back")
    }

    /// Closing the insert prompt is a cancel, as the core hears it — never a
    /// hand-over the person did not ask for.
    @Test func closingTheInsertPromptCancelsTheCeremony() async {
        let prompts = CountingPrompts(.cancelled)
        let ceremony = SmartCardCtapCeremony(prompts: prompts)
        do {
            _ = try await ceremony.assert(challenge: Data(repeating: 7, count: 32), credentialIdHex: nil)
            Issue.record("a ceremony with no key returned an assertion")
        } catch let failure as PasskeyFailure {
            #expect(failure.kind == .cancelled)
            #expect(prompts.asked == 1)
        } catch is SmartCardCtapCeremony.SystemSheetFallback {
            // Where nothing would ever answer, the insert prompt is never
            // shown, so there is nothing to close.
            #expect(prompts.asked == 0)
        } catch {
            Issue.record("an unexpected failure: \(error)")
        }
    }

    // MARK: - A Lightning iPhone is not made to wait

    /// Every iPhone before the 15 has a Lightning port; every one since has
    /// USB-C. An iPad is not guessed (its line mixes both), nor is anything
    /// that does not read as an iPhone.
    @Test func whichPhonesHaveALightningPort() {
        let lightning = [
            "iPhone12,1",  // iPhone 11
            "iPhone13,2",  // iPhone 12
            "iPhone14,5",  // iPhone 13
            "iPhone14,6",  // iPhone SE (3rd generation)
            "iPhone14,7",  // iPhone 14
            "iPhone15,2",  // iPhone 14 Pro
            "iPhone15,3",  // iPhone 14 Pro Max
        ]
        let usbC = [
            "iPhone15,4",  // iPhone 15
            "iPhone15,5",  // iPhone 15 Plus
            "iPhone16,1",  // iPhone 15 Pro
            "iPhone16,2",  // iPhone 15 Pro Max
            "iPhone17,3",  // iPhone 16
            "iPhone17,5",  // iPhone 16e
            "iPhone18,3",  // iPhone 17
        ]
        for model in lightning { #expect(KeyPort.lightningPhone(model: model), "\(model)") }
        for model in usbC { #expect(!KeyPort.lightningPhone(model: model), "\(model)") }
        for model in ["iPad12,1", "iPad13,18", "arm64", "", "iPhone", "iPhoneX,Y"] {
            #expect(!KeyPort.lightningPhone(model: model), "\(model) was guessed")
        }
    }

    // MARK: - A card that never answers

    /// ISO 7816-4: a reply ending `90 00` completed its command.
    @Test func aReplyEndingNinetyZeroZeroCompletedItsCommand() {
        #expect(SmartCardCtapPort.completes(Data([0x46, 0x49, 0x44, 0x4f, 0x90, 0x00])))
        #expect(SmartCardCtapPort.completes(Data([0x90, 0x00])))
        // No such applet, wrong class, more to fetch, and nothing at all.
        #expect(!SmartCardCtapPort.completes(Data([0x6a, 0x82])))
        #expect(!SmartCardCtapPort.completes(Data([0x6e, 0x00])))
        #expect(!SmartCardCtapPort.completes(Data([0x61, 0x10])))
        #expect(!SmartCardCtapPort.completes(Data([0x90])))
        #expect(!SmartCardCtapPort.completes(Data()))
    }

    /// A ceremony the card never answered goes to Apple's sheet — the key
    /// does not speak FIDO on this interface. One it answered and then
    /// refused is the key's own answer, reported as it is. A cancel is always
    /// a cancel.
    @Test func onlyACardThatNeverAnsweredFallsBack() {
        let select = CtapError.Other(detail: "SELECT FIDO applet failed: SW=6A82")
        #expect(SmartCardCtapCeremony.fallsBackToSystem(select, answered: false))
        #expect(SmartCardCtapCeremony.fallsBackToSystem(.NotSupported(detail: "no reply"), answered: false))

        #expect(!SmartCardCtapCeremony.fallsBackToSystem(.Other(detail: "The PIN was not accepted."), answered: true))
        #expect(!SmartCardCtapCeremony.fallsBackToSystem(.NotDiscoverable(detail: ""), answered: true))

        #expect(!SmartCardCtapCeremony.fallsBackToSystem(.Cancelled, answered: false))
        #expect(!SmartCardCtapCeremony.fallsBackToSystem(.Cancelled, answered: true))
    }

    // MARK: - The hand-over

    /// The method still decides: a security-key assertion takes the app-owned
    /// path FIRST. The hand-over is that path's own, when it cannot run.
    @Test func theAppOwnedPathStaysFirst() {
        #expect(PasskeyExecutor.assertionPath(transports: "usb,nfc", method: .securityKey) == .securityKey)
        #expect(PasskeyExecutor.assertionPath(transports: "", method: .securityKey) == .securityKey)
    }

    /// Handed over, the system sheet is asked for the security key alone —
    /// the person said which object they are holding — pinned to the same
    /// credential, with user verification, over every cable the system knows
    /// (the reason for the hand-over is that USB did not reach it).
    @Test func theHandOverAsksForTheSecurityKeyAlone() throws {
        let credential = "b2b2b2b2"
        let challenge = Data(repeating: 7, count: 32)
        let handed = try PasskeyExecutor().systemRequests(
            challenge: challenge, credentialIdHex: credential, transports: "usb", securityKeyOnly: true
        )
        #expect(handed.count == 1)
        let request = try #require(handed.first as? ASAuthorizationSecurityKeyPublicKeyCredentialAssertionRequest)
        #expect(request.allowedCredentials.map(\.credentialID) == [Data([0xb2, 0xb2, 0xb2, 0xb2])])
        #expect(request.userVerificationPreference == .required)
        #expect(request.allowedCredentials.first?.transports
                == ASAuthorizationSecurityKeyPublicKeyCredentialDescriptor.Transport.allSupported)

        // …even for a route whose hints name no removable cable at all.
        let phoneOnly = try PasskeyExecutor().systemRequests(
            challenge: challenge, credentialIdHex: nil, transports: "internal", securityKeyOnly: true
        )
        #expect(phoneOnly.count == 1)
        #expect(phoneOnly.first is ASAuthorizationSecurityKeyPublicKeyCredentialAssertionRequest)

        // Not handed over, nothing changed: the platform request first, the
        // security key beside it.
        let system = try PasskeyExecutor().systemRequests(
            challenge: challenge, credentialIdHex: credential, transports: "usb,nfc"
        )
        #expect(system.count == 2)
        #expect(system.first is ASAuthorizationPlatformPublicKeyCredentialAssertionRequest)
    }
}
