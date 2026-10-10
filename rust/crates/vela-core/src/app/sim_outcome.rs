//! What a transaction simulation's answer MEANS for the signing sheet (spec 082, L-D5).
//!
//! Pure. The pool's raw `eth_simulateV1` reply (an answer, a JSON-RPC error,
//! or no answer at all) becomes one outcome: the balance deltas it shows, a
//! revert with an untrusted reason made safe to print, "this node does not
//! offer simulation", or "unreachable" — and the notice each one draws.
//! The desktop, iOS and Android parsers move here so the severity is one rule.
//!
//! ```text
//! pool gave up                           ─► Unreachable ─► Caution  simUnavailableWarning
//! JSON-RPC error, or no call results     ─► NotOffered  ─► Caution  simUnavailableWarning
//! any call with status 0x0 or an error   ─► Reverts     ─► Danger   simWillFailReason | simWillFail
//! a status neither 0 nor 1 (no error)    ─► NotOffered  ─► Caution  simUnavailableWarning
//! otherwise, something of theirs moves   ─► Deltas      ─► none (token_trust draws the balances)
//! otherwise, nothing of theirs moves     ─► Deltas []   ─► quiet    simResultNoChange
//! ```
//!
//! ## "No asset changes" is said, and it is one line (PR 3)
//!
//! A checked answer under which nothing of the person's moves is a verdict
//! too, and the place the sheet keeps for the verdict says it:
//! [`KEY_NO_CHANGE`]. The desktop and Android said it, iOS said another
//! sentence ("No assets leave your wallet") and the web said nothing. Which
//! line, and when, is [`no_change_key`] — carried by the outcome
//! ([`SimOutcome::no_change_key`], the UniFFI and wasm records) and by the
//! judged view the sheets draw from (`token_trust::TrustSimView`).
//!
//! ## Why the severity is one rule (research RG6)
//!
//! Before 082 each shell collapsed every non-answer into "unavailable" and
//! drew it in its own tone — danger on iOS and the desktop, caution on
//! Android — so a public node that simply does not implement
//! `eth_simulateV1` (Arbitrum's answers `-32603 "method handler crashed"`)
//! looked exactly like "this transaction will fail". And a real revert was
//! skipped: its logs were dropped and the sheet said nothing moves. Here a
//! node that cannot check is a caution, and a call the chain says fails is a
//! danger, on every client.
//!
//! ## A revert reason is untrusted text (research RG8)
//!
//! The contract the dApp chose writes it, and it is drawn on a signing sheet.
//! Only the standard `Error(string)` payload is read; a panic, a custom error
//! or anything else has no reason and the plain sentence is shown. The text
//! loses the characters no name may carry (controls, bidi overrides, zero
//! width and the other invisible formatting codepoints), every run of
//! whitespace becomes one space, and it is capped at
//! [`REVERT_REASON_MAX_CHARS`] characters. A reason that still carries `$t(`
//! is dropped whole: the corpus engine expands that reference after
//! `{{reason}}` is filled in, so it would print a wallet sentence of the
//! contract's choosing inside the danger line.
//!
//! ## What it must never do
//!
//! Decide a write. The deltas are untrusted input by construction — a site can
//! emit any `Transfer` log it likes from a contract it controls — so which of
//! them may be shown with a confident amount stays `token_trust`'s judgment,
//! and nothing here may add a token to anybody's list (that entrance belongs
//! to the confirmed receipt alone).

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::clear_signing::ClearRisk;
use super::name_verify::is_never_in_a_name;
use super::token_trust::{TrustAssetDelta, TrustDeltaKind, NATIVE_LOG_ADDRESSES, TRANSFER_TOPIC};

/// The selector of Solidity's `Error(string)` revert payload.
pub const ERROR_STRING_SELECTOR: [u8; 4] = [0x08, 0xc3, 0x79, 0xa0];

/// The longest revert reason a sheet prints, in characters (research RG8).
pub const REVERT_REASON_MAX_CHARS: usize = 64;

/// What the corpus engine reads as a reference to another sentence
/// (`i18n::interpolate::find_nest`). A revert reason containing it is no
/// reason (see [`revert_reason`]).
const NESTING_OPENER: &str = "$t(";

/// The corpus key for a revert with a readable reason (`{{reason}}`).
pub const KEY_WILL_FAIL_REASON: &str = "componentsUi.signing.simWillFailReason";
/// The corpus key for a revert without one.
pub const KEY_WILL_FAIL: &str = "componentsUi.signing.simWillFail";
/// The corpus key for "Vela couldn't check what this transaction does".
pub const KEY_UNAVAILABLE: &str = "componentsUi.signing.simUnavailableWarning";
/// The corpus key for "No asset changes": checked, and nothing of the
/// person's moves ([`no_change_key`]).
pub const KEY_NO_CHANGE: &str = "componentsUi.signing.simResultNoChange";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The pool's answer to one `eth_simulateV1` call, normalised by the shell.
#[derive(Clone, Debug, PartialEq)]
pub enum SimReply {
    /// The JSON-RPC `result`, as the node sent it.
    Result(Value),
    /// A JSON-RPC error answer: the node spoke, and did not simulate.
    Error {
        code: Option<i64>,
        message: Option<String>,
    },
    /// No endpoint answered at all.
    Unreachable,
}

impl SimReply {
    /// The reply from its wire form (contract §8): the JSON-RPC envelope
    /// itself — `{"result": …}` or `{"error": {"code", "message"}}` — or
    /// `{"unreachable": true}` when the pool gave up.
    ///
    /// An error wins over anything else in the envelope; a shape that is none
    /// of the three is a node answer nobody can read, which is an error with
    /// no code (it draws the caution, never "nothing moves").
    pub fn from_envelope(envelope: &Value) -> SimReply {
        if let Some(error) = envelope.get("error").filter(|error| !error.is_null()) {
            return SimReply::Error {
                code: error.get("code").and_then(Value::as_i64),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .or_else(|| error.as_str())
                    .map(str::to_owned),
            };
        }
        if envelope.get("unreachable").and_then(Value::as_bool) == Some(true) {
            return SimReply::Unreachable;
        }
        match envelope.get("result") {
            Some(result) => SimReply::Result(result.clone()),
            None => SimReply::Error {
                code: None,
                message: None,
            },
        }
    }

    /// [`SimReply::from_envelope`] of a JSON string; text that is not JSON is
    /// an unreadable answer (an error with no code).
    pub fn from_json(text: &str) -> SimReply {
        match serde_json::from_str::<Value>(text) {
            Ok(envelope) => SimReply::from_envelope(&envelope),
            Err(_) => SimReply::Error {
                code: None,
                message: None,
            },
        }
    }
}

/// What the simulation says about the transaction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SimOutcome"))]
pub enum SimOutcome {
    /// Every call ran. The signed per-asset moves for the user — an empty
    /// list means it was checked and nothing of theirs moves.
    Deltas { deltas: Vec<TrustAssetDelta> },
    /// A call fails. `reason` is the sanitised `Error(string)` text, if any.
    Reverts { reason: Option<String> },
    /// The node answered and did not simulate (it does not offer the method,
    /// refused the params, crashed, or sent no call results).
    NotOffered,
    /// No node answered.
    Unreachable,
}

/// The line the signing sheet draws for an outcome.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SimNotice {
    pub risk: ClearRisk,
    /// The corpus key of the sentence.
    pub key: &'static str,
    /// The `{{reason}}` of [`KEY_WILL_FAIL_REASON`]; `None` for every other key.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// The classifier
// ---------------------------------------------------------------------------

/// The outcome of one simulation reply for `user` (the account signing).
///
/// A result must be an array of blocks carrying at least one call result;
/// anything less is [`SimOutcome::NotOffered`] — "could not check" — and never
/// "nothing moves", because we always ask about at least one call and a node
/// that reports none has not answered the question. A call the node did not
/// shape as an object is the same unreadable answer.
pub fn classify(reply: SimReply, user: &str) -> SimOutcome {
    let result = match reply {
        SimReply::Unreachable => return SimOutcome::Unreachable,
        SimReply::Error { .. } => return SimOutcome::NotOffered,
        SimReply::Result(result) => result,
    };
    let Some(blocks) = result.as_array() else {
        return SimOutcome::NotOffered;
    };
    let calls: Vec<&Value> = blocks
        .iter()
        .filter_map(|block| block.get("calls").and_then(Value::as_array))
        .flatten()
        .collect();
    if calls.is_empty() || calls.iter().any(|call| !call.is_object()) {
        return SimOutcome::NotOffered;
    }
    // The calls run in order inside one simulated block, so the FIRST failure
    // is the one the person needs to hear about; a later call's reason would
    // describe a world the first failure already ended. A failure the node
    // reported speaks even beside a call whose status nobody can read.
    if let Some(failed) = calls
        .iter()
        .find(|call| call_status(call) == CallStatus::Failed)
    {
        return SimOutcome::Reverts {
            reason: revert_reason(failed),
        };
    }
    // A status that is neither 0 nor 1 is an answer nobody can read: "could
    // not check", never a danger and never "nothing moves".
    if calls
        .iter()
        .any(|call| call_status(call) == CallStatus::Unreadable)
    {
        return SimOutcome::NotOffered;
    }
    let logs: Vec<&Value> = calls
        .iter()
        .filter_map(|call| call.get("logs").and_then(Value::as_array))
        .flatten()
        .collect();
    SimOutcome::Deltas {
        deltas: derive_deltas(&logs, user),
    }
}

/// The sheet's line for an outcome: a revert is a danger, a node that could
/// not check is a caution, and a checked answer draws nothing here (its
/// balances are `token_trust`'s block).
pub fn notice(outcome: &SimOutcome) -> Option<SimNotice> {
    match outcome {
        SimOutcome::Deltas { .. } => None,
        SimOutcome::Reverts {
            reason: Some(reason),
        } => Some(SimNotice {
            risk: ClearRisk::Danger,
            key: KEY_WILL_FAIL_REASON,
            reason: Some(reason.clone()),
        }),
        SimOutcome::Reverts { reason: None } => Some(SimNotice {
            risk: ClearRisk::Danger,
            key: KEY_WILL_FAIL,
            reason: None,
        }),
        SimOutcome::NotOffered | SimOutcome::Unreachable => Some(SimNotice {
            risk: ClearRisk::Caution,
            key: KEY_UNAVAILABLE,
            reason: None,
        }),
    }
}

impl SimOutcome {
    /// [`KEY_NO_CHANGE`] when this answer was checked and moves nothing of
    /// the person's; `None` for moves, and for every answer that is not a
    /// check (a revert, a node that could not or did not simulate).
    #[must_use]
    pub fn no_change_key(&self) -> Option<&'static str> {
        match self {
            SimOutcome::Deltas { deltas } => {
                no_change_key(deltas.iter().map(|delta| delta.delta.as_str()))
            }
            SimOutcome::Reverts { .. } | SimOutcome::NotOffered | SimOutcome::Unreachable => None,
        }
    }
}

/// The verdict's quiet line over a CHECKED answer's signed moves (each a
/// decimal string, as [`TrustAssetDelta::delta`] and the judgments carry
/// them): [`KEY_NO_CHANGE`] when there are none, or every one of them is a
/// zero — a move of nothing is not a move, and a card with a title and
/// nothing under it would read as a verdict that never came. `None` as soon
/// as one of them is anything else, a figure nobody can read included: what
/// cannot be read is never "nothing moves".
///
/// Only for an answer that was a check. The callers are
/// [`SimOutcome::no_change_key`] and `token_trust`'s view of its judgments.
#[must_use]
pub fn no_change_key<'a>(deltas: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    deltas.into_iter().all(is_zero).then_some(KEY_NO_CHANGE)
}

/// A signed decimal that is zero: an optional sign, then one or more `0`s.
fn is_zero(delta: &str) -> bool {
    let digits = delta.trim();
    let digits = digits
        .strip_prefix('-')
        .or_else(|| digits.strip_prefix('+'))
        .unwrap_or(digits);
    !digits.is_empty() && digits.bytes().all(|b| b == b'0')
}

/// An outcome and every line a sheet draws for it, as one record: what the
/// UniFFI and wasm exports hand a client ([`verdict`]). A client reads the
/// tone and the sentences from here and decides none of them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SimVerdict {
    /// `deltas | reverts | not_offered | unreachable`. Only `deltas` means
    /// the node checked the transaction.
    pub kind: String,
    /// The person's signed per-asset moves (the input `token_trust` takes).
    /// Empty for every kind but `deltas` — and empty WITH `deltas` is
    /// "checked, nothing of theirs moves" ([`Self::no_change_key`]).
    pub deltas: Vec<TrustAssetDelta>,
    /// The sanitised `Error(string)` of a revert, at most
    /// [`REVERT_REASON_MAX_CHARS`] characters — the `{{reason}}` of
    /// [`Self::notice_key`]. Untrusted text made safe to print.
    pub revert_reason: Option<String>,
    /// The notice's tone (`danger` for a revert, `caution` for
    /// could-not-check); `None` for `deltas`.
    pub notice_risk: Option<ClearRisk>,
    /// The notice's corpus key; `None` for `deltas`.
    pub notice_key: Option<String>,
    /// [`KEY_NO_CHANGE`] when the answer was a check and nothing of the
    /// person's moves: the quiet line the verdict's place says then.
    pub no_change_key: Option<String>,
}

/// [`classify`] and what the sheet says for it, in one record.
#[must_use]
pub fn verdict(reply: SimReply, user: &str) -> SimVerdict {
    let outcome = classify(reply, user);
    let notice = notice(&outcome);
    let no_change_key = outcome.no_change_key().map(str::to_owned);
    let kind = match &outcome {
        SimOutcome::Deltas { .. } => "deltas",
        SimOutcome::Reverts { .. } => "reverts",
        SimOutcome::NotOffered => "not_offered",
        SimOutcome::Unreachable => "unreachable",
    };
    let (deltas, revert_reason) = match outcome {
        SimOutcome::Deltas { deltas } => (deltas, None),
        SimOutcome::Reverts { reason } => (Vec::new(), reason),
        SimOutcome::NotOffered | SimOutcome::Unreachable => (Vec::new(), None),
    };
    SimVerdict {
        kind: kind.to_owned(),
        deltas,
        revert_reason,
        notice_risk: notice.as_ref().map(|notice| notice.risk),
        notice_key: notice.map(|notice| notice.key.to_owned()),
        no_change_key,
    }
}

/// [`verdict`] over the reply's wire form ([`SimReply::from_json`]), as JSON
/// — the wasm client's export. A record that cannot be written out is an
/// answer nobody can read: the could-not-check line, never an empty list
/// that reads "nothing moves".
#[must_use]
pub fn verdict_json(user: &str, reply_json: &str) -> String {
    serde_json::to_string(&verdict(SimReply::from_json(reply_json), user))
        .or_else(|_| {
            serde_json::to_string(&verdict(
                SimReply::Error {
                    code: None,
                    message: None,
                },
                user,
            ))
        })
        .unwrap_or_default()
}

/// What one call result says about itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CallStatus {
    Succeeded,
    Failed,
    /// A status that is neither 0 nor 1, and no error beside it.
    Unreadable,
}

/// "Any call with status 0x0 or an error" fails (RG6): an error is never
/// outvoted by a success status, so a contradictory answer cannot hide a
/// revert. Otherwise the status decides — `1` succeeded, `0` failed, two
/// spellings in the wild (`"0x1"` and a number) — and a call with neither a
/// status nor an error ran. Any other status is unreadable (review of 082 G:
/// reading it as a revert drew the danger line on a call the node may well
/// have run).
fn call_status(call: &Value) -> CallStatus {
    if call.get("error").is_some_and(|error| !error.is_null()) {
        return CallStatus::Failed;
    }
    let status = match call.get("status") {
        None | Some(Value::Null) => return CallStatus::Succeeded,
        Some(Value::String(status)) => quantity(status),
        Some(Value::Number(status)) => status.as_u64(),
        Some(_) => None,
    };
    match status {
        Some(1) => CallStatus::Succeeded,
        Some(0) => CallStatus::Failed,
        _ => CallStatus::Unreadable,
    }
}

/// A `0x` hex quantity as a number; `None` when it is not one.
fn quantity(text: &str) -> Option<u64> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(hex, 16).ok()
}

// ---------------------------------------------------------------------------
// The revert reason
// ---------------------------------------------------------------------------

/// The printable reason of one failed call, or `None`.
///
/// Reads the call's `returnData`, then `error.data` (nodes differ on where the
/// revert bytes go), and decodes only `Error(string)`
/// ([`ERROR_STRING_SELECTOR`]). The node's own `error.message` is never used:
/// it is prose around the same untrusted bytes. The text is sanitised and
/// capped (see the module doc); a reason that sanitises to nothing, or that
/// carries a `$t(` sentence reference, is `None`.
pub fn revert_reason(call: &Value) -> Option<String> {
    let candidates = [
        call.get("returnData").and_then(Value::as_str),
        call.get("error")
            .and_then(|error| error.get("data"))
            .and_then(Value::as_str),
    ];
    candidates
        .into_iter()
        .flatten()
        .find_map(decode_error_string)
        .and_then(|text| sanitise(&text))
}

/// `Error(string)` ABI: selector ‖ offset ‖ length ‖ bytes. Every bound is
/// checked; invalid UTF-8 is not a reason.
fn decode_error_string(hex: &str) -> Option<String> {
    let bytes = crate::primitives::from_hex(hex.trim()).ok()?;
    let body = bytes.strip_prefix(&ERROR_STRING_SELECTOR[..])?;
    let offset = word_as_usize(body.get(..32)?)?;
    let len_end = offset.checked_add(32)?;
    let len = word_as_usize(body.get(offset..len_end)?)?;
    let text = body.get(len_end..len_end.checked_add(len)?)?;
    String::from_utf8(text.to_vec()).ok()
}

/// A 32-byte ABI word that fits a `usize` (so it can index), else `None`.
fn word_as_usize(word: &[u8]) -> Option<usize> {
    let (high, low) = word.split_at(word.len().checked_sub(8)?);
    if high.iter().any(|b| *b != 0) {
        return None;
    }
    usize::try_from(u64::from_be_bytes(low.try_into().ok()?)).ok()
}

/// The reason as a sheet may print it: invisible and control characters
/// dropped, whitespace runs folded to one space, trimmed, at most
/// [`REVERT_REASON_MAX_CHARS`] characters (a cut one ends in `…`). `None`
/// when nothing printable is left, or when it carries [`NESTING_OPENER`].
fn sanitise(raw: &str) -> Option<String> {
    let mut out = String::new();
    let mut space = false;
    for c in raw.chars() {
        // Whitespace first: a newline is a control character too, and it
        // separates words, so it becomes a space rather than vanishing.
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if is_never_in_a_name(c) {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    // The corpus engine expands `$t(key)` in a rendered sentence after
    // `{{reason}}` is filled in: a reason carrying one would print a wallet
    // sentence of the contract's choosing inside the danger line. Checked
    // after the invisible characters are gone, so none of them can hide one.
    // Such a reason is no reason; the danger stays, in the plain words.
    if out.is_empty() || out.contains(NESTING_OPENER) {
        return None;
    }
    if out.chars().count() > REVERT_REASON_MAX_CHARS {
        let mut cut: String = out.chars().take(REVERT_REASON_MAX_CHARS - 1).collect();
        cut.truncate(cut.trim_end().len());
        cut.push('…');
        out = cut;
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The deltas (moved from app-desktop/vela-wallet/src/executor/sim.rs)
// ---------------------------------------------------------------------------

/// Net every `Transfer` log that touches `user` into one signed delta per
/// asset, in first-seen order, zeros dropped.
///
/// In and out of the same token nets to nothing — printing a pair of moves
/// that cancel is how a swap looks like a theft. A log from one of the native
/// sentinels ([`NATIVE_LOG_ADDRESSES`]; `traceTransfers` reports value moves
/// from `0xeeee…`) is the native coin. Malformed logs contribute nothing,
/// silently: a simulation is untrusted input by definition. The arithmetic is
/// exact over 256-bit values (the shells' own copies used big integers; the
/// desktop's saturated at `i128`).
pub fn derive_deltas(logs: &[&Value], user: &str) -> Vec<TrustAssetDelta> {
    let user = user.trim().to_lowercase();
    if user.is_empty() {
        return Vec::new();
    }
    // (key, received, sent) in first-seen order.
    let mut totals: Vec<(String, U256, U256)> = Vec::new();

    for log in logs {
        let Some(topics) = log.get("topics").and_then(Value::as_array) else {
            continue;
        };
        if topics.len() != 3 {
            continue;
        }
        if !topics[0]
            .as_str()
            .is_some_and(|topic| topic.eq_ignore_ascii_case(TRANSFER_TOPIC))
        {
            continue;
        }
        // A topic that is not an address makes the whole log malformed —
        // never half a transfer.
        let (Some(from), Some(to)) = (topic_address(&topics[1]), topic_address(&topics[2])) else {
            continue;
        };
        let (sent, received) = (from == user, to == user);
        if !sent && !received {
            continue;
        }
        let Some(value) = first_word(log.get("data").and_then(Value::as_str).unwrap_or("")) else {
            continue;
        };
        if value.is_zero() {
            continue;
        }
        let address = log
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_lowercase();
        if address.is_empty() {
            continue;
        }
        let key = if NATIVE_LOG_ADDRESSES.contains(&address.as_str()) {
            NATIVE_KEY.to_owned()
        } else {
            address
        };
        let index = match totals.iter().position(|(k, _, _)| *k == key) {
            Some(index) => index,
            None => {
                totals.push((key, U256::ZERO, U256::ZERO));
                totals.len() - 1
            }
        };
        let entry = &mut totals[index];
        // A self-transfer (from == to == user) adds to both sides and nets to
        // zero, which is the truth about it.
        if received {
            entry.1 = entry.1.saturating_add(value);
        }
        if sent {
            entry.2 = entry.2.saturating_add(value);
        }
    }

    totals
        .into_iter()
        .filter_map(|(key, received, sent)| {
            let delta = match received.cmp(&sent) {
                std::cmp::Ordering::Equal => return None,
                std::cmp::Ordering::Greater => (received - sent).to_string(),
                std::cmp::Ordering::Less => format!("-{}", sent - received),
            };
            let native = key == NATIVE_KEY;
            Some(TrustAssetDelta {
                kind: if native {
                    TrustDeltaKind::Native
                } else {
                    TrustDeltaKind::Erc20
                },
                token: (!native).then_some(key),
                delta,
            })
        })
        .collect()
}

/// The totals' key for the native coin (never a lowercased address).
const NATIVE_KEY: &str = "native";

/// The address in a 32-byte topic's low 20 bytes, lowercased; `None` when the
/// topic is too short or not hex.
fn topic_address(topic: &Value) -> Option<String> {
    let raw = topic.as_str()?;
    let hex = raw.strip_prefix("0x").unwrap_or(raw);
    if hex.len() < 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("0x{}", &hex[hex.len() - 40..]).to_lowercase())
}

/// The first 32-byte word of a log's data; anything past it belongs to
/// another field. `None` when there is no whole word or it is not hex.
fn first_word(data: &str) -> Option<U256> {
    let hex = data.strip_prefix("0x").unwrap_or(data);
    let word = hex.get(..64)?;
    if !word.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    U256::from_str_radix(word, 16).ok()
}
