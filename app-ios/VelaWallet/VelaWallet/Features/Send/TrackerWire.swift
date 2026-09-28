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
    /// unknown), the window-expiry case, and `not_sent` (spec 082 RA4: a
    /// may-have-been-sent op the relay said twice, a minute apart, it never
    /// had — terminal, and the records read failed). Collapsing these into
    /// success/failure is what turns "we could not find out" into "it failed".
    /// A string rather than an enum on purpose: a status this build has never
    /// heard of must not fail the whole view.
    let status: String
    let txHash: String?
    /// The core is still following this entry — in its wait window or past
    /// it (not terminal, not abandoned at 24 h). Spec 079: the foreground
    /// clock runs on this, not on `status == "pending"`.
    let polling: Bool
    let submittedAtMs: Double?
    /// Where the operation is in its life, for the words (spec 079): `landing`
    /// (inside the wait window), `still_confirming` (past it and still asked
    /// about — never a failure), `unknown` (past the 24-hour line), `final`,
    /// and `maybe_sent` (spec 082: the submit's reply was lost and neither
    /// the relay nor the chain has shown the op yet).
    /// The status says why; this says when, the same way on every client.
    let outcome: String?
    /// The bundle transaction the relay's status named while no receipt has
    /// (spec 082 RA7, the 079 D2 explorer link). A link only: `txHash` above
    /// is the verdict's.
    var relayTxHash: String? = nil

    /// This entry as the core's own `TrackEntryView` JSON — what
    /// `signEndingState` and `sendReceiptOutcomeOf` read.
    var coreJSON: String {
        CoreJSON.string([
            "user_op_hash": userOpHash,
            "chain_id": chainId,
            "record_ids": recordIds,
            "status": status,
            "tx_hash": txHash.map { $0 as Any } ?? NSNull(),
            "polling": polling,
            "submitted_at_ms": submittedAtMs.map { $0 as Any } ?? NSNull(),
            "outcome": outcome ?? "landing",
            "relay_tx_hash": relayTxHash.map { $0 as Any } ?? NSNull(),
        ])
    }
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
