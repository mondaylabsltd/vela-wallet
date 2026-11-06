//! Where a reviewed thing's one review stands (§12): `loading`, `ready`, or
//! `failed {reason_key}` — for every draft (a send, a member change, a
//! rename, a rejection's cancellation, a site's request), every proposal a
//! member approves or executes, and every transaction the outside page
//! signs.
//!
//! The Bybit rule: nothing is signed on a review that was not read. A shell
//! enables a sign, propose, approve or execute button ONLY when the state is
//! `ready` and the review's own sign rule allows it ([`door`]); a missing
//! state, `loading` and `failed` keep it disabled. `loading` is bounded here
//! ([`REVIEW_TIMEOUT_MS`]): past it, it is `failed` with
//! [`REASON_TIMED_OUT`] — it never spins for ever and never becomes "allowed"
//! by default. `failed` is said with its reason and a "Try again" that sends
//! the machine's `review_again`.
//!
//! D1 (real-device pass 2): a `ready` review whose state-change check AND
//! simulation could not be read (its `unchecked` concern) is drawn, but its
//! door asks for "… anyway" and carries a danger line ([`UNCHECKED_KEY`],
//! `reason_key`) and a Try again (`retry`) — never one plain tap.

use std::future::Future;

use futures::future::{self, Either};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_shell::Io;

/// How long one review is waited for, from the moment it is asked to the
/// moment it is drawn: past it, it is `failed` ([`REASON_TIMED_OUT`]). Above
/// the reads' own bounds (three code rounds and the state reads, each
/// `REVIEW_READ_MS`), so a review that is slow but bounded is never cut — it
/// stops only one a shell never answered (a simulation, a recipient's facts).
pub const REVIEW_TIMEOUT_MS: u32 = 40_000;

/// The words of the placeholder while a review is read ("Reading the
/// transaction…").
pub const LOADING_KEY: &str = "home.multisig.review.state.loading";
/// Under a failed review: nothing can be signed until it is read.
pub const BLOCKED_KEY: &str = "home.multisig.review.state.blocked";
/// The button under a failed review that sends `review_again`.
pub const TRY_AGAIN_KEY: &str = "home.multisig.review.state.tryAgain";
/// The review did not come back in [`REVIEW_TIMEOUT_MS`].
pub const REASON_TIMED_OUT: &str = "home.multisig.review.state.timedOut";
/// The core could not read the transaction (or not gather its review).
pub const REASON_UNREADABLE: &str = "home.multisig.review.state.unreadable";
/// The transaction is no longer in the list it was opened from.
pub const REASON_GONE: &str = "home.multisig.review.state.gone";
/// r14-13: the review's reads came back, but its network answered none of
/// them — not the multisig's own state, not a code, not a check: the
/// transaction could not be read against the chain at all. `{{network}}`:
/// the network's name. Never signable (not "… anyway"): Try again.
pub const REASON_CHAIN_UNREAD: &str = "home.multisig.review.state.chainUnread";
/// D1 (real-device pass 2): a review drawn, but neither safety check read —
/// not the state-diff guard, not the simulation. A danger line over the
/// review, with Try again; the button signs only "… anyway".
pub const UNCHECKED_KEY: &str = "home.multisig.review.state.unchecked";

/// Whether `review` (the core's `MultisigReview`) says neither safety check
/// was read (its `unchecked` concern, D1).
pub fn unchecked(review: Option<&Value>) -> bool {
    review
        .and_then(|r| r.get("concerns"))
        .and_then(Value::as_array)
        .is_some_and(|cs| {
            cs.iter()
                .any(|c| c.get("kind").and_then(Value::as_str) == Some("unchecked"))
        })
}

/// Where a review stands. Wire: `{"kind": "loading"}`, `{"kind": "ready"}`,
/// `{"kind": "failed", "reason_key": "home.multisig.review.state.timedOut"}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigReviewState"))]
pub enum ReviewState {
    /// Being read: say [`LOADING_KEY`]; nothing can be signed.
    #[default]
    Loading,
    /// Read: draw the review; its sign rule decides the button.
    Ready,
    /// Not read: say `reason_key` (and [`BLOCKED_KEY`]) with a "Try again"
    /// (`review_again`); nothing can be signed.
    Failed { reason_key: String },
}

impl ReviewState {
    pub fn failed(reason_key: &str) -> Self {
        Self::Failed {
            reason_key: reason_key.to_owned(),
        }
    }

    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }

    /// What a gathered review says of its state: read, or not readable.
    pub fn of(review: Option<&Value>) -> Self {
        match review {
            Some(r) if !r.is_null() => Self::Ready,
            _ => Self::failed(REASON_UNREADABLE),
        }
    }

    /// r14-13, the one rule every shell's review lands by: a gathered review
    /// is `ready` only when its network answered at least one of the reads
    /// it made (`answers`: each read's envelope, `{result}` / `{error}` /
    /// `{unreachable}`; [`chain_answered`]). Nothing answered — a network
    /// hung or refused end to end — is `failed {chainUnread}`: the
    /// transaction was not read against the chain, so nothing is signed, not
    /// even "… anyway", and Try again reads it again. Only when the chain
    /// answered and the two safety checks alone are unread is the review
    /// drawn `unchecked` (D1: "… anyway").
    pub fn settled(review: Option<&Value>, answers: &[Value]) -> Self {
        match Self::of(review) {
            Self::Ready if !chain_answered(answers) => Self::failed(REASON_CHAIN_UNREAD),
            other => other,
        }
    }
}

/// Whether the network answered any of a review's reads (r14-13): a result
/// or an error (a node that says "method not found" answered); only
/// `{unreachable}` — a hang, a refused connection, a read cut at its bound —
/// is silence. No read made at all needs no answer.
pub fn chain_answered(answers: &[Value]) -> bool {
    answers.is_empty()
        || answers
            .iter()
            .any(|a| a.get("result").is_some() || a.get("error").is_some())
}

/// r16-3: how long a review's re-ask of safety checks that came back as
/// silence may run, from its first re-ask, before "unchecked" is final for
/// this read. A network back from an outage answers its multisig's own
/// state first; its checks were turned away for another 1–5 s (Arc, R16).
pub const RECHECK_MS: u32 = 10_000;
/// r16-3: the pause before each re-ask of silent checks (a pool that turns
/// a read away does so in milliseconds; asking again at once meets the same).
pub const RECHECK_PAUSE_MS: u32 = 1_000;

/// r16-3, the rule "unchecked" is published by: whether a gathered review's
/// safety checks — `checks`, the envelopes of the state trace, the probe and
/// the simulation — are FINAL for this read. Final when the review would not
/// land unchecked anyway (no review, or its state guard or simulation read),
/// when the network did not answer the multisig's own state (`anchor`: it
/// lands `failed {chainUnread}`, never unchecked), when no check is still
/// silent (each answered — a refusal is the network's answer), or when the
/// re-ask has run `rechecked_ms` ≥ [`RECHECK_MS`]. Otherwise the silent
/// checks are asked again: a review on its way back from an outage goes
/// failed → Reading → ready and never passes through "… anyway".
pub fn checks_final(
    review: Option<&Value>,
    anchor: &Value,
    checks: &[Value],
    rechecked_ms: f64,
) -> bool {
    let answered = |e: &Value| e.get("result").is_some() || e.get("error").is_some();
    let review = review.filter(|r| !r.is_null());
    review.is_none()
        || !unchecked(review)
        || !answered(anchor)
        || !checks.iter().any(|c| !answered(c))
        || rechecked_ms >= f64::from(RECHECK_MS)
}

/// `review.checksFinal {review?, anchor, checks, rechecked_ms}` through the
/// link door: [`checks_final`].
pub fn checks_final_json(args: &Value) -> Result<Value, String> {
    let checks = match args.get("checks") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err("checks: not a list".to_owned()),
    };
    let anchor = args.get("anchor").cloned().unwrap_or(Value::Null);
    let ms = args
        .get("rechecked_ms")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    Ok(Value::Bool(checks_final(
        args.get("review"),
        &anchor,
        &checks,
        ms,
    )))
}

/// What a sheet draws in the review's place.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigReviewShows"))]
pub enum ReviewShows {
    /// The calm placeholder ([`LOADING_KEY`]).
    Loading,
    /// The review itself.
    Review,
    /// The reason, [`BLOCKED_KEY`] and "Try again".
    Failed,
}

/// The one rule a sign / propose / approve / execute button follows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigSignDoor"))]
pub struct SignDoor {
    pub shows: ReviewShows,
    /// The button may be pressed.
    pub enabled: bool,
    /// Pressed, it goes through the review's "… anyway" step first.
    pub anyway: bool,
    /// `failed`: the reason's corpus key. `review` (D1): when set
    /// ([`UNCHECKED_KEY`]), a danger line drawn ABOVE the review — neither
    /// safety check was read.
    pub reason_key: Option<String>,
    /// `failed`: offer "Try again" (`review_again`). `review` (D1): the
    /// unchecked line's Try again.
    pub retry: bool,
}

/// The sign door for a review in `state` (`None`: not asked yet — the same
/// as `loading`): enabled only when it is `ready`, a review is there, and its
/// sign rule allows it. Nothing else opens it — not a missing review, not a
/// missing sign rule.
pub fn door(state: Option<&ReviewState>, review: Option<&Value>) -> SignDoor {
    let closed = |shows, reason_key: Option<String>| SignDoor {
        shows,
        enabled: false,
        anyway: false,
        retry: reason_key.is_some(),
        reason_key,
    };
    match state {
        None | Some(ReviewState::Loading) => closed(ReviewShows::Loading, None),
        Some(ReviewState::Failed { reason_key }) => {
            closed(ReviewShows::Failed, Some(reason_key.clone()))
        }
        Some(ReviewState::Ready) => {
            let Some(sign) = review.filter(|r| !r.is_null()).map(|r| &r["sign"]) else {
                // Ready without a review is a contradiction: never signed on it.
                return closed(ReviewShows::Failed, Some(REASON_UNREADABLE.to_owned()));
            };
            let allowed = sign["allowed"].as_bool() == Some(true);
            // D1: neither safety check read — drawn, said in danger with
            // Try again, and signed only "… anyway" (whatever the sign rule
            // handed says: the door never lets it be one plain tap).
            let unchecked = unchecked(review);
            SignDoor {
                shows: ReviewShows::Review,
                enabled: allowed,
                anyway: allowed && (sign["anyway"].as_bool() == Some(true) || unchecked),
                reason_key: unchecked.then(|| UNCHECKED_KEY.to_owned()),
                retry: unchecked,
            }
        }
    }
}

/// `fut` (a review being gathered), bounded by [`REVIEW_TIMEOUT_MS`]: its
/// answer, or `Err(failed {timedOut})`.
pub async fn bounded<Ev, T, F>(io: &Io<Ev>, fut: F) -> Result<T, ReviewState>
where
    Ev: Send + 'static,
    F: Future<Output = T>,
{
    let read = Box::pin(fut);
    let cap = Box::pin(io.wait(u64::from(REVIEW_TIMEOUT_MS)));
    match future::select(read, cap).await {
        Either::Left((out, _)) => Ok(out),
        Either::Right(_) => Err(ReviewState::failed(REASON_TIMED_OUT)),
    }
}

// ---------------------------------------------------------------------------
// r13-3: the step BEFORE a review exists — a draft being built, a link's
// transaction being opened — has the review's own discipline: bounded here,
// and when it fails it says which read failed, with Try again.
// ---------------------------------------------------------------------------

/// How long the reads a draft is built from (the multisig's members and
/// number on its network, its waiting transactions from the multisig
/// service), or a link's transaction is found by, are waited for. Past it the
/// step has failed, naming the read still out. Below [`REVIEW_TIMEOUT_MS`]
/// (the review still has to be read after it) and above one hung node's
/// bound plus a hedge (the RPC pool's 8 s + 1.5 s, r13-4): a slow read is
/// not cut, a hung one is.
pub const OPEN_TIMEOUT_MS: u32 = 25_000;

/// The multisig's network did not answer (`{{network}}`: its name).
pub const UNREAD_CHAIN_KEY: &str = "home.multisig.proposal.unread.chain";
/// The multisig service did not answer: its waiting transactions are unread
/// — the words `ActionState::MailboxUnreadable` always had.
pub const UNREAD_SERVICE_KEY: &str = "home.multisig.proposal.mailboxDown";
/// Neither answered — the network AND the service: most likely this
/// device's connection (`{{network}}`: the network's name).
pub const UNREAD_BOTH_KEY: &str = "home.multisig.proposal.unread.both";
/// A link's transaction: the network did not answer in time.
pub const LINK_UNREAD_CHAIN_KEY: &str = "home.multisig.proposal.unread.linkChain";
/// A link's transaction: the multisig service did not answer in time.
pub const LINK_UNREAD_SERVICE_KEY: &str = "home.multisig.proposal.unread.linkService";
/// A link's transaction: neither answered in time.
pub const LINK_UNREAD_BOTH_KEY: &str = "home.multisig.proposal.unread.linkBoth";

/// Which read the step before a review could not get — said as itself,
/// never as another (r13-3: a hung network was said as the service's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnreadRead {
    /// The multisig's network (its members, its number).
    Chain,
    /// The multisig service (its waiting transactions).
    Service,
    /// Both: most likely this device's connection.
    Both,
}

impl UnreadRead {
    /// From what failed (or was still out at [`OPEN_TIMEOUT_MS`]); `None`
    /// when both answered.
    pub fn of(chain: bool, service: bool) -> Option<Self> {
        match (chain, service) {
            (true, true) => Some(Self::Both),
            (true, false) => Some(Self::Chain),
            (false, true) => Some(Self::Service),
            (false, false) => None,
        }
    }

    /// The words for a draft that was not built: nothing was proposed.
    pub fn draft_key(self) -> &'static str {
        match self {
            Self::Chain => UNREAD_CHAIN_KEY,
            Self::Service => UNREAD_SERVICE_KEY,
            Self::Both => UNREAD_BOTH_KEY,
        }
    }

    /// The words for a link's transaction that could not be opened.
    pub fn link_key(self) -> &'static str {
        match self {
            Self::Chain => LINK_UNREAD_CHAIN_KEY,
            Self::Service => LINK_UNREAD_SERVICE_KEY,
            Self::Both => LINK_UNREAD_BOTH_KEY,
        }
    }
}

/// `review.opening {chain, service, link?}` through the link door: the key
/// for what failed (`chain` / `service`: whether that read failed or was
/// still out), or null when both answered.
pub fn opening_reason_json(args: &Value) -> Result<Value, String> {
    let flag = |k: &str| args.get(k).and_then(Value::as_bool).unwrap_or(false);
    let link = flag("link");
    Ok(
        UnreadRead::of(flag("chain"), flag("service")).map_or(Value::Null, |r| {
            Value::String(if link { r.link_key() } else { r.draft_key() }.to_owned())
        }),
    )
}

/// §18 leftover (b), round 14: what a proposal sheet opened WITHOUT a link
/// says while its transaction is not in the list — read from the machine's
/// facts, so a service that did not answer is never said as "it hasn't been
/// shared with the members yet". `key` is a corpus key (`{{network}}`: the
/// sheet's network); `retry`: offer Try again (the proposals machine's
/// `retry`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingSaid {
    pub key: String,
    pub retry: bool,
}

/// The sheet's transaction is still being looked for.
pub const MISSING_OPENING_KEY: &str = "home.multisig.proposal.opening";
/// The network did not answer: who can act, and what is waiting, unread.
pub const MISSING_CHAIN_KEY: &str = "home.multisig.proposal.cantCheck";
/// The multisig service did not answer: the transaction was not looked for.
pub const MISSING_SERVICE_KEY: &str = LINK_UNREAD_SERVICE_KEY;
/// The service answered, and it is not among the multisig's transactions.
pub const MISSING_NOT_LISTED_KEY: &str = "home.multisig.proposal.notInMailbox";
/// The sheet's network is not one the multisig shown is read on.
pub const MISSING_NOT_HERE_KEY: &str = "home.multisig.proposal.notHere";
/// Round 16 leftover 3 (§25 r16-16): the multisig service answered with
/// its rate limit — the home's busy words, with Try again; never "Couldn't
/// reach the multisig service".
pub const MISSING_BUSY_KEY: &str = "home.multisig.service.busyNote";

/// [`MissingSaid`] from the facts: `loading` (the list's first read is
/// out), `unreadable` (the network did not answer), `mailbox` (`None`: the
/// multisig is not read on this network; `Some(read)`: whether the service's
/// list was read), `busy` (the service's last answer was its rate limit —
/// the proposals view's `busy`). Only a read that came back and lacks it is
/// final; every unread one offers Try again. (This device being offline is
/// the shell's to say first, as it always has.)
pub fn missing_said(
    loading: bool,
    unreadable: bool,
    mailbox: Option<bool>,
    busy: bool,
) -> MissingSaid {
    let said = |key: &str, retry: bool| MissingSaid {
        key: key.to_owned(),
        retry,
    };
    if loading {
        return said(MISSING_OPENING_KEY, false);
    }
    if unreadable {
        return said(MISSING_CHAIN_KEY, true);
    }
    match mailbox {
        Some(true) => said(MISSING_NOT_LISTED_KEY, false),
        Some(false) if busy => said(MISSING_BUSY_KEY, true),
        Some(false) => said(MISSING_SERVICE_KEY, true),
        None => said(MISSING_NOT_HERE_KEY, false),
    }
}

/// `review.missing {loading, unreadable, mailbox?, busy?}` through the link
/// door: [`missing_said`].
pub fn missing_json(args: &Value) -> Result<Value, String> {
    let flag = |k: &str| args.get(k).and_then(Value::as_bool).unwrap_or(false);
    let mailbox = args.get("mailbox").and_then(Value::as_bool);
    serde_json::to_value(missing_said(
        flag("loading"),
        flag("unreadable"),
        mailbox,
        flag("busy"),
    ))
    .map_err(|e| e.to_string())
}

/// `review.settle {review?, answers}` through the link door: the state a
/// gathered review lands in ([`ReviewState::settled`]).
pub fn settle_json(args: &Value) -> Result<Value, String> {
    let review = args.get("review").filter(|r| !r.is_null());
    let answers = match args.get("answers") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => a.clone(),
        Some(_) => return Err("answers: not a list".to_owned()),
    };
    serde_json::to_value(ReviewState::settled(review, &answers)).map_err(|e| e.to_string())
}

/// `review.door {state?, review?}` through the link door: the [`door`].
pub fn door_json(args: &Value) -> Result<Value, String> {
    let state: Option<ReviewState> = match args.get("state") {
        None | Some(Value::Null) => None,
        Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| format!("state: {e}"))?),
    };
    let review = args.get("review").filter(|r| !r.is_null());
    serde_json::to_value(door(state.as_ref(), review)).map_err(|e| e.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_wire_is_tagged_by_kind() {
        assert_eq!(
            serde_json::to_value(ReviewState::Loading).unwrap(),
            json!({"kind": "loading"})
        );
        assert_eq!(
            serde_json::to_value(ReviewState::failed(REASON_TIMED_OUT)).unwrap(),
            json!({"kind": "failed", "reason_key": REASON_TIMED_OUT})
        );
    }

    #[test]
    fn only_a_ready_review_that_allows_it_opens_the_door() {
        let allowed = json!({"sign": {"allowed": true, "anyway": false}});
        let anyway = json!({"sign": {"allowed": true, "anyway": true}});
        let refused = json!({"sign": {"allowed": false, "anyway": false}});
        assert!(!door(None, Some(&allowed)).enabled);
        assert!(!door(Some(&ReviewState::Loading), Some(&allowed)).enabled);
        let failed = door(Some(&ReviewState::failed(REASON_TIMED_OUT)), Some(&allowed));
        assert!(!failed.enabled && failed.retry);
        assert!(door(Some(&ReviewState::Ready), Some(&allowed)).enabled);
        let a = door(Some(&ReviewState::Ready), Some(&anyway));
        assert!(a.enabled && a.anyway);
        assert!(!door(Some(&ReviewState::Ready), Some(&refused)).enabled);
        // A review without a sign rule, or no review at all, is never "allowed".
        assert!(!door(Some(&ReviewState::Ready), Some(&json!({}))).enabled);
        assert!(!door(Some(&ReviewState::Ready), None).enabled);
    }

    #[test]
    fn the_step_before_a_review_names_the_read_that_failed() {
        assert_eq!(UnreadRead::of(false, false), None);
        assert_eq!(
            UnreadRead::of(true, false).map(UnreadRead::draft_key),
            Some(UNREAD_CHAIN_KEY)
        );
        assert_eq!(
            UnreadRead::of(false, true).map(UnreadRead::draft_key),
            Some(UNREAD_SERVICE_KEY)
        );
        assert_eq!(
            UnreadRead::of(true, true).map(UnreadRead::draft_key),
            Some(UNREAD_BOTH_KEY)
        );
        assert_eq!(
            opening_reason_json(&json!({"chain": true, "link": true})).unwrap(),
            json!(LINK_UNREAD_CHAIN_KEY)
        );
        assert_eq!(opening_reason_json(&json!({})).unwrap(), Value::Null);
        // Bounded below the review's own bound, above one hung node's.
        const { assert!(OPEN_TIMEOUT_MS < REVIEW_TIMEOUT_MS && OPEN_TIMEOUT_MS > 9_500) };
    }

    #[test]
    fn a_review_neither_check_read_signs_only_anyway_with_try_again() {
        // Even a sign rule handed without its "anyway" cannot make it one tap.
        let unchecked = json!({
            "sign": {"allowed": true, "anyway": false},
            "concerns": [{"kind": "stateUnread", "reason": "unreachable"}, {"kind": "unsimulated"}, {"kind": "unchecked"}]
        });
        let d = door(Some(&ReviewState::Ready), Some(&unchecked));
        assert_eq!(d.shows, ReviewShows::Review);
        assert!(d.enabled && d.anyway && d.retry);
        assert_eq!(d.reason_key.as_deref(), Some(UNCHECKED_KEY));
        // One check unread alone: today's caution — plain, no line.
        let one = json!({
            "sign": {"allowed": true, "anyway": false},
            "concerns": [{"kind": "stateUnread", "reason": "unreachable"}]
        });
        let d = door(Some(&ReviewState::Ready), Some(&one));
        assert!(d.enabled && !d.anyway && !d.retry && d.reason_key.is_none());
    }
}
