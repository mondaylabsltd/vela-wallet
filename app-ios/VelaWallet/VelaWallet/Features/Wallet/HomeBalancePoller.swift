//
//  HomeBalancePoller.swift
//  VelaWallet
//
//  The home's 10-minute balance refresh — the web's aggregate poll
//  (`wallet/+page.svelte`, `setInterval(() => balance.refresh(false), 600_000)`).
//  A transfer nothing else notices, a native coin arriving while the home sits
//  open, is at most this stale (issue 188).
//
//  The timer is the shell's; the refresh is the core's. What this owns is only
//  WHEN it runs: while the wallet home is on screen AND the app is active. The
//  web can leave its interval running behind a hidden tab because it tells the
//  core the tab went away; this one stops instead, so a backgrounded phone is
//  not woken to sweep twelve chains.
//
//  One loop at most. The home re-appears every time a flow closes over it, and
//  a second `onAppear` must not start a second timer beside the first.
//

import Foundation

@MainActor
final class HomeBalancePoller {

    /// The core's `AUTO_REFRESH_MS`, as the web polls it.
    static let interval: Duration = .seconds(600)

    private let interval: Duration
    private let sleep: (Duration) async throws -> Void
    private let tick: () -> Void

    private var onScreen = false
    private var active = false
    private var loop: Task<Void, Never>?

    /// Whether a timer is running. For tests; nothing decides on it.
    var isRunning: Bool { loop != nil }

    /// `sleep` is injected so a test can drive the clock instead of waiting ten
    /// minutes; it must throw when its task is cancelled, as `Task.sleep` does.
    init(
        interval: Duration = HomeBalancePoller.interval,
        sleep: @escaping (Duration) async throws -> Void = { try await Task.sleep(for: $0) },
        tick: @escaping () -> Void
    ) {
        self.interval = interval
        self.sleep = sleep
        self.tick = tick
    }

    /// The wallet home came on screen, or went away.
    func homeVisible(_ visible: Bool) {
        onScreen = visible
        reconcile()
    }

    /// The scene became active, or stopped being (inactive or background).
    func sceneActive(_ isActive: Bool) {
        active = isActive
        reconcile()
    }

    private func reconcile() {
        guard onScreen && active else {
            loop?.cancel()
            loop = nil
            return
        }
        guard loop == nil else { return }
        let (interval, sleep, tick) = (interval, sleep, tick)
        loop = Task {
            while !Task.isCancelled {
                do { try await sleep(interval) } catch { return }
                guard !Task.isCancelled else { return }
                tick()
            }
        }
    }
}
