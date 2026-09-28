//! Rules of the post-submit transaction tracker, one test per rule.
//!
//! Inventory `tx_tracker` invariants ①–⑧, each pinned by name. The fake clock
//! is just `now_ms` on each result — the core never owns time. The Safari
//! real-device matrix's four money-safety invariants (never lose / never
//! false-decline / never hang / never double-resolve) become deterministic
//! unit tests here.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::tx_tracker::{
    receipt_interval_ms, Event, TrackLifecycle, TrackOperation as Op, TrackOutcome,
    TrackPendingRecord, TrackRecordPatch, TrackRecordStatus, TrackShellResult as Res, TrackStatus,
    TxTracker, ABANDON_AGE_MS, FEE_HOLD_STAGE, RECONCILE_MIN_INTERVAL_MS,
    SLOWEST_RECEIPT_INTERVAL_MS, SLOW_RECEIPT_INTERVAL_MS, WAIT_WINDOW_MS,
};

type Sut = DomainDriver<TxTracker>;

const T0: f64 = 1_754_700_000_000.0;
const HASH: &str = "0xaaaa1111bbbb2222cccc3333dddd4444eeee5555ffff6666aaaa7777bbbb8888";
const TX: &str = "0x9999888877776666555544443333222211110000ffffeeeeddddccccbbbbaaaa";
const CHAIN: u32 = 8453;

fn poll_receipt() -> Op {
    Op::PollReceipt {
        user_op_hash: HASH.to_owned(),
        chain_id: CHAIN,
    }
}

fn poll_status() -> Op {
    Op::PollStatus {
        user_op_hash: HASH.to_owned(),
        chain_id: CHAIN,
    }
}

fn receipt_pending(now_ms: f64) -> Res {
    Res::ReceiptPending {
        user_op_hash: HASH.to_owned(),
        now_ms,
    }
}

fn receipt_confirmed(now_ms: f64) -> Res {
    Res::Receipt {
        user_op_hash: HASH.to_owned(),
        tx_hash: TX.to_owned(),
        now_ms,
    }
}

fn confirm_patch() -> Op {
    Op::UpdateTxRecords {
        ids: vec!["rec-1".to_owned()],
        patch: TrackRecordPatch {
            status: TrackRecordStatus::Confirmed,
            tx_hash: Some(TX.to_owned()),
        },
    }
}

fn fail_patch() -> Op {
    Op::UpdateTxRecords {
        ids: vec!["rec-1".to_owned()],
        patch: TrackRecordPatch {
            status: TrackRecordStatus::Failed,
            tx_hash: None,
        },
    }
}

fn notify_confirmed() -> Op {
    Op::NotifyConfirmed {
        user_op_hash: HASH.to_owned(),
        chain_id: CHAIN,
        tx_hash: TX.to_owned(),
    }
}

/// Submit one op at `T0` and settle the immediate Now + first receipt poll,
/// leaving the shared receipt request answered `pending` at `T0 + 300`.
fn submitted(sut: &mut Sut) {
    let ops = sut.dispatch(Event::Submitted {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
    });
    assert_eq!(
        ops,
        vec![Op::Now, poll_receipt()],
        "first receipt poll goes out immediately, like waitForReceipt"
    );
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    assert!(sut.resolve(receipt_pending(T0 + 300.0)).is_empty());
}

/// Let the shell's cadence timer fire and answer the clock.
fn tick(sut: &mut Sut, now_ms: f64) -> Vec<Op> {
    let ops = sut.dispatch(Event::Tick);
    assert_eq!(ops, vec![Op::Now]);
    sut.resolve(Res::Clock { now_ms })
}

fn entry_status(sut: &Sut) -> TrackStatus {
    let view = sut.view();
    assert_eq!(view.entries.len(), 1);
    view.entries[0].status
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

/// A receipt within the window confirms: records patched in one atomic batch
/// (same ids, in place) and token_trust notified with the authentic receipt.
#[test]
fn receipt_confirms_patches_records_and_notifies() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(ops, vec![poll_receipt()], "3s cooldown elapsed");

    let ops = sut.resolve(receipt_confirmed(T0 + 3_700.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);

    let view = sut.view();
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].status, TrackStatus::Confirmed);
    assert_eq!(view.entries[0].tx_hash.as_deref(), Some(TX));
    assert!(!view.entries[0].polling);

    assert!(sut.resolve(Res::RecordsPatched).is_empty());
    assert!(sut.resolve(Res::Notified).is_empty());

    // Terminal: the tracker goes inert — not even a clock request.
    assert!(sut.dispatch(Event::Tick).is_empty());
}

// ---------------------------------------------------------------------------
// ① — a timeout or unreachable bundler is NEVER a failure
// ---------------------------------------------------------------------------

/// The whole window passes with clean "not landed yet" answers: no failed
/// patch is ever written — the op may still land, and marking it failed
/// invites a re-send and a double spend.
#[test]
fn timeout_never_marks_records_failed() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 500.0);
    // Window classified; polling continues at the reconcile cadence. The only
    // outstanding work is the next receipt poll — no UpdateTxRecords, ever.
    assert_eq!(ops, vec![poll_receipt()]);
    assert_eq!(entry_status(&sut), TrackStatus::AcceptedNotLanded);
    assert!(sut.view().entries[0].polling, "still reconciling");

    assert!(sut
        .resolve(receipt_pending(T0 + WAIT_WINDOW_MS + 800.0))
        .is_empty());
    assert_eq!(sut.outstanding(), vec![], "no patch was issued anywhere");
}

// ---------------------------------------------------------------------------
// ② — fee-hold keeps the record pending, only the wording changes
// ---------------------------------------------------------------------------

/// A relay fee-hold at the window's end becomes `FeeHeld` — no failed patch,
/// polling continues, and the receipt that the relay eventually produces
/// still confirms the same records.
#[test]
fn fee_hold_stays_pending_and_later_confirms() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    // 12s in: the status endpoint gets its first question.
    let ops = tick(&mut sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_700.0)).is_empty());
    assert!(sut
        .resolve(Res::Status {
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Queued,
            stage: Some(FEE_HOLD_STAGE.to_owned()),
            now_ms: T0 + 12_800.0,
        })
        .is_empty());
    assert_eq!(
        entry_status(&sut),
        TrackStatus::Pending,
        "fee-hold wording only surfaces once the window ends, as today"
    );

    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 100.0);
    assert_eq!(ops, vec![poll_receipt()], "keeps polling at reconcile pace");
    assert_eq!(entry_status(&sut), TrackStatus::FeeHeld);
    assert!(sut.view().entries[0].polling);

    // Fees settled, the relay sent it, the receipt lands: confirmed.
    let ops = sut.resolve(receipt_confirmed(T0 + 300_000.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

// ---------------------------------------------------------------------------
// ③ — only rejected / definitive drop mark failed, and terminate immediately
// ---------------------------------------------------------------------------

/// A relay `rejected` status: nothing was sent, nothing will land. Records
/// flip to failed at once and tracking stops dead.
#[test]
fn rejected_marks_failed_and_terminates_immediately() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_400.0)).is_empty());

    let ops = sut.resolve(Res::Status {
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: None,
        now_ms: T0 + 12_500.0,
    });
    assert_eq!(ops, vec![fail_patch()]);
    assert_eq!(entry_status(&sut), TrackStatus::Rejected);
    assert!(!sut.view().entries[0].polling);

    assert!(sut.resolve(Res::RecordsPatched).is_empty());
    assert!(sut.dispatch(Event::Tick).is_empty(), "terminal ⇒ inert");
}

/// A definitive `success === false` receipt (dropped/reverted): failed, now.
#[test]
fn dropped_receipt_marks_failed_immediately() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 3_500.0);
    assert_eq!(ops, vec![poll_receipt()]);
    let ops = sut.resolve(Res::ReceiptFailed {
        user_op_hash: HASH.to_owned(),
        tx_hash: TX.to_owned(),
        now_ms: T0 + 3_800.0,
    });
    assert_eq!(ops, vec![fail_patch()]);

    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Dropped);
    assert_eq!(
        view.entries[0].tx_hash.as_deref(),
        Some(TX),
        "the receipt sheet links the explorer even for a drop"
    );
    assert!(sut.resolve(Res::RecordsPatched).is_empty());
    assert!(sut.dispatch(Event::Tick).is_empty());
}

/// Lifecycle answers other than `rejected` — including this endpoint's own
/// `failed` — are recorded but never terminate (ported verbatim from
/// waitForReceipt, which only acts on 'rejected').
#[test]
fn non_rejected_status_answers_never_fail_records() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_400.0)).is_empty());
    assert!(sut
        .resolve(Res::Status {
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Failed,
            stage: None,
            now_ms: T0 + 12_500.0,
        })
        .is_empty());
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    assert_eq!(sut.outstanding(), vec![], "no patch issued");
}

// ---------------------------------------------------------------------------
// ④ — past 24h: stop polling, stay pending
// ---------------------------------------------------------------------------

/// An entry older than 24h stops polling for good but is never failed — an
/// honest unknown the user can check on the explorer.
#[test]
fn after_24h_polling_stops_but_record_stays_pending() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + ABANDON_AGE_MS + 1_000.0);
    assert_eq!(ops, vec![], "no RPC polls past the abandon line");

    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Pending, "never failed");
    assert!(!view.entries[0].polling);

    // Fully inert from here — not even a clock request.
    assert!(sut.dispatch(Event::Tick).is_empty());
    assert_eq!(sut.outstanding(), vec![], "no patch was ever written");
}

/// The reconcile sweep skips stored records already older than 24h: no entry,
/// no polls — they stay pending in storage untouched.
#[test]
fn reconcile_skips_records_older_than_24h() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AppResumed);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: T0 });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            record_id: "rec-old".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - ABANDON_AGE_MS - 60_000.0,
        }],
        now_ms: T0 + 100.0,
    });
    assert_eq!(ops, vec![], "too old: not polled");
    assert!(sut.view().entries.is_empty());
}

// ---------------------------------------------------------------------------
// ⑤ — one hash, one shared 3s-throttled receipt request
// ---------------------------------------------------------------------------

/// A second consumer (the receipt sheet joining the background waiter) and a
/// chatty Tick can never double a hash's receipt traffic: in-flight requests
/// are shared and completions start a 3s cooldown.
#[test]
fn same_hash_shares_one_throttled_receipt_request() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Submitted {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());

    // Receipt request still in flight: a tick issues nothing for it.
    assert_eq!(tick(&mut sut, T0 + 1_000.0), vec![]);

    // A second consumer of the same hash joins — no duplicate poll.
    let ops = sut.dispatch(Event::Submitted {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-2".to_owned()],
        chain_id: CHAIN,
    });
    assert_eq!(ops, vec![Op::Now], "joined the in-flight request");
    assert!(sut
        .resolve(Res::Clock {
            now_ms: T0 + 1_100.0
        })
        .is_empty());

    // The request completes; the cooldown counts from completion.
    assert!(sut.resolve(receipt_pending(T0 + 1_200.0)).is_empty());
    assert_eq!(tick(&mut sut, T0 + 2_500.0), vec![], "1.3s < 3s: throttled");
    assert_eq!(tick(&mut sut, T0 + 4_300.0), vec![poll_receipt()]);
}

/// The reconcile sweep itself is single-flight and 12s-throttled — Home focus
/// plus the interval can call it as often as they like.
#[test]
fn reconcile_sweep_is_throttled_and_single_flight() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::HomeFocused);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: T0 });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);

    // Sweep still in flight: a resume asks for nothing more.
    let ops = sut.dispatch(Event::AppResumed);
    assert_eq!(ops, vec![Op::Now]);
    assert!(sut
        .resolve(Res::Clock {
            now_ms: T0 + 1_000.0
        })
        .is_empty());

    assert!(sut
        .resolve(Res::RecordsLoaded {
            records: vec![],
            now_ms: T0 + 1_500.0
        })
        .is_empty());

    // Within 12s of the last run: throttled.
    let ops = sut.dispatch(Event::HomeFocused);
    assert_eq!(ops, vec![Op::Now]);
    assert!(sut
        .resolve(Res::Clock {
            now_ms: T0 + 5_000.0
        })
        .is_empty());

    // Past it: a new sweep.
    let ops = sut.dispatch(Event::HomeFocused);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock {
        now_ms: T0 + 12_500.0,
    });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);
}

// ---------------------------------------------------------------------------
// ⑥ — pending records survive restart and still resolve
// ---------------------------------------------------------------------------

/// A fresh core (post-restart) recovers still-pending submissions from
/// storage on resume and converges them to confirmed — the recovery half of
/// "never lose a pending tx".
#[test]
fn pending_records_survive_restart_and_resolve() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AppResumed);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: T0 });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);

    // A record submitted 10 minutes before this launch.
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - 600_000.0,
        }],
        now_ms: T0 + 100.0,
    });
    assert_eq!(ops, vec![poll_receipt()], "recovered and re-polled");
    assert_eq!(
        entry_status(&sut),
        TrackStatus::AcceptedNotLanded,
        "its wait window is long over — honest wording, still pending"
    );

    let ops = sut.resolve(receipt_confirmed(T0 + 400.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed()],
        "the SAME stored record id is patched in place"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

// ---------------------------------------------------------------------------
// ⑦ — same id updated in place, never a second tracking line
// ---------------------------------------------------------------------------

/// A live submission plus the reconcile sweep finding its stored record meet
/// at ONE entry, and confirmation patches its ids exactly once.
#[test]
fn recovery_merges_into_the_live_entry_never_a_second_one() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = sut.dispatch(Event::HomeFocused);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock {
        now_ms: T0 + 13_000.0,
    });
    // The sweep starts, and the tracked entry's own polls come due with it.
    assert_eq!(ops, vec![Op::LoadPendingTxs, poll_receipt(), poll_status()]);

    // The sweep returns the very record the live entry already tracks —
    // with different hash casing, which must not fork a second entry.
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_uppercase().replace("0X", "0x"),
            chain_id: CHAIN,
            submitted_at_ms: T0,
        }],
        now_ms: T0 + 13_200.0,
    });
    assert_eq!(ops, vec![], "merged: nothing new to poll");
    let view = sut.view();
    assert_eq!(view.entries.len(), 1, "one hash, one entry");
    assert_eq!(view.entries[0].record_ids, vec!["rec-1".to_owned()]);

    // Confirm via the in-flight receipt poll: exactly one patch, same id.
    let ops = sut.resolve(receipt_confirmed(T0 + 13_400.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);
}

/// The other half of never-double-resolve: a sweep that still sees the
/// record as pending (the patch hasn't landed yet) must not resurrect or
/// re-poll an entry that already confirmed.
#[test]
fn sweep_never_resurrects_a_confirmed_entry() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(ops, vec![poll_receipt()]);
    let ops = sut.resolve(receipt_confirmed(T0 + 3_700.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);
    assert!(sut.resolve(Res::RecordsPatched).is_empty());
    assert!(sut.resolve(Res::Notified).is_empty());

    let ops = sut.dispatch(Event::HomeFocused);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock {
        now_ms: T0 + 20_000.0,
    });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0,
        }],
        now_ms: T0 + 20_200.0,
    });
    assert_eq!(ops, vec![], "terminal entries are never re-polled");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

// ---------------------------------------------------------------------------
// ⑧ — unreachable is honestly distinct from pending
// ---------------------------------------------------------------------------

/// A window in which the bundler NEVER answered ends as `Unreachable` — the
/// op's fate is genuinely unknown, which is not the same claim as "submitted
/// and confirming". (`timeout_never_marks_records_failed` pins the clean
/// window ending as `AcceptedNotLanded`; this is the other arm.)
#[test]
fn all_unreachable_window_is_reported_as_unknown_not_pending() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Submitted {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    assert!(sut
        .resolve(Res::ReceiptUnreachable {
            user_op_hash: HASH.to_owned(),
            now_ms: T0 + 400.0
        })
        .is_empty());

    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 200.0);
    assert_eq!(ops, vec![poll_receipt()], "still reconciled later");
    assert_eq!(entry_status(&sut), TrackStatus::Unreachable);
    assert!(sut.view().entries[0].polling);
    // And crucially: no failed patch anywhere (①).
    assert!(sut
        .resolve(Res::ReceiptUnreachable {
            user_op_hash: HASH.to_owned(),
            now_ms: T0 + WAIT_WINDOW_MS + 500.0,
        })
        .is_empty());
    assert_eq!(sut.outstanding(), vec![]);
}

// ---------------------------------------------------------------------------
// Cadence
// ---------------------------------------------------------------------------

/// The relay gets one full receipt interval of peace before the status
/// endpoint is asked at all, then every 12s — never on the 3s receipt beat.
#[test]
fn status_polls_wait_a_full_interval_then_run_every_12s() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(ops, vec![poll_receipt()], "3.4s: receipt only, no status");
    assert!(sut.resolve(receipt_pending(T0 + 3_700.0)).is_empty());

    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()], "12s: both due");
}

// ---------------------------------------------------------------------------
// Stale / late results
// ---------------------------------------------------------------------------

/// A late relay answer for an op whose receipt already confirmed must never
/// un-confirm it (never false-decline): the in-flight status poll's
/// `rejected` is dropped once the entry is terminal.
#[test]
fn late_rejected_status_never_unconfirms_a_receipt() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);

    // The receipt (older outstanding) confirms first…
    let ops = sut.resolve(receipt_confirmed(T0 + 12_300.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);

    // …then the status poll comes back "rejected", too late to matter.
    let ops = sut.resolve(Res::Status {
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: None,
        now_ms: T0 + 12_400.0,
    });
    assert_eq!(ops, vec![], "no failed patch after a confirmation");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// Results for a hash this machine never tracked change nothing.
#[test]
fn results_for_unknown_hashes_are_dropped() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(ops, vec![poll_receipt()]);
    let ops = sut.resolve(Res::Receipt {
        user_op_hash: "0xdeadbeef".to_owned(),
        tx_hash: TX.to_owned(),
        now_ms: T0 + 3_600.0,
    });
    assert_eq!(ops, vec![], "unknown hash: no patch, no notify");
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
}

// ---------------------------------------------------------------------------
// Abort
// ---------------------------------------------------------------------------

/// Aborting the wait (screen unmounted) is never a failure: no window verdict
/// is pronounced, polling drops to the reconcile cadence, and a late receipt
/// still confirms the records.
#[test]
fn abort_keeps_tracking_and_a_late_receipt_still_confirms() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    assert!(sut
        .dispatch(Event::Abort {
            user_op_hash: HASH.to_owned()
        })
        .is_empty());

    // 3s cadence no longer applies…
    assert_eq!(tick(&mut sut, T0 + 4_000.0), vec![]);
    // …the reconcile cadence does, and no status poll rides along.
    assert_eq!(tick(&mut sut, T0 + 12_500.0), vec![poll_receipt()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_800.0)).is_empty());

    // Past the would-be window end: no fee-hold/unreachable verdict — the
    // wait was cancelled, the op is simply still pending.
    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 5_000.0);
    assert_eq!(ops, vec![poll_receipt()]);
    assert_eq!(entry_status(&sut), TrackStatus::Pending);

    let ops = sut.resolve(receipt_confirmed(T0 + WAIT_WINDOW_MS + 5_300.0));
    assert_eq!(ops, vec![confirm_patch(), notify_confirmed()]);
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

// ---------------------------------------------------------------------------
// Spec 079 — the asking slows with age; the words follow the lifecycle
// ---------------------------------------------------------------------------

const MINUTE: f64 = 60_000.0;

/// The cadence table itself: 3 s in the window, 12 s for the first ten
/// minutes, once a minute until an hour, once every five minutes after.
#[test]
fn receipt_polls_slow_down_as_an_op_ages() {
    assert_eq!(receipt_interval_ms(true, 30_000.0), 3_000.0);
    assert_eq!(
        receipt_interval_ms(false, 5.0 * MINUTE),
        RECONCILE_MIN_INTERVAL_MS
    );
    assert_eq!(
        receipt_interval_ms(false, 30.0 * MINUTE),
        SLOW_RECEIPT_INTERVAL_MS
    );
    assert_eq!(
        receipt_interval_ms(false, 180.0 * MINUTE),
        SLOWEST_RECEIPT_INTERVAL_MS
    );
}

/// Driven through the machine: at 30 minutes old a poll 20 s after the last
/// answer is not due; one a minute after is. Still no patch anywhere.
#[test]
fn a_half_hour_old_op_is_asked_once_a_minute() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let at = T0 + 30.0 * MINUTE;
    assert_eq!(tick(&mut sut, at), vec![poll_receipt()]);
    assert!(sut.resolve(receipt_pending(at + 200.0)).is_empty());
    assert_eq!(
        tick(&mut sut, at + 20_000.0),
        vec![],
        "not due at a 12 s pace any more"
    );
    assert_eq!(tick(&mut sut, at + 61_000.0), vec![poll_receipt()]);
    assert!(sut.resolve(receipt_pending(at + 61_300.0)).is_empty());
    assert_eq!(
        entry_status(&sut),
        TrackStatus::AcceptedNotLanded,
        "time never fails it"
    );
    assert_eq!(sut.outstanding(), vec![], "no patch was issued anywhere");
}

/// Three hours old: five minutes between polls.
#[test]
fn a_three_hour_old_op_is_asked_every_five_minutes() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let at = T0 + 180.0 * MINUTE;
    assert_eq!(tick(&mut sut, at), vec![poll_receipt()]);
    assert!(sut.resolve(receipt_pending(at + 200.0)).is_empty());
    assert_eq!(tick(&mut sut, at + 2.0 * MINUTE), vec![]);
    assert_eq!(
        tick(&mut sut, at + 5.0 * MINUTE + 1_000.0),
        vec![poll_receipt()]
    );
}

fn outcome(sut: &Sut) -> TrackOutcome {
    sut.view().entries[0].outcome
}

/// The four words a client picks from, in the order an op lives them.
#[test]
fn the_outcome_follows_the_lifecycle_never_the_clock_alone() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    assert_eq!(outcome(&sut), TrackOutcome::Landing);

    tick(&mut sut, T0 + WAIT_WINDOW_MS + 500.0);
    assert!(sut
        .resolve(receipt_pending(T0 + WAIT_WINDOW_MS + 800.0))
        .is_empty());
    assert_eq!(outcome(&sut), TrackOutcome::StillConfirming);

    assert_eq!(tick(&mut sut, T0 + ABANDON_AGE_MS + 1_000.0), vec![]);
    assert_eq!(outcome(&sut), TrackOutcome::Unknown);
    assert_eq!(
        entry_status(&sut),
        TrackStatus::AcceptedNotLanded,
        "unknown, not failed"
    );
}

#[test]
fn a_landed_op_is_final() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    assert_eq!(tick(&mut sut, T0 + 3_400.0), vec![poll_receipt()]);
    sut.resolve(receipt_confirmed(T0 + 3_700.0));
    assert_eq!(outcome(&sut), TrackOutcome::Final);
}

/// An abort (the person closed the waiting sheet) keeps tracking, worded as
/// still confirming — it is not landing any more as far as the sheet goes.
#[test]
fn an_aborted_op_reads_still_confirming() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    sut.dispatch(Event::Abort {
        user_op_hash: HASH.to_owned(),
    });
    assert_eq!(outcome(&sut), TrackOutcome::StillConfirming);
}
