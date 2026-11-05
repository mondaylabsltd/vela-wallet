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
