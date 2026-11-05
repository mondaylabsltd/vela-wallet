//
//  R13FollowupsTests.swift
//  VelaWalletTests
//
//  Follow-ups after Round 13 (docs/multisig-machines.md §18):
//
//  - item 1: a proposal link whose read failed — not in time (`failed`), its
//    network silent (`unreadable`), the multisig service silent (`notFound`)
//    — is said in the core's words for that read and offers Try again
//    whenever the core's `retry` says so; `notInMailbox` (an answer) offers
//    none, and nothing says "it hasn't been shared" for a read that failed.
//  - item 2: a stored account record this device cannot read is said (the
//    welcome's notice, the switcher's line) and kept byte for byte on every
//    write — the core's `stored_accounts` rule; signing in is the way back.
//  - item 3: an unread history is said once, on Activity's empty state with
//    Try again; the card does not say it again.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct R13FollowupsTests {

    private let loc = Loc(overrideTag: "en")
    private var words: MsWords { MsWords(loc: loc, hub: nil, localNames: [:]) }

    private func linked(_ json: String) throws -> MsLinkedView {
        try JSONDecoder().decode(MsLinkedView.self, from: Data(json.utf8))
    }

    // MARK: - item 1: every link the core says is retryable offers Try again

    @Test func everyLinkTheCoreSaysRetryableOffersTryAgain() throws {
        for (stand, leaf) in [("failed", "linkChain"), ("unreadable", "linkChain"), ("notFound", "linkService")] {
            let view = try linked(
                #"{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"\#(stand)","#
                    + #""reason_key":"home.multisig.proposal.unread.\#(leaf)","retry":true}"#
            )
            #expect(view.offersTryAgain, "\(stand)")
            let said = view.failure(words)
            #expect(said == words.t("proposal.unread.\(leaf)", ["network": words.network(100)]), "\(stand)")
            #expect(said != words.t("proposal.notFound"), "\(stand)")
        }
        let answered = try linked(
            #"{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"notInMailbox","reason_key":null,"retry":false}"#
        )
        #expect(!answered.offersTryAgain)
        #expect(answered.failure(words) == nil)
    }

    // MARK: - item 2: an unreadable stored account list is said and kept

    private static let good = #"{"id":"cred-1","name":"Ann","address":"0x88cCA0EeDbF2C4426110bbFc998F048689266894","public_key_hex":"04ab","created_at_iso":"2026-08-25T10:00:00.000Z","keys":[{"credential_id":"cred-1","public_key_hex":"04ab","name":"Ann"}]}"#
    private static let other: [String: Any] = [
        "id": "cred-2", "name": "Bo", "address": "0x1F9840a85d5aF5bf1D1762F925BDADdC4201F984",
        "public_key_hex": "04cd", "created_at_iso": "2026-08-26T10:00:00.000Z",
        "keys": [["credential_id": "cred-2", "public_key_hex": "04cd", "name": "Bo"]],
    ]
    /// Its key's id is a number: a record this build cannot read. Its `\/`
    /// would not survive a JSONSerialization round trip.
    private static let bad = #"{"id":"x","address":"0x00000000000000000000000000000000000000Cc","keys":[{"credential_id":7}],"note":"a\/b"}"#

    private func store(_ raw: String?) -> (AccountStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: "vela.tests.r13f.\(UUID().uuidString)")!
        if let raw { defaults.set(raw, forKey: "vela.accounts") }
        return (AccountStore(defaults: defaults), defaults)
    }

    @Test func oneRecordItCannotReadCostsOnlyItselfAndIsKeptByteForByte() async {
        let (accounts, defaults) = store("[\(Self.good),\(Self.bad)]")
        let read = await accounts.loadAccounts()
        #expect(read.map { $0["id"] as? String } == ["cred-1"])
        #expect(await accounts.unreadable == 1)
        #expect(await accounts.lastReadFailed)

        await accounts.saveAccount(Self.other)
        let saved = defaults.string(forKey: "vela.accounts") ?? ""
        #expect(saved.contains(Self.bad), "the record it cannot read, as stored: \(saved)")
        #expect(await accounts.loadAccounts().count == 2)

        await accounts.removeAccount(address: "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984")
        let removed = defaults.string(forKey: "vela.accounts") ?? ""
        #expect(removed.contains(Self.bad) && !removed.contains("cred-2"), "never shorter by what could not be read")
        await accounts.removeAccount(id: "cred-1")
        #expect(defaults.string(forKey: "vela.accounts") == "[\(Self.bad)]")
    }

    @Test func aStoredValueThatIsNoListIsKeptWhenSigningIn() async throws {
        let (accounts, defaults) = store(#"{"a":1}"#)
        #expect(await accounts.loadAccounts().isEmpty)
        #expect(await accounts.unreadable == 1)
        // The way back: the passkey's sign-in writes its record — beside it.
        let good = try #require(try JSONSerialization.jsonObject(with: Data(Self.good.utf8)) as? [String: Any])
        await accounts.saveAccount(good)
        let saved = defaults.string(forKey: "vela.accounts") ?? ""
        #expect(saved.hasPrefix(#"[{"a":1},"#), "\(saved)")
        #expect(await accounts.loadAccounts().count == 1)
        #expect(await accounts.unreadable == 1)
    }

    @Test func theWelcomeAndTheSwitcherSayItInDeviceNeutralWords() throws {
        #expect(StoredAccountsNotice.shown(sessionUnreadable: true, storedUnreadable: 0))
        #expect(StoredAccountsNotice.shown(sessionUnreadable: false, storedUnreadable: 1))
        #expect(!StoredAccountsNotice.shown(sessionUnreadable: false, storedUnreadable: 0))
        let title = loc.t(I18nKeys.Storage.deviceUnreadableTitle)
        #expect(!title.hasPrefix("onboarding.") && !title.localizedCaseInsensitiveContains("browser"))
        #expect(!loc.t(I18nKeys.Storage.deviceUnreadableBody).localizedCaseInsensitiveContains("browser"))
        let session = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Ann", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]]],
        ])
        let sheet = { (unreadable: Int) in
            SettingsLive.withAccounts(
                session: session, balances: [], display: .live(nil, language: "en"),
                on: SettingsFixtures.build(.st1, loc: self.loc), loc: self.loc, unreadable: unreadable
            ).accountsSheet
        }
        #expect(sheet(1).unreadable == title)
        #expect(sheet(0).unreadable == nil)
    }

    // MARK: - item 3: an unread history is said once, on Activity's empty state

    @Test func anUnreadHistoryIsSaidOnceOnActivitysEmptyState() throws {
        let history = { (stand: String) in
            WalletLive.MultisigHistory(stand: stand, unreadText: "Couldn't read it", retryText: loc.t("common.tryAgain"))
        }
        let empty = WalletFixtures.buildMobileState(.h2, loc: loc)
        let rows = WalletFixtures.buildMobileState(.h1, loc: loc)
        #expect(WalletLive.historySaidInActivity(WalletLive.withMultisigHistory(empty, history("unread"))))
        #expect(!WalletLive.historySaidInActivity(WalletLive.withMultisigHistory(rows, history("unread"))))
        #expect(!WalletLive.historySaidInActivity(WalletLive.withMultisigHistory(empty, history("read"))))
        let note = try JSONDecoder().decode(MsTagged.self, from: Data(#"{"kind":"historyUnreadable","chain_ids":[5042002]}"#.utf8))
        #expect(MultisigHomeCard.cardNote(note, historySaidInActivity: true) == nil)
        #expect(MultisigHomeCard.cardNote(note, historySaidInActivity: false) == note)
        let away = try JSONDecoder().decode(MsTagged.self, from: Data(#"{"kind":"unreachable"}"#.utf8))
        #expect(MultisigHomeCard.cardNote(away, historySaidInActivity: true) == away)
    }
}
