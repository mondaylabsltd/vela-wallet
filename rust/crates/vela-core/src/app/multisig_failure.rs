//! Round 16 leftover 4: what a failed multisig action says — a stable KIND,
//! worded from the catalog, never the English of whoever failed.
//!
//! A failed action (`ActionState::Failed { detail }`, the accounts machine's
//! create and share-again failures) carries its `detail` as JSON with a
//! `kind`: the shell's own failures as the shell wrote them (`passkey`,
//! `page_declined`, `not_sent`, `cannot_pay`, …), the core's own as
//! [`passkey_check`] or [`core_reason`] (`{"kind":"unknown","reason":…}` —
//! a stable code, for a log, never words). Anything else a shell hands the
//! core as plain text is served as [`unknown`]: its text is dropped.
//!
//! [`failure_said`] reads any detail into a [`FailureKind`] and, for every
//! kind whose words need no more than the network's name, the catalog key
//! that says it. The kinds whose words are built from the detail's own facts
//! — the fee that moved, what the executor has and needs, the signing venue's
//! block — carry no key: a shell words them from those facts. No shell reads
//! a detail's text or matches on it; the shells map kinds. Replayed by every
//! shell through the link door `review.failure {detail}` (vectors
//! `tests/vectors/multisig/review-state.json` `failure`).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// What a failure was, as every shell words it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// The passkey prompt timed out.
    PasskeyTimedOut,
    /// The passkey prompt was closed.
    PasskeyClosed,
    /// This device has no passkey for the member.
    PasskeyMissing,
    /// Any other passkey failure.
    PasskeyFailed,
    /// What the passkey (or a page) signed does not check out as the
    /// member's.
    PasskeyCheck,
    /// The member's signing venue blocks it (worded from its `block`).
    VenueBlocked,
    /// Declined on the trusted signing page.
    PageDeclined,
    /// The signing page did not answer.
    PageUnanswered,
    /// The page's answer does not match the request.
    PageMismatch,
    /// The signing page could not be opened.
    PageNotOpened,
    /// The signing page refused.
    PageRefused,
    /// The relay provably never took it: nothing was sent.
    NotSent,
    /// The executor cannot pay the fee (worded from `need` and `have`).
    CannotPay,
    /// The fee moved since the press (worded from `amount` and `executor`).
    FeeMoved,
    /// A read before the passkey on a network that is failing
    /// (`{{network}}`).
    ChainUnreachable,
    /// r18-8 (§28, §32): this test build refused to write to the live
    /// service — nothing was sent (`common.testBuild.productionWriteRefused`).
    /// Never worded as the network, never retried by itself.
    ProductionWriteRefused,
    /// Anything else: "Something went wrong, so nothing was sent."
    Unknown,
}

/// A failure's kind and, when its words need nothing but the network's
/// name, the catalog key that says them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureSaid {
    pub kind: FailureKind,
    pub key: Option<String>,
}

const REASON_KEY: &str = "home.multisig.proposal.reason.";

/// The core's own: what was signed does not check out as the member's.
pub fn passkey_check() -> String {
    json!({ "kind": "passkey_check" }).to_string()
}

/// The core's own refusal, as a stable code (for a log; said as
/// [`FailureKind::Unknown`]).
pub fn core_reason(reason: &str) -> String {
    json!({ "kind": "unknown", "reason": reason }).to_string()
}

/// r18-8: this test build refused the write before it left (§28).
pub fn production_write_refused() -> String {
    json!({ "kind": "production_write_refused" }).to_string()
}

/// Nothing that can be said more precisely.
pub fn unknown() -> String {
    json!({ "kind": "unknown" }).to_string()
}

/// A detail as the views serve it: JSON with a `kind` stays as it is;
/// anything else (a shell's plain text, an error's English) becomes
/// [`unknown`] — its text is never handed to a shell to draw.
pub fn served(detail: &str) -> String {
    match kind_of(detail) {
        Some(_) => detail.to_owned(),
        None => unknown(),
    }
}

fn parsed(detail: &str) -> Option<Value> {
    if !detail.trim_start().starts_with('{') {
        return None;
    }
    serde_json::from_str::<Value>(detail)
        .ok()
        .filter(|v| v.get("kind").and_then(Value::as_str).is_some())
}

fn kind_of(detail: &str) -> Option<String> {
    parsed(detail).and_then(|v| v.get("kind").and_then(Value::as_str).map(str::to_owned))
}

/// [`FailureSaid`] of any detail.
pub fn failure_said(detail: &str) -> FailureSaid {
    let said = |kind: FailureKind, key: Option<String>| FailureSaid { kind, key };
    let reason = |leaf: &str| Some(format!("{REASON_KEY}{leaf}"));
    let unknown = || said(FailureKind::Unknown, reason("unknown"));
    let Some(v) = parsed(detail) else {
        return unknown();
    };
    let field = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default();
    match field("kind") {
        "passkey" => {
            if v.get("timedOut").and_then(Value::as_bool) == Some(true) {
                said(FailureKind::PasskeyTimedOut, reason("passkeyTimedOut"))
            } else {
                match field("failure") {
                    "cancelled" => said(FailureKind::PasskeyClosed, reason("passkeyClosed")),
                    "not_discoverable" | "not_supported" => {
                        said(FailureKind::PasskeyMissing, reason("passkeyMissing"))
                    }
                    _ => said(FailureKind::PasskeyFailed, reason("passkeyFailed")),
                }
            }
        }
        "passkey_check" => said(FailureKind::PasskeyCheck, reason("passkeyFailed")),
        "venue_blocked" => said(FailureKind::VenueBlocked, None),
        "page_declined" => said(
            FailureKind::PageDeclined,
            Some("componentsUi.signing.trustedSignerClosed".to_owned()),
        ),
        "page_sign_failed" | "page_approval" => {
            let page = match field("page") {
                "" => field("failure"),
                page => page,
            };
            let (kind, leaf) = match page {
                "unanswered" | "silent" => (FailureKind::PageUnanswered, "trustedSignerTimeout"),
                "mismatch" | "unverified" => (FailureKind::PageMismatch, "trustedSignerMismatch"),
                "not_opened" => (FailureKind::PageNotOpened, "signerDown"),
                _ => (FailureKind::PageRefused, "trustedSignerRefused"),
            };
            said(kind, Some(format!("componentsUi.signing.{leaf}")))
        }
        "not_sent" => said(
            FailureKind::NotSent,
            Some("home.multisig.proposal.notSent".to_owned()),
        ),
        "cannot_pay" => said(FailureKind::CannotPay, None),
        "fee_moved" => said(FailureKind::FeeMoved, None),
        "chain_unreachable" => said(FailureKind::ChainUnreachable, reason("chainUnreachable")),
        "production_write_refused" => said(
            FailureKind::ProductionWriteRefused,
            Some(crate::service_defaults::REFUSED_KEY.to_owned()),
        ),
        _ => unknown(),
    }
}

/// `review.failure {detail}` through the link door: [`failure_said`].
pub fn failure_json(args: &Value) -> Result<Value, String> {
    let detail = args.get("detail").and_then(Value::as_str).unwrap_or("");
    serde_json::to_value(failure_said(detail)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_served_as_unknown_and_json_as_it_is() {
        assert_eq!(served("Error: relay said 500"), unknown());
        assert_eq!(served(""), unknown());
        assert_eq!(served("{not json"), unknown());
        assert_eq!(served(r#"{"no":"kind"}"#), unknown());
        let moved = r#"{"kind":"fee_moved","amount":"1"}"#;
        assert_eq!(served(moved), moved);
        assert_eq!(served(&passkey_check()), passkey_check());
        assert_eq!(
            failure_said(&core_reason("nothing_to_execute")).kind,
            FailureKind::Unknown
        );
    }
}
