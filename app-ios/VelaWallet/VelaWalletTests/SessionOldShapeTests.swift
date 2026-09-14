//
//  SessionOldShapeTests.swift
//  VelaWalletTests
//
//  A wallet written by a RETIRED client still opens (contract 048, spec 057).
//
//  The Expo tree wrote `vela.accounts` in camelCase — `publicKeyHex`,
//  `createdAt`, `credentialId`. The core reads both spellings through serde
//  aliases and writes snake_case back. That is on `main`; what this file pins
//  is the iOS half: that such a record RESTORES, and that a record which cannot
//  be read at all is not mistaken for no record.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct SessionOldShapeTests {

    private let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    /// The Expo-era shape, verbatim from the contract.
    private var oldShapeRecord: String {
        """
        [{"id":"cred-1","name":"Ann","address":"\(address)",
          "publicKeyHex":"04ab","createdAt":"2026-08-25T10:00:00.000Z",
          "keys":[{"credentialId":"cred-1","publicKeyHex":"04ab","name":"Ann"}]}]
        """
    }

    private func store(_ accounts: String?) -> (AccountStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: "vela.tests.session.\(UUID().uuidString)")!
        if let accounts { defaults.set(accounts, forKey: "vela.accounts") }
        return (AccountStore(defaults: defaults), defaults)
    }

    /// **The claim.** A record written by the retired client is read, and the
    /// account it names comes back with its address intact.
    @Test func anExpoEraRecordStillOpensTheWallet() async {
        let (accounts, _) = store(oldShapeRecord)
        let loaded = await accounts.loadAccounts()
        #expect(loaded.count == 1)
        #expect(loaded.first?["address"] as? String == address)
        // The camelCase fields survive the read: the CORE maps them, and a
        // shell that renamed them here would be a second, disagreeing reader.
        #expect(loaded.first?["publicKeyHex"] as? String == "04ab")
        #expect(await !accounts.lastReadFailed)
    }

    /// The same record WITHOUT `keys` — the older shape still, where the
    /// account's own id and key are the one key it has.
    @Test func aRecordWithNoKeysArrayIsStillAnAccount() async {
        let (accounts, _) = store("""
        [{"id":"cred-1","name":"Ann","address":"\(address)",
          "publicKeyHex":"04ab","createdAt":"2026-08-25T10:00:00.000Z"}]
        """)
        let loaded = await accounts.loadAccounts()
        #expect(loaded.count == 1)
        #expect(loaded.first?["keys"] == nil, "absent, and the core fills it")
    }

    /// **"Cannot be read" is not "there is none."** ANDROID-8, 2026-09-13: a
    /// test device's own account record went missing, and the app said the
    /// wallet did not exist. One sends somebody to create a wallet they already
    /// have; the other is a fault they can act on.
    @Test func anUnreadableStoreIsNotAnEmptyOne() async {
        let (torn, _) = store("[{\"id\":\"cred-1\",")
        #expect(await torn.loadAccounts().isEmpty)
        #expect(await torn.lastReadFailed, "a torn record must announce itself")

        let (absent, _) = store(nil)
        #expect(await absent.loadAccounts().isEmpty)
        #expect(await !absent.lastReadFailed, "a device that never held a wallet is not a fault")
    }

    /// A refused answer becomes the machine's OWN failure, exactly once
    /// (contract 048 §2) — so the session leaves `restoring` instead of sitting
    /// there, which a person sees as a wallet that will not open.
    @Test func aRefusedAnswerEndsTheWait() throws {
        let core = SessionCore()
        let started = try core.dispatch(eventJson: CoreJSON.string(["type": "boot"]))
        let effects = try CoreJSON.object(started)["effects"] as? [[String: Any]] ?? []
        let id = try #require((effects.first?["id"] as? NSNumber)?.uint64Value,
                              "the session asks for something on start")

        let answered = try core.resolveEffect(
            effectId: id,
            resultJson: CoreJSON.string(["type": "accounts_unavailable"])
        )
        let view = try CoreJSON.decode(
            SessionView.self, from: try CoreJSON.object(answered)["view"] as? [String: Any] ?? [:]
        )
        // Whatever the route is, it is no longer "still reading".
        #expect(!view.loading, "the session stayed in its loading state after a refusal")
    }
}
