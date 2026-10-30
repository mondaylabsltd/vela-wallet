//
//  PerfMarks.swift
//  VelaWallet
//
//  The signposts the performance tests read (native parity Phase 4).
//
//  Three intervals, under `app.getvela.VelaWallet` / `perf`, which
//  `XCTOSSignpostMetric` (VelaWalletUITests/PerfTests.swift) and an
//  Instruments "os_signpost" lane both see:
//
//  - `HomeCachedPaint` — from the app's init to the first time the home
//    draws a figure (the cached total, or the first answer): what "cold start
//    to cached home" means once the process exists.
//  - `ConfirmToPasskey` — from the person confirming a send to the passkey
//    prompt being raised (or the trusted page asked): the shell's and the
//    network's share of the wait before Face ID.
//  - `CatalogLoad` — the locale catalog read and merged at launch.
//
//  Release carries them on purpose. A signpost nobody records costs a load
//  and a branch, and the numbers that matter are the shipped build's. They
//  carry no value — no address, no amount, no hash — only a name and a time.
//

import Foundation
import os

nonisolated enum PerfMarks {

    static let signposter = OSSignposter(subsystem: VelaLog.subsystem, category: "perf")

    // MARK: - Launch → cached home

    nonisolated(unsafe) private static var home: OSSignpostIntervalState?

    /// The app's init: the earliest moment this code runs.
    static func launchBegan() {
        guard home == nil else { return }
        home = signposter.beginInterval("HomeCachedPaint")
    }

    /// The home drew a figure. Only the first one counts; later calls are free.
    static func homePainted() {
        guard let state = home else { return }
        home = nil
        signposter.endInterval("HomeCachedPaint", state)
    }

    // MARK: - Confirm → passkey

    nonisolated(unsafe) private static var confirm: OSSignpostIntervalState?

    /// The person confirmed a send.
    static func confirmed() {
        if let open = confirm { signposter.endInterval("ConfirmToPasskey", open) }
        confirm = signposter.beginInterval("ConfirmToPasskey")
    }

    /// The prompt is being raised — or the attempt ended before it was. Ends
    /// the interval opened by `confirmed`, if any.
    static func passkeyRaised() {
        guard let state = confirm else { return }
        confirm = nil
        signposter.endInterval("ConfirmToPasskey", state)
    }

    // MARK: - Spans

    /// A named span of launch work, for an Instruments lane.
    static func span<T>(_ name: StaticString, _ work: () throws -> T) rethrows -> T {
        let state = signposter.beginInterval(name)
        defer { signposter.endInterval(name, state) }
        return try work()
    }
}
