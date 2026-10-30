//
//  PerfPathTests.swift
//  VelaWalletTests
//
//  Native parity Phase 4's shortcuts, pinned to what they must never change:
//
//  - `ViewBytes` finds a core answer's view by its bytes — including strings
//    holding braces, commas, quotes and backslashes — and cuts an ignored
//    top-level field out wherever it stands;
//  - a store that skips unchanged views still hears every CHANGED one, from
//    the real core, and a byte-identical one is not decoded again;
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

    // MARK: - ViewBytes

    @Test func theViewIsFoundByItsBytes() throws {
        let text = #"{"view":{"a":"x,}]\"{","b":[1,{"c":null}]},"effects":[{"id":1}],"cancelled_effect_ids":[]}"#
        let split = try #require(ViewBytes.split(text))
        #expect(String(decoding: split.comparable, as: UTF8.self) == #"{"a":"x,}]\"{","b":[1,{"c":null}]}"#)
        #expect(split.rest == #"{"effects":[{"id":1}],"cancelled_effect_ids":[]}"#)
        // What is left parses, and is what the whole answer said.
        let rest = try CoreJSON.object(split.rest)
        let whole = try CoreJSON.object(text)
        #expect(NSDictionary(dictionary: rest).isEqual(to: whole.filter { $0.key != "view" }))
    }

    @Test func aBackslashBeforeTheClosingQuoteIsItsOwn() throws {
        let text = #"{"view":{"path":"C:\\","n":1},"effects":[]}"#
        let split = try #require(ViewBytes.split(text))
        #expect(String(decoding: split.comparable, as: UTF8.self) == #"{"path":"C:\\","n":1}"#)
    }

    @Test func anIgnoredFieldIsCutWhereverItStands() throws {
        let a = #"{"view":{"banned":[],"pending_urls":{"1":["https://a,b"]},"z":[2]},"effects":[]}"#
        let b = #"{"view":{"banned":[],"pending_urls":{},"z":[2]},"effects":[]}"#
        let c = #"{"view":{"banned":[],"pending_urls":{},"z":[3]},"effects":[]}"#
        let last = #"{"view":{"banned":[],"pending_urls":{"9":[]}},"effects":[]}"#
        let keys = ["pending_urls"]
        let sa = try #require(ViewBytes.split(a, ignoring: keys))
        let sb = try #require(ViewBytes.split(b, ignoring: keys))
        let sc = try #require(ViewBytes.split(c, ignoring: keys))
        #expect(sa.comparable == sb.comparable, "only the ignored field moved")
        #expect(sa.comparable != sc.comparable, "a field that is read moved")
        #expect(ViewBytes.split(last, ignoring: keys) != nil)
        // A key that merely CONTAINS the name, or the name inside a string, is
        // not the field.
        let decoy = #"{"view":{"note":"\"pending_urls\":1","x_pending_urls":1},"effects":[]}"#
        let sd = try #require(ViewBytes.split(decoy, ignoring: keys))
        #expect(String(decoding: sd.comparable, as: UTF8.self) == #"{"note":"\"pending_urls\":1","x_pending_urls":1}"#)
    }

    @Test func anotherShapeIsLeftToTheParser() {
        #expect(ViewBytes.split(#"{"effects":[],"view":{}}"#) == nil)
        #expect(ViewBytes.split(#"{"view":{"a":1}"#) == nil, "truncated")
        #expect(ViewBytes.split("") == nil)
    }

    // MARK: - CoreStore, skipping unchanged views

    /// A machine that answers the same view twice, then a changed one.
    private final class Echo: CoreBridge {
        var views: [String]
        var served = 0
        init(_ views: [String]) { self.views = views }
        func dispatch(eventJson: String) throws -> String { next() }
        func resolveEffect(effectId: UInt64, resultJson: String) throws -> String { next() }
        func view() throws -> String { "{}" }
        private func next() -> String {
            defer { served += 1 }
            let view = views[min(served, views.count - 1)]
            return #"{"view":"# + view + #","effects":[],"cancelled_effect_ids":[]}"#
        }
    }

    private struct Wire: Decodable, Equatable { let n: Int }

    @Test func aStoreThatSkipsHearsEveryChangeAndNoRepeat() {
        let bridge = Echo([#"{"n":1}"#, #"{"n":1}"#, #"{"n":2}"#, #"{"n":2}"#, #"{"n":1}"#])
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

    @Test func aStoreThatDoesNotSkipHearsEveryView() {
        let bridge = Echo([#"{"n":1}"#])
        var heard = 0
        let store = CoreStore<Wire>(bridge: bridge, perform: { _ in "{}" }, onView: { _ in heard += 1 })
        store.boot(#"{"type":"x"}"#)
        for _ in 0..<3 { store.dispatch(#"{"type":"x"}"#) }
        #expect(heard == 4)
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
