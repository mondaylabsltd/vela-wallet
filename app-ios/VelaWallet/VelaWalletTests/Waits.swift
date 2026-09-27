//
//  Waits.swift
//  VelaWalletTests
//
//  How a test in this target waits: on STATE, never on the clock.
//
//  Three CI runs in a row (2026-09-27/28) each failed a different unit test
//  that never failed on a laptop, and every one had a clock under its
//  assertion: a five-second `settle`, a stubbed URLSession's 15 s timeout, a
//  count of 25 ms sleeps. The runner is three cores shared by a hundred
//  suites that all want the one main actor; a test that takes 0.1 s here took
//  57 s there. A clock measures how busy the machine was, not whether the code
//  did its job.
//
//  So a wait ends when what it waits for is true — or, when that depends on a
//  core machine, when the machine has nothing left in flight (`CoreDriver`'s
//  `isIdle`), because then only a new event could make it true and waiting
//  longer cannot change the answer. A wait for something that never comes is
//  a hang, and the hang is the suite's `timeLimit`'s to report, not an
//  assertion's to half-see. `MainActorLoadTests` reproduces the runner.
//

import Foundation

@MainActor
enum Wait {
    /// Until `done` holds, or `idle` says nothing is left in flight that could
    /// still make it hold.
    static func until(_ done: () -> Bool, orIdle idle: () -> Bool) async {
        while !done(), !idle() {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    /// The same, for a condition that has to ask an actor (storage, mostly:
    /// the core shows a value before the write it asked for has landed).
    static func until(_ done: () async -> Bool, orIdle idle: () -> Bool) async {
        while await !done(), !idle() {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    /// Until `done` holds, however long that takes: for what no machine's
    /// idleness bounds — a test's own port being called, a task reaching the
    /// point the test holds it at.
    static func until(_ done: () -> Bool) async {
        while !done() {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }
}

/// Raised once by a task, read by a wait: "the call I started has returned".
@MainActor
final class Flag {
    private(set) var isSet = false
    func set() { isSet = true }
}
