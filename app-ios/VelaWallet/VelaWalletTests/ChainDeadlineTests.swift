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
    @Test(.timeLimit(.minutes(1)))
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
    @Test(.timeLimit(.minutes(1)))
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
    @Test(.timeLimit(.minutes(1)))
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
    @Test(.timeLimit(.minutes(1)))
    func theDeadlineFiresWhileTheMainActorIsHeld() async {
        let hold = Task { @MainActor in
            let end = Date().addingTimeInterval(3)
            while Date() < end {}
        }
        let started = Date()
        let result = await TokenReads.bounded(chainId: 56, deadlineMs: 150) {
            await withCheckedContinuation { (_: CheckedContinuation<TokenReads.ChainResult, Never>) in }
        }
        #expect(result.failed)
        #expect(Date().timeIntervalSince(started) < 2.5, "the deadline waited for the main actor")
        await hold.value
    }

    /// The default is the core's number, not one typed into this client.
    @Test func theDefaultDeadlineIsTheCores() {
        #expect(balanceChainReadDeadlineMs() == 18_000)
    }
}
