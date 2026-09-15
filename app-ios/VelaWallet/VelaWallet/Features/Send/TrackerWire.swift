//
//  TrackerWire.swift
//  VelaWallet
//
//  The `tx_tracker` machine's view model, in Swift.
//
//  Small, because almost nothing about this machine is a view: what it
//  publishes is which operations it is still following. The cadence, the
//  receipt classification and the 24-hour abandon deadline are the core's, and
//  the shell only supplies a clock and a transport.
//

import Foundation

/// One operation being followed.
struct TrackEntryWire: Decodable, Equatable {
    let userOpHash: String
    let chainId: Int
    /// Every record this operation wrote — a split has several.
    let recordIds: [String]
    /// The core's own vocabulary, and wider than a screen's three states:
    /// `pending` (or aborted — still confirming), `fee_held` (queued until
    /// network fees settle; the relay sends it itself), `confirmed`, `dropped`
    /// (reverted, terminal), `rejected` (the relay refused it, nothing was
    /// sent), `unreachable` (the bundler could not be asked all window — fate
    /// unknown), and the window-expiry case. Collapsing these into
    /// success/failure is what turns "we could not find out" into "it failed".
    let status: String
    let txHash: String?
    /// A poll is in flight for this entry right now.
    let polling: Bool
    let submittedAtMs: Double?
}

struct TrackViewWire: Decodable, Equatable {
    let entries: [TrackEntryWire]

    /// Whether anything is still in flight. The foreground tick runs while this
    /// is true and stops when it is not — a wallet that polled forever would
    /// spend a person's battery on nothing.
    var hasPending: Bool { entries.contains { $0.status == "pending" } }
}
