//
//  ScannedLinkTests.swift
//  VelaWalletTests
//
//  r9-4 on iOS (real-device pass 2): a scanned multisig invite landed in
//  Send's "To" as text. A scanned multisig link is now opened through the
//  one link door; everything else is still Send's to read.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ScannedLinkTests {

    private let safe = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83"
    private let hash = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa"
    private let member = "88cca0eedbf2c4426110bbfc998f048689266894"

    @Test func aScannedInviteIsALinkNotARecipient() {
        let invite = "https://wallet.getvela.app/en/wallet#msa=100:\(safe)&a=\(member)pInterleave"
        #expect(AppLink.scannedMultisig(invite)?.absoluteString == invite)
        // With the white space a camera or a paste can carry.
        #expect(AppLink.scannedMultisig("  \(invite)\n") != nil)
    }

    @Test func aScannedProposalAndADamagedOneAreLinks() {
        #expect(AppLink.scannedMultisig("https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)") != nil)
        // Meant as an invite, and cut short: the link door says why.
        #expect(AppLink.scannedMultisig("https://wallet.getvela.app/en/wallet#msa=100:0x0cb9") != nil)
    }

    @Test func everythingElseIsSendsToRead() {
        #expect(AppLink.scannedMultisig(safe) == nil)
        #expect(AppLink.scannedMultisig("ethereum:\(safe)@100") == nil)
        #expect(AppLink.scannedMultisig("https://getvela.app/pay?to=\(safe)&chain=100") == nil)
        #expect(AppLink.scannedMultisig("velawallet://scan") == nil)
    }

    @Test func aCoreThatCannotAnswerClaimsNothing() {
        let dead: (String, String) throws -> String = { _, _ in throw NSError(domain: "dead", code: 1) }
        #expect(AppLink.scannedMultisig("https://wallet.getvela.app/en/wallet#ms=100.\(safe).\(hash)", call: dead) == nil)
    }
}
