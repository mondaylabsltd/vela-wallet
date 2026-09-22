//
//  IdentityLiveTests.swift
//  VelaWalletTests
//
//  The name waterfall against the real registries, behind the usual flag:
//
//      xcodebuild test ... -only-testing:VelaWalletTests/IdentityLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct IdentityLiveTests {

    /// The most famously-named address on Ethereum. Public, and not ours.
    private let vitalik = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"

    private func fresh() -> (VelaStore, RpcPool, RecipientIdentity) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        return (store, pool, RecipientIdentity(store: store, pool: pool, accounts: accounts))
    }

    /// ENS reverse resolution, end to end: `namehash("<addr>.addr.reverse")` →
    /// `registry.resolver(node)` → `resolver.name(node)` — and then, since
    /// FR-010, the name back FORWARD through `vela_core::app::name_verify`
    /// before it counts.
    ///
    /// Every step is one this client computes itself, so a wrong namehash, a
    /// mis-sliced resolver word or a mis-decoded string all show up here as
    /// "nobody could name them" — and so, now, does a forward lookup this
    /// shell fails to carry: vitalik.eth is verified because its forward
    /// record really does point back at this address.
    @Test func ensNamesResolveThroughTheWaterfall() async {
        let (store, _, identity) = fresh()
        guard let found = await identity.resolve(vitalik) else {
            Issue.record("ENS did not name the most-named address on Ethereum")
            return
        }
        #expect(found.name == "vitalik.eth")
        #expect(found.source == "ENS")
        print("[live] \(vitalik) is \(found.name) (\(found.source))")

        // Only positive answers are cached — and this one is, so the second
        // lookup asks nobody.
        let cached = store.readObject("recipient_id.v2:" + vitalik.lowercased())
        #expect((cached["identity"] as? [String: Any])?["name"] as? String == "vitalik.eth")
    }

    /// An address with no name anywhere stays `nil` — not an invented label,
    /// and not an error.
    @Test func anUnnamedAddressStaysUnnamed() async {
        let (store, _, identity) = fresh()
        // A burn-adjacent address nobody has ever registered a name for.
        let nobody = "0x000000000000000000000000000000000000dEaD"
        #expect(await identity.resolve(nobody) == nil)
        #expect(store.readObject("recipient_id.v2:" + nobody.lowercased()).isEmpty,
                "an absence was cached")
    }

    /// `classify_recipient` reads the RAW code — the core owns both
    /// projections. A contract has code; a plain EOA answers `0x`, which is a
    /// verdict, not a failure.
    ///
    /// This test first used vitalik.eth as its EOA and failed: that address now
    /// answers `0xef0100…`, an **EIP-7702 delegation designator**. The test was
    /// wrong about the world, and the world is worth recording — a client that
    /// read "has code ⇒ contract" would badge a growing share of ordinary
    /// wallets as contracts. `contacts.rs` already carves it out
    /// (`is_eip7702_delegation`), which is exactly why the shell hands over the
    /// raw bytes instead of a boolean.
    @Test func theRecipientClassifierSeesCodeAndItsAbsence() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let executor = ContactsExecutor(store: store, pool: pool)

        let contract = (try? CoreJSON.object(await executor.perform([
            "type": "classify_recipient", "chain_id": 1, "address": Multicall.address,
        ]))) ?? [:]
        let code = contract["code"] as? String ?? ""
        #expect(code.count > 2, "Multicall3 came back with no code: \(contract)")

        // The burn address: no code, and `0x` is an ANSWER — the core reads it
        // as "definitely an EOA", which `null` must never become.
        let eoa = (try? CoreJSON.object(await executor.perform([
            "type": "classify_recipient", "chain_id": 1,
            "address": "0x000000000000000000000000000000000000dEaD",
        ]))) ?? [:]
        #expect(eoa["code"] as? String == "0x", "a codeless address must answer 0x, not null")

        let delegated = (try? CoreJSON.object(await executor.perform([
            "type": "classify_recipient", "chain_id": 1, "address": vitalik,
        ]))) ?? [:]
        let designator = delegated["code"] as? String ?? ""
        // Whatever it is today, the shell passes it through untouched.
        #expect(designator.hasPrefix("0x"))
        print("[live] Multicall3 code \(code.count) chars; " +
              "vitalik.eth code \(designator.prefix(10))… (\(designator.count) chars)")
    }
}

#endif
