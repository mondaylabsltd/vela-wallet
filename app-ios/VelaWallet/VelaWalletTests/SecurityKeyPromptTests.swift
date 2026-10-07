//
//  SecurityKeyPromptTests.swift
//  VelaWalletTests
//
//  Issue #450: on an iPad, choosing the security key with none plugged in
//  said "Not Supported — Biometric authentication is not available on this
//  device". A missing key now waits for one (the insert sheet); a key route
//  this device cannot use is flagged by the core (`security_key`) and said as
//  the key's problem.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct SecurityKeyPromptTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    @Test func aSecurityKeyThatCannotRunIsAboutTheKey() {
        for type in ["not_supported_login", "not_supported_create"] {
            let copy = promptCopy(PromptKind(type: type, securityKey: true), loc: loc)
            #expect(copy.title == loc.t(I18nKeys.Flow.keyUnavailableTitle))
            #expect(copy.message == loc.t(I18nKeys.Flow.keyUnavailableBody))
            #expect(!copy.message.contains("Biometric"))
        }
        let device = promptCopy(PromptKind(type: "not_supported_login"), loc: loc)
        #expect(device.message == loc.t(I18nKeys.Login.alertNotSupportedBody))
    }

    @Test func theFlagIsReadFromTheCoresJson() {
        #expect(PromptKind(json: ["type": "not_supported_login", "security_key": true]).securityKey)
        #expect(!PromptKind(json: ["type": "not_supported_login"]).securityKey)
    }

    /// The insert sheet's words fit an iPad too: "this device", not "this phone".
    @Test func theInsertSheetNamesTheDeviceNotAPhone() {
        #expect(!loc.t(I18nKeys.Flow.insertKeyBody).contains("phone"))
        #expect(loc.t(I18nKeys.Flow.insertKeyTitle) != I18nKeys.Flow.insertKeyTitle)
    }
}
