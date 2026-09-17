//
//  NetworkAdminLiveTests.swift
//  VelaWalletTests
//
//  The add-network wizard against the real world.
//
//  Everything else in this target is hermetic. This file is not: it fetches the
//  live chain index, resolves a chain, races real RPC endpoints and reads real
//  contract code. That is the only way to know the probe pipeline works, and it
//  is exactly why it must not run in an ordinary test pass — a flaky network
//  would make an unrelated change look broken.
//
//  So it is behind a compile flag:
//
//      xcodebuild test -project VelaWallet.xcodeproj -scheme VelaWallet \
//        -destination 'id=<simulator udid>' \
//        OTHER_SWIFT_FLAGS='$(inherited) -DVELA_LIVE_TESTS'
//
//  It asserts on the SHAPE the pipeline reaches, never on a particular chain's
//  answer: "the core got far enough to have a verdict" is durable, while "Zora
//  is compatible" is a fact about somebody else's deployment.
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

/// `.serialized` because these tests are `@MainActor` and each one *polls* the
/// main actor while waiting for the effect loop. Run in parallel they starve
/// each other, and the wizard — which needs a debounce, an index fetch, a chain
/// resolve and an RPC race to all complete — is the one that loses. It failed
/// alongside its siblings and passed alone, which is the signature.
@MainActor
@Suite(.serialized)
struct NetworkAdminLiveTests {

    private func settle(
        until condition: @escaping () -> Bool,
        seconds: Double = 40
    ) async {
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            if condition() { return }
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
    }

    /// Typing a **built-in** chain's id gets the core's refusal, not a verdict.
    ///
    /// This test used to type 100 and demand a compatibility verdict, which was
    /// the assertion being wrong rather than the app: Gnosis is a built-in, so
    /// `already_added` is the correct and only answer. Asking the core to probe
    /// a chain it has just refused is asking it to contradict itself.
    @Test func typingABuiltinChainIdIsRefusedAsAlreadyAdded() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let shelf = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let store = SettingsStore(store: shelf, accounts: accounts,
                                  pool: RpcPool(store: shelf, accounts: accounts))
        store.open()
        await settle(until: { store.isLoaded })

        store.search("100")
        await settle(until: { !(store.networkAdmin?.wizard.suggestions.isEmpty ?? true) },
                     seconds: 15)
        store.selectChain(100)
        await settle(until: { store.networkAdmin?.wizard.error != nil }, seconds: 20)

        #expect(store.networkAdmin?.wizard.error == .alreadyAdded(chainId: 100))
        #expect(store.networkAdmin?.wizard.canAdd == false,
                "the gate must stay shut on a chain that is already there")
    }

    /// Type a chain id, and the core reaches a verdict it did not invent.
    ///
    /// Zora (7777777) because it is **not** a built-in, so the wizard actually
    /// runs: index → resolve → RPC race → eleven `eth_getCode` reads → P256.
    /// Whether it comes back compatible is a fact about somebody else's
    /// deployment, so only the *arrival* of a verdict is asserted.
    @Test func typingAChainIdReachesARealVerdict() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let shelf = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let store = SettingsStore(store: shelf, accounts: accounts,
                                  pool: RpcPool(store: shelf, accounts: accounts))
        store.open()
        await settle(until: { store.isLoaded })
        #expect(store.isLoaded, "the store never loaded")

        store.search("7777777")
        await settle(until: { !(store.networkAdmin?.wizard.suggestions.isEmpty ?? true) },
                     seconds: 15)
        let suggestions = store.networkAdmin?.wizard.suggestions ?? []
        let phase = String(describing: store.networkAdmin?.wizard.phase)
        let query = store.networkAdmin?.wizard.query ?? "?"
        #expect(!suggestions.isEmpty, "no suggestions; phase = \(phase), query = \(query)")

        store.selectChain(7_777_777)
        await settle(until: { store.networkAdmin?.wizard.compat != nil }, seconds: 90)

        let compat = store.networkAdmin?.wizard.compat
        let reached = String(describing: store.networkAdmin?.wizard.phase)
        let failed = String(describing: store.networkAdmin?.wizard.error)
        #expect(compat != nil, "no verdict; phase = \(reached), error = \(failed)")
        // A verdict means the RPC race finished and eleven contracts were read.
        #expect(compat?.contracts.count == 11)
        #expect(compat?.bestRpcUrl != nil, "no endpoint won the race")
    }

    /// The chain index fetch, on its own — so a failure upstream of the core
    /// is distinguishable from the core never asking.
    @Test func theChainIndexFetchReturnsChains() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let executor = NetworkAdminExecutor(store: VelaStore(defaults: defaults),
                                            accounts: AccountStore(defaults: defaults))
        let reply = try? CoreJSON.object(await executor.perform(["type": "fetch_search_index"]))
        let chains = reply?["chains"] as? [[String: Any]] ?? []
        #expect(chains.count > 100, "the index returned \(chains.count) chains")
        #expect(chains.first?["chain_id"] as? Int == 1)
    }

    /// **The pool routes a real call, and the answer is the golden Safe's own
    /// balance.**
    ///
    /// `0x88cCA0…6894` is the multi-key Safe every client's read path is
    /// checked against; its Gnosis balance is independently verifiable with one
    /// `eth_getBalance` from a terminal, which is what makes it a fixture worth
    /// having.
    ///
    /// Note what the caller does NOT do: name a URL. Which endpoint, after
    /// which failure, under which ban is one decision and `rpc_pool` owns it —
    /// the whole point of spec 051.
    @Test func theGoldenSafesGnosisBalanceComesBackThroughThePool() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let pool = RpcPool(store: VelaStore(defaults: defaults),
                           accounts: AccountStore(defaults: defaults))
        pool.boot()

        let outcome = await pool.call(
            chainId: 100,
            method: "eth_getBalance",
            params: ["0x88cCA0EeDbF2C4426110bbFc998F048689266894", "latest"]
        )

        guard case .ok(let result) = outcome else {
            Issue.record("the pool failed the whole sweep: \(outcome)")
            return
        }
        let hex = result as? String ?? ""
        #expect(hex.hasPrefix("0x"), "not a quantity: \(hex)")
        // A balance, not a claim about its size: the figure moves when the
        // founder sends from it, and a test that pinned it would fail for the
        // wrong reason.
        let wei = UInt64(hex.dropFirst(2), radix: 16)
        #expect(wei != nil, "unparseable quantity: \(hex)")
        print("[live] golden Safe on Gnosis: \(hex)")
    }

    /// A chain with no endpoints anywhere fails — and says whether it was
    /// rate-limited, because invariant ④ forbids offering "swap in your own
    /// RPC" to somebody who is merely being throttled.
    ///
    /// The id matters. This test first used 999999999 on the assumption that
    /// nobody serves it; the chain index **does**, and it answered `eth_chainId`
    /// with `0x3b9ac9ff`. That was the test being wrong about the world. A
    /// `u32`-max id is one the registry genuinely has no row for.
    @Test func aChainWithNoEndpointsFailsRatherThanHanging() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let pool = RpcPool(store: VelaStore(defaults: defaults),
                           accounts: AccountStore(defaults: defaults))
        pool.boot()

        let nowhere = 4_294_967_294
        let outcome = await pool.call(chainId: nowhere, method: "eth_chainId")
        guard case .failed(let rateLimited) = outcome else {
            Issue.record("expected a failure for a chain nobody serves, got \(outcome)")
            return
        }
        #expect(!rateLimited, "nothing answered, so nothing rate-limited us")
        #expect(pool.failedChains.contains(nowhere))
    }

    /// The `/api/health` probe reaches a real service and comes back with an
    /// identity rather than a failure.
    @Test func theServiceHealthProbeReachesTheRelay() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let executor = NetworkAdminExecutor(store: VelaStore(defaults: defaults),
                                            accounts: AccountStore(defaults: defaults))
        let reply = try? CoreJSON.object(await executor.perform([
            "type": "fetch_service_health",
            "field": "bundler_service",
            "base_url": "https://vela-relay-cf.getvela.app",
        ]))
        let body = reply?["body"] as? [String: Any]
        #expect(body?["type"] as? String == "identity",
                "the relay answered \(body ?? [:])")
    }
}

#endif
