//! Rules of the Activity feed, one test per invariant (inventory
//! `activity_feed` ① – ⑧ plus the celebration lifecycle and staleness).
//!
//! The fake clock is `now_ms` on each `StoreLoaded`; day keys are
//! shell-supplied `day_start_ms` values — the core never owns time or
//! timezone.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::activity_feed::{
    is_stable, tx_usd_value, ActivityFeed, Event, FeedBatchKind, FeedDapp, FeedDappChange,
    FeedDirection, FeedOperation as Op, FeedRow, FeedShellResult as Res, FeedTxKind, FeedTxRecord,
    FeedTxStatus,
};
use vela_core::app::clear_signing::ClearTerm;
use vela_core::app::token_trust::TrustSimJudgment;

type Sut = DomainDriver<ActivityFeed>;

const ADDR: &str = "0xA11ceFeedAA";
const OTHER: &str = "0xSomebodyElse";
const T0: f64 = 1_754_700_000_000.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A deterministic "local midnight" for a fixture timestamp — what the shell
/// would supply from the device timezone.
fn day_of(ts_seconds: f64) -> f64 {
    (ts_seconds / 86_400.0).floor() * 86_400_000.0
}

fn base(id: &str, ts: f64) -> FeedTxRecord {
    FeedTxRecord {
        id: id.to_owned(),
        user_op_hash: String::new(),
        tx_hash: format!("0xtx{id}"),
        from: String::new(),
        to: String::new(),
        to_name: None,
        value: "1".to_owned(),
        symbol: "USDC".to_owned(),
        decimals: 6,
        logo_urls: None,
        chain_id: 8453,
        timestamp: ts,
        day_start_ms: day_of(ts),
        status: FeedTxStatus::Confirmed,
        kind: None,
        usd: None,
        dapp_url: None,
        intent: None,
        balance_changes: None,
        calldata: None,
    }
}

/// An outgoing transfer from the active account. `to_name` is set so the
/// fixture generates no alias traffic unless a test wants it.
fn send(id: &str, uoh: &str, to: &str, value: &str, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::Send);
    r.user_op_hash = uoh.to_owned();
    r.from = ADDR.to_owned();
    r.to = to.to_owned();
    r.to_name = Some(format!("name-of-{to}"));
    r.value = value.to_owned();
    r
}

/// An incoming transfer to the active account.
fn recv(id: &str, from: &str, value: &str, symbol: &str, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::Receive);
    r.from = from.to_owned();
    r.to = ADDR.to_owned();
    r.value = value.to_owned();
    r.symbol = symbol.to_owned();
    r
}

/// A real address for a dApp row's recipient: the row names nothing else
/// (083 H2 review).
const CAFE: &str = "0x000000000000000000000000000000000000cafe";

/// A transaction a dApp asked this account to send, in the shape every
/// client's signing path stores (`buildSigningRecord`): the call's own wei
/// figure, the chain's coin at 18 decimals, the site's origin (083 H2).
fn dapp_tx(id: &str, to: &str, wei_hex: &str, intent: Option<&str>, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::DappTx);
    r.from = ADDR.to_owned();
    r.to = to.to_owned();
    r.value = wei_hex.to_owned();
    r.symbol = "xDAI".to_owned();
    r.decimals = 18;
    r.chain_id = 100;
    r.status = FeedTxStatus::Pending;
    r.tx_hash = String::new();
    r.user_op_hash = format!("0xop{id}");
    r.dapp_url = Some("http://127.0.0.1:5173".to_owned());
    r.intent = intent.map(str::to_owned);
    r
}

/// A `StoreLoaded` echoing an explicit read id.
fn loaded_for(read_id: u32, records: Vec<FeedTxRecord>, now_ms: f64) -> Res {
    Res::StoreLoaded {
        records,
        now_ms,
        read_id,
    }
}

/// A `StoreLoaded` for the OLDEST outstanding read — what a shell answering in
/// order produces, and what every test here means unless it says otherwise.
fn loaded(sut: &Sut, records: Vec<FeedTxRecord>, now_ms: f64) -> Res {
    loaded_for(oldest_read_id(sut), records, now_ms)
}

/// The id the oldest outstanding `ReadTxStore` is waiting to be echoed.
fn oldest_read_id(sut: &Sut) -> u32 {
    sut.outstanding()
        .iter()
        .find_map(|op| match op {
            Op::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a ReadTxStore is outstanding")
}

/// Matches any `ReadTxStore` for the default address, whatever its id.
fn read_op_for(address: &str, read_id: u32) -> Op {
    Op::ReadTxStore {
        address: address.to_owned(),
        read_id,
    }
}

fn read_op() -> Op {
    read_op_for(ADDR, 0)
}

/// Zero every read id so an assertion can name the SHAPE of the requested
/// operations without pinning ids the core mints internally.
fn shapes(ops: Vec<Op>) -> Vec<Op> {
    ops.into_iter()
        .map(|op| match op {
            Op::ReadTxStore { address, .. } => Op::ReadTxStore {
                address,
                read_id: 0,
            },
            other => other,
        })
        .collect()
}

fn scan_op() -> Op {
    Op::ScanIncomingTransfers {
        address: ADDR.to_owned(),
    }
}

/// Answer every outstanding alias request (front of the queue) with "nothing
/// resolved" so later assertions see a clean queue.
fn drain_aliases(sut: &mut Sut) {
    while let Some(Op::ResolveRecipientIdentity { addr }) = sut.outstanding().first().cloned() {
        sut.resolve(Res::AliasResolved { addr, name: None });
    }
}

/// Switch to `ADDR`, commit the given records, run a no-op first sync pass
/// (so `initialized` is spent) and drain any alias traffic.
fn boot(records: Vec<FeedTxRecord>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    assert_eq!(
        shapes(ops),
        vec![read_op(), scan_op()],
        "tick = read + scan"
    );
    sut.resolve(loaded(&sut, records, T0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    drain_aliases(&mut sut);
    sut
}

/// The item payloads of the current rows, in order.
fn items(sut: &Sut) -> Vec<vela_core::app::activity_feed::FeedItem> {
    sut.view()
        .rows
        .into_iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item),
            FeedRow::Header { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ① — one row per id; batch folding with id-dedupe first
// ---------------------------------------------------------------------------

/// A legacy same-id duplicate (a resubmitted single send) renders exactly one
/// row and is NOT mistaken for a batch (`activity.ts:436-451`).
#[test]
fn same_id_duplicate_renders_once_and_is_not_a_batch() {
    let dup = send("op1-0", "0xHASH", "0xCafe", "10", 100_000.0);
    let sut = boot(vec![dup.clone(), dup]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "op1-0");
    assert!(rows[0].batch.is_none(), "a duplicate is not a batch of two");
    assert_eq!(rows[0].value.as_deref(), Some("10"));
}

/// Sibling lines sharing one userOpHash fold to ONE row carrying the
/// breakdown; split = one token, many recipients — the summed figure is a
/// decimal string the shell formats.
#[test]
fn batch_siblings_fold_to_one_split_row() {
    let mut a = send("h-0", "0xH", "0xAaa", "10", 100_000.0);
    let mut b = send("h-1", "0xH", "0xBbb", "20", 100_000.0);
    let mut c = send("h-2", "0xH", "0xCcc", "30", 100_000.0);
    a.usd = Some("$10.00".to_owned());
    b.usd = Some("$20.00".to_owned());
    c.usd = Some("$30.00".to_owned());
    let solo = recv("r-solo", "0xBob", "5", "USDT", 90_000.0);
    let mut sut = boot(vec![a, b, c, solo]);
    drain_aliases(&mut sut);

    let rows = items(&sut);
    assert_eq!(rows.len(), 2, "three siblings became one row");
    let batch_row = &rows[0];
    assert_eq!(batch_row.id, "0xH", "row id is the shared userOpHash");
    assert_eq!(batch_row.direction, FeedDirection::Out);
    assert_eq!(
        batch_row.value.as_deref(),
        Some("60"),
        "split sums one token"
    );
    assert_eq!(batch_row.symbol, "USDC");
    assert_eq!(batch_row.decimals, Some(6));
    assert!(batch_row.counterparty.is_none(), "no single recipient");
    assert!((batch_row.usd_value - 60.0).abs() < 1e-9);
    let batch = batch_row.batch.as_ref().expect("batch payload");
    assert_eq!(batch.kind, FeedBatchKind::Split);
    assert_eq!(batch.count, 3);
    assert_eq!(batch.ids, vec!["h-0", "h-1", "h-2"]);
    assert_eq!(batch.symbol.as_deref(), Some("USDC"));
    assert_eq!(batch.transfers.len(), 3);
}

/// Many tokens to one recipient = multi_select: no summable token figure
/// (`value` is None), the single recipient is the counterparty.
#[test]
fn multi_token_batch_is_multi_select() {
    let mut a = send("m-0", "0xM", "0xSameGuy", "10", 100_000.0);
    let mut b = send("m-1", "0xM", "0xSameGuy", "3", 100_000.0);
    a.symbol = "USDC".to_owned();
    b.symbol = "DAI".to_owned();
    let sut = boot(vec![a, b]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.value.is_none(), "mixed tokens cannot sum");
    assert_eq!(row.symbol, "", "token: b.symbol ?? ''");
    assert_eq!(row.decimals, None);
    assert_eq!(row.counterparty.as_deref(), Some("0xSameGuy"));
    let batch = row.batch.as_ref().expect("batch payload");
    assert_eq!(batch.kind, FeedBatchKind::MultiSelect);
    assert_eq!(batch.to.as_deref(), Some("0xSameGuy"));
    assert!(batch.symbol.is_none());
}

/// A resubmitted line INSIDE a real batch counts once toward the group size
/// and the sum (`ids.has(t.id)` before the group push).
#[test]
fn resubmitted_line_in_a_real_batch_counts_once() {
    let a = send("h-0", "0xH", "0xAaa", "10", 100_000.0);
    let b = send("h-1", "0xH", "0xBbb", "20", 100_000.0);
    let sut = boot(vec![a.clone(), b, a]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let batch = rows[0].batch.as_ref().expect("batch payload");
    assert_eq!(batch.count, 2, "the duplicate line counted once");
    assert_eq!(rows[0].value.as_deref(), Some("30"));
}

/// Feed scoping: only this account's transfers, connection events excluded —
/// and the raw `transactions` view is scoped the same way.
#[test]
fn feed_and_transactions_are_account_scoped() {
    let ours_send = send("s1", "0xU1", "0xCafe", "1", 300_000.0);
    let mut foreign_send = send("s2", "0xU2", "0xCafe", "1", 290_000.0);
    foreign_send.from = OTHER.to_owned();
    let ours_recv = recv("r1", "0xBob", "2", "USDT", 280_000.0);
    let mut foreign_recv = recv("r2", "0xBob", "2", "USDT", 270_000.0);
    foreign_recv.to = OTHER.to_owned();
    let mut dapp = base("d1", 260_000.0);
    dapp.kind = Some(FeedTxKind::DappTx);
    dapp.from = ADDR.to_owned();
    // Legacy untyped record ⇒ send (`t.type ?? 'send'`), matched case-insensitively.
    let mut legacy = base("l1", 250_000.0);
    legacy.from = ADDR.to_uppercase();
    legacy.to = "0xCafe".to_owned();
    legacy.to_name = Some("Ann".to_owned());

    let mut sut = boot(vec![
        ours_send,
        foreign_send,
        ours_recv,
        foreign_recv,
        dapp,
        legacy,
    ]);
    drain_aliases(&mut sut);

    let ids: Vec<String> = items(&sut).into_iter().map(|i| i.id).collect();
    assert_eq!(
        ids,
        vec!["s1", "r1", "d1", "l1"],
        "a dApp's transaction is a row (083 H2)"
    );
    let tx_ids: Vec<String> = sut.view().transactions.into_iter().map(|t| t.id).collect();
    assert_eq!(tx_ids, vec!["s1", "r1", "d1", "l1"]);
}

// ---------------------------------------------------------------------------
// ② — cache paints first; a no-op sync never re-reads (no flicker)
// ---------------------------------------------------------------------------

#[test]
fn cached_feed_survives_a_noop_sync() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);
    let before = sut.view();
    assert_eq!(items(&sut).len(), 1);

    let ops = sut.dispatch(Event::FocusTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
    assert_eq!(sut.view(), before, "nothing blanks while requests are out");

    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert!(ops.is_empty(), "newCount = 0 must not re-read (no flicker)");
    assert_eq!(sut.view(), before);
}

#[test]
fn sync_with_new_receipts_rereads_the_store() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()], "something landed — read it");
}

#[test]
fn live_tick_runs_the_same_pipeline() {
    let mut sut = boot(vec![]);
    let ops = sut.dispatch(Event::LiveTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
}

// ---------------------------------------------------------------------------
// ③ — the first sync pass never celebrates the backlog
// ---------------------------------------------------------------------------

#[test]
fn first_sync_pass_never_celebrates_but_the_second_does() {
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    sut.resolve(loaded(&sut, vec![], T0));
    // First pass discovers a 2-receipt BACKLOG.
    let ops = sut.resolve(Res::SyncCompleted { new_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 1_000.0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbob".to_owned()
        }],
        "no haptic, no toast timer on the first pass"
    );
    assert!(sut.view().toast.is_none(), "history never celebrates");
    assert!(sut.view().new_item_id.is_none());
    sut.resolve(Res::AliasResolved {
        addr: "0xbob".to_owned(),
        name: None,
    });

    // Second pass: a genuinely-new in-session receipt.
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 2_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 1 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let newer_send = send("s9", "0xU9", "0xCafe", "1", 300_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let ops = sut.resolve(loaded(&sut, vec![newer_send, r2, r1], T0 + 3_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ],
        "buzz + arm the toast countdown"
    );
    let view = sut.view();
    let toast = view.toast.expect("toast shows");
    assert_eq!(
        toast.item_id, "r2",
        "the newest INCOMING item, not the newest row"
    );
    assert_eq!(toast.value, "7");
    assert_eq!(toast.symbol, "USDT");
    assert_eq!(toast.deadline_ms, T0 + 3_000.0 + 2_800.0);
    assert_eq!(view.new_item_id.as_deref(), Some("r2"), "row glow");
}

/// tx_tracker convergence re-reads the store but NEVER celebrates — a flipped
/// pending→confirmed is not new money in.
#[test]
fn reconcile_rereads_without_celebrating() {
    let mut sut = boot(vec![]);
    assert!(sut
        .dispatch(Event::ReconcileCompleted { resolved_count: 0 })
        .is_empty());
    let ops = sut.dispatch(Event::ReconcileCompleted { resolved_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r1], T0 + 1_000.0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbob".to_owned()
        }],
        "no haptic / timer"
    );
    assert!(sut.view().toast.is_none());
    assert!(sut.view().new_item_id.is_none());
}

// ---------------------------------------------------------------------------
// ④ — privacy suppresses the toast (and only the toast)
// ---------------------------------------------------------------------------

#[test]
fn privacy_suppresses_the_toast_but_not_glow_or_haptic() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let mut sut = boot(vec![r1.clone()]);
    sut.dispatch(Event::PrivacyChanged { hidden: true });

    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 1 });
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r2, r1], T0 + 2_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ],
        "the haptic still fires while hidden (ported: only the toast render is gated)"
    );
    let view = sut.view();
    assert!(view.toast.is_none(), "a toast would leak the masked number");
    assert_eq!(
        view.new_item_id.as_deref(),
        Some("r2"),
        "glow is amount-free"
    );

    // Unhide within the toast window: the withheld state becomes visible.
    sut.dispatch(Event::PrivacyChanged { hidden: false });
    assert_eq!(sut.view().toast.expect("toast").item_id, "r2");
}

// ---------------------------------------------------------------------------
// Toast lifecycle
// ---------------------------------------------------------------------------

fn celebrated(records_before: Vec<FeedTxRecord>, new_record: FeedTxRecord) -> Sut {
    let mut sut = boot(records_before.clone());
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, records_before.clone(), T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 1 });
    let mut all = vec![new_record];
    all.extend(records_before);
    let ops = sut.resolve(loaded(&sut, all, T0 + 2_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ]
    );
    sut.resolve(Res::HapticPlayed);
    sut
}

#[test]
fn toast_expiry_clears_the_toast_but_keeps_the_glow() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);
    assert!(sut.view().toast.is_some());

    sut.resolve(Res::ToastExpired { generation: 1 });
    let view = sut.view();
    assert!(view.toast.is_none(), "2.8s over — the toast goes");
    assert_eq!(
        view.new_item_id.as_deref(),
        Some("r2"),
        "the row glow outlives the toast (newItemId is not timer-cleared)"
    );
}

/// A timer echo bearing a stale generation (a superseded celebration's
/// countdown firing late) must never clear the current toast — the core twin
/// of `clearTimeout(toastTimer.current)`.
#[test]
fn stale_toast_timer_is_ignored() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);

    let ops = sut.resolve(Res::ToastExpired { generation: 42 });
    assert!(ops.is_empty(), "a stale echo is a no-op");
    assert!(sut.view().toast.is_some(), "the live toast survives");
}

// ---------------------------------------------------------------------------
// ⑤ — tombstones: a deleted row cannot be repainted by a concurrent reload
// ---------------------------------------------------------------------------

#[test]
fn deleted_row_is_not_resurrected_by_a_concurrent_reload() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    // A background reload is already in flight (it read storage BEFORE the
    // delete write lands)…
    sut.dispatch(Event::FocusTick);
    // …when the user deletes the row.
    let ops = sut.dispatch(Event::DeleteRequested {
        id: "s1".to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::DeleteTxRecord {
            id: "s1".to_owned()
        }]
    );
    assert!(items(&sut).is_empty(), "optimistic removal is instant");

    // The stale read commits — the tombstone filters the ghost.
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    assert!(items(&sut).is_empty(), "the ghost never repaints");
    sut.resolve(Res::SyncCompleted { new_count: 0 });

    // The write settles; post-delete reloads stay clean.
    assert!(sut
        .resolve(Res::DeleteCommitted {
            id: "s1".to_owned()
        })
        .is_empty());
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![], T0 + 2_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert!(items(&sut).is_empty());
}

/// Ported verbatim: the TS `.finally()` drops the tombstone on FAILURE too,
/// so the next reload resurrects the row — honest, since the record really is
/// still in storage.
#[test]
fn failed_delete_lets_the_next_reload_resurrect_the_row() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    sut.dispatch(Event::DeleteRequested {
        id: "s1".to_owned(),
    });
    assert!(items(&sut).is_empty());
    sut.resolve(Res::DeleteFailed {
        id: "s1".to_owned(),
    });

    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert_eq!(items(&sut).len(), 1, "the record still exists — show it");
}

// ---------------------------------------------------------------------------
// ⑥ — date headers interleave with items in feed order
// ---------------------------------------------------------------------------

#[test]
fn day_headers_interleave_in_feed_order() {
    // 200_000s and 190_000s share a "day"; 100_000s is an earlier one.
    let r_a = recv("a", "0xBob", "1", "USDT", 200_000.0);
    let r_b = recv("b", "0xBob", "2", "USDT", 190_000.0);
    let s_c = send("c", "0xU1", "0xCafe", "3", 100_000.0);
    let mut sut = boot(vec![s_c, r_a, r_b]); // store order ≠ feed order
    drain_aliases(&mut sut);

    let day_new = day_of(200_000.0);
    let day_old = day_of(100_000.0);
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 5);
    match &rows[0] {
        FeedRow::Header {
            id,
            day_start_ms,
            timestamp,
        } => {
            assert_eq!(id, &format!("day-{day_new}"));
            assert_eq!(*day_start_ms, day_new);
            assert_eq!(*timestamp, 200_000.0, "labelled from the day's first item");
        }
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[1], FeedRow::Item { item } if item.id == "a"));
    assert!(matches!(&rows[2], FeedRow::Item { item } if item.id == "b"));
    match &rows[3] {
        FeedRow::Header { day_start_ms, .. } => assert_eq!(*day_start_ms, day_old),
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[4], FeedRow::Item { item } if item.id == "c"));
}

#[test]
fn chain_filter_regroups_headers_over_the_filtered_list() {
    let mut r_a = recv("a", "0xBob", "1", "USDT", 200_000.0);
    r_a.chain_id = 1;
    let r_b = recv("b", "0xBob", "2", "USDT", 190_000.0); // 8453
    let mut s_c = send("c", "0xU1", "0xCafe", "3", 100_000.0);
    s_c.chain_id = 1;
    let mut sut = boot(vec![r_a, r_b, s_c]);
    drain_aliases(&mut sut);

    sut.dispatch(Event::ChainFilterChanged {
        chain_id: Some(8453),
    });
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 2, "one day, one item on 8453");
    match &rows[0] {
        FeedRow::Header { timestamp, .. } => assert_eq!(
            *timestamp, 190_000.0,
            "the header derives from the FILTERED day's first item"
        ),
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[1], FeedRow::Item { item } if item.id == "b"));

    // A day whose items are all filtered out emits no header at all.
    sut.dispatch(Event::ChainFilterChanged { chain_id: Some(1) });
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 4); // header + a, header + c

    sut.dispatch(Event::ChainFilterChanged { chain_id: None });
    assert_eq!(sut.view().rows.len(), 5, "unfiltered again");
}

// ---------------------------------------------------------------------------
// ⑦ — alias memoisation: local name first, one network attempt ever
// ---------------------------------------------------------------------------

#[test]
fn stored_name_blocks_network_and_attempts_never_repeat() {
    let named = send("s1", "0xU1", "0xCafe", "1", 300_000.0); // to_name set
    let unnamed = recv("r1", "0xBeEf", "2", "USDT", 200_000.0);
    let unnamed_again = recv("r2", "0xbeef", "3", "USDT", 100_000.0); // same addr, other case

    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![named, unnamed, unnamed_again], T0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbeef".to_owned()
        }],
        "one lowercased request; the stored-name row never asks"
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });

    sut.resolve(Res::AliasResolved {
        addr: "0xbeef".to_owned(),
        name: Some("Bob".to_owned()),
    });
    let rows = items(&sut);
    assert_eq!(rows[0].alias.as_deref(), Some("name-of-0xCafe"));
    assert_eq!(rows[1].alias.as_deref(), Some("Bob"));
    assert_eq!(rows[2].alias.as_deref(), Some("Bob"), "case-folded lookup");

    // A reload re-derives pending addresses — attempted ones never re-ask.
    sut.dispatch(Event::FocusTick);
    let r3 = recv("r3", "0xBEEF", "4", "USDT", 50_000.0);
    let ops = sut.resolve(loaded(
        &sut,
        vec![
            send("s1", "0xU1", "0xCafe", "1", 300_000.0),
            recv("r1", "0xBeEf", "2", "USDT", 200_000.0),
            r3,
        ],
        T0 + 1_000.0,
    ));
    assert!(
        ops.is_empty(),
        "0xbeef was attempted — never again this session"
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });
}

/// The resolved overlay wins over a stored name for display
/// (`aliasMap.get(...) ?? item.alias`, HomeScreen.tsx:244-250).
#[test]
fn resolved_name_wins_over_stored_for_display() {
    let with_stored = send("s1", "0xU1", "0xD00d", "1", 200_000.0);
    let mut bare = recv("r1", "0xd00d", "2", "USDT", 100_000.0);
    bare.symbol = "USDT".to_owned();

    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![with_stored, bare], T0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xd00d".to_owned()
        }]
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    sut.resolve(Res::AliasResolved {
        addr: "0xd00d".to_owned(),
        name: Some("vitalik.eth".to_owned()),
    });
    let rows = items(&sut);
    assert_eq!(
        rows[0].alias.as_deref(),
        Some("vitalik.eth"),
        "the resolved name overrides the stored one"
    );
    assert_eq!(rows[1].alias.as_deref(), Some("vitalik.eth"));
}

/// An unresolved address is memoised as attempted too — `None` answers are
/// never retried (and an empty name is falsy, as in TS).
#[test]
fn unresolved_alias_never_retries() {
    let r1 = recv("r1", "0xGhost", "1", "USDT", 100_000.0);
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![r1.clone()], T0));
    assert_eq!(ops.len(), 1);
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    let ops = sut.resolve(Res::AliasResolved {
        addr: "0xghost".to_owned(),
        name: Some(String::new()),
    });
    assert!(
        ops.is_empty(),
        "empty name is falsy — not memoised as a name"
    );
    assert!(items(&sut)[0].alias.is_none());

    sut.dispatch(Event::FocusTick);
    let ops = sut.resolve(loaded(&sut, vec![r1], T0 + 1_000.0));
    assert!(ops.is_empty(), "no second network attempt");
    sut.resolve(Res::SyncCompleted { new_count: 0 });
}

// ---------------------------------------------------------------------------
// ⑧ — stablecoin face-value: never $0.00
// ---------------------------------------------------------------------------

#[test]
fn stablecoin_face_value_is_never_zero() {
    // No stored USD at all → face value.
    let r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    assert_eq!(tx_usd_value(&r), 45.5);

    // Stored "$0.00" (the legacy unknown marker) → face value, not zero.
    let mut r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    r.usd = Some("$0.00".to_owned());
    assert_eq!(tx_usd_value(&r), 45.5);

    // The on-chain Tether glyph folds: "USD₮0" is a stablecoin.
    let r = recv("r", "0xBob", "12", "USD₮0", 100_000.0);
    assert_eq!(tx_usd_value(&r), 12.0);

    // A real stored value wins over the face value ("$1,234.56" → 1234.56).
    let mut r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    r.usd = Some("$1,234.56".to_owned());
    assert_eq!(tx_usd_value(&r), 1234.56);

    // Unpriced non-stablecoins honestly stay 0.
    let r = recv("r", "0xBob", "4840000", "SNDRA", 100_000.0);
    assert_eq!(tx_usd_value(&r), 0.0);

    // Empty value string → parseFloat('0') → no face value to show.
    let mut r = recv("r", "0xBob", "", "USDT", 100_000.0);
    r.usd = Some(String::new()); // falsy, like an absent field
    assert_eq!(tx_usd_value(&r), 0.0);

    assert!(is_stable("usdc.e"), "case-folded match");
    assert!(!is_stable("WETH"));
}

/// The valuation flows into the folded items (both directions).
#[test]
fn items_carry_the_stable_face_value() {
    let r1 = recv("r1", "0xBob", "45.5", "USDT", 200_000.0);
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0); // USDC, no usd stored
    let mut sut = boot(vec![r1, s1]);
    drain_aliases(&mut sut);
    let rows = items(&sut);
    assert_eq!(rows[0].usd_value, 45.5);
    assert_eq!(rows[1].usd_value, 10.0);
}

// ---------------------------------------------------------------------------
// Staleness — the addressRef guard
// ---------------------------------------------------------------------------

#[test]
fn switching_accounts_drops_in_flight_answers() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    // A load for the old account is in flight when the user switches.
    sut.dispatch(Event::FocusTick);
    let ops = sut.dispatch(Event::AccountSwitched {
        address: "0xNewAccount".to_owned(),
    });
    assert_eq!(
        shapes(ops),
        vec![
            Op::ReadTxStore {
                address: "0xNewAccount".to_owned(),
                read_id: 0
            },
            Op::ScanIncomingTransfers {
                address: "0xNewAccount".to_owned()
            }
        ]
    );
    let before = sut.view();
    assert_eq!(
        before.rows.len(),
        2,
        "the previous feed keeps painting until the new read commits"
    );

    // The OLD account's answers land late — and change nothing.
    let stale_extra = send("s2", "0xU2", "0xCafe", "99", 300_000.0);
    let ops = sut.resolve(loaded(&sut, vec![s1, stale_extra], T0 + 1_000.0));
    assert!(ops.is_empty());
    assert_eq!(sut.view(), before, "a stale commit never paints");
    let ops = sut.resolve(Res::SyncCompleted { new_count: 5 });
    assert!(ops.is_empty(), "a stale sync never re-reads");

    // The NEW account's read commits its own feed.
    let mut theirs = base("n1", 400_000.0);
    theirs.kind = Some(FeedTxKind::Send);
    theirs.from = "0xNewAccount".to_owned();
    theirs.to = "0xCafe".to_owned();
    theirs.to_name = Some("Ann".to_owned());
    theirs.user_op_hash = "0xN".to_owned();
    sut.resolve(loaded(&sut, vec![theirs], T0 + 2_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    let ids: Vec<String> = items(&sut).into_iter().map(|i| i.id).collect();
    assert_eq!(ids, vec!["n1"]);
}

#[test]
fn account_switch_clears_the_celebration() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);
    assert!(sut.view().toast.is_some());

    sut.dispatch(Event::AccountSwitched {
        address: "0xNewAccount".to_owned(),
    });
    let view = sut.view();
    assert!(view.toast.is_none(), "setReceipt(null) on switch");
    assert!(view.new_item_id.is_none(), "setNewItemId(null) on switch");
}

// ---------------------------------------------------------------------------
// Inertness without an account
// ---------------------------------------------------------------------------

#[test]
fn empty_address_is_inert() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(Event::FocusTick).is_empty());
    assert!(sut.dispatch(Event::LiveTick).is_empty());
    assert!(sut
        .dispatch(Event::ReconcileCompleted { resolved_count: 3 })
        .is_empty());
    assert!(sut
        .dispatch(Event::AccountSwitched {
            address: String::new()
        })
        .is_empty());
    assert!(sut.view().rows.is_empty());
}

/// A celebration belongs to the read the sync asked for — not to whichever
/// read answers next.
///
/// A tick issues the store read and the incoming-transfer scan together. If the
/// scan answers first, the sync flags a celebration and asks for a FRESH read;
/// the earlier read is still in flight and its snapshot predates the receipt.
/// Before `read_id` correlation that stale answer consumed the flag, and the
/// real receipt landed with no toast, glow or haptic — a regression against the
/// TypeScript original, which bound the celebration to one `loadData` by
/// closure.
#[test]
fn a_stale_read_never_eats_the_celebration_the_sync_earned() {
    let old = send("tx-old", "uoh-old", "0xdead", "5", T0 / 1000.0);
    let mut sut = boot(vec![old.clone()]);

    // A tick: read + scan, both outstanding. Remember the tick's read id.
    let ops = sut.dispatch(Event::FocusTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
    let stale_read = oldest_read_id(&sut);

    // The scan answers FIRST: one receipt landed. That flags a celebration and
    // asks for a read of its own.
    sut.resolve_matching(
        |op| matches!(op, Op::ScanIncomingTransfers { .. }),
        Res::SyncCompleted { new_count: 1 },
    );
    let fresh_read = sut
        .outstanding()
        .iter()
        .filter_map(|op| match op {
            Op::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .find(|id| *id != stale_read)
        .expect("the sync asked for a read of its own");

    // The stale read finally answers — with the pre-receipt snapshot.
    sut.resolve_matching(
        |op| matches!(op, Op::ReadTxStore { read_id, .. } if *read_id == stale_read),
        loaded_for(stale_read, vec![old.clone()], T0),
    );
    drain_aliases(&mut sut);
    assert!(
        sut.view().toast.is_none(),
        "a snapshot that predates the receipt must not celebrate"
    );

    // The read the sync asked for arrives, carrying the receipt. THIS one
    // celebrates.
    let landed = recv("tx-in", "0xbeef", "9", "USDC", T0 / 1000.0 + 1.0);
    sut.resolve_matching(
        |op| matches!(op, Op::ReadTxStore { read_id, .. } if *read_id == fresh_read),
        loaded_for(fresh_read, vec![landed, old], T0),
    );
    drain_aliases(&mut sut);
    assert!(
        sut.view().toast.is_some(),
        "the receipt's own read must celebrate"
    );
}

// ---------------------------------------------------------------------------
// 083 H2 (079 D3) — a dApp's transaction is in Activity
// ---------------------------------------------------------------------------

/// A plain native send a site asked for reads as a send of its amount, from
/// that site: the page's wei figure scaled to the coin, the origin's host.
#[test]
fn a_dapp_native_send_is_a_row_with_its_amount_and_site() {
    let sut = boot(vec![dapp_tx(
        "dapp-1-tx",
        CAFE,
        "0x2386f26fc10000",
        Some("Send"),
        100_000.0,
    )]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.direction, FeedDirection::Out);
    assert_eq!(row.value.as_deref(), Some("0.01"), "0x2386f26fc10000 wei");
    assert_eq!(row.symbol, "xDAI");
    assert_eq!(row.decimals, Some(18));
    assert_eq!(row.counterparty.as_deref(), Some(CAFE));
    assert_eq!(row.tx_hash, None, "pending: no hash yet");
    assert_eq!(
        row.dapp,
        Some(FeedDapp {
            site: Some("127.0.0.1".to_owned()),
            intent: Some("Send".to_owned()),
            intent_term: Some(ClearTerm::IntentSend),
            changes: Vec::new(),
            received: None,
            estimated: false,
            contract_call: false,
        })
    );
    // The detail reads the status off the record, which is in the view.
    assert_eq!(sut.view().transactions[0].status, FeedTxStatus::Pending);
}

/// A call that moves no coin shows no "0 xDAI"; a decoded one carries its
/// intent, an undecoded one none (the shell reads "Contract interaction"); a
/// batch (`wallet_sendCalls`) has no single counterparty.
#[test]
fn a_dapp_contract_call_carries_its_intent_and_no_zero_amount() {
    let mut swap = dapp_tx("dapp-2-tx", "0xRouter", "0x0", Some("Swap"), 200_000.0);
    swap.dapp_url = Some("https://app.uniswap.org".to_owned());
    let blind = dapp_tx("dapp-3-tx", "", "0x0", None, 190_000.0);
    let sut = boot(vec![swap, blind]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 2);

    assert_eq!(rows[0].value, None);
    assert_eq!(rows[0].symbol, "");
    assert_eq!(rows[0].decimals, None);
    assert!(rows[0].usd_value.abs() < f64::EPSILON);
    let swap = rows[0].dapp.clone().expect("a dApp row");
    assert_eq!(swap.site.as_deref(), Some("app.uniswap.org"));
    assert_eq!(swap.intent_term, Some(ClearTerm::IntentSwap));

    assert_eq!(
        rows[1].counterparty, None,
        "a batch has no single recipient"
    );
    let blind = rows[1].dapp.clone().expect("a dApp row");
    assert_eq!(blind.site.as_deref(), Some("127.0.0.1"));
    assert_eq!((blind.intent, blind.intent_term), (None, None));
}

/// The site is read from the ORIGIN the request came from, and only from an
/// origin: a bare word — the shape a dApp's self-declared name takes — names
/// no site, even when it reads like a host (083 H2 review). The host reader
/// alone would take `app.uniswap.org` as one.
#[test]
fn a_dapp_row_names_no_site_from_a_bare_name() {
    let mut named = dapp_tx("d-named", "0xRouter", "0x0", None, 100_000.0);
    named.dapp_url = Some("app.uniswap.org".to_owned());
    let mut junk = dapp_tx("d-junk", "0xRouter", "0x0", None, 90_000.0);
    junk.dapp_url = Some("https://trusted.org:evil@/".to_owned());
    let mut userinfo = dapp_tx("d-userinfo", "0xRouter", "0x0", None, 80_000.0);
    userinfo.dapp_url = Some("https://app.uniswap.org@evil.example".to_owned());
    // An opaque page's origin serializes as the word "null".
    let mut opaque = dapp_tx("d-opaque", "0xRouter", "0x0", None, 70_000.0);
    opaque.dapp_url = Some("null".to_owned());
    let sut = boot(vec![named, junk, userinfo, opaque]);
    let sites: Vec<Option<String>> = items(&sut)
        .into_iter()
        .map(|item| item.dapp.and_then(|dapp| dapp.site))
        .collect();
    assert_eq!(
        sites,
        vec![None, None, Some("evil.example".to_owned()), None],
        "no name read as a host; an unparseable origin names nothing; \
         userinfo is not the host; an opaque origin is no site"
    );
}

/// A dApp row's recipient is an ADDRESS or nothing (083 H2 review). A batch
/// submits no top-level `to`, so a page can put any text there without the
/// sheet showing it; drawn, it was shortened by byte and took the desktop
/// down on every launch. Text that is not an address is no counterparty,
/// keeps no stored name, and is never sent to the name lookup.
#[test]
fn a_dapp_row_names_no_recipient_that_is_not_an_address() {
    let mut forged = dapp_tx("d-forged", "日本語日本語", "0x0", None, 100_000.0);
    forged.to_name = Some("Vitalik".to_owned());
    let short = dapp_tx("d-short", "0xCafe", "0x0", None, 90_000.0);
    let upper = dapp_tx(
        "d-real",
        "0x000000000000000000000000000000000000CAFE",
        "0x0",
        None,
        80_000.0,
    );
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![forged, short, upper], T0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0x000000000000000000000000000000000000cafe".to_owned()
        }],
        "only the real address is looked up"
    );
    let rows = items(&sut);
    assert_eq!(
        rows.iter()
            .map(|item| (item.counterparty.as_deref(), item.alias.as_deref()))
            .collect::<Vec<_>>(),
        vec![
            (None, None),
            (None, None),
            (Some("0x000000000000000000000000000000000000CAFE"), None),
        ]
    );
}

/// Signatures and connections move nothing, so they are no row and no
/// record in the view — the rule the dApp transaction row must not loosen.
#[test]
fn signatures_and_connections_stay_out_of_activity() {
    let mut records = Vec::new();
    for (id, kind) in [
        ("m1", FeedTxKind::SignMessage),
        ("t1", FeedTxKind::SignTypedData),
        ("c1", FeedTxKind::Connect),
    ] {
        let mut r = base(id, 100_000.0);
        r.kind = Some(kind);
        r.from = ADDR.to_owned();
        r.to = ADDR.to_owned();
        r.dapp_url = Some("https://app.uniswap.org".to_owned());
        records.push(r);
    }
    records.push(dapp_tx("d1", "0xCafe", "0x1", None, 90_000.0));
    let sut = boot(records);
    let ids: Vec<String> = items(&sut).into_iter().map(|i| i.id).collect();
    assert_eq!(ids, vec!["d1"]);
    let tx_ids: Vec<String> = sut.view().transactions.into_iter().map(|t| t.id).collect();
    assert_eq!(tx_ids, vec!["d1"]);
}

/// A stored `decimals` sizes the scaling's padding, so a corrupted or
/// imported row cannot be allowed to say four billion: past the bound the row
/// shows no figure, and the feed still loads (083 H2 review).
#[test]
fn a_dapp_row_with_absurd_decimals_shows_no_figure() {
    let mut absurd = dapp_tx("d-absurd", "0xCafe", "0x2386f26fc10000", None, 100_000.0);
    absurd.decimals = u32::MAX;
    let mut edge = dapp_tx("d-edge", "0xCafe", "0x1", None, 90_000.0);
    edge.decimals = vela_core::app::activity_feed::MAX_DAPP_DECIMALS;
    let sut = boot(vec![absurd, edge]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (None, "")
    );
    assert_eq!(rows[0].decimals, None);
    assert_eq!(
        rows[1].value.as_deref(),
        Some("0.000000000000000000000000000000000001")
    );
}

/// Another account's dApp transaction is not this account's Activity, and a
/// record from a shell that stores no origin names no site.
#[test]
fn a_dapp_row_is_account_scoped_and_survives_a_missing_origin() {
    let mut ours = dapp_tx("d-ours", "0xCafe", "0x1", None, 100_000.0);
    ours.dapp_url = None;
    let mut theirs = dapp_tx("d-theirs", "0xCafe", "0x1", None, 90_000.0);
    theirs.from = OTHER.to_owned();
    let sut = boot(vec![ours, theirs]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "d-ours");
    assert_eq!(rows[0].value.as_deref(), Some("0.000000000000000001"));
    assert_eq!(rows[0].dapp.as_ref().and_then(|d| d.site.clone()), None);
    let tx_ids: Vec<String> = sut.view().transactions.into_iter().map(|t| t.id).collect();
    assert_eq!(tx_ids, vec!["d-ours"]);
}

/// Pending -> landed: the tracker patches the dApp record in place and says
/// so; the re-read carries the confirmed record and its hash, like a send's,
/// and never celebrates, because money leaving is not money arriving.
#[test]
fn a_dapp_row_settles_when_the_tracker_lands_it() {
    let pending = dapp_tx("dapp-4-tx", "0xCafe", "0x1", Some("Send"), 100_000.0);
    let mut sut = boot(vec![pending.clone()]);
    assert_eq!(sut.view().transactions[0].status, FeedTxStatus::Pending);

    let ops = sut.dispatch(Event::ReconcileCompleted { resolved_count: 1 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let mut landed = pending;
    landed.status = FeedTxStatus::Confirmed;
    landed.tx_hash = "0xlanded".to_owned();
    sut.resolve(loaded(&sut, vec![landed], T0 + 1_000.0));
    drain_aliases(&mut sut);

    assert_eq!(sut.view().transactions[0].status, FeedTxStatus::Confirmed);
    assert_eq!(items(&sut)[0].tx_hash.as_deref(), Some("0xlanded"));
    assert!(sut.view().toast.is_none());
    assert!(sut.view().new_item_id.is_none());
}

/// A shell that has not mapped the new fields yet still loads: both are
/// optional on the wire, and a record that carries neither serializes as
/// before.
#[test]
fn the_dapp_fields_are_optional_on_the_wire() {
    let json = r#"{"id":"x","user_op_hash":"","tx_hash":"","from":"","to":"","to_name":null,
        "value":"1","symbol":"USDC","decimals":6,"logo_urls":null,"chain_id":1,"timestamp":1,
        "day_start_ms":0,"status":"confirmed","kind":"send","usd":null}"#;
    let record: FeedTxRecord = serde_json::from_str(json).expect("an older shell's record");
    assert_eq!(
        (record.dapp_url.clone(), record.intent.clone()),
        (None, None)
    );
    let out = serde_json::to_value(&record).expect("serializes");
    assert!(out.get("dapp_url").is_none() && out.get("intent").is_none());
}

// ---------------------------------------------------------------------------
// 083 F1 — a dApp row says what the transaction moved, from what was approved
// ---------------------------------------------------------------------------

/// Base's USDC — one of the chain's stables, so `token_trust` judged it in
/// the trusted set: its lines carry figures, and it can be the row's.
const USDC_BASE: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
/// A token a site controls: whatever it emits, the sheet showed no number.
const SITE_TOKEN: &str = "0x00000000000000000000000000000000000bad01";
/// The Universal Router a Uniswap swap calls — a contract, not a recipient.
const ROUTER: &str = "0xd614000000000000000000000000000000009c40";

fn usdc(delta: &str) -> TrustSimJudgment {
    TrustSimJudgment::Erc20Trusted {
        token: USDC_BASE.to_owned(),
        delta: delta.to_owned(),
        symbol: "USDC".to_owned(),
        decimals: 6,
        in_trusted_set: true,
    }
}

/// What the sheet makes of a contract a SITE deployed that emits
/// `Transfer(you, …)` and answers `symbol()` with "USDC": an outflow renders
/// on metadata alone, but the token is in no set the wallet trusts.
fn site_usdc(delta: &str) -> TrustSimJudgment {
    TrustSimJudgment::Erc20Trusted {
        token: SITE_TOKEN.to_owned(),
        delta: delta.to_owned(),
        symbol: "USDC".to_owned(),
        decimals: 6,
        in_trusted_set: false,
    }
}

fn eth(delta: &str) -> TrustSimJudgment {
    TrustSimJudgment::Native {
        delta: delta.to_owned(),
    }
}

/// A swap on Base, as the desktop stores it: the router, the call's own wei
/// value, no decoded intent, the site, and what the sheet's simulation showed.
fn swap(id: &str, wei_hex: &str, changes: Option<Vec<TrustSimJudgment>>, ts: f64) -> FeedTxRecord {
    let mut r = dapp_tx(id, ROUTER, wei_hex, None, ts);
    r.chain_id = 8453;
    r.symbol = "ETH".to_owned();
    r.dapp_url = Some("https://app.uniswap.org".to_owned());
    r.calldata = Some(true);
    r.balance_changes = changes;
    r
}

fn line(direction: FeedDirection, symbol: &str, value: &str, decimals: u32) -> FeedDappChange {
    FeedDappChange {
        direction,
        verified: true,
        symbol: symbol.to_owned(),
        value: Some(value.to_owned()),
        decimals: Some(decimals),
        exact: false,
    }
}

/// A line the wallet can vouch for: the coin the transaction itself sent.
fn exact(line: FeedDappChange) -> FeedDappChange {
    FeedDappChange {
        exact: true,
        ..line
    }
}

/// The device pass's three swaps (2026-09-29): 0.1 USDC -> ETH, all of the
/// USDC -> ETH, 0.0001 ETH -> USDC. Each row's figure is what left — the
/// sheet's own line, not the page's — and the one coin that came back rides
/// beside it as expected. The title stays the recorded intent (none here, so
/// the shell's "Contract interaction"): one coin out and one in is also a
/// deposit, a wrap or a vault share, and nothing the wallet holds says swap.
#[test]
fn a_swap_row_says_what_left_and_what_was_expected_back() {
    let sut = boot(vec![
        swap(
            "dapp-1-tx",
            "0x0",
            Some(vec![usdc("-100000"), eth("37000000000000")]),
            100_000.0,
        ),
        swap(
            "dapp-2-tx",
            "0x0",
            Some(vec![usdc("-271741"), eth("100548000000000")]),
            200_000.0,
        ),
        swap(
            "dapp-3-tx",
            "0x5af3107a4000",
            Some(vec![eth("-100000000000000"), usdc("269487")]),
            300_000.0,
        ),
    ]);
    let rows = items(&sut);
    assert_eq!(
        rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        vec!["dapp-3-tx", "dapp-2-tx", "dapp-1-tx"],
        "newest first, as every row"
    );
    let figures: Vec<(FeedDirection, Option<&str>, &str, Option<u32>)> = rows
        .iter()
        .map(|r| {
            (
                r.direction,
                r.value.as_deref(),
                r.symbol.as_str(),
                r.decimals,
            )
        })
        .collect();
    assert_eq!(
        figures,
        vec![
            (FeedDirection::Out, Some("0.0001"), "ETH", Some(18)),
            (FeedDirection::Out, Some("0.271741"), "USDC", Some(6)),
            (FeedDirection::Out, Some("0.1"), "USDC", Some(6)),
        ]
    );
    // A stablecoin out is worth its face; ETH has no stored price.
    assert!((rows[2].usd_value - 0.1).abs() < 1e-9);
    assert!(rows[0].usd_value.abs() < f64::EPSILON);

    // What the wallet itself sent is a fact; an outflow the sheet measured is
    // the simulation's — an exact-output swap may spend another amount, and
    // nothing here says which kind was signed (083 F1 review).
    let estimated: Vec<bool> = rows
        .iter()
        .map(|r| r.dapp.as_ref().expect("a dApp row").estimated)
        .collect();
    assert_eq!(estimated, vec![false, true, true]);
    assert_eq!(
        rows[0].dapp.as_ref().map(|d| d.changes.clone()),
        Some(vec![
            exact(line(FeedDirection::Out, "ETH", "0.0001", 18)),
            line(FeedDirection::In, "USDC", "0.269487", 6),
        ])
    );

    let dapp = rows[2].dapp.clone().expect("a dApp row");
    assert_eq!(dapp.intent, None, "no title the wallet cannot know");
    assert_eq!(dapp.intent_term, None);
    assert_eq!(dapp.site.as_deref(), Some("app.uniswap.org"));
    assert!(dapp.contract_call, "the router is what it called");
    assert_eq!(
        dapp.changes,
        vec![
            line(FeedDirection::Out, "USDC", "0.1", 6),
            line(FeedDirection::In, "ETH", "0.000037", 18),
        ],
        "the sheet's lines, in its order"
    );
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "ETH", "0.000037", 18))
    );
    assert_eq!(
        rows[0].dapp.as_ref().and_then(|d| d.received.clone()),
        Some(line(FeedDirection::In, "USDC", "0.269487", 6))
    );
    // The counterparty is still the router, and only because it is an
    // address (083 H2 review).
    assert_eq!(rows[2].counterparty.as_deref(), Some(ROUTER));
}

/// A record that kept no lines — an older row, a web or phone record, a
/// simulation that never answered — draws EXACTLY as before, and an empty
/// list is no lines.
#[test]
fn no_recorded_changes_leaves_the_row_as_it_was() {
    let before = boot(vec![
        swap("d-call", "0x0", None, 100_000.0),
        swap("d-send", "0x2386f26fc10000", None, 90_000.0),
    ]);
    let after = boot(vec![
        swap("d-call", "0x0", Some(Vec::new()), 100_000.0),
        swap("d-send", "0x2386f26fc10000", Some(Vec::new()), 90_000.0),
    ]);
    assert_eq!(items(&before), items(&after));
    let rows = items(&before);
    assert_eq!(rows[0].value, None, "a call that moved no coin: no figure");
    assert_eq!(rows[0].symbol, "");
    assert_eq!(rows[1].value.as_deref(), Some("0.01"), "its own wei value");
    assert_eq!(rows[1].symbol, "ETH");
    for row in &rows {
        let dapp = row.dapp.as_ref().expect("a dApp row");
        assert!(dapp.changes.is_empty() && dapp.received.is_none());
    }
    // …and a record that says nothing about calldata is no contract call.
    let mut plain = swap("d-plain", "0x1", None, 80_000.0);
    plain.calldata = None;
    let sut = boot(vec![plain]);
    assert!(
        !items(&sut)[0]
            .dapp
            .as_ref()
            .expect("a dApp row")
            .contract_call
    );
}

/// A page cannot put a figure on the row. The lines come from the approve
/// alone (the sign_request tests hold that end); here, the sheet's own
/// judgment is kept: a token the wallet did not trust arrives as a direction
/// with no number — whatever `Transfer` its contract emitted — and it is
/// never the row's figure or its "expected back".
#[test]
fn a_page_cannot_put_a_figure_on_the_row() {
    let unverified_in = TrustSimJudgment::Erc20Unverified {
        token: Some(SITE_TOKEN.to_owned()),
        delta: "1000000000000000000000000".to_owned(),
    };
    let unverified_out = TrustSimJudgment::Erc20Unverified {
        token: Some(SITE_TOKEN.to_owned()),
        delta: "-5000000".to_owned(),
    };
    let sut = boot(vec![
        // USDC out, the site's token "in": the figure is the USDC.
        swap(
            "d-lure",
            "0x0",
            Some(vec![usdc("-100000"), unverified_in.clone()]),
            100_000.0,
        ),
        // Only an unverified outflow: no figure it could give.
        swap("d-blind", "0x0", Some(vec![unverified_out]), 90_000.0),
        // Two coins out: no ONE figure, so the call's own value, as before.
        swap(
            "d-two",
            "0x0",
            Some(vec![usdc("-100000"), eth("-1000")]),
            80_000.0,
        ),
    ]);
    let rows = items(&sut);

    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (Some("0.1"), "USDC")
    );
    let lure = rows[0].dapp.clone().expect("a dApp row");
    assert_eq!(lure.received, None, "no figure the site wrote beside it");
    assert_eq!(
        lure.changes[1],
        FeedDappChange {
            direction: FeedDirection::In,
            verified: false,
            symbol: String::new(),
            value: None,
            decimals: None,
            exact: false,
        }
    );

    assert_eq!(
        (rows[1].value.as_deref(), rows[1].symbol.as_str()),
        (None, "")
    );
    let blind = rows[1].dapp.clone().expect("a dApp row");
    assert_eq!(blind.changes.len(), 1);
    assert_eq!(blind.changes[0].direction, FeedDirection::Out);
    assert_eq!(blind.changes[0].value, None);

    assert_eq!(
        (rows[2].value.as_deref(), rows[2].symbol.as_str()),
        (None, "")
    );
    let two = rows[2].dapp.clone().expect("a dApp row");
    assert_eq!(two.changes.len(), 2, "the detail still lists both");
    assert_eq!(two.received, None);
}

/// A line that does not say something is not drawn: zero moved nothing, and
/// a delta that is not a signed integer has no direction to state. A stored
/// decimals past the bound keeps its direction and loses its figure.
#[test]
fn a_line_that_does_not_read_is_not_drawn() {
    let odd = TrustSimJudgment::Erc20Trusted {
        token: USDC_BASE.to_owned(),
        delta: "-1".to_owned(),
        symbol: "ODD".to_owned(),
        decimals: u32::MAX,
        in_trusted_set: true,
    };
    let sut = boot(vec![swap(
        "d-odd",
        "0x0",
        Some(vec![eth("0"), eth("12abc"), eth(""), usdc("+250000"), odd]),
        100_000.0,
    )]);
    let rows = items(&sut);
    let dapp = rows[0].dapp.clone().expect("a dApp row");
    assert_eq!(
        dapp.changes,
        vec![
            line(FeedDirection::In, "USDC", "0.25", 6),
            FeedDappChange {
                direction: FeedDirection::Out,
                verified: true,
                symbol: "ODD".to_owned(),
                value: None,
                decimals: None,
                exact: false,
            },
        ]
    );
    assert_eq!(
        rows[0].value, None,
        "an outflow with no figure is no figure"
    );
    assert_eq!(dapp.received, None, "nothing out, so nothing beside it");
}

/// A swap in flight says what it is doing, and one that lands keeps it. One
/// that FAILED moved nothing (083 F1 review): Activity's rows carry no status
/// mark, so "−0.1 USDC / ≈ +0.000037 ETH" on a reverted swap would read as a
/// swap that happened. It keeps none of what the sheet expected — no figure
/// from it, nothing "≈ back", no lines for the detail — and draws exactly as
/// it did before F1: the call's own value, here none.
#[test]
fn a_failed_swap_claims_nothing_it_expected() {
    let pending = swap(
        "dapp-9-tx",
        "0x0",
        Some(vec![usdc("-100000"), eth("37000000000000")]),
        100_000.0,
    );
    let mut sut = boot(vec![pending.clone()]);
    assert_eq!(sut.view().transactions[0].status, FeedTxStatus::Pending);
    let in_flight = items(&sut).remove(0);
    assert_eq!(in_flight.value.as_deref(), Some("0.1"));
    assert!(in_flight
        .dapp
        .as_ref()
        .is_some_and(|d| d.received.is_some()));

    let settle = |sut: &mut Sut, status: FeedTxStatus| {
        sut.dispatch(Event::ReconcileCompleted { resolved_count: 1 });
        let mut record = pending.clone();
        record.status = status;
        sut.resolve(loaded(sut, vec![record], T0 + 1_000.0));
        drain_aliases(sut);
        assert_eq!(sut.view().transactions[0].status, status);
        items(sut).remove(0)
    };
    let landed = settle(&mut sut, FeedTxStatus::Confirmed);
    assert_eq!(landed, in_flight, "confirmed: what it was doing, it did");

    let failed = settle(&mut sut, FeedTxStatus::Failed);
    assert_eq!(
        (failed.value.as_deref(), failed.symbol.as_str()),
        (None, "")
    );
    assert!(failed.usd_value.abs() < f64::EPSILON);
    let dapp = failed.dapp.expect("still a dApp row");
    assert_eq!(dapp.received, None, "nothing \"≈ back\" on a revert");
    assert!(
        dapp.changes.is_empty(),
        "no balance changes that never were"
    );
    assert!(!dapp.estimated);
    assert_eq!(dapp.site.as_deref(), Some("app.uniswap.org"));
    assert!(dapp.contract_call, "it still called the router");

    // A failed call that sent the coin keeps the figure it always had.
    let mut sent = swap(
        "dapp-8-tx",
        "0x5af3107a4000",
        Some(vec![eth("-100000000000000"), usdc("269487")]),
        90_000.0,
    );
    sent.status = FeedTxStatus::Failed;
    let row = items(&boot(vec![sent])).remove(0);
    assert_eq!(
        (row.value.as_deref(), row.symbol.as_str()),
        (Some("0.0001"), "ETH")
    );
    assert_eq!(row.dapp.and_then(|d| d.received), None);
}

/// A site cannot write the row's headline with a token of its own (083 F1
/// review). Its contract can emit `Transfer(you, …)` and answer "USDC", and
/// the sheet shows that outflow — it overstates a spend, the safe side there.
/// But only the chain's coin or a token the wallet already trusts is ever
/// the ROW's figure (or its "≈ back"), and so its dollar value: the site's
/// line stays in the detail, as the sheet drew it, and the row falls back to
/// the call's own value.
#[test]
fn a_site_token_is_never_the_rows_figure() {
    let sut = boot(vec![
        // The site's "USDC" leaving alone: no figure, no dollars.
        swap("d-fake", "0x0", Some(vec![site_usdc("-100000")]), 100_000.0),
        // The real USDC out and the site's "USDC" out: two coins, no one
        // figure — and the fake is not counted as the trusted one either.
        swap(
            "d-both",
            "0x0",
            Some(vec![site_usdc("-100000000000"), usdc("-100000")]),
            90_000.0,
        ),
        // Real USDC out, the site's "USDC" in (as a trusted-looking line):
        // the figure is the real one and nothing rides beside it.
        swap(
            "d-back",
            "0x0",
            Some(vec![usdc("-100000"), site_usdc("5000000")]),
            80_000.0,
        ),
        // The coin the call sent is its own figure, whatever a token says.
        swap(
            "d-coin",
            "0x2386f26fc10000",
            Some(vec![eth("-10000000000000000"), site_usdc("-1")]),
            70_000.0,
        ),
    ]);
    let rows = items(&sut);

    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (None, "")
    );
    assert!(
        rows[0].usd_value.abs() < f64::EPSILON,
        "no price for a claim"
    );
    let fake = rows[0].dapp.clone().expect("a dApp row");
    assert!(!fake.estimated && fake.received.is_none());
    assert_eq!(
        fake.changes,
        vec![line(FeedDirection::Out, "USDC", "0.1", 6)],
        "the detail still says what the sheet showed"
    );

    assert_eq!(
        (rows[1].value.as_deref(), rows[1].symbol.as_str()),
        (None, "")
    );
    assert_eq!(rows[1].dapp.as_ref().map(|d| d.changes.len()), Some(2));

    assert_eq!(
        (rows[2].value.as_deref(), rows[2].symbol.as_str()),
        (Some("0.1"), "USDC")
    );
    assert_eq!(rows[2].dapp.as_ref().and_then(|d| d.received.clone()), None);
    assert_eq!(
        rows[2].dapp.as_ref().map(|d| d.changes[1].clone()),
        Some(FeedDappChange {
            direction: FeedDirection::In,
            verified: false,
            symbol: String::new(),
            value: None,
            decimals: None,
            exact: false,
        }),
        "a site's coin arriving is an unverified token in the detail too, whatever the shell judged"
    );

    // Two coins out, so no simulated figure: the call's own 0.01 ETH, exact.
    assert_eq!(
        (rows[3].value.as_deref(), rows[3].symbol.as_str()),
        (Some("0.01"), "ETH")
    );
    assert!(!rows[3].dapp.as_ref().expect("a dApp row").estimated);
}

/// The new fields are optional on the wire both ways: an older shell's
/// record loads, and a row with none of them serializes as it did.
#[test]
fn the_balance_change_fields_are_optional_on_the_wire() {
    let json = r#"{"id":"x","user_op_hash":"","tx_hash":"","from":"","to":"","to_name":null,
        "value":"1","symbol":"USDC","decimals":6,"logo_urls":null,"chain_id":1,"timestamp":1,
        "day_start_ms":0,"status":"confirmed","kind":"dapp_tx","usd":null}"#;
    let record: FeedTxRecord = serde_json::from_str(json).expect("an older shell's record");
    assert_eq!(
        (record.balance_changes.clone(), record.calldata),
        (None, None)
    );
    let out = serde_json::to_value(&record).expect("serializes");
    assert!(out.get("balance_changes").is_none() && out.get("calldata").is_none());

    let sut = boot(vec![swap("d-old", "0x0", None, 100_000.0)]);
    let mut row = items(&sut).remove(0);
    row.dapp.as_mut().expect("a dApp row").contract_call = false;
    let dapp = serde_json::to_value(row.dapp).expect("serializes");
    for key in ["changes", "received", "estimated", "contract_call"] {
        assert!(dapp.get(key).is_none(), "{key} is absent when empty");
    }
    // A line's `exact`, and a judgment's `in_trusted_set`, likewise.
    let change = serde_json::to_value(line(FeedDirection::In, "ETH", "1", 18)).expect("serializes");
    assert!(change.get("exact").is_none());
    let judgment = serde_json::to_value(site_usdc("-1")).expect("serializes");
    assert!(judgment.get("in_trusted_set").is_none());
    let older: TrustSimJudgment = serde_json::from_str(
        r#"{"type":"erc20_trusted","token":"0xa","delta":"-1","symbol":"A","decimals":6}"#,
    )
    .expect("an older judgment");
    assert!(matches!(
        older,
        TrustSimJudgment::Erc20Trusted {
            in_trusted_set: false,
            ..
        }
    ));
}
