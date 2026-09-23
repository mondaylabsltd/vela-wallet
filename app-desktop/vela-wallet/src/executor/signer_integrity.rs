//! Is the Clear Signer's page the page it is supposed to be? (spec 076 phase C)
//!
//! The core decides; this fetches. `vela_core::clear_signer::integrity` holds
//! the whole of the judgement — which hashes this build accepts, that the
//! person's deny-list outranks everything, that a check which cannot complete
//! is a check that failed — and every shell asks it the same question. What is
//! platform work, and therefore here, is three things: ask the endpoint what it
//! publishes, fetch a version by its hash, and hash the bytes that came back.
//!
//! **A plain HTTPS request, on purpose** (owner, 2026-09-23: 「我们先用 http
//! 请求 html 来判断吧」). Not a hidden WebView navigation: verifying by
//! navigation means loading the page, which means running the attacker's code,
//! and probe P2 measured what that code still gets out of a locked-down
//! WebView — a WebSocket handshake, and WebRTC over UDP. A plain request runs
//! not one line of it.
//!
//! What this catches is a REPLACED BUILD — a hijacked bucket, a bad CDN config,
//! a poisoned release — served to everyone at once. It does not catch a server
//! that serves bad bytes only to the browser and good ones to this check; the
//! spec says so plainly, and so must anything this produces.
//!
//! # Observe only, for now
//!
//! [`BUILD_ALLOWED`] ships empty until the page is published under
//! `/b/<sha256>/`, and an empty set opens nothing. So [`check`] is written to
//! be CALLED and LOGGED, not to gate the open: turning it into a gate before
//! there is a published page would brick a Clear Signer that works today. The
//! gate is one `if` away, and belongs in the commit that fills the set.

use std::time::Duration;

use serde_json::Value;
use vela_core::clear_signer::integrity::{self, BUILD_ALLOWED, CheckFailure, Page, Verdict};

use crate::executor::storage;

/// Hashes this person trusted on this device (FR-009), and hashes they blocked
/// (FR-010). Per device: these never sync, and Settings says so.
pub const KEY_SIGNER_TRUSTED: &str = "vela.signerPage.trusted";
pub const KEY_SIGNER_BLOCKED: &str = "vela.signerPage.blocked";

/// Where the endpoint lists what it still publishes (FR-002).
const INDEX_PATH: &str = "b/index.json";

/// Short on purpose: this runs in the background at an unpredictable moment
/// (FR-007), and a slow endpoint must not become a hang.
const TIMEOUT: Duration = Duration::from_secs(10);

/// The biggest page this will read. The signing page is one file of about
/// 300KB; a megabyte is room to grow and a refusal to hash something absurd.
const MAX_BYTES: usize = 4 * 1024 * 1024;

fn hashes_at(key: &str) -> Vec<String> {
    storage::read_value(key)
        .ok()
        .flatten()
        .as_ref()
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .filter_map(integrity::normalize_hash)
                .collect()
        })
        .unwrap_or_default()
}

/// What the endpoint says it publishes, as hashes.
///
/// The index has no authority — it narrows the choice and never makes it
/// (`choose_version`) — so a malformed or missing one is not an error worth
/// shouting about. It yields an empty list, and the caller falls back to asking
/// for a version directly.
fn published(base: &str) -> Vec<String> {
    let url = format!("{}{INDEX_PATH}", with_trailing_slash(base));
    let Ok(mut response) = crate::executor::proxy::agent(TIMEOUT).get(&url).call() else {
        return Vec::new();
    };
    if response.status() != 200 {
        return Vec::new();
    }
    let Ok(text) = response.body_mut().read_to_string() else {
        return Vec::new();
    };
    serde_json::from_str::<Value>(&text)
        .ok()
        .as_ref()
        .and_then(|value| value.get("versions").or(Some(value)))
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(Value::as_str)
                .filter_map(integrity::normalize_hash)
                .collect()
        })
        .unwrap_or_default()
}

fn with_trailing_slash(base: &str) -> String {
    if base.ends_with('/') {
        base.to_owned()
    } else {
        format!("{base}/")
    }
}

/// Fetch one version's bytes and hash them.
///
/// Every failure is the same answer — the check did not complete — because
/// that is what the core does with it: nothing opens (FR-006). A timeout, a
/// refused connection, a 404 and a body too large are not different verdicts;
/// they are all "this wallet could not see the page".
fn fetch_and_hash(url: &str) -> Result<String, CheckFailure> {
    let mut response = crate::executor::proxy::agent(TIMEOUT)
        .get(url)
        .call()
        .map_err(|_| CheckFailure::Unreachable)?;
    if response.status() != 200 {
        return Err(CheckFailure::Unreachable);
    }
    let mut bytes = Vec::new();
    response
        .body_mut()
        .with_config()
        .limit(MAX_BYTES as u64)
        .read_to_vec()
        .map(|read| bytes = read)
        .map_err(|_| CheckFailure::NoCachedBytes)?;
    Ok(integrity::hash_page(&bytes))
}

/// The verdict for the page at `base`, having actually gone and looked.
///
/// `base` is the address Settings holds — the official page, or one the person
/// deployed themselves.
#[must_use]
pub fn check(base: &str) -> Verdict {
    let trusted = hashes_at(KEY_SIGNER_TRUSTED);
    let blocked = hashes_at(KEY_SIGNER_BLOCKED);

    // Which version to ask for. The endpoint's index narrows the candidates;
    // the order is this client's, so the endpoint cannot steer the choice.
    let available = published(base);
    let wanted = integrity::choose_version(&available, &trusted, &blocked);

    let Some(hash) = wanted else {
        // Nothing this wallet accepts is published — or, today, this build
        // accepts nothing at all because `BUILD_ALLOWED` is still empty.
        return Verdict::CouldNotCheck(CheckFailure::NotChecked);
    };
    let Some(url) = integrity::content_addressed_url(base, &hash) else {
        // A custom deployment has no content-addressed path; its page is
        // wherever the person said it is.
        return verdict_for(
            base,
            fetch_and_hash(&with_trailing_slash(base)),
            &trusted,
            &blocked,
        );
    };
    verdict_for(base, fetch_and_hash(&url), &trusted, &blocked)
}

/// The pure half: bytes-or-failure in, the core's verdict out.
fn verdict_for(
    base: &str,
    observed: Result<String, CheckFailure>,
    trusted: &[String],
    blocked: &[String],
) -> Verdict {
    let (hash, failure) = match observed {
        Ok(hash) => (Some(hash), CheckFailure::NotChecked),
        Err(why) => (None, why),
    };
    integrity::decide(&Page {
        url: base,
        observed: hash.as_deref(),
        failure,
        trusted,
        blocked,
        // FR-009: only ever honoured for a custom address, and the core is
        // what enforces that. Settings will own this in phase E; until then
        // the check is at its strictest.
        verification_off: false,
    })
}

/// One line for the log, saying what was found without claiming more than the
/// check can support.
///
/// **Never "a distribution attack was prevented".** The spec's threat model is
/// explicit: this catches a replaced build, and cannot tell that apart from a
/// wallet that is simply older than the page. The true sentence is the one
/// about hashes.
#[must_use]
pub fn describe(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Open => "signer page: matches a version this build accepts".to_owned(),
        Verdict::OpenUnverified => {
            "signer page: NOT checked — verification is off for this address".to_owned()
        }
        Verdict::Denied { hash } => {
            format!(
                "signer page: {} is on this device's block list",
                &hash[..16]
            )
        }
        Verdict::Refused { actual, expected } => format!(
            "signer page: served {}…, which is not one of the {} version(s) this build knows",
            &actual[..16],
            expected.len()
        ),
        Verdict::AskToTrust { actual } => format!(
            "signer page: {}… is a version this device has not decided about",
            &actual[..16]
        ),
        Verdict::CouldNotCheck(why) => format!("signer page: could not be checked ({why:?})"),
    }
}

/// Whether this build can gate on the check at all.
///
/// False while [`BUILD_ALLOWED`] is empty: with nothing to accept, gating would
/// refuse every page, including the good one. The call site logs instead.
#[must_use]
pub fn can_enforce() -> bool {
    !BUILD_ALLOWED.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aa11223344556677889900aabbccddeeff00112233445566778899aabbccddee";

    #[test]
    fn a_fetch_that_failed_opens_nothing() {
        // FR-006, through this shell's own path: every way a request can fail
        // is the same answer, because that is what the core does with it.
        for why in [
            CheckFailure::Unreachable,
            CheckFailure::NoCachedBytes,
            CheckFailure::NotChecked,
        ] {
            let verdict = verdict_for("https://sign.getvela.app/", Err(why), &[], &[]);
            assert_eq!(verdict, Verdict::CouldNotCheck(why));
        }
    }

    #[test]
    fn bytes_this_person_trusted_open_on_their_own_address() {
        let trusted = vec![A.to_owned()];
        let verdict = verdict_for(
            "https://signer.example.test/",
            Ok(A.to_owned()),
            &trusted,
            &[],
        );
        assert_eq!(verdict, Verdict::Open);
    }

    #[test]
    fn the_block_list_still_wins_here() {
        let trusted = vec![A.to_owned()];
        let blocked = vec![A.to_owned()];
        let verdict = verdict_for(
            "https://signer.example.test/",
            Ok(A.to_owned()),
            &trusted,
            &blocked,
        );
        assert!(matches!(verdict, Verdict::Denied { .. }));
    }

    #[test]
    fn the_words_never_claim_an_attack_was_stopped() {
        // The spec's threat model allows one sentence and not the other: this
        // cannot tell a replaced build from a wallet older than the page.
        let verdict = Verdict::Refused {
            actual: A.to_owned(),
            expected: Vec::new(),
        };
        let said = describe(&verdict);
        assert!(said.contains("not one of the"), "{said}");
        for forbidden in ["attack", "prevented", "malicious", "compromis"] {
            assert!(
                !said.to_ascii_lowercase().contains(forbidden),
                "the log claimed more than the check can support: {said}"
            );
        }
    }

    #[test]
    fn this_build_cannot_enforce_yet_and_says_so() {
        // The guard that keeps phase C from bricking a working Clear Signer.
        assert!(
            !can_enforce(),
            "BUILD_ALLOWED filled: wire the gate, and say so in 076"
        );
    }

    #[test]
    fn a_trailing_slash_is_added_once_and_only_once() {
        assert_eq!(with_trailing_slash("https://a.test"), "https://a.test/");
        assert_eq!(with_trailing_slash("https://a.test/"), "https://a.test/");
    }
}
