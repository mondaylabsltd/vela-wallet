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
//! Spec 082 round 2 (RJ1, RJ4):
//!
//! - The write-ahead: the sign and send machines hand an op over "may have
//!   been sent" BEFORE its POST, naming no record — held off "not sent" while
//!   the POST is out — then again with its records once the POST's verdict
//!   is in: `admitted` when the relay took it (an accepted op never reads
//!   MaybeSent), may-have-been-sent when its reply was lost (the not-found
//!   grace counts from then), or `Withdrawn` once it is proven never sent
//!   (the entry forgets those records; no patch, no balance read).
//! - The relay's tx hash is used, not only shown: a status that names one for
//!   a live entry asks the chain for that transaction's receipt
//!   ([`TrackOperation::TxReceipt`], at the receipt cadence, one in flight per
//!   hash); the op's own `UserOperationEvent` in its logs confirms or fails
//!   the op exactly as a relay receipt does. EX13's landed op read "not on
//!   chain yet" for 5 min 49 s with the tx hash in hand.
//! - A relay `rejected` that names a bundle tx is not a refusal before any
//!   block: the relay marks every op of a mined bundle `rejected` unless its
//!   own event succeeded. That tx's receipt decides — the op's event fails
//!   it (Dropped, gas spent) or confirms it; the tx mined without the op
//!   upholds the refusal (082 second review).
//! - The chain read proves an op absent only from at or below its submit:
//!   the head read before the POST, or a lookback anchored while the op was
//!   young enough ([`FIND_OP_LOOKBACK_COVERS_MS`]). A write-ahead record
//!   resumed hours after a quit, with no head read, is past the relay's
//!   memory of it (its status records live an hour) and past the lookback:
//!   it stays "may have been sent" unless its event is found.
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
///
/// The cooldown counts from when a poll was ISSUED (issue #464). It used to
/// count from when the answer came back, and every shell ticks every 3 s: a
/// poll answered 0.3 s after its tick was then 2.7 s old on the next tick and
/// waited for the one after — an op was asked every 6 s, not every 3.
pub const RECEIPT_POLL_INTERVAL_MS: f64 = 3_000.0;
/// How early a receipt poll may be issued against its cooldown. A shell's
/// tick is a timer, and the clock it reads wobbles by a few milliseconds
/// either way: a poll issued on one 3 s tick must be due on the next even
/// when that tick reads 2 999 ms later, or a tick 1 ms "early" waits a whole
/// extra tick — the 6 s cadence of issue #464 again, by chance. Far below
/// every interval of the cadence ladder ([`receipt_interval_ms`]).
pub const RECEIPT_TICK_SLACK_MS: f64 = 250.0;
/// Relay lifecycle-status cadence while the wait window is open —
/// `USER_OP_STATUS_POLL_INTERVAL_MS` (`safe-transaction.ts`). The first status
/// poll waits one full interval: "not ready yet" is by far the common case.
pub const STATUS_POLL_INTERVAL_MS: f64 = 12_000.0;
/// …except the first, 3 s after acceptance (spec 099 R6): the relay's queue,
/// its funding, its send are what the landing has to say in its first
/// seconds, and twelve of silence read as a hang.
pub const FIRST_STATUS_POLL_MS: f64 = 3_000.0;
/// An op the relay acknowledged and later answers `not_found` for — its
/// record expired (spec 099 R6): ended "not sent" this long after the first
/// such answer, with the chain read to its head and no event for it.
pub const FORGOTTEN_NOT_SENT_MS: f64 = 10.0 * 60.0 * 1000.0;
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
/// What the landing's countdown line says (spec 099 R6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum LandingLine {
    /// The relay has not put it on the network yet: no countdown — the
    /// landing says what the relay is doing instead.
    Waiting,
    /// No usual time is known for this chain: no countdown line.
    None,
    /// "~`seconds` s remaining" — inside the chain's usual time.
    Remaining,
    /// "`seconds` s so far" — past it, inside twice it.
    Elapsed,
    /// "Taking longer than usual".
    Slow,
}

/// The landing's pace: its countdown line and the ring round its disc.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct LandingPace {
    pub line: LandingLine,
    /// What `Remaining` / `Elapsed` count.
    pub seconds: u32,
    /// How full the ring is, 0..0.92; `None` — the ring roams.
    pub progress: Option<f32>,
}

/// The one countdown every landing draws (spec 099 R6), counted from when the
/// relay put the bundle on the network — never from acceptance, which counted
/// the relay's own queue and funding as the chain being slow.
///
/// Inside the usual time the line counts DOWN ("~9 s remaining" is a promise
/// with an end); past it, it counts what has passed; past twice it, it says
/// it is taking longer. The ring eases toward full and never gets there
/// (about 70% at the usual time, 86% at twice it, a 92% ceiling): only the
/// confirmation closes it.
#[must_use]
pub fn landing_pace(sent_at_ms: Option<f64>, typical_s: Option<u16>, now_ms: f64) -> LandingPace {
    let roaming = |line| LandingPace {
        line,
        seconds: 0,
        progress: None,
    };
    let Some(sent_at_ms) = sent_at_ms else {
        return roaming(LandingLine::Waiting);
    };
    let Some(typical) = typical_s.filter(|typical| *typical > 0).map(u32::from) else {
        return roaming(LandingLine::None);
    };
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "whole seconds since a send, clamped at zero"
    )]
    let elapsed = ((now_ms - sent_at_ms) / 1000.0)
        .max(0.0)
        .min(f64::from(u32::MAX)) as u32;
    #[allow(clippy::cast_precision_loss, reason = "seconds, far below 2^24")]
    let progress = 0.92 * (1.0 - (-1.4 * elapsed as f32 / typical as f32).exp());
    let (line, seconds) = if elapsed < typical {
        (LandingLine::Remaining, typical - elapsed)
    } else if elapsed < typical.saturating_mul(2) {
        (LandingLine::Elapsed, elapsed)
    } else {
        (LandingLine::Slow, elapsed)
    };
    LandingPace {
        line,
        seconds,
        progress: Some(progress),
    }
}

/// [`landing_pace`] as JSON — the shape the UniFFI and wasm shells read.
#[must_use]
pub fn landing_pace_json(sent_at_ms: Option<f64>, typical_s: Option<u16>, now_ms: f64) -> String {
    serde_json::to_string(&landing_pace(sent_at_ms, typical_s, now_ms)).unwrap_or_default()
}

/// The executor stage that parks an op until network fees fit its signed
/// reimbursement — `FEE_HOLD_STAGE` (`tx-reconciler.ts:84`).
pub const FEE_HOLD_STAGE: &str = "in_band_settlement_hold";

/// The executor stage of an op waiting while the relay tops up the gas it
/// pays with on the chain — the treasury → relayer transfer has to land
/// before the bundle can be sent (vela-relay `execution.rs`, "waiting for
/// relayer funding transaction confirmation").
///
/// Until it was read, a person watched "taking longer than usual" over an op
/// the relay was deliberately holding — and on Arbitrum, where every top-up
/// was refused (2026-10-03), over one it would never send.
pub const RELAY_FUNDING_STAGE: &str = "funding";

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
/// How old an op may be when that lookback is anchored and still reach its
/// submit on every chain Vela serves (5 000 Arbitrum blocks ≈ 20 min 50 s).
/// A lookback anchored later — a record resumed hours after a quit, with no
/// head read before its POST — may find the op's event but can never prove
/// it absent: it may start above the landing (082 second review).
pub const FIND_OP_LOOKBACK_COVERS_MS: f64 = 20.0 * 60.0 * 1000.0;

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
    /// Why the relay refused or failed it (`rejection_reason`, relay
    /// `fix/held-nonce-and-floor`): present on `rejected` / `failed` from a
    /// relay that says. Older relays send none; [`RefusalReason::of`] then
    /// reads the stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
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
        rejection_reason: text("rejection_reason"),
    })
}

/// Why the relay refused an operation, in the relay's own vocabulary
/// (`rejection_reason`, relay contract §2). A refusal is told by its reason —
/// never every one as "network fees stayed above the amount you approved",
/// which was true of one reason in eleven.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RefusalReason {
    /// Network fees stayed above what the signed fee covers through the hold.
    FeeBelowMarket,
    /// The fee was under the relay's minimum at settlement.
    FeeBelowMinimum,
    /// The fee payment is missing, unreadable or unproven.
    FeePaymentInvalid,
    /// Another operation of this account already used this nonce on-chain.
    NonceUsed,
    /// The operation fails when simulated.
    SimulationFailed,
    /// The queued payload is malformed.
    InvalidOperation,
    /// Tempo: a fee token other than pathUSD.
    UnsupportedFeeToken,
    /// The relay stopped retrying without sending (dead letter).
    RelayGaveUp,
    /// Mined, and its execution failed.
    RevertedOnchain,
    /// The whole bundle transaction reverted.
    BundleFailed,
    /// Anything else — a reason a newer relay names that this core does not.
    #[serde(other)]
    Unknown,
}

/// The sentence for a refusal whose reason is unknown, or has no words of its
/// own: "The network refused it — nothing was sent."
pub const REFUSED_KEY: &str = "componentsUi.signing.refused";
/// The sentence for [`RefusalReason::FeeBelowMarket`].
pub const REFUSED_FEES_KEY: &str = "send.txRejectedFees";
/// The sentence for [`RefusalReason::NonceUsed`]: another transaction of the
/// account went first.
pub const REFUSED_NONCE_KEY: &str = "componentsUi.signing.wentFirst";

impl RefusalReason {
    /// The reason the relay named, else the one its executor stage implies
    /// (an older relay names none: relay contract §2's own derivation).
    /// `None` when neither says.
    #[must_use]
    pub fn of(rejection_reason: Option<&str>, stage: Option<&str>) -> Option<Self> {
        if let Some(reason) = rejection_reason.filter(|reason| !reason.is_empty()) {
            return serde_json::from_value(serde_json::Value::String(reason.to_owned())).ok();
        }
        Some(match stage? {
            "nonce" => Self::NonceUsed,
            "simulation" => Self::SimulationFailed,
            "dead_letter" => Self::RelayGaveUp,
            "queue" => Self::InvalidOperation,
            "tempo_fee_token" => Self::UnsupportedFeeToken,
            "in_band_settlement" | FEE_HOLD_STAGE => Self::FeeBelowMarket,
            _ => return None,
        })
    }

    /// The corpus key of the sentence that tells a person this refusal.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::FeeBelowMarket => REFUSED_FEES_KEY,
            Self::NonceUsed => REFUSED_NONCE_KEY,
            _ => REFUSED_KEY,
        }
    }
}

/// The sentence for a refusal, whatever is known of it.
#[must_use]
pub fn refusal_key(reason: Option<RefusalReason>) -> &'static str {
    reason.map_or(REFUSED_KEY, RefusalReason::key)
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
    /// `eth_getTransactionReceipt(tx_hash)` through the chain pool (spec 082
    /// RJ4): the bundle transaction the relay's status named for this op.
    /// Answered [`TrackShellResult::TxReceipt`] with the result as it came —
    /// the shell judges nothing.
    TxReceipt {
        chain_id: u32,
        tx_hash: String,
        user_op_hash: String,
    },
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
        /// [`TrackStatusAnswer::rejection_reason`], passed through. Absent
        /// (an older shell or relay): the reason is read from `stage`.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(optional))]
        rejection_reason: Option<String>,
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
    /// The answer to [`TrackOperation::TxReceipt`] (spec 082 RJ4):
    /// `receipt_json` is the JSON-RPC `result` as it came — `"null"` while
    /// the transaction is not mined; `None` when the pool got no answer. The
    /// core reads the op's own `UserOperationEvent` out of its `logs` (the
    /// find-event reader) and runs the `ExecutionFailure` rule over that op's
    /// logs; a receipt without the op's event is ignored.
    TxReceipt {
        user_op_hash: String,
        now_ms: f64,
        #[serde(default)]
        receipt_json: Option<String>,
    },
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
    /// The account that signed it (the stored record's `from`) — so after a
    /// restart the device still knows this account has an operation in
    /// flight on this chain ([`in_flight_ops`]). `None` from an older shell.
    #[serde(default)]
    pub sender: Option<String>,
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
    /// How the operation ended (spec 097): the shell stores it with the
    /// records verbatim (`settlement`) and hands it back to the feed
    /// untouched (`FeedTxRecord::settlement`). Absent from a shell's wire
    /// when there is nothing to say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<TrackSettlement>,
}

/// How an operation ended, as the tracker proved it (spec 097 N4, N5).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackSettlement {
    /// A confirmed operation whose own receipt logs were read
    /// ([`proven_moves`]): what it moved for the account, one net line per
    /// coin — the figures Activity shows for it. `None` when no receipt logs
    /// were read (the chain's event alone found it, or a shell that sends no
    /// logs): nothing is claimed either way.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub moved: Option<Vec<TrackMove>>,
    /// A failed operation: why.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<TrackFailure>,
}

/// One coin a landed operation moved for its account, as its receipt proves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackMove {
    /// The token contract, lower-case; `None` for the chain's own coin.
    #[serde(default)]
    pub token: Option<String>,
    /// Base units, signed, as a decimal string: `-1160000000000000000`
    /// left the account, `300000000000000000` arrived.
    pub delta: String,
}

/// Why an operation failed — the words its request ended with (spec 097 N4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrackFailure {
    /// It landed and its execution failed: gas was spent, nothing else moved
    /// (`componentsTx.receipt.failedHint`).
    Reverted,
    /// The relay refused it before any block: nothing was sent
    /// (`componentsUi.signing.refused`).
    Refused,
    /// The relay never had it: nothing was sent (`send.txErrorGeneric`).
    NotSent,
}

impl TrackSettlement {
    fn moved(moved: Option<Vec<TrackMove>>) -> Option<Self> {
        moved.map(|moved| TrackSettlement {
            moved: Some(moved),
            failure: None,
        })
    }

    fn failed(failure: TrackFailure) -> Option<Self> {
        Some(TrackSettlement {
            moved: None,
            failure: Some(failure),
        })
    }
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
    ///
    /// Spec 082 RJ1 (second review): `record_ids` EMPTY and not `admitted`
    /// is the write-ahead's hand-off — a POST of the op is about to leave.
    /// Until a hand-off that names records (the POST's verdict) or a
    /// [`Event::Withdrawn`] ends that POST, a relay `not_found` counts for
    /// nothing and the op is never `NotSent`; the not-found grace then counts
    /// from that verdict. The chain check runs meanwhile, and records named
    /// after the op landed take the landing's patch.
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
        /// The relay accepted this op (spec 082 RJ1): the hand-off after a
        /// write-ahead one. Sets the entry acknowledged — an accepted op
        /// never reads [`TrackOutcome::MaybeSent`], and a relay `not_found`
        /// no longer counts against it.
        #[serde(default)]
        admitted: bool,
        /// The account that signed it — `SendOperation::TrackSubmitted` /
        /// `SignTrackerHandoff`'s `sender`, forwarded. What makes the op one
        /// this account must wait for ([`in_flight_ops`]).
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(optional))]
        sender: Option<String>,
    },
    /// The op was proven never sent after its write-ahead hand-off (spec 082
    /// RJ1): the submit failed before any POST, or the relay answered with
    /// another hash. Ends that POST. An entry the write-ahead made drops
    /// `record_ids` and goes when none is left; patches nothing, reads no
    /// balance. An entry an earlier POST of the identical op named is kept
    /// whole — that op may still land. A later submit of the same hash is
    /// tracked from the start.
    Withdrawn {
        user_op_hash: String,
        record_ids: Vec<String>,
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
    /// The account that signed it, lower-cased; `None` when no shell said.
    sender: Option<String>,
    /// Why the relay refused it, once it has ([`RefusalReason::of`]).
    refusal: Option<RefusalReason>,
    /// Stored-record ids patched on resolution — in place, same ids, never a
    /// second record (`dapp-history` rule; batch siblings patched together).
    record_ids: Vec<String>,
    status: EntryStatus,
    /// Stamped from the first observed clock after `Submitted` (or carried by
    /// the recovered record). Every deadline measures from it.
    submitted_at_ms: Option<f64>,
    /// Issue time of the last receipt poll — the cooldown counts from it
    /// (issue #464). A poll issued before any clock was seen (the first, at
    /// `Submitted`) is stamped by the first clock that arrives, or failing
    /// that by its own answer: a result only ever fills an EMPTY stamp. The
    /// in-flight flag, not this stamp, keeps two requests from overlapping.
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
    /// A [`TrackOperation::TxReceipt`] for `relay_tx_hash` is out (RJ4).
    tx_receipt_in_flight: bool,
    /// Issue time of the last one — the receipt cadence counts from it, as
    /// for `last_receipt_poll_ms` (issue #464).
    last_tx_receipt_ms: Option<f64>,
    /// A POST of this op is out (spec 082 RJ1, second review): the write-
    /// ahead handed the op over with no record, before its bytes left, and
    /// the POST's verdict has not come. A relay `not_found` says nothing
    /// then, and the op is never "not sent".
    posting: bool,
    /// This entry exists only because of that hand-off (no earlier verdict
    /// or reload named the op): a withdrawal forgets it. An earlier POST of
    /// the identical op keeps its entry whatever a later one proves.
    from_posting: bool,
    /// Where the not-found grace counts from, when not the submission: the
    /// POST's verdict. `Some(None)` = the verdict came, its clock is next.
    grace_from_ms: Option<Option<f64>>,
    /// The relay said `rejected` while naming a bundle tx (082 second
    /// review): the op may be on chain, reverted. Only that tx's receipt
    /// settles it — the op's event (landed), or the tx mined without it
    /// (the refusal stands).
    relay_rejected: bool,
    /// What the confirmed op moved, read from its own receipt logs
    /// ([`proven_moves`]) — kept for records named after it landed.
    moved: Option<Vec<TrackMove>>,
    /// A status poll has been issued (the first comes sooner, 099 R6).
    status_asked: bool,
    /// When the tracker first learned the bundle is on the network — the
    /// relay's `submitted`/`included`, a bundle tx it named, or a receipt.
    /// The landing counts the chain's usual time from here (099 R6).
    relay_sent_at_ms: Option<f64>,
    /// Acknowledged, then `not_found` past the grace: the relay's record
    /// expired. Since when, and how many such answers in a row (099 R6).
    forgotten_since_ms: Option<f64>,
    forgotten_streak: u32,
    /// Where the relay last said the op is — its lifecycle, its stage, and
    /// whether it has named a bundle tx. `not_found` places it nowhere and
    /// is not kept. A change of it is progress ([`Entry::heard`]). An op seen
    /// submitted starts at `queued`; one reloaded after a restart starts
    /// nowhere, and its first word is only where it is.
    progress_mark: Option<(TrackLifecycle, Option<String>, bool)>,
    /// When the relay's word last moved; `None` = not since the submission
    /// (`submitted_at_ms` then). The hold's time limit counts from here
    /// ([`in_flight_stalled`]).
    progressed_at_ms: Option<f64>,
    /// [`IN_FLIGHT_STALL_MS`] has passed with no progress: the op no longer
    /// holds its account's nonce ([`in_flight_ops`]). Kept until the next
    /// progress, so a clock that steps back never re-holds a released one.
    stalled: bool,
}

/// Where the relay-independent landing check stands for one entry.
#[derive(Clone, Debug)]
struct FindScan {
    /// The next block to read; `None` until a start is known (the submit's
    /// head, or the first head read minus [`FIND_OP_LOOKBACK_BLOCKS`]).
    from: Option<u64>,
    /// The current window width, ≤ [`FIND_OP_MAX_RANGE`], halved on a range
    /// error down to one block, doubled again after a clean full-width read.
    width: u64,
    /// The highest head the shell has reported.
    head: Option<u64>,
    /// The window asked and not yet answered (`from None` = head only).
    in_flight: Option<(Option<u64>, Option<u64>)>,
    /// Issue time of the last find op — the status-poll cadence.
    last_ms: Option<f64>,
    /// Every block from the start up to here has been read, with no event.
    scanned_through: Option<u64>,
    /// The start is known to be at or below the submit: the head read before
    /// the POST, or a lookback anchored while the op was young enough for
    /// it to reach the submit ([`FIND_OP_LOOKBACK_COVERS_MS`]).
    anchored: bool,
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
            anchored: submit_block.is_some(),
        }
    }

    /// The chain has been read, with no event, from at or below the submit up
    /// to the latest head seen — what a relay's `not_found` must be joined by
    /// before "not sent". A scan that may have started above the submit
    /// proves nothing absent.
    fn caught_up(&self) -> bool {
        self.anchored
            && matches!((self.scanned_through, self.head), (Some(through), Some(head)) if through >= head)
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
            tx_receipt_in_flight: false,
            last_tx_receipt_ms: None,
            posting: false,
            from_posting: false,
            grace_from_ms: None,
            relay_rejected: false,
            moved: None,
            status_asked: false,
            relay_sent_at_ms: None,
            forgotten_since_ms: None,
            forgotten_streak: 0,
            sender: None,
            refusal: None,
            progress_mark: None,
            progressed_at_ms: None,
            stalled: false,
        }
    }

    /// The relay's word on the op now: `(status, stage)`, with or without a
    /// bundle tx named. A word that differs from the last one is progress —
    /// the hold's ten minutes start again, and a released hold is taken up
    /// again (the op is moving, so its nonce is spoken for). With no mark
    /// yet (a reload) the first word is only a starting point: after a
    /// restart it says where the op is, not that it moved, and taking a
    /// released hold back on it would flip the confirm shut seconds after it
    /// opened.
    fn heard(&mut self, status: TrackLifecycle, stage: Option<&str>, now_ms: f64) {
        if status == TrackLifecycle::NotFound {
            return;
        }
        let mark = (
            status,
            stage.map(str::to_owned),
            self.relay_tx_hash.is_some(),
        );
        if self
            .progress_mark
            .as_ref()
            .is_some_and(|last| *last != mark)
        {
            self.progressed_at_ms = Some(now_ms);
            self.stalled = false;
        }
        self.progress_mark = Some(mark);
    }

    /// The first sender anyone named for this op wins (one op, one account).
    fn merge_sender(&mut self, sender: Option<String>) {
        if self.sender.is_none() {
            self.sender = sender
                .map(|sender| sender.trim().to_lowercase())
                .filter(|sender| !sender.is_empty());
        }
    }

    /// The relay forgot an op it had acknowledged (099 R6).
    fn forgotten(&self) -> bool {
        self.forgotten_since_ms.is_some() && !self.status.is_terminal() && !self.abandoned
    }

    /// The chain is read for the op's own event: an op the relay may never
    /// have had, or one it has forgotten.
    fn scanning(&self) -> bool {
        self.in_doubt() || self.forgotten()
    }

    /// How long the relay has had to write the op down, for its `not_found`
    /// (RA4): since the POST's verdict when the write-ahead preceded it, else
    /// since the submission. `None` while no POST of it has ended.
    fn grace_age(&self, now_ms: f64) -> Option<f64> {
        if self.posting {
            return None;
        }
        let from = match self.grace_from_ms {
            Some(stamped) => stamped?,
            None => self.submitted_at_ms.unwrap_or(now_ms),
        };
        Some(now_ms - from)
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
                self.find.anchored = submit_block.is_some();
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

    /// The relay's last word is that it is topping up its gas on the chain
    /// ([`RELAY_FUNDING_STAGE`]). Live, not a window verdict: it says why the
    /// op is waiting for exactly as long as the relay says so, and the next
    /// status that moves on takes it away.
    fn relay_funding(&self) -> bool {
        matches!(
            &self.last_status,
            Some((TrackLifecycle::Queued, Some(stage))) if stage == RELAY_FUNDING_STAGE
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
    /// Queued while the relay tops up the gas it pays with on this chain
    /// ([`RELAY_FUNDING_STAGE`]) — it sends the op itself once that lands.
    /// Pending; it lasts only as long as the relay says so.
    RelayFunding,
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
    /// When the tracker learned the relay had put the bundle on the network
    /// (spec 099 R6). `None` while the relay still holds it: the landing
    /// says what the relay is doing, and counts nothing down.
    #[serde(default)]
    pub relay_sent_at_ms: Option<f64>,
    /// The account that signed it, lower-cased; `None` when no shell said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
    /// `Rejected` only: why the relay refused it, when the relay (or its
    /// stage) says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<RefusalReason>,
    /// `Rejected` only: the corpus key of the sentence that says why —
    /// [`RefusalReason::key`], or the plain "refused, nothing was sent" when
    /// the reason is unknown. Every surface that tells a refusal (the send
    /// receipt, the signing sheet's ending, a row's detail) draws THIS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal_key: Option<String>,
    /// Not final, and the relay has said nothing new of it for
    /// [`IN_FLIGHT_STALL_MS`]: it no longer holds its account's nonce
    /// ([`in_flight_ops`]). Off the wire while false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stalled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrackView {
    /// Newest first.
    pub entries: Vec<TrackEntryView>,
}

// ---------------------------------------------------------------------------
// One operation in flight per account and network (wait for the first)
// ---------------------------------------------------------------------------

/// An operation of an account that holds its nonce on a chain: accepted by
/// the relay and not yet final. A second operation of the same account on
/// the same chain signed now would take the SAME nonce (every client reads it
/// from the chain, where the first has not landed) — the relay refuses it, or
/// one of the two is dropped after the other lands. So the second waits:
/// every confirm that would sign one is held while this exists
/// (`ConfirmBlock::PreviousPending`), and opens once the first is final — or
/// once the relay has said nothing new of it for [`IN_FLIGHT_STALL_MS`].
///
/// Read from the tracker, which persists every pending record — a restart
/// still knows (`TrackPendingRecord::sender`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct InFlightOp {
    /// The account, lower-cased.
    pub sender: String,
    pub chain_id: u32,
    pub user_op_hash: String,
}

/// How long an operation in flight holds its account's nonce with no
/// progress: ten minutes (owner's decision, 2026-10-09). Before it, the hold
/// lasted until the tracker's verdict — up to the 24 h line while the relay
/// was unreachable, with a second send held all that time.
///
/// Releasing is safe because the relay keeps the rule too: a second op at a
/// nonce still live there is refused (`nonce_in_flight`; an older relay's
/// `[existingHash:…]`), and both read as `RelayRejection::NonceHeld` — the
/// "previous transaction pending" refusal, with Try again. The worst case is
/// that clear refusal, never a silent drop of either operation.
pub const IN_FLIGHT_STALL_MS: f64 = 10.0 * 60.0 * 1000.0;

/// The hold's time limit, the one rule: an operation whose relay status last
/// moved at `progressed_at_ms` (its submission, before any move) has stalled
/// by `now_ms` once [`IN_FLIGHT_STALL_MS`] has passed, and holds no nonce
/// from then until it moves again. Progress is a change in what the relay
/// says of it — its lifecycle or stage, or a bundle tx named; a poll that
/// fails, `not_found`, or the same answer again is none.
#[must_use]
pub fn in_flight_stalled(progressed_at_ms: f64, now_ms: f64) -> bool {
    now_ms - progressed_at_ms >= IN_FLIGHT_STALL_MS
}

/// Every operation in [`TrackView`] that holds its account's nonce: the
/// sender is known, it is not final, the tracker still follows it, the
/// relay has it, and it has not stalled ([`in_flight_stalled`]). One whose
/// submit reply was lost and that the relay has not shown it holds
/// ([`TrackOutcome::MaybeSent`]) reserves nothing: a retry of THE SAME op
/// reuses its nonce (RA5), and a different op at that nonce is one of two
/// that cannot both land — the relay refuses it while the first is live.
/// Nor does one past the 24 h line, which nothing follows any more.
#[must_use]
pub fn in_flight_ops(view: &TrackView) -> Vec<InFlightOp> {
    view.entries
        .iter()
        .filter(|entry| {
            entry.polling
                && matches!(
                    entry.outcome,
                    TrackOutcome::Landing | TrackOutcome::StillConfirming
                )
                && matches!(
                    entry.status,
                    TrackStatus::Pending
                        | TrackStatus::FeeHeld
                        | TrackStatus::RelayFunding
                        | TrackStatus::AcceptedNotLanded
                        | TrackStatus::Unreachable
                )
                && !entry.stalled
        })
        .filter_map(|entry| {
            Some(InFlightOp {
                sender: entry.sender.clone()?,
                chain_id: entry.chain_id,
                user_op_hash: entry.user_op_hash.clone(),
            })
        })
        .collect()
}

/// [`in_flight_ops`] over the tracker's view as JSON (UniFFI, wasm): a JSON
/// array, `[]` when the view does not read. A shell forwards it to the send
/// and signing machines (`Event::InFlightOps`) on every tracker render.
#[must_use]
pub fn in_flight_ops_json(view_json: &str) -> String {
    let ops = serde_json::from_str::<TrackView>(view_json)
        .map(|view| in_flight_ops(&view))
        .unwrap_or_default();
    serde_json::to_string(&ops).unwrap_or_else(|_| "[]".to_owned())
}

/// The operation `sender` must wait for on `chain_id`, if any — never `own`,
/// the operation the asking surface itself submitted.
#[must_use]
pub fn previous_in_flight<'a>(
    ops: &'a [InFlightOp],
    sender: &str,
    chain_id: u32,
    own: Option<&str>,
) -> Option<&'a InFlightOp> {
    ops.iter().find(|op| {
        op.chain_id == chain_id
            && op.sender.eq_ignore_ascii_case(sender.trim())
            && !own.is_some_and(|own| own.eq_ignore_ascii_case(&op.user_op_hash))
    })
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
                admitted,
                sender,
            } => submitted(
                model,
                &user_op_hash,
                record_ids,
                chain_id,
                (maybe_sent, submit_block, admitted),
                sender,
            ),
            Event::Withdrawn {
                user_op_hash,
                record_ids,
            } => {
                let key = normalize(&user_op_hash);
                let emptied = model.entries.get_mut(&key).is_some_and(|entry| {
                    // That POST is over: nothing of it left.
                    entry.posting = false;
                    if !entry.from_posting {
                        // An earlier POST of the identical op named this
                        // entry (the Send's ids are its hash, and a may-have-
                        // been-sent op keeps its nonce): it may still land,
                        // and a later POST's "not sent" takes nothing from
                        // it (082 second review).
                        return false;
                    }
                    entry.record_ids.retain(|id| !record_ids.contains(id));
                    entry.record_ids.is_empty()
                });
                if emptied {
                    model.entries.remove(&key);
                }
                render()
            }
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
                    EntryStatus::Pending if entry.relay_funding() => TrackStatus::RelayFunding,
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
                    relay_sent_at_ms: entry.relay_sent_at_ms,
                    sender: entry.sender.clone(),
                    refusal: (status == TrackStatus::Rejected)
                        .then_some(entry.refusal)
                        .flatten(),
                    refusal_key: (status == TrackStatus::Rejected)
                        .then(|| refusal_key(entry.refusal).to_owned()),
                    stalled: entry.stalled && !entry.status.is_terminal(),
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
    (maybe_sent, submit_block, admitted): (bool, Option<u64>, bool),
    sender: Option<String>,
) -> Command<TrackEffect, Event> {
    let key = normalize(user_op_hash);
    let attempt = model.attempt;
    // A verdict that nothing was sent (NotSent, Rejected) does not outlive a
    // new submit of the identical op — same nonce (never bumped for a
    // may-have-been-sent op, RA5), calldata, gas and fees, so the same hash.
    // New records mark the new submission; the same hand-off again is an
    // echo and changes nothing. Merged into the dead entry, the new op would
    // go unpolled and read "not sent" at once, over an op the relay holds.
    //
    // Spec 082 round 2 (review): the write-ahead hands the op over BEFORE
    // its POST, so a slow POST can outlast the not-found grace. `NotSent`
    // judges an op the relay never showed it holds; the relay then taking it
    // (`admitted`) proves that verdict came too early — and an admitted op is
    // never in doubt again, so this can never undo a real `NotSent`. The
    // entry lives again (a `Rejected` after the relay took it stays: DX-W3).
    //
    // Spec 082 round 2 (second review): the write-ahead's hand-off names no
    // record — the op before its POST (`posting`). Its records come with the
    // POST's verdict, and the not-found grace counts from that verdict: a
    // relay cannot have written down bytes that have not left. A POST of the
    // identical op after a `NotSent` / `Rejected` is a new submission too.
    let posting = record_ids.is_empty() && !admitted;
    let new_life = model.entries.get(&key).is_some_and(|entry| {
        let refused_or_unsent =
            matches!(entry.status, EntryStatus::NotSent | EntryStatus::Rejected);
        (refused_or_unsent
            && (posting || record_ids.iter().any(|id| !entry.record_ids.contains(id))))
            || (admitted && entry.status == EntryStatus::NotSent)
    });
    if new_life {
        model.entries.remove(&key);
    }
    let fresh = !model.entries.contains_key(&key);
    let entry = model
        .entries
        .entry(key.clone())
        .or_insert_with(|| Entry::new(chain_id, None));
    if fresh {
        // Seen submitted: it starts where the relay takes an op, queued — so
        // a first answer that says more (on the network) is a move.
        entry.progress_mark = Some((TrackLifecycle::Queued, None, false));
    }
    if posting {
        // A POST of this op is about to leave: held off from "not sent"
        // until its verdict. An earlier POST's entry of the identical op
        // stays what it is otherwise — this one only ever adds to it.
        if !entry.status.is_terminal() {
            entry.posting = true;
            entry.not_found_streak = 0;
        }
        entry.from_posting |= fresh;
    } else if entry.posting || entry.from_posting {
        // The POST's verdict: the op is out, or may be. The relay's grace
        // starts now (stamped with the next clock).
        entry.posting = false;
        entry.from_posting = false;
        entry.not_found_streak = 0;
        entry.grace_from_ms = Some(None);
    }
    let named: Vec<String> = record_ids
        .iter()
        .filter(|id| !entry.record_ids.contains(id))
        .cloned()
        .collect();
    for id in record_ids {
        entry.merge_record_id(id);
    }
    entry.merge_submit_facts(maybe_sent, submit_block);
    entry.merge_sender(sender);
    if admitted {
        // The relay took it (RJ1): an ordinary op from here on.
        entry.acknowledged = true;
        entry.not_found_streak = 0;
    }
    // Records named after the op already landed (the chain showed it while
    // the POST was out) take the landing's patch — the one they missed.
    let late_patch = match &entry.status {
        _ if named.is_empty() => None,
        EntryStatus::Confirmed { tx_hash } => Some(TrackRecordPatch {
            status: TrackRecordStatus::Confirmed,
            tx_hash: Some(tx_hash.clone()),
            settlement: TrackSettlement::moved(entry.moved.clone()),
        }),
        EntryStatus::Failed { .. } => Some(TrackRecordPatch {
            status: TrackRecordStatus::Failed,
            tx_hash: None,
            settlement: TrackSettlement::failed(TrackFailure::Reverted),
        }),
        _ => None,
    };

    // First receipt poll goes out immediately, like `waitForReceipt`'s first
    // loop iteration. A second consumer of an already-tracked hash joins the
    // shared request/cooldown instead (invariant ⑤) — its polls resume on
    // the next Tick under the 3s throttle. No clock is known yet, so this one
    // poll goes out unstamped; the first clock seen stamps it (issue #464).
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
    if let Some(patch) = late_patch {
        commands.push(shell_request(
            attempt,
            TrackOperation::UpdateTxRecords { ids: named, patch },
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
            // inside a "successful" UserOp is a failed payment (#D1) — inside
            // THIS op's execution, not a bundle neighbour's (083).
            if op_execution_failed(&logs, &user_op_hash) {
                return accept(
                    model,
                    TrackShellResult::ReceiptFailed {
                        user_op_hash,
                        tx_hash,
                        now_ms,
                    },
                );
            }
            // What it moved, from these same authentic logs (spec 097 N5).
            let moved = proven_moves(&logs, &user_op_hash);
            on_receipt(model, &user_op_hash, tx_hash, now_ms, moved)
        }
        TrackShellResult::Receipt {
            user_op_hash,
            tx_hash,
            now_ms,
        } => on_receipt(model, &user_op_hash, tx_hash, now_ms, None),
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
            entry.last_receipt_poll_ms.get_or_insert(now_ms);
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
            fail_records(model.attempt, ids, landed, TrackFailure::Reverted)
        }

        // -- non-answers ------------------------------------------------------
        TrackShellResult::ReceiptPending {
            user_op_hash,
            now_ms,
        } => {
            let key = normalize(&user_op_hash);
            if let Some(entry) = model.entries.get_mut(&key) {
                entry.receipt_in_flight = false;
                entry.last_receipt_poll_ms.get_or_insert(now_ms);
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
                entry.last_receipt_poll_ms.get_or_insert(now_ms);
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
            rejection_reason,
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
                let sent = entry.relay_tx_hash.is_some()
                    || matches!(status, TrackLifecycle::Submitted | TrackLifecycle::Included);
                if sent && entry.relay_sent_at_ms.is_none() {
                    entry.relay_sent_at_ms = Some(now_ms);
                }
                if status != TrackLifecycle::NotFound {
                    entry.forgotten_since_ms = None;
                    entry.forgotten_streak = 0;
                }
                if matches!(status, TrackLifecycle::Rejected | TrackLifecycle::Failed) {
                    entry.refusal =
                        RefusalReason::of(rejection_reason.as_deref(), stage.as_deref())
                            .or(entry.refusal);
                }
                entry.heard(status, stage.as_deref(), now_ms);
                entry.last_status = Some((status, stage));
                if status == TrackLifecycle::Rejected && entry.relay_tx_hash.is_some() {
                    // The relay marks every op of a MINED bundle `rejected`
                    // unless the op's own event succeeded (both relay
                    // shells' `mark_bundle_confirmed`), and names that
                    // bundle tx: an on-chain revert, gas spent — never
                    // "refused, nothing was sent" (RJ3's words, ruling 9's
                    // tx-hash answer, RE8's balance read). The chain decides
                    // through the tx's receipt: the op's own event fails (or
                    // confirms) it; the tx mined without it upholds the
                    // refusal (082 second review).
                    entry.acknowledged = true;
                    entry.not_found_streak = 0;
                    entry.relay_rejected = true;
                    None
                } else if status == TrackLifecycle::Rejected {
                    // The relay refused it before any block: nothing was
                    // sent, nothing will land. Terminal, immediately (③).
                    entry.acknowledged = true;
                    entry.status = EntryStatus::Rejected;
                    Some((entry.record_ids.clone(), TrackFailure::Refused))
                } else if status == TrackLifecycle::NotFound {
                    // Spec 082 RA4: only a may-have-been-sent op the relay has
                    // never shown it holds is ended by `not_found` — a plain
                    // op's stays inert (079), and so does one the relay has
                    // already acknowledged. Inside the grace it is ignored.
                    // The grace counts from the POST's verdict when the
                    // write-ahead handed the op over before it; while a POST
                    // is out, `not_found` says nothing (082 second review).
                    let past_grace = entry
                        .grace_age(now_ms)
                        .is_some_and(|age| age >= NOT_FOUND_GRACE_MS);
                    if entry.in_doubt() && past_grace {
                        entry.not_found_streak = entry.not_found_streak.saturating_add(1);
                    }
                    // Ruling 8: the relay's word alone is not enough — the chain
                    // must have been read up to its head with no event for this
                    // op, or a relay that lost track of a landed op would say
                    // "not sent" over money that moved.
                    // Spec 099 R6: an op the relay acknowledged and has since
                    // forgotten (its record expired) is not "on its way" for a
                    // day. The same proof ends it — repeated `not_found`, the
                    // chain read to its head with no event — after a longer
                    // wait. Never one whose bundle tx the relay named: that
                    // tx's receipt decides (RJ4).
                    let forgotten = !entry.in_doubt()
                        && entry.acknowledged
                        && entry.relay_tx_hash.is_none()
                        && !entry.posting
                        && past_grace;
                    if forgotten {
                        entry.forgotten_since_ms.get_or_insert(now_ms);
                        entry.forgotten_streak = entry.forgotten_streak.saturating_add(1);
                    }
                    let forgotten_long_enough = forgotten
                        && entry.forgotten_streak >= NOT_FOUND_CONFIRMATIONS
                        && entry
                            .forgotten_since_ms
                            .is_some_and(|since| now_ms - since >= FORGOTTEN_NOT_SENT_MS);
                    if (entry.in_doubt()
                        && !entry.posting
                        && entry.not_found_streak >= NOT_FOUND_CONFIRMATIONS
                        && entry.find.caught_up())
                        || (forgotten_long_enough && entry.find.caught_up())
                    {
                        entry.status = EntryStatus::NotSent;
                        Some((entry.record_ids.clone(), TrackFailure::NotSent))
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
                Some((ids, failure)) => fail_records(model.attempt, ids, None, failure),
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
                // …and still knows whose op it is (one in flight per account
                // and chain, `in_flight_ops`).
                entry.merge_sender(record.sender);
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

        TrackShellResult::TxReceipt {
            user_op_hash,
            now_ms,
            receipt_json,
        } => on_tx_receipt(model, &user_op_hash, now_ms, receipt_json.as_deref()),

        // Acks — nothing may change.
        TrackShellResult::RecordsPatched | TrackShellResult::Notified => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Scheduler — one pass per clock reading
// ---------------------------------------------------------------------------

/// Is a receipt read (the relay's receipt, or the bundle tx's from the
/// chain) due? `last` is when the previous one was ISSUED; `None` = never.
/// [`RECEIPT_TICK_SLACK_MS`] absorbs a tick's jitter.
fn receipt_due(last: Option<f64>, now_ms: f64, interval: f64) -> bool {
    last.is_none_or(|last| now_ms - last >= interval - RECEIPT_TICK_SLACK_MS)
}

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

        // The hold's time limit: no progress for ten minutes releases the
        // account's nonce (`in_flight_ops`).
        if in_flight_stalled(entry.progressed_at_ms.unwrap_or(submitted_at), now_ms) {
            entry.stalled = true;
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
        // shared (⑤). Stamped at issue (issue #464), so a 3 s tick asks
        // every 3 s.
        let in_window = !entry.window_closed && !entry.aborted;
        let receipt_interval = receipt_interval_ms(in_window, age);
        if !entry.receipt_in_flight
            && receipt_due(entry.last_receipt_poll_ms, now_ms, receipt_interval)
        {
            entry.receipt_in_flight = true;
            entry.last_receipt_poll_ms = Some(now_ms);
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
        //
        // Spec 099 R6: the first ask comes 3 s after acceptance, and past the
        // window every pending op is asked at the receipt pace — an op the
        // relay acknowledged can still be forgotten by it, and only its
        // `not_found` says so.
        let status_interval = if in_window && !entry.status_asked {
            Some(FIRST_STATUS_POLL_MS)
        } else if in_window {
            Some(STATUS_POLL_INTERVAL_MS)
        } else {
            Some(receipt_interval)
        };
        let status_due = status_interval.is_some_and(|interval| {
            !entry.status_in_flight
                && entry
                    .last_status_poll_ms
                    .is_none_or(|last| now_ms - last >= interval)
        });
        if status_due {
            entry.status_in_flight = true;
            entry.status_asked = true;
            entry.last_status_poll_ms = Some(now_ms);
            commands.push(shell_request(
                attempt,
                TrackOperation::PollStatus {
                    user_op_hash: hash.clone(),
                    chain_id: entry.chain_id,
                },
            ));
        }

        // RJ4: the relay named the bundle tx — read its receipt from the
        // chain at the receipt cadence, one request per hash in flight,
        // stamped at issue like the receipt poll.
        if let Some(tx_hash) = entry.relay_tx_hash.clone() {
            let due = !entry.tx_receipt_in_flight
                && receipt_due(entry.last_tx_receipt_ms, now_ms, receipt_interval);
            if due {
                entry.tx_receipt_in_flight = true;
                entry.last_tx_receipt_ms = Some(now_ms);
                commands.push(shell_request(
                    attempt,
                    TrackOperation::TxReceipt {
                        chain_id: entry.chain_id,
                        tx_hash,
                        user_op_hash: hash.clone(),
                    },
                ));
            }
        }

        // Ruling 8: while the relay has not shown it holds a may-have-been-
        // sent op, read the chain for the op's own event, on the same
        // cadence as the status polls.
        let find_interval = if in_window {
            STATUS_POLL_INTERVAL_MS
        } else {
            receipt_interval
        };
        if entry.scanning()
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
    match find_in_log_array(logs, user_op_hash) {
        Some(event) => Found::Landed {
            tx_hash: event.tx_hash,
            success: event.success,
        },
        None => Found::Nothing,
    }
}

/// The op's own `UserOperationEvent` in a log array, with where it sits.
struct OpEventLog {
    /// Its position in the array.
    index: usize,
    tx_hash: String,
    success: bool,
}

fn log_text(log: &serde_json::Value, key: &str) -> String {
    log.get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn log_topics(log: &serde_json::Value) -> Vec<String> {
    log.get("topics")
        .and_then(serde_json::Value::as_array)
        .map(|topics| {
            topics
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// A live (not `removed`) `UserOperationEvent` of the EntryPoint — any op's.
fn is_op_event(log: &serde_json::Value) -> bool {
    log.get("removed").and_then(serde_json::Value::as_bool) != Some(true)
        && log_text(log, "address").eq_ignore_ascii_case(crate::safe::ENTRY_POINT)
        && log_topics(log).first().is_some_and(|topic| {
            topic.eq_ignore_ascii_case(crate::user_op::USER_OPERATION_EVENT_TOPIC)
        })
}

/// The find-event reader: the EntryPoint's `UserOperationEvent` whose
/// `topics[1]` is this op's hash. `data` is `(uint256 nonce, bool success,
/// uint256 actualGasCost, uint256 actualGasUsed)`; `success` is its second
/// word. An event whose tx hash or success word cannot be read is no verdict.
fn find_in_log_array(logs: &[serde_json::Value], user_op_hash: &str) -> Option<OpEventLog> {
    logs.iter().enumerate().find_map(|(index, log)| {
        if !is_op_event(log) {
            return None;
        }
        let ours = log_topics(log)
            .get(1)
            .is_some_and(|topic| topic.eq_ignore_ascii_case(user_op_hash));
        if !ours {
            return None;
        }
        let tx_hash = log_text(log, "transactionHash");
        let data = log_text(log, "data");
        let data = data.strip_prefix("0x").unwrap_or(&data);
        let word = data.get(64..128)?;
        if tx_hash.is_empty() || !word.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        Some(OpEventLog {
            index,
            tx_hash,
            success: word.bytes().any(|b| b != b'0'),
        })
    })
}

/// A definitive successful receipt for `user_op_hash`: the op confirms, with
/// what its own logs prove it moved when they were read (`moved`).
fn on_receipt(
    model: &mut Model,
    user_op_hash: &str,
    tx_hash: String,
    now_ms: f64,
    moved: Option<Vec<TrackMove>>,
) -> Command<TrackEffect, Event> {
    let key = normalize(user_op_hash);
    let Some(entry) = model.entries.get_mut(&key) else {
        return Command::done();
    };
    entry.receipt_in_flight = false;
    entry.last_receipt_poll_ms.get_or_insert(now_ms);
    entry.saw_clean_response = true;
    entry.acknowledged = true;
    if entry.status.is_terminal() {
        // Already resolved by another path — never double-resolve.
        return Command::done();
    }
    entry.relay_sent_at_ms.get_or_insert(now_ms);
    entry.status = EntryStatus::Confirmed {
        tx_hash: tx_hash.clone(),
    };
    entry.moved = moved;
    let ids = entry.record_ids.clone();
    let chain_id = entry.chain_id;
    let moved = entry.moved.clone();
    confirm_records(model.attempt, key, chain_id, ids, tx_hash, moved)
}

/// A bundle transaction's receipt, read for this op (RJ4). The op's own
/// logs are the ones after the previous op's `UserOperationEvent` (or the
/// start) up to its own — how the EntryPoint orders a bundle's execution —
/// so another op's Safe failing in the same bundle is not this op failing.
fn on_tx_receipt(
    model: &mut Model,
    user_op_hash: &str,
    now_ms: f64,
    receipt_json: Option<&str>,
) -> Command<TrackEffect, Event> {
    let key = normalize(user_op_hash);
    let attempt = model.attempt;
    let Some(entry) = model.entries.get_mut(&key) else {
        return Command::done();
    };
    entry.tx_receipt_in_flight = false;
    entry.last_tx_receipt_ms.get_or_insert(now_ms);
    if entry.status.is_terminal() {
        return Command::done(); // resolved meanwhile — never double-resolve
    }
    // No answer, not mined (`null`), or not a receipt: ask again later.
    let receipt =
        receipt_json.and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok());
    let receipt = receipt
        .as_ref()
        .map(|value| value.get("result").unwrap_or(value));
    let Some(logs) = receipt
        .and_then(|receipt| receipt.get("logs"))
        .and_then(serde_json::Value::as_array)
    else {
        return render();
    };
    let Some(event) = find_in_log_array(logs, &key) else {
        let the_named_tx = receipt
            .and_then(|receipt| receipt.get("transactionHash"))
            .and_then(serde_json::Value::as_str)
            .zip(entry.relay_tx_hash.as_deref())
            .is_some_and(|(mined, named)| mined.eq_ignore_ascii_case(named));
        if entry.relay_rejected && the_named_tx {
            // The relay said `rejected` and named this tx, and the tx is
            // mined without the op: it never reached a block. The refusal
            // stands — nothing landed, so no balance read.
            entry.status = EntryStatus::Rejected;
            let ids = entry.record_ids.clone();
            return fail_records(attempt, ids, None, TrackFailure::Refused);
        }
        // Mined without this op's event (a replaced bundle): proves nothing.
        return render();
    };
    let start = logs[..event.index]
        .iter()
        .rposition(is_op_event)
        .map_or(0, |previous| previous + 1);
    let own_logs: Vec<super::token_trust::TrustReceiptLog> = logs[start..=event.index]
        .iter()
        .map(|log| super::token_trust::TrustReceiptLog {
            address: log_text(log, "address"),
            topics: log_topics(log),
            data: log_text(log, "data"),
        })
        .collect();
    entry.acknowledged = true;
    let ids = entry.record_ids.clone();
    let chain_id = entry.chain_id;
    if event.success && !safe_execution_failed(&own_logs) {
        entry.status = EntryStatus::Confirmed {
            tx_hash: event.tx_hash.clone(),
        };
        entry.moved = proven_moves(&own_logs, &key);
        confirm_records(
            attempt,
            key,
            chain_id,
            ids,
            event.tx_hash,
            entry.moved.clone(),
        )
    } else {
        entry.status = EntryStatus::Failed {
            tx_hash: event.tx_hash,
        };
        fail_records(attempt, ids, Some(chain_id), TrackFailure::Reverted)
    }
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
                // Reaches the submit only while the op is young enough (or
                // the lookback reaches the genesis).
                let age = entry.submitted_at_ms.map(|at| now_ms - at);
                entry.find.anchored = head <= FIND_OP_LOOKBACK_BLOCKS
                    || age.is_some_and(|age| age <= FIND_OP_LOOKBACK_COVERS_MS);
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
                // The event alone names no transfer: nothing is claimed
                // about what moved (`moved: None`), never "nothing moved".
                return if success {
                    entry.status = EntryStatus::Confirmed {
                        tx_hash: tx_hash.clone(),
                    };
                    confirm_records(attempt, key, chain_id, ids, tx_hash, None)
                } else {
                    entry.status = EntryStatus::Failed { tx_hash };
                    fail_records(attempt, ids, Some(chain_id), TrackFailure::Reverted)
                };
            }
            Found::Nothing => {
                // A clean read of the full width shows the node takes it:
                // grow back toward the widest window. At the head the ask
                // is only the new blocks, so one range error there (often a
                // load-balanced node a block behind) would otherwise pin the
                // scan to a sliver for good — and on a fast chain a catch-up
                // of one block per read never reaches the head.
                let asked = to.saturating_sub(from).saturating_add(1);
                if asked >= entry.find.width {
                    entry.find.width = entry.find.width.saturating_mul(2).min(FIND_OP_MAX_RANGE);
                }
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
    if !entry.scanning() || entry.find.next_window().is_none() {
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

/// keccak256("SafeReceived(address,uint256)") — a Safe's `receive()`: the
/// chain's own coin arriving, logged by the account itself. Pinned against
/// the core's own keccak by a test below.
pub const SAFE_RECEIVED_TOPIC: &str =
    "0x3d0ce9bfc3ed7d6862dbb28b2dea94561fe714a1b4d019aa8af39730d1ad7c3d";

/// What a landed operation moved for the account that sent it, read from
/// the op's OWN receipt logs (spec 097 N5) — the figures Activity shows for
/// it, never a sign-time simulation's.
///
/// The account is the `sender` the EntryPoint's own `UserOperationEvent` for
/// this op names: no such event, no reading (`None`) — logs without it give
/// nothing to stand on. Only the op's execution counts, as
/// [`user_op_outcome_in_logs`] scopes it: the logs after the last EntryPoint
/// boundary (`BeforeExecution`, or the previous op's event) up to its own
/// event, so a neighbour's swap in the same bundle, or the validation
/// phase's deposit, is never ours.
///
/// Counted: every ERC-20 `Transfer` from or to the account, netted per token
/// ([`super::token_trust::derive_asset_deltas`]); and the chain's coin
/// arriving, which the account logs itself (`SafeReceived` emitted BY it —
/// no other contract can write that log). Not counted: the coin the call
/// itself sent, which no log states (the record's own `value` does), nor an
/// EIP-7708 native line, which would state it a second time. A token whose
/// net is zero moved nothing and has no line.
pub fn proven_moves(
    logs: &[super::token_trust::TrustReceiptLog],
    user_op_hash: &str,
) -> Option<Vec<TrackMove>> {
    let hex = |bytes: &[u8]| crate::primitives::to_hex(&crate::primitives::keccak256(bytes), true);
    let event = hex(USER_OPERATION_EVENT.as_bytes());
    let before_execution = hex(b"BeforeExecution()");
    let names = |log: &super::token_trust::TrustReceiptLog, index: usize, want: &str| {
        log.topics
            .get(index)
            .is_some_and(|value| value.eq_ignore_ascii_case(want))
    };
    let mut start = 0;
    let mut found = None;
    for (index, log) in logs.iter().enumerate() {
        if !log.address.eq_ignore_ascii_case(crate::safe::ENTRY_POINT) {
            continue;
        }
        if names(log, 0, &before_execution) {
            start = index + 1;
        } else if names(log, 0, &event) {
            if names(log, 1, user_op_hash) {
                found = Some((index, log.topics.get(2)?));
                break;
            }
            start = index + 1;
        }
    }
    let (end, sender_topic) = found?;
    let sender = address_of_topic(sender_topic)?;
    let own = &logs[start..end];

    let mut moved: Vec<TrackMove> = super::token_trust::derive_asset_deltas(own, &sender)
        .into_iter()
        .filter(|delta| !delta.is_native)
        .filter_map(|delta| {
            Some(TrackMove {
                token: Some(delta.token?),
                delta: delta.delta.to_string(),
            })
        })
        .collect();
    let mut arrived: u128 = 0;
    for log in own {
        if log.address.eq_ignore_ascii_case(&sender) && names(log, 0, SAFE_RECEIVED_TOPIC) {
            let data = log.data.strip_prefix("0x").unwrap_or(&log.data);
            let word = data.get(..64).unwrap_or(data);
            // A word that will not read, or a sum past u128, states no
            // figure: the coin line is left out rather than guessed.
            match u128::from_str_radix(word, 16)
                .ok()
                .and_then(|value| arrived.checked_add(value))
            {
                Some(sum) => arrived = sum,
                None => {
                    arrived = 0;
                    break;
                }
            }
        }
    }
    if arrived > 0 {
        moved.push(TrackMove {
            token: None,
            delta: arrived.to_string(),
        });
    }
    Some(moved)
}

/// The address an indexed topic holds: its last 20 bytes, lower-case. `None`
/// for a topic that is not a 32-byte word with its top 12 bytes zero.
fn address_of_topic(topic: &str) -> Option<String> {
    let hex = topic.strip_prefix("0x").unwrap_or(topic);
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if hex[..24].bytes().any(|b| b != b'0') {
        return None;
    }
    Some(format!("0x{}", hex[24..].to_ascii_lowercase()))
}

/// How `user_op_hash` ended, by the logs of the bundle transaction that
/// carried it (083): `Some(true)` executed; `Some(false)` when the
/// EntryPoint's `UserOperationEvent` for it says `success: false`, or when a
/// Safe `ExecutionFailure` was logged inside the op's OWN execution (#D1).
/// `None` when the logs carry no EntryPoint event for the op.
///
/// A bundle holds several operations, so only the op's own execution logs
/// count: the EntryPoint (v0.7) validates every op, emits `BeforeExecution`,
/// then runs each op and closes it with its `UserOperationEvent` — the logs
/// since the previous boundary are that op's. Only the EntryPoint's own logs
/// are boundaries; any contract can emit a look-alike.
pub fn user_op_outcome_in_logs(
    logs: &[super::token_trust::TrustReceiptLog],
    user_op_hash: &str,
) -> Option<bool> {
    let hex = |bytes: &[u8]| crate::primitives::to_hex(&crate::primitives::keccak256(bytes), true);
    let event = hex(USER_OPERATION_EVENT.as_bytes());
    let before_execution = hex(b"BeforeExecution()");
    let mut execution_failed = false;
    for log in logs {
        let names = |index: usize, want: &str| {
            log.topics
                .get(index)
                .is_some_and(|value| value.eq_ignore_ascii_case(want))
        };
        let from_entry_point = log.address.eq_ignore_ascii_case(crate::safe::ENTRY_POINT);
        if from_entry_point && names(0, &before_execution) {
            execution_failed = false;
        } else if from_entry_point && names(0, &event) {
            if names(1, user_op_hash) {
                // Unindexed: nonce, success, actualGasCost, actualGasUsed.
                let data = log.data.strip_prefix("0x")?;
                let success = data.get(64..128)?.bytes().any(|digit| digit != b'0');
                return Some(success && !execution_failed);
            }
            execution_failed = false;
        } else if names(0, SAFE_EXECUTION_FAILURE_TOPIC) {
            execution_failed = true;
        }
    }
    None
}

/// Did THIS operation fail inside a receipt the relay called a success?
/// Scoped to the op's own execution when the logs carry its
/// `UserOperationEvent` — another account's `ExecutionFailure` in the same
/// bundle is not ours, and calling our swap failed would invite the user to
/// send it twice (083). Logs without that event (a relay that returns only
/// the op's own) are read whole, as [`safe_execution_failed`] always did.
pub fn op_execution_failed(
    logs: &[super::token_trust::TrustReceiptLog],
    user_op_hash: &str,
) -> bool {
    match user_op_outcome_in_logs(logs, user_op_hash) {
        Some(executed) => !executed,
        None => safe_execution_failed(logs),
    }
}

/// The EntryPoint's word on how one operation inside a bundle ended:
/// `UserOperationEvent(bytes32 indexed userOpHash, address indexed sender,
/// address indexed paymaster, uint256 nonce, bool success, uint256
/// actualGasCost, uint256 actualGasUsed)`.
const USER_OPERATION_EVENT: &str =
    "UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)";

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
        | TrackShellResult::OpEvent { now_ms, .. }
        | TrackShellResult::TxReceipt { now_ms, .. } => Some(*now_ms),
        TrackShellResult::RecordsPatched | TrackShellResult::Notified => None,
    }
}

/// Entries created before any clock was observed get stamped by the first
/// result that carries one; `last_status_poll` starts at submission
/// (`lastStatusAt = start`) so the first status poll waits a full interval.
/// The receipt poll that went out with the op is stamped the same way: it
/// was issued then, and the cooldown counts from the issue (issue #464).
fn stamp_unstamped(model: &mut Model, now_ms: f64) {
    for entry in model.entries.values_mut() {
        if entry.submitted_at_ms.is_none() {
            entry.submitted_at_ms = Some(now_ms);
            entry.last_status_poll_ms = Some(now_ms);
            entry.find.last_ms = Some(now_ms);
            if entry.receipt_in_flight {
                entry.last_receipt_poll_ms.get_or_insert(now_ms);
            }
        }
        // The POST's verdict came: the not-found grace counts from here.
        if entry.grace_from_ms == Some(None) {
            entry.grace_from_ms = Some(Some(now_ms));
        }
    }
}

/// The ONLY constructor of a `failed` patch — reachable from a `success ===
/// false` receipt, a relay rejection (③) and, since spec 082, a relay that
/// never admitted a may-have-been-sent op (RA4). Timeouts, aborts, age and
/// unreachable bundlers can never arrive here (①). `landed` names the chain
/// when the op reached a block (gas spent): the holdings moved (RE8).
///
/// `failure` is why, kept with the records (spec 097 N4): the words the
/// request ended with, which Activity says beside "Failed".
fn fail_records(
    attempt: u64,
    ids: Vec<String>,
    landed: Option<u32>,
    failure: TrackFailure,
) -> Command<TrackEffect, Event> {
    let mut commands = vec![shell_request(
        attempt,
        TrackOperation::UpdateTxRecords {
            ids,
            patch: TrackRecordPatch {
                status: TrackRecordStatus::Failed,
                tx_hash: None,
                settlement: TrackSettlement::failed(failure),
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
/// logs to token_trust — and then the balance (spec 082 RE8). `moved` is
/// what its own receipt logs prove it moved (spec 097 N5), kept with the
/// records; `None` when no logs were read.
fn confirm_records(
    attempt: u64,
    user_op_hash: String,
    chain_id: u32,
    ids: Vec<String>,
    tx_hash: String,
    moved: Option<Vec<TrackMove>>,
) -> Command<TrackEffect, Event> {
    Command::all([
        shell_request(
            attempt,
            TrackOperation::UpdateTxRecords {
                ids,
                patch: TrackRecordPatch {
                    status: TrackRecordStatus::Confirmed,
                    tx_hash: Some(tx_hash.clone()),
                    settlement: TrackSettlement::moved(moved),
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

    /// 083: a bundle carries other accounts' operations. Only the logs of
    /// THIS op's execution — since the last EntryPoint boundary — can fail
    /// it; a neighbour's `ExecutionFailure` cannot, and an impostor's
    /// look-alike event is no boundary.
    #[test]
    fn only_the_operations_own_execution_can_fail_it() {
        let entry_point = crate::safe::ENTRY_POINT.to_lowercase();
        let hex =
            |bytes: &[u8]| crate::primitives::to_hex(&crate::primitives::keccak256(bytes), true);
        let event_topic = hex(USER_OPERATION_EVENT.as_bytes());
        assert_eq!(
            event_topic,
            "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f"
        );
        let ours = format!("0x{}", "ab".repeat(32));
        let theirs = format!("0x{}", "ef".repeat(32));
        let word = |value: u8| format!("{value:064x}");
        let log = |address: &str, topics: Vec<String>, data: String| TrustReceiptLog {
            address: address.to_owned(),
            topics,
            data,
        };
        let event = |op: &str, address: &str, success: u8| {
            log(
                address,
                vec![event_topic.clone(), op.to_owned(), word(1), word(0)],
                format!("0x{}{}{}{}", word(7), word(success), word(9), word(9)),
            )
        };
        let before_execution = log(&entry_point, vec![hex(b"BeforeExecution()")], "0x".into());
        let failure = log(
            "0x88cca0eedbf2c4426110bbfc998f048689266894",
            vec![SAFE_EXECUTION_FAILURE_TOPIC.to_owned()],
            format!("0x{}{}", word(1), word(0)),
        );

        let neighbour_failed = [
            failure.clone(), // validation-phase noise, before execution
            before_execution,
            failure.clone(),
            event(&theirs, &entry_point, 1),
            event(&ours, &entry_point, 1),
        ];
        assert_eq!(
            user_op_outcome_in_logs(&neighbour_failed, &theirs),
            Some(false)
        );
        assert_eq!(
            user_op_outcome_in_logs(&neighbour_failed, &ours),
            Some(true)
        );
        assert!(op_execution_failed(&neighbour_failed, &theirs));
        assert!(
            !op_execution_failed(&neighbour_failed, &ours),
            "another op's failure is not this one's"
        );
        assert!(
            safe_execution_failed(&neighbour_failed),
            "the whole-bundle rule would have failed it"
        );

        let reverted = [event(&ours, &entry_point, 0)];
        assert!(op_execution_failed(&reverted, &ours));
        let capitals = format!("0x{}", "AB".repeat(32));
        assert_eq!(
            user_op_outcome_in_logs(&[event(&ours, crate::safe::ENTRY_POINT, 1)], &capitals),
            Some(true),
            "hex case is not identity"
        );

        // No event for the op: the logs are read whole, as before.
        let impostor = [
            failure.clone(),
            event(&ours, "0x1111111111111111111111111111111111111111", 1),
        ];
        assert_eq!(user_op_outcome_in_logs(&impostor, &ours), None);
        assert!(op_execution_failed(&impostor, &ours));
        assert!(op_execution_failed(std::slice::from_ref(&failure), &ours));
        assert!(!op_execution_failed(&[], &ours));
    }
}
