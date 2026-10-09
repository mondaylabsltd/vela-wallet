//! Is the signing page the page it is supposed to be? (spec 076 phase C, and
//! spec 102 R6)
//!
//! The core decides; this fetches. `vela_core::trusted_signer::launch` holds
//! the whole of the rule — which version of a page to open, that the URL
//! fetched to be checked is the URL that is opened, that the bytes there must
//! BE that version, that the person's deny-list outranks everything, that a
//! check which cannot complete is a check that failed — and every shell asks it
//! the same question. What is platform work, and therefore here, is three
//! things: ask the endpoint what it publishes, fetch the target's URL, and hash
//! the bytes that came back.
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
//! # Refusing, since spec 102
//!
//! `integrity::ENFORCE` is on. A check that admits the page leaves a
//! [`CheckedPage`] here, and that is the only thing a launch can be built
//! from ([`checked_page`]); a page whose check failed, or never ran, is not
//! opened. The check still runs at launch, off to one side (FR-007), and again
//! whenever the one it left is too old to vouch for the page.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;
use vela_core::trusted_signer::integrity::{self, CheckFailure, NoVersion, Verdict};
use vela_core::trusted_signer::launch::{self, Admission, CheckedPage, IntegrityLine, Target};

use crate::executor::storage;

/// Hashes this person trusted on this device (FR-009), and hashes they blocked
/// (FR-010). Per device: these never sync, and Settings says so.
pub const KEY_SIGNER_TRUSTED: &str = "vela.signerPage.trusted";
pub const KEY_SIGNER_BLOCKED: &str = "vela.signerPage.blocked";

/// Where the endpoint lists what it still publishes (FR-002).
///
/// The deployment IS `app-web/trusted-signer/dist/`: an index at its root and
/// every published version under `b/<sha256>/sign.html`. Copying that directory
/// is the whole of publishing.
const INDEX_PATH: &str = "index.json";

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

/// What reading the endpoint's index came to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Index {
    /// The endpoint answered: what it lists (empty for a missing or
    /// malformed index — it has no authority, so that is no error).
    Listed(Vec<String>),
    /// Nothing answered: the network, a proxy, a name — never the page
    /// (spec 082 G67). `kind` is the probe's word for the failure.
    Unfetched { host: String, kind: &'static str },
}

/// What the endpoint says it publishes, as hashes.
///
/// The index has no authority — it narrows the choice and never makes it
/// (`choose_version`) — so a malformed or missing one is not an error worth
/// shouting about. It yields an empty list, and the caller falls back to asking
/// for a version directly. A request that reached nothing is another matter:
/// "publishes no version this wallet knows — update the wallet" was logged for
/// a dead proxy (spec 082 G67).
fn published(base: &str) -> Index {
    let url = format!("{}{INDEX_PATH}", with_trailing_slash(base));
    let mut response =
        match crate::executor::proxy::with_routes(&url, TIMEOUT, |agent| agent.get(&url).call()) {
            Ok(response) => response,
            Err(failure) => return unanswered(&url, &failure),
        };
    if response.status() != 200 {
        return Index::Listed(Vec::new());
    }
    let Ok(text) = response.body_mut().read_to_string() else {
        return Index::Listed(Vec::new());
    };
    Index::Listed(listed(&text))
}

/// A request for the index that did not come back with a body: an HTTP
/// status is an answer (no index, which is allowed); anything else reached
/// nothing.
fn unanswered(url: &str, failure: &crate::executor::proxy::Transport) -> Index {
    if matches!(failure.error, ureq::Error::StatusCode(_)) {
        return Index::Listed(Vec::new());
    }
    Index::Unfetched {
        host: crate::diag::host_of(url),
        kind: crate::explore::probe::verdict_word(Err(crate::explore::probe::code_of(
            &failure.error,
            failure.proxy.as_ref(),
        ))),
    }
}

/// The index's hashes, from its body.
fn listed(text: &str) -> Vec<String> {
    serde_json::from_str::<Value>(text)
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
    let mut response =
        crate::executor::proxy::with_routes(url, TIMEOUT, |agent| agent.get(url).call())
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

/// The verdict for the page at `base`, having actually gone and looked — and,
/// when it admits the page, the checked page left for [`checked_page`].
///
/// `base` is the page an account signs on, or one in Settings → Signing pages.
/// **Blocks** on the network (two requests, each within [`TIMEOUT`]): run it
/// off the frame.
#[must_use]
pub fn check(base: &str) -> Verdict {
    check_with(
        base,
        &hashes_at(KEY_SIGNER_TRUSTED),
        &hashes_at(KEY_SIGNER_BLOCKED),
    )
}

/// The same, with the two per-device lists handed in.
///
/// Separated so the network path can be exercised against a real server
/// without a configured store — which is how it was first driven end to end,
/// over a LAN address, before anything was published.
#[must_use]
pub fn check_with(base: &str, trusted: &[String], blocked: &[String]) -> Verdict {
    let base = key_of(base);
    let now = crate::executor::now_ms() as u64;
    with_board(|board| board.running.insert(base.clone()));
    let outcome = admission_with(&base, trusted, blocked, now);
    let verdict = match &outcome {
        Ok(Admission::Admitted(page)) => page.verdict().clone(),
        Ok(Admission::Refused { verdict, .. }) | Err(verdict) => verdict.clone(),
    };
    record(&base, outcome);
    verdict
}

/// R6 through this shell's own I/O: which version to open (the endpoint's index
/// narrows, the core chooses), fetch EXACTLY that version's URL, hash the
/// bytes, and let the core rule. `Err` when there was no version to ask for.
fn admission_with(
    base: &str,
    trusted: &[String],
    blocked: &[String],
    now_ms: u64,
) -> Result<Admission, Verdict> {
    let target = choose(base, &published(base), trusted, blocked)?;
    Ok(admit_for(
        &target,
        fetch_and_hash(target.url()),
        trusted,
        blocked,
        now_ms,
    ))
}

/// The version to ask for, or the verdict without one.
///
/// The index has no authority and is an optimisation, not a dependency: one
/// that never answered is logged ("could not fetch …", spec 082 G67) and the
/// version is asked for directly — the official page's launch version, which
/// is deployed by definition. Nothing to ask for, and WHY, still matters:
/// "this endpoint publishes nothing this wallet knows" sends a person to
/// update, and "you have blocked every usable version" to their own list.
fn choose(
    base: &str,
    index: &Index,
    trusted: &[String],
    blocked: &[String],
) -> Result<Target, Verdict> {
    let listed = match index {
        Index::Unfetched { host, kind } => {
            crate::diag::vlog!("signer page", "{}", fetch_line(host, kind));
            None
        }
        Index::Listed(available) => Some(available.as_slice()),
    };
    launch::target(base, listed, trusted, blocked).map_err(Verdict::NoVersionToAsk)
}

/// The log line for an index that never answered (the area is the caller's
/// `signer page`).
fn fetch_line(host: &str, kind: &str) -> String {
    format!("could not fetch {host} ({kind})")
}

/// The pure half: the target's bytes-or-failure in, the core's ruling out.
fn admit_for(
    target: &Target,
    observed: Result<String, CheckFailure>,
    trusted: &[String],
    blocked: &[String],
    now_ms: u64,
) -> Admission {
    let (hash, failure) = match observed {
        Ok(hash) => (Some(hash), CheckFailure::NotChecked),
        Err(why) => (None, why),
    };
    launch::admit(
        target,
        hash.as_deref(),
        failure,
        trusted,
        blocked,
        // FR-009: only ever honoured for a custom address, and the core is
        // what enforces that. Settings will own this; until then the check
        // is at its strictest.
        false,
        now_ms,
    )
}

/// What the checks so far came to, by the page's normal address.
///
/// - `checked` — pages a check ADMITTED: what the launch path opens without a
///   network round trip (a person pressing Open must not wait on one, and a
///   unit test of the launch path must not reach the internet: three tests
///   that used to take milliseconds once took five minutes);
/// - `refused` — the line of the last check that did NOT admit a page, so the
///   hand-off card, Settings and the choosers can say why it will not open
///   (a refusal is not "checking…" forever);
/// - `running` — checks under way, which read as "checking" and are not
///   started twice.
#[derive(Default)]
struct Board {
    checked: HashMap<String, CheckedPage>,
    refused: HashMap<String, IntegrityLine>,
    running: HashSet<String>,
    /// Who hears that a check finished — the screens that draw a line.
    listeners: Vec<UnboundedSender<()>>,
}

static BOARD: LazyLock<Mutex<Board>> = LazyLock::new(|| Mutex::new(Board::default()));

fn with_board<T>(change: impl FnOnce(&mut Board) -> T) -> T {
    change(
        &mut BOARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}

/// The one spelling of a page's address the board keys on: the core's
/// normalisation (`https://sign.getvela.app/`), so an account's venue, a saved
/// page and a ceremony's `page` all find the same check.
#[must_use]
pub fn key_of(base: &str) -> String {
    vela_core::trusted_signer::signer_url(base).unwrap_or_else(|_| base.trim().to_owned())
}

/// Put a finished check on the board and tell whoever is drawing a line.
fn record(base: &str, outcome: Result<Admission, Verdict>) {
    let listeners = with_board(|board| {
        board.running.remove(base);
        match outcome {
            Ok(Admission::Admitted(page)) => {
                board.refused.remove(base);
                board.checked.insert(base.to_owned(), page);
            }
            Ok(refused @ Admission::Refused { .. }) => {
                board.checked.remove(base);
                board.refused.insert(base.to_owned(), refused.line());
            }
            Err(verdict) => {
                board.checked.remove(base);
                board
                    .refused
                    .insert(base.to_owned(), IntegrityLine::of(&verdict, "", None));
            }
        }
        board.listeners.retain(|tx| !tx.is_closed());
        board.listeners.clone()
    });
    for listener in listeners {
        let _ = listener.unbounded_send(());
    }
}

/// A stream that says "a check finished" — the screens' cue to draw their
/// lines again. Nothing is lost by a listener that goes away.
#[must_use]
pub fn subscribe() -> UnboundedReceiver<()> {
    let (tx, rx) = unbounded();
    with_board(|board| board.listeners.push(tx));
    rx
}

/// The page to OPEN for `base`: the version that was checked, at the address
/// that was checked — or `None`, and nothing opens, when no check admitted it
/// or the one that did is too old to vouch for it (check again first).
///
/// This replaces opening "the remembered version, else the address as typed":
/// the fallback to the bare address opened bytes nobody had checked (spec 102
/// R6, `ENFORCE` on).
#[must_use]
pub fn checked_page(base: &str, now_ms: u64) -> Option<CheckedPage> {
    let base = key_of(base);
    with_board(|board| {
        board
            .checked
            .get(&base)
            .filter(|page| page.is_fresh(now_ms))
            .cloned()
    })
}

/// The integrity line for `base` at `now_ms` — what the hand-off card,
/// Settings → Signing pages and the choosers draw: the admitted page's line
/// while it is fresh; "checking" while a check runs (or none has finished);
/// otherwise why the last check did not admit it.
#[must_use]
pub fn line(base: &str, now_ms: u64) -> IntegrityLine {
    let base = key_of(base);
    with_board(|board| {
        if let Some(page) = board.checked.get(&base).filter(|p| p.is_fresh(now_ms)) {
            return page.line(now_ms);
        }
        if board.running.contains(&base) {
            return IntegrityLine::checking();
        }
        board
            .refused
            .get(&base)
            .cloned()
            .unwrap_or_else(IntegrityLine::checking)
    })
}

/// Should a screen start a check of `base` now? Only when nothing vouches for
/// it and nothing is running — never again for a page whose last check
/// REFUSED it: that one is checked again when somebody asks (Retry, or Open,
/// which checks before it launches), not on every frame that draws its line.
#[must_use]
pub fn wants_check(base: &str, now_ms: u64) -> bool {
    let base = key_of(base);
    with_board(|board| {
        !board.running.contains(&base)
            && !board.refused.contains_key(&base)
            && board
                .checked
                .get(&base)
                .is_none_or(|page| !page.is_fresh(now_ms))
    })
}

/// Forget the last refusal of `base`, so the next look checks it again — the
/// "check again" a refused line offers.
pub fn forget_refusal(base: &str) {
    let base = key_of(base);
    with_board(|board| board.refused.remove(&base));
}

/// Check `base` on a thread of its own when [`wants_check`] says so. The
/// result reaches the screens through [`subscribe`].
pub fn check_in_background(base: &str) {
    let now = crate::executor::now_ms() as u64;
    if !wants_check(base, now) {
        return;
    }
    let base = key_of(base);
    // Marked before the thread starts, so a second frame in the same breath
    // does not start a second check.
    with_board(|board| board.running.insert(base.clone()));
    let checking = base.clone();
    let spawned = std::thread::Builder::new()
        .name("signer-page-check".to_owned())
        .spawn(move || {
            let verdict = check(&checking);
            crate::diag::vlog!("signer page", "{}", unprefixed(&describe(&verdict)));
        });
    if spawned.is_err() {
        with_board(|board| board.running.remove(&base));
    }
}

/// Tests elsewhere in this crate that launch a page: leave an admitted check
/// of `base` on the board, as a check that found the bytes in order would —
/// the launch path then opens without reaching the network. The official page
/// is admitted at its launch version; any other at a version "trusted on this
/// device".
#[cfg(test)]
pub(crate) fn admit_for_tests(base: &str) {
    const TRUSTED: &str = "7e57000000000000000000000000000000000000000000000000000000007e57";
    let base = key_of(base);
    let now = crate::executor::now_ms() as u64;
    let trusted = vec![TRUSTED.to_owned()];
    let (target, observed) = if integrity::is_official(&base) {
        (
            launch::target(&base, None, &[], &[]),
            integrity::LAUNCH.to_owned(),
        )
    } else {
        (
            launch::target(&base, Some(&trusted), &trusted, &[]),
            TRUSTED.to_owned(),
        )
    };
    let target = target.unwrap_or_else(|_| unreachable!("a version to ask for"));
    let admission = admit_for(&target, Ok(observed), &trusted, &[], now);
    assert!(admission.page().is_some(), "{admission:?}");
    record(&base, Ok(admission));
}

/// [`describe`]'s line without its own `signer page: `, for a log line whose
/// area already says it.
#[must_use]
pub fn unprefixed(line: &str) -> &str {
    line.strip_prefix("signer page: ").unwrap_or(line)
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
        Verdict::NoVersionToAsk(NoVersion::NothingPublishedThisWalletKnows) => {
            "signer page: this address publishes no version this wallet knows — update the wallet"
                .to_owned()
        }
        Verdict::NoVersionToAsk(NoVersion::EverythingUsableIsBlocked) => {
            "signer page: every version this wallet could use is on this device's block list"
                .to_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::trusted_signer::integrity::BUILD_ALLOWED;

    const A: &str = "aa11223344556677889900aabbccddeeff00112233445566778899aabbccddee";

    fn official() -> Target {
        launch::target("https://sign.getvela.app/", None, &[], &[])
            .unwrap_or_else(|_| unreachable!())
    }

    fn custom(version: &str) -> Target {
        launch::target(
            "https://signer.example.test/",
            Some(&[version.to_owned()]),
            &[],
            &[],
        )
        .unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn a_fetch_that_failed_opens_nothing() {
        // FR-006, through this shell's own path: every way a request can fail
        // is the same answer, because that is what the core does with it.
        for why in [
            CheckFailure::Unreachable,
            CheckFailure::NoCachedBytes,
            CheckFailure::NotChecked,
        ] {
            let admission = admit_for(&official(), Err(why), &[], &[], 1);
            assert_eq!(admission.page(), None);
            assert!(matches!(
                admission,
                Admission::Refused { verdict: Verdict::CouldNotCheck(w), .. } if w == why
            ));
        }
    }

    #[test]
    fn bytes_this_person_trusted_open_on_their_own_address() {
        let trusted = vec![A.to_owned()];
        let admission = admit_for(&custom(A), Ok(A.to_owned()), &trusted, &[], 1);
        assert_eq!(
            admission.page().map(|page| page.verdict().clone()),
            Some(Verdict::Open)
        );
    }

    #[test]
    fn the_block_list_still_wins_here() {
        let trusted = vec![A.to_owned()];
        let blocked = vec![A.to_owned()];
        let admission = admit_for(&custom(A), Ok(A.to_owned()), &trusted, &blocked, 1);
        assert!(matches!(
            admission,
            Admission::Refused {
                verdict: Verdict::Denied { .. },
                ..
            }
        ));
    }

    /// R6: what is opened is what was checked — the admitted page's address is
    /// the one this shell fetched.
    ///
    /// Its own address: the board is the process's, and the official page is
    /// what other modules' tests seed as admitted.
    #[test]
    fn the_page_left_for_the_launch_is_the_one_that_was_checked() {
        const BASE: &str = "https://board-left.example.test/";
        let trusted = vec![A.to_owned()];
        let target = launch::target(BASE, Some(&[A.to_owned()]), &trusted, &[])
            .unwrap_or_else(|_| unreachable!());
        let admission = admit_for(&target, Ok(A.to_owned()), &trusted, &[], 1_000);
        record(BASE, Ok(admission));
        let page = checked_page(BASE, 2_000).unwrap_or_else(|| unreachable!());
        assert_eq!(page.target().url(), target.url());
        // The line says what was checked, and that it opens.
        let said = line(BASE, 2_000);
        assert!(said.opens, "{said:?}");
        assert_eq!(said.version, &A[..8]);
        // Too old to vouch for anything: nothing opens until it is checked again,
        // and a screen asks for that check.
        let stale = 1_000 + launch::MAX_CHECK_AGE_MS + 1;
        assert!(checked_page(BASE, stale).is_none());
        assert!(wants_check(BASE, stale));
        assert!(!wants_check(BASE, 2_000), "a fresh check is not run again");
        // A refused check leaves nothing to open — and says why, rather than
        // "checking…" forever, and is not re-run on every frame.
        let refused = admit_for(
            &target,
            Err(CheckFailure::Unreachable),
            &trusted,
            &[],
            3_000,
        );
        record(BASE, Ok(refused));
        assert!(checked_page(BASE, 3_000).is_none());
        let said = line(BASE, 3_000);
        assert!(!said.opens);
        assert_eq!(said.key, "componentsUi.signing.integrity.couldNotCheck");
        assert!(!wants_check(BASE, 3_000));
        // "Check again" forgets the refusal; the next look checks.
        forget_refusal(BASE);
        assert!(wants_check(BASE, 3_000));
    }

    /// Every spelling of one page is one entry on the board.
    #[test]
    fn a_page_is_one_entry_however_its_address_was_written() {
        assert_eq!(
            key_of("https://sign.getvela.app"),
            "https://sign.getvela.app/"
        );
        assert_eq!(key_of(" SIGN.getvela.app "), "https://sign.getvela.app/");
        assert_eq!(
            key_of("http://LOCALHOST:8140/clearsigning/"),
            "http://localhost:8140/clearsigning/"
        );
    }

    /// A finished check is announced to whoever draws a line.
    #[test]
    fn a_finished_check_is_announced() {
        const BASE: &str = "https://board-announce.example.test/";
        let mut heard = subscribe();
        record(
            BASE,
            Err(Verdict::NoVersionToAsk(
                NoVersion::NothingPublishedThisWalletKnows,
            )),
        );
        assert!(heard.try_recv().is_ok());
        assert_eq!(
            line(BASE, 1).key,
            "componentsUi.signing.integrity.noVersion"
        );
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
    fn this_build_refuses_a_page_that_failed_its_check() {
        // Spec 102 R6: the page has been published at the official address
        // since 079, and a failed or missing check opens nothing.
        // The core owns the switch (`integrity::ENFORCE`); this build obeys it.
        const { assert!(integrity::ENFORCE, "102: enforcement is off") };
    }

    /// The whole chain, over a real socket: index → choose → fetch by hash →
    /// hash the bytes → the core's verdict.
    ///
    /// Run with a `dist/` served somewhere reachable:
    ///
    /// ```sh
    /// (cd app-web/trusted-signer/dist && python3 -m http.server 8920)
    /// SIGNER_DIST=http://127.0.0.1:8920/ cargo test signer_integrity -- --ignored --nocapture
    /// ```
    ///
    /// Ignored by default because it needs that server; it is the test that
    /// proved the design works before anything was published, against a LAN
    /// address rather than the production host.
    #[test]
    #[ignore = "needs a dist/ served at $SIGNER_DIST"]
    fn the_whole_chain_against_a_served_dist() {
        let Ok(base) = std::env::var("SIGNER_DIST") else {
            return;
        };
        let verdict = check_with(&base, &[], &[]);
        println!("LAN check({base}) → {verdict:?}\n  {}", describe(&verdict));
        assert_eq!(
            verdict,
            Verdict::Open,
            "a dist/ this build ships a hash for must open"
        );

        // And the deny-list still outranks the build's own set, over the wire.
        // Blocking the only version this build ships means there is nothing to
        // ask for — and the person is told THAT, not "could not be checked".
        let blocked = vec![BUILD_ALLOWED[0].to_owned()];
        let denied = check_with(&base, &[], &blocked);
        println!("LAN check with it blocked → {}", describe(&denied));
        assert_eq!(
            denied,
            Verdict::NoVersionToAsk(NoVersion::EverythingUsableIsBlocked)
        );
        assert!(describe(&denied).contains("block list"));
    }

    #[test]
    #[ignore = "needs a dist/ served at $SIGNER_DIST"]
    fn the_page_opened_is_the_page_that_was_checked() {
        let Ok(base) = std::env::var("SIGNER_DIST") else {
            return;
        };
        let verdict = check_with(&base, &[], &[]);
        assert_eq!(verdict, Verdict::Open);
        let now = crate::executor::now_ms() as u64;
        let opened = checked_page(&base, now).unwrap_or_else(|| unreachable!("admitted"));
        println!("checked_page({base}) → {}", opened.target().url());
        // It must be the content-addressed path of a version this build knows,
        // not the bare address — otherwise the check and the browser look at
        // different bytes.
        assert!(
            opened.target().url().contains("/b/") && opened.target().url().ends_with("/sign.html"),
            "opened the wrong page: {}",
            opened.target().url()
        );
        assert!(
            opened.target().url().contains(BUILD_ALLOWED[0]),
            "opened an unknown version: {}",
            opened.target().url()
        );
    }

    /// Spec 082 G67: an index request that reached nothing (a dead dev
    /// proxy) is "could not fetch <host> (<kind>)" and "could not be checked"
    /// — never "publishes no version … update the wallet".
    #[test]
    fn a_transport_failure_is_the_fetch_line_not_update_the_wallet() {
        use crate::executor::proxy::{ProxyFailure, ProxyFailureKind, Transport};
        let url = "https://sign.getvela.app/versions.json";
        let dead = Transport {
            error: ureq::Error::ConnectionFailed,
            local: true,
            proxy: Some(ProxyFailure {
                proxy: "127.0.0.1:9".to_owned(),
                kind: ProxyFailureKind::Unreachable,
            }),
        };
        let index = unanswered(url, &dead);
        assert_eq!(
            index,
            Index::Unfetched {
                host: "sign.getvela.app".to_owned(),
                kind: "proxy"
            }
        );
        assert_eq!(
            fetch_line("sign.getvela.app", "proxy"),
            "could not fetch sign.getvela.app (proxy)"
        );
        // The index is an optimisation, not a dependency: with none, the
        // official page's launch version is asked for directly — never "update
        // the wallet".
        let target = choose("https://sign.getvela.app/", &index, &[], &[])
            .unwrap_or_else(|_| unreachable!("the launch version is always there to ask for"));
        assert_eq!(target.version(), integrity::LAUNCH);
        // An HTTP status is an answer: no index, which is allowed.
        let missing = Transport {
            error: ureq::Error::StatusCode(404),
            local: false,
            proxy: None,
        };
        assert_eq!(unanswered(url, &missing), Index::Listed(Vec::new()));
    }

    /// An endpoint that answered with no version this wallet knows is the
    /// version line — the one that sends a person to update.
    #[test]
    fn an_empty_version_list_is_the_version_line() {
        let Err(verdict) = choose(
            "https://sign.getvela.app/",
            &Index::Listed(Vec::new()),
            &[],
            &[],
        ) else {
            unreachable!("an empty index offers nothing")
        };
        assert_eq!(
            verdict,
            Verdict::NoVersionToAsk(NoVersion::NothingPublishedThisWalletKnows)
        );
        assert!(describe(&verdict).contains("publishes no version this wallet knows"));
        assert_eq!(listed(r#"{"versions":[]}"#), Vec::<String>::new());
    }

    #[test]
    fn a_trailing_slash_is_added_once_and_only_once() {
        assert_eq!(with_trailing_slash("https://a.test"), "https://a.test/");
        assert_eq!(with_trailing_slash("https://a.test/"), "https://a.test/");
    }
}
