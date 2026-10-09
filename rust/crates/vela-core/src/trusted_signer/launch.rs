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
//! Vela's published build list · checked …", never "certified untampered" —
//! this catches a replaced build served to everyone, not a server that serves
//! one victim different bytes (spec 076, threat model).

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
        trusted,
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
        now_ms.saturating_sub(self.checked_at_ms) <= MAX_CHECK_AGE_MS
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
    /// [`Target::url`] — exactly what was checked.
    ///
    /// # Errors
    ///
    /// [`LaunchRefused::Stale`] when the check no longer vouches for the page.
    pub fn url_launch(
        &self,
        request: &Value,
        callback: &str,
        token: &str,
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
        ))
    }

    /// The page, told to connect to the wallet's loopback WebSocket (spec 071)
    /// — the same checked address.
    ///
    /// # Errors
    ///
    /// [`LaunchRefused::Stale`], as [`Self::url_launch`].
    pub fn ws_launch(&self, port: u16, token: &str, now_ms: u64) -> Result<String, LaunchRefused> {
        if !self.is_fresh(now_ms) {
            return Err(LaunchRefused::Stale);
        }
        Ok(super::socket_launch(self.target.url(), port, token))
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
