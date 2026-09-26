//
//  EraseDeviceTests.swift
//  VelaWalletTests
//
//  抹除此设备, as a scan (spec 072 T032).
//
//  The old erase walked a hand-kept list of eighteen keys, so everything not on
//  it survived the one action that promises nothing does: the name cache
//  (`recipient_id:`), every dApp grant (`vela.perm.*`), the contacts' siblings,
//  the signing preferences, and every key added since the list was written.
//  Each test here seeds a store the way this app (and the other shells) write
//  it, erases, and looks at what is left.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct EraseDeviceTests {

    private func fresh() -> (UserDefaults, VelaStore) {
        let defaults = UserDefaults(suiteName: "vela.tests.erase.\(UUID().uuidString)")!
        return (defaults, VelaStore(defaults: defaults))
    }

    /// Every key this wallet writes — the list the old erase kept, and the
    /// ones it missed.
    private func seedEverything(_ store: VelaStore) {
        store.writeList(VelaStore.Key.accounts, [["id": "a", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]])
        store.writeString(VelaStore.Key.activeIndex, "0")
        store.writeList(VelaStore.Key.contacts, [["address": "0x1"]])
        store.writeList(VelaStore.Key.contactGroups, [["id": "g"]])
        store.writeList(VelaStore.Key.transactionHistory, [["id": "t"]])
        store.writeString(VelaStore.Key.theme, "dark")
        store.writeString(VelaStore.Key.balanceCache, #"{"0x1":{"usd":1}}"#)
        // Missed by the hand-kept list:
        store.writeString("vela.perm.https://app.uniswap.org", #"{"accounts":["0x1"]}"#)
        store.writeString("vela.chain.https://app.uniswap.org", "100")
        store.writeString("recipient_id:0xabc", #"{"name":"alice"}"#)
        store.writeString("vela.receiveWarned.0x1", "1")
        store.writeString(VelaStore.Key.trustedSignerUrl, "https://my.signer/")
        store.writeString(VelaStore.Key.feeTier, "slow")
        store.writeString("vela.balanceHidden", "true")
        store.writeString("vela.some-key-added-next-year", "x")
        // Kept on purpose, by the core's rule:
        store.writeList(VelaStore.Key.pendingUploads, [["publicKey": "04ab"]])
    }

    /// Everything of ours goes — including every key the old list never
    /// named — and the answer is "nothing survived".
    @Test func theEraseIsAScanNotAList() async {
        let (defaults, store) = fresh()
        seedEverything(store)

        let survivors = await DeviceStorage.erase(store)

        #expect(survivors.isEmpty, "survived: \(survivors)")
        for key in [
            "vela.perm.https://app.uniswap.org", "vela.chain.https://app.uniswap.org",
            "recipient_id:0xabc", "vela.receiveWarned.0x1", VelaStore.Key.trustedSignerUrl,
            VelaStore.Key.feeTier, "vela.balanceHidden", "vela.some-key-added-next-year",
            VelaStore.Key.accounts, VelaStore.Key.activeIndex, VelaStore.Key.theme,
        ] {
            #expect(defaults.object(forKey: key) == nil, "\(key) survived the erase")
        }
    }

    /// The one record the erase keeps: a passkey public key the index never
    /// confirmed. Deleting it would make that key unfindable on every device.
    /// And keys that are not ours are not ours to delete.
    @Test func thePendingUploadAndOtherAppsKeysAreKept() async {
        let (defaults, store) = fresh()
        seedEverything(store)
        defaults.set("keep me", forKey: "com.apple.something")

        _ = await DeviceStorage.erase(store)

        #expect(store.readList(VelaStore.Key.pendingUploads).count == 1)
        #expect(defaults.string(forKey: "com.apple.something") == "keep me")
    }

    /// A store that will not let go of one key — what a failed write looks
    /// like from the erase's side.
    private final class StubbornDefaults: UserDefaults {
        let stubborn: String
        init?(suiteName: String, stubborn: String) {
            self.stubborn = stubborn
            super.init(suiteName: suiteName)
        }
        override func removeObject(forKey defaultName: String) {
            guard defaultName != stubborn else { return }
            super.removeObject(forKey: defaultName)
        }
    }

    /// An erase that could not delete something SAYS so — it never answers
    /// "done" over a key that is still there.
    @Test func whatSurvivesIsReportedNotSwallowed() async throws {
        let defaults = try #require(StubbornDefaults(
            suiteName: "vela.tests.erase.\(UUID().uuidString)", stubborn: "vela.contacts"
        ))
        let store = VelaStore(defaults: defaults)
        seedEverything(store)

        let survivors = await DeviceStorage.erase(store)

        #expect(survivors == ["vela.contacts"])
    }

    /// After an erase the store is a first run: a session booted over it has
    /// no wallet and goes to onboarding — with no sign-out question to answer.
    @Test func anErasedStoreBootsToTheFirstRun() async {
        let (defaults, store) = fresh()
        seedEverything(store)
        #expect(await DeviceStorage.erase(store).isEmpty)

        let session = SessionController(store: AccountStore(defaults: defaults))
        session.boot()
        let deadline = Date().addingTimeInterval(5)
        while Date() < deadline, session.view.loading {
            try? await Task.sleep(nanoseconds: 20_000_000)
        }
        #expect(session.view.allowedRoute == .onboarding)
        #expect(!session.view.hasWallet)
        #expect(session.view.signOut == nil)
    }

    /// The erase sheet, when something survived: its own callout says the
    /// erase did not finish, and the person is still signed in.
    @Test func aFailedEraseKeepsItsSheetAndSaysSo() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let base = SettingsFixtures.build(.st16, loc: loc)

        let failed = SettingsLive.withEraseFailure(["vela.contacts"], on: base, loc: loc)
        // The sentence, and WHICH key stayed — the desktop's callout says the
        // same, because a person can act on a name and not on "something".
        #expect(failed.eraseSheet.callout?.text.hasPrefix(loc.t(I18nKeys.SettingsUi.eraseFailed)) == true)
        #expect(failed.eraseSheet.callout?.text.contains("vela.contacts") == true)
        #expect(failed.eraseSheet.callout?.tone == .danger)

        // Nothing survived: no callout of ours, whether that is `nil` or empty.
        #expect(SettingsLive.withEraseFailure([], on: base, loc: loc)
            .eraseSheet.callout?.text == base.eraseSheet.callout?.text)
        let fine = SettingsLive.withEraseFailure(nil, on: base, loc: loc)
        #expect(fine.eraseSheet.callout?.text == base.eraseSheet.callout?.text)
    }
}
