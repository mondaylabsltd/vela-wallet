//! Which page is opened — and that it is the page that was checked (spec 102,
//! R6).
//!
//! Spec 076 decided whether a page's bytes may be trusted
//! ([`super::integrity::decide`]). What it left to each shell was the two
//! questions around that decision, and the shells answered them apart:
//!
//! - **which URL is fetched to be checked** — the desktop fetched
//!   `<base>/b/<chosen>/sign.html` once at launch;
//! - **which URL is opened** — its signatures opened the core's fixed
//!   `LAUNCH` path instead, its ceremonies the remembered version, and a custom
//!   page the unhashed root. They matched only because `LAUNCH` happened to
//!   equal `BUILD_ALLOWED[0]`. The phones checked nothing at all.
//!
//! Here both are one value. [`target`] picks the version — and with it the
//! one URL that is fetched AND opened. [`admit`] rules on the bytes that URL
//! served, and only an admitted [`CheckedPage`] can build a launch URL: there
//! is no other function in this crate that writes one. So a shell cannot open
//! a page it did not check, nor a version other than the one it checked, and
//! a check that failed — or never ran — opens nothing (FR-006).
//!
//! The words a person reads are the verdict's ([`IntegrityLine`]): "matches
//! Vela's published build list · checked 14:32", never "certified
//! untampered" — this catches a replaced build served to everyone, not a
//! server that serves one victim different bytes (spec 076, threat model).
//! The time is [`checked_time`]'s, the same in every shell.
//!
//! **The fetch is a browser's.** The shell fetches [`Target::url`] with
//! [`CHECK_HEADERS`] — what a browser sends when it navigates to the page —
//! because a host may serve a navigation other bytes than a plain request:
//! Cloudflare's Web Analytics injected its beacon into `text/html` answers
//! only, so a check that asked for `*/*` hashed the clean page while the
//! browser opened the rewritten one (found on Android, spec 102; the page's
//! `_headers` now say `Cache-Control: no-transform`). Asking as a browser
//! asks is what makes "the bytes that were checked" the bytes that open.
//!
//! **Freshness.** A check vouches for [`MAX_CHECK_AGE_MS`]; a launch past it
//! is refused, and the line reads "checking" while the check runs again
//! ([`line_while_checking`]). So that rarely happens at Open, a shell checks
//! every page in use in the background ([`refresh_due`]): on start and on
//! return to the foreground, and every [`REFRESH_POLL_MS`] while it runs, each
//! page whose check is older than [`REFRESH_AFTER_MS`] (or missing) — spaced
//! [`RETRY_AFTER_MS`] after an attempt that could not complete. A refresh that
//! could not complete keeps the check before it while that one is fresh
//! ([`keep_or_replace`]); one that completed — a mismatch included — replaces
//! it.
//!
//! **Trust is a self-hoster's.** The `trusted` versions a shell passes are the
//! ones the person trusted for THIS page ([`crate::signing_venue::
//! trusted_versions`]). For the official page they are ignored: nothing is
//! ever asked about it, and nothing a person was talked into trusting may open
//! in its name.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::integrity::{self, CheckFailure, NoVersion, Page, Verdict};

/// How long a check vouches for a page. Past this, the page is checked again
/// before it is opened: a build swapped after the check would otherwise be
/// opened under a stale "checked" line.
///
/// A day, so a person who signs several times a day pays for one fetch, and
/// "checked" never means "some time last week".
pub const MAX_CHECK_AGE_MS: u64 = 24 * 60 * 60 * 1000;

/// A check older than this is refreshed in the background ([`refresh_due`]) —
/// half of [`MAX_CHECK_AGE_MS`], so a page in use is re-checked well before
/// its check stops vouching, and a person pressing Open almost never waits on
/// a fetch.
pub const REFRESH_AFTER_MS: u64 = MAX_CHECK_AGE_MS / 2;

/// How often a running app asks [`refresh_due`] about the pages in use (on
/// top of start and every return to the foreground). An hour: the question is
/// a comparison of two numbers, and the answer changes twice a day.
pub const REFRESH_POLL_MS: u64 = 60 * 60 * 1000;

/// After a background check that could not complete (no network), the next
/// attempt waits this long — so a phone offline for an afternoon does not
/// fetch on every poll, and one back online is checked within minutes.
pub const RETRY_AFTER_MS: u64 = 10 * 60 * 1000;

/// What a check's fetch of [`Target::url`] sends — what a browser sends when
/// it NAVIGATES to the page, so the host answers the check with the bytes it
/// answers the launch with (see the module doc: a host may rewrite HTML for a
/// navigation only). Every shell that fetches sends exactly these, and
/// follows no redirect it would not follow for the launch.
pub const CHECK_HEADERS: &[(&str, &str)] = &[(
    "Accept",
    "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
)];

/// Is a background check of a page due at `now_ms`? `checked_at_ms` is when
/// its last COMPLETED check ran (`None`: never, or its verdict was not an
/// admission), `last_attempt_ms` when a check last started — completed or
/// not — so attempts that cannot reach the page are spaced
/// [`RETRY_AFTER_MS`] apart.
///
/// Due when there is no check, or it is older than [`REFRESH_AFTER_MS`], and
/// no attempt started within [`RETRY_AFTER_MS`].
#[must_use]
pub fn refresh_due(checked_at_ms: Option<u64>, last_attempt_ms: Option<u64>, now_ms: u64) -> bool {
    let stale = checked_at_ms.is_none_or(|at| now_ms.saturating_sub(at) > REFRESH_AFTER_MS);
    let resting = last_attempt_ms.is_some_and(|at| now_ms.saturating_sub(at) < RETRY_AFTER_MS);
    stale && !resting
}

/// Does a check made at `checked_at_ms` still vouch for a page at `now_ms`?
#[must_use]
pub fn is_fresh_at(checked_at_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(checked_at_ms) <= MAX_CHECK_AGE_MS
}

/// `{{time}}` in "… · checked {{time}}" — when the check ran, as the person
/// reads times: the clock time in their format ("14:32", "2:32 PM") when the
/// check ran today, else the date and the time ("10/08/2026, 14:32") — a
/// check vouches for a day, so "14:32" alone could be yesterday's.
///
/// A moment rather than "2 min ago", because every line it fills says
/// "checked AT" in its language (「检查于 {{time}}」, 「{{time}} に確認」,
/// "{{time}} kontrol edildi"): a moment reads right in all fifteen, needs no
/// new words, and does not go stale on a card left open — the relative words
/// the corpus has ("2m", 「2分钟前」) are an activity list's, and read wrongly
/// there. `utc_offset_minutes` is the host's offset now (the core has no
/// timezone database); `date_format` / `time_format` are the person's
/// presets as stored (`ymd_slash`… / `h24`, `h12`), `auto` already resolved;
/// `language` names the day period of a 12-hour clock.
///
/// **One unbreakable unit.** Every space inside it is a no-break space
/// (U+00A0): a moment split over two lines reads as two things — a phone
/// drew 「检查于 下午」 on one line and 「10:07」 on the next. The line it
/// fills may still wrap before or after it.
#[must_use]
pub fn checked_time(
    checked_at_ms: u64,
    now_ms: u64,
    utc_offset_minutes: i32,
    date_format: &str,
    time_format: &str,
    language: &str,
) -> String {
    use crate::l10n::datetime::{
        date_preset_of, format_date_time, format_time, time_preset_of, Civil,
    };
    let millis = |ms: u64| i64::try_from(ms).unwrap_or(i64::MAX);
    let at = Civil::from_unix_millis(millis(checked_at_ms), utc_offset_minutes);
    let now = Civil::from_unix_millis(millis(now_ms), utc_offset_minutes);
    let clock = time_preset_of(time_format);
    let moment = if (at.year, at.month, at.day) == (now.year, now.month, now.day) {
        format_time(&at, clock, language)
    } else {
        format_date_time(&at, date_preset_of(date_format), clock, language)
    };
    moment.replace(' ', "\u{a0}")
}

/// The page a wallet will fetch, check and open: one version of one
/// deployment, and the one URL that serves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    base: String,
    version: String,
    url: String,
    proposed: bool,
}

impl Target {
    /// The deployment, as Settings or the account names it.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }

    /// The version — the sha256 of the page's bytes, lowercase hex.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The first eight characters of [`Self::version`], as a person reads it.
    #[must_use]
    pub fn short_version(&self) -> &str {
        self.version.get(..8).unwrap_or(&self.version)
    }

    /// The URL that is fetched to be checked, and the URL that is opened.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// A version this wallet does not know, that a custom page's own index
    /// named. It can only ever end in [`Verdict::AskToTrust`] — the person
    /// decides — never in a page that opens on the index's word.
    #[must_use]
    pub fn proposed_by_index(&self) -> bool {
        self.proposed
    }

    fn new(base: &str, version: String, proposed: bool) -> Self {
        let url = page_url(base, &version);
        Self {
            base: base.to_owned(),
            version,
            url,
            proposed,
        }
    }
}

/// The content-addressed page of `version` at `base`.
///
/// The official host serves `/b/<sha256>/sign` itself (Cloudflare Pages
/// answers `sign.html` with a redirect to it, which a phone opening the page
/// offline could not follow — spec 079). Anybody else's deployment of `dist/`
/// is a plain static host: the file is `sign.html`.
fn page_url(base: &str, version: &str) -> String {
    let root = base.trim().trim_end_matches('/');
    if integrity::is_official(base) {
        format!("{root}/b/{version}/sign")
    } else {
        format!("{root}/b/{version}/sign.html")
    }
}

/// R6 — which version of the page at `base` to open, and therefore to check.
///
/// `index` is what the deployment's `index.json` lists, or `None` when it
/// could not be read. The index narrows and never chooses
/// ([`integrity::choose_version`]): the candidates are this build's
/// [`integrity::BUILD_ALLOWED`] and then the person's own trusted hashes, in
/// that order, and the first one the index lists wins. With no index the
/// version is asked for directly — the official host's
/// [`integrity::LAUNCH`] (the newest version known to be deployed), else the
/// first candidate — because the index is an optimisation, not a dependency.
///
/// One exception, and only for a page that is not the official one: when the
/// index lists nothing this wallet knows, its first unblocked entry is the
/// target, marked [`Target::proposed_by_index`]. Checking it can only end in
/// asking the person ([`Verdict::AskToTrust`]), which is how a self-hoster's
/// own build is trusted at all.
///
/// # Errors
///
/// Why there is no version to ask for: none this wallet knows is published,
/// or every one that would do is blocked on this device.
pub fn target(
    base: &str,
    index: Option<&[String]>,
    trusted: &[String],
    blocked: &[String],
) -> Result<Target, NoVersion> {
    let base = super::signer_url(base).unwrap_or_else(|_| base.trim().to_owned());
    let official = integrity::is_official(&base);
    let trusted = trusted_for(&base, trusted);
    let unblocked = |hash: &String| !contains(blocked, hash);
    let candidates: Vec<String> = integrity::BUILD_ALLOWED
        .iter()
        .map(|hash| (*hash).to_owned())
        .chain(trusted.iter().cloned())
        .filter_map(|hash| integrity::normalize_hash(&hash))
        .collect();
    let Some(listed) = index else {
        let launch = integrity::normalize_hash(integrity::LAUNCH).filter(|_| official);
        return launch
            .into_iter()
            .chain(candidates.iter().cloned())
            .find(unblocked)
            .map(|version| Target::new(&base, version, false))
            .ok_or(if candidates.is_empty() {
                NoVersion::NothingPublishedThisWalletKnows
            } else {
                NoVersion::EverythingUsableIsBlocked
            });
    };
    match integrity::choose_version(listed, trusted, blocked) {
        Ok(version) => Ok(Target::new(&base, version, false)),
        Err(why) if official => Err(why),
        Err(why) => listed
            .iter()
            .filter_map(|hash| integrity::normalize_hash(hash))
            .find(unblocked)
            .map(|version| Target::new(&base, version, true))
            .ok_or(why),
    }
}

/// The person's trusted versions as they apply to `base`: none for the
/// official page (nothing is ever asked about it, so nothing a person was
/// talked into trusting may open in its name), the list itself otherwise.
fn trusted_for<'a>(base: &str, trusted: &'a [String]) -> &'a [String] {
    if integrity::is_official(base) {
        &[]
    } else {
        trusted
    }
}

/// `true` when `list` holds `hash`, compared in normal form.
fn contains(list: &[String], hash: &str) -> bool {
    list.iter()
        .filter_map(|entry| integrity::normalize_hash(entry))
        .any(|entry| entry == hash)
}

/// What [`admit`] decided.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admission {
    /// The page may be opened — through this, and only this.
    Admitted(CheckedPage),
    /// The page may not be opened. The verdict is what the hand-off card
    /// draws (its [`IntegrityLine`]).
    Refused { target: Target, verdict: Verdict },
}

impl Admission {
    /// The line the hand-off card and Settings draw for this check.
    #[must_use]
    pub fn line(&self) -> IntegrityLine {
        match self {
            Self::Admitted(page) => page.line(page.checked_at_ms),
            Self::Refused { target, verdict } => IntegrityLine::of(verdict, target.version(), None),
        }
    }

    /// The page, when it may be opened.
    #[must_use]
    pub fn page(&self) -> Option<&CheckedPage> {
        match self {
            Self::Admitted(page) => Some(page),
            Self::Refused { .. } => None,
        }
    }

    /// Does this check admit the page AND still vouch for it at `now_ms`? A
    /// launch at `now_ms` succeeds exactly when this is `true`.
    #[must_use]
    pub fn is_fresh(&self, now_ms: u64) -> bool {
        self.page().is_some_and(|page| page.is_fresh(now_ms))
    }

    /// The full version to store when the person answers "Trust this
    /// version" (`SigningPagesEvent::VersionTrusted`) — `Some` only when the
    /// check ended in [`Verdict::AskToTrust`], which only a self-hosted page
    /// can.
    #[must_use]
    pub fn version_to_trust(&self) -> Option<&str> {
        match self {
            Self::Refused {
                verdict: Verdict::AskToTrust { actual },
                ..
            } => Some(actual),
            _ => None,
        }
    }

    /// Is a background refresh of this page due ([`refresh_due`])?
    /// `last_attempt_ms` as there.
    #[must_use]
    pub fn refresh_due(&self, last_attempt_ms: Option<u64>, now_ms: u64) -> bool {
        refresh_due(
            self.page().map(CheckedPage::checked_at_ms),
            last_attempt_ms,
            now_ms,
        )
    }
}

/// Which check to keep after a background refresh: `next`, unless it could
/// not complete ([`Verdict::CouldNotCheck`] — no network, a timeout) and
/// `previous` still admits the page at `now_ms`; then `previous`, which
/// [`refresh_due`] will ask about again after [`RETRY_AFTER_MS`].
///
/// A refresh that COMPLETED always wins, whatever it says: bytes that no
/// longer match are the news the refresh exists to catch, and a page that now
/// matches replaces an older refusal.
#[must_use]
pub fn keep_or_replace(previous: Option<Admission>, next: Admission, now_ms: u64) -> Admission {
    match previous {
        Some(previous) if keeps_previous(Some(&previous), &next, now_ms) => previous,
        _ => next,
    }
}

/// [`keep_or_replace`]'s question alone: does `previous` stay?
#[must_use]
pub fn keeps_previous(previous: Option<&Admission>, next: &Admission, now_ms: u64) -> bool {
    let incomplete = matches!(
        next,
        Admission::Refused {
            verdict: Verdict::CouldNotCheck(_),
            ..
        }
    );
    incomplete && previous.is_some_and(|previous| previous.is_fresh(now_ms))
}

/// The line to draw while a check of the page runs: the previous check's
/// verdict while it still vouches for the page (a background refresh does not
/// make a good line flicker to "checking"), else "checking" — the state a
/// shell draws when a stale check re-runs at Open, the Open button disabled
/// until it completes.
#[must_use]
pub fn line_while_checking(previous: Option<&Admission>, now_ms: u64) -> IntegrityLine {
    match previous.and_then(Admission::page) {
        Some(page) if page.is_fresh(now_ms) => page.line(now_ms),
        _ => IntegrityLine::checking(),
    }
}

/// R6 — rule on the bytes `target`'s URL served.
///
/// `observed` is the sha256 of exactly those bytes ([`integrity::hash_page`]),
/// or `None` when the fetch failed, with `failure` saying how.
/// `checked_at_ms` is the shell's clock when the bytes arrived.
///
/// On top of [`integrity::decide`]'s ordering, one rule only a target can
/// state: the bytes at `/b/<version>/` must BE that version. A deployment
/// whose content address serves other bytes — even bytes of another accepted
/// version — is broken or tampered with, and the version a person would be
/// told they are looking at is not the one they would get.
#[must_use]
pub fn admit(
    target: &Target,
    observed: Option<&str>,
    failure: CheckFailure,
    trusted: &[String],
    blocked: &[String],
    verification_off: bool,
    checked_at_ms: u64,
) -> Admission {
    let mut verdict = integrity::decide(&Page {
        url: target.base(),
        observed,
        failure,
        trusted: trusted_for(target.base(), trusted),
        blocked,
        verification_off,
    });
    let actual = observed.and_then(integrity::normalize_hash);
    if matches!(verdict, Verdict::Open | Verdict::AskToTrust { .. })
        && actual.as_deref() != Some(target.version())
    {
        verdict = Verdict::Refused {
            actual: actual.unwrap_or_default(),
            expected: vec![target.version().to_owned()],
        };
    }
    let opens = matches!(verdict, Verdict::Open | Verdict::OpenUnverified);
    // `ENFORCE` is the kill switch spec 076 kept apart from the allow-list:
    // were it off, a refused page would open anyway, its refusal still drawn.
    if opens || !integrity::ENFORCE {
        Admission::Admitted(CheckedPage {
            target: target.clone(),
            verdict,
            checked_at_ms,
        })
    } else {
        Admission::Refused {
            target: target.clone(),
            verdict,
        }
    }
}

/// A page that was checked and may be opened — the only thing in this crate
/// that can build a launch URL. Made by [`admit`] alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckedPage {
    target: Target,
    verdict: Verdict,
    checked_at_ms: u64,
}

/// Why a checked page was not opened after all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum LaunchRefused {
    /// The check is older than [`MAX_CHECK_AGE_MS`]: check again first.
    #[error("the signing page was checked too long ago — check it again")]
    Stale,
}

impl CheckedPage {
    #[must_use]
    pub fn target(&self) -> &Target {
        &self.target
    }

    #[must_use]
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    #[must_use]
    pub fn checked_at_ms(&self) -> u64 {
        self.checked_at_ms
    }

    /// Does the check still vouch for the page at `now_ms`?
    #[must_use]
    pub fn is_fresh(&self, now_ms: u64) -> bool {
        is_fresh_at(self.checked_at_ms, now_ms)
    }

    /// Is a background refresh of this page due ([`refresh_due`])?
    #[must_use]
    pub fn refresh_due(&self, last_attempt_ms: Option<u64>, now_ms: u64) -> bool {
        refresh_due(Some(self.checked_at_ms), last_attempt_ms, now_ms)
    }

    /// The line to draw at `now_ms`: the verdict, or "checking" once the check
    /// is too old to vouch for anything.
    #[must_use]
    pub fn line(&self, now_ms: u64) -> IntegrityLine {
        if self.is_fresh(now_ms) {
            IntegrityLine::of(
                &self.verdict,
                self.target.version(),
                Some(self.checked_at_ms),
            )
        } else {
            IntegrityLine::checking()
        }
    }

    /// The page, told to answer over `callback` (PROTOCOL.md §7.2): the request
    /// deflated into the URL's FRAGMENT, which no server sees. The address is
    /// [`Target::url`] — exactly what was checked — and the query names the
    /// app's language (`lang=`, the language the app shows: `zh-HK`, `pt-BR`),
    /// which the page resolves by the apps' own rule, so the page speaks the
    /// person's language and not the browser's. An empty or malformed `lang`
    /// is left out (the page then follows the browser).
    ///
    /// # Errors
    ///
    /// [`LaunchRefused::Stale`] when the check no longer vouches for the page.
    pub fn url_launch(
        &self,
        request: &Value,
        callback: &str,
        token: &str,
        lang: &str,
        now_ms: u64,
    ) -> Result<String, LaunchRefused> {
        if !self.is_fresh(now_ms) {
            return Err(LaunchRefused::Stale);
        }
        Ok(super::fragment_launch(
            self.target.url(),
            request,
            callback,
            token,
            lang,
        ))
    }

    /// The page, told to connect to the wallet's loopback WebSocket (spec 071)
    /// — the same checked address, in the app's language as for
    /// [`Self::url_launch`].
    ///
    /// # Errors
    ///
    /// [`LaunchRefused::Stale`], as [`Self::url_launch`].
    pub fn ws_launch(
        &self,
        port: u16,
        token: &str,
        lang: &str,
        now_ms: u64,
    ) -> Result<String, LaunchRefused> {
        if !self.is_fresh(now_ms) {
            return Err(LaunchRefused::Stale);
        }
        Ok(super::socket_launch(self.target.url(), port, token, lang))
    }
}

/// What the integrity line says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum IntegrityState {
    /// The check is running (or has to run again).
    Checking,
    /// A version this build ships — Vela's published build list.
    Matches,
    /// A version this person trusted on this device.
    TrustedHere,
    /// A custom page whose owner turned checking off: opens, and says so.
    Unchecked,
    /// Bytes that are not a version this device accepts. Not opened.
    Mismatch,
    /// A version blocked on this device. Not opened.
    Blocked,
    /// A custom page serving a version nobody decided about yet. Not opened
    /// until the person trusts it.
    AskToTrust,
    /// The page could not be fetched or read. Not opened.
    CouldNotCheck,
    /// The page publishes no version this wallet knows. Not opened.
    NoVersion,
    /// Every version that would do is blocked on this device. Not opened.
    AllBlocked,
}

impl IntegrityState {
    /// The corpus key of the line. `{{version}}` and `{{time}}` where the
    /// sentence has them.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Checking => "componentsUi.signing.integrity.checking",
            Self::Matches => "componentsUi.signing.integrity.matches",
            Self::TrustedHere => "componentsUi.signing.integrity.trusted",
            Self::Unchecked => "componentsUi.signing.integrity.unchecked",
            Self::Mismatch => "componentsUi.signing.integrity.mismatch",
            Self::Blocked => "componentsUi.signing.integrity.blocked",
            Self::AskToTrust => "componentsUi.signing.integrity.askTrust",
            Self::CouldNotCheck => "componentsUi.signing.integrity.couldNotCheck",
            Self::NoVersion => "componentsUi.signing.integrity.noVersion",
            Self::AllBlocked => "componentsUi.signing.integrity.allBlocked",
        }
    }

    /// May the page be opened in this state?
    #[must_use]
    pub const fn opens(self) -> bool {
        matches!(self, Self::Matches | Self::TrustedHere | Self::Unchecked)
    }
}

/// The one line the hand-off card (D4) and Settings → Signing pages draw
/// about a page: "Version 0ba8ee8c · matches Vela's published build list ·
/// checked 2 min ago", or why it will not be opened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct IntegrityLine {
    pub state: IntegrityState,
    /// The version's first eight hex characters — the served version for a
    /// refusal, else the checked one; empty when there is none.
    pub version: String,
    /// When the check ran (the shell's clock), for "checked …". `None` when
    /// the line is not about a completed check.
    pub checked_at_ms: Option<u64>,
    /// The corpus key, so no shell maps the state itself.
    pub key: String,
    /// May the page be opened? The Open button is enabled only when `true`.
    pub opens: bool,
}

impl IntegrityLine {
    /// The line for a check that is running, or must run again.
    #[must_use]
    pub fn checking() -> Self {
        Self::with(IntegrityState::Checking, "", None)
    }

    /// The line for a verdict about `version`.
    #[must_use]
    pub fn of(verdict: &Verdict, version: &str, checked_at_ms: Option<u64>) -> Self {
        let shipped = integrity::BUILD_ALLOWED
            .iter()
            .any(|hash| hash.eq_ignore_ascii_case(version));
        match verdict {
            Verdict::Open if shipped => Self::with(IntegrityState::Matches, version, checked_at_ms),
            Verdict::Open => Self::with(IntegrityState::TrustedHere, version, checked_at_ms),
            Verdict::OpenUnverified => Self::with(IntegrityState::Unchecked, "", checked_at_ms),
            Verdict::Denied { hash } => Self::with(IntegrityState::Blocked, hash, None),
            Verdict::Refused { actual, .. } => Self::with(IntegrityState::Mismatch, actual, None),
            Verdict::AskToTrust { actual } => Self::with(IntegrityState::AskToTrust, actual, None),
            Verdict::CouldNotCheck(_) => Self::with(IntegrityState::CouldNotCheck, "", None),
            Verdict::NoVersionToAsk(why) => Self::no_version(*why),
        }
    }

    /// The line when [`target`] found no version to ask for.
    #[must_use]
    pub fn no_version(why: NoVersion) -> Self {
        match why {
            NoVersion::NothingPublishedThisWalletKnows => {
                Self::with(IntegrityState::NoVersion, "", None)
            }
            NoVersion::EverythingUsableIsBlocked => {
                Self::with(IntegrityState::AllBlocked, "", None)
            }
        }
    }

    fn with(state: IntegrityState, version: &str, checked_at_ms: Option<u64>) -> Self {
        Self {
            state,
            version: version.get(..8).unwrap_or(version).to_owned(),
            checked_at_ms,
            key: state.key().to_owned(),
            opens: state.opens(),
        }
    }
}
