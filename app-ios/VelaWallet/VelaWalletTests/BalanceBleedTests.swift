//
//  BalanceBleedTests.swift
//  VelaWalletTests
//
//  Real-device pass 2, D3 (S2): the multisig's home showed the MEMBER
//  account's money ($2.4M — its ETH, xDAI and pathUSD) under the multisig,
//  "Updated 24m", and kept it across a relaunch; Send and the pay link's Send
//  offered 999.98 where the multisig held 99.75. First seen right after the
//  dev-only `VELA_PARALLEL_ACCOUNT` switch.
//
//  The cause was the shell's: the balance executor's mid-fetch snapshots
//  went out labelled with the account in FRONT when they landed
//  (`WalletStore.scopedTo`), not the account the fetch read. At launch the
//  home opens the account in front (the member: the dev switch put it
//  there) and a beat later the multisig the accounts machine remembers —
//  so every snapshot of the member's fetch still out was the multisig's to
//  the core, drawn, kept as its rows (`HoldingsCache`), and restored on the
//  next launch. The core now paints a snapshot only for the fetch it came
//  from (its `generation`), and the shell labels each with the fetch's own
//  address and generation.
//

#if DEBUG

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BalanceBleedTests {
    private static let multisig = "0x5aFE00000000000000000000000000000000Cafe"

    private func token(chain: Int, symbol: String, balance: String, price: Double) -> [String: Any] {
        [
            "chain_id": chain, "symbol": symbol, "name": symbol, "balance": balance,
            "decimals": 18, "token_address": NSNull(), "price_usd": price, "spam": false,
        ]
    }

    private func device() -> (store: VelaStore, accounts: AccountStore, pool: RpcPool, suite: String) {
        let suite = "balance-bleed-\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        return (store, accounts, RpcPool(store: store, accounts: accounts, offline: true), suite)
    }

    /// The dev switch's member: `VELA_PARALLEL_ACCOUNT=1` puts fixture key
    /// 1's own Safe in front — the account the money was the member's of.
    private func memberInFront(_ store: VelaStore, _ accounts: AccountStore) async throws -> String {
        let binding = ParallelSpaceBinding()
        await binding.apply(
            environment: ["VELA_PARALLEL_SPACE": "1", "VELA_PARALLEL_ACCOUNT": "1"],
            store: store, accounts: accounts
        )
        #expect(binding.isActive)
        let list = await accounts.loadAccounts()
        let index = await accounts.loadActiveIndex()
        return try #require(list.indices.contains(index) ? list[index]["address"] as? String : nil)
    }

    /// The shell's half: a snapshot is labelled with the fetch it came from
    /// — its address and generation, from the `fetch_tokens` it answers —
    /// whatever account is in front by then.
    @Test func eachSnapshotIsLabelledWithItsOwnFetch() async throws {
        let (store, accounts, pool, suite) = device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        let executor = BalanceExecutor(store: store, pool: pool, held: HeldTokens(), chainDeadlineMs: .max)
        _ = accounts
        var labels: [(String, Int)] = []
        executor.onChainAssets = { address, generation, _ in labels.append((address, generation)) }
        _ = await executor.perform([
            "type": "fetch_tokens", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            "force": false, "pull": false, "generation": 7,
        ])
        #expect(!labels.isEmpty, "a snapshot as each chain lands")
        for (address, generation) in labels {
            #expect(address == "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
            #expect(generation == 7)
        }
    }

    /// The device's sequence, on the REAL core: the dev switch puts the
    /// member in front, the home opens it, then the multisig. The member's
    /// fetch still streams — labelled the way the old shell did (the
    /// multisig's address): the multisig shows nothing of it, Send's figures
    /// are the multisig's own, and a relaunch restores only its own rows.
    @Test func theMembersMoneyIsNeverTheMultisigsAcrossASwitchAndARelaunch() async throws {
        let (store, accounts, pool, suite) = device()
        defer { UserDefaults().removePersistentDomain(forName: suite) }
        let member = try await memberInFront(store, accounts)
        #expect(member.caseInsensitiveCompare(Self.multisig) != .orderedSame)

        let wallet = WalletStore(store: store, pool: pool, held: HeldTokens())
        wallet.open(address: member)        // generation 1: the member's fetch
        wallet.open(address: Self.multisig) // generation 2: the multisig's
        #expect(wallet.balance?.address?.caseInsensitiveCompare(Self.multisig) == .orderedSame)

        // The member's in-flight snapshot, labelled with the account in front.
        wallet.chainAssetsArrived(address: Self.multisig, generation: 1, tokens: [
            token(chain: 1, symbol: "ETH", balance: "999.98", price: 2400),
            token(chain: 100, symbol: "xDAI", balance: "500", price: 1),
        ])
        let view = try #require(wallet.balance)
        #expect(view.tokens.isEmpty, "the member's money under the multisig: \(view.tokens.map(\.symbol))")
        #expect(view.displayTotalUsd == nil || view.displayTotalUsd == 0)
        #expect(SendExecutor.sameAccount(view, Self.multisig))
        #expect(SendExecutor.sendTokens(view).isEmpty, "Send offers nothing that is not the multisig's")

        // The multisig's own snapshot paints, and is what Send offers.
        wallet.chainAssetsArrived(address: Self.multisig, generation: 2, tokens: [
            token(chain: 100, symbol: "xDAI", balance: "99.75", price: 1),
        ])
        let own = try #require(wallet.balance)
        #expect(own.tokens.map(\.balance) == ["99.75"])
        let offered = SendExecutor.sendTokens(own)
        #expect(offered.compactMap { $0["balance"] as? String } == ["99.75"])

        // A relaunch: only the multisig's own kept rows open its list.
        HoldingsCache.save(address: member, space: "mainnet",
                           tokens: [token(chain: 1, symbol: "ETH", balance: "999.98", price: 2400)], store: store)
        let relaunched = WalletStore(store: store, pool: pool, held: HeldTokens())
        relaunched.open(address: Self.multisig)
        let after = try #require(relaunched.balance)
        #expect(after.address?.caseInsensitiveCompare(Self.multisig) == .orderedSame)
        #expect(!after.tokens.contains { $0.balance == "999.98" }, "the member's kept rows restored as the multisig's")
    }
}

#endif
