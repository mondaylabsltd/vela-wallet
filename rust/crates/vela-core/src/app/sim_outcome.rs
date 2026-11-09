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
//! no reply inside SIM_VERDICT_WAIT_MS    ─► (none yet)  ─► Caution  simUnavailableWarning, until one lands
//! ```
//!
//! The last row is `sign_request`'s (PR 3): the confirm waits for the verdict
//! and the wait has an end. When it passes, the request's view names the same
//! could-not-check line (`SignView::sim_waited_out_key`) and the sheet draws
//! it in the verdict's place; a reply that still arrives is read by the rows
//! above and replaces it.
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
use super::corroborate::{self, Standing};
use super::name_verify::is_never_in_a_name;
use super::token_trust::{
    TrustAssetDelta, TrustDeltaKind, TrustSimJudgment, NATIVE_LOG_ADDRESSES, TRANSFER_TOPIC,
};

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
    /// R22 (review 1-2): every call ran on the node that answered, and nobody
    /// independent confirmed what it moves — one node answered, or two
    /// independent nodes said different things (`disagreed`). What moves is
    /// not believed: the sheet says it could not confirm, and signs only
    /// "… anyway". A lie of "nothing moves" from one node ends here.
    Unconfirmed { disagreed: bool },
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
        SimOutcome::Unconfirmed { disagreed } => Some(SimNotice {
            risk: ClearRisk::Caution,
            key: if *disagreed {
                KEY_DISAGREED
            } else {
                KEY_UNCONFIRMED
            },
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
            SimOutcome::Reverts { .. }
            | SimOutcome::NotOffered
            | SimOutcome::Unreachable
            | SimOutcome::Unconfirmed { .. } => None,
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
/// [`SimOutcome::no_change_key`] and [`no_change_key_of`].
#[must_use]
pub fn no_change_key<'a>(deltas: impl IntoIterator<Item = &'a str>) -> Option<&'static str> {
    deltas.into_iter().all(is_zero).then_some(KEY_NO_CHANGE)
}

/// [`no_change_key`] over a checked answer's JUDGMENTS — what `token_trust`'s
/// view and a sheet hold. The same rule, asked of each judgment
/// ([`TrustSimJudgment::moves_nothing`]): an unverified token carries no
/// figure to read a zero from, only its direction.
#[must_use]
pub fn no_change_key_of(judgments: &[TrustSimJudgment]) -> Option<&'static str> {
    judgments
        .iter()
        .all(TrustSimJudgment::moves_nothing)
        .then_some(KEY_NO_CHANGE)
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
    /// the node checked the transaction. A multisig review adds `withheld`
    /// (`multisig_review::sim_withheld`): refused, so nothing is said about
    /// what moves — every other field empty.
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
    /// No node answered (`unreachable`): the could-not-check line is about
    /// the network, not the transaction, so a sheet asks again once the
    /// chain answers — and on the person's "Check again" (dapp-15). A node
    /// that spoke and did not simulate (`not_offered`) is not asked again:
    /// it would say the same.
    #[serde(default)]
    pub recheck: bool,
    /// R22: who stands behind `deltas` — `corroborated` (two independent
    /// nodes agreed) or `own_node` (the person's own node alone, by their
    /// choice: "Only use my node"). `None` for every kind but `deltas`, and
    /// for a verdict another reading made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub source: Option<String>,
    /// R22: the line the sheet says about `source` — [`KEY_OWN_NODE`]
    /// ("Checked by your own node only") when the person's node answered
    /// alone; `None` otherwise. The UI says who it trusted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub source_key: Option<String>,
}

/// [`classify`] and what the sheet says for it, in one record.
#[must_use]
pub fn verdict(reply: SimReply, user: &str) -> SimVerdict {
    verdict_of(classify(reply, user))
}

/// The verdict record of an outcome another reading made — a multisig's
/// state probe reads the Safe's balances before and after its execution
/// (`multisig_review`), the same outcome a simulation's logs give.
#[must_use]
pub fn verdict_of(outcome: SimOutcome) -> SimVerdict {
    let notice = notice(&outcome);
    let no_change_key = outcome.no_change_key().map(str::to_owned);
    let kind = match &outcome {
        SimOutcome::Deltas { .. } => "deltas",
        SimOutcome::Reverts { .. } => "reverts",
        SimOutcome::NotOffered => "not_offered",
        SimOutcome::Unreachable => "unreachable",
        SimOutcome::Unconfirmed { .. } => "unconfirmed",
    };
    // An unconfirmed answer is asked again too: the second node may answer
    // next time, and the person's "Check again" asks both.
    let recheck = matches!(
        outcome,
        SimOutcome::Unreachable | SimOutcome::Unconfirmed { .. }
    );
    let (deltas, revert_reason) = match outcome {
        SimOutcome::Deltas { deltas } => (deltas, None),
        SimOutcome::Reverts { reason } => (Vec::new(), reason),
        SimOutcome::NotOffered | SimOutcome::Unreachable | SimOutcome::Unconfirmed { .. } => {
            (Vec::new(), None)
        }
    };
    SimVerdict {
        kind: kind.to_owned(),
        deltas,
        revert_reason,
        notice_risk: notice.as_ref().map(|notice| notice.risk),
        notice_key: notice.map(|notice| notice.key.to_owned()),
        no_change_key,
        recheck,
        source: None,
        source_key: None,
    }
}

/// r9-2: one line per balance. A chain whose native coin also wears an
/// ERC-20 interface (`manage_tokens::native_alias_token`: Arc's USDC at
/// `0x3600…0000`, Stable's USDT0) moves ONE balance when either is used:
/// the coin's line says it, and the interface's line — the same money in
/// other units — is dropped. Without a coin line the interface's stays: it
/// is then the only word about that balance.
#[must_use]
pub fn one_line_per_balance(chain_id: u64, verdict: SimVerdict) -> SimVerdict {
    let Some(alias) = u32::try_from(chain_id)
        .ok()
        .and_then(super::manage_tokens::native_alias_token)
    else {
        return verdict;
    };
    if !verdict
        .deltas
        .iter()
        .any(|d| d.kind == TrustDeltaKind::Native)
    {
        return verdict;
    }
    let mut verdict = verdict;
    verdict.deltas.retain(|d| {
        !(d.kind == TrustDeltaKind::Erc20
            && d.token
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(alias)))
    });
    if verdict.kind == "deltas" && verdict.deltas.is_empty() {
        return verdict_of(SimOutcome::Deltas { deltas: Vec::new() });
    }
    verdict
}

// ---------------------------------------------------------------------------
// R22: what moves, believed only when two independent nodes agree
// ---------------------------------------------------------------------------

/// "Only one node answered — Vela couldn't confirm what this does."
pub const KEY_UNCONFIRMED: &str = "componentsUi.signing.simUnconfirmed";
/// "Two nodes gave different answers about what this does."
pub const KEY_DISAGREED: &str = "componentsUi.signing.simDisagreed";
/// "Checked by your own node only" — "Only use my node" is on.
pub const KEY_OWN_NODE: &str = "componentsUi.signing.simOwnNode";

/// How far apart two honest nodes' figures for one balance may be and still
/// agree, in basis points of the larger: two nodes a block apart price a
/// busy pool differently. Within it the sheet shows the WORSE of the two
/// (the larger outflow, the smaller inflow), so a node that shades a figure
/// gains nothing by it; beyond it they disagree.
pub const SIM_AGREE_BPS: u64 = 200;

/// A signed decimal, read: (negative, magnitude). `None`: not a number.
fn signed(delta: &str) -> Option<(bool, U256)> {
    let t = delta.trim();
    let (neg, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let v = U256::from_str_radix(digits, 10).ok()?;
    Some((neg && !v.is_zero(), v))
}

fn delta_key(d: &TrustAssetDelta) -> (u8, String) {
    let kind = match d.kind {
        TrustDeltaKind::Native => 0,
        TrustDeltaKind::Erc20 => 1,
    };
    (
        kind,
        d.token.clone().unwrap_or_default().to_ascii_lowercase(),
    )
}

/// The two nodes' moves agree ([`SIM_AGREE_BPS`]); `Some` with what the
/// sheet shows — each balance's worse figure — or `None`.
pub fn agreed_deltas(a: &[TrustAssetDelta], b: &[TrustAssetDelta]) -> Option<Vec<TrustAssetDelta>> {
    use std::collections::BTreeMap;
    /// Each balance once: (negative, magnitude, the delta as given).
    type Moves = BTreeMap<(u8, String), (bool, U256, TrustAssetDelta)>;
    let read = |list: &[TrustAssetDelta]| -> Option<Moves> {
        let mut out = BTreeMap::new();
        for d in list {
            let (neg, v) = signed(&d.delta)?;
            if v.is_zero() {
                continue;
            }
            // One balance named twice: an answer nobody can read.
            if out.insert(delta_key(d), (neg, v, d.clone())).is_some() {
                return None;
            }
        }
        Some(out)
    };
    let (x, y) = (read(a)?, read(b)?);
    if x.len() != y.len() {
        return None;
    }
    let mut merged = Vec::new();
    for (key, (xn, xv, xd)) in &x {
        let (yn, yv, yd) = y.get(key)?;
        if xn != yn {
            return None;
        }
        let (hi, lo) = if xv >= yv { (*xv, *yv) } else { (*yv, *xv) };
        let spread = hi - lo;
        // spread / hi ≤ bps / 10_000, without division.
        if spread.saturating_mul(U256::from(10_000u64))
            > hi.saturating_mul(U256::from(SIM_AGREE_BPS))
        {
            return None;
        }
        // The worse of the two: the larger outflow, the smaller inflow.
        let worse = if *xn {
            if xv >= yv {
                xd
            } else {
                yd
            }
        } else if xv <= yv {
            xd
        } else {
            yd
        };
        merged.push(worse.clone());
    }
    // In the first node's order.
    let order: Vec<(u8, String)> = a.iter().map(delta_key).collect();
    merged.sort_by_key(|d| {
        order
            .iter()
            .position(|k| *k == delta_key(d))
            .unwrap_or(usize::MAX)
    });
    Some(merged)
}

/// What a corroborated envelope's answer is, and who stands behind it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimRead {
    pub outcome: SimOutcome,
    /// `Some` for an answer that moves something, or nothing: whether it is
    /// believed (two nodes agreed, or the person's own node alone) or not.
    pub standing: Option<Standing>,
}

/// The outcome of a corroborated envelope (`corroborate` module doc) for
/// `user`: the first node's answer, believed for a checked answer only when
/// a second, independent node's agrees ([`agreed_deltas`]) or the person's
/// own node answered alone by their choice. A revert, a node that did not
/// simulate and silence need no second word: none of them is a plain
/// verdict ("… anyway" or a could-not-check stands either way).
#[must_use]
pub fn classify_envelope(envelope: &Value, user: &str) -> SimRead {
    let first = classify(
        SimReply::from_envelope(&corroborate::first_answer(envelope)),
        user,
    );
    let SimOutcome::Deltas { deltas } = &first else {
        return SimRead {
            outcome: first,
            standing: None,
        };
    };
    if corroborate::own_node(envelope) {
        return SimRead {
            outcome: first.clone(),
            standing: Some(Standing::OwnNode),
        };
    }
    let Some(witness) = corroborate::witness_of(envelope) else {
        return SimRead {
            outcome: SimOutcome::Unconfirmed { disagreed: false },
            standing: Some(Standing::Single),
        };
    };
    match classify(SimReply::from_envelope(witness), user) {
        SimOutcome::Deltas { deltas: theirs } => match agreed_deltas(deltas, &theirs) {
            Some(merged) => SimRead {
                outcome: SimOutcome::Deltas { deltas: merged },
                standing: Some(Standing::Corroborated),
            },
            None => SimRead {
                outcome: SimOutcome::Unconfirmed { disagreed: true },
                standing: Some(Standing::Disagreed),
            },
        },
        // The second node says it fails: the two disagree about what it does.
        SimOutcome::Reverts { .. } => SimRead {
            outcome: SimOutcome::Unconfirmed { disagreed: true },
            standing: Some(Standing::Disagreed),
        },
        // The second node did not simulate: one node's word.
        SimOutcome::NotOffered | SimOutcome::Unreachable | SimOutcome::Unconfirmed { .. } => {
            SimRead {
                outcome: SimOutcome::Unconfirmed { disagreed: false },
                standing: Some(Standing::Single),
            }
        }
    }
}

/// [`classify_envelope`] and what the sheet says for it, in one record —
/// `source` and `source_key` say who stood behind a checked answer.
#[must_use]
pub fn verdict_of_envelope(envelope: &Value, user: &str) -> SimVerdict {
    let read = classify_envelope(envelope, user);
    let mut verdict = verdict_of(read.outcome);
    if verdict.kind == "deltas" {
        verdict.source = read.standing.map(|s| match s {
            Standing::OwnNode => "own_node".to_owned(),
            _ => "corroborated".to_owned(),
        });
        verdict.source_key =
            (read.standing == Some(Standing::OwnNode)).then(|| KEY_OWN_NODE.to_owned());
    }
    verdict
}

/// [`verdict_of_envelope`] over the envelope's JSON text; text that is not
/// JSON is an answer nobody can read.
#[must_use]
pub fn verdict_of_envelope_json(envelope_json: &str, user: &str) -> SimVerdict {
    match serde_json::from_str::<Value>(envelope_json) {
        Ok(envelope) => verdict_of_envelope(&envelope, user),
        Err(_) => verdict(
            SimReply::Error {
                code: None,
                message: None,
            },
            user,
        ),
    }
}

/// [`verdict_of_envelope`] over the reply's wire form (a corroborated
/// envelope, R22), as JSON — the wasm client's export. A record that cannot be written out is an
/// answer nobody can read: the could-not-check line, never an empty list
/// that reads "nothing moves".
#[must_use]
pub fn verdict_json(user: &str, reply_json: &str) -> String {
    serde_json::to_string(&verdict_of_envelope_json(reply_json, user))
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
/// from `0xeeee…`) is the native coin — once: where the traced logs are
/// there, a chain's own native log of the same move (Arc's `0xffff…fffe`) is
/// its copy and is not counted again (r5reg-1). Malformed logs contribute nothing,
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
    // r5reg-1: a node that ran `traceTransfers` reports EVERY value move
    // from `0xeeee…`; a chain that logs its own native moves (Arc's EIP-7708
    // log from `0xffff…fffe`) reports the same move again in the same answer.
    // Counted twice, a multisig's batch read −0.004 for −0.002. When the
    // traced logs are there they are the record, and the chain's own native
    // logs are the copy; without them (a receipt, a node with no trace) the
    // chain's own logs are the only record and count.
    let traced = logs
        .iter()
        .any(|log| log_address(log) == NATIVE_LOG_ADDRESSES[2]);

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
        let address = log_address(log);
        if address.is_empty() {
            continue;
        }
        let native = NATIVE_LOG_ADDRESSES.contains(&address.as_str());
        if native && traced && address != NATIVE_LOG_ADDRESSES[2] {
            continue;
        }
        let key = if native {
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

/// A log's emitting address, lowercased (`""` when it has none).
fn log_address(log: &Value) -> String {
    log.get("address")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_lowercase()
}

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
