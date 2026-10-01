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
use serde_json::{Value, json};

use vela_core::app::token_trust::TrustReceiptLog;
use vela_core::app::tx_tracker::{
    Event, TrackOperation, TrackPendingRecord, TrackRecordStatus, TrackShellResult, TxTracker,
};

use crate::diag::{short, vlog};
use crate::executor::pool::PoolError;
use crate::executor::{pool, relay, storage, token_trust};
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
pub(crate) fn pending_records(rows: &[Value]) -> Vec<TrackPendingRecord> {
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
                // Spec 082 T181: an op whose submit reply was lost is still
                // one after a relaunch — its "may have been sent" ending, the
                // NotSent verdict and the on-chain search all hang on these.
                // A row from before the fields reads as it always did.
                maybe_sent: row
                    .get("maybeSent")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                submit_block: row.get("submitBlock").and_then(Value::as_u64),
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

/// Is a record for `user_op_hash` on disk the way the relaunch sweep hands
/// it back — pending, under this hash (spec 082 RJ1)? What a write-ahead must
/// have left before its POST may go, so a quit at any moment after finds the
/// payment: `ClearToPost` follows the write's acknowledgement, which is given
/// whether or not the store took the row.
pub(crate) fn resumable(user_op_hash: &str) -> bool {
    pending_records(&read_rows())
        .iter()
        .any(|record| record.user_op_hash.eq_ignore_ascii_case(user_op_hash))
}

/// Patch the named records in place — same ids, never a second record — in
/// ONE write (`updateTransactions`).
fn patch_records(ids: &[String], status: TrackRecordStatus, tx_hash: Option<&str>) {
    // Under the store's lock (spec 082 RJ1 review): a dApp's record is
    // written on a worker meanwhile, and a stale copy written back here
    // would take it away.
    let _ = storage::update_list(TX_KEY, |rows| patch_rows(rows, ids, status, tx_hash));
}

fn patch_rows(
    rows: &mut [Value],
    ids: &[String],
    status: TrackRecordStatus,
    tx_hash: Option<&str>,
) -> bool {
    let mut touched = false;
    for row in rows.iter_mut() {
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
    touched
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
                            // Its nonce is spent, however it ended: the
                            // account's next operation builds on it (083).
                            crate::executor::user_op::note_landed(&hash);
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
                        Some(answer) => TrackShellResult::Status {
                            user_op_hash: hash,
                            status: answer.status,
                            stage: answer.stage,
                            now_ms: now_ms(),
                            // The bundle tx the relay names while no receipt
                            // has (RA7): an explorer link, never a verdict.
                            tx_hash: answer.tx_hash,
                        },
                        None => TrackShellResult::StatusUnavailable {
                            user_op_hash: hash,
                            now_ms: now_ms(),
                        },
                    }
                }))
            }

            // Spec 082 ruling 8: the relay-independent landing check. The
            // pool's answer goes back as it came — what a range error is,
            // which window comes next and what a found log means are the
            // core's.
            TrackOperation::FindOpEvent {
                chain_id,
                entry_point,
                topic0,
                user_op_hash,
                from_block,
                to_block,
            } => {
                let search = OpSearch {
                    chain_id: *chain_id,
                    entry_point: entry_point.clone(),
                    topic0: topic0.clone(),
                    user_op_hash: user_op_hash.clone(),
                    from_block: *from_block,
                    to_block: *to_block,
                };
                Answer::Blocking(Box::new(move || {
                    let chain_id = search.chain_id;
                    find_op_event(&search, |method, params| {
                        pool::call(chain_id, method, params)
                    })
                }))
            }

            // Spec 082 RE8 (G26): an op of ours landed — confirmed, or failed
            // with gas spent — so the balance on screen is out of date now,
            // not at the next ten-minute pass.
            TrackOperation::HoldingsMoved { chain_id } => {
                vlog!(
                    "tracker",
                    "holdings moved on chain={chain_id}; balances re-read"
                );
                crate::executor::balance_dashboard::invalidate();
                Answer::Now(TrackShellResult::Notified)
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

            // Spec 082 RJ4 (G38): the bundle transaction the relay's status
            // named, read from the chain rather than waited for from the
            // relay. The answer goes back as it came; which log is the op's,
            // and what it says, are the core's.
            TrackOperation::TxReceipt {
                chain_id,
                tx_hash,
                user_op_hash,
            } => {
                let (chain_id, tx_hash, user_op_hash) =
                    (*chain_id, tx_hash.clone(), user_op_hash.clone());
                Answer::Blocking(Box::new(move || {
                    tx_receipt(chain_id, &tx_hash, &user_op_hash, |method, params| {
                        pool::call(chain_id, method, params)
                    })
                }))
            }
        }
    }
}

/// One `FindOpEvent`, owned so it can cross to the worker.
struct OpSearch {
    chain_id: u32,
    entry_point: String,
    topic0: String,
    user_op_hash: String,
    from_block: Option<u64>,
    to_block: Option<u64>,
}

fn quantity(block: u64) -> String {
    format!("0x{block:x}")
}

/// Run one find-event through `call` (the pool, or a fake in the tests):
/// the head from `eth_blockNumber`, and — unless the core asked for the head
/// only — `eth_getLogs` on the EntryPoint filtered by the event topic and the
/// operation's hash. Answered as it came: the log array, or the JSON-RPC
/// error the endpoint gave (a range limit is one), or neither.
fn find_op_event(
    search: &OpSearch,
    mut call: impl FnMut(&str, Value) -> Result<Value, PoolError>,
) -> TrackShellResult {
    let head_block = call("eth_blockNumber", json!([]))
        .ok()
        .and_then(|body| {
            body.get("result")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .and_then(|hex| u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok());
    let (mut logs_json, mut error_json) = (None, None);
    if let Some(from) = search.from_block {
        let filter = json!({
            "address": search.entry_point,
            "topics": [search.topic0, search.user_op_hash],
            "fromBlock": quantity(from),
            "toBlock": search.to_block.map_or_else(|| "latest".to_owned(), quantity),
        });
        match call("eth_getLogs", json!([filter])) {
            Ok(body) => match body.get("error").filter(|error| !error.is_null()) {
                Some(error) => error_json = Some(error.to_string()),
                None => logs_json = body.get("result").map(Value::to_string),
            },
            // The pool held the endpoint's words for a range limit (T180);
            // they go to the core, which halves the window.
            Err(PoolError::RangeCap {
                error_json: held, ..
            }) => error_json = held,
            Err(PoolError::Failed { .. } | PoolError::Unavailable) => {}
        }
    }
    vlog!(
        "tracker",
        "op={} find-event chain={} from={:?} to={:?} head={head_block:?} → {}",
        short(&search.user_op_hash),
        search.chain_id,
        search.from_block,
        search.to_block,
        if logs_json.is_some() {
            "logs"
        } else if error_json.is_some() {
            "error"
        } else if search.from_block.is_none() {
            "head only"
        } else {
            "no answer"
        }
    );
    TrackShellResult::OpEvent {
        user_op_hash: search.user_op_hash.clone(),
        now_ms: crate::executor::now_ms(),
        logs_json,
        error_json,
        head_block,
    }
}

/// One `eth_getTransactionReceipt` through `call` (the chain pool, or a fake
/// in the tests), answered as it came: the JSON-RPC `result` — `null` while
/// the transaction is not mined — or no answer at all (an error member, or
/// nothing reached).
///
/// No token auto-add reads this receipt: its logs are the whole bundle's,
/// other senders' operations included, and which of them are this op's own
/// is the core's reading, not handed back here.
fn tx_receipt(
    chain_id: u32,
    tx_hash: &str,
    user_op_hash: &str,
    call: impl FnOnce(&str, Value) -> Result<Value, PoolError>,
) -> TrackShellResult {
    let receipt_json = call("eth_getTransactionReceipt", json!([tx_hash]))
        .ok()
        .filter(|body| body.get("error").is_none_or(Value::is_null))
        .and_then(|body| body.get("result").map(Value::to_string));
    vlog!(
        "tracker",
        "op={} receipt by tx={} chain={chain_id} → {}",
        short(user_op_hash),
        short(tx_hash),
        match receipt_json.as_deref() {
            None => "no answer",
            Some("null") => "not mined",
            Some(_) => "receipt",
        }
    );
    TrackShellResult::TxReceipt {
        user_op_hash: user_op_hash.to_owned(),
        now_ms: crate::executor::now_ms(),
        receipt_json,
    }
}

static TICKING: AtomicBool = AtomicBool::new(false);
/// Records the tracker has patched since the feed was last told.
static PATCHED: AtomicU32 = AtomicU32::new(0);
/// Records the signing path wrote (a dApp's pending record, its close) since
/// the feed was last told (spec 082 RG3).
static WRITTEN: AtomicU32 = AtomicU32::new(0);

/// A dApp record was written or closed in the background (spec 082 RG3,
/// T072): the feed is told on the next tick — `ReconcileCompleted`, the same
/// way the tracker's own patches reach it — so the row is on screen within
/// one tick rather than at the feed's 10–30 s pass. Counted here because the
/// write runs with no `cx`; never from the tracker hand-off.
pub fn records_written() {
    WRITTEN.fetch_add(1, Ordering::SeqCst);
}

/// How many record writes are waiting to be told to the feed.
#[cfg(test)]
pub fn records_waiting() -> u32 {
    WRITTEN.load(Ordering::SeqCst)
}

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
            let patched = PATCHED
                .swap(0, Ordering::SeqCst)
                .saturating_add(WRITTEN.swap(0, Ordering::SeqCst));
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

/// One operation for the tracker to follow (spec 082 RA4, ruling 8): its
/// hash, the records it patches, and — when the submit's reply was lost —
/// that it may have been sent and the head read before its first POST.
pub struct Handoff {
    pub user_op_hash: String,
    pub record_ids: Vec<String>,
    pub chain_id: u32,
    pub maybe_sent: bool,
    pub submit_block: Option<u64>,
    /// The relay took the op a write-ahead hand-off announced (spec 082
    /// RJ1): it is no longer in doubt, and a relay's `not_found` stops
    /// counting against it.
    pub admitted: bool,
}

/// A user operation is about to leave the device (the write-ahead, spec 082
/// RJ1) or has left it: hand it to the tracker, whose patches will find the
/// records the submit path already persisted.
pub fn submitted(handoff: Handoff, cx: &mut App) {
    vlog!(
        "tracker",
        "op={} handed over chain={} maybe_sent={} submit_block={:?} admitted={}",
        short(&handoff.user_op_hash),
        handoff.chain_id,
        handoff.maybe_sent,
        handoff.submit_block,
        handoff.admitted
    );
    resident::resident::<TxTracker>(cx).update(cx, |resident, cx| {
        resident.dispatch(
            Event::Submitted {
                user_op_hash: handoff.user_op_hash,
                record_ids: handoff.record_ids,
                chain_id: handoff.chain_id,
                maybe_sent: handoff.maybe_sent,
                submit_block: handoff.submit_block,
                admitted: handoff.admitted,
            },
            cx,
        );
    });
}

/// A written-ahead op was proven never sent (spec 082 RJ1): the tracker
/// drops these records — it never patches them, and they are deleted from
/// the store by the path that wrote them.
pub fn withdrawn(user_op_hash: &str, record_ids: &[String], cx: &mut App) {
    vlog!(
        "tracker",
        "op={} withdrawn ({} record{}): never sent",
        short(user_op_hash),
        record_ids.len(),
        if record_ids.len() == 1 { "" } else { "s" }
    );
    resident::resident::<TxTracker>(cx).update(cx, |resident, cx| {
        resident.dispatch(
            Event::Withdrawn {
                user_op_hash: user_op_hash.to_owned(),
                record_ids: record_ids.to_vec(),
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

    /// Spec 082 T181: a row written for a may-have-been-sent op reads back as
    /// one — through the same writers the two submit paths use — and a row
    /// from before the fields reads back as an ordinary pending op.
    #[test]
    fn a_may_have_been_sent_op_survives_a_relaunch() {
        crate::executor::storage::tests::with_temp_state("tracker-maybe-sent", || {
            let send = vela_core::app::send::SendTxRecord {
                id: "0xop-0".to_owned(),
                user_op_hash: "0xop".to_owned(),
                tx_hash: String::new(),
                from: "0xme".to_owned(),
                to: "0xyou".to_owned(),
                to_name: None,
                value: "0.001".to_owned(),
                symbol: "xDAI".to_owned(),
                decimals: 18,
                logo_urls: Vec::new(),
                chain_id: 100,
                timestamp_s: 1_700_000_000.0,
                usd: None,
                maybe_sent: true,
                submit_block: Some(48_479_132),
            };
            assert!(crate::executor::send::persist_records(&[send]));
            // A dApp's op whose reply was lost, through the signing path's
            // own writer.
            crate::executor::sign_request::persist_record(
                &vela_core::app::sign_request::SignRecord {
                    record_id: "dapp-1-tx".to_owned(),
                    kind: vela_core::app::sign_request::SignRecordKind::DappTx,
                    method: "eth_sendTransaction".to_owned(),
                    params_json:
                        r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x1"}]"#
                            .to_owned(),
                    result: String::new(),
                    from: "0xme".to_owned(),
                    chain_id: 100,
                    now_ms: 1_700_000_000_000.0,
                    status: vela_core::app::sign_request::SignRecordStatus::Pending,
                    user_op_hash: "0xdapp".to_owned(),
                    dapp_origin: "http://127.0.0.1:8137".to_owned(),
                    dapp_url: "http://127.0.0.1:8137".to_owned(),
                    intent: None,
                    maybe_sent: true,
                    submit_block: None,
                    balance_changes: None,
                },
            );
            let mut rows = read_rows();
            rows.push(row("legacy", "pending", "0xold", "", Some("send")));
            let live = pending_records(&rows);
            assert_eq!(live.len(), 3);
            assert_eq!(live[0].record_id, "0xop-0");
            assert!(live[0].maybe_sent);
            assert_eq!(live[0].submit_block, Some(48_479_132));
            assert_eq!(live[1].record_id, "dapp-1-tx");
            assert!(live[1].maybe_sent, "a dApp's lost reply is still one");
            assert_eq!(live[1].submit_block, None, "an unknown head stays unknown");
            assert!(!live[2].maybe_sent, "an old row is not may-have-been-sent");
            assert_eq!(live[2].submit_block, None);
        });
    }

    fn search(from_block: Option<u64>) -> OpSearch {
        OpSearch {
            chain_id: 100,
            entry_point: vela_core::safe::ENTRY_POINT.to_owned(),
            topic0: vela_core::user_op::USER_OPERATION_EVENT_TOPIC.to_owned(),
            user_op_hash: format!("0x{}", "ab".repeat(32)),
            from_block,
            to_block: from_block.map(|from| from + 999),
        }
    }

    /// The fake pool: the head is 0x2e3b5dc; `eth_getLogs` answers `logs`.
    fn run_search(
        from_block: Option<u64>,
        logs: Result<Value, PoolError>,
    ) -> (TrackShellResult, Vec<(String, Value)>) {
        let mut asked = Vec::new();
        let mut logs = Some(logs);
        let answer = find_op_event(&search(from_block), |method, params| {
            asked.push((method.to_owned(), params));
            match method {
                "eth_blockNumber" => Ok(json!({ "result": "0x2e3b5dc" })),
                _ => logs.take().unwrap_or(Err(PoolError::Unavailable)),
            }
        });
        (answer, asked)
    }

    fn op_event(answer: &TrackShellResult) -> (Option<&str>, Option<&str>, Option<u64>) {
        match answer {
            TrackShellResult::OpEvent {
                logs_json,
                error_json,
                head_block,
                ..
            } => (logs_json.as_deref(), error_json.as_deref(), *head_block),
            other => unreachable!("not an OpEvent: {other:?}"),
        }
    }

    /// Found — success or failure alike: the logs go back as they came, the
    /// filter names the EntryPoint, the event topic and the op's hash.
    #[test]
    fn a_found_event_is_answered_as_it_came() {
        for success in ["0x1", "0x0"] {
            let log = json!({ "transactionHash": "0xtx", "data": success, "topics": [] });
            let (answer, asked) =
                run_search(Some(48_479_000), Ok(json!({ "result": [log.clone()] })));
            let (logs, error, head) = op_event(&answer);
            assert_eq!(logs, Some(json!([log]).to_string().as_str()));
            assert_eq!(error, None);
            assert_eq!(head, Some(0x2e3b5dc));
            let filter = &asked[1].1[0];
            assert_eq!(filter["address"], vela_core::safe::ENTRY_POINT);
            assert_eq!(
                filter["topics"][0],
                vela_core::user_op::USER_OPERATION_EVENT_TOPIC
            );
            assert_eq!(filter["topics"][1], search(None).user_op_hash);
            assert_eq!(filter["fromBlock"], "0x2e3bb18");
            assert_eq!(filter["toBlock"], "0x2e3beff");
        }
    }

    /// A range limit reaches the core as the endpoint's own error member —
    /// through the pool's `RangeCap` or as a plain JSON error — and nothing
    /// here decides it is one.
    #[test]
    fn a_range_error_goes_to_the_core_as_it_came() {
        let words = json!({"code": -32005, "message": "query exceeds max block range 1000"});
        let (answer, _) = run_search(
            Some(1),
            Err(PoolError::RangeCap {
                max_span: 1000.0,
                error_json: Some(words.to_string()),
            }),
        );
        assert_eq!(
            op_event(&answer),
            (None, Some(words.to_string().as_str()), Some(0x2e3b5dc))
        );
        let (answer, _) = run_search(Some(1), Ok(json!({ "error": words.clone() })));
        assert_eq!(op_event(&answer).1, Some(words.to_string().as_str()));
        // No answer at all is neither.
        let (answer, _) = run_search(
            Some(1),
            Err(PoolError::Failed {
                rate_limited: false,
            }),
        );
        assert_eq!(op_event(&answer), (None, None, Some(0x2e3b5dc)));
    }

    /// The core asks for the head alone first when it does not know where
    /// the op could start: no logs are read.
    #[test]
    fn a_head_only_search_reads_no_logs() {
        let (answer, asked) = run_search(None, Ok(json!({ "result": [] })));
        assert_eq!(op_event(&answer), (None, None, Some(0x2e3b5dc)));
        assert_eq!(asked.len(), 1);
        assert_eq!(asked[0].0, "eth_blockNumber");
    }

    /// Spec 082 RJ4: the bundle transaction's receipt goes back as it came —
    /// the JSON-RPC `result`, `null` while not mined — and an error member or
    /// no answer at all is no answer.
    #[test]
    fn a_tx_receipt_goes_back_as_it_came() {
        let answer = |body: Result<Value, PoolError>| {
            let mut asked = None;
            let result = tx_receipt(100, "0xtx", "0xop", |method, params| {
                asked = Some((method.to_owned(), params));
                body
            });
            let Some((method, params)) = asked else {
                unreachable!("nothing was asked");
            };
            assert_eq!(method, "eth_getTransactionReceipt");
            assert_eq!(params, json!(["0xtx"]));
            match result {
                TrackShellResult::TxReceipt {
                    user_op_hash,
                    receipt_json,
                    ..
                } => {
                    assert_eq!(user_op_hash, "0xop");
                    receipt_json
                }
                other => unreachable!("not a TxReceipt: {other:?}"),
            }
        };
        let receipt = json!({ "transactionHash": "0xtx", "logs": [] });
        assert_eq!(
            answer(Ok(json!({ "result": receipt.clone() }))),
            Some(receipt.to_string())
        );
        assert_eq!(
            answer(Ok(json!({ "result": null }))).as_deref(),
            Some("null"),
            "not mined yet"
        );
        assert_eq!(answer(Ok(json!({ "error": { "code": -32000 } }))), None);
        assert_eq!(answer(Err(PoolError::Unavailable)), None);
    }

    /// Spec 082 RJ4 (G38, EX13): the relay's receipt never came (it said
    /// `null` for minutes) while its status said `included` with the bundle's
    /// tx hash — the op read "not landed yet" for 5 min 49 s. Now that tx's
    /// receipt is read from the chain on the next pass, and the op is
    /// confirmed from its own `UserOperationEvent` there, within one poll.
    #[test]
    fn an_included_op_is_confirmed_from_its_bundle_within_one_poll() {
        use crate::core_host::CoreHost;
        use vela_core::app::tx_tracker::{TrackLifecycle, TrackStatus};

        const OP: &str = "0x9da32d527e0000000000000000000000000000000000000000000000000000aa";
        const TX: &str = "0x111ce047000000000000000000000000000000000000000000000000000000bb";
        let event_log = |hash: &str| {
            json!({
                "address": vela_core::safe::ENTRY_POINT,
                "topics": [
                    vela_core::user_op::USER_OPERATION_EVENT_TOPIC,
                    hash,
                    format!("0x{:0>64}", "88cca0eedbf2c4426110bbfc998f048689266894"),
                    format!("0x{:0>64}", "0"),
                ],
                "data": format!("0x{:064x}{:064x}{:064x}{:064x}", 44, 1, 0, 90_000),
                "transactionHash": TX,
                "removed": false,
            })
        };
        let receipt = json!({
            "transactionHash": TX,
            "status": "0x1",
            // Another sender's op first in the bundle, then this one.
            "logs": [event_log("0xother"), event_log(OP)],
        });

        let mut host = CoreHost::<TxTracker>::new();
        let mut now = 1_757_000_000_000.0;
        let mut pending = host.dispatch(Event::Submitted {
            user_op_hash: OP.to_owned(),
            record_ids: vec!["dapp-1-tx".to_owned()],
            chain_id: 100,
            maybe_sent: false,
            submit_block: Some(48_487_259),
            admitted: true,
        });
        let mut receipts_by_tx = 0;
        let mut ticks = 0..5;
        for _ in 0..64 {
            let Some(next) = pending.pop() else {
                if ticks.next().is_none() {
                    break;
                }
                now += 3_000.0;
                pending = host.dispatch(Event::Tick);
                continue;
            };
            let result = match &next.operation {
                TrackOperation::Now => TrackShellResult::Clock { now_ms: now },
                // The relay has no receipt for it…
                TrackOperation::PollReceipt { user_op_hash, .. } => {
                    TrackShellResult::ReceiptPending {
                        user_op_hash: user_op_hash.clone(),
                        now_ms: now,
                    }
                }
                // …while its status names the bundle.
                TrackOperation::PollStatus { user_op_hash, .. } => TrackShellResult::Status {
                    user_op_hash: user_op_hash.clone(),
                    status: TrackLifecycle::Included,
                    stage: None,
                    now_ms: now,
                    tx_hash: Some(TX.to_owned()),
                },
                TrackOperation::TxReceipt {
                    chain_id,
                    tx_hash,
                    user_op_hash,
                } => {
                    receipts_by_tx += 1;
                    assert_eq!(tx_hash, TX);
                    tx_receipt(*chain_id, tx_hash, user_op_hash, |_, _| {
                        Ok(json!({ "result": receipt.clone() }))
                    })
                }
                TrackOperation::UpdateTxRecords { .. } => TrackShellResult::RecordsPatched,
                TrackOperation::NotifyConfirmed { .. } | TrackOperation::HoldingsMoved { .. } => {
                    TrackShellResult::Notified
                }
                TrackOperation::LoadPendingTxs => TrackShellResult::RecordsLoaded {
                    records: Vec::new(),
                    now_ms: now,
                },
                other => unreachable!("not asked here: {other:?}"),
            };
            pending.extend(host.resolve(next.id, result));
            let view = host.view();
            if view
                .entries
                .iter()
                .any(|entry| entry.status == TrackStatus::Confirmed)
            {
                break;
            }
        }
        let entry = host
            .view()
            .entries
            .into_iter()
            .find(|entry| entry.user_op_hash == OP)
            .unwrap_or_else(|| unreachable!("the op is tracked"));
        assert_eq!(entry.status, TrackStatus::Confirmed);
        assert_eq!(entry.tx_hash.as_deref(), Some(TX));
        assert_eq!(receipts_by_tx, 1, "one read of the bundle's receipt");
    }

    /// Spec 082 RJ1: a written-ahead op proven never sent is withdrawn — the
    /// tracker forgets it and patches nothing. The write-ahead's hand-off
    /// names no record (the POST's verdict names them, 082 second review).
    #[test]
    fn a_withdrawn_op_is_forgotten() {
        use crate::core_host::CoreHost;
        let mut host = CoreHost::<TxTracker>::new();
        let _ = host.dispatch(Event::Submitted {
            user_op_hash: "0xop".to_owned(),
            record_ids: Vec::new(),
            chain_id: 100,
            maybe_sent: true,
            submit_block: None,
            admitted: false,
        });
        assert_eq!(host.view().entries.len(), 1);
        let after = host.dispatch(Event::Withdrawn {
            user_op_hash: "0xop".to_owned(),
            record_ids: vec!["r1".to_owned()],
        });
        assert!(host.view().entries.is_empty());
        assert!(
            !after
                .iter()
                .any(|op| matches!(op.operation, TrackOperation::UpdateTxRecords { .. })),
            "a withdrawal patches no record"
        );
    }

    /// RE8: holdings moved → the balances are invalidated, once, and the core
    /// is answered `Notified`.
    #[test]
    fn holdings_moved_invalidates_the_balances_once() {
        let _turn = crate::executor::balance_dashboard::invalidation_turn();
        let _ = crate::executor::balance_dashboard::take_invalidation();
        assert!(matches!(
            TxTracker::perform(&TrackOperation::HoldingsMoved { chain_id: 100 }),
            Answer::Now(TrackShellResult::Notified)
        ));
        assert!(crate::executor::balance_dashboard::take_invalidation());
        assert!(!crate::executor::balance_dashboard::take_invalidation());
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
