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
//! Spec 082 — a submit whose reply was lost (G21, owner rulings 1 and 8):
//!
//! ```text
//! Submitted{maybe_sent} ─► MaybeSent ──relay status ≠ not_found / any receipt──► ordinary op
//!     │ status polls past the window     │ its own UserOperationEvent on chain (find-event)
//!     │ + the find-event, same cadence   ▼
//!     │                              Confirmed / Dropped (+ HoldingsMoved)
//!     └─ not_found ×2 past 60 s, and the chain read to its head with no event ─► NotSent (records failed)
//! ```
//!
//! - A may-have-been-sent op is never "try again" while anything could still
//!   show it landed: the relay's word is joined by the chain's before NotSent.
//! - The relay-independent landing check reads the EntryPoint's
//!   `UserOperationEvent` for the op's own hash in bounded windows from the
//!   head read before the submit; the core, not the shell, reads a range
//!   limit (`rpc_pool::is_log_range_error`) and halves the window.
//! - `HoldingsMoved` asks for a balance read only when an op landed.
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
/// Past the window, an op this old is asked about once a minute rather than
/// every [`RECONCILE_MIN_INTERVAL_MS`] (spec 079 FR-005): an op the relay has
/// sat on for ten minutes is not landing in the next twelve seconds, and a
/// phone polling it every few seconds for a day is the waste the device pass
/// logged for two stuck Arbitrum ops.
pub const SLOW_POLL_AFTER_MS: f64 = 10.0 * 60.0 * 1000.0;
/// …and once every five minutes past the first hour.
pub const SLOWEST_POLL_AFTER_MS: f64 = 60.0 * 60.0 * 1000.0;
pub const SLOW_RECEIPT_INTERVAL_MS: f64 = 60_000.0;
pub const SLOWEST_RECEIPT_INTERVAL_MS: f64 = 300_000.0;

/// How long to wait between receipt polls for an op of `age_ms`: the 3 s
/// window cadence while it is open, then a pace that slows with age. Time
/// only ever slows the asking — it never decides the outcome (the module's
/// first rule).
pub fn receipt_interval_ms(in_window: bool, age_ms: f64) -> f64 {
    if in_window {
        RECEIPT_POLL_INTERVAL_MS
    } else if age_ms < SLOW_POLL_AFTER_MS {
        RECONCILE_MIN_INTERVAL_MS
    } else if age_ms < SLOWEST_POLL_AFTER_MS {
        SLOW_RECEIPT_INTERVAL_MS
    } else {
        SLOWEST_RECEIPT_INTERVAL_MS
    }
}
/// The executor stage that parks an op until network fees fit its signed
/// reimbursement — `FEE_HOLD_STAGE` (`tx-reconciler.ts:84`).
pub const FEE_HOLD_STAGE: &str = "in_band_settlement_hold";

/// A relay `not_found` counts only from this age on (spec 082 RA4): before
/// it, the relay may simply not have written the op down yet.
pub const NOT_FOUND_GRACE_MS: f64 = 60_000.0;
/// Consecutive `not_found` answers (each at ≥ [`NOT_FOUND_GRACE_MS`], no
/// receipt, nothing else from the relay in between) that end a may-have-been-
/// sent op as never sent. The relay answers `not_found` only for hashes it
/// never admitted (`relay:wire.rs:597-620`).
pub const NOT_FOUND_CONFIRMATIONS: u32 = 2;

/// The widest `eth_getLogs` window the find-event asks for, in blocks (ruling
/// 8). Alchemy's documented 2K is the strictest common paid cap; stricter
/// public nodes answer a range error and the window halves (dRPC's free tier
/// on Gnosis passes 100 and refuses 500), looser ones never see a wider ask.
pub const FIND_OP_MAX_RANGE: u64 = 2_000;
/// Where the find-event starts below the head when the head before the first
/// submit POST is unknown (ruling 8): ≥ 20 minutes on the fastest chain Vela
/// serves (Arbitrum, 0.25 s blocks), hours elsewhere.
pub const FIND_OP_LOOKBACK_BLOCKS: u64 = 5_000;

/// The relay's lifecycle-status method (spec 082 RA7, G13). The relay serves
/// only this name; the `eth_`-prefixed spelling every client had been asking
/// answers `-32601` there (`evidence/relay-status-probe.txt`).
pub const USER_OP_STATUS_METHOD: &str = "pimlico_getUserOperationStatus";

/// One parsed answer of [`USER_OP_STATUS_METHOD`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackStatusAnswer {
    pub status: TrackLifecycle,
    /// The executor stage that last touched the op (`last_executor_stage`),
    /// e.g. [`FEE_HOLD_STAGE`].
    pub stage: Option<String>,
    /// The bundle transaction the relay names, when it has one — an explorer
    /// link for an op that is still pending (079 D2).
    pub tx_hash: Option<String>,
}

/// Parse the `result` of [`USER_OP_STATUS_METHOD`] — the one parser every
/// client uses (RA7). A whole JSON-RPC body is accepted too (its `result` is
/// read; an `error` is no answer). `None` for anything that is not an object
/// with a known `status` string: an unknown status is not guessed at.
pub fn parse_user_op_status(result_json: &str) -> Option<TrackStatusAnswer> {
    let value: serde_json::Value = serde_json::from_str(result_json).ok()?;
    let result = match value.get("status") {
        Some(_) => &value,
        None => value.get("result")?,
    };
    let status_text = result.get("status")?.as_str()?;
    let status: TrackLifecycle =
        serde_json::from_value(serde_json::Value::String(status_text.to_owned())).ok()?;
    let text = |key: &str| {
        result
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(TrackStatusAnswer {
        status,
        stage: text("last_executor_stage"),
        tx_hash: text("transactionHash"),
    })
}

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
    /// [`USER_OP_STATUS_METHOD`], parsed with [`parse_user_op_status`]. Null /
    /// error / unknown status / older relay → `StatusUnavailable`
    /// (`tx-reconciler.ts:96-115`).
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
    /// The account's holdings on this chain changed because an op of ours
    /// landed — confirmed, or failed with gas spent (spec 082 RE8, G26). The
    /// shell re-reads the balance; answered [`TrackShellResult::Notified`].
    /// Never for an op that did not land (pending, unreachable, age, NotSent,
    /// a relay rejection).
    HoldingsMoved { chain_id: u32 },
    /// The relay-independent landing check (spec 082 ruling 8): run
    /// `eth_getLogs{address: entry_point, topics: [topic0, user_op_hash],
    /// fromBlock, toBlock}` and `eth_blockNumber` through the pool and answer
    /// [`TrackShellResult::OpEvent`] with what came back — the shell judges
    /// nothing. `from_block: None` asks for the head only (no logs read).
    /// `topic0` is `user_op::USER_OPERATION_EVENT_TOPIC`, carried here so no
    /// shell needs another export to build the filter.
    FindOpEvent {
        chain_id: u32,
        entry_point: String,
        topic0: String,
        user_op_hash: String,
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        from_block: Option<u64>,
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        to_block: Option<u64>,
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
    /// The relay's view of an op with no receipt ([`USER_OP_STATUS_METHOD`]).
    Status {
        user_op_hash: String,
        status: TrackLifecycle,
        /// Executor stage that last touched the op, e.g.
        /// `in_band_settlement_hold`.
        stage: Option<String>,
        now_ms: f64,
        /// The relay's bundle tx, when it names one ([`TrackStatusAnswer`]).
        #[serde(default)]
        tx_hash: Option<String>,
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
    /// The pool's answer to [`TrackOperation::FindOpEvent`], as it came
    /// (ruling 8): `logs_json` the `eth_getLogs` result (the log array), or
    /// `error_json` the JSON-RPC error member the pool answered (a range
    /// limit is one, T180), plus the head from `eth_blockNumber`. Neither =
    /// no answer. The core decides what a range error is, never the shell.
    OpEvent {
        user_op_hash: String,
        now_ms: f64,
        #[serde(default)]
        logs_json: Option<String>,
        #[serde(default)]
        error_json: Option<String>,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        head_block: Option<u64>,
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
    /// The submit ended "may have been sent" (spec 082 RA4) — persisted with
    /// the record so a restart keeps following it as such.
    #[serde(default)]
    pub maybe_sent: bool,
    /// The chain head read before the first submit POST, persisted with the
    /// record: where the relay-independent landing check starts (ruling 8).
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub submit_block: Option<u64>,
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
        /// The submit ended "may have been sent" (spec 082 RA4, ruling 1):
        /// the hash is the locally computed one and the relay may never have
        /// seen it. Followed to its end — see [`TrackOutcome::MaybeSent`].
        #[serde(default)]
        maybe_sent: bool,
        /// The chain head read once before the first submit POST; `None` =
        /// unknown (ruling 8's find-event then starts below the head).
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
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
    /// A may-have-been-sent op the relay never admitted (spec 082 RA4): it
    /// answered `not_found` [`NOT_FOUND_CONFIRMATIONS`] times past the grace,
    /// with no receipt. Nothing will land. Terminal.
    NotSent,
    /// The bundler was unreachable for the whole wait window: the op's fate
    /// is genuinely unknown, which is NOT the same as "pending".
    Unknown,
}

impl EntryStatus {
    fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntryStatus::Confirmed { .. }
                | EntryStatus::Failed { .. }
                | EntryStatus::Rejected
                | EntryStatus::NotSent
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
    /// The bundle tx the relay's status named (spec 082 RA7) — a link for an
    /// op that has not produced a receipt yet. Never a verdict.
    relay_tx_hash: Option<String>,
    /// The submit ended "may have been sent" (spec 082 RA4).
    maybe_sent: bool,
    /// The relay has shown it holds the op: a receipt, or any status other
    /// than `not_found`. From then on the op is an ordinary one.
    acknowledged: bool,
    /// Consecutive `not_found` answers at age ≥ [`NOT_FOUND_GRACE_MS`].
    not_found_streak: u32,
    /// The head read before the first submit POST (ruling 8); `None` = unknown.
    submit_block: Option<u64>,
    /// The find-event scan (ruling 8) — see [`FindScan`].
    find: FindScan,
}

/// Where the relay-independent landing check stands for one entry.
#[derive(Clone, Debug)]
struct FindScan {
    /// The next block to read; `None` until a start is known (the submit's
    /// head, or the first head read minus [`FIND_OP_LOOKBACK_BLOCKS`]).
    from: Option<u64>,
    /// The current window width, ≤ [`FIND_OP_MAX_RANGE`], halved on a range
    /// error down to one block.
    width: u64,
    /// The highest head the shell has reported.
    head: Option<u64>,
    /// The window asked and not yet answered (`from None` = head only).
    in_flight: Option<(Option<u64>, Option<u64>)>,
    /// Issue time of the last find op — the status-poll cadence.
    last_ms: Option<f64>,
    /// Every block from the start up to here has been read, with no event.
    scanned_through: Option<u64>,
}

impl FindScan {
    fn new(submit_block: Option<u64>, submitted_at_ms: Option<f64>) -> Self {
        FindScan {
            from: submit_block,
            width: FIND_OP_MAX_RANGE,
            head: None,
            in_flight: None,
            last_ms: submitted_at_ms,
            scanned_through: None,
        }
    }

    /// The chain has been read, with no event, up to the latest head seen —
    /// what a relay's `not_found` must be joined by before "not sent".
    fn caught_up(&self) -> bool {
        matches!((self.scanned_through, self.head), (Some(through), Some(head)) if through >= head)
    }

    /// The next window, or `None` (head only) when there is nothing to read
    /// below a known head yet.
    fn next_window(&self) -> Option<(u64, u64)> {
        let head = self.head?;
        let from = self.from?;
        if from > head {
            return None;
        }
        let to = from.saturating_add(self.width.max(1) - 1).min(head);
        Some((from, to))
    }
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
            relay_tx_hash: None,
            maybe_sent: false,
            acknowledged: false,
            not_found_streak: 0,
            submit_block: None,
            find: FindScan::new(None, submitted_at_ms),
        }
    }

    /// Spec 082 RA4: still in the dark about whether the relay ever had it.
    fn in_doubt(&self) -> bool {
        self.maybe_sent && !self.acknowledged && !self.status.is_terminal() && !self.abandoned
    }

    /// Merge what a submit (or a reloaded record) knows: "may have been
    /// sent" is sticky, and the first known head wins.
    fn merge_submit_facts(&mut self, maybe_sent: bool, submit_block: Option<u64>) {
        self.maybe_sent |= maybe_sent;
        if self.submit_block.is_none() {
            self.submit_block = submit_block;
            if self.find.from.is_none() && self.find.scanned_through.is_none() {
                self.find.from = submit_block;
            }
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
    /// May have been sent, and the relay never had it (spec 082 RA4): two
    /// `not_found` answers past the grace, no receipt. Terminal; the records
    /// are failed. Its words are "not sent", never the fee-rejected ones.
    NotSent,
}

/// Where an op is in its life, for the words a person reads (spec 079): the
/// status says WHY (fee-held, unreachable, accepted-not-landed…); this says
/// WHEN, and every client words the four the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrackOutcome {
    /// Inside the wait window: "submitted, waiting to land".
    Landing,
    /// Past the window and still asked about: "not landed yet, Vela keeps
    /// checking — do not send it again". Never a failure.
    StillConfirming,
    /// Past the 24 h line: no longer asked about, fate unknown — check the
    /// explorer. Still not a failure.
    Unknown,
    /// Confirmed, dropped or rejected — the status says which.
    Final,
    /// The submit's reply was lost and the relay has not yet shown it holds
    /// the op (spec 082 RA4, ruling 1): "it may have been sent — Vela keeps
    /// checking, don't send it again". Never "try again".
    MaybeSent,
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
    pub outcome: TrackOutcome,
    /// The bundle tx the relay's status named while no receipt has (spec 082
    /// RA7, the 079 D2 explorer link). A link only — `tx_hash` above is the
    /// verdict's.
    #[serde(default)]
    pub relay_tx_hash: Option<String>,
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
                maybe_sent,
                submit_block,
            } => submitted(
                model,
                &user_op_hash,
                record_ids,
                chain_id,
                (maybe_sent, submit_block),
            ),
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
                    EntryStatus::NotSent => TrackStatus::NotSent,
                    EntryStatus::Unknown => TrackStatus::Unreachable,
                };
                let tx_hash = match &entry.status {
                    EntryStatus::Confirmed { tx_hash } | EntryStatus::Failed { tx_hash } => {
                        Some(tx_hash.clone())
                    }
                    _ => None,
                };
                let outcome = if entry.status.is_terminal() {
                    TrackOutcome::Final
                } else if entry.abandoned {
                    TrackOutcome::Unknown
                } else if entry.in_doubt() {
                    TrackOutcome::MaybeSent
                } else if entry.window_closed || entry.aborted {
                    TrackOutcome::StillConfirming
                } else {
                    TrackOutcome::Landing
                };
                TrackEntryView {
                    user_op_hash: hash.clone(),
                    chain_id: entry.chain_id,
                    record_ids: entry.record_ids.clone(),
                    status,
                    tx_hash,
                    polling: !entry.status.is_terminal() && !entry.abandoned,
                    submitted_at_ms: entry.submitted_at_ms,
                    outcome,
                    relay_tx_hash: entry.relay_tx_hash.clone(),
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
    (maybe_sent, submit_block): (bool, Option<u64>),
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
    entry.merge_submit_facts(maybe_sent, submit_block);

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
            entry.acknowledged = true;
            if entry.status.is_terminal() {
                // Already resolved by another path — never double-resolve.
                return Command::done();
            }
            entry.status = EntryStatus::Confirmed {
                tx_hash: tx_hash.clone(),
            };
            let ids = entry.record_ids.clone();
            let chain_id = entry.chain_id;
            confirm_records(model.attempt, key, chain_id, ids, tx_hash)
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
            entry.acknowledged = true;
            if entry.status.is_terminal() {
                return Command::done();
            }
            // "Dropped from the network" — the one receipt shape that may
            // fail records, and it terminates tracking immediately (③). It
            // landed, so gas was spent: the holdings moved too (RE8).
            let landed = (!tx_hash.is_empty()).then_some(entry.chain_id);
            entry.status = EntryStatus::Failed { tx_hash };
            let ids = entry.record_ids.clone();
            fail_records(model.attempt, ids, landed)
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
            tx_hash,
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
                if let Some(tx_hash) = tx_hash.filter(|hash| !hash.is_empty()) {
                    entry.relay_tx_hash = Some(tx_hash);
                }
                entry.last_status = Some((status, stage));
                if status == TrackLifecycle::Rejected {
                    // The relay refused it before any block: nothing was
                    // sent, nothing will land. Terminal, immediately (③).
                    entry.acknowledged = true;
                    entry.status = EntryStatus::Rejected;
                    Some(entry.record_ids.clone())
                } else if status == TrackLifecycle::NotFound {
                    // Spec 082 RA4: only a may-have-been-sent op the relay has
                    // never shown it holds is ended by `not_found` — a plain
                    // op's stays inert (079), and so does one the relay has
                    // already acknowledged. Inside the grace it is ignored.
                    let age = now_ms - entry.submitted_at_ms.unwrap_or(now_ms);
                    if entry.in_doubt() && age >= NOT_FOUND_GRACE_MS {
                        entry.not_found_streak = entry.not_found_streak.saturating_add(1);
                    }
                    // Ruling 8: the relay's word alone is not enough — the chain
                    // must have been read up to its head with no event for this
                    // op, or a relay that lost track of a landed op would say
                    // "not sent" over money that moved.
                    if entry.in_doubt()
                        && entry.not_found_streak >= NOT_FOUND_CONFIRMATIONS
                        && entry.find.caught_up()
                    {
                        entry.status = EntryStatus::NotSent;
                        Some(entry.record_ids.clone())
                    } else {
                        None
                    }
                } else {
                    // Anything else means the relay holds the op: it is an
                    // ordinary one from here on (RA4), and the streak resets.
                    // Beyond that — including `included`/`failed` — recorded
                    // only, verbatim from `waitForReceipt`.
                    entry.acknowledged = true;
                    entry.not_found_streak = 0;
                    None
                }
            };
            match rejected_ids {
                Some(ids) => fail_records(model.attempt, ids, None),
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
                // A restart keeps a may-have-been-sent op one (spec 082).
                entry.merge_submit_facts(record.maybe_sent, record.submit_block);
            }
            run_scheduler(model, now_ms)
        }

        TrackShellResult::OpEvent {
            user_op_hash,
            now_ms,
            logs_json,
            error_json,
            head_block,
        } => on_op_event(
            model,
            &user_op_hash,
            now_ms,
            OpEventAnswer {
                logs_json,
                error_json,
                head_block,
            },
        ),

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

        // Receipt cadence: 3s inside the window, then a pace that slows with
        // the op's age (or after an abort). One in-flight request per hash,
        // shared (⑤).
        let in_window = !entry.window_closed && !entry.aborted;
        let receipt_interval = receipt_interval_ms(in_window, age);
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
        // second endpoint). Stamped at issue, as `lastStatusAt` is. A
        // may-have-been-sent op the relay has not acknowledged is asked past
        // the window too, at the receipt pace (spec 082 RA4): in exactly this
        // fault the relay was silent during the window.
        let status_interval = if in_window {
            Some(STATUS_POLL_INTERVAL_MS)
        } else if entry.in_doubt() {
            Some(receipt_interval)
        } else {
            None
        };
        let status_due = status_interval.is_some_and(|interval| {
            !entry.status_in_flight
                && entry
                    .last_status_poll_ms
                    .is_none_or(|last| now_ms - last >= interval)
        });
        if status_due {
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

        // Ruling 8: while the relay has not shown it holds a may-have-been-
        // sent op, read the chain for the op's own event, on the same
        // cadence as the status polls.
        let find_interval = if in_window {
            STATUS_POLL_INTERVAL_MS
        } else {
            receipt_interval
        };
        if entry.in_doubt()
            && entry.find.in_flight.is_none()
            && entry
                .find
                .last_ms
                .is_none_or(|last| now_ms - last >= find_interval)
        {
            entry.find.last_ms = Some(now_ms);
            commands.push(find_op(attempt, hash, entry));
        }
    }

    commands.push(render());
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// The relay-independent landing check (ruling 8)
// ---------------------------------------------------------------------------

/// One find op for `entry`: the next window below the known head, or the head
/// alone when there is none.
fn find_op(attempt: u64, hash: &str, entry: &mut Entry) -> Command<TrackEffect, Event> {
    let window = entry.find.next_window();
    let (from_block, to_block) = match window {
        Some((from, to)) => (Some(from), Some(to)),
        None => (None, None),
    };
    entry.find.in_flight = Some((from_block, to_block));
    shell_request(
        attempt,
        TrackOperation::FindOpEvent {
            chain_id: entry.chain_id,
            entry_point: crate::safe::ENTRY_POINT.to_owned(),
            topic0: crate::user_op::USER_OPERATION_EVENT_TOPIC.to_owned(),
            user_op_hash: hash.to_owned(),
            from_block,
            to_block,
        },
    )
}

struct OpEventAnswer {
    logs_json: Option<String>,
    error_json: Option<String>,
    head_block: Option<u64>,
}

/// What the op's own `UserOperationEvent` said, if the logs hold it.
enum Found {
    Landed {
        tx_hash: String,
        success: bool,
    },
    Nothing,
    /// Not a log array — no answer.
    Unreadable,
}

/// Read the op's event out of an `eth_getLogs` result: the EntryPoint's
/// `UserOperationEvent` whose `topics[1]` is this op's hash. `data` is
/// `(uint256 nonce, bool success, uint256 actualGasCost, uint256
/// actualGasUsed)`; `success` is its second word.
fn find_in_logs(logs_json: &str, user_op_hash: &str) -> Found {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(logs_json) else {
        return Found::Unreadable;
    };
    let logs = match value.get("result") {
        Some(result) => result,
        None => &value,
    };
    let Some(logs) = logs.as_array() else {
        return Found::Unreadable;
    };
    let text = |log: &serde_json::Value, key: &str| {
        log.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    for log in logs {
        if log.get("removed").and_then(serde_json::Value::as_bool) == Some(true) {
            continue;
        }
        if !text(log, "address").eq_ignore_ascii_case(crate::safe::ENTRY_POINT) {
            continue;
        }
        let topics: Vec<&str> = log
            .get("topics")
            .and_then(serde_json::Value::as_array)
            .map(|topics| {
                topics
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect()
            })
            .unwrap_or_default();
        let ours = topics.first().is_some_and(|topic| {
            topic.eq_ignore_ascii_case(crate::user_op::USER_OPERATION_EVENT_TOPIC)
        }) && topics
            .get(1)
            .is_some_and(|topic| topic.eq_ignore_ascii_case(user_op_hash));
        if !ours {
            continue;
        }
        let tx_hash = text(log, "transactionHash");
        let data = text(log, "data");
        let data = data.strip_prefix("0x").unwrap_or(&data);
        // The second 32-byte word; anything unreadable is not a verdict.
        let Some(word) = data.get(64..128) else {
            continue;
        };
        if tx_hash.is_empty() || !word.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let success = word.bytes().any(|b| b != b'0');
        return Found::Landed { tx_hash, success };
    }
    Found::Nothing
}

/// The JSON-RPC error member the pool answered, as the range rule reads it.
fn error_info(error_json: &str) -> Option<super::rpc_pool::RpcErrorInfo> {
    let value: serde_json::Value = serde_json::from_str(error_json).ok()?;
    let error = value.get("error").unwrap_or(&value);
    let code = error
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .and_then(|code| i32::try_from(code).ok());
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    Some(super::rpc_pool::RpcErrorInfo { code, message })
}

fn on_op_event(
    model: &mut Model,
    user_op_hash: &str,
    now_ms: f64,
    answer: OpEventAnswer,
) -> Command<TrackEffect, Event> {
    let key = normalize(user_op_hash);
    let attempt = model.attempt;
    let Some(entry) = model.entries.get_mut(&key) else {
        return Command::done();
    };
    let Some((asked_from, asked_to)) = entry.find.in_flight.take() else {
        return Command::done(); // unsolicited — dropped by construction
    };
    if let Some(head) = answer.head_block {
        entry.find.head = Some(entry.find.head.map_or(head, |known| known.max(head)));
    }
    if entry.status.is_terminal() {
        return Command::done(); // resolved meanwhile — never double-resolve
    }

    // A head-only answer: start the scan, and read the first window now.
    let Some((from, to)) = asked_from.zip(asked_to) else {
        if entry.find.from.is_none() {
            if let Some(head) = entry.find.head {
                entry.find.from = Some(head.saturating_sub(FIND_OP_LOOKBACK_BLOCKS));
            }
        }
        return continue_scan(attempt, &key, entry, now_ms);
    };

    if let Some(logs_json) = answer.logs_json.as_deref() {
        match find_in_logs(logs_json, &key) {
            Found::Landed { tx_hash, success } => {
                // The op's own event on chain: a fact no later relay
                // `not_found` can undo (the entry is terminal from here).
                entry.acknowledged = true;
                let ids = entry.record_ids.clone();
                let chain_id = entry.chain_id;
                return if success {
                    entry.status = EntryStatus::Confirmed {
                        tx_hash: tx_hash.clone(),
                    };
                    confirm_records(attempt, key, chain_id, ids, tx_hash)
                } else {
                    entry.status = EntryStatus::Failed { tx_hash };
                    fail_records(attempt, ids, Some(chain_id))
                };
            }
            Found::Nothing => {
                entry.find.scanned_through = Some(to);
                entry.find.from = Some(to.saturating_add(1));
                return continue_scan(attempt, &key, entry, now_ms);
            }
            Found::Unreadable => return render(),
        }
    }
    if let Some(info) = answer.error_json.as_deref().and_then(error_info) {
        if super::rpc_pool::is_log_range_error(&info) {
            let asked = to.saturating_sub(from).saturating_add(1);
            let halved = (asked / 2).max(1);
            if halved < asked {
                entry.find.width = halved;
                return continue_scan(attempt, &key, entry, now_ms);
            }
        }
    }
    // Any other error, or no answer: the same window on the next tick.
    render()
}

/// After an answer that moved the scan: read the next window at once while
/// there is one below the known head (a catch-up is bounded by the gap);
/// caught up, wait for the next tick.
fn continue_scan(
    attempt: u64,
    key: &str,
    entry: &mut Entry,
    now_ms: f64,
) -> Command<TrackEffect, Event> {
    if !entry.in_doubt() || entry.find.next_window().is_none() {
        return render();
    }
    entry.find.last_ms = Some(now_ms);
    Command::all([find_op(attempt, key, entry), render()])
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
        | TrackShellResult::RecordsLoaded { now_ms, .. }
        | TrackShellResult::OpEvent { now_ms, .. } => Some(*now_ms),
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
            entry.find.last_ms = Some(now_ms);
        }
    }
}

/// The ONLY constructor of a `failed` patch — reachable from a `success ===
/// false` receipt, a relay rejection (③) and, since spec 082, a relay that
/// never admitted a may-have-been-sent op (RA4). Timeouts, aborts, age and
/// unreachable bundlers can never arrive here (①). `landed` names the chain
/// when the op reached a block (gas spent): the holdings moved (RE8).
fn fail_records(
    attempt: u64,
    ids: Vec<String>,
    landed: Option<u32>,
) -> Command<TrackEffect, Event> {
    let mut commands = vec![shell_request(
        attempt,
        TrackOperation::UpdateTxRecords {
            ids,
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Failed,
                tx_hash: None,
            },
        },
    )];
    if let Some(chain_id) = landed {
        commands.push(shell_request(
            attempt,
            TrackOperation::HoldingsMoved { chain_id },
        ));
    }
    commands.push(render());
    Command::all(commands)
}

/// A landed op: patch first, notify second — the reconciler's order
/// (`tx-reconciler.ts:236-240`): flip the records, then hand the authentic
/// logs to token_trust — and then the balance (spec 082 RE8).
fn confirm_records(
    attempt: u64,
    user_op_hash: String,
    chain_id: u32,
    ids: Vec<String>,
    tx_hash: String,
) -> Command<TrackEffect, Event> {
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
                user_op_hash,
                chain_id,
                tx_hash,
            },
        ),
        shell_request(attempt, TrackOperation::HoldingsMoved { chain_id }),
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
        let hex = format!(
            "0x{}",
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        assert_eq!(hex, SAFE_EXECUTION_FAILURE_TOPIC);
    }

    #[test]
    fn a_failure_log_fails_the_receipt_and_a_transfer_does_not() {
        let transfer = TrustReceiptLog {
            address: "0xtoken".to_owned(),
            topics: vec![
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".to_owned(),
            ],
            data: "0x".to_owned(),
        };
        let failure = TrustReceiptLog {
            address: "0xsafe".to_owned(),
            topics: vec![SAFE_EXECUTION_FAILURE_TOPIC
                .to_uppercase()
                .replace("0X", "0x")],
            data: "0x".to_owned(),
        };
        assert!(!safe_execution_failed(std::slice::from_ref(&transfer)));
        assert!(safe_execution_failed(&[transfer, failure]));
        assert!(!safe_execution_failed(&[]));
    }
}
