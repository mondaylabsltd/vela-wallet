//
//  LaunchReplayTests.swift
//  VelaWalletTests
//
//  The launch lockup's replay rule is the core's (core-perf-1,
//  `vela_core::launch`): on a first start, then not again for a week — what
//  the web and the desktop always did, where iOS played it on every cold
//  start. `LaunchAnimation.claim` asks the core, writes the time it played
//  under the shared key, and is held here to the core's own vectors.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite
struct LaunchReplayTests {

    private func store() -> VelaStore {
        VelaStore(defaults: UserDefaults(suiteName: "launch-replay-\(UUID().uuidString)")!)
    }

    private static let vectors = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/launch/launch-animation.json")

    private struct Vectors: Decodable {
        let storage_key: String
        let cases: [Case]
        struct Case: Decodable {
            let name: String
            let last_played_ms: Double?
            let now_ms: Double
            let disabled: Bool
            let due: Bool
        }
    }

    /// Every vector, through the shell's own path: what is stored, as text,
    /// under the key; the decision; and the time written when it plays.
    @Test func theShellFollowsTheCoresVectors() throws {
        let vectors = try JSONDecoder().decode(Vectors.self, from: Data(contentsOf: Self.vectors))
        #expect(launchAnimationStorageKey() == vectors.storage_key)
        for vector in vectors.cases {
            let store = store()
            if let last = vector.last_played_ms, last.isFinite {
                store.writeString(vectors.storage_key, String(Int64(last)))
            }
            let now = Date(timeIntervalSince1970: vector.now_ms / 1000)
            let played = LaunchAnimation.claim(store: store, now: now, disabled: vector.disabled)
            #expect(played == vector.due, "\(vector.name)")
            let stored = store.readString(vectors.storage_key)
            if vector.due {
                #expect(stored == String(Int64(vector.now_ms)), "\(vector.name): the time it played was not written")
            } else {
                #expect(stored == vector.last_played_ms.map { String(Int64($0)) }, "\(vector.name): a start that did not play wrote")
            }
        }
    }

    /// The point of the rule: the second cold start of the day is quiet, and
    /// a week later it plays again.
    @Test func itPlaysOnceAWeek() {
        let store = store()
        let monday = Date(timeIntervalSince1970: 1_700_000_000)
        #expect(LaunchAnimation.claim(store: store, now: monday, disabled: false))
        #expect(!LaunchAnimation.claim(store: store, now: monday.addingTimeInterval(60), disabled: false))
        #expect(!LaunchAnimation.claim(store: store, now: monday.addingTimeInterval(6 * 86_400), disabled: false))
        #expect(LaunchAnimation.claim(store: store, now: monday.addingTimeInterval(7 * 86_400), disabled: false))
    }

    /// Text that does not read as a number is "never played": a bad write
    /// must not silence the lockup for good.
    @Test func anUnreadableValueIsNeverPlayed() {
        let store = store()
        store.writeString(launchAnimationStorageKey(), "yesterday")
        #expect(LaunchAnimation.claim(store: store, now: Date(), disabled: false))
    }

    /// Turned off (a test's skip): it neither plays nor counts as played.
    @Test func disabledNeitherPlaysNorWrites() {
        let store = store()
        #expect(!LaunchAnimation.claim(store: store, now: Date(), disabled: true))
        #expect(store.readString(launchAnimationStorageKey()) == nil)
    }
}
