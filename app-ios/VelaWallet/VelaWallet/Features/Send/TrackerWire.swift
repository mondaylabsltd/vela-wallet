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
    /// The core is still following this entry — in its wait window or past
    /// it (not terminal, not abandoned at 24 h). Spec 079: the foreground
    /// clock runs on this, not on `status == "pending"`.
    let polling: Bool
    let submittedAtMs: Double?
    /// Where the operation is in its life, for the words (spec 079): `landing`
    /// (inside the wait window), `still_confirming` (past it and still asked
    /// about — never a failure), `unknown` (past the 24-hour line), `final`.
    /// The status says why; this says when, the same way on every client.
    let outcome: String?
}

struct TrackViewWire: Decodable, Equatable {
    let entries: [TrackEntryWire]

    /// Whether anything is still in flight. The foreground tick runs while this
    /// is true and stops when it is not — a wallet that polled forever would
    /// spend a person's battery on nothing.
    var hasPending: Bool { entries.contains { $0.status == "pending" } }

    /// Whether the core still follows anything — inside its wait window OR
    /// past it (spec 079). The foreground clock used to stop at the window's
    /// end, when the status turns `accepted_not_landed`: an operation still on
    /// its way was then asked about only on the next resume, and "Vela keeps
    /// checking" was not true (Android device pass: a landed op never showed
    /// as landed). The core paces the asking itself — 3 s, 12 s, then once a
    /// minute, then every five — so ticking while this is true costs nothing
    /// it did not decide to spend.
    var isFollowing: Bool { entries.contains { $0.polling } }

    /// The entry for one operation, matched without regard to case.
    func entry(userOpHash: String?) -> TrackEntryWire? {
        guard let userOpHash, !userOpHash.isEmpty else { return nil }
        return entries.first { $0.userOpHash.caseInsensitiveCompare(userOpHash) == .orderedSame }
    }
}
