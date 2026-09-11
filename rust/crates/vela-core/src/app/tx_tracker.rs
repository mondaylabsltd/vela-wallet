//! Machine — post-submit transaction lifecycle / reconciliation (spec
//! `016-crux-wallet-state`, tx_tracker).
//!
//! ```text
//! Submitted ─► entry{Pending} ─► 3s receipt polls + 12s status polls (120s window)
//!                  │                  │ receipt.success        │ receipt.failed / status=rejected
//!                  │                  ▼                        ▼
//!                  │              Confirmed{tx_hash}       Failed / Rejected  (the ONLY failure paths)
//!                  │ window ends with no receipt
//!                  ▼
//!            FeeHeld | Unreachable | AcceptedNotLanded ─► 12s reconcile polls ─► 24h: stop, stay pending
//! ```
//!
//! One machine replaces three concurrent pollers — `waitForReceipt`
//! (`safe-transaction.ts:2185-2332`), the reconciler (`tx-reconciler.ts`) and
//! the receipt sheet's self-poll (`TransactionReceipt.tsx:593-629`) — plus the
//! dApp startup recovery scan (`dapp-connection.tsx:1029-1048`). The rules are
//! all about never lying about money in flight:
//!
//! - A timeout or an unreachable bundler is NEVER a failure. The op may still
//!   land; marking it failed invites a re-send and a double spend
//!   (`safe-transaction.ts:2185-2332`, `useSendController.ts:1000-1004`).
//! - Only a definitive receipt with `success === false` (dropped/reverted) or
//!   an explicit relay `rejected` status may mark records failed — and either
//!   one terminates tracking immediately (`tx-reconciler.ts:205-252`).
//! - A fee-hold (`queued` at the `in_band_settlement_hold` stage) is a
//!   *waiting* outcome: the relay sends the op itself when fees settle, so the
//!   record stays pending and only the wording changes
//!   (`UserOpFeeHoldError`, `useSendController.ts:1051-1058`).
//! - "Unreachable the whole window" is honestly distinct from "the bundler
//!   answered but the op has not landed" — the `net.ts` timeout/aborted/network
//!   classification collapsed into one typed axis (`sawCleanResponse` vs
//!   `rpcFailures` in `waitForReceipt`).
//! - Past 24h the machine stops polling but the record stays pending — an
//!   honest "unknown", never a fabricated failure (`tx-reconciler.ts:16-23`).
//!
//! The shell owns the regex wording layer that used to *be* the classification
//! (`/dropped from the network/`, `UserOpRejectedError` instanceof checks):
//! it maps RPC answers to the typed results below, and this core owns every
//! throttle, deadline and verdict. Time never originates here — every result
//! carries `now_ms` (the 011 `now_iso` pattern), and cadence is driven by
//! shell `Tick`s that the core throttles.

use std::collections::BTreeMap;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// Shared per-hash receipt throttle — `USER_OP_RECEIPT_POLL_INTERVAL_MS`
/// (`tx-reconciler.ts:36`). Every consumer of the same hash shares one
/// in-flight request and one 3s cooldown, so opening the receipt sheet never
/// doubles `eth_getUserOperationReceipt` traffic (`safe-transaction.ts:2237-2240`).
pub const RECEIPT_POLL_INTERVAL_MS: f64 = 3_000.0;
/// Relay lifecycle-status cadence while the wait window is open —
/// `USER_OP_STATUS_POLL_INTERVAL_MS` (`safe-transaction.ts`). The first status
/// poll waits one full interval: "not ready yet" is by far the common case.
pub const STATUS_POLL_INTERVAL_MS: f64 = 12_000.0;
/// Reconcile sweep throttle — `MIN_INTERVAL_MS` (`tx-reconciler.ts:32`). Home
/// focus + interval call it a lot; also the receipt cadence once the wait
/// window has closed.
pub const RECONCILE_MIN_INTERVAL_MS: f64 = 12_000.0;
/// The active wait window — `waitForReceipt`'s default `timeout` of 120s.
pub const WAIT_WINDOW_MS: f64 = 120_000.0;
/// Stop polling past this age — `RECONCILE_MAX_AGE_MS` (`tx-reconciler.ts:30`).
/// The bundler has likely pruned the receipt; hammering it forever helps no
/// one, and the record stays pending (honest unknown), never failed.
pub const ABANDON_AGE_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// The executor stage that parks an op until network fees fit its signed
/// reimbursement — `FEE_HOLD_STAGE` (`tx-reconciler.ts:84`).
pub const FEE_HOLD_STAGE: &str = "in_band_settlement_hold";

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. The shell performs the RPC and
/// maps the answer to a typed [`TrackShellResult`] — every message-regex
/// classification that used to live in three call sites happens exactly once,
/// in that mapping layer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrackOperation"))]
pub enum TrackOperation {
    /// `eth_getUserOperationReceipt`. Mapping (`tx-reconciler.ts:140-186`):
    /// RPC error / thrown → `ReceiptUnreachable`; no result / no txHash →
    /// `ReceiptPending`; `success !== false` → `Receipt`; `success === false`
    /// → `ReceiptFailed`.
    PollReceipt { user_op_hash: String, chain_id: u32 },
    /// `eth_getUserOperationStatus` (a Vela relay extension). Null / error /
    /// older relay → `StatusUnavailable` (`tx-reconciler.ts:96-115`).
    PollStatus { user_op_hash: String, chain_id: u32 },
    /// Load still-pending submissions from storage: records with
    /// `status === 'pending'`, a `userOpHash` and `txHash === ''`
    /// (`tx-reconciler.ts:217-224`; the dApp scan's `dapp_tx` filter,
    /// `dapp-connection.tsx:1038-1040`). A load failure answers an empty
    /// list, exactly as `loadTransactions().catch(() => [])` does.
    LoadPendingTxs,
    /// Patch the given records in ONE atomic batch write
    /// (`storage.ts updateTransactions`) — same ids, in place, never a
    /// second record.
    UpdateTxRecords {
        ids: Vec<String>,
        patch: TrackRecordPatch,
    },
    /// A confirmation landed — the shell forwards the AUTHENTIC receipt logs
    /// (which it just polled) to token_trust as `ReceiptLogsConfirmed`, the
    /// single auto-add entry point (`tx-reconciler.ts:238-240`).
    NotifyConfirmed {
        user_op_hash: String,
        chain_id: u32,
        tx_hash: String,
    },
    /// Read the clock. The core owns every cadence decision but no clock —
    /// each `Tick`/resume asks, and the answer drives one scheduler pass.
    Now,
}

/// What the shell observed. Every time-bearing variant carries `now_ms`
/// (epoch milliseconds, f64) — the core is a pure function of its inputs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrackShellResult"))]
pub enum TrackShellResult {
    Clock {
        now_ms: f64,
    },
    /// A definitive successful receipt (`success !== false`, txHash present).
    Receipt {
        user_op_hash: String,
        tx_hash: String,
        now_ms: f64,
    },
    /// A definitive successful receipt WITH its authentic logs (spec 038 Part
    /// D, #D1). Additive: shells that can read the logs send this instead of
    /// [`Self::Receipt`], and the core decides whether the Safe inside the
    /// UserOp actually executed — `ExecutionFailure` in the logs means the
    /// EntryPoint counted the op a success while the payment did not happen,
    /// and the record must say `failed`, not `confirmed`.
    ReceiptWithLogs {
        user_op_hash: String,
        tx_hash: String,
        now_ms: f64,
        logs: Vec<super::token_trust::TrustReceiptLog>,
    },
    /// A definitive failed receipt (`success === false`) — the op was dropped
    /// or reverted on-chain. The one receipt shape that may mark failure.
    ReceiptFailed {
        user_op_hash: String,
        tx_hash: String,
        now_ms: f64,
    },
    /// The bundler answered cleanly but the op has not landed yet
    /// (`reachedBundler: true, resolution: null`).
    ReceiptPending {
        user_op_hash: String,
        now_ms: f64,
    },
    /// The bundler could not answer — timeout / network / RPC error
    /// (`reachedBundler: false`; `net.ts` classification). NOT a failure.
    ReceiptUnreachable {
        user_op_hash: String,
        now_ms: f64,
    },
    /// The relay's view of an op with no receipt (`eth_getUserOperationStatus`).
    Status {
        user_op_hash: String,
        status: TrackLifecycle,
        /// Executor stage that last touched the op, e.g.
        /// `in_band_settlement_hold`.
        stage: Option<String>,
        now_ms: f64,
    },
    /// The status endpoint yielded nothing (unreachable or an older relay).
    StatusUnavailable {
        user_op_hash: String,
        now_ms: f64,
    },
    RecordsLoaded {
        records: Vec<TrackPendingRecord>,
        now_ms: f64,
    },
    RecordsPatched,
    Notified,
}

impl Operation for TrackOperation {
    type Output = TrackShellResult;
}

#[effect]
pub enum TrackEffect {
    Render(RenderOperation),
    Shell(TrackOperation),
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// `UserOpLifecycle` (`tx-reconciler.ts:66-73`), verbatim. Ported quirk: only
/// `rejected` is ever acted on — `included`/`failed` from this endpoint are
/// recorded but never terminate the wait, exactly as `waitForReceipt` does
/// (ported verbatim, see inventory open questions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrackLifecycle {
    NotFound,
    Queued,
    NotSubmitted,
    Submitted,
    Rejected,
    Included,
    Failed,
}

/// One still-pending stored submission, as the shell maps it from
/// `LocalTransaction` (`timestamp` seconds → `submitted_at_ms`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackPendingRecord {
    pub record_id: String,
    pub user_op_hash: String,
    pub chain_id: u32,
    pub submitted_at_ms: f64,
}

/// Storage vocabulary is `pending | confirmed | failed` — a relay rejection
/// is persisted as `failed` (as `useSendController.ts:1060-1068` does); the
/// view keeps the honest distinction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrackRecordStatus {
    Confirmed,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackRecordPatch {
    pub status: TrackRecordStatus,
    /// Present only on confirmation — a failed patch never writes a hash,
    /// matching `updateTransaction(tx.id, { status: 'failed' })`.
    pub tx_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrackEvent"))]
pub enum Event {
    /// A UserOp was just accepted by the bundler (send screen or dApp sheet).
    /// `record_ids` are the already-persisted pending records for this hash —
    /// one per batch recipient (`<hash>-<i>`), all patched together later.
    /// A resubmitted op shares its hash and merges into the same entry, the
    /// core-side twin of storage's de-dupe by id (`storage.ts:445-448`).
    Submitted {
        user_op_hash: String,
        record_ids: Vec<String>,
        chain_id: u32,
    },
    /// The shell's cadence timer. Any frequency is safe — the core enforces
    /// every throttle, so a chatty shell can never double-poll the bundler.
    Tick,
    /// App came to the foreground — run a reconcile sweep (12s-throttled).
    AppResumed,
    /// Home gained focus — same sweep trigger as today's
    /// `reconcilePendingTransactions` call sites.
    HomeFocused,
    /// The surface waiting on this hash went away (screen unmount / user
    /// cancelled — `signal?.aborted` in `waitForReceipt`). Tracking continues
    /// at the reconcile cadence; a late receipt still confirms. Never a
    /// failure.
    Abort { user_op_hash: String },
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made. This machine never abandons a run — a receipt is
    /// a fact about the op, not about a UI session — so staleness is enforced
    /// per hash instead: results for terminal or unknown hashes are dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: TrackShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum EntryStatus {
    Pending,
    /// Relay parked the op until fees settle — still pending, new wording.
    FeeHeld,
    Confirmed {
        tx_hash: String,
    },
    /// Dropped from the network / reverted (`success === false`).
    Failed {
        tx_hash: String,
    },
    /// The relay refused it before any block — nothing was sent.
    Rejected,
    /// The bundler was unreachable for the whole wait window: the op's fate
    /// is genuinely unknown, which is NOT the same as "pending".
    Unknown,
}

impl EntryStatus {
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntryStatus::Confirmed { .. } | EntryStatus::Failed { .. } | EntryStatus::Rejected
        )
    }
}

#[derive(Clone, Debug)]
struct Entry {
    chain_id: u32,
    /// Stored-record ids patched on resolution — in place, same ids, never a
    /// second record (`dapp-history` rule; batch siblings patched together).
    record_ids: Vec<String>,
    status: EntryStatus,
    /// Stamped from the first observed clock after `Submitted` (or carried by
    /// the recovered record). Every deadline measures from it.
    submitted_at_ms: Option<f64>,
    /// Completion time of the last receipt poll — the 3s cooldown counts
    /// from completion, exactly as `completedAt` does (`tx-reconciler.ts:184`).
    last_receipt_poll_ms: Option<f64>,
    /// Issue time of the last status poll (`lastStatusAt` is stamped before
    /// the call, and starts at `start` so the first poll waits a full 12s).
    last_status_poll_ms: Option<f64>,
    /// One in-flight receipt request per hash, shared by every consumer —
    /// the coalescing half of invariant ⑤.
    receipt_in_flight: bool,
    status_in_flight: bool,
    /// Did the bundler ever answer cleanly this window? Drives the honest
    /// unreachable-vs-not-landed distinction at the window's end.
    saw_clean_response: bool,
    rpc_failures: u32,
    /// The relay's last lifecycle answer — `isFeeHold(lastStatus)` at the
    /// window end is what turns a timeout into a fee-hold.
    last_status: Option<(TrackLifecycle, Option<String>)>,
    /// The 120s wait window ended (and was classified). Receipt polls drop
    /// to the reconcile cadence; status polls stop.
    window_closed: bool,
    /// `Abort` was received: no window classification, reconcile cadence.
    aborted: bool,
    /// Older than 24h — polls stopped for good, record left pending.
    abandoned: bool,
}

impl Entry {
    fn new(chain_id: u32, submitted_at_ms: Option<f64>) -> Self {
        Entry {
            chain_id,
            record_ids: Vec::new(),
            status: EntryStatus::Pending,
            submitted_at_ms,
            last_receipt_poll_ms: None,
            last_status_poll_ms: submitted_at_ms,
            receipt_in_flight: false,
            status_in_flight: false,
            saw_clean_response: false,
            rpc_failures: 0,
            last_status: None,
            window_closed: false,
            aborted: false,
            abandoned: false,
        }
    }

    fn merge_record_id(&mut self, id: String) {
        if !self.record_ids.contains(&id) {
            self.record_ids.push(id);
        }
    }

    /// `isFeeHold` (`tx-reconciler.ts:87-89`): a deliberate wait for cheaper
    /// gas, not a stall and not a failure.
    fn fee_held(&self) -> bool {
        matches!(
            &self.last_status,
            Some((TrackLifecycle::Queued, Some(stage))) if stage == FEE_HOLD_STAGE
        )
    }
}

#[derive(Default)]
pub struct Model {
    /// Keyed by lowercased hash — the shared-throttle key lowercases too
    /// (`receiptPollKey`, `tx-reconciler.ts:125-127`), so two consumers who
    /// case the hash differently still share one request. (Ported with the
    /// key narrowed from `chainId:hash` to the hash alone — the chain rides
    /// in the entry; see inventory tx_tracker Model.)
    entries: BTreeMap<String, Entry>,
    /// Sweep single-flight + 12s throttle (`_running` / `_lastRunAt`).
    last_reconcile_ms: Option<f64>,
    reconcile_in_flight: bool,
    /// A resume/focus asked for a sweep; consumed by the next clock reading.
    reconcile_requested: bool,
    /// Captured into every request. Never bumped: no event abandons the whole
    /// tracking session (see [`Event::ShellCompleted`]); per-hash guards do
    /// the staleness work.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// The six-way verdict from the machine's scope, plus plain `Pending`. i18n
/// keys and wording live in the shell — this is the semantic axis only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrackStatus {
    /// Inside the wait window (or aborted): confirming.
    Pending,
    /// Queued until network fees settle — the relay sends it itself.
    FeeHeld,
    Confirmed,
    /// Dropped from the network / reverted. Terminal.
    Dropped,
    /// The relay refused it; nothing was sent. Terminal.
    Rejected,
    /// Bundler unreachable all window — fate unknown, check the explorer.
    Unreachable,
    /// The bundler accepted it but produced no receipt in the window — it
    /// may still land.
    AcceptedNotLanded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackEntryView {
    pub user_op_hash: String,
    pub chain_id: u32,
    pub record_ids: Vec<String>,
    pub status: TrackStatus,
    /// Present once a definitive receipt named it — for confirmations AND
    /// drops (the receipt sheet links the explorer either way,
    /// `TransactionReceipt.tsx:619-621`).
    pub tx_hash: Option<String>,
    /// False once terminal or abandoned (24h) — drives "check the explorer".
    pub polling: bool,
    pub submitted_at_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackView {
    /// Newest first.
    pub entries: Vec<TrackEntryView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TxTracker;

impl App for TxTracker {
    type Event = Event;
    type Model = Model;
    type ViewModel = TrackView;
    type Effect = TrackEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<TrackEffect, Event> {
        match event {
            Event::Submitted {
                user_op_hash,
                record_ids,
                chain_id,
            } => submitted(model, &user_op_hash, record_ids, chain_id),
            Event::Tick => {
                // Inert unless something still needs the clock — a tracker
                // with only terminal/abandoned entries makes no requests.
                let live = model
                    .entries
                    .values()
                    .any(|entry| !entry.status.is_terminal() && !entry.abandoned);
                if !live {
                    return Command::done();
                }
                shell_request(model.attempt, TrackOperation::Now)
            }
            Event::AppResumed | Event::HomeFocused => {
                model.reconcile_requested = true;
                shell_request(model.attempt, TrackOperation::Now)
            }
            Event::Abort { user_op_hash } => {
                let key = normalize(&user_op_hash);
                if let Some(entry) = model.entries.get_mut(&key) {
                    if !entry.status.is_terminal() {
                        entry.aborted = true;
                    }
                }
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> TrackView {
        let mut entries: Vec<TrackEntryView> = model
            .entries
            .iter()
            .map(|(hash, entry)| {
                let status = match &entry.status {
                    EntryStatus::Pending if entry.window_closed => TrackStatus::AcceptedNotLanded,
                    EntryStatus::Pending => TrackStatus::Pending,
                    EntryStatus::FeeHeld => TrackStatus::FeeHeld,
                    EntryStatus::Confirmed { .. } => TrackStatus::Confirmed,
                    EntryStatus::Failed { .. } => TrackStatus::Dropped,
                    EntryStatus::Rejected => TrackStatus::Rejected,
                    EntryStatus::Unknown => TrackStatus::Unreachable,
                };
                let tx_hash = match &entry.status {
                    EntryStatus::Confirmed { tx_hash } | EntryStatus::Failed { tx_hash } => {
                        Some(tx_hash.clone())
                    }
                    _ => None,
                };
                TrackEntryView {
                    user_op_hash: hash.clone(),
                    chain_id: entry.chain_id,
                    record_ids: entry.record_ids.clone(),
                    status,
                    tx_hash,
                    polling: !entry.status.is_terminal() && !entry.abandoned,
                    submitted_at_ms: entry.submitted_at_ms,
                }
            })
            .collect();
        entries.sort_by(|a, b| {
            let ta = a.submitted_at_ms.unwrap_or(f64::INFINITY);
            let tb = b.submitted_at_ms.unwrap_or(f64::INFINITY);
            tb.total_cmp(&ta)
                .then_with(|| a.user_op_hash.cmp(&b.user_op_hash))
        });
        TrackView { entries }
    }
}

// ---------------------------------------------------------------------------
// User/shell-initiated transitions
// ---------------------------------------------------------------------------

fn submitted(
    model: &mut Model,
    user_op_hash: &str,
    record_ids: Vec<String>,
    chain_id: u32,
) -> Command<TrackEffect, Event> {
    let key = normalize(user_op_hash);
    let attempt = model.attempt;
    let entry = model
        .entries
        .entry(key.clone())
        .or_insert_with(|| Entry::new(chain_id, None));
    for id in record_ids {
        entry.merge_record_id(id);
    }

    // First receipt poll goes out immediately, like `waitForReceipt`'s first
    // loop iteration. A second consumer of an already-tracked hash joins the
    // shared request/cooldown instead (invariant ⑤) — its polls resume on
    // the next Tick under the 3s throttle.
    let poll_now = !entry.status.is_terminal()
        && !entry.receipt_in_flight
        && entry.last_receipt_poll_ms.is_none();
    if poll_now {
        entry.receipt_in_flight = true;
    }

    let mut commands = vec![shell_request(attempt, TrackOperation::Now)];
    if poll_now {
        commands.push(shell_request(
            attempt,
            TrackOperation::PollReceipt {
                user_op_hash: key,
                chain_id,
            },
        ));
    }
    commands.push(render());
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: TrackShellResult) -> Command<TrackEffect, Event> {
    // Any clock-bearing result stamps entries created before the machine knew
    // the time (the `Now` issued at `Submitted` answers within a beat).
    if let Some(now_ms) = clock_of(&result) {
        stamp_unstamped(model, now_ms);
    }

    match result {
        TrackShellResult::Clock { now_ms } => run_scheduler(model, now_ms),

        // -- definitive receipts ---------------------------------------------
        TrackShellResult::ReceiptWithLogs {
            user_op_hash,
            tx_hash,
            now_ms,
            logs,
        } => {
            // One rule, then the ordinary path: a Safe `ExecutionFailure`
            // inside a "successful" UserOp is a failed payment (#D1).
            let result = if safe_execution_failed(&logs) {
                TrackShellResult::ReceiptFailed {
                    user_op_hash,
                    tx_hash,
                    now_ms,
                }
            } else {
                TrackShellResult::Receipt {
                    user_op_hash,
                    tx_hash,
                    now_ms,
                }
            };
            accept(model, result)
        }
        TrackShellResult::Receipt {
            user_op_hash,
            tx_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            let Some(entry) = model.entries.get_mut(&key) else {
                return Command::done();
            };
            entry.receipt_in_flight = false;
            entry.last_receipt_poll_ms = Some(now_ms);
            entry.saw_clean_response = true;
            if entry.status.is_terminal() {
                // Already resolved by another path — never double-resolve.
                return Command::done();
            }
            entry.status = EntryStatus::Confirmed {
                tx_hash: tx_hash.clone(),
            };
            let ids = entry.record_ids.clone();
            let chain_id = entry.chain_id;
            let attempt = model.attempt;
            // Patch first, notify second — the reconciler's order
            // (`tx-reconciler.ts:236-240`): flip the records, then hand the
            // authentic logs to token_trust.
            Command::all([
                shell_request(
                    attempt,
                    TrackOperation::UpdateTxRecords {
                        ids,
                        patch: TrackRecordPatch {
                            status: TrackRecordStatus::Confirmed,
                            tx_hash: Some(tx_hash.clone()),
                        },
                    },
                ),
                shell_request(
                    attempt,
                    TrackOperation::NotifyConfirmed {
                        user_op_hash: key,
                        chain_id,
                        tx_hash,
                    },
                ),
                render(),
            ])
        }
        TrackShellResult::ReceiptFailed {
            user_op_hash,
            tx_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            let Some(entry) = model.entries.get_mut(&key) else {
                return Command::done();
            };
            entry.receipt_in_flight = false;
            entry.last_receipt_poll_ms = Some(now_ms);
            entry.saw_clean_response = true;
            if entry.status.is_terminal() {
                return Command::done();
            }
            // "Dropped from the network" — the one receipt shape that may
            // fail records, and it terminates tracking immediately (③).
            entry.status = EntryStatus::Failed { tx_hash };
            let ids = entry.record_ids.clone();
            fail_records(model.attempt, ids)
        }

        // -- non-answers ------------------------------------------------------
        TrackShellResult::ReceiptPending {
            user_op_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            if let Some(entry) = model.entries.get_mut(&key) {
                entry.receipt_in_flight = false;
                entry.last_receipt_poll_ms = Some(now_ms);
                entry.saw_clean_response = true;
            }
            // Mirror the `waitForReceipt` loop: right after a receipt
            // attempt is when the 12s status check runs.
            run_scheduler(model, now_ms)
        }
        TrackShellResult::ReceiptUnreachable {
            user_op_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            if let Some(entry) = model.entries.get_mut(&key) {
                entry.receipt_in_flight = false;
                entry.last_receipt_poll_ms = Some(now_ms);
                // NOT a failure (①) — counted so the window's end can be
                // honest about never having reached the bundler (⑧).
                entry.rpc_failures = entry.rpc_failures.saturating_add(1);
            }
            run_scheduler(model, now_ms)
        }

        // -- relay lifecycle --------------------------------------------------
        TrackShellResult::Status {
            user_op_hash,
            status,
            stage,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            let rejected_ids = {
                let Some(entry) = model.entries.get_mut(&key) else {
                    return Command::done();
                };
                entry.status_in_flight = false;
                if entry.status.is_terminal() {
                    // e.g. the receipt confirmed while this poll was in
                    // flight — a late "rejected" must never un-confirm.
                    return Command::done();
                }
                entry.last_status = Some((status, stage));
                if status == TrackLifecycle::Rejected {
                    // The relay refused it before any block: nothing was
                    // sent, nothing will land. Terminal, immediately (③).
                    entry.status = EntryStatus::Rejected;
                    Some(entry.record_ids.clone())
                } else {
                    // Everything else — including `included`/`failed` — is
                    // recorded only, verbatim from `waitForReceipt`.
                    None
                }
            };
            match rejected_ids {
                Some(ids) => fail_records(model.attempt, ids),
                None => run_scheduler(model, now_ms),
            }
        }
        TrackShellResult::StatusUnavailable {
            user_op_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            if let Some(entry) = model.entries.get_mut(&key) {
                entry.status_in_flight = false;
            }
            run_scheduler(model, now_ms)
        }

        // -- reconcile sweep --------------------------------------------------
        TrackShellResult::RecordsLoaded { records, now_ms } => {
            model.reconcile_in_flight = false;
            for record in records {
                if record.user_op_hash.is_empty() {
                    continue;
                }
                // The 24h line is the core's rule even if the shell forgot to
                // filter (④): too old to poll, left pending in storage.
                if now_ms - record.submitted_at_ms >= ABANDON_AGE_MS {
                    continue;
                }
                let key = normalize(&record.user_op_hash);
                let entry = model
                    .entries
                    .entry(key)
                    .or_insert_with(|| Entry::new(record.chain_id, Some(record.submitted_at_ms)));
                // Same hash ⇒ same entry, merged ids — recovery can never
                // fork a second tracking line for a live submission (⑦),
                // and a record whose entry is already terminal is not
                // resurrected (the patch simply hasn't landed yet).
                entry.merge_record_id(record.record_id);
            }
            run_scheduler(model, now_ms)
        }

        // Acks — nothing may change.
        TrackShellResult::RecordsPatched | TrackShellResult::Notified => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Scheduler — one pass per clock reading
// ---------------------------------------------------------------------------

/// The unified cadence policy. Runs on every clock-bearing answer, mirroring
/// the `waitForReceipt` loop body: attempt/complete a receipt poll, then the
/// 12s status check, then classification when the window has elapsed.
fn run_scheduler(model: &mut Model, now_ms: f64) -> Command<TrackEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<TrackEffect, Event>> = Vec::new();

    // Reconcile sweep: single-flight, 12s apart (`tx-reconciler.ts:205-211`),
    // stamped at run start as `_lastRunAt` is.
    if model.reconcile_requested {
        model.reconcile_requested = false;
        let due = model
            .last_reconcile_ms
            .is_none_or(|last| now_ms - last >= RECONCILE_MIN_INTERVAL_MS);
        if !model.reconcile_in_flight && due {
            model.last_reconcile_ms = Some(now_ms);
            model.reconcile_in_flight = true;
            commands.push(shell_request(attempt, TrackOperation::LoadPendingTxs));
        }
    }

    for (hash, entry) in model.entries.iter_mut() {
        if entry.status.is_terminal() || entry.abandoned {
            continue;
        }
        let submitted_at = entry.submitted_at_ms.unwrap_or(now_ms);
        let age = now_ms - submitted_at;

        // ④ — past 24h: stop polling for good, stay pending. Checked before
        // window classification so a record first seen this old gets no
        // verdict it never earned.
        if age >= ABANDON_AGE_MS {
            entry.abandoned = true;
            continue;
        }

        // The wait window ended without a definitive receipt — classify,
        // exactly in `waitForReceipt`'s order: fee-hold first, then the
        // never-reached-the-bundler case, else accepted-but-not-landed.
        // (Verbatim quirk kept: a window with zero completed polls has
        // `rpc_failures == 0` and lands on accepted-not-landed.)
        if !entry.window_closed && !entry.aborted && age >= WAIT_WINDOW_MS {
            entry.window_closed = true;
            if entry.fee_held() {
                entry.status = EntryStatus::FeeHeld; // ②: pending, reworded
            } else if !entry.saw_clean_response && entry.rpc_failures > 0 {
                entry.status = EntryStatus::Unknown; // ⑧: honest unknown
            }
        }

        // Receipt cadence: 3s inside the window, reconcile pace after it
        // (or after an abort). One in-flight request per hash, shared (⑤).
        let in_window = !entry.window_closed && !entry.aborted;
        let receipt_interval = if in_window {
            RECEIPT_POLL_INTERVAL_MS
        } else {
            RECONCILE_MIN_INTERVAL_MS
        };
        if !entry.receipt_in_flight
            && entry
                .last_receipt_poll_ms
                .is_none_or(|last| now_ms - last >= receipt_interval)
        {
            entry.receipt_in_flight = true;
            commands.push(shell_request(
                attempt,
                TrackOperation::PollReceipt {
                    user_op_hash: hash.clone(),
                    chain_id: entry.chain_id,
                },
            ));
        }

        // Status cadence: every 12s while the window is open, first poll a
        // full interval after submission (the receipt usually needs no
        // second endpoint). Stamped at issue, as `lastStatusAt` is.
        if in_window
            && !entry.status_in_flight
            && entry
                .last_status_poll_ms
                .is_none_or(|last| now_ms - last >= STATUS_POLL_INTERVAL_MS)
        {
            entry.status_in_flight = true;
            entry.last_status_poll_ms = Some(now_ms);
            commands.push(shell_request(
                attempt,
                TrackOperation::PollStatus {
                    user_op_hash: hash.clone(),
                    chain_id: entry.chain_id,
                },
            ));
        }
    }

    commands.push(render());
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The shared-throttle key lowercases the hash (`receiptPollKey`) — so does
/// the entry key, making every consumer meet at one entry.
fn normalize(user_op_hash: &str) -> String {
    user_op_hash.to_lowercase()
}

/// The clock a result carries, if any — acks carry none.
/// `keccak256("ExecutionFailure(bytes32,uint256)")` — the event Safe's
/// `execTransaction` emits when the inner call fails WITHOUT reverting the
/// outer one. Pinned against the core's own keccak by a test below.
pub const SAFE_EXECUTION_FAILURE_TOPIC: &str =
    "0x23428b18acfb3ea64b08dc0c1d296ea9c09702c09083ca5272e64d115b687d23";

/// Did a Safe inside this receipt report an execution failure? (#D1)
///
/// A UserOp's receipt carries only the logs of that op's execution, and the
/// only Safe executing in one of OUR ops is ours — so any log with this
/// topic is the payment not happening, whatever the EntryPoint's `success`
/// says. Case-insensitive on the topic so a checksummed relay cannot hide it.
pub fn safe_execution_failed(logs: &[super::token_trust::TrustReceiptLog]) -> bool {
    logs.iter().any(|log| {
        log.topics
            .first()
            .is_some_and(|topic| topic.eq_ignore_ascii_case(SAFE_EXECUTION_FAILURE_TOPIC))
    })
}

fn clock_of(result: &TrackShellResult) -> Option<f64> {
    match result {
        TrackShellResult::Clock { now_ms }
        | TrackShellResult::Receipt { now_ms, .. }
        | TrackShellResult::ReceiptWithLogs { now_ms, .. }
        | TrackShellResult::ReceiptFailed { now_ms, .. }
        | TrackShellResult::ReceiptPending { now_ms, .. }
        | TrackShellResult::ReceiptUnreachable { now_ms, .. }
        | TrackShellResult::Status { now_ms, .. }
        | TrackShellResult::StatusUnavailable { now_ms, .. }
        | TrackShellResult::RecordsLoaded { now_ms, .. } => Some(*now_ms),
        TrackShellResult::RecordsPatched | TrackShellResult::Notified => None,
    }
}

/// Entries created before any clock was observed get stamped by the first
/// result that carries one; `last_status_poll` starts at submission
/// (`lastStatusAt = start`) so the first status poll waits a full interval.
fn stamp_unstamped(model: &mut Model, now_ms: f64) {
    for entry in model.entries.values_mut() {
        if entry.submitted_at_ms.is_none() {
            entry.submitted_at_ms = Some(now_ms);
            entry.last_status_poll_ms = Some(now_ms);
        }
    }
}

/// The ONLY constructor of a `failed` patch — reachable from exactly two
/// places: a `success === false` receipt and a relay rejection (③). Timeouts,
/// aborts and unreachable bundlers can never arrive here (①).
fn fail_records(attempt: u64, ids: Vec<String>) -> Command<TrackEffect, Event> {
    Command::all([
        shell_request(
            attempt,
            TrackOperation::UpdateTxRecords {
                ids,
                patch: TrackRecordPatch {
                    status: TrackRecordStatus::Failed,
                    tx_hash: None,
                },
            },
        ),
        render(),
    ])
}

/// Issue one operation whose answer must match the current attempt.
fn shell_request(attempt: u64, operation: TrackOperation) -> Command<TrackEffect, Event> {
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

impl super::SplitEffect for TrackEffect {
    type Op = TrackOperation;
    fn into_shell(self) -> Option<crux_core::Request<TrackOperation>> {
        match self {
            TrackEffect::Render(_) => None,
            TrackEffect::Shell(request) => Some(request),
        }
    }
}

#[cfg(test)]
mod execution_failure {
    use super::*;
    use crate::app::token_trust::TrustReceiptLog;

    /// The topic constant is the core's own keccak of the Safe event
    /// signature — a typo here would make every failed payment "confirmed".
    #[test]
    fn the_topic_is_the_keccak_of_the_safe_event() {
        let digest = crate::primitives::keccak256(b"ExecutionFailure(bytes32,uint256)");
        let hex = format!("0x{}", digest.iter().map(|b| format!("{b:02x}")).collect::<String>());
        assert_eq!(hex, SAFE_EXECUTION_FAILURE_TOPIC);
    }

    #[test]
    fn a_failure_log_fails_the_receipt_and_a_transfer_does_not() {
        let transfer = TrustReceiptLog {
            address: "0xtoken".to_owned(),
            topics: vec!["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_owned()],
            data: "0x".to_owned(),
        };
        let failure = TrustReceiptLog {
            address: "0xsafe".to_owned(),
            topics: vec![SAFE_EXECUTION_FAILURE_TOPIC.to_uppercase().replace("0X", "0x")],
            data: "0x".to_owned(),
        };
        assert!(!safe_execution_failed(&[transfer.clone()]));
        assert!(safe_execution_failed(&[transfer, failure]));
        assert!(!safe_execution_failed(&[]));
    }
}
