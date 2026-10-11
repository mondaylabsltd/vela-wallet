//
//  ChainDeadlineTests.swift
//  VelaWalletTests
//
//  Spec 092: a chain whose connection is held open (a real mode behind a
//  blocking network) answers nothing at all. Before the core's per-chain
//  deadline reached this client, the balance round waited on it for as long
//  as the pool kept sweeping, and Home never settled.
//
//  Not `@MainActor`, on purpose (CI #386): the deadline runs off the main
//  actor, so these hold even while the main actor is starved — run them under
//  `MainActorLoadTests` (TEST_RUNNER_VELA_TEST_LOAD_SECONDS) to see it.
//

import Foundation
import os
import Testing
import VelaCore
@testable import VelaWallet

struct ChainDeadlineTests {

    /// A read that never answers — the transport held open — is a failed
    /// chain at the deadline, never a rate-limited one, and nothing it held;
    /// and the read is cancelled, not left running. Without the deadline this
    /// read would not end for an hour, so the time limit is the proof.
    @Test(.hangLimit)
    func aReadThatNeverAnswersIsFailedAtTheDeadline() async {
        let cancelled = OSAllocatedUnfairLock(initialState: false)
        let started = Date()
        let result = await TokenReads.bounded(chainId: 56, deadlineMs: 150) {
            try? await Task.sleep(nanoseconds: 3_600_000_000_000)
            cancelled.withLock { $0 = Task.isCancelled }
            return TokenReads.ChainResult(chainId: 56, tokens: [["late": true]], failed: false, rateLimited: false)
        }
        #expect(result.chainId == 56)
        #expect(result.failed)
        #expect(!result.rateLimited)
        #expect(result.tokens.isEmpty, "the late answer is dropped")
        // Not before the deadline: a slow chain is still given its time.
        #expect(Date().timeIntervalSince(started) >= 0.15)
        // The read the deadline beat is cancelled (its sleep wakes at once).
        let deadline = Date().addingTimeInterval(30)
        while !cancelled.withLock({ $0 }), Date() < deadline {
            try? await Task.sleep(nanoseconds: 10_000_000)
        }
        #expect(cancelled.withLock { $0 })
    }

    /// A chain that answers in time is that answer, untouched.
    @Test(.hangLimit)
    func anAnswerInTimeIsTheAnswer() async {
        // A deadline no loaded runner reaches: this read answers at once.
        let result = await TokenReads.bounded(chainId: 1, deadlineMs: 60_000) {
            TokenReads.ChainResult(chainId: 1, tokens: [["symbol": "ETH"]], failed: false, rateLimited: false)
        }
        #expect(!result.failed)
        #expect(result.tokens.count == 1)
    }

    /// The round as the executor runs it: one chain never answers, the other
    /// does — the round settles at the deadline with both, and the silent one
    /// failed.
    @Test(.hangLimit)
    func aRoundWithAStalledChainStillSettles() async {
        let results = await withTaskGroup(of: TokenReads.ChainResult.self) { group in
            group.addTask {
                await TokenReads.bounded(chainId: 56, deadlineMs: 150) {
                    await withCheckedContinuation { (_: CheckedContinuation<TokenReads.ChainResult, Never>) in }
                }
            }
            group.addTask {
                // The answering chain's verdict must not hang on CPU time:
                // only the silent one is given a short deadline.
                await TokenReads.bounded(chainId: 1, deadlineMs: 60_000) {
                    TokenReads.ChainResult(chainId: 1, tokens: [["symbol": "ETH"]], failed: false, rateLimited: false)
                }
            }
            var all: [TokenReads.ChainResult] = []
            for await result in group { all.append(result) }
            return all
        }
        #expect(results.filter(\.failed).map(\.chainId) == [56])
        #expect(results.first { $0.chainId == 1 }?.tokens.count == 1)
    }

    /// The deadline fires while the main actor is held — it never needed it.
    /// (The executor's own round still collects on the main actor; this is
    /// the part that must not.)
    ///
    /// Proved by ORDER, never by a stopwatch (CI, spec 096: a loaded runner
    /// took 5.15 s against the old `< 2.5 s`). The main actor is taken and
    /// held — not one yield — from before the read starts until the
    /// deadline's answer has arrived. A deadline that needed the main actor
    /// could not answer while it is held, so the hold would never end by
    /// itself.
    ///
    /// Every main-actor test in the run waits behind that hold. It used to
    /// be the time limit that let it go — one minute, when this test had a
    /// minute of its own. The limit is the run's now (`hangLimit`), and a
    /// hold as long as that would take the whole run with it; so the hold
    /// has a bound of its own, counted from the moment it begins
    /// (`longestHold`). Past it the hold lets go, the deadline — the main
    /// actor free again — answers, and the test fails on what it is about:
    /// one failure, and nothing else hangs behind it. A hold that passes is
    /// the deadline's 150 ms; on the runner, 215 ms.
    @Test(.hangLimit)
    func theDeadlineFiresWhileTheMainActorIsHeld() async {
        struct Hold: Sendable {
            var holding = false
            var answered = false
            var abandoned = false
        }
        let hold = OSAllocatedUnfairLock(initialState: Hold())
        // `true` = the answer arrived while this held the main actor; and
        // for how long it was held, which every main-actor test beside this
        // one waited out.
        let holder = Task { @MainActor () -> (answered: Bool, held: TimeInterval) in
            let began = Date()
            hold.withLock { $0.holding = true }
            while true {
                let now = hold.withLock { $0 }
                let held = Date().timeIntervalSince(began)
                if now.answered { return (true, held) }
                if now.abandoned || held > Self.longestHold { return (false, held) }
            }
        }
        // The read runs in a detached task. This target's nonisolated async
        // functions run on their caller's executor (approachable
        // concurrency), so a test body the runner happened to start on the
        // main actor would wait behind the holder's spin for as long as it
        // lasts, and every main-actor test running beside it with it (CI #395).
        let reader = Task.detached { () -> TokenReads.ChainResult in
            // The read starts only once the main actor is held.
            while !hold.withLock({ $0.holding }), !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 1_000_000)
            }
            let result = await TokenReads.bounded(chainId: 56, deadlineMs: 150) {
                await withCheckedContinuation { (_: CheckedContinuation<TokenReads.ChainResult, Never>) in }
            }
            hold.withLock { $0.answered = true }
            return result
        }
        let result = await withTaskCancellationHandler {
            await reader.value
        } onCancel: {
            hold.withLock { $0.abandoned = true }
            reader.cancel()
        }
        #expect(result.failed)
        let held = await holder.value
        print("ChainDeadlineTests: the main actor was held \(Int(held.held * 1000)) ms for a 150 ms deadline")
        #expect(held.answered, "the deadline waited for the main actor")
    }

    /// How long the hold above may last before it gives the main actor back
    /// and fails: hundreds of times what a deadline that fires off the main
    /// actor takes, and well inside the run's limit.
    private static let longestHold: TimeInterval = 60

    /// The default is the core's number, not one typed into this client.
    @Test func theDefaultDeadlineIsTheCores() {
        #expect(balanceChainReadDeadlineMs() == 18_000)
    }
}
