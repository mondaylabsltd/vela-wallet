//
//  ReadPathContractTests.swift
//  VelaWalletTests
//
//  The two promises spec 051 made about the read path, pinned where they can be
//  checked without a network: **an unreachable chain is unreachable, never
//  zero** (SC-003), and **one pool serves every caller** (SC-002).
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct ReadPathContractTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func fresh() -> (VelaStore, RpcPool) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        // Unbooted: every routed call is refused without a packet leaving the
        // machine, which is the same shape as a device with no network.
        return (store, RpcPool(store: store, accounts: AccountStore(defaults: defaults)))
    }

    /// **SC-003.** A chain that could not be read is reported FAILED — never as
    /// a holding of zero, and never simply omitted.
    ///
    /// Both of the wrong answers make the total look complete when it is not.
    /// The core has `balance_partial` precisely so the screen can say "at least
    /// this much" instead of a number that is quietly wrong.
    @Test func anUnreachableChainIsReportedFailedRatherThanEmpty() async {
        let (store, pool) = fresh()
        let executor = BalanceExecutor(store: store, pool: pool, held: HeldTokens())

        let reply = (try? CoreJSON.object(await executor.perform([
            "type": "fetch_tokens", "address": golden, "force": true, "pull": false,
        ]))) ?? [:]

        #expect(reply["type"] as? String == "fetch_settled")
        let failed = reply["failed_chain_ids"] as? [Int] ?? []
        let tokens = reply["tokens"] as? [[String: Any]] ?? []

        // Every chain with a native coin to read failed — Tempo has none, so it
        // asks nothing and cannot fail.
        let expected = ChainCatalog.chains.filter { $0.gasModel != .tempo }.map(\.chainId)
        #expect(Set(failed) == Set(expected), "failed: \(failed)")
        #expect(tokens.isEmpty, "an unreachable chain contributed a holding: \(tokens)")
        // And the account's own switcher read answers `null`, not an empty
        // wallet: a switcher row shows one figure or none.
        let assets = (try? CoreJSON.object(await executor.perform([
            "type": "fetch_account_assets", "address": golden,
        ]))) ?? [:]
        #expect(assets["tokens"] is NSNull)
    }

    /// A chain the pool merely THROTTLED is separated from one that is down.
    ///
    /// Invariant ④: a rate limit lifts by itself, so the "swap in your own RPC"
    /// banner must never nag for it. The two lists are different fields, and
    /// this pins that the shell fills them separately.
    @Test func throttlingIsReportedApartFromFailure() async {
        let (store, pool) = fresh()
        let executor = BalanceExecutor(store: store, pool: pool, held: HeldTokens())
        let reply = (try? CoreJSON.object(await executor.perform([
            "type": "fetch_tokens", "address": golden, "pull": false,
        ]))) ?? [:]
        // A refused-before-boot call is a failure, not a throttle — nothing
        // asked us to slow down.
        #expect((reply["rate_limited_chain_ids"] as? [Int] ?? []).isEmpty)
        #expect(!(reply["failed_chain_ids"] as? [Int] ?? []).isEmpty)
    }

    /// **SC-002, the half that needs no network.** Two machines' executors on
    /// one pool: when it cannot route, BOTH fail closed and neither invents an
    /// answer of its own.
    ///
    /// That two callers share one session is proven where a ban can actually be
    /// observed — `RpcPoolLiveTests.oneBanMapServesEveryCaller`. What this pins
    /// is the other half: a caller that cannot reach the pool does not fall
    /// back to reading a chain by itself.
    @Test func everyCallerFailsClosedThroughTheSamePool() async {
        let (store, pool) = fresh()
        let balances = BalanceExecutor(store: store, pool: pool, held: HeldTokens())
        let contacts = ContactsExecutor(store: store, pool: pool)

        let fetched = (try? CoreJSON.object(await balances.perform([
            "type": "fetch_tokens", "address": golden, "pull": false,
        ]))) ?? [:]
        let classified = (try? CoreJSON.object(await contacts.perform([
            "type": "classify_recipient", "chain_id": 100, "address": golden,
        ]))) ?? [:]

        // The balance read reports failed chains rather than zeros…
        #expect(!(fetched["failed_chain_ids"] as? [Int] ?? []).isEmpty)
        // …and the classifier reports UNKNOWN rather than "definitely an EOA".
        #expect(classified["code"] is NSNull)
        // Nothing went out: no caller opened its own transport behind the
        // pool's back.
        #expect(!pool.booted)
    }
}
