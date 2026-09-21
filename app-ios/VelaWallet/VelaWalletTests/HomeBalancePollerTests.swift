//
//  HomeBalancePollerTests.swift
//  VelaWalletTests
//
//  The home's 10-minute balance refresh: when it runs, and that it runs once.
//  The clock is a hand-cranked one — nothing here waits ten minutes.
//

import Foundation
import Testing
@testable import VelaWallet

/// A clock the test advances. Every sleeper waits until `advance()`; one the
/// poller cancelled throws when it wakes, exactly as `Task.sleep` would.
@MainActor
private final class ManualClock {
    private(set) var requested: [Duration] = []
    private var sleepers: [CheckedContinuation<Void, Never>] = []

    var waiting: Int { sleepers.count }

    func sleep(_ duration: Duration) async throws {
        requested.append(duration)
        await withCheckedContinuation { sleepers.append($0) }
        try Task.checkCancellation()
    }

    func advance() {
        let woken = sleepers
        sleepers = []
        woken.forEach { $0.resume() }
    }
}

@MainActor
struct HomeBalancePollerTests {

    private func settle() async {
        for _ in 0..<20 { await Task.yield() }
    }

    private func poller(_ clock: ManualClock, ticks: @escaping () -> Void) -> HomeBalancePoller {
        HomeBalancePoller(sleep: { try await clock.sleep($0) }, tick: ticks)
    }

    /// Ten minutes, the web's `600_000`, and not forced — the web's
    /// `balance.refresh(false)`.
    @Test func itPollsEveryTenMinutesUnforced() async throws {
        #expect(HomeBalancePoller.interval == .seconds(600))
        let event = try #require(
            JSONSerialization.jsonObject(with: Data(WalletStore.autoRefreshEvent.utf8)) as? [String: Any]
        )
        #expect(event["type"] as? String == "refresh_requested")
        #expect(event["force"] as? Bool == false)
        #expect(event["pull"] as? Bool == false)

        let clock = ManualClock()
        var ticks = 0
        let subject = poller(clock) { ticks += 1 }
        subject.sceneActive(true)
        subject.homeVisible(true)
        await settle()
        #expect(clock.requested == [.seconds(600)])
        #expect(ticks == 0, "nothing on appear — the home's own open already read")

        clock.advance()
        await settle()
        #expect(ticks == 1)
        clock.advance()
        await settle()
        #expect(ticks == 2)
        #expect(clock.waiting == 1, "and it keeps going")
    }

    /// The home shows again every time a flow closes over it: one timer, not
    /// one per appearance.
    @Test func reappearingDoesNotStartASecondTimer() async {
        let clock = ManualClock()
        var ticks = 0
        let subject = poller(clock) { ticks += 1 }
        subject.sceneActive(true)
        subject.homeVisible(true)
        subject.homeVisible(true)
        subject.sceneActive(true)
        await settle()
        #expect(clock.waiting == 1)
        clock.advance()
        await settle()
        #expect(ticks == 1)
    }

    /// Off screen, or not active: no timer, and a sleep already under way is
    /// not allowed to fire.
    @Test func itStopsOffScreenAndInTheBackground() async {
        let clock = ManualClock()
        var ticks = 0
        let subject = poller(clock) { ticks += 1 }

        subject.homeVisible(true)
        await settle()
        #expect(!subject.isRunning, "not active yet: nothing runs")

        subject.sceneActive(true)
        await settle()
        #expect(subject.isRunning)
        subject.sceneActive(false)
        #expect(!subject.isRunning)
        clock.advance()
        await settle()
        #expect(ticks == 0, "a backgrounded app is not polled")

        subject.sceneActive(true)
        await settle()
        subject.homeVisible(false)
        #expect(!subject.isRunning)
        clock.advance()
        await settle()
        #expect(ticks == 0, "a home nobody sees is not polled")

        // Back on screen, back in front: one fresh timer.
        subject.homeVisible(true)
        await settle()
        #expect(clock.waiting == 1)
        clock.advance()
        await settle()
        #expect(ticks == 1)
    }
}
