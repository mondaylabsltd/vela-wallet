//
//  VelaStoreTests.swift
//  VelaWalletTests
//
//  The storage seam that three other clients read.
//
//  Two properties matter more than the rest, and both are silent when broken:
//  values must be JSON **text** (so the bytes are the ones web and Expo write),
//  and a record must come out of a round trip **whole** (so a multi-key account
//  cannot be quietly repaired into a different, wrong Safe).
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct VelaStoreTests {

    private func fresh() -> (VelaStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        return (VelaStore(defaults: defaults), defaults)
    }

    /// Not a plist array — the exact string `localStorage` and `AsyncStorage`
    /// hold. This is what makes "a contact written on the web reads on the
    /// phone" literally true rather than approximately true.
    @Test func valuesAreStoredAsJsonText() {
        let (store, defaults) = fresh()
        store.writeList("vela.test", [["a": 1]])
        #expect(defaults.string(forKey: "vela.test") == #"[{"a":1}]"#)
        #expect(defaults.array(forKey: "vela.test") == nil, "must not be a plist array")
    }

    @Test func corruptAndAbsentBothReadEmpty() {
        let (store, defaults) = fresh()
        #expect(store.readList("vela.missing").isEmpty)
        #expect(store.readObject("vela.missing").isEmpty)
        #expect(store.readString("vela.missing") == nil)

        defaults.set("{{{", forKey: "vela.broken")
        #expect(store.readList("vela.broken").isEmpty)
        #expect(store.readObject("vela.broken").isEmpty)
    }

    /// An object read as a list — and the reverse — is empty rather than a
    /// crash or a half-record.
    @Test func aWrongShapeReadsEmpty() {
        let (store, defaults) = fresh()
        defaults.set(#"{"a":1}"#, forKey: "vela.object")
        #expect(store.readList("vela.object").isEmpty)
        defaults.set("[1,2,3]", forKey: "vela.list")
        #expect(store.readObject("vela.list").isEmpty)
    }

    /// A value that cannot be serialized keeps yesterday's good bytes rather
    /// than writing something three other clients would have to survive.
    @Test func anUnserializableValueIsDroppedRatherThanCorruptingTheShelf() {
        let (store, defaults) = fresh()
        store.writeList("vela.test", [["good": 1]])
        store.writeList("vela.test", [["bad": Double.nan]])
        #expect(defaults.string(forKey: "vela.test") == #"[{"good":1}]"#)
    }

    /// An empty string is not a value: several machines read "never written" as
    /// a real answer, distinct from anything the key could hold.
    @Test func anEmptyStringIsTreatedAsUnwritten() {
        let (store, defaults) = fresh()
        store.writeString("vela.currency", "CNY")
        #expect(store.readString("vela.currency") == "CNY")
        store.writeString("vela.currency", "")
        #expect(store.readString("vela.currency") == nil)
        #expect(defaults.string(forKey: "vela.currency") == nil)
    }

    /// The `AccountStore` invariant, restated at the level below it: a record
    /// goes in and comes out whole. A mapper that copies an account field by
    /// field drops `keys`, and the address derives from ALL of them — so the
    /// next restore would produce a different Safe, at an address nothing can
    /// deploy.
    @Test func aMultiKeyRecordSurvivesARoundTripWhole() {
        let (store, _) = fresh()
        let account: [String: Any] = [
            "id": "cred-1",
            "address": "0x88cca07a5d3e0b1eb5f7c0f2b1f3f4a5b6c76894",
            "keys": [["x": "0xaa", "y": "0xbb"], ["x": "0xcc", "y": "0xdd"]],
            "name": "Vela",
        ]
        store.writeList(VelaStore.Key.accounts, [account])
        let read = store.readList(VelaStore.Key.accounts).first
        #expect((read?["keys"] as? [[String: Any]])?.count == 2)
        #expect(read?.keys.count == account.keys.count, "a field was lost in the round trip")
    }

    /// The keys are a contract, not this app's private business. A typo here is
    /// not a local bug — it is a wallet that cannot see the address book it
    /// wrote yesterday.
    @Test func theKeyNamesAreTheCrossClientOnes() {
        #expect(VelaStore.Key.contacts == "vela.contacts")
        #expect(VelaStore.Key.contactsDismissed == "vela.contacts.dismissed")
        #expect(VelaStore.Key.contactGroups == "vela.contactGroups")
        #expect(VelaStore.Key.customNetworks == "vela.customNetworks")
        #expect(VelaStore.Key.networkConfig == "vela.networkConfig")
        #expect(VelaStore.Key.rpcProviders == "vela.rpcProviders")
        #expect(VelaStore.Key.serviceEndpoints == "vela.serviceEndpoints")
        #expect(VelaStore.Key.displayCurrency == "vela.displayCurrency")
        #expect(VelaStore.Key.accounts == "vela.accounts")
    }
}
