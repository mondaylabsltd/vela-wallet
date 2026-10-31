//
//  PerfPathTests.swift
//  VelaWalletTests
//
//  Native parity Phase 4's shortcuts, pinned to what they must never change:
//
//  - every store opts its machine in to the core's unchanged views
//    (core-perf-1): an answer without a view is the one held, a store that
//    skips hears every CHANGED view and no repeat, one that does not hears
//    every answer, and volatile fields are folded back for a driver that
//    asked for them;
//  - `Loc`'s memo answers the engine's words, and a language change is heard;
//  - `PerfSeed` writes the shapes the stores read.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct PerfPathTests {

    // MARK: - CoreStore, the core's unchanged views (core-perf-1)

    /// A machine that answers what it is scripted to — each answer's text
    /// whole, so a test can hand over the core's own shapes: a view, none
    /// ("keep the one you hold"), a volatile part alone.
    private final class Echo: CoreBridge {
        var answers: [String]
        var served = 0
        private(set) var optedIn: Bool?
        init(_ answers: [String]) { self.answers = answers }
        func dispatch(eventJson: String) throws -> String { next() }
        func resolveEffect(effectId: UInt64, resultJson: String) throws -> String { next() }
        func view() throws -> String { "{}" }
        func skipUnchangedViews(includeVolatile: Bool) throws { optedIn = includeVolatile }
        private func next() -> String {
            defer { served += 1 }
            return answers[min(served, answers.count - 1)]
        }

        static func view(_ view: String) -> String {
            #"{"view":"# + view + #","view_version":1,"effects":[],"cancelled_effect_ids":[]}"#
        }
        static let same = #"{"view_version":1,"effects":[],"cancelled_effect_ids":[]}"#
    }

    private struct Wire: Decodable, Equatable { let n: Int }

    @Test func everyStoreOptsInWithoutVolatileFields() {
        let bridge = Echo([Echo.view(#"{"n":1}"#)])
        _ = CoreStore<Wire>(bridge: bridge, perform: { _ in "{}" })
        #expect(bridge.optedIn == false, "the store never asked the core to leave unchanged views out")
    }

    @Test func aStoreThatSkipsHearsEveryChangeAndNoRepeat() {
        let bridge = Echo([
            Echo.view(#"{"n":1}"#), Echo.same, Echo.view(#"{"n":2}"#), Echo.same, Echo.view(#"{"n":1}"#),
        ])
        var heard: [Int] = []
        let store = CoreStore<Wire>(
            bridge: bridge, perform: { _ in "{}" }, onView: { heard.append($0.n) },
            keepsJson: true, skipsUnchanged: true
        )
        store.boot(#"{"type":"x"}"#)
        for _ in 0..<4 { store.dispatch(#"{"type":"x"}"#) }
        #expect(heard == [1, 2, 1])
        #expect(store.view == Wire(n: 1))
        #expect(store.json == #"{"n":1}"#)
    }

    /// A store that resumes waiters or counts commits hears every answer —
    /// the one it holds, again, for an answer that carries none.
    @Test func aStoreThatDoesNotSkipHearsEveryAnswer() {
        let bridge = Echo([Echo.view(#"{"n":1}"#), Echo.same, Echo.same, Echo.view(#"{"n":3}"#)])
        var heard: [Int] = []
        let store = CoreStore<Wire>(bridge: bridge, perform: { _ in "{}" }, onView: { heard.append($0.n) })
        store.boot(#"{"type":"x"}"#)
        for _ in 0..<3 { store.dispatch(#"{"type":"x"}"#) }
        #expect(heard == [1, 1, 1, 3])
    }

    /// A machine's volatile fields, for a driver that asked for them, come
    /// apart and are folded back into the view it holds.
    @Test func volatileFieldsAreFoldedBackIntoTheView() {
        let bridge = Echo([
            #"{"view":{"a":1},"view_version":1,"volatile":{"p":{}},"effects":[],"cancelled_effect_ids":[]}"#,
            #"{"view_version":1,"volatile":{"p":{"1":["u"]}},"effects":[],"cancelled_effect_ids":[]}"#,
            Echo.same,
            #"{"view":{"a":2},"view_version":2,"effects":[],"cancelled_effect_ids":[]}"#,
        ])
        var views: [[String: Any]] = []
        let driver = CoreDriver(bridge: bridge, perform: { _ in "{}" }, onView: { views.append($0) },
                                includeVolatile: true)
        driver.skipsUnchangedView = true
        #expect(bridge.optedIn == true)
        for _ in 0..<4 { driver.dispatch(#"{"type":"x"}"#) }
        #expect(views.count == 3, "an answer with neither moved nothing")
        #expect(views.map { $0["a"] as? Int } == [1, 1, 2])
        #expect((views[1]["p"] as? [String: Any])?["1"] != nil, "the volatile part was not folded in")
        #expect((views[2]["p"] as? [String: Any])?["1"] != nil, "a new view lost the volatile part it holds")
    }

    /// The real RPC pool, opted in without its volatile `pending_urls`:
    /// a call's steps are not a changed view.
    @Test func theRealPoolSendsNoViewForACallsSteps() throws {
        let core = RpcPoolCore()
        try core.skipUnchangedViews(includeVolatile: false)
        let booted = try CoreJSON.object(core.dispatch(eventJson: #"{"type":"bans_loaded","entries":[]}"#))
        let view = try #require(booted["view"] as? [String: Any], "the first answer carries the view")
        #expect(view["pending_urls"] == nil, "a volatile field reached a shell that did not ask for it")
        #expect(booted["volatile"] == nil)
        // The same event again changes nothing: no view crosses.
        let again = try CoreJSON.object(core.dispatch(eventJson: #"{"type":"bans_loaded","entries":[]}"#))
        #expect(again["view"] == nil)
        #expect(again["view_version"] as? Int == booted["view_version"] as? Int)
    }

    // MARK: - RecipientIdentity, the feed's turn

    /// Who is being looked up right now, by the calls in flight.
    @MainActor
    final class Inflight {
        private var open: [String: Int] = [:]
        private(set) var peak = 0
        private(set) var calls = 0
        private(set) var seen: Set<String> = []

        func answer(_ params: [Any]) async -> RpcOutcome {
            calls += 1
            // Whose lookup this call is part of: the task's own mark.
            let who = Inflight.who
            seen.insert(who)
            open[who, default: 0] += 1
            peak = max(peak, open.count)
            try? await Task.sleep(nanoseconds: 3_000_000)
            open[who]! -= 1
            if open[who] == 0 { open[who] = nil }
            return .failed(rateLimited: false)
        }

        static let addresses = (1...24).map { "0x" + String(format: "%040x", 0xabc000 + $0) }
        @TaskLocal nonisolated static var who = ""
    }

    private func identity(_ inflight: Inflight) -> RecipientIdentity {
        let defaults = UserDefaults(suiteName: "perf-identity-\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        return RecipientIdentity(
            store: store, pool: RpcPool(store: store, accounts: accounts, offline: true), accounts: accounts,
            chainCall: { _, _, params in await inflight.answer(params) }
        )
    }

    /// The feed's 24 counterparties are looked up four at a time, and every
    /// one of them is looked up.
    @Test func theFeedsLookupsTakeTurns() async {
        let inflight = Inflight()
        let identity = identity(inflight)
        await withTaskGroup(of: Void.self) { group in
            for address in Inflight.addresses {
                group.addTask { @MainActor in
                    await Inflight.$who.withValue(address) { _ = await identity.resolve(address, queued: true) }
                }
            }
        }
        #expect(inflight.calls > 0)
        #expect(inflight.peak <= RecipientIdentity.concurrentLookups,
                "\(inflight.peak) addresses were looked up at once")
        // Every address was asked about: none was dropped while it waited.
        #expect(inflight.seen == Set(Inflight.addresses))
    }

    /// A lookup somebody is waiting on is not queued behind the feed's.
    @Test func aLookupSomebodyWaitsOnDoesNotQueue() async {
        let inflight = Inflight()
        let identity = identity(inflight)
        await withTaskGroup(of: Void.self) { group in
            for address in Inflight.addresses {
                group.addTask { @MainActor in
                    await Inflight.$who.withValue(address) { _ = await identity.resolve(address) }
                }
            }
        }
        #expect(inflight.peak > RecipientIdentity.concurrentLookups)
    }

    // MARK: - Loc

    @Test func theMemoAnswersTheEnginesWordsAndHearsALanguageChange() {
        let loc = Loc(overrideTag: nil, preferredLanguages: ["en"])
        let first = loc.t("common.done")
        #expect(first == loc.t("common.done"))
        #expect(first != "common.done")
        loc.apply("zh")
        #expect(loc.t("common.done") != first, "the memo outlived its language")
        #expect(loc.t("common.done") == Loc(overrideTag: "zh", preferredLanguages: []).t("common.done"))
    }

    // MARK: - PerfSeed

    @Test func theSeedWritesWhatTheStoresRead() throws {
        let defaults = try #require(UserDefaults(suiteName: "perf-seed-\(UUID().uuidString)"))
        let store = VelaStore(defaults: defaults)
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        #expect(PerfSeed.apply(environment: ["VELA_PERF_SEED": "200", "VELA_PERF_ADDRESS": address], store: store))
        #expect(TxRecords.load(store: store).count == 200)
        #expect(HoldingsCache.load(address: address, space: "mainnet", store: store).count == PerfSeed.holdings.count)
        #expect(!PerfSeed.apply(environment: ["VELA_PERF_SEED": "200"], store: store), "no address, nothing written")
    }
}
