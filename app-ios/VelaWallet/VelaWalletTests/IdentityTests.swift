//
//  IdentityTests.swift
//  VelaWalletTests
//
//  Who an address belongs to: the parts that can be decided without a network.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct IdentityTests {
    private let vitalik = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"

    private func fresh() -> (VelaStore, RecipientIdentity) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        // Unbooted on purpose: every routed call is refused, so nothing here
        // reaches the network.
        return (store, RecipientIdentity(
            store: store, pool: RpcPool(store: store, accounts: accounts), accounts: accounts
        ))
    }

    // MARK: - The node

    /// A reverse node must not depend on how the address was CASED. The
    /// registry's names are lowercase; hashing a checksummed address gives a
    /// different node, which resolves to nothing — silently, which is the worst
    /// way for a name lookup to fail.
    @Test func theReverseNodeIgnoresAddressCasing() {
        #expect(Ens.reverseNode(vitalik) == Ens.reverseNode(vitalik.lowercased()))
        #expect(Ens.reverseNode(vitalik) == Ens.reverseNode(vitalik.uppercased()
            .replacingOccurrences(of: "0X", with: "0x")))
        #expect(Ens.reverseNode(vitalik).count == 32)
        // And it is the namehash of the documented name, not of the address.
        let tail = String(vitalik.dropFirst(2)).lowercased()
        #expect(Ens.reverseNode(vitalik) == Ens.namehash("\(tail).addr.reverse"))
    }

    /// EIP-137's published vector, and the empty name's zero node — the same
    /// hash the fiat feeds address their contracts with, now that both callers
    /// share one implementation.
    @Test func namehashIsTheSharedOne() {
        #expect(Ens.namehash("eth").hexString
                == "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae")
        #expect(Ens.namehash("").hexString == String(repeating: "0", count: 64))
        #expect(Ens.namehash("gbp-usd.data.eth") == FiatRates.namehash("gbp-usd.data.eth"))
    }

    @Test func anAddressWordIsItsLowTwentyBytes() {
        let word = String(repeating: "0", count: 24) + "d8da6bf26964af9d7eed9e03e53415d37aa96045"
        #expect(Ens.addressWord(Data(hexString: word)!) == "0x" + vitalik.dropFirst(2).lowercased())
        #expect(Ens.isZeroAddress("0x" + String(repeating: "0", count: 40)))
        #expect(!Ens.isZeroAddress(vitalik))
    }

    // MARK: - The waterfall's refusals

    /// Three addresses nobody should be asked about: a malformed one, a short
    /// one, and the zero address — a mint/burn counterparty in EIP-7708 native
    /// events, and a guaranteed 404 at the index.
    @Test func someAddressesAreNotWorthAsking() async {
        let (_, identity) = fresh()
        #expect(await identity.resolve("not an address") == nil)
        #expect(await identity.resolve("0xabc") == nil)
        #expect(await identity.resolve("0x" + String(repeating: "0", count: 40)) == nil)
    }

    /// With nothing reachable the answer is `nil` — and NOTHING is written.
    /// Caching an absence would keep somebody nameless for a day after they
    /// registered one.
    @Test func anUnresolvedLookupCachesNothing() async {
        let (store, identity) = fresh()
        #expect(await identity.resolve(vitalik) == nil)
        #expect(store.readObject("recipient_id.v2:" + vitalik.lowercased()).isEmpty)
    }

    /// A cached name is used without asking anybody, and it expires.
    @Test func aCachedNameIsReadBackAndExpires() async {
        let (store, identity) = fresh()
        let key = "recipient_id.v2:" + vitalik.lowercased()
        store.writeObject(key, [
            "identity": ["name": "vitalik.eth", "source": "ENS"],
            "cachedAt": Date().timeIntervalSince1970 * 1000,
        ])
        let found = await identity.resolve(vitalik)
        #expect(found?.name == "vitalik.eth")
        #expect(found?.source == "ENS")

        // A day and a bit old: not an answer any more.
        let (staleStore, staleIdentity) = fresh()
        staleStore.writeObject(key, [
            "identity": ["name": "vitalik.eth", "source": "ENS"],
            "cachedAt": Date().timeIntervalSince1970 * 1000 - 25 * 60 * 60 * 1000,
        ])
        #expect(await staleIdentity.resolve(vitalik) == nil)
    }

    /// The cache key is web's, so a name resolved in one client is not
    /// re-resolved in the other — and it is lowercased, or the same person
    /// gets two entries.
    @Test func theCacheKeyIsTheOneWebWrites() async {
        let (store, identity) = fresh()
        store.writeObject("recipient_id.v2:" + vitalik.lowercased(), [
            "identity": ["name": "vitalik.eth", "source": "ENS"],
            "cachedAt": Date().timeIntervalSince1970 * 1000,
        ])
        // Asked with the CHECKSUMMED address, answered from the lowercased key.
        #expect(await identity.resolve(vitalik)?.name == "vitalik.eth")
    }
}
