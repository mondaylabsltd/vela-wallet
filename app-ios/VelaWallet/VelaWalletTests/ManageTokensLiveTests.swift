//
//  ManageTokensLiveTests.swift
//  VelaWalletTests
//
//  The add-token probe against the real world, behind the usual flag:
//
//      xcodebuild test ... -only-testing:VelaWalletTests/ManageTokensLiveTests \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct ManageTokensLiveTests {

    private func executor() -> (VelaStore, ManageTokensExecutor) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        pool.boot()
        return (store, ManageTokensExecutor(store: store, pool: pool))
    }

    private func probe(_ executor: ManageTokensExecutor, chainId: Int, address: String) async
        -> [String: Any] {
        (try? CoreJSON.object(await executor.perform([
            "type": "multicall_erc20_meta", "chain_id": chainId, "address": address,
        ]))) ?? [:]
    }

    /// `name()`, `symbol()` and `decimals()` in one batch, off a contract every
    /// chain knows.
    ///
    /// The decimals are the assertion that matters: USDT has **6**, and a
    /// wallet that recorded 18 would show somebody's 1,000 USDT as
    /// 0.000000000001.
    @Test func aRealContractAnswersWithItsOwnMetadata() async {
        let (_, executor) = executor()
        let reply = await probe(executor, chainId: 1,
                                address: "0xdAC17F958D2ee523a2206206994597C13D831ec7")
        #expect(reply["type"] as? String == "chain_meta_resolved")
        // The echo the core correlates on — verbatim, including its casing.
        #expect(reply["address"] as? String == "0xdAC17F958D2ee523a2206206994597C13D831ec7")
        guard let meta = reply["meta"] as? [String: Any] else {
            Issue.record("Ethereum did not know USDT: \(reply)")
            return
        }
        #expect(meta["symbol"] as? String == "USDT")
        #expect(meta["decimals"] as? Int == 6)
        print("[live] USDT on Ethereum: \(meta)")
    }

    /// The address is sent **exactly as typed**. An `eth_call` does not care
    /// about the casing, and only the save lowercases — so a lowercased
    /// contract must resolve identically.
    @Test func theProbeIsCaseInsensitiveOnChain() async {
        let (_, executor) = executor()
        let reply = await probe(executor, chainId: 1,
                                address: "0xdac17f958d2ee523a2206206994597c13d831ec7")
        #expect((reply["meta"] as? [String: Any])?["symbol"] as? String == "USDT")
    }

    /// A contract that is not an ERC-20 answers `null` — not an error, and not
    /// a token with an invented symbol. The core reads that as "this chain does
    /// not have it".
    @Test func anAddressThatIsNotATokenAnswersNothing() async {
        let (_, executor) = executor()
        // Multicall3 itself: a real contract, and not an ERC-20.
        let reply = await probe(executor, chainId: 1, address: Multicall.address)
        #expect(reply["type"] as? String == "chain_meta_resolved")
        #expect(reply["meta"] is NSNull, "a non-token was given metadata")
    }

    /// The same contract on a chain that has never heard of it.
    @Test func aChainWithoutTheTokenAnswersNothing() async {
        let (_, executor) = executor()
        let reply = await probe(executor, chainId: 100,
                                address: "0xdAC17F958D2ee523a2206206994597C13D831ec7")
        #expect(reply["meta"] is NSNull, "Gnosis claimed to know Ethereum's USDT")
    }
}

#endif
