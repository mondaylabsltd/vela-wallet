//
//  ChainDeadlineTests.swift
//  VelaWalletTests
//
//  Spec 092: a chain whose connection is held open (a real mode behind a
//  blocking network) answers nothing at all. Before the core's per-chain
//  deadline reached this client, the balance round waited on it for as long
//  as the pool kept sweeping, and Home never settled.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ChainDeadlineTests {

    /// A read that never answers — the transport held open — is a failed
    /// chain at the deadline, never a rate-limited one, and nothing it held.
    /// Without the deadline this read never ends, so the time limit (not a
    /// wall-clock bound, which a loaded suite makes flaky) is the proof.
    @Test(.timeLimit(.minutes(1)))
    func aReadThatNeverAnswersIsFailedAtTheDeadline() async {
        let started = Date()
        let result = await TokenReads.bounded(chainId: 56, deadlineMs: 150) {
            await withCheckedContinuation { (_: CheckedContinuation<TokenReads.ChainResult, Never>) in }
        }
        #expect(result.chainId == 56)
        #expect(result.failed)
        #expect(!result.rateLimited)
        #expect(result.tokens.isEmpty)
        // Not before the deadline: a slow chain is still given its time.
        #expect(Date().timeIntervalSince(started) >= 0.15)
    }

    /// A chain that answers in time is that answer, untouched.
    @Test func anAnswerInTimeIsTheAnswer() async {
        let result = await TokenReads.bounded(chainId: 1, deadlineMs: 5_000) {
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
                await TokenReads.bounded(chainId: 1, deadlineMs: 150) {
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

    /// The default is the core's number, not one typed into this client.
    @Test func theDefaultDeadlineIsTheCores() {
        #expect(balanceChainReadDeadlineMs() == 18_000)
    }
}
