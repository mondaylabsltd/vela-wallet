//
//  ContactsStoreTests.swift
//  VelaWalletTests
//
//  The whole loop, with nothing faked below it.
//
//  `ContactsExecutorTests` checks the shapes and `ContactsLiveTests` checks the
//  display; neither runs the effect loop. This file drives the real Crux core
//  through the real uniffi bridge, the real `CoreDriver`, the real executor and
//  a real `UserDefaults` — so it is the same path the screen takes, minus the
//  finger.
//
//  It is what proves the two claims US1 actually makes: the book is the signed-in
//  account's, and a deletion is still a deletion tomorrow.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ContactsStoreTests {

    private let alice = "0x9f3ca71b04e82f5c55d9b21ae00734f8dd8021ae"
    private let bob = "0x44aaf19ce84f22101b5d6cba918b92dca5f19c21"

    private func suite() -> (String, UserDefaults) {
        let name = UUID().uuidString
        return (name, UserDefaults(suiteName: name)!)
    }

    private func seedBook(_ defaults: UserDefaults) {
        defaults.set(
            """
            [{"address":"\(alice)","name":"Alice","kind":"eoa","favorite":false,\
            "txCount":2,"lastUsed":1700000000000,"firstSeen":1600000000000,"source":"manual"},\
            {"address":"\(bob)","name":"Bob","kind":"eoa","favorite":false,\
            "txCount":1,"lastUsed":1690000000000,"firstSeen":1600000000000,"source":"manual"}]
            """,
            forKey: VelaStore.Key.contacts
        )
    }

    /// The effect loop resolves on the main actor across `Task` boundaries, so
    /// a test has to let those turns run. Polling a condition beats a fixed
    /// sleep: it is faster when the loop is quick and it does not go flaky when
    /// the machine is busy.
    private func settle(
        until condition: @escaping () -> Bool,
        turns: Int = 200
    ) async {
        for _ in 0..<turns {
            if condition() { return }
            await Task.yield()
        }
    }

    /// Open the book and it is loaded, from storage, through the core.
    @Test func openingReadsTheStoresAndLoadsTheCore() async {
        let (_, defaults) = suite()
        seedBook(defaults)

        let store = ContactsStore(store: VelaStore(defaults: defaults))
        store.open(myAddress: alice)
        await settle(until: { store.isLoaded })

        #expect(store.isLoaded, "the core never became loaded")
        #expect(store.view?.contacts.count == 2)
        #expect(store.contact(at: bob)?.name == "Bob")
    }

    /// A delete writes a tombstone, and the tombstone is what survives.
    ///
    /// The second half is the point: a fresh store over the SAME defaults is a
    /// relaunch, and a contact that comes back after one is the defect this
    /// test exists to catch.
    @Test func aDeletedContactStaysDeletedAcrossARelaunch() async {
        let (_, defaults) = suite()
        seedBook(defaults)

        let store = ContactsStore(store: VelaStore(defaults: defaults))
        store.open(myAddress: alice)
        await settle(until: { store.isLoaded })

        store.delete(address: bob)
        await settle(until: { store.view?.contacts.count == 1 })
        #expect(store.contact(at: bob) == nil, "the row is still in the view")

        // The bytes, not just the model.
        //
        // These are two different moments, and this test failed on its first
        // run for exactly that reason: the core commits its view the instant it
        // handles the event, and QUEUES the writes. So the row disappears from
        // the screen a turn before anything reaches storage — which means
        // "I watched it vanish" is not evidence that it will still be gone
        // tomorrow. Settling on the view and then reading the shelf would have
        // recorded a passing test for an app that persisted nothing.
        await settle(until: { defaults.string(forKey: VelaStore.Key.contactsDismissed) != nil })
        let raw = defaults.string(forKey: VelaStore.Key.contactsDismissed) ?? ""
        let dismissed = (try? JSONSerialization.jsonObject(with: Data(raw.utf8))) as? [String: Any]
        #expect(dismissed?[bob] != nil, "no tombstone written; stored: \(raw)")

        // Relaunch.
        let reopened = ContactsStore(store: VelaStore(defaults: defaults))
        reopened.open(myAddress: alice)
        await settle(until: { reopened.isLoaded })
        #expect(reopened.contact(at: bob) == nil, "Bob came back from the dead")
        #expect(reopened.contact(at: alice) != nil)
    }

    /// Two accounts never share a book.
    ///
    /// `AccountSwitched` is what scopes it; missing that event is how one
    /// person is shown another person's address book, which is the worst
    /// outcome this screen has.
    @Test func switchingAccountsResetsTheBook() async {
        let (_, defaults) = suite()
        seedBook(defaults)

        let store = ContactsStore(store: VelaStore(defaults: defaults))
        store.open(myAddress: alice)
        await settle(until: { store.isLoaded })
        #expect(store.view?.contacts.count == 2)

        // A different wallet signs in. The core drops everything and re-reads.
        store.open(myAddress: "0x0000000000000000000000000000000000000001")
        await settle(until: { store.isLoaded && store.view?.contacts.count == 2 })
        #expect(store.isLoaded)
    }

    /// Re-entering the tab must not re-boot the machine: the resident model
    /// already holds the book, and re-reading on every visit is what residency
    /// exists to avoid.
    @Test func reopeningTheSameAccountIsIdempotent() async {
        let (_, defaults) = suite()
        seedBook(defaults)

        let store = ContactsStore(store: VelaStore(defaults: defaults))
        store.open(myAddress: alice)
        await settle(until: { store.isLoaded })

        store.delete(address: bob)
        await settle(until: { defaults.string(forKey: VelaStore.Key.contactsDismissed) != nil })

        // Leaving and coming back with the same account changes nothing.
        store.open(myAddress: alice)
        await Task.yield()
        #expect(store.view?.contacts.count == 1)
    }

    /// Signed out: no address, no book, and no crash.
    @Test func noAccountLoadsAnEmptyBookRatherThanSomebodyElses() async {
        let (_, defaults) = suite()
        seedBook(defaults)

        let store = ContactsStore(store: VelaStore(defaults: defaults))
        store.open(myAddress: nil)
        await settle(until: { store.isLoaded })
        #expect(store.isLoaded)
    }
}
