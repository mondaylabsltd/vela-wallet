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

    /// Type a chain id, and the core reaches a verdict it did not invent.
    ///
    /// Gnosis (100) because it is the chain the golden Safe lives on, so a
    /// verdict of "not compatible" here would be news rather than noise.
    @Test func typingAChainIdReachesARealVerdict() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = SettingsStore(store: VelaStore(defaults: defaults),
                                  accounts: AccountStore(defaults: defaults))
        store.open()
        await settle(until: { store.isLoaded })
        #expect(store.isLoaded, "the store never loaded")

        store.search("100")
        await settle(until: { !(store.networkAdmin?.wizard.suggestions.isEmpty ?? true) },
                     seconds: 15)
        let suggestions = store.networkAdmin?.wizard.suggestions ?? []
        let phase = String(describing: store.networkAdmin?.wizard.phase)
        let query = store.networkAdmin?.wizard.query ?? "?"
        #expect(!suggestions.isEmpty, "no suggestions; phase = \(phase), query = \(query)")

        store.selectChain(100)
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

    /// The `/api/health` probe reaches a real service and comes back with an
    /// identity rather than a failure.
    @Test func theServiceHealthProbeReachesTheRelay() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let executor = NetworkAdminExecutor(store: VelaStore(defaults: defaults),
                                            accounts: AccountStore(defaults: defaults))
        let reply = try? CoreJSON.object(await executor.perform([
            "type": "fetch_service_health",
            "field": "bundler_service",
            "base_url": "https://vela-relay.getvela.app",
        ]))
        let body = reply?["body"] as? [String: Any]
        #expect(body?["type"] as? String == "identity",
                "the relay answered \(body ?? [:])")
    }
}

#endif
