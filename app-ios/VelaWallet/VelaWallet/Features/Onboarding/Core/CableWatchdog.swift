//
//  CableWatchdog.swift
//  VelaWallet
//
//  The caBLE channels' read watchdog: fail a read that outlives the ceremony
//  budget (the person approving on the other phone), and never fire once the
//  read it guards has finished.
//
//  The second half is the whole point. Cancelling a sleeping Task makes
//  `Task.sleep` throw AT ONCE, and a `try?` that swallows that falls through to
//  the expiry — a "cancelled" watchdog that kills the connection the instant a
//  frame arrives. That shipped twice: on the BLE channel (device-found
//  2026-08-28, 51 ms from success to teardown) and then on the WebSocket tunnel,
//  where every phone sign-in failed with NSURLErrorCancelled (issue #446).
//

import Foundation

enum CableWatchdog {
    /// The ceremony's budget for one read: 130 s, the time a person may take
    /// to approve on the other device.
    static let ceremonyBudget: UInt64 = 130_000_000_000

    /// Runs `onExpire` on the main actor after `nanoseconds`, unless the
    /// returned task is cancelled first.
    @discardableResult
    static func start(
        after nanoseconds: UInt64,
        _ onExpire: @escaping @MainActor () -> Void
    ) -> Task<Void, Never> {
        Task { @MainActor in
            do {
                try await Task.sleep(nanoseconds: nanoseconds)
            } catch {
                return
            }
            guard !Task.isCancelled else { return }
            onExpire()
        }
    }
}
