//
//  RpcPoolLiveTests.swift
//  VelaWalletTests
//
//  **SC-002** against the real network, behind the usual flag:
//
//      xcodebuild test ... -only-testing:VelaWalletTests/RpcPoolLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct RpcPoolLiveTests {

    /// A `u32`-max chain id — one the registry genuinely has no row for.
    /// (999999999 was tried first and IS served; see phase 1's results.)
    private let nowhere = 4_294_967_294

    /// One session, one set of facts about the network.
    ///
    /// Two different machines' executors call the same dead chain. The failure
    /// the FIRST one caused is visible to the pool the second one uses — that
    /// is the whole architectural claim of this cut, and the alternative (a
    /// pool per screen) is how the Expo client got a ban map that disagreed
    /// with itself.
    @Test func oneBanMapServesEveryCaller() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()

        // Caller A: the balance read, through `TokenReads`.
        let read = await TokenReads.read(
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            chainId: nowhere, tokens: [], pool: pool
        )
        #expect(read.failed, "a chain nobody serves answered a balance")
        #expect(pool.failedChains.contains(nowhere))

        // Caller B: the address book's classifier, on the same chain, through
        // the SAME pool instance.
        let contacts = ContactsExecutor(store: store, pool: pool)
        let classified = (try? CoreJSON.object(await contacts.perform([
            "type": "classify_recipient", "chain_id": nowhere,
            "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ]))) ?? [:]
        #expect(classified["code"] is NSNull, "an unroutable chain produced a verdict")

        // The list is the pool's, not either caller's: one entry, still there
        // after both of them asked.
        #expect(pool.failedChains.filter { $0 == nowhere }.count == 1)
        print("[live] failed chains after two callers: \(pool.failedChains)")
    }

    /// A chain that CAN be routed answers for both callers — the same session
    /// serving two machines, which is the other half of the claim.
    @Test func oneSessionAnswersTwoMachines() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()

        let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let balance = await TokenReads.read(address: golden, chainId: 100, tokens: [], pool: pool)
        #expect(!balance.failed)

        let contacts = ContactsExecutor(store: store, pool: pool)
        let classified = (try? CoreJSON.object(await contacts.perform([
            "type": "classify_recipient", "chain_id": 100, "address": golden,
        ]))) ?? [:]
        // The golden Safe is deployed, so it has code — and the answer came
        // through the same pool the balance did.
        #expect((classified["code"] as? String ?? "").count > 2,
                "the golden Safe reported no code: \(classified)")
        #expect(!pool.failedChains.contains(100))
    }
}

#endif
