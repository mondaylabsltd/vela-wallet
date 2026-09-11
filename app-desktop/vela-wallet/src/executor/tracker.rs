//! The only place the `tx_tracker` machine touches the outside world — and
//! the app-resident that keeps money in flight alive across every screen.
//!
//! Six operations. Every throttle (3 s receipt, 12 s status, 12 s reconcile),
//! the 120 s window, the 24 h abandon line and every verdict are the core's.
//! What is here is what the core's own note assigns to the shell:
//!
//! - **The wording layer, collapsed to a typed axis.** `relay::user_op_receipt`
//!   already says whether the relay was reached and whether it answered
//!   definitively; an unreachable relay is `ReceiptUnreachable` and NEVER a
//!   failure. `relay::user_op_status` answers `None` for an older relay.
//! - **The clock.** Every time-bearing result carries `now_ms`.
//! - **The receipt's by-products.** `NotifyConfirmed` carries only the hash;
//!   the AUTHENTIC logs and the sender are facts this shell held from the poll
//!   it just made, and they are what `token_trust` admits tokens from. Cached
//!   per hash here, consumed once.
//!
//! ## Started on every boot, not when a send opens
//!
//! The web learned this the hard way: a tracker that starts with the send
//! flow never sweeps an operation a closed window left pending. This one is
//! booted by the wallet page and ticked every three seconds by
//! [`start_ticks`]; the core decides which tick actually polls.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/wallet/core/tracker-executor.ts`
//! @ `origin/main`.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use gpui::App;
use serde_json::Value;

use vela_core::app::token_trust::TrustReceiptLog;
use vela_core::app::tx_tracker::{
    Event, TrackOperation, TrackPendingRecord, TrackRecordStatus, TrackShellResult, TxTracker,
};

use crate::executor::{relay, storage, token_trust};
use crate::resident::{self, Answer, Machine};

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";
/// The two record kinds a submission produces; `type` is optional on older
/// rows and defaults to `send`, exactly as the store always said.
const TRACKED_TYPES: [&str; 2] = ["send", "dapp_tx"];
/// A long-lived window can watch many ops; the cap keeps the by-product
/// cache bounded and the entries are consumed by their own `NotifyConfirmed`.
const MAX_TRACKED_RECEIPTS: usize = 64;
/// The shell's cadence. Any frequency is safe — the core throttles.
const TICK: Duration = Duration::from_secs(3);

/// hash → (sender, authentic logs) of the receipt that confirmed it.
static RECEIPTS: Mutex<Option<HashMap<String, (Option<String>, Vec<TrustReceiptLog>)>>> =
    Mutex::new(None);

fn normalize(hash: &str) -> String {
    hash.to_lowercase()
}

fn remember_receipt(hash: &str, sender: Option<String>, logs: Vec<TrustReceiptLog>) {
    if let Ok(mut cache) = RECEIPTS.lock() {
        let map = cache.get_or_insert_with(HashMap::new);
        if map.len() >= MAX_TRACKED_RECEIPTS {
            map.clear();
        }
        map.insert(normalize(hash), (sender, logs));
    }
}

fn take_receipt(hash: &str) -> Option<(Option<String>, Vec<TrustReceiptLog>)> {
    RECEIPTS.lock().ok()?.as_mut()?.remove(&normalize(hash))
}

/// The still-pending submissions the reconcile sweep answers with — the
/// union of the two scans it replaces (any `send`, and the dApp scan's
/// `dapp_tx`). Deliberately NOT filtered by account: the core keys by hash
/// and patches by id, so the honest superset is the right answer. The 24 h
/// line is the core's.
fn pending_records(rows: &[Value]) -> Vec<TrackPendingRecord> {
    rows.iter()
        .filter_map(|row| {
            let text = |key: &str| row.get(key).and_then(Value::as_str).unwrap_or_default();
            if text("status") != "pending" {
                return None;
            }
            let user_op_hash = text("userOpHash");
            if user_op_hash.is_empty() || !text("txHash").is_empty() {
                return None;
            }
            let kind = row.get("type").and_then(Value::as_str).unwrap_or("send");
            if !TRACKED_TYPES.contains(&kind) {
                return None;
            }
            Some(TrackPendingRecord {
                record_id: row.get("id")?.as_str()?.to_owned(),
                user_op_hash: user_op_hash.to_owned(),
                chain_id: row
                    .get("chainId")
                    .and_then(Value::as_u64)
                    .and_then(|id| u32::try_from(id).ok())?,
                // Stored in SECONDS; the core measures every deadline in ms.
                submitted_at_ms: row.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0)
                    * 1000.0,
            })
        })
        .collect()
}

fn read_rows() -> Vec<Value> {
    match storage::read_value(TX_KEY) {
        Ok(Some(Value::Array(rows))) => rows,
        _ => Vec::new(),
    }
}

/// Patch the named records in place — same ids, never a second record — in
/// ONE write (`updateTransactions`).
fn patch_records(ids: &[String], status: TrackRecordStatus, tx_hash: Option<&str>) {
    let mut rows = read_rows();
    let mut touched = false;
    for row in &mut rows {
        let Some(id) = row.get("id").and_then(Value::as_str) else {
            continue;
        };
        if !ids.iter().any(|wanted| wanted == id) {
            continue;
        }
        if let Some(object) = row.as_object_mut() {
            object.insert(
                "status".to_owned(),
                Value::String(
                    match status {
                        TrackRecordStatus::Confirmed => "confirmed",
                        TrackRecordStatus::Failed => "failed",
                    }
                    .to_owned(),
                ),
            );
            if let Some(tx_hash) = tx_hash {
                object.insert("txHash".to_owned(), Value::String(tx_hash.to_owned()));
            }
            touched = true;
        }
    }
    if touched {
        let _ = storage::write_value(TX_KEY, Value::Array(rows));
    }
}

/// The sender of a submission, from its own stored row — the fallback when
/// the relay's receipt did not name one.
fn stored_sender(user_op_hash: &str) -> Option<String> {
    read_rows().iter().find_map(|row| {
        let hash = row.get("userOpHash").and_then(Value::as_str)?;
        if !hash.eq_ignore_ascii_case(user_op_hash) {
            return None;
        }
        row.get("from")
            .and_then(Value::as_str)
            .filter(|from| !from.is_empty())
            .map(str::to_owned)
    })
}

impl Machine for TxTracker {
    const LABEL: &'static str = "tx_tracker";

    /// A reconcile sweep: whatever a closed window left pending is picked up
    /// before any screen asks.
    fn boot_event(_cx: &App) -> Event {
        Event::AppResumed
    }

    fn perform(operation: &TrackOperation) -> Answer<TrackShellResult, Self::Event> {
        let now_ms = crate::executor::now_ms;
        match operation {
            TrackOperation::PollReceipt {
                user_op_hash,
                chain_id,
            } => {
                let (hash, chain_id) = (user_op_hash.clone(), *chain_id);
                Answer::Blocking(Box::new(move || {
                    let poll = relay::user_op_receipt(&hash, chain_id);
                    match poll.resolution {
                        None if !poll.reached_bundler => TrackShellResult::ReceiptUnreachable {
                            user_op_hash: hash,
                            now_ms: now_ms(),
                        },
                        None => TrackShellResult::ReceiptPending {
                            user_op_hash: hash,
                            now_ms: now_ms(),
                        },
                        Some(resolution) => {
                            let logs = resolution.logs.clone();
                            remember_receipt(&hash, resolution.sender, resolution.logs);
                            if resolution.confirmed {
                                // With the authentic logs: the core decides whether
                                // the Safe inside the op actually executed (spec 038
                                // #D1 — `ExecutionFailure` under a `success: true`).
                                TrackShellResult::ReceiptWithLogs {
                                    user_op_hash: hash,
                                    tx_hash: resolution.tx_hash,
                                    now_ms: now_ms(),
                                    logs,
                                }
                            } else {
                                TrackShellResult::ReceiptFailed {
                                    user_op_hash: hash,
                                    tx_hash: resolution.tx_hash,
                                    now_ms: now_ms(),
                                }
                            }
                        }
                    }
                }))
            }

            TrackOperation::PollStatus {
                user_op_hash,
                chain_id,
            } => {
                let (hash, chain_id) = (user_op_hash.clone(), *chain_id);
                Answer::Blocking(Box::new(move || {
                    match relay::user_op_status(&hash, chain_id) {
                        Some((status, stage)) => TrackShellResult::Status {
                            user_op_hash: hash,
                            status,
                            stage,
                            now_ms: now_ms(),
                        },
                        None => TrackShellResult::StatusUnavailable {
                            user_op_hash: hash,
                            now_ms: now_ms(),
                        },
                    }
                }))
            }

            TrackOperation::LoadPendingTxs => Answer::Now(TrackShellResult::RecordsLoaded {
                records: pending_records(&read_rows()),
                now_ms: now_ms(),
            }),

            TrackOperation::UpdateTxRecords { ids, patch } => {
                patch_records(ids, patch.status, patch.tx_hash.as_deref());
                // Records changed UNDER the feed, which is reading the same
                // store and has no way to know. Counted here and handed over on
                // the next tick (this function has no `cx` to reach another
                // machine with); the feed's own event for it is
                // `ReconcileCompleted`, and the core is explicit that it
                // re-reads without celebrating.
                PATCHED.fetch_add(
                    u32::try_from(ids.len()).unwrap_or(u32::MAX),
                    Ordering::SeqCst,
                );
                Answer::Now(TrackShellResult::RecordsPatched)
            }

            // The single auto-add entry point: the AUTHENTIC logs of the
            // receipt that confirmed this op, handed to `token_trust`.
            TrackOperation::NotifyConfirmed {
                user_op_hash,
                chain_id,
                ..
            } => {
                if let Some((sender, logs)) = take_receipt(user_op_hash) {
                    let from = sender.or_else(|| stored_sender(user_op_hash));
                    if let Some(from) = from
                        && !logs.is_empty()
                    {
                        token_trust::receipt_confirmed(&from, *chain_id, logs);
                    }
                }
                Answer::Now(TrackShellResult::Notified)
            }

            TrackOperation::Now => Answer::Now(TrackShellResult::Clock { now_ms: now_ms() }),
        }
    }
}

static TICKING: AtomicBool = AtomicBool::new(false);
/// Records the tracker has patched since the feed was last told.
static PATCHED: AtomicU32 = AtomicU32::new(0);

/// Boot the tracker and keep it ticking for the life of the process. Called
/// by the wallet page on every sign-in; a second call is a no-op.
pub fn start(cx: &mut App) {
    let _ = resident::resident::<TxTracker>(cx);
    if TICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(TICK).await;
            // Re-fetched each tick: a sign-out drops every resident, and the
            // next sign-in's tracker must be the one that gets the ticks.
            let tracker = cx.update(|cx| resident::resident::<TxTracker>(cx));
            tracker.update(cx, |resident, cx| resident.dispatch(Event::Tick, cx));
            // Anything the sweep just converged, handed to the feed within one
            // tick rather than at its own 30 s pass: the row a person is
            // watching says "pending" until somebody re-reads the store.
            let patched = PATCHED.swap(0, Ordering::SeqCst);
            if patched > 0 {
                let _ = cx.update(|cx| crate::executor::activity_feed::reconciled(patched, cx));
            }
        }
    })
    .detach();
}

/// The window came back. The core turns this into a reconcile sweep — the
/// pass that converges every pending submission, not just the throttled poll a
/// tick would run.
pub fn focused(cx: &mut App) {
    resident::resident::<TxTracker>(cx).update(cx, |resident, cx| {
        resident.dispatch(Event::HomeFocused, cx);
    });
}

/// A user operation was accepted: hand it to the tracker, whose patches
/// will find the records the send path already persisted.
pub fn submitted(user_op_hash: String, record_ids: Vec<String>, chain_id: u32, cx: &mut App) {
    resident::resident::<TxTracker>(cx).update(cx, |resident, cx| {
        resident.dispatch(
            Event::Submitted {
                user_op_hash,
                record_ids,
                chain_id,
            },
            cx,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: &str, status: &str, hash: &str, tx: &str, kind: Option<&str>) -> Value {
        let mut row = json!({
            "id": id, "status": status, "userOpHash": hash, "txHash": tx,
            "chainId": 100, "timestamp": 1_700_000_000, "from": "0xfrom"
        });
        if let Some(kind) = kind {
            row["type"] = json!(kind);
        }
        row
    }

    /// Only a pending submission with a hash and no receipt yet is a live
    /// submission; a legacy row without `type` is a send.
    #[test]
    fn the_sweep_finds_live_submissions_only() {
        let rows = vec![
            row("a", "pending", "0xaaa", "", None),
            row("b", "pending", "0xbbb", "", Some("dapp_tx")),
            row("c", "pending", "", "", Some("send")),
            row("d", "pending", "0xddd", "0xtx", Some("send")),
            row("e", "confirmed", "0xeee", "", Some("send")),
            row("f", "pending", "0xfff", "", Some("receive")),
        ];
        let live = pending_records(&rows);
        let ids: Vec<&str> = live.iter().map(|r| r.record_id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(live[0].chain_id, 100);
        assert_eq!(live[0].submitted_at_ms, 1_700_000_000_000.0);
    }

    /// A patch rewrites the named rows in place and leaves every other row
    /// — and a failed patch never writes a hash.
    #[test]
    fn a_patch_rewrites_in_place() {
        crate::executor::storage::tests::with_temp_state("tracker-patch", || {
            let rows = vec![
                row("a", "pending", "0xaaa", "", None),
                row("b", "pending", "0xaaa", "", None),
                row("c", "pending", "0xccc", "", None),
            ];
            let _ = storage::write_value(TX_KEY, Value::Array(rows));
            patch_records(
                &["a".to_owned(), "b".to_owned()],
                TrackRecordStatus::Confirmed,
                Some("0xtx"),
            );
            patch_records(&["c".to_owned()], TrackRecordStatus::Failed, None);
            let rows = read_rows();
            assert_eq!(rows.len(), 3);
            assert_eq!(rows[0]["status"], "confirmed");
            assert_eq!(rows[0]["txHash"], "0xtx");
            assert_eq!(rows[1]["txHash"], "0xtx");
            assert_eq!(rows[2]["status"], "failed");
            assert_eq!(rows[2]["txHash"], "");
            assert_eq!(pending_records(&rows).len(), 0);
            assert_eq!(stored_sender("0xAAA").as_deref(), Some("0xfrom"));
        });
    }

    /// The receipt's by-products are held for exactly one notification.
    #[test]
    fn a_receipt_s_logs_are_consumed_once() {
        remember_receipt(
            "0xABC",
            Some("0xsender".to_owned()),
            vec![TrustReceiptLog {
                address: "0xtoken".to_owned(),
                topics: Vec::new(),
                data: "0x".to_owned(),
            }],
        );
        let (sender, logs) = take_receipt("0xabc").unwrap_or_else(|| unreachable!("held"));
        assert_eq!(sender.as_deref(), Some("0xsender"));
        assert_eq!(logs.len(), 1);
        assert!(take_receipt("0xabc").is_none());
    }

    /// Local operations answer now; the relay ones block.
    #[test]
    fn the_operation_split_is_by_where_the_answer_comes_from() {
        crate::executor::storage::tests::with_temp_state("tracker-split", || {
            assert!(matches!(
                TxTracker::perform(&TrackOperation::LoadPendingTxs),
                Answer::Now(TrackShellResult::RecordsLoaded { .. })
            ));
            assert!(matches!(
                TxTracker::perform(&TrackOperation::Now),
                Answer::Now(TrackShellResult::Clock { .. })
            ));
            assert!(matches!(
                TxTracker::perform(&TrackOperation::PollReceipt {
                    user_op_hash: "0x1".to_owned(),
                    chain_id: 100
                }),
                Answer::Blocking(_)
            ));
            assert!(matches!(
                TxTracker::perform(&TrackOperation::PollStatus {
                    user_op_hash: "0x1".to_owned(),
                    chain_id: 100
                }),
                Answer::Blocking(_)
            ));
            assert!(matches!(
                TxTracker::perform(&TrackOperation::NotifyConfirmed {
                    user_op_hash: "0xnothing".to_owned(),
                    chain_id: 100,
                    tx_hash: "0x".to_owned()
                }),
                Answer::Now(TrackShellResult::Notified)
            ));
        });
    }
}
