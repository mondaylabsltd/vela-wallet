//
//  RpcEndpointsTests.swift
//  VelaWalletTests
//
//  What endpoints exist for a chain — and, more importantly, what this layer
//  must never decide.
//
//  The one that would be silently wrong: **bans are not filtered here.** The
//  core excludes banned URLs at selection (invariant ⑧) precisely so its
//  all-banned self-rescue has something to reconsider. A shell that helpfully
//  removed them at collection would leave the core rescuing an empty list.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct RpcEndpointsTests {

    private func fresh() -> (VelaStore, AccountStore, UserDefaults) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        return (VelaStore(defaults: defaults), AccountStore(defaults: defaults), defaults)
    }

    // MARK: - The chain table

    /// Every chain the settings screen lists must be routable, or a person can
    /// add a network that reads as present and returns nothing.
    @Test func theCatalogCoversTheCoresBuiltins() {
        let coreBuiltins = [
            1, 56, 137, 42_161, 10, 8_453, 43_114, 100, 130, 4_217, 143, 480,
            5_042, 196, 988, 1_868, 4_326, 4_663, 5_000, 8_217, 42_220, 57_073,
            98_866, 1_440_000,
        ]
        #expect(ChainCatalog.chains.count == coreBuiltins.count)
        for chainId in coreBuiltins {
            #expect(ChainCatalog.meta(chainId) != nil, "chain \(chainId) is not routable")
        }
    }

    /// `chainId` and `apiNetworkId` are an inverse pair — the balance API's
    /// tokens carry the name, and the wallet has to map back.
    @Test func theApiNetworkIdRoundTrips() {
        for chain in ChainCatalog.chains {
            #expect(ChainCatalog.meta(apiNetworkId: chain.apiNetworkId)?.chainId == chain.chainId)
        }
    }

    /// Tempo has no native coin — gas is a USD stablecoin. Marking it is what
    /// keeps a chain whose RPC returns a constant 4e75 from putting 4e57
    /// dollars into somebody's total.
    @Test func tempoIsMarkedAsHavingNoNativeCoin() {
        #expect(ChainCatalog.meta(4_217)?.gasModel == .tempo)
        #expect(ChainCatalog.meta(1)?.gasModel == .native)
    }

    // MARK: - Provider URLs

    @Test func providerUrlsAreBuiltOnlyWhereTheProviderServesTheChain() {
        #expect(RpcProvider.alchemy.url(chainId: 100, key: "k")
            == "https://gnosis-mainnet.g.alchemy.com/v2/k")
        #expect(RpcProvider.drpc.url(chainId: 1, key: "k")
            == "https://lb.drpc.org/ogrpc?network=ethereum&dkey=k")
        #expect(RpcProvider.ankr.url(chainId: 56, key: "k") == "https://rpc.ankr.com/bsc/k")

        // Ankr does not serve Unichain, and inventing a slug would produce an
        // endpoint that 404s on every call.
        #expect(RpcProvider.ankr.url(chainId: 130, key: "k") == nil)
        #expect(RpcProvider.alchemy.url(chainId: 1, key: "") == nil)
    }

    // MARK: - Collection

    /// The cold-start order: the person's own endpoint first, then a paid
    /// provider, then the built-in, then curated public.
    @Test func tiersComeInTheColdStartOrder() async {
        let (store, accounts, _) = fresh()
        store.writeList(VelaStore.Key.networkConfig,
                        [["chainId": 100, "rpcURL": "https://mine.example"]])
        store.writeObject(VelaStore.Key.rpcProviders, ["alchemy": "abc"])

        let collected = await RpcEndpoints.collect(chainId: 100, store: store, accounts: accounts)
        let sources = collected.map(\.source)
        #expect(collected.first?.url == "https://mine.example")
        #expect(sources.first == "user")
        #expect(sources.contains("provider"))
        #expect(sources.contains("default"))
        // The order is monotonic through the tiers it has.
        let rank = ["user": 0, "provider": 1, "default": 2, "public": 3, "builtin": 4, "fallback": 5]
        let ranks = sources.compactMap { rank[$0] }
        #expect(ranks == ranks.sorted(), "tiers came out of order: \(sources)")
    }

    /// A URL appears once, in its highest tier.
    @Test func aUrlIsCollectedOnce() async {
        let (store, accounts, _) = fresh()
        // The same URL as the built-in default: it must not also appear as a
        // user override, or the pool would score one endpoint twice.
        let builtin = ChainCatalog.meta(100)!.rpcURL
        store.writeList(VelaStore.Key.networkConfig,
                        [["chainId": 100, "rpcURL": builtin]])

        let collected = await RpcEndpoints.collect(chainId: 100, store: store, accounts: accounts)
        #expect(collected.filter { $0.url == builtin }.count == 1)
        #expect(collected.first { $0.url == builtin }?.source == "default")
    }

    /// **Invariant ⑧.** A banned URL is still collected — the core filters at
    /// selection, and its self-rescue needs the banned ones to reconsider.
    @Test func bannedUrlsAreStillCollected() async {
        let (store, accounts, _) = fresh()
        let builtin = ChainCatalog.meta(100)!.rpcURL
        RpcEndpoints.saveBans(
            [["url": builtin, "banned_at_ms": Date().timeIntervalSince1970 * 1000,
              "permanent": true]],
            store: store
        )
        let collected = await RpcEndpoints.collect(chainId: 100, store: store, accounts: accounts)
        #expect(collected.contains { $0.url == builtin },
                "collection filtered a banned URL — the core's self-rescue is now blind")
    }

    /// A chain nobody built in is reachable only through the index, so an
    /// unknown chain collects nothing rather than crashing.
    @Test func anUnknownChainCollectsNothingLocally() async {
        let (store, accounts, _) = fresh()
        let collected = await RpcEndpoints.collect(chainId: 999_999_999,
                                                   store: store, accounts: accounts)
        #expect(collected.allSatisfy { $0.source == "builtin" || $0.source == "fallback" })
    }

    // MARK: - The ban map

    /// Stored camelCase, wire snake_case — the 050 alphabet rule, one shelf
    /// further along.
    @Test func bansRoundTripThroughTheStoredShape() {
        let (store, _, defaults) = fresh()
        RpcEndpoints.saveBans(
            [["url": "https://x.test", "banned_at_ms": 1_700_000_000_000, "permanent": true]],
            store: store
        )
        let raw = defaults.string(forKey: RpcEndpoints.banStorageKey) ?? ""
        #expect(raw.contains("bannedAt"), "stored: \(raw)")
        #expect(!raw.contains("banned_at_ms"))

        let read = RpcEndpoints.loadBans(store: store)
        #expect(read.first?["url"] as? String == "https://x.test")
        #expect(read.first?["banned_at_ms"] as? Double == 1_700_000_000_000)
        #expect(read.first?["permanent"] as? Bool == true)
    }

    /// Corrupt reads as nothing banned. The safe direction: the core re-bans
    /// anything still broken on its first failure, whereas a phantom ban hides
    /// a working endpoint with nothing to clear it.
    @Test func aCorruptBanMapReadsAsNothingBanned() {
        let (store, _, defaults) = fresh()
        defaults.set("{{{", forKey: RpcEndpoints.banStorageKey)
        #expect(RpcEndpoints.loadBans(store: store).isEmpty)
    }

    /// A call before `boot()` is REFUSED, not queued.
    ///
    /// `CoreStore` drops events sent before a machine's first one — on purpose,
    /// since every machine reads its stores on boot — so a routed call made
    /// first would wait on a continuation nothing will ever resume. That is not
    /// a slow call, it is a hang for the life of the process: a spinner with no
    /// explanation on screen, and a test suite that never finishes. This suite
    /// found it that way.
    @Test func aCallBeforeBootIsRefusedRatherThanHanging() async {
        let (store, accounts, _) = fresh()
        let pool = RpcPool(store: store, accounts: accounts)
        #expect(!pool.booted)

        let outcome = await pool.call(chainId: 100, method: "eth_blockNumber")
        guard case .failed(let rateLimited) = outcome else {
            Issue.record("an unbooted pool answered \(outcome)")
            return
        }
        #expect(!rateLimited, "nothing was asked, so nothing throttled us")
        #expect(await pool.bestRpcUrl(chainId: 100) == nil)
    }
}
