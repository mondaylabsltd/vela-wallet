//
//  Issue483Tests.swift
//  VelaWalletTests
//
//  Issue #483: a dApp's network fee stuck on "Can't reach Polygon" / "Tap to
//  retry" until the app was killed. Measured on a simulator before the fix
//  (`Issue483DeviceTests`): a text-size change re-ran `RootView.init`, which
//  built a second request pool, relay and spine; `@State` kept the first pool
//  (booted), the plain `let`s took the second (never booted), and the dApp
//  sheet read through the second — `rpc: before_boot` on every fee read.
//
//  What these hold, hermetically:
//
//  - the root's init builds nothing and reads no observable state, so a
//    Settings change neither re-runs it for a reason nor gives it anything to
//    build when SwiftUI re-runs it anyway;
//  - every object on the money path is the graph's, over the graph's one
//    booted pool — through re-inits and a settings change;
//  - the trusted page (spec 102) is the graph's one, and a re-init no longer
//    re-points the "trust this version" hook at an orphan settings machine;
//  - a pool asked before anyone booted it boots and routes the call
//    (`late_boot`), never refuses it as if the chain were down.
//

import Foundation
import Observation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.serialized)
struct Issue483Tests {

    private func graph(_ defaults: UserDefaults = UserDefaults(suiteName: "483.\(UUID().uuidString)")!) -> AppGraph {
        AppGraph(loc: Loc(overrideTag: nil, preferredLanguages: ["en"]), firstLaunch: false, defaults: defaults)
    }

    /// The graph's hook on `SignerPageChecks.shared` is process-wide; put the
    /// host app's back after a test built its own graph.
    private func keepingTrustHook(_ body: () async throws -> Void) async rethrows {
        let hook = SignerPageChecks.shared.recordTrust
        defer { SignerPageChecks.shared.recordTrust = hook }
        try await body()
    }

    // MARK: - The root builds nothing

    /// The trigger, pinned: building the root registers no dependency on
    /// observable state, so changing the language or the text size does not
    /// invalidate the scene's content. The control beside it shows the test
    /// can see such a dependency — building the graph the way the old root
    /// init did (inside the content's evaluation) registers exactly these.
    @Test func theRootsInitReadsNoObservableState() async {
        await keepingTrustHook {
            let g = graph()
            var invalidated = false
            withObservationTracking {
                _ = RootView(graph: g)
                _ = RootView(graph: g, onErased: {})
            } onChange: {
                invalidated = true
            }
            g.preferences.setTextScale(.large)
            g.preferences.setLanguage("zh")
            g.loc.apply("zh")
            g.preferences.setNumberFormat(.dotComma)
            #expect(!invalidated, "building the root read observable state: a Settings change re-runs it")

            // The control: the old init's reads (`loc.apply(prefs.language)`,
            // `UiScale.apply(prefs)`, the welcome content through `loc.t`) are
            // dependencies, and a Settings change fires them.
            var oldWayInvalidated = false
            let other = graph()
            withObservationTracking {
                other.loc.apply(other.preferences.language)
                UiScale.apply(other.preferences)
            } onChange: {
                oldWayInvalidated = true
            }
            other.preferences.setTextScale(.large)
            #expect(oldWayInvalidated, "the control saw no dependency — the check above would prove nothing")
        }
    }

    /// A re-run of the root's init — any number, around a Settings change —
    /// builds no graph, no pool, no relay: the money path the dApp sheet uses
    /// is the graph's, over the graph's booted pool.
    @Test func reInitsAndASettingsChangeKeepTheDappPathOnTheBootedPool() async {
        await keepingTrustHook {
            let g = graph()
            let built = AppGraph.built
            #expect(g.pool.booted, "the graph's pool answers from its first moment")

            let first = RootView(graph: g)
            g.preferences.setTextScale(.large)
            g.preferences.setLanguage("ja")
            let second = RootView(graph: g)
            let third = RootView(graph: g, onErased: {})

            #expect(AppGraph.built == built, "a root init built a graph")
            for root in [first, second, third] {
                #expect(root.graph === g)
                #expect(root.graph.relay.boundPool === g.pool,
                        "the dApp sheet's relay reads through another pool")
            }
            #expect(g.relay.boundPool === g.pool)
            #expect(g.pool.booted)
        }
    }

    /// RESEARCH §7 Q4: after a re-init, the dApp signature's venue. The page
    /// is the account's own record (storage), so it was never the wrong PAGE
    /// — but the spine's trusted signer was a second instance (two sessions
    /// that could race to present), and the "trust this version" hook was
    /// re-pointed at a settings machine nothing drew. Now: one trusted signer,
    /// the graph's, whatever re-runs; and the venue is the account's.
    @Test func aTrustedSignerSignatureAfterAReInitGoesToTheAccountsPageThroughTheOneSigner() async throws {
        try await keepingTrustHook {
            let fixture = TrustedSignerFixture()
            let defaults = UserDefaults(suiteName: "483.\(UUID().uuidString)")!
            defaults.set("[\(fixture.pageRecordJson)]", forKey: VelaStore.Key.accounts)
            let g = graph(defaults)

            var trustHookReached = false
            SignerPageChecks.shared.recordTrust = { _, _ in trustHookReached = true }
            _ = RootView(graph: g)
            g.preferences.setTextScale(.xlarge)
            _ = RootView(graph: g)

            #expect((g.spine.trustedSigner as AnyObject?) === g.trustedSigner,
                    "the dApp spine signs through another trusted signer")
            #expect((g.onboarding.trustedSigner as AnyObject?) === g.trustedSigner)
            let plan = try #require(await g.spine.plan(account: fixture.account))
            #expect(plan.venue.pageUrl == fixture.signerUrl)

            // Nothing re-pointed the process-wide hook.
            await SignerPageChecks.shared.recordTrust?("https://sign.getvela.app/", "v1")
            #expect(trustHookReached, "a root init re-pointed the trust hook")
        }
    }

    /// One scene: the app declares no second window, so no second root draws
    /// the one graph (and none could build another).
    @Test func theAppDeclaresOneScene() {
        let manifest = Bundle.main.infoDictionary?["UIApplicationSceneManifest"] as? [String: Any]
        #expect(manifest?["UIApplicationSupportsMultipleScenes"] as? Bool == false)
    }

    // MARK: - The pool never refuses for want of a boot

    /// A pool nobody booted — what the re-run init left the dApp sheet with —
    /// boots on its first call and ROUTES it. Before: refused inside the app
    /// (`before_boot`) as a `.failed` the fee read as "can't reach the chain",
    /// for ever — and refused before the routing core ever heard of it, so
    /// `onOutcome` (told only of a call the core concluded) never fired. The
    /// chain here has no endpoint that answers (its index is a refused local
    /// port), so the routed call ends in the core's own verdict, quickly.
    @Test func aCallBeforeBootBootsAndIsRouted() async {
        let defaults = UserDefaults(suiteName: "483.\(UUID().uuidString)")!
        defaults.set(#"{"ethereumDataURL":"https://127.0.0.1:1"}"#, forKey: VelaStore.Key.serviceEndpoints)
        let store = VelaStore(defaults: defaults)
        let orphan = RpcPool(store: store, accounts: AccountStore(defaults: defaults))
        var concluded: [Int] = []
        orphan.onOutcome = { _, chainId in concluded.append(chainId) }
        #expect(!orphan.booted)

        _ = await orphan.call(chainId: 999_483_001, method: "eth_blockNumber")

        #expect(orphan.booted, "the pool did not boot itself")
        #expect(concluded == [999_483_001], "the call never reached the routing core")
    }

    /// The relay over an orphan pool — RESEARCH §4A's reproduction — reads
    /// through the pool instead of failing inside the app: the deployment
    /// read reaches the routing core.
    @Test func aRelayOnAnUnbootedPoolReadsInsteadOfRefusing() async {
        let defaults = UserDefaults(suiteName: "483.\(UUID().uuidString)")!
        defaults.set(#"{"ethereumDataURL":"https://127.0.0.1:1"}"#, forKey: VelaStore.Key.serviceEndpoints)
        let store = VelaStore(defaults: defaults)
        let orphan = RpcPool(store: store, accounts: AccountStore(defaults: defaults))
        var concluded: [Int] = []
        orphan.onOutcome = { _, chainId in concluded.append(chainId) }
        let relay = RelayClient(port: PoolRelayPort(pool: orphan))
        _ = await relay.deploymentRead(chainId: 999_483_002, address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894")
        #expect(orphan.booted)
        #expect(concluded == [999_483_002], "the read was refused inside the app")
    }

    // MARK: - The account read is the fee's own

    /// The fee machine asks for the account read (`read_deployment`) and the
    /// executor answers each of its three cases on the wire the core reads.
    @Test func theDeploymentReadAnswersTheFeeMachineInItsThreeCases() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let executor = FeeExecutor(relay: relay, accounts: ScriptedAccounts())
        let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

        let read = await executor.perform(["type": "read_deployment", "chain_id": 100, "account": account])
        #expect(read.contains(#""type":"deployment""#) && read.contains(#""deployed":true"#), "\(read)")

        port.rpc["eth_getCode"] = .failed(rateLimited: true)
        let limited = await executor.perform([
            "type": "read_deployment", "chain_id": 137, "account": account, "fresh": true,
        ])
        #expect(limited.contains(#""unreachable""#) && limited.contains(#""rate_limited":true"#), "\(limited)")

        let internalRead = await executor.perform(["type": "read_deployment", "chain_id": 137, "account": "nope"])
        #expect(internalRead.contains(#""internal""#), "\(internalRead)")
        #expect(FeeExecutor.operations.contains("read_deployment"))
    }

    /// `fresh` is a real new read: a deployed account is held for good, but a
    /// retry after a failure asks the chain again.
    @Test func aFreshReadAsksTheChainAgain() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        #expect(await relay.deploymentRead(chainId: 100, address: account) == .deployed(true))
        #expect(await relay.deploymentRead(chainId: 100, address: account) == .deployed(true))
        #expect(port.calls.filter { $0 == "eth_getCode" }.count == 1, "held after the first")
        #expect(await relay.deploymentRead(chainId: 100, address: account, fresh: true) == .deployed(true))
        #expect(port.calls.filter { $0 == "eth_getCode" }.count == 2, "a fresh read went to the chain")

        port.rpc["eth_gasPrice"] = .ok("0x1")
        _ = await relay.gasSignals(chainId: 100, wantTip: false)
        _ = await relay.gasSignals(chainId: 100, wantTip: false)
        let held = port.calls.filter { $0 == "eth_gasPrice" }.count
        _ = await relay.gasSignals(chainId: 100, wantTip: false, fresh: true)
        #expect(port.calls.filter { $0 == "eth_gasPrice" }.count == held + 1, "a fresh reading was measured again")
    }

    /// An internal fault is never told as the chain being down: the core's
    /// `internal` has its own sentence, and it retries on the schedule.
    @Test func anInternalFaultIsNeverCantReachTheChain() {
        let en = Loc(overrideTag: "en", preferredLanguages: [])
        let key = feeFailureReasonKey(failure: "internal")
        #expect(key == "componentsUi.gas.reasonInternal")
        let sentence = en.t(key ?? "", vars: ["chain": "Polygon"])
        #expect(!sentence.contains("Polygon") && !sentence.lowercased().contains("can't reach"), "\(sentence)")
        #expect(feeRequoteDelayMs(failure: "internal", attempt: 1) == 3_000)
        let down = feeFailureReasonKey(failure: FeeFailureText.chainRead(rateLimited: false).text)
        #expect(down == "componentsUi.gas.reasonChainDown")
        #expect(en.t(down ?? "", vars: ["chain": "Polygon"]).contains("Polygon"))
    }
}
