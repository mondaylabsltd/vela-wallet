//
//  RefreshSpin.swift
//  VelaWallet
//
//  How long the hero's refresh control keeps turning (issue 462).
//
//  The core says when a refresh the person asked for is out
//  (`BalanceView.refreshing`) and leaves the minimum visible spin to the shell
//  (balance_dashboard.rs). Without one, a read answered from warm connections
//  flips true and false inside a frame or two: the tap looks like it did
//  nothing, which is the bug all over again.
//
//  So the spin starts the moment the person taps (before the core's view has
//  even come back), or the moment the core says a refresh is out (a pull, a
//  rescue sheet's retry), and ends only when BOTH the core has finished AND
//  `minimumMs` has passed since it started. While it turns a second tap is
//  refused — `tapped` answers `nil` and nothing is dispatched.
//
//  Pure: times are passed in, so the rule is pinned by a test, and the store
//  only feeds it the core's flag and a clock. Android's `RefreshSpin` is the
//  same rule, line for line.
//

import Foundation

struct RefreshSpin: Equatable {
    /// The minimum visible spin (the core doc's 650 ms, the shells' to hold).
    static let minimumMs: Double = 650

    private(set) var spinning = false
    /// When this spin started — a monotonic clock, in ms.
    private(set) var since: Double = 0
    /// The core's `refreshing`, as last told.
    private(set) var coreBusy = false

    /// The person tapped: the spin starts now — or `nil` while it is already
    /// turning (the control is inert).
    func tapped(now: Double) -> RefreshSpin? {
        guard !spinning else { return nil }
        var next = self
        next.spinning = true
        next.since = now
        return next
    }

    /// The core's `refreshing` changed. A refresh the shell did not start
    /// itself (a pull) starts the spin too.
    func core(busy: Bool, now: Double) -> RefreshSpin {
        var next = self
        if busy && !spinning {
            next.spinning = true
            next.since = now
        }
        next.coreBusy = busy
        return next
    }

    /// When the spin may stop — `nil` while not spinning, or while the core is
    /// still busy.
    var releaseAt: Double? {
        spinning && !coreBusy ? since + Self.minimumMs : nil
    }

    /// The clock moved on: stop if the release time has come.
    func settle(now: Double) -> RefreshSpin {
        guard let at = releaseAt, now >= at else { return self }
        var next = self
        next.spinning = false
        return next
    }
}
