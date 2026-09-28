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
    is_stable, tx_usd_value, ActivityFeed, Event, FeedBatchKind, FeedDapp, FeedDirection,
    FeedOperation as Op, FeedRow, FeedShellResult as Res, FeedTxKind, FeedTxRecord, FeedTxStatus,
};
use vela_core::app::clear_signing::ClearTerm;

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
        dapp_origin: None,
        intent: None,
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
    r.dapp_origin = Some("http://127.0.0.1:5173".to_owned());
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
        "0xCafe",
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
    assert_eq!(row.counterparty.as_deref(), Some("0xCafe"));
    assert_eq!(row.tx_hash, None, "pending: no hash yet");
    assert_eq!(
        row.dapp,
        Some(FeedDapp {
            site: Some("127.0.0.1".to_owned()),
            intent: Some("Send".to_owned()),
            intent_term: Some(ClearTerm::IntentSend),
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
    swap.dapp_origin = Some("https://app.uniswap.org".to_owned());
    let mut blind = dapp_tx("dapp-3-tx", "", "0x0", None, 190_000.0);
    blind.dapp_origin = Some("Some dApp".to_owned());
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
    assert_eq!(
        blind.site.as_deref(),
        Some("Some dApp"),
        "a stored name, verbatim"
    );
    assert_eq!((blind.intent, blind.intent_term), (None, None));
}

/// Another account's dApp transaction is not this account's Activity, and a
/// record from a shell that stores no origin names no site.
#[test]
fn a_dapp_row_is_account_scoped_and_survives_a_missing_origin() {
    let mut ours = dapp_tx("d-ours", "0xCafe", "0x1", None, 100_000.0);
    ours.dapp_origin = None;
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
        (record.dapp_origin.clone(), record.intent.clone()),
        (None, None)
    );
    let out = serde_json::to_value(&record).expect("serializes");
    assert!(out.get("dapp_origin").is_none() && out.get("intent").is_none());
}
