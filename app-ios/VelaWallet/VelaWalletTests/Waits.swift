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
//  a hang, and the hang is the suite's time limit's to report (`hangLimit`,
//  below), not an assertion's to half-see. `MainActorLoadTests` reproduces
//  the runner.
//
//  The time limit reports it by cancelling the test, so a wait also ends
//  when its task is cancelled: the assertion after it then says what never
//  came. A wait that slept through the cancellation would spin — a cancelled
//  `Task.sleep` throws at once — holding the main actor, and the whole run
//  would hang instead of failing one test.
//
//  A clock can also sit inside the code under test. The fee machine bounds a
//  whole quote at 15 s (spec 094 S9) and the shell ran that timer on the wall
//  clock, so a starved runner failed scripted quotes (2026-10-04, twice). A
//  `FeeStore` under a scripted relay is built with `timers: .stopped`: no
//  time passes, and `isIdle` does not count the timers it holds.
//

import Foundation
import Testing

extension Trait where Self == TimeLimitTrait {
    /// The time limit of every test in this target that has one: how long a
    /// hang may cost — never how long the test may take.
    ///
    /// A limit here cannot be a test's own number, because the clock it runs
    /// on is not the test's. Swift Testing starts every test at once, and
    /// each one's clock with it — while the test stands in line for the one
    /// main actor. `thePolishKeysResolve`, a lookup in a string table, was
    /// "passed after 100.172 seconds" on the runner in one run and "Time
    /// limit was exceeded: 120.000 seconds" in the next (PR 489, 2026-10-10):
    /// it had waited 128 s to be run at all. Every limit that said two
    /// minutes went the same way — 63 tests in eleven files — and Xcode,
    /// which takes the trait as its own execution time allowance, then killed
    /// the test host with whatever was still running in it.
    ///
    /// So the number is one, and it is the run's: well past the longest any
    /// test has stood in that line (157 s, in that run), so that what it
    /// reports is a wait that never ends. A wait that can be TOLD to be
    /// hopeless is told sooner — by its machine's idleness (`Wait.until(_:
    /// orIdle:)`) — and says what it waited for.
    static var hangLimit: Self { .timeLimit(.minutes(5)) }
}

@MainActor
enum Wait {
    /// Until `done` holds, or `idle` says nothing is left in flight that could
    /// still make it hold.
    static func until(_ done: () -> Bool, orIdle idle: () -> Bool) async {
        while !done(), !idle(), !Task.isCancelled {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    /// The same, for a condition that has to ask an actor (storage, mostly:
    /// the core shows a value before the write it asked for has landed).
    static func until(_ done: () async -> Bool, orIdle idle: () -> Bool) async {
        while await !done(), !idle(), !Task.isCancelled {
            try? await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    /// Until `done` holds, however long that takes: for what no machine's
    /// idleness bounds — a test's own port being called, a task reaching the
    /// point the test holds it at.
    static func until(_ done: () -> Bool) async {
        while !done(), !Task.isCancelled {
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
