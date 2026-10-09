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
    parse_user_op_status, receipt_interval_ms, Event, TrackFailure, TrackLifecycle,
    TrackOperation as Op, TrackOutcome, TrackPendingRecord, TrackRecordPatch, TrackRecordStatus,
    TrackSettlement, TrackShellResult as Res, TrackStatus, TrackStatusAnswer, TxTracker,
    ABANDON_AGE_MS, FEE_HOLD_STAGE, FIND_OP_LOOKBACK_BLOCKS, FIND_OP_LOOKBACK_COVERS_MS,
    FIND_OP_MAX_RANGE, NOT_FOUND_CONFIRMATIONS, NOT_FOUND_GRACE_MS, RECONCILE_MIN_INTERVAL_MS,
    RELAY_FUNDING_STAGE, SLOWEST_RECEIPT_INTERVAL_MS, SLOW_RECEIPT_INTERVAL_MS,
    USER_OP_STATUS_METHOD, WAIT_WINDOW_MS,
};
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::USER_OPERATION_EVENT_TOPIC;

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
            settlement: None,
        },
    }
}

/// The failed patch, with why (spec 097 N4).
fn fail_patch(failure: TrackFailure) -> Op {
    Op::UpdateTxRecords {
        ids: vec!["rec-1".to_owned()],
        patch: TrackRecordPatch {
            status: TrackRecordStatus::Failed,
            tx_hash: None,
            settlement: Some(TrackSettlement {
                moved: None,
                failure: Some(failure),
            }),
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

/// Spec 082 RE8: the balance is re-read after an op of ours landed.
fn holdings_moved() -> Op {
    Op::HoldingsMoved { chain_id: CHAIN }
}

/// Submit one op at `T0` and settle the immediate Now + first receipt poll,
/// leaving the shared receipt request answered `pending` at `T0 + 300`.
fn submitted(sut: &mut Sut) {
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    assert_eq!(
        ops,
        vec![Op::Now, poll_receipt()],
        "first receipt poll goes out immediately, like waitForReceipt"
    );
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    assert!(sut.resolve(receipt_pending(T0 + 300.0)).is_empty());
}

/// Let the shell's cadence timer fire and answer the clock (by name: other
/// requests may legitimately still be in flight).
fn tick(sut: &mut Sut, now_ms: f64) -> Vec<Op> {
    let ops = sut.dispatch(Event::Tick);
    assert_eq!(ops, vec![Op::Now]);
    sut.resolve_matching(|op| matches!(op, Op::Now), Res::Clock { now_ms })
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
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "3s cooldown elapsed"
    );

    let ops = sut.resolve(receipt_confirmed(T0 + 3_700.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );

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
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert_eq!(entry_status(&sut), TrackStatus::AcceptedNotLanded);
    assert!(sut.view().entries[0].polling, "still reconciling");

    assert!(sut
        .resolve(receipt_pending(T0 + WAIT_WINDOW_MS + 800.0))
        .is_empty());
    assert!(sut
        .resolve(status(TrackLifecycle::Queued, T0 + WAIT_WINDOW_MS + 900.0))
        .is_empty());
    assert_eq!(sut.outstanding(), vec![], "no patch was issued anywhere");
}

// ---------------------------------------------------------------------------
// The relay topping up its gas: said while it lasts, gone when it moves on
// ---------------------------------------------------------------------------

/// Arbitrum, 2026-10-03: the relay held the op at stage `funding` while its
/// treasury → relayer top-up was meant to land, and the wallet said only
/// "taking longer than usual". The stage is said as soon as the relay says it
/// — inside the window, unlike the fee hold's window verdict — and it is not
/// sticky: the next status that moves on takes it away.
#[test]
fn the_relay_topping_up_its_gas_is_said_while_it_lasts() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_700.0)).is_empty());
    assert!(sut
        .resolve(Res::Status {
            rejection_reason: None,
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Queued,
            stage: Some(RELAY_FUNDING_STAGE.to_owned()),
            now_ms: T0 + 12_800.0,
            tx_hash: None,
        })
        .is_empty());
    assert_eq!(entry_status(&sut), TrackStatus::RelayFunding);
    assert!(sut.view().entries[0].polling, "still pending, still asking");

    // Funded: the relay sends the bundle and says so.
    let ops = tick(&mut sut, T0 + 25_000.0);
    assert!(ops.contains(&poll_status()), "{ops:?}");
    assert!(sut
        .resolve(Res::Status {
            rejection_reason: None,
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Submitted,
            stage: None,
            now_ms: T0 + 25_100.0,
            tx_hash: None,
        })
        .is_empty());
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
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
            rejection_reason: None,
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Queued,
            stage: Some(FEE_HOLD_STAGE.to_owned()),
            now_ms: T0 + 12_800.0,
            tx_hash: None,
        })
        .is_empty());
    assert_eq!(
        entry_status(&sut),
        TrackStatus::Pending,
        "fee-hold wording only surfaces once the window ends, as today"
    );

    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 100.0);
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "keeps polling at reconcile pace"
    );
    assert_eq!(entry_status(&sut), TrackStatus::FeeHeld);
    assert!(sut.view().entries[0].polling);

    // Fees settled, the relay sent it, the receipt lands: confirmed.
    let ops = sut.resolve(receipt_confirmed(T0 + 300_000.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
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
        rejection_reason: None,
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: None,
        now_ms: T0 + 12_500.0,
        tx_hash: None,
    });
    assert_eq!(ops, vec![fail_patch(TrackFailure::Refused)]);
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
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    let ops = sut.resolve(Res::ReceiptFailed {
        user_op_hash: HASH.to_owned(),
        tx_hash: TX.to_owned(),
        now_ms: T0 + 3_800.0,
    });
    // It landed and reverted: gas was spent, so the holdings moved (RE8).
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );

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
            rejection_reason: None,
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Failed,
            stage: None,
            now_ms: T0 + 12_500.0,
            tx_hash: None,
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
            sender: None,
            record_id: "rec-old".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - ABANDON_AGE_MS - 60_000.0,
            maybe_sent: false,
            submit_block: None,
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
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());

    // Receipt request still in flight: a tick issues nothing for it.
    assert_eq!(tick(&mut sut, T0 + 1_000.0), vec![]);

    // A second consumer of the same hash joins — no duplicate poll.
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-2".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    assert_eq!(ops, vec![Op::Now], "joined the in-flight request");
    assert!(sut
        .resolve(Res::Clock {
            now_ms: T0 + 1_100.0
        })
        .is_empty());

    // The request completes. The cooldown counts from the ask, which went
    // out with the op and was stamped by the first clock, T0 (issue #464).
    assert!(sut.resolve(receipt_pending(T0 + 1_200.0)).is_empty());
    assert_eq!(tick(&mut sut, T0 + 2_500.0), vec![], "2.5s < 3s: throttled");
    assert_eq!(
        tick(&mut sut, T0 + 4_300.0),
        vec![poll_receipt(), poll_status()]
    );
}

/// Issue #464: every shell ticks every 3 s, and the receipt cooldown used to
/// count from the ANSWER — a poll answered 0.3 s after its tick was 2.7 s old
/// on the next tick and waited for the one after, so an op was asked every
/// 6 s. It counts from the ASK now: the receipt is asked every 3 s, including
/// on a tick whose clock reads a millisecond short of 3 s, and a slow answer
/// does not push the next ask out.
#[test]
fn a_three_second_tick_asks_for_the_receipt_every_three_seconds() {
    /// Answer every outstanding poll at `at`, noting any receipt ask the
    /// answers themselves make.
    fn answer_all(sut: &mut Sut, at: f64, asks: &mut Vec<f64>) {
        while sut.outstanding().iter().any(is_receipt) {
            let ops = sut.resolve_matching(is_receipt, receipt_pending(T0 + at));
            if ops.contains(&poll_receipt()) {
                asks.push(at);
            }
        }
        while sut.outstanding().iter().any(is_status) {
            sut.resolve_matching(is_status, status(TrackLifecycle::Queued, T0 + at));
        }
    }

    let mut sut = Sut::new();
    submitted(&mut sut); // asked with the op at T0, answered at T0 + 0.3 s
    let mut asks = vec![0.0];
    // (the tick, when what it asked is answered)
    let ticks = [
        (3_000.0, 3_300.0),
        (6_000.0, 6_300.0),
        (8_999.0, 9_299.0),   // the timer's clock reads 1 ms early
        (12_000.0, 14_900.0), // a slow answer, 2.9 s after the ask
        (15_000.0, 15_300.0),
        (18_000.0, 18_300.0),
    ];
    for (at, answered) in ticks {
        if tick(&mut sut, T0 + at).contains(&poll_receipt()) {
            asks.push(at);
        }
        answer_all(&mut sut, answered, &mut asks);
    }
    assert_eq!(
        asks,
        vec![0.0, 3_000.0, 6_000.0, 8_999.0, 12_000.0, 14_900.0, 18_000.0],
        "asked every 3 s — the slow answer re-asks at once, as it is already 2.9 s since the ask"
    );
    // Between asks nothing is due early: 1 s after one is still too soon.
    assert!(!tick(&mut sut, T0 + 19_000.0).contains(&poll_receipt()));
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
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - 600_000.0,
            maybe_sent: false,
            submit_block: None,
        }],
        now_ms: T0 + 100.0,
    });
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "recovered and re-polled"
    );
    assert_eq!(
        entry_status(&sut),
        TrackStatus::AcceptedNotLanded,
        "its wait window is long over — honest wording, still pending"
    );

    let ops = sut.resolve(receipt_confirmed(T0 + 400.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()],
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
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_uppercase().replace("0X", "0x"),
            chain_id: CHAIN,
            submitted_at_ms: T0,
            maybe_sent: false,
            submit_block: None,
        }],
        now_ms: T0 + 13_200.0,
    });
    assert_eq!(ops, vec![], "merged: nothing new to poll");
    let view = sut.view();
    assert_eq!(view.entries.len(), 1, "one hash, one entry");
    assert_eq!(view.entries[0].record_ids, vec!["rec-1".to_owned()]);

    // Confirm via the in-flight receipt poll: exactly one patch, same id.
    let ops = sut.resolve(receipt_confirmed(T0 + 13_400.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
}

/// The other half of never-double-resolve: a sweep that still sees the
/// record as pending (the patch hasn't landed yet) must not resurrect or
/// re-poll an entry that already confirmed.
#[test]
fn sweep_never_resurrects_a_confirmed_entry() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    let ops = sut.resolve(receipt_confirmed(T0 + 3_700.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
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
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0,
            maybe_sent: false,
            submit_block: None,
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
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
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
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "still reconciled later"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Unreachable);
    assert!(sut.view().entries[0].polling);
    // And crucially: no failed patch anywhere (①).
    assert!(sut
        .resolve(Res::ReceiptUnreachable {
            user_op_hash: HASH.to_owned(),
            now_ms: T0 + WAIT_WINDOW_MS + 500.0,
        })
        .is_empty());
    assert!(sut
        .resolve(status(
            TrackLifecycle::NotFound,
            T0 + WAIT_WINDOW_MS + 600.0
        ))
        .is_empty());
    assert_eq!(entry_status(&sut), TrackStatus::Unreachable);
    assert_eq!(sut.outstanding(), vec![]);
}

// ---------------------------------------------------------------------------
// Cadence
// ---------------------------------------------------------------------------

/// The relay is first asked 3 s after acceptance (spec 099 R6: its queue,
/// its funding, its send are what the landing says in its first seconds),
/// then every 12 s — never on the 3 s receipt beat.
#[test]
fn status_is_first_asked_at_3s_then_every_12s() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let ops = tick(&mut sut, T0 + 1_000.0);
    assert_eq!(ops, vec![], "1s: nothing due yet");

    let ops = tick(&mut sut, T0 + 3_400.0);
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "3.4s: the first status ask"
    );
    assert!(sut.resolve(receipt_pending(T0 + 3_700.0)).is_empty());
    assert!(sut
        .resolve(status(TrackLifecycle::Queued, T0 + 3_800.0))
        .is_empty());

    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt()], "12.1s: receipt only");
    assert!(sut.resolve(receipt_pending(T0 + 12_300.0)).is_empty());

    let ops = tick(&mut sut, T0 + 15_500.0);
    assert_eq!(
        ops,
        vec![poll_receipt(), poll_status()],
        "12s after the first ask"
    );
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
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );

    // …then the status poll comes back "rejected", too late to matter.
    let ops = sut.resolve(Res::Status {
        rejection_reason: None,
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: None,
        now_ms: T0 + 12_400.0,
        tx_hash: None,
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
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
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
    // …the reconcile cadence does, and the relay is asked at it too (spec
    // 099 R6: only its `not_found` tells a forgotten op from a slow one).
    assert_eq!(
        tick(&mut sut, T0 + 12_500.0),
        vec![poll_receipt(), poll_status()]
    );
    assert!(sut.resolve(receipt_pending(T0 + 12_800.0)).is_empty());

    // Past the would-be window end: no fee-hold/unreachable verdict — the
    // wait was cancelled, the op is simply still pending.
    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 5_000.0);
    assert_eq!(ops, vec![poll_receipt()]);
    assert_eq!(entry_status(&sut), TrackStatus::Pending);

    let ops = sut.resolve(receipt_confirmed(T0 + WAIT_WINDOW_MS + 5_300.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
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
/// ask is not due; one a minute after is (the pace counts from the ask, issue
/// #464). Still no patch anywhere.
#[test]
fn a_half_hour_old_op_is_asked_once_a_minute() {
    let mut sut = Sut::new();
    submitted(&mut sut);

    let at = T0 + 30.0 * MINUTE;
    assert_eq!(tick(&mut sut, at), vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(at + 200.0)).is_empty());
    assert!(sut
        .resolve(status(TrackLifecycle::Queued, at + 300.0))
        .is_empty());
    assert_eq!(
        tick(&mut sut, at + 20_000.0),
        vec![],
        "not due at a 12 s pace any more"
    );
    assert_eq!(
        tick(&mut sut, at + 61_000.0),
        vec![poll_receipt(), poll_status()]
    );
    assert!(sut.resolve(receipt_pending(at + 61_300.0)).is_empty());
    assert!(sut
        .resolve(status(TrackLifecycle::Queued, at + 61_400.0))
        .is_empty());
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
    assert_eq!(tick(&mut sut, at), vec![poll_receipt(), poll_status()]);
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
    assert_eq!(
        tick(&mut sut, T0 + 3_400.0),
        vec![poll_receipt(), poll_status()]
    );
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

// ---------------------------------------------------------------------------
// Spec 082 T016 (RA7, G13) — the relay's status method and its one parser
// ---------------------------------------------------------------------------

/// The relay serves `pimlico_getUserOperationStatus`; the spelling every
/// client asked for answered -32601 (`evidence/relay-status-probe.txt`).
#[test]
fn the_status_method_is_the_one_the_relay_serves() {
    assert_eq!(USER_OP_STATUS_METHOD, "pimlico_getUserOperationStatus");
    let source = include_str!("../src/app/tx_tracker.rs");
    assert!(
        !source.contains(&format!("eth_get{}", "UserOperationStatus")),
        "the dead method name is gone from the tracker"
    );
}

#[test]
fn the_live_probe_s_answers_parse() {
    // Verbatim from the 2026-09-28 probe.
    assert_eq!(
        parse_user_op_status(r#"{"status":"not_found","transactionHash":null}"#),
        Some(TrackStatusAnswer {
            rejection_reason: None,
            status: TrackLifecycle::NotFound,
            stage: None,
            tx_hash: None,
        })
    );
    assert_eq!(
        parse_user_op_status(&format!(
            r#"{{"status":"included","transactionHash":"{TX}","last_executor_stage":"bundled"}}"#
        )),
        Some(TrackStatusAnswer {
            rejection_reason: None,
            status: TrackLifecycle::Included,
            stage: Some("bundled".to_owned()),
            tx_hash: Some(TX.to_owned()),
        })
    );
    // The whole JSON-RPC body is read through to its result.
    assert_eq!(
        parse_user_op_status(
            r#"{"jsonrpc":"2.0","id":1,"result":{"status":"queued","last_executor_stage":"in_band_settlement_hold"}}"#
        ),
        Some(TrackStatusAnswer {
            rejection_reason: None,
            status: TrackLifecycle::Queued,
            stage: Some(FEE_HOLD_STAGE.to_owned()),
            tx_hash: None,
        })
    );
}

/// An unknown status string is not guessed at — the e2e stub's `pending` was
/// never a relay status.
#[test]
fn an_unknown_status_parses_to_nothing() {
    for junk in [
        r#"{"status":"pending?"}"#,
        r#"{"status":"pending"}"#,
        r#"{"status":7}"#,
        r#"{"transactionHash":"0x1"}"#,
        r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"method not found"}}"#,
        "null",
        "not json",
    ] {
        assert_eq!(parse_user_op_status(junk), None, "{junk}");
    }
}

/// The relay's bundle tx rides the view as a link while no receipt exists —
/// never as the verdict's `tx_hash`.
#[test]
fn the_relay_s_tx_hash_is_a_link_not_a_verdict() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_700.0)).is_empty());
    // Since round 2 (RJ4) the tx hash is also asked about on chain — the
    // receipt of that tx, not a verdict of its own.
    assert_eq!(
        sut.resolve(Res::Status {
            rejection_reason: None,
            user_op_hash: HASH.to_owned(),
            status: TrackLifecycle::Submitted,
            stage: None,
            now_ms: T0 + 12_800.0,
            tx_hash: Some(TX.to_owned()),
        }),
        vec![Op::TxReceipt {
            chain_id: CHAIN,
            tx_hash: TX.to_owned(),
            user_op_hash: HASH.to_owned(),
        }]
    );
    let view = sut.view();
    assert_eq!(view.entries[0].relay_tx_hash.as_deref(), Some(TX));
    assert_eq!(view.entries[0].tx_hash, None);
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    // A shell that predates the field still decodes.
    let old: Option<Res> = serde_json::from_str(&format!(
        r#"{{"type":"status","user_op_hash":"{HASH}","status":"queued","stage":null,"now_ms":1}}"#
    ))
    .ok();
    assert!(matches!(old, Some(Res::Status { tx_hash: None, .. })));
}

// ---------------------------------------------------------------------------
// Spec 082 T017 (RA4) — a submit whose reply was lost: MaybeSent, then either
// the relay shows it holds the op, or it never had it (NotSent)
// ---------------------------------------------------------------------------

const SUBMIT_BLOCK: u64 = 48_478_700;
/// The bundle tx a found event names — distinct from any receipt's.
const EVENT_TX: &str = "0xc6f3544fc4e3ac769e92c92ab4804cd3f38ffb8d607ba07b5103a59710094dc4";

fn find_op(from: Option<u64>, to: Option<u64>) -> Op {
    Op::FindOpEvent {
        chain_id: CHAIN,
        entry_point: ENTRY_POINT.to_owned(),
        topic0: USER_OPERATION_EVENT_TOPIC.to_owned(),
        user_op_hash: HASH.to_owned(),
        from_block: from,
        to_block: to,
    }
}

fn head_only() -> Op {
    find_op(None, None)
}

fn window(from: u64, to: u64) -> Op {
    find_op(Some(from), Some(to))
}

fn is_find(op: &Op) -> bool {
    matches!(op, Op::FindOpEvent { .. })
}

fn is_status(op: &Op) -> bool {
    matches!(op, Op::PollStatus { .. })
}

fn is_receipt(op: &Op) -> bool {
    matches!(op, Op::PollReceipt { .. })
}

fn status(lifecycle: TrackLifecycle, now_ms: f64) -> Res {
    Res::Status {
        rejection_reason: None,
        user_op_hash: HASH.to_owned(),
        status: lifecycle,
        stage: None,
        now_ms,
        tx_hash: None,
    }
}

fn op_event(now_ms: f64, logs: Option<&str>, error: Option<&str>, head: Option<u64>) -> Res {
    Res::OpEvent {
        user_op_hash: HASH.to_owned(),
        now_ms,
        logs_json: logs.map(str::to_owned),
        error_json: error.map(str::to_owned),
        head_block: head,
    }
}

/// The op's own `UserOperationEvent`, as `eth_getLogs` returns it.
fn event_logs(success: bool) -> String {
    let data = format!(
        "0x{:064x}{:064x}{:064x}{:064x}",
        22,
        u8::from(success),
        0x359a5_u64,
        0x359a5_u64
    );
    serde_json::json!([{
        "address": ENTRY_POINT.to_lowercase(),
        "topics": [
            USER_OPERATION_EVENT_TOPIC,
            HASH,
            "0x000000000000000000000000d400866e00b055b20752a826cd5c89b811de130b",
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        ],
        "data": data,
        "blockNumber": "0x2e3ba09",
        "transactionHash": EVENT_TX,
        "removed": false
    }])
    .to_string()
}

fn event_confirmed() -> Vec<Op> {
    vec![
        Op::UpdateTxRecords {
            ids: vec!["rec-1".to_owned()],
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Confirmed,
                tx_hash: Some(EVENT_TX.to_owned()),
                settlement: None,
            },
        },
        Op::NotifyConfirmed {
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            tx_hash: EVENT_TX.to_owned(),
        },
        holdings_moved(),
    ]
}

/// Submit a may-have-been-sent op at `T0` (the head before the POST known or
/// not), settling the immediate Now and the first receipt poll as `pending`.
fn submitted_maybe(sut: &mut Sut, submit_block: Option<u64>) {
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: true,
        submit_block,
        admitted: false,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    assert!(sut.resolve(receipt_pending(T0 + 300.0)).is_empty());
}

/// Answer every outstanding receipt poll `pending` and every status poll
/// with `answer`, leaving find ops outstanding. Returns what those answers
/// asked for beyond more reading (patches, notifications).
fn settle_polls(sut: &mut Sut, now_ms: f64, answer: TrackLifecycle) -> Vec<Op> {
    let mut asked = Vec::new();
    while sut.outstanding().iter().any(is_receipt) {
        asked.extend(sut.resolve_matching(is_receipt, receipt_pending(now_ms)));
    }
    while sut.outstanding().iter().any(is_status) {
        asked.extend(sut.resolve_matching(is_status, status(answer, now_ms)));
    }
    asked.retain(|op| !is_find(op) && !is_status(op) && !is_receipt(op));
    asked
}

/// Read the chain up to `head` with no event: answer the outstanding find op
/// and every window it chains into.
fn scan_to_head(sut: &mut Sut, now_ms: f64, head: u64) {
    while sut.outstanding().iter().any(is_find) {
        sut.resolve_matching(is_find, op_event(now_ms, Some("[]"), None, Some(head)));
    }
}

fn outcome_of(sut: &Sut) -> TrackOutcome {
    sut.view().entries[0].outcome
}

#[test]
fn the_not_found_constants_are_ra4_s() {
    assert_eq!(NOT_FOUND_GRACE_MS, 60_000.0);
    assert_eq!(NOT_FOUND_CONFIRMATIONS, 2);
}

/// A lost reply reads MaybeSent — never Landing, never "try again" — until the
/// relay shows it holds the op; from then on it is an ordinary one.
#[test]
fn a_lost_reply_reads_maybe_sent_until_the_relay_acknowledges() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    assert_eq!(entry_status(&sut), TrackStatus::Pending);

    let ops = tick(&mut sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status(), head_only()]);
    settle_polls(&mut sut, T0 + 12_700.0, TrackLifecycle::Queued);
    assert_eq!(outcome_of(&sut), TrackOutcome::Landing, "acknowledged");

    // Acknowledged: the chain read stops chaining, and no new one is asked.
    assert!(sut
        .resolve_matching(
            is_find,
            op_event(T0 + 12_800.0, None, None, Some(SUBMIT_BLOCK + 3))
        )
        .is_empty());
    let ops = tick(&mut sut, T0 + 24_800.0);
    assert!(!ops.iter().any(is_find), "{ops:?}");
}

/// Past the window, a may-have-been-sent op the relay has not acknowledged is
/// still asked about — in exactly this fault the relay was silent during it.
#[test]
fn status_polls_outlive_the_window_while_in_doubt() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let ops = tick(&mut sut, T0 + WAIT_WINDOW_MS + 1_000.0);
    assert!(ops.contains(&poll_status()), "{ops:?}");
    sut.resolve_matching(
        is_status,
        Res::StatusUnavailable {
            user_op_hash: HASH.to_owned(),
            now_ms: T0 + WAIT_WINDOW_MS + 1_100.0,
        },
    );
    // StatusUnavailable changes nothing.
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    let ops = tick(
        &mut sut,
        T0 + WAIT_WINDOW_MS + 1_000.0 + receipt_interval_ms(false, WAIT_WINDOW_MS),
    );
    assert!(ops.contains(&poll_status()), "{ops:?}");
}

/// Drive a may-have-been-sent op to its next status poll at `at`, reading the
/// chain to `head` with no event and answering the relay `not_found`.
fn not_found_at(sut: &mut Sut, at: f64, head: u64) -> Vec<Op> {
    let ops = tick(sut, at);
    assert!(ops.contains(&poll_status()), "{ops:?}");
    scan_to_head(sut, at + 100.0, head);
    settle_polls(sut, at + 200.0, TrackLifecycle::NotFound)
}

/// Two `not_found` answers past the grace, the chain read to its head with no
/// event: never sent. Records failed; no balance re-read (nothing landed).
#[test]
fn two_not_found_past_the_grace_end_it_as_not_sent() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    // 30 s: inside the grace — ignored.
    assert!(not_found_at(&mut sut, T0 + 30_000.0, SUBMIT_BLOCK + 6).is_empty());
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    // 61 s: first past the grace.
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    // 73 s: second — never sent.
    let ops = not_found_at(&mut sut, T0 + 73_000.0, SUBMIT_BLOCK + 14);
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::NotSent)],
        "no HoldingsMoved: nothing landed"
    );
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::NotSent);
    assert_eq!(view.entries[0].outcome, TrackOutcome::Final);
    assert!(!view.entries[0].polling);
    assert!(sut.dispatch(Event::Tick).is_empty(), "terminal: inert");
}

/// A `not_found`, then any other answer, then `not_found`: the streak resets —
/// and once the relay has shown it holds the op, `not_found` is inert.
#[test]
fn a_not_found_streak_resets_on_any_other_answer() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    let ops = tick(&mut sut, T0 + 73_000.0);
    assert!(ops.contains(&poll_status()));
    scan_to_head(&mut sut, T0 + 73_100.0, SUBMIT_BLOCK + 14);
    assert!(settle_polls(&mut sut, T0 + 73_200.0, TrackLifecycle::Submitted).is_empty());
    // Past the window now; not_found twice more changes nothing.
    for at in [T0 + 150_000.0, T0 + 200_000.0] {
        let _ = tick(&mut sut, at);
        let asked = settle_polls(&mut sut, at + 100.0, TrackLifecycle::NotFound);
        assert!(
            !asked.contains(&fail_patch(TrackFailure::NotSent)),
            "{asked:?}"
        );
    }
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::AcceptedNotLanded);
    assert_eq!(view.entries[0].outcome, TrackOutcome::StillConfirming);
}

/// A receipt wins over `not_found`: the op landed, whatever the relay says.
#[test]
fn a_receipt_wins_over_not_found() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    let ops = tick(&mut sut, T0 + 64_500.0);
    assert!(ops.contains(&poll_receipt()), "{ops:?}");
    let ops = sut.resolve_matching(is_receipt, receipt_confirmed(T0 + 64_800.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
    // Terminal: the tracker asks nothing more, and a late not_found for the
    // poll still in flight is dropped.
    assert!(sut.dispatch(Event::Tick).is_empty());
    let asked = settle_polls(&mut sut, T0 + 80_000.0, TrackLifecycle::NotFound);
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// A plain (accepted) op's `not_found` stays inert, as in 079.
#[test]
fn a_plain_op_s_not_found_stays_inert() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    for at in [T0 + 61_000.0, T0 + 73_000.0, T0 + 85_000.0] {
        let ops = tick(&mut sut, at);
        assert!(!ops.iter().any(is_find), "no chain read for a plain op");
        let asked = settle_polls(&mut sut, at + 100.0, TrackLifecycle::NotFound);
        assert!(asked.is_empty(), "{asked:?}");
    }
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    assert_eq!(outcome_of(&sut), TrackOutcome::Landing);
}

/// Time alone never produces a failure: 24 h of silence leaves a
/// may-have-been-sent op Unknown with its records untouched.
#[test]
fn a_day_of_silence_is_unknown_never_failed() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let _ = tick(&mut sut, T0 + 12_400.0); // polls go out and are never answered
    let _ = tick(&mut sut, T0 + ABANDON_AGE_MS + 1.0);
    let view = sut.view();
    assert_eq!(view.entries[0].outcome, TrackOutcome::Unknown);
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    assert!(!view.entries[0].polling);
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::UpdateTxRecords { .. } | Op::HoldingsMoved { .. })),
        "no patch, no balance read"
    );
}

/// A restart keeps a may-have-been-sent op one, and its scan starts where
/// the submit's head was — both restored from the stored record.
#[test]
fn a_reload_restores_maybe_sent_and_the_submit_block() {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::AppResumed), vec![Op::Now]);
    assert_eq!(
        sut.resolve(Res::Clock { now_ms: T0 }),
        vec![Op::LoadPendingTxs]
    );
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - 30_000.0,
            maybe_sent: true,
            submit_block: Some(SUBMIT_BLOCK),
        }],
        now_ms: T0 + 100.0,
    });
    assert_eq!(ops, vec![poll_receipt(), poll_status(), head_only()]);
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 200.0, None, None, Some(SUBMIT_BLOCK + 9)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 9)]);

    // A record stored before 082 reads back as a plain op.
    let old: Option<TrackPendingRecord> = serde_json::from_str(&format!(
        r#"{{"record_id":"r","user_op_hash":"{HASH}","chain_id":100,"submitted_at_ms":1}}"#
    ))
    .ok();
    assert_eq!(
        old.map(|record| (record.maybe_sent, record.submit_block)),
        Some((false, None))
    );
    let old: Option<Event> = serde_json::from_str(&format!(
        r#"{{"type":"submitted","user_op_hash":"{HASH}","record_ids":[],"chain_id":100}}"#
    ))
    .ok();
    assert!(matches!(
        old,
        Some(Event::Submitted {
            maybe_sent: false,
            submit_block: None,
            admitted: false,
            ..
        })
    ));
}

// ---------------------------------------------------------------------------
// Spec 082 T018 (RE8) — the balance is re-read after an op of ours landed
// ---------------------------------------------------------------------------

/// Only a landing moves holdings: a confirmed receipt (after the patch and the
/// token_trust hand-off) or a failed one with a tx hash (gas was spent). A
/// failed receipt that names no tx, a relay rejection and NotSent never do.
#[test]
fn holdings_move_only_when_an_op_landed() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let _ = tick(&mut sut, T0 + 3_500.0);
    let ops = sut.resolve(Res::ReceiptFailed {
        user_op_hash: HASH.to_owned(),
        tx_hash: String::new(),
        now_ms: T0 + 3_800.0,
    });
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted)],
        "no tx, no landing"
    );

    let mut sut = Sut::new();
    submitted(&mut sut);
    let _ = tick(&mut sut, T0 + WAIT_WINDOW_MS + 100.0);
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::HoldingsMoved { .. })),
        "pending, unreachable or old: nothing moved"
    );
}

// ---------------------------------------------------------------------------
// Spec 082 T019 (ruling 8) — the relay-independent landing check
// ---------------------------------------------------------------------------

/// Tick to the first chain read of a may-have-been-sent op and answer the
/// head, returning the window it asks for next.
fn first_window(sut: &mut Sut, head: u64) -> Vec<Op> {
    let ops = tick(sut, T0 + 12_400.0);
    assert!(ops.contains(&head_only()), "{ops:?}");
    sut.resolve_matching(is_find, op_event(T0 + 12_500.0, None, None, Some(head)))
}

#[test]
fn the_find_constants_are_pinned() {
    assert_eq!(FIND_OP_MAX_RANGE, 2_000);
    assert_eq!(FIND_OP_LOOKBACK_BLOCKS, 5_000);
    // The lookback reaches the submit of an op this young on Arbitrum (0.25 s).
    assert!(FIND_OP_LOOKBACK_COVERS_MS <= FIND_OP_LOOKBACK_BLOCKS as f64 * 250.0);
}

/// The op's own event with success: confirmed with the event's tx hash —
/// records confirmed, token_trust told, the balance re-read.
#[test]
fn a_found_event_with_success_confirms() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let ops = first_window(&mut sut, SUBMIT_BLOCK + 40);
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 40)]);
    let ops = sut.resolve_matching(
        is_find,
        op_event(
            T0 + 12_600.0,
            Some(&event_logs(true)),
            None,
            Some(SUBMIT_BLOCK + 40),
        ),
    );
    assert_eq!(ops, event_confirmed());
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Confirmed);
    assert_eq!(view.entries[0].tx_hash.as_deref(), Some(EVENT_TX));
    assert_eq!(view.entries[0].outcome, TrackOutcome::Final);

    // The found event wins over any later relay `not_found`.
    let asked = settle_polls(&mut sut, T0 + 90_000.0, TrackLifecycle::NotFound);
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// The op's own event with `success = false`: it landed and reverted —
/// records failed, the balance re-read (gas was spent).
#[test]
fn a_found_event_without_success_is_dropped() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let _ = first_window(&mut sut, SUBMIT_BLOCK + 40);
    let ops = sut.resolve_matching(
        is_find,
        op_event(
            T0 + 12_600.0,
            Some(&event_logs(false)),
            None,
            Some(SUBMIT_BLOCK + 40),
        ),
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Dropped);
    assert_eq!(view.entries[0].tx_hash.as_deref(), Some(EVENT_TX));
}

/// Another op's event, a removed log, or an event from another contract is
/// not this op landing.
#[test]
fn only_this_op_s_own_event_counts() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let _ = first_window(&mut sut, SUBMIT_BLOCK + 40);
    let other_hash = event_logs(true).replace(&HASH[2..], &"ab".repeat(32));
    let removed = event_logs(true).replace("\"removed\":false", "\"removed\":true");
    let elsewhere = event_logs(true).replace(
        &ENTRY_POINT.to_lowercase(),
        "0x1111111111111111111111111111111111111111",
    );
    let logs = format!(
        "[{},{},{}]",
        &other_hash[1..other_hash.len() - 1],
        &removed[1..removed.len() - 1],
        &elsewhere[1..elsewhere.len() - 1]
    );
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_600.0, Some(&logs), None, Some(SUBMIT_BLOCK + 40)),
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
}

/// An empty window steps forward at once while blocks remain below the head;
/// caught up, the scan waits for the next tick, which asks for the head.
#[test]
fn an_empty_window_steps_forward_until_caught_up() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let head = SUBMIT_BLOCK + 4_500;
    let ops = first_window(&mut sut, head);
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 1_999)]);
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_600.0, Some("[]"), None, Some(head)),
    );
    assert_eq!(
        ops,
        vec![window(SUBMIT_BLOCK + 2_000, SUBMIT_BLOCK + 3_999)]
    );
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_700.0, Some("[]"), None, Some(head)),
    );
    assert_eq!(
        ops,
        vec![window(SUBMIT_BLOCK + 4_000, head)],
        "clipped to the head"
    );
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_800.0, Some("[]"), None, Some(head)),
    );
    assert!(ops.is_empty(), "caught up: wait for the next tick");

    // Next tick: the head first, then the new blocks only.
    settle_polls(&mut sut, T0 + 12_900.0, TrackLifecycle::NotFound);
    let ops = tick(&mut sut, T0 + 24_900.0);
    assert!(ops.contains(&head_only()), "{ops:?}");
    let ops = sut.resolve_matching(is_find, op_event(T0 + 25_000.0, None, None, Some(head + 3)));
    assert_eq!(ops, vec![window(head + 1, head + 3)]);
}

/// A range limit halves the window at once, down to one block; any other
/// error keeps the window for the next tick. The pool's range answer is
/// judged here, never by a shell.
#[test]
fn a_range_error_halves_the_window_and_another_error_keeps_it() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let head = SUBMIT_BLOCK + 10_000;
    let ops = first_window(&mut sut, head);
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 1_999)]);
    // dRPC's free tier on Gnosis, verbatim.
    let drpc = r#"{"message":"ranges over 10000 blocks are not supported on free plan","code":35}"#;
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_600.0, None, Some(drpc), Some(head)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 999)]);
    // publicnode's on Gnosis, verbatim.
    let publicnode = r#"{"code":-32602,"message":"Block range 20001 exceeds the maximum of 10000 blocks per logs request. Use a narrower fromBlock/toBlock range or increase Receipt.MaxBlockDepth."}"#;
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_700.0, None, Some(publicnode), Some(head)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 499)]);
    // Something else went wrong: the same window, on the next tick.
    let ops = sut.resolve_matching(
        is_find,
        op_event(
            T0 + 12_800.0,
            None,
            Some(r#"{"code":-32603,"message":"internal error"}"#),
            Some(head),
        ),
    );
    assert!(ops.is_empty());
    settle_polls(&mut sut, T0 + 12_900.0, TrackLifecycle::NotFound);
    let ops = tick(&mut sut, T0 + 24_900.0);
    assert!(
        ops.contains(&window(SUBMIT_BLOCK, SUBMIT_BLOCK + 499)),
        "{ops:?}"
    );
    // No answer at all: the same.
    let ops = sut.resolve_matching(is_find, op_event(T0 + 25_000.0, None, None, None));
    assert!(ops.is_empty());
}

/// Halving stops at one block; a one-block range error waits for the next
/// tick rather than spinning.
#[test]
fn a_one_block_window_is_not_halved_further() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let _ = first_window(&mut sut, SUBMIT_BLOCK + 1);
    let range = r#"{"code":-32005,"message":"query returned more than 10000 results"}"#;
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_600.0, None, Some(range), Some(SUBMIT_BLOCK + 1)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK)]);
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_700.0, None, Some(range), Some(SUBMIT_BLOCK + 1)),
    );
    assert!(ops.is_empty(), "one block: wait for the next tick");
}

/// With the submit's head unknown, the scan asks for the head first and
/// starts `FIND_OP_LOOKBACK_BLOCKS` below it.
#[test]
fn an_unknown_submit_block_starts_below_the_first_head() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, None);
    let head = 48_480_000;
    let ops = first_window(&mut sut, head);
    let from = head - FIND_OP_LOOKBACK_BLOCKS;
    assert_eq!(ops, vec![window(from, from + FIND_OP_MAX_RANGE - 1)]);
}

/// No chain read for a plain op, nor for one the relay has acknowledged.
#[test]
fn no_chain_read_for_a_plain_or_acknowledged_op() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    for at in [T0 + 12_400.0, T0 + 24_800.0, T0 + WAIT_WINDOW_MS + 500.0] {
        let ops = tick(&mut sut, at);
        assert!(!ops.iter().any(is_find), "{ops:?}");
        settle_polls(&mut sut, at + 100.0, TrackLifecycle::NotFound);
    }

    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let _ = tick(&mut sut, T0 + 12_400.0);
    settle_polls(&mut sut, T0 + 12_500.0, TrackLifecycle::Queued);
    scan_to_head(&mut sut, T0 + 12_600.0, SUBMIT_BLOCK + 3);
    let ops = tick(&mut sut, T0 + 24_800.0);
    assert!(!ops.iter().any(is_find), "{ops:?}");
}

/// Ruling 8 over RA4: the relay's `not_found` is not enough while the chain
/// has not been read up to its head — "not sent" waits for the read.
#[test]
fn not_sent_waits_for_the_chain_read() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    for at in [T0 + 61_000.0, T0 + 73_000.0] {
        let ops = tick(&mut sut, at);
        assert!(ops.contains(&poll_status()), "{ops:?}");
        // The chain read gets no answer.
        let asked = settle_polls(&mut sut, at + 100.0, TrackLifecycle::NotFound);
        assert!(asked.is_empty(), "{asked:?}");
    }
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    // The read catches up with no event; the next not_found ends it.
    scan_to_head(&mut sut, T0 + 80_000.0, SUBMIT_BLOCK + 20);
    let ops = tick(&mut sut, T0 + 85_000.0);
    assert!(ops.contains(&poll_status()), "{ops:?}");
    scan_to_head(&mut sut, T0 + 85_100.0, SUBMIT_BLOCK + 20);
    let asked = settle_polls(&mut sut, T0 + 85_200.0, TrackLifecycle::NotFound);
    assert_eq!(asked, vec![fail_patch(TrackFailure::NotSent)]);
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);
}

/// An op resumed after a restart, its event some blocks above the stored
/// head, is found by it.
#[test]
fn a_reloaded_op_is_found_by_its_event() {
    let mut sut = Sut::new();
    let _ = sut.dispatch(Event::HomeFocused);
    let _ = sut.resolve(Res::Clock { now_ms: T0 });
    let _ = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - 600_000.0,
            maybe_sent: true,
            submit_block: Some(SUBMIT_BLOCK),
        }],
        now_ms: T0 + 100.0,
    });
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 200.0, None, None, Some(SUBMIT_BLOCK + 120)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 120)]);
    let ops = sut.resolve_matching(
        is_find,
        op_event(
            T0 + 300.0,
            Some(&event_logs(true)),
            None,
            Some(SUBMIT_BLOCK + 120),
        ),
    );
    assert_eq!(ops, event_confirmed());
}

/// Review of T019: a range error halves the window, but the window must not
/// stay small for the op's whole life. At the head the ask is only the new
/// blocks, so ONE range error there — a load-balanced node a block behind
/// the head answering "block range extends beyond current head block" —
/// used to cut the width to a block or two for good; on a fast chain
/// (Arbitrum, four blocks a second) a catch-up of one block per read never
/// reaches the head, and a may-have-been-sent op then never gets its
/// chain-backed verdict. Clean reads of the full width grow it back, up to
/// `FIND_OP_MAX_RANGE`.
#[test]
fn the_window_grows_back_after_a_range_error() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    // The first read is at the head: three new blocks.
    let ops = first_window(&mut sut, SUBMIT_BLOCK + 2);
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK + 2)]);
    let lagging = r#"{"code":-32000,"message":"block range extends beyond current head block"}"#;
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 12_600.0, None, Some(lagging), Some(SUBMIT_BLOCK + 2)),
    );
    assert_eq!(ops, vec![window(SUBMIT_BLOCK, SUBMIT_BLOCK)]);
    // The chain runs far ahead (a phone asleep, a fast chain): the catch-up
    // takes a bounded number of reads and the window is back at its widest.
    let head = SUBMIT_BLOCK + 20_000;
    let mut reads = 0_u32;
    let mut widest = 0_u64;
    let mut now = T0 + 12_700.0;
    while let Some(Op::FindOpEvent {
        from_block: Some(from),
        to_block: Some(to),
        ..
    }) = sut.outstanding().into_iter().find(is_find)
    {
        reads += 1;
        widest = widest.max(to - from + 1);
        assert!(reads <= 25, "{reads} reads and still at block {from}");
        now += 100.0;
        sut.resolve_matching(is_find, op_event(now, Some("[]"), None, Some(head)));
    }
    assert_eq!(widest, FIND_OP_MAX_RANGE);
    assert!(!sut.outstanding().iter().any(is_find), "caught up");
}

/// Review of T017: `NotSent` says the relay never had the op, so a later
/// submit of the SAME op — same nonce (never bumped for a may-have-been-sent
/// op, RA5), same calldata, gas and fees (a stable chain quotes the same) —
/// is a new life for that hash, not an echo of the dead one. Merged into the
/// terminal entry, the new submission was never polled: its record stayed
/// pending for good, and every reader of the entry (the Send receipt, the
/// signing sheet's ending) said "not sent" at once over an op the relay had
/// just accepted — the words that make a person pay again. A repeat of the
/// old hand-off (no new record) is still an echo and changes nothing.
#[test]
fn a_new_submit_of_a_never_sent_op_is_tracked_again() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert!(not_found_at(&mut sut, T0 + 30_000.0, SUBMIT_BLOCK + 6).is_empty());
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    let ops = not_found_at(&mut sut, T0 + 73_000.0, SUBMIT_BLOCK + 14);
    assert_eq!(ops, vec![fail_patch(TrackFailure::NotSent)]);
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);

    // The old hand-off again: an echo, nothing restarts.
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
        admitted: false,
    });
    assert!(!ops.contains(&poll_receipt()), "{ops:?}");
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 90_000.0,
        },
    );
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);

    // The person sends it again and the relay accepts the identical op.
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-2".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()], "tracked again");
    let view = sut.view();
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    assert_eq!(view.entries[0].outcome, TrackOutcome::Landing);
    assert_eq!(
        view.entries[0].record_ids,
        vec!["rec-2".to_owned()],
        "the dead record stays failed; only the new one is followed"
    );
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 200_000.0,
        },
    );
    let ops = sut.resolve_matching(is_receipt, receipt_confirmed(T0 + 200_300.0));
    assert!(
        ops.contains(&Op::UpdateTxRecords {
            ids: vec!["rec-2".to_owned()],
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Confirmed,
                tx_hash: Some(TX.to_owned()),
                settlement: None,
            },
        }),
        "{ops:?}"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// The same rule for the relay's own "rejected" (nothing was sent either): a
/// later accepted submit of the identical op, with new records, is followed
/// — never merged into the rejection it would otherwise read at once.
#[test]
fn a_new_submit_of_a_rejected_op_is_tracked_again() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 12_100.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_400.0)).is_empty());
    assert_eq!(
        sut.resolve(status(TrackLifecycle::Rejected, T0 + 12_500.0)),
        vec![fail_patch(TrackFailure::Refused)]
    );
    assert_eq!(entry_status(&sut), TrackStatus::Rejected);

    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-2".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    assert_eq!(view.entries[0].record_ids, vec!["rec-2".to_owned()]);
}

// ---------------------------------------------------------------------------
// Spec 082 round 2 (T186): admitted, withdrawn, and the relay's tx hash
// confirmed through the chain (RJ1, RJ4)
// ---------------------------------------------------------------------------

fn tx_receipt_op() -> Op {
    Op::TxReceipt {
        chain_id: CHAIN,
        tx_hash: EVENT_TX.to_owned(),
        user_op_hash: HASH.to_owned(),
    }
}

fn is_tx_receipt(op: &Op) -> bool {
    matches!(op, Op::TxReceipt { .. })
}

fn tx_receipt(now_ms: f64, receipt_json: Option<&str>) -> Res {
    Res::TxReceipt {
        user_op_hash: HASH.to_owned(),
        now_ms,
        receipt_json: receipt_json.map(str::to_owned),
    }
}

/// A log of the bundle transaction.
fn log(address: &str, topics: &[&str], data: &str) -> serde_json::Value {
    serde_json::json!({
        "address": address,
        "topics": topics,
        "data": data,
        "transactionHash": EVENT_TX,
        "removed": false,
    })
}

/// `eth_getTransactionReceipt` for the bundle, with `logs` in it: another
/// op's execution (and its own Safe's `ExecutionFailure`), then ours.
fn bundle_receipt(ours: Vec<serde_json::Value>) -> String {
    let other_hash = "0x1111111111111111111111111111111111111111111111111111111111111111";
    let other_event = log(
        ENTRY_POINT,
        &[USER_OPERATION_EVENT_TOPIC, other_hash],
        &format!("0x{:064x}{:064x}{:064x}{:064x}", 1, 1, 1, 1),
    );
    let other_failure = log(
        "0x2222222222222222222222222222222222222222",
        &[vela_core::app::tx_tracker::SAFE_EXECUTION_FAILURE_TOPIC],
        "0x",
    );
    let mut logs = vec![other_failure, other_event];
    logs.extend(ours);
    serde_json::json!({
        "transactionHash": EVENT_TX,
        "status": "0x1",
        "blockNumber": "0x2e3ba09",
        "logs": logs,
    })
    .to_string()
}

fn our_event(success: bool) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&event_logs(success))
        .ok()
        .and_then(|logs| logs.get(0).cloned())
        .unwrap_or_default()
}

/// Up to the relay saying `included` with its bundle tx, the relay's own
/// receipt still null (EX13): the tracker asks the chain for that tx.
fn included_with_tx(sut: &mut Sut) -> Vec<Op> {
    submitted(sut);
    let ops = tick(sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    assert!(sut.resolve(receipt_pending(T0 + 12_700.0)).is_empty());
    sut.resolve(Res::Status {
        rejection_reason: None,
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Included,
        stage: None,
        now_ms: T0 + 12_800.0,
        tx_hash: Some(EVENT_TX.to_owned()),
    })
}

/// EX13 (G38): the relay's receipt stayed null for 5 min 49 s while its
/// status said `included` with the tx hash. The tracker now reads that tx's
/// receipt from the chain: one request per hash in flight, and the op's own
/// event in it confirms — records, the token-trust hand-off and the balance,
/// as for a relay receipt.
#[test]
fn an_included_tx_hash_is_confirmed_through_the_chain() {
    let mut sut = Sut::new();
    let ops = included_with_tx(&mut sut);
    assert_eq!(ops, vec![tx_receipt_op()], "asked at once");
    // One in flight per hash: a tick before it answers asks nothing new.
    let ops = tick(&mut sut, T0 + 14_000.0);
    assert!(!ops.iter().any(is_tx_receipt), "{ops:?}");
    // Not mined yet (a null receipt): asked again at the receipt cadence,
    // which counts from when the last ask went out (12.8 s), not from this
    // answer (issue #464).
    let ops = sut.resolve_matching(is_tx_receipt, tx_receipt(T0 + 14_100.0, Some("null")));
    assert!(!ops.iter().any(is_tx_receipt), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    while sut.outstanding().iter().any(is_receipt) {
        sut.resolve_matching(is_receipt, receipt_pending(T0 + 14_200.0));
    }
    let ops = tick(&mut sut, T0 + 15_900.0);
    assert!(ops.contains(&tx_receipt_op()), "{ops:?}");
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 16_100.0, Some(&bundle_receipt(vec![our_event(true)]))),
    );
    // The whole receipt was read: the op's own logs say it moved nothing
    // for the account (spec 097) — unlike the event alone, which says
    // nothing either way.
    let mut expected = event_confirmed();
    if let Op::UpdateTxRecords { patch, .. } = &mut expected[0] {
        patch.settlement = Some(TrackSettlement {
            moved: Some(Vec::new()),
            failure: None,
        });
    }
    assert_eq!(ops, expected, "another op's failure is not ours");
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Confirmed);
    assert_eq!(view.entries[0].tx_hash.as_deref(), Some(EVENT_TX));
    assert_eq!(view.entries[0].outcome, TrackOutcome::Final);
}

/// A Safe `ExecutionFailure` inside the op's own logs is a failed payment,
/// whatever the EntryPoint's `success` says (#D1) — gas was spent.
#[test]
fn an_execution_failure_in_the_op_s_own_logs_fails_it() {
    let mut sut = Sut::new();
    included_with_tx(&mut sut);
    let failure = log(
        "0x3333333333333333333333333333333333333333",
        &[vela_core::app::tx_tracker::SAFE_EXECUTION_FAILURE_TOPIC],
        "0x",
    );
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(
            T0 + 13_000.0,
            Some(&bundle_receipt(vec![failure, our_event(true)])),
        ),
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );
    assert_eq!(entry_status(&sut), TrackStatus::Dropped);

    // The op's event saying `success = false` is a failure too.
    let mut sut = Sut::new();
    included_with_tx(&mut sut);
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 13_000.0, Some(&bundle_receipt(vec![our_event(false)]))),
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );
}

/// A mined tx without the op's own event (a replaced bundle) proves nothing:
/// ignored; and no answer at all keeps asking.
#[test]
fn a_receipt_without_the_op_s_event_is_ignored() {
    let mut sut = Sut::new();
    included_with_tx(&mut sut);
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 13_000.0, Some(&bundle_receipt(vec![]))),
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    while sut.outstanding().iter().any(is_receipt) {
        sut.resolve_matching(is_receipt, receipt_pending(T0 + 13_100.0));
    }
    let ops = tick(&mut sut, T0 + 16_100.0);
    assert!(ops.contains(&tx_receipt_op()), "{ops:?}");
    let ops = sut.resolve_matching(is_tx_receipt, tx_receipt(T0 + 16_300.0, None));
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
}

/// A late chain receipt never double-resolves an op the relay's receipt
/// already confirmed.
#[test]
fn a_late_tx_receipt_changes_nothing_after_the_relay_s() {
    let mut sut = Sut::new();
    included_with_tx(&mut sut);
    let ops = tick(&mut sut, T0 + 15_800.0);
    assert!(ops.contains(&poll_receipt()), "{ops:?}");
    let ops = sut.resolve_matching(is_receipt, receipt_confirmed(T0 + 16_000.0));
    assert_eq!(
        ops,
        vec![confirm_patch(), notify_confirmed(), holdings_moved()]
    );
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 16_100.0, Some(&bundle_receipt(vec![our_event(false)]))),
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// RJ1: the write-ahead hands the op over "may have been sent" before the
/// POST; the relay accepting it hands it over again `admitted`. An accepted
/// op never reads "may have been sent".
#[test]
fn an_admitted_hand_off_is_never_maybe_sent() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent, "before the POST");
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: Some(SUBMIT_BLOCK),
        admitted: true,
    });
    assert!(
        !ops.contains(&poll_receipt()),
        "one entry, one poll line: {ops:?}"
    );
    assert_eq!(outcome_of(&sut), TrackOutcome::Landing);
    let view = sut.view();
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].record_ids, vec!["rec-1".to_owned()]);
    // A `not_found` past the grace no longer counts: the relay has it.
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock { now_ms: T0 + 500.0 },
    );
    assert!(not_found_at(&mut sut, T0 + 70_000.0, SUBMIT_BLOCK + 5).is_empty());
    assert!(not_found_at(&mut sut, T0 + 90_000.0, SUBMIT_BLOCK + 9).is_empty());
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    // A round-1 shell's hand-off (no `admitted`) still reads.
    let old: Option<Event> = serde_json::from_str(&format!(
        r#"{{"type":"submitted","user_op_hash":"{HASH}","record_ids":[],"chain_id":1}}"#
    ))
    .ok();
    assert!(matches!(
        old,
        Some(Event::Submitted {
            admitted: false,
            ..
        })
    ));
}

/// RJ1: a proven "not sent" withdraws the write-ahead — the entry its
/// hand-off made goes, with no patch and no balance read. The same hash
/// submitted again afterwards starts fresh. (Since the second review the
/// write-ahead's hand-off names no record: the POST's verdict does.)
#[test]
fn a_withdrawn_op_is_forgotten_and_a_resubmit_starts_fresh() {
    let mut sut = Sut::new();
    posting(&mut sut);
    let ops = sut.dispatch(Event::Withdrawn {
        user_op_hash: HASH.to_uppercase().replace("0X", "0x"),
        record_ids: vec!["rec-1".to_owned()],
    });
    assert!(ops.is_empty(), "no patch, no balance read: {ops:?}");
    assert!(sut.view().entries.is_empty());
    assert!(tick_ops(&mut sut).is_empty(), "nothing left to poll");

    let ops = sut.dispatch(posting_hand_off());
    assert_eq!(ops, vec![Op::Now, poll_receipt()], "tracked from the start");
    assert_eq!(sut.view().entries.len(), 1);

    // A split's write-ahead, its records swept in from the store while the
    // POST is out: withdrawn together, the entry goes.
    let mut split = Sut::new();
    split.dispatch(posting_hand_off());
    split.resolve_matching(|op| matches!(op, Op::Now), Res::Clock { now_ms: T0 });
    split.dispatch(Event::HomeFocused);
    split.resolve_matching(|op| matches!(op, Op::Now), Res::Clock { now_ms: T0 + 1.0 });
    split.resolve_matching(
        |op| matches!(op, Op::LoadPendingTxs),
        Res::RecordsLoaded {
            records: [0, 1]
                .map(|i| TrackPendingRecord {
                    sender: None,
                    record_id: format!("{HASH}-{i}"),
                    user_op_hash: HASH.to_owned(),
                    chain_id: CHAIN,
                    submitted_at_ms: T0,
                    maybe_sent: true,
                    submit_block: Some(SUBMIT_BLOCK),
                })
                .to_vec(),
            now_ms: T0 + 2.0,
        },
    );
    assert_eq!(split.view().entries[0].record_ids.len(), 2);
    split.dispatch(Event::Withdrawn {
        user_op_hash: HASH.to_owned(),
        record_ids: vec![format!("{HASH}-0"), format!("{HASH}-1")],
    });
    assert!(split.view().entries.is_empty());
    // An unknown hash is nothing.
    assert!(split
        .dispatch(Event::Withdrawn {
            user_op_hash: "0xdead".to_owned(),
            record_ids: vec![],
        })
        .is_empty());
}

/// `Tick` with nothing live asks nothing.
fn tick_ops(sut: &mut Sut) -> Vec<Op> {
    sut.dispatch(Event::Tick)
}

// ===========================================================================
// Spec 082 round 2 — adversarial review of T186
// ===========================================================================

/// The write-ahead hands the op over BEFORE its POST (RJ1), so the not-found
/// grace can run out while a slow POST is still going (a 15 s relay timeout
/// per endpoint, "currently processing" retries). `NotSent` is a verdict on
/// an op the relay never showed it holds; the relay then taking it (the
/// `admitted` hand-off) proves the verdict came too early. Left terminal,
/// the accepted op was never polled again and every reader of the entry said
/// "not sent" over it — the words that make a person pay again. An admitted
/// hand-off can never FOLLOW a real `NotSent` (admitted ops are never in
/// doubt), so it revives only a premature one; a `Rejected` after the relay
/// took the op (DX-W3) stays terminal under the hand-off's echo.
#[test]
fn a_not_sent_reached_while_the_post_was_out_yields_to_the_relay_taking_it() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert!(not_found_at(&mut sut, T0 + 30_000.0, SUBMIT_BLOCK + 6).is_empty());
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    let ops = not_found_at(&mut sut, T0 + 73_000.0, SUBMIT_BLOCK + 14);
    assert_eq!(ops, vec![fail_patch(TrackFailure::NotSent)]);
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);

    // The POST that was still out comes back Accepted.
    let admitted = Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: Some(SUBMIT_BLOCK),
        admitted: true,
    };
    let ops = sut.dispatch(admitted.clone());
    assert_eq!(ops, vec![Op::Now, poll_receipt()], "tracked again");
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 74_000.0,
        },
    );
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    assert_ne!(view.entries[0].outcome, TrackOutcome::MaybeSent);
    // It lands: the records are confirmed over the early failed patch.
    let ops = sut.resolve_matching(is_receipt, receipt_confirmed(T0 + 80_000.0));
    assert_eq!(ops.first(), Some(&confirm_patch()), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
    // Its echo changes nothing.
    let ops = sut.dispatch(admitted);
    assert!(!ops.contains(&poll_receipt()), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);

    // DX-W3: taken, then refused — the echo of the admitted hand-off after
    // the refusal is an echo, not a new life.
    let mut refused = Sut::new();
    refused.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: true,
    });
    refused.resolve(Res::Clock { now_ms: T0 });
    refused.resolve(receipt_pending(T0 + 300.0));
    let _ = tick(&mut refused, T0 + 12_400.0);
    while refused.outstanding().iter().any(is_status) {
        refused.resolve_matching(is_status, status(TrackLifecycle::Rejected, T0 + 12_500.0));
    }
    assert_eq!(entry_status(&refused), TrackStatus::Rejected);
    let ops = refused.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: None,
        admitted: true,
    });
    assert!(!ops.contains(&poll_receipt()), "{ops:?}");
    assert_eq!(entry_status(&refused), TrackStatus::Rejected);
}

// ===========================================================================
// Spec 082 round 2 — second review: the grace counts from the POST's end
// ===========================================================================

/// The write-ahead's hand-off (RJ1): the op, before its POST, with no record
/// attached yet — its records come with the POST's verdict.
fn posting_hand_off() -> Event {
    Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec![],
        chain_id: CHAIN,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
        admitted: false,
    }
}

/// The POST's verdict "may have been sent", naming the op's record.
fn maybe_sent_verdict() -> Event {
    Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
        admitted: false,
    }
}

/// Hand the op over before its POST, at `T0`.
fn posting(sut: &mut Sut) {
    let ops = sut.dispatch(posting_hand_off());
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    assert!(sut.resolve(receipt_pending(T0 + 300.0)).is_empty());
}

/// The not-found grace (RA4) is there because a relay may not have written a
/// just-POSTed op down yet. The write-ahead hands the op over BEFORE its
/// POST, so counted from that hand-off the grace ran while nothing had been
/// sent: a POST longer than a minute (15 s per relay endpoint, "currently
/// processing" retries) ended the op NotSent before its bytes were even
/// out — and a POST that then came back "may have been sent" was never told
/// to the tracker. The op, possibly landing, stayed "not sent" for good.
#[test]
fn an_op_whose_post_is_out_is_never_not_sent() {
    let mut sut = Sut::new();
    posting(&mut sut);
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
    for (at, head) in [(61_000.0, 12), (73_000.0, 14), (85_000.0, 16)] {
        let ops = not_found_at(&mut sut, T0 + at, SUBMIT_BLOCK + head);
        assert!(ops.is_empty(), "at {at}: {ops:?}");
        assert_eq!(entry_status(&sut), TrackStatus::Pending, "at {at}");
    }
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
}

/// The grace of a may-have-been-sent op counts from its POST's verdict: two
/// `not_found` past a minute AFTER it, the chain read to its head, and only
/// then "not sent" — the op's record failed.
#[test]
fn the_grace_counts_from_the_post_s_verdict() {
    let mut sut = Sut::new();
    posting(&mut sut);
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    assert!(not_found_at(&mut sut, T0 + 73_000.0, SUBMIT_BLOCK + 14).is_empty());
    // The POST comes back at 80 s: may have been sent.
    let ops = sut.dispatch(maybe_sent_verdict());
    assert!(ops.contains(&Op::Now), "{ops:?}");
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 80_000.0,
        },
    );
    assert_eq!(sut.view().entries[0].record_ids, vec!["rec-1".to_owned()]);
    for (at, head) in [(92_000.0, 18), (104_000.0, 20), (128_000.0, 24)] {
        let ops = not_found_at(&mut sut, T0 + at, SUBMIT_BLOCK + head);
        assert!(
            ops.is_empty(),
            "inside the verdict's grace at {at}: {ops:?}"
        );
    }
    assert!(not_found_at(&mut sut, T0 + 141_000.0, SUBMIT_BLOCK + 26).is_empty());
    let ops = not_found_at(&mut sut, T0 + 153_000.0, SUBMIT_BLOCK + 28);
    assert_eq!(ops, vec![fail_patch(TrackFailure::NotSent)]);
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);
}

/// Proven never sent while the POST was out: the withdrawal forgets the op —
/// no patch, no balance read, nothing left to poll.
#[test]
fn a_withdrawn_post_is_forgotten() {
    let mut sut = Sut::new();
    posting(&mut sut);
    let ops = sut.dispatch(Event::Withdrawn {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
    });
    assert!(ops.is_empty(), "{ops:?}");
    assert!(sut.view().entries.is_empty());
    assert!(tick_ops(&mut sut).is_empty());
}

/// The chain shows the op landed while its POST was still out: confirmed at
/// once; the records the verdict then names are patched confirmed with the
/// event's tx hash.
#[test]
fn records_named_after_a_landing_take_its_patch() {
    let mut sut = Sut::new();
    posting(&mut sut);
    let ops = tick(&mut sut, T0 + 12_400.0);
    assert!(ops.iter().any(is_find), "{ops:?}");
    sut.resolve_matching(
        is_find,
        op_event(T0 + 12_500.0, None, None, Some(SUBMIT_BLOCK + 5)),
    );
    let ops = sut.resolve_matching(
        is_find,
        op_event(
            T0 + 12_600.0,
            Some(&event_logs(true)),
            None,
            Some(SUBMIT_BLOCK + 5),
        ),
    );
    assert!(
        ops.contains(&Op::HoldingsMoved { chain_id: CHAIN }),
        "{ops:?}"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
    let ops = sut.dispatch(maybe_sent_verdict());
    assert!(
        ops.contains(&Op::UpdateTxRecords {
            ids: vec!["rec-1".to_owned()],
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Confirmed,
                tx_hash: Some(EVENT_TX.to_owned()),
                settlement: None,
            },
        }),
        "{ops:?}"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Confirmed);
}

/// The identical op POSTed again while an earlier POST of it may still land
/// (the Send's ids are its hash; a may-have-been-sent op keeps its nonce, so
/// the hash repeats). The second POST changes nothing for the first, and its
/// proven "not sent" takes nothing away from it: the earlier op is still
/// followed, its record still named.
#[test]
fn a_second_post_of_the_same_op_never_withdraws_the_first() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    let ops = sut.dispatch(posting_hand_off());
    assert!(!ops.contains(&poll_receipt()), "{ops:?}");
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 20_000.0,
        },
    );
    let ops = sut.dispatch(Event::Withdrawn {
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
    });
    assert!(ops.is_empty(), "{ops:?}");
    let view = sut.view();
    assert_eq!(view.entries.len(), 1, "the earlier op is still followed");
    assert_eq!(view.entries[0].record_ids, vec!["rec-1".to_owned()]);
    assert_eq!(view.entries[0].outcome, TrackOutcome::MaybeSent);
}

/// The identical op POSTed again after a verdict that it was never sent is a
/// new submission: tracked from the start, with the new POST's grace.
#[test]
fn a_post_after_not_sent_is_tracked_again() {
    let mut sut = Sut::new();
    submitted_maybe(&mut sut, Some(SUBMIT_BLOCK));
    assert!(not_found_at(&mut sut, T0 + 61_000.0, SUBMIT_BLOCK + 12).is_empty());
    assert_eq!(
        not_found_at(&mut sut, T0 + 73_000.0, SUBMIT_BLOCK + 14),
        vec![fail_patch(TrackFailure::NotSent)]
    );
    let ops = sut.dispatch(posting_hand_off());
    assert_eq!(ops, vec![Op::Now, poll_receipt()], "tracked again");
    let _ = sut.resolve_matching(
        |op| matches!(op, Op::Now),
        Res::Clock {
            now_ms: T0 + 90_000.0,
        },
    );
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);
}

// ===========================================================================
// Spec 082 round 2 — second adversarial review (paths: a Safe inner revert,
// the NotSent proof on a reload)
// ===========================================================================

/// The relay's status for an op in a mined bundle whose own event says it
/// failed — an on-chain revert, gas spent (a swap's slippage between the
/// relay's simulation and the block; a Safe call reverting inside the op,
/// which module v0.3.0 turns into `success = false`). The relay marks every
/// member of a mined bundle `rejected` unless its event succeeded
/// (`mark_bundle_confirmed`, both relay shells), and names the bundle tx.
fn rejected_on_chain(now_ms: f64) -> Res {
    Res::Status {
        rejection_reason: None,
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: None,
        now_ms,
        tx_hash: Some(EVENT_TX.to_owned()),
    }
}

/// A `rejected` that names the bundle tx is not "refused before any block":
/// the op reached a block. Read as a refusal it was terminal at once — the
/// records failed with no balance re-read (gas was spent, RE8), the dApp
/// answered "the network refused this transaction; nothing was sent" (RJ3)
/// for an op that is on chain (ruling 9 answers its tx hash), and the Send
/// receipt said the fee was rejected. The chain decides: the op's own event
/// in that tx fails it — Dropped, with the tx hash and the balance read.
#[test]
fn a_rejected_status_naming_the_bundle_tx_is_read_from_the_chain() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let ops = tick(&mut sut, T0 + 12_400.0);
    assert_eq!(ops, vec![poll_receipt(), poll_status()]);
    // The status answers first (the relay's receipt read raced its store).
    let ops = sut.resolve_matching(is_status, rejected_on_chain(T0 + 12_600.0));
    assert_eq!(ops, vec![tx_receipt_op()], "no verdict yet: ask the chain");
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Pending);
    assert!(view.entries[0].polling);
    assert!(sut
        .resolve_matching(is_receipt, receipt_pending(T0 + 12_700.0))
        .is_empty());
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 12_900.0, Some(&bundle_receipt(vec![our_event(false)]))),
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );
    let view = sut.view();
    assert_eq!(view.entries[0].status, TrackStatus::Dropped);
    assert_eq!(view.entries[0].tx_hash.as_deref(), Some(EVENT_TX));
    assert_eq!(view.entries[0].outcome, TrackOutcome::Final);

    // The relay's own receipt (success = false) settles it the same way.
    let mut sut = Sut::new();
    submitted(&mut sut);
    let _ = tick(&mut sut, T0 + 12_400.0);
    let _ = sut.resolve_matching(is_status, rejected_on_chain(T0 + 12_600.0));
    let ops = sut.resolve_matching(
        is_receipt,
        Res::ReceiptFailed {
            user_op_hash: HASH.to_owned(),
            tx_hash: EVENT_TX.to_owned(),
            now_ms: T0 + 12_700.0,
        },
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Reverted), holdings_moved()]
    );
    assert_eq!(entry_status(&sut), TrackStatus::Dropped);
}

/// The chain shows that bundle tx mined without the op: it never reached a
/// block, and the relay's refusal stands — Rejected, no balance read. Not
/// mined yet (`null`) is no verdict: asked again.
#[test]
fn a_rejected_op_absent_from_its_mined_bundle_is_refused() {
    let mut sut = Sut::new();
    submitted(&mut sut);
    let _ = tick(&mut sut, T0 + 12_400.0);
    let _ = sut.resolve_matching(is_status, rejected_on_chain(T0 + 12_600.0));
    let _ = sut.resolve_matching(is_receipt, receipt_pending(T0 + 12_700.0));
    let ops = sut.resolve_matching(is_tx_receipt, tx_receipt(T0 + 12_900.0, Some("null")));
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::Pending);
    let ops = tick(&mut sut, T0 + 16_000.0);
    assert!(ops.contains(&tx_receipt_op()), "{ops:?}");
    let ops = sut.resolve_matching(
        is_tx_receipt,
        tx_receipt(T0 + 16_200.0, Some(&bundle_receipt(vec![]))),
    );
    assert_eq!(
        ops,
        vec![fail_patch(TrackFailure::Refused)],
        "nothing landed: no balance read"
    );
    assert_eq!(entry_status(&sut), TrackStatus::Rejected);

    // A refusal that names no bundle tx is terminal at once, as before.
    let mut sut = Sut::new();
    submitted(&mut sut);
    let _ = tick(&mut sut, T0 + 12_400.0);
    let ops = sut.resolve_matching(is_status, status(TrackLifecycle::Rejected, T0 + 12_600.0));
    assert_eq!(ops, vec![fail_patch(TrackFailure::Refused)]);
    assert_eq!(entry_status(&sut), TrackStatus::Rejected);
}

/// Resume a may-have-been-sent record stored `age_ms` before `T0` with no
/// head read before its POST, and read the chain from below the first head
/// up to `head`: returns once the scan has caught up.
fn resumed_without_a_submit_block(sut: &mut Sut, age_ms: f64, head: u64) {
    assert_eq!(sut.dispatch(Event::AppResumed), vec![Op::Now]);
    assert_eq!(
        sut.resolve(Res::Clock { now_ms: T0 }),
        vec![Op::LoadPendingTxs]
    );
    let ops = sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            sender: None,
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - age_ms,
            maybe_sent: true,
            submit_block: None,
        }],
        now_ms: T0 + 100.0,
    });
    assert_eq!(ops, vec![poll_receipt(), poll_status(), head_only()]);
    let ops = sut.resolve_matching(is_find, op_event(T0 + 200.0, None, None, Some(head)));
    let from = head - FIND_OP_LOOKBACK_BLOCKS;
    assert_eq!(ops, vec![window(from, from + FIND_OP_MAX_RANGE - 1)]);
}

/// A write-ahead record left by a quit during its POST — its head read
/// failed on the same bad network, so no submit block — resumed three hours
/// later. The relay forgets an op an hour after admitting it (its status
/// records' TTL) and answers `not_found` for one that landed long ago. The
/// chain read starts `FIND_OP_LOOKBACK_BLOCKS` below the head read NOW —
/// 21 minutes on Arbitrum, under three hours on Base — above the landing:
/// caught up with no event, it proved nothing, yet two `not_found` ended a
/// landed op "not sent", its record failed. A lookback anchored when the op
/// was older than the lookback covers may find the event, never prove it
/// absent: the op stays "may have been sent" (then "unknown" at 24 h).
#[test]
fn a_lookback_that_cannot_reach_the_submit_never_proves_not_sent() {
    let head = 48_480_000;
    let mut sut = Sut::new();
    resumed_without_a_submit_block(&mut sut, 3.0 * 3_600_000.0, head);
    scan_to_head(&mut sut, T0 + 300.0, head);
    assert!(settle_polls(&mut sut, T0 + 400.0, TrackLifecycle::NotFound).is_empty());
    let ops = not_found_at(
        &mut sut,
        T0 + 400.0 + SLOWEST_RECEIPT_INTERVAL_MS,
        head + 1_200,
    );
    assert!(ops.is_empty(), "no proof the op never landed: {ops:?}");
    let ops = not_found_at(
        &mut sut,
        T0 + 800.0 + 2.0 * SLOWEST_RECEIPT_INTERVAL_MS,
        head + 2_400,
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(entry_status(&sut), TrackStatus::AcceptedNotLanded);
    assert_eq!(outcome_of(&sut), TrackOutcome::MaybeSent);

    // The same scan still finds an event inside the lookback.
    let mut sut = Sut::new();
    resumed_without_a_submit_block(&mut sut, 3.0 * 3_600_000.0, head);
    let ops = sut.resolve_matching(
        is_find,
        op_event(T0 + 300.0, Some(&event_logs(true)), None, Some(head)),
    );
    assert_eq!(ops, event_confirmed());

    // Resumed soon enough that the lookback reaches the submit: the chain
    // read joins the relay's word, as before.
    let mut sut = Sut::new();
    resumed_without_a_submit_block(&mut sut, 5.0 * 60_000.0, head);
    scan_to_head(&mut sut, T0 + 300.0, head);
    assert!(settle_polls(&mut sut, T0 + 400.0, TrackLifecycle::NotFound).is_empty());
    let ops = not_found_at(&mut sut, T0 + 400.0 + RECONCILE_MIN_INTERVAL_MS, head + 50);
    assert_eq!(ops, vec![fail_patch(TrackFailure::NotSent)]);
    assert_eq!(entry_status(&sut), TrackStatus::NotSent);
}

// ---------------------------------------------------------------------------
// 083 H2 — a dApp's operation settles like a send's
// ---------------------------------------------------------------------------

/// Drive the sign machine from a site's `eth_sendTransaction` to the moment
/// the bundler accepts it; answer the record it persisted and the handoff.
fn dapp_submitted() -> (String, vela_core::app::sign_request::SignTrackerHandoff) {
    use vela_core::app::sign_request::{
        Event as SignEvent, SignAccountRef, SignApproveOpts, SignOperation as SignOp, SignRequest,
        SignShellResult as SignRes,
    };
    let mut sign = DomainDriver::<SignRequest>::new();
    sign.dispatch(SignEvent::NetworksChanged {
        chain_ids: vec![CHAIN],
    });
    sign.dispatch(SignEvent::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sign.dispatch(SignEvent::RequestArrived {
        id: "rid-h2".to_owned(),
        method: "eth_sendTransaction".to_owned(),
        params_json:
            r#"[{"to":"0x3333333333333333333333333333333333333333","data":"0x","value":"0x1"}]"#
                .to_owned(),
        origin: "http://127.0.0.1:5173".to_owned(),
        transport_id: "tab-1".to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(CHAIN),
        dapp: None,
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: T0,
        first_party: false,
    });
    sign.dispatch(SignEvent::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    let ops = sign.resolve(SignRes::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [SignOp::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    let ops = sign.dispatch(SignEvent::OpSubmitted {
        id: "rid-h2".to_owned(),
        user_op_hash: HASH.to_owned(),
        now_ms: T0,
        maybe_sent: false,
        submit_block: None,
    });
    let record_id = match ops.as_slice() {
        [SignOp::PersistRecord { record }] => record.record_id.clone(),
        other => panic!("the pending record first: {other:?}"),
    };
    let handoff = sign.view().tracker_handoff.expect("the op is handed over");
    (record_id, handoff)
}

/// The sign machine hands the tracker the record it just persisted, and the
/// tracker's verdict patches exactly that record — the one the Activity row
/// reads its status from. Landed…
#[test]
fn a_dapp_operation_lands_on_the_record_the_sign_path_persisted() {
    let (record_id, handoff) = dapp_submitted();
    assert_eq!(handoff.record_ids, vec![record_id.clone()]);

    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: handoff.user_op_hash,
        record_ids: handoff.record_ids,
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    });
    assert_eq!(ops, vec![Op::Now, poll_receipt()]);
    assert!(sut.resolve(Res::Clock { now_ms: T0 }).is_empty());
    let ops = sut.resolve(receipt_confirmed(T0 + 300.0));
    assert_eq!(
        ops.first(),
        Some(&Op::UpdateTxRecords {
            ids: vec![record_id],
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Confirmed,
                tx_hash: Some(TX.to_owned()),
                settlement: None,
            },
        })
    );
}

/// …or failed, the only way a record may be: a definitive drop.
#[test]
fn a_dropped_dapp_operation_fails_the_record_the_sign_path_persisted() {
    let (record_id, handoff) = dapp_submitted();

    let mut sut = Sut::new();
    sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: handoff.user_op_hash,
        record_ids: handoff.record_ids,
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    });
    sut.resolve(Res::Clock { now_ms: T0 });
    let ops = sut.resolve(Res::ReceiptFailed {
        user_op_hash: HASH.to_owned(),
        tx_hash: TX.to_owned(),
        now_ms: T0 + 300.0,
    });
    assert_eq!(
        ops,
        vec![
            Op::UpdateTxRecords {
                ids: vec![record_id],
                patch: TrackRecordPatch {
                    status: TrackRecordStatus::Failed,
                    tx_hash: None,
                    settlement: Some(TrackSettlement {
                        moved: None,
                        failure: Some(TrackFailure::Reverted),
                    }),
                },
            },
            // Included, so its fee moved (spec 082).
            Op::HoldingsMoved { chain_id: CHAIN },
        ]
    );
}

// ---------------------------------------------------------------------------
// A refusal's reason (relay `fix/held-nonce-and-floor` §2), and the one
// operation in flight per account and network
// ---------------------------------------------------------------------------

const SENDER: &str = "0xAbCdEf0000000000000000000000000000000001";

/// Submitted by `SENDER`, polled to the first status question.
fn submitted_by(sut: &mut Sut, sender: Option<&str>, maybe_sent: bool) {
    sut.dispatch(Event::Submitted {
        sender: sender.map(str::to_owned),
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent,
        submit_block: None,
        admitted: false,
    });
    sut.resolve(Res::Clock { now_ms: T0 });
    sut.resolve(receipt_pending(T0 + 300.0));
}

fn refused(sut: &mut Sut, rejection_reason: Option<&str>, stage: Option<&str>) {
    tick(sut, T0 + 12_100.0);
    sut.resolve(receipt_pending(T0 + 12_400.0));
    sut.resolve(Res::Status {
        user_op_hash: HASH.to_owned(),
        status: TrackLifecycle::Rejected,
        stage: stage.map(str::to_owned),
        now_ms: T0 + 12_500.0,
        tx_hash: None,
        rejection_reason: rejection_reason.map(str::to_owned),
    });
}

/// The relay's reason decides the sentence: the fee words only for a fee
/// refusal. An older relay names no reason; its stage does (the relay
/// contract's own derivation). Neither: the plain "refused".
#[test]
fn a_refusal_carries_its_reason_and_its_sentence() {
    use vela_core::app::tx_tracker::RefusalReason as R;
    for (reason, stage, want, key) in [
        (
            Some("nonce_used"),
            None,
            Some(R::NonceUsed),
            "componentsUi.signing.wentFirst",
        ),
        (
            Some("fee_below_market"),
            Some("in_band_settlement"),
            Some(R::FeeBelowMarket),
            "send.txRejectedFees",
        ),
        (
            Some("simulation_failed"),
            None,
            Some(R::SimulationFailed),
            "componentsUi.signing.refused",
        ),
        // A reason this core does not know yet.
        (
            Some("relay_learned_a_new_word"),
            None,
            Some(R::Unknown),
            "componentsUi.signing.refused",
        ),
        // An older relay: the stage says it.
        (
            None,
            Some("nonce"),
            Some(R::NonceUsed),
            "componentsUi.signing.wentFirst",
        ),
        (
            None,
            Some("in_band_settlement"),
            Some(R::FeeBelowMarket),
            "send.txRejectedFees",
        ),
        (
            None,
            Some(FEE_HOLD_STAGE),
            Some(R::FeeBelowMarket),
            "send.txRejectedFees",
        ),
        (
            None,
            Some("dead_letter"),
            Some(R::RelayGaveUp),
            "componentsUi.signing.refused",
        ),
        (None, None, None, "componentsUi.signing.refused"),
    ] {
        let mut sut = Sut::new();
        submitted_by(&mut sut, Some(SENDER), false);
        refused(&mut sut, reason, stage);
        let entry = &sut.view().entries[0];
        assert_eq!(entry.status, TrackStatus::Rejected);
        assert_eq!(entry.refusal, want, "{reason:?} / {stage:?}");
        assert_eq!(
            entry.refusal_key.as_deref(),
            Some(key),
            "{reason:?} / {stage:?}"
        );
    }
}

/// Only a refusal has a reason and a sentence.
#[test]
fn a_live_op_has_no_refusal() {
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    let entry = &sut.view().entries[0];
    assert_eq!((entry.refusal, entry.refusal_key.as_deref()), (None, None));
}

/// The status method's `rejection_reason` is read, and an older relay's
/// absence of it is no answer.
#[test]
fn the_status_parser_reads_the_rejection_reason() {
    let parsed = parse_user_op_status(
        r#"{"status":"rejected","last_executor_stage":"nonce","rejection_reason":"nonce_used"}"#,
    )
    .unwrap();
    assert_eq!(parsed.rejection_reason.as_deref(), Some("nonce_used"));
    let older =
        parse_user_op_status(r#"{"status":"rejected","last_executor_stage":"nonce"}"#).unwrap();
    assert_eq!(older.rejection_reason, None);
}

/// An accepted op of an account, not final, holds its nonce on its chain:
/// the second of that account's sends waits for it. Lower-cased, so any
/// casing matches.
#[test]
fn an_accepted_op_in_flight_holds_its_account_s_nonce() {
    use vela_core::app::tx_tracker::{in_flight_ops, in_flight_ops_json, previous_in_flight};
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    let ops = in_flight_ops(&sut.view());
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].sender, SENDER.to_lowercase());
    assert_eq!(ops[0].chain_id, CHAIN);
    assert!(previous_in_flight(&ops, SENDER, CHAIN, None).is_some());
    assert!(previous_in_flight(
        &ops,
        &SENDER.to_uppercase().replace("0X", "0x"),
        CHAIN,
        None
    )
    .is_some());
    assert!(
        previous_in_flight(&ops, SENDER, 1, None).is_none(),
        "another chain"
    );
    assert!(
        previous_in_flight(&ops, SENDER, CHAIN, Some(HASH)).is_none(),
        "never the asking surface's own op"
    );
    // The JSON bridge says the same.
    let json = serde_json::to_string(&sut.view()).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&in_flight_ops_json(&json)).unwrap(),
        serde_json::to_value(&ops).unwrap()
    );
    assert_eq!(in_flight_ops_json("not json"), "[]");

    // Final: it holds nothing.
    let ops = tick(&mut sut, T0 + 3_500.0);
    assert!(ops.contains(&poll_receipt()), "{ops:?}");
    sut.resolve_matching(
        |op| matches!(op, Op::PollReceipt { .. }),
        receipt_confirmed(T0 + 3_800.0),
    );
    assert!(in_flight_ops(&sut.view()).is_empty());
}

/// A refused op, one whose reply was lost and the relay has not shown it
/// holds, and one no shell named a sender for, hold nothing.
#[test]
fn only_an_op_the_relay_holds_and_whose_sender_is_known_waits() {
    use vela_core::app::tx_tracker::in_flight_ops;
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    refused(&mut sut, Some("simulation_failed"), None);
    assert!(in_flight_ops(&sut.view()).is_empty(), "refused");

    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), true);
    assert_eq!(sut.view().entries[0].outcome, TrackOutcome::MaybeSent);
    assert!(in_flight_ops(&sut.view()).is_empty(), "may have been sent");

    let mut sut = Sut::new();
    submitted_by(&mut sut, None, false);
    assert!(in_flight_ops(&sut.view()).is_empty(), "no sender");
}

/// After a restart the device still knows: the reloaded pending record
/// carries its sender.
#[test]
fn a_restart_still_knows_the_op_in_flight() {
    use vela_core::app::tx_tracker::in_flight_ops;
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AppResumed);
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: T0 });
    assert_eq!(ops, vec![Op::LoadPendingTxs]);
    sut.resolve(Res::RecordsLoaded {
        records: vec![TrackPendingRecord {
            record_id: "rec-1".to_owned(),
            user_op_hash: HASH.to_owned(),
            chain_id: CHAIN,
            submitted_at_ms: T0 - 30_000.0,
            maybe_sent: false,
            submit_block: None,
            sender: Some(SENDER.to_owned()),
        }],
        now_ms: T0,
    });
    let ops = in_flight_ops(&sut.view());
    assert_eq!(ops.len(), 1, "{:?}", sut.view());
    assert_eq!(ops[0].sender, SENDER.to_lowercase());
}

// ---------------------------------------------------------------------------
// The hold's time limit: final, or ten minutes without progress
// ---------------------------------------------------------------------------

const MIN: f64 = 60_000.0;

/// One tick at `now_ms`, answering every receipt and status poll it (or an
/// earlier tick) left out: receipts `pending`; statuses with `word`, the
/// relay's `(lifecycle, stage)` — `None` = the relay could not be reached.
fn advance(sut: &mut Sut, now_ms: f64, word: Option<(TrackLifecycle, Option<&str>)>) {
    tick(sut, now_ms);
    while sut.outstanding().iter().any(is_receipt) {
        let answer = match word {
            Some(_) => receipt_pending(now_ms),
            None => Res::ReceiptUnreachable {
                user_op_hash: HASH.to_owned(),
                now_ms,
            },
        };
        sut.resolve_matching(is_receipt, answer);
    }
    while sut.outstanding().iter().any(is_status) {
        let answer = match word {
            Some((status, stage)) => Res::Status {
                user_op_hash: HASH.to_owned(),
                status,
                stage: stage.map(str::to_owned),
                now_ms,
                tx_hash: None,
                rejection_reason: None,
            },
            None => Res::StatusUnavailable {
                user_op_hash: HASH.to_owned(),
                now_ms,
            },
        };
        sut.resolve_matching(is_status, answer);
    }
}

fn holds(sut: &Sut) -> bool {
    !vela_core::app::tx_tracker::in_flight_ops(&sut.view()).is_empty()
}

const QUEUED: Option<(TrackLifecycle, Option<&str>)> = Some((TrackLifecycle::Queued, None));

/// The limit is ten minutes, and the rule's boundary is inclusive.
#[test]
fn the_hold_s_time_limit_is_ten_minutes() {
    use vela_core::app::tx_tracker::{in_flight_stalled, IN_FLIGHT_STALL_MS};
    assert_eq!(IN_FLIGHT_STALL_MS, 600_000.0);
    assert!(!in_flight_stalled(T0, T0 + IN_FLIGHT_STALL_MS - 1.0));
    assert!(in_flight_stalled(T0, T0 + IN_FLIGHT_STALL_MS));
}

/// The relay unreachable after its first answer: no progress, so the hold
/// is released ten minutes after the submission — not at the 24 h line. The
/// tracker keeps following the op; only the nonce is no longer held. While
/// it holds, what the send and signing machines are handed does not change
/// from one render to the next: one line, nothing to flicker.
#[test]
fn ten_minutes_without_progress_releases_the_hold() {
    use vela_core::app::tx_tracker::{in_flight_ops, in_flight_ops_json};
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    advance(&mut sut, T0 + 3_100.0, QUEUED);
    let first = in_flight_ops(&sut.view());
    assert_eq!(first.len(), 1);
    for minute in 1..10 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, None);
        assert_eq!(in_flight_ops(&sut.view()), first, "minute {minute}");
    }
    advance(&mut sut, T0 + 10.0 * MIN - 1_000.0, None);
    assert!(holds(&sut), "not yet");
    assert!(!sut.view().entries[0].stalled);

    advance(&mut sut, T0 + 10.0 * MIN, None);
    assert!(!holds(&sut), "ten minutes without progress");
    let entry = &sut.view().entries[0];
    assert!(entry.stalled);
    assert!(entry.polling, "still followed");
    assert_eq!(entry.outcome, TrackOutcome::StillConfirming);
    let json = serde_json::to_string(&sut.view()).unwrap();
    assert_eq!(
        in_flight_ops_json(&json),
        "[]",
        "the JSON bridge says the same"
    );

    // The relay answering the same thing again is not progress.
    advance(&mut sut, T0 + 11.0 * MIN, QUEUED);
    assert!(!holds(&sut));
}

/// Every move of the relay's word restarts the ten minutes: a stage, a new
/// lifecycle. Once released, a move takes the hold up again — the op is
/// moving, its nonce is spoken for.
#[test]
fn progress_keeps_the_hold() {
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    advance(&mut sut, T0 + 3_100.0, QUEUED);
    for minute in 1..=5 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, QUEUED);
    }
    // 6 min: the relay is topping up its gas — a move.
    let funding = Some((TrackLifecycle::Queued, Some(RELAY_FUNDING_STAGE)));
    advance(&mut sut, T0 + 6.0 * MIN, funding);
    for minute in 7..=14 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, funding);
        assert!(holds(&sut), "minute {minute}: 6 + 10 not reached");
    }
    // 15 min: on the network — a move. (Past ten minutes of age the relay is
    // asked once a minute: a move is heard on the minute.)
    let submitted = Some((TrackLifecycle::Submitted, None));
    for minute in 15..=24 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, submitted);
        assert!(holds(&sut), "minute {minute}: it moved at six and fifteen");
    }
    advance(&mut sut, T0 + 25.0 * MIN, submitted);
    assert!(!holds(&sut), "ten minutes since the last move");

    // It moves again: held again.
    advance(
        &mut sut,
        T0 + 26.0 * MIN,
        Some((TrackLifecycle::Included, None)),
    );
    assert!(holds(&sut));
    assert!(!sut.view().entries[0].stalled);
}

/// An op seen submitted starts where the relay takes one, queued: when the
/// relay's first word after a silence says it is on the network, that is a
/// move, and the ten minutes start again from it.
#[test]
fn a_first_answer_past_queued_is_a_move_for_an_op_seen_submitted() {
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    for minute in 1..=8 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, None);
    }
    let submitted = Some((TrackLifecycle::Submitted, None));
    for minute in 9..=18 {
        advance(&mut sut, T0 + f64::from(minute) * MIN, submitted);
        assert!(holds(&sut), "minute {minute}: it moved at nine");
    }
    advance(&mut sut, T0 + 19.0 * MIN, submitted);
    assert!(!holds(&sut), "ten minutes since the move");
}

/// Final releases at once, long before the ten minutes: confirmed, or
/// refused.
#[test]
fn final_releases_the_hold_at_once() {
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    advance(&mut sut, T0 + 3_100.0, QUEUED);
    advance(&mut sut, T0 + MIN, QUEUED);
    assert!(holds(&sut));
    tick(&mut sut, T0 + 2.0 * MIN);
    sut.resolve_matching(is_receipt, receipt_confirmed(T0 + 2.0 * MIN + 200.0));
    assert_eq!(sut.view().entries[0].outcome, TrackOutcome::Final);
    assert!(!holds(&sut), "confirmed");
    assert!(!sut.view().entries[0].stalled, "final, not stalled");

    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    assert!(holds(&sut));
    refused(&mut sut, Some("simulation_failed"), None);
    assert!(!holds(&sut), "refused");
}

/// After a restart the tracker knows of no progress since the submission:
/// an op submitted more than ten minutes ago is released at once, and the
/// relay's first answer — where the op is, not a move — leaves it so, so
/// the confirm does not shut again seconds after it opened. A real move
/// holds it again. One submitted five minutes ago holds for five more.
#[test]
fn a_restart_counts_from_the_submission_and_its_first_answer_is_no_move() {
    let reload = |sut: &mut Sut, age_ms: f64| {
        sut.dispatch(Event::AppResumed);
        sut.resolve(Res::Clock { now_ms: T0 });
        sut.resolve(Res::RecordsLoaded {
            records: vec![TrackPendingRecord {
                record_id: "rec-1".to_owned(),
                user_op_hash: HASH.to_owned(),
                chain_id: CHAIN,
                submitted_at_ms: T0 - age_ms,
                maybe_sent: false,
                submit_block: None,
                sender: Some(SENDER.to_owned()),
            }],
            now_ms: T0,
        });
    };

    let mut sut = Sut::new();
    reload(&mut sut, 30.0 * MIN);
    assert!(!holds(&sut), "half an hour with no progress known");
    let submitted = Some((TrackLifecycle::Submitted, None));
    advance(&mut sut, T0 + 3_000.0, submitted);
    advance(&mut sut, T0 + MIN, submitted);
    assert!(!holds(&sut), "the first answer is where it is, not a move");
    advance(
        &mut sut,
        T0 + 2.0 * MIN,
        Some((TrackLifecycle::Included, None)),
    );
    assert!(holds(&sut), "a move");

    let mut sut = Sut::new();
    reload(&mut sut, 5.0 * MIN);
    assert!(holds(&sut));
    advance(&mut sut, T0 + 5.0 * MIN - 1_000.0, None);
    assert!(holds(&sut));
    advance(&mut sut, T0 + 5.0 * MIN, None);
    assert!(!holds(&sut), "ten minutes from the submission");
}

/// A released hold stays released when the device's clock steps back: only
/// progress takes it up again.
#[test]
fn a_clock_that_steps_back_never_re_holds() {
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    advance(&mut sut, T0 + 3_100.0, None);
    advance(&mut sut, T0 + 10.0 * MIN, None);
    assert!(!holds(&sut));
    advance(&mut sut, T0 + 9.0 * MIN, None);
    assert!(!holds(&sut));
}

/// A shell that predates the fields still decodes.
#[test]
fn the_sender_and_reason_default_on_the_wire() {
    let event: Event = serde_json::from_str(
        r#"{"type":"submitted","user_op_hash":"0x1","record_ids":[],"chain_id":1}"#,
    )
    .unwrap();
    assert!(matches!(event, Event::Submitted { sender: None, .. }));
    let record: TrackPendingRecord = serde_json::from_str(
        r#"{"record_id":"r","user_op_hash":"0x1","chain_id":1,"submitted_at_ms":1}"#,
    )
    .unwrap();
    assert_eq!(record.sender, None);
    let status: Res = serde_json::from_str(
        r#"{"type":"status","user_op_hash":"0x1","status":"rejected","stage":"nonce","now_ms":1}"#,
    )
    .unwrap();
    assert!(matches!(
        status,
        Res::Status {
            rejection_reason: None,
            ..
        }
    ));
    // A live op's view says nothing of the stall until it has one.
    let mut sut = Sut::new();
    submitted_by(&mut sut, Some(SENDER), false);
    let wire = serde_json::to_value(&sut.view().entries[0]).unwrap();
    assert!(wire.get("stalled").is_none(), "{wire}");
    let entry: vela_core::app::tx_tracker::TrackEntryView = serde_json::from_value(wire).unwrap();
    assert!(!entry.stalled);
}
