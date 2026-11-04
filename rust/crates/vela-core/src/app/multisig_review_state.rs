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
    /// `failed`: the reason's corpus key.
    pub reason_key: Option<String>,
    /// `failed`: offer "Try again" (`review_again`).
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
            SignDoor {
                shows: ReviewShows::Review,
                enabled: allowed,
                anyway: allowed && sign["anyway"].as_bool() == Some(true),
                reason_key: None,
                retry: false,
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
}
