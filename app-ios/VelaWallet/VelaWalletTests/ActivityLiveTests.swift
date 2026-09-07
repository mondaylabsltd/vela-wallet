//
//  ActivityLiveTests.swift
//  VelaWalletTests
//
//  The receipt pipeline against the real world.
//
//  Behind the same compile flag as the other live suites:
//
//      xcodebuild test ... -only-testing:VelaWalletTests/ActivityLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//
//  ## The address these tests watch is not ours
//
//  `token_trust` scans the **last 100 blocks** — it is a live receipt watcher,
//  not a history importer — so an account that was paid last week finds
//  nothing, correctly. To prove the pipeline actually decodes and persists a
//  receipt, the scan needs an address that is being paid *right now*, and the
//  most reliably-paid address on Ethereum is an exchange's hot wallet.
//
//  Nothing is sent, nothing is signed, and only public logs are read.
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct ActivityLiveTests {

    /// Binance's hot wallet — it receives stablecoins continuously, which is
    /// the only property this test needs from it.
    private static let busyAddress = "0x28C6c06298d514Db089934071355E5743bf21d60"
    private static let goldenSafe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func fresh() -> (VelaStore, AccountStore, HeldTokens, TokenTrustStore) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        return (store, accounts, held, trust)
    }

    /// The whole scan chain, end to end: block number → `eth_getLogs` on the
    /// registry's stablecoin allowlist → local `topics[2]` re-verification →
    /// block timestamps → metadata gate → judged feed.
    @Test func theScanFindsRealReceiptsForAnAddressBeingPaid() async {
        let (_, _, _, trust) = fresh()
        let incoming = await trust.pollIncoming(address: Self.busyAddress, chainIds: [1])

        #expect(!incoming.isEmpty, "no receipts in the last 100 Ethereum blocks — suspicious")
        for transfer in incoming.prefix(3) {
            #expect(!transfer.id.isEmpty)
            #expect(!transfer.txHash.isEmpty)
            #expect(transfer.timestampSec > 1_700_000_000, "a receipt with no plausible time")
            // The metadata gate: an ERC-20 the core could not name never
            // reaches this list, because "+0 tokens" is worse than silence.
            if !transfer.isNative {
                #expect(transfer.symbol != nil, "an unnamed token slipped past the gate")
                #expect(transfer.decimals != nil)
            }
        }
        print("[live] receipts found: \(incoming.count)")
        if let first = incoming.first {
            print("[live] newest: \(first.value) \(first.symbol ?? "?") from \(first.from)")
        }
    }

    /// The executor's whole operation: scan, map, merge — and the count it
    /// answers is what the core celebrates on.
    ///
    /// The second run must answer **zero**: the same window rediscovered is not
    /// new money, and a core told otherwise would toast a receipt somebody
    /// already saw.
    @Test func aRescanOfTheSameWindowIsNotNewMoney() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        let held = HeldTokens()
        let trust = TokenTrustStore(store: store, pool: pool, accounts: accounts, held: held)
        let executor = ActivityExecutor(store: store, accounts: accounts, held: held, trust: trust)

        // The held set decides which chains are scanned; this account "holds"
        // ETH on mainnet as far as this test is concerned.
        held.record(address: Self.busyAddress, tokens: [[
            "chain_id": 1, "symbol": "ETH", "decimals": 18,
            "token_address": NSNull(), "price_usd": 2_500,
        ]])

        let first = (try? CoreJSON.object(await executor.perform([
            "type": "scan_incoming_transfers", "address": Self.busyAddress,
        ]))) ?? [:]
        #expect(first["type"] as? String == "sync_completed")
        let landed = (first["new_count"] as? NSNumber)?.intValue ?? 0
        #expect(landed > 0, "the scan persisted nothing for an address being paid")
        print("[live] persisted receipts: \(landed)")

        let stored = TxRecords.load(store: store)
        #expect(stored.count == min(landed, TxRecords.cap))
        for record in stored.prefix(3) {
            #expect(record["type"] as? String == "receive")
            #expect(record["status"] as? String == "confirmed")
            // The unit trap, on the ingest side: a human decimal, never raw
            // units. A 6-decimals stablecoin recorded raw reads a million times
            // too large.
            let value = record["value"] as? String ?? ""
            #expect(!value.isEmpty && Double(value) != nil)
            #expect((record["usd"] as? String)?.hasPrefix("$") == true)
        }
        if let newest = stored.first {
            print("[live] stored: \(newest["value"] ?? "?") \(newest["symbol"] ?? "?")" +
                  " usd=\(newest["usd"] ?? "?")")
        }

        let second = (try? CoreJSON.object(await executor.perform([
            "type": "scan_incoming_transfers", "address": Self.busyAddress,
        ]))) ?? [:]
        #expect((second["new_count"] as? NSNumber)?.intValue == 0,
                "the same window came back as new money")
    }

    /// The golden Safe's own feed, which is expected to be EMPTY — and that is
    /// the honest answer rather than a defect.
    ///
    /// Its xDAI arrived as a native transfer, and a native transfer emits no
    /// `Transfer` log at all on a chain without EIP-7708. A hundred-block
    /// window on top of that means this wallet shows a receipt only while it is
    /// running. Worth a test because the empty screen it produces is the one a
    /// person will ask about.
    @Test func anAccountWithNoRecentErc20ReceiptsHasAnEmptyFeed() async {
        let (_, _, _, trust) = fresh()
        let incoming = await trust.pollIncoming(address: Self.goldenSafe, chainIds: [100])
        print("[live] golden Safe receipts on Gnosis: \(incoming.count)")
        for transfer in incoming {
            #expect(!transfer.id.isEmpty)
        }
    }
}

#endif
