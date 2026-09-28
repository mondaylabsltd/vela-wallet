//! What a page load that went wrong IS, and what to do about it (spec 079).
//!
//! The in-app browsers of Android, iOS and the desktop each decided these
//! alone, and each decided differently: Android recorded the engine's error
//! page as a visit and never said why a load failed, iOS printed the
//! system's own sentence and an error code, the desktop showed nothing at
//! all. None of them retried. These are RULES with one right answer, so they
//! live here, and every shell draws the same panel with the same words:
//!
//! - [`classify`] — the platform's raw error (Android `WebViewClient.ERROR_*`,
//!   Apple `NSURLErrorDomain`, or the desktop's own probe) → one class, the
//!   corpus key of its sentence, and whether retrying can help. A cancelled
//!   navigation is not a failure.
//! - [`retry_delay_ms`] — the short, capped schedule a failed page retries on
//!   while it is in front. No platform here may listen for connectivity
//!   (Android's manifest refuses ACCESS_NETWORK_STATE, spec 047), so this is
//!   the honest substitute.
//! - [`visit_to_record`] — which finished loads count as a visit in Recents:
//!   never a failed one, never an error status, and always the address,
//!   title and icon of ONE document (Android recorded bscscan.com under
//!   Uniswap's title and icon).
//! - [`site_letter`] — the avatar letter, skipping the `app.` / `www.` / `m.`
//!   that made app.uniswap.org an "A".
//! - [`LoadWatch`] — the desktop's own account of a load (spec 082, RD4):
//!   asked, probed, failed, retried. Moved here from the desktop shell so the
//!   rules have one home and one set of tests; the desktop uses it through
//!   the crate (no FFI export), the phones share its constants.

use serde::{Deserialize, Serialize};

/// Where an error code comes from: each platform numbers its errors itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadPlatform {
    /// `android.webkit.WebViewClient.ERROR_*` (negative integers).
    Android,
    /// `NSError` from WKWebView — `domain` says which table the code is in.
    Apple,
    /// The desktop's own reachability probe (see [`probe_code`]); wry reports
    /// no load failures of its own.
    Probe,
}

/// What went wrong, as a person would tell it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadFailureClass {
    /// The connection itself failed or dropped: the network is the problem.
    Offline,
    /// The site did not answer in time.
    Timeout,
    /// No such site: the name did not resolve, or the address is malformed.
    NotFound,
    /// The site refused the connection.
    Refused,
    /// The certificate is wrong. Never retried, never "continue anyway".
    Certificate,
    /// Anything else.
    Other,
}

/// A classified main-frame failure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadFailure {
    pub class: LoadFailureClass,
    /// The corpus key of the panel's reason line ([`reason_key`]).
    pub reason_key: String,
    /// Whether [`retry_delay_ms`] schedules anything for this class.
    pub auto_retry: bool,
}

/// The desktop probe's codes: a HEAD request the shell makes itself when a
/// navigation does not commit (spec 079 research R4).
pub mod probe_code {
    pub const DNS: i64 = 1;
    pub const REFUSED: i64 = 2;
    pub const TIMEOUT: i64 = 3;
    pub const TLS: i64 = 4;
    pub const CONNECT: i64 = 5;
}

const NS_URL_ERROR_DOMAIN: &str = "NSURLErrorDomain";
const WEBKIT_ERROR_DOMAIN: &str = "WebKitErrorDomain";

/// The failure a platform error stands for, or `None` when it is not a
/// failure at all (a navigation the page or the person cancelled, a frame
/// load a newer navigation interrupted). `certificate` is set by a shell that
/// learned of the failure from its certificate callback rather than an error
/// code (Android `onReceivedSslError`).
pub fn classify(
    platform: LoadPlatform,
    code: i64,
    domain: Option<&str>,
    certificate: bool,
) -> Option<LoadFailure> {
    if certificate {
        return Some(failure(LoadFailureClass::Certificate));
    }
    let class = match platform {
        LoadPlatform::Android => match code {
            // ERROR_HOST_LOOKUP, ERROR_BAD_URL, ERROR_UNSUPPORTED_SCHEME
            -2 | -12 | -10 => LoadFailureClass::NotFound,
            // ERROR_CONNECT
            -6 => LoadFailureClass::Refused,
            // ERROR_TIMEOUT
            -8 => LoadFailureClass::Timeout,
            // ERROR_IO, and ERROR_UNKNOWN — which is how a connection that
            // closed without an answer (`net::ERR_EMPTY_RESPONSE`) arrives.
            -7 | -1 => LoadFailureClass::Offline,
            // ERROR_FAILED_SSL_HANDSHAKE
            -11 => LoadFailureClass::Certificate,
            _ => LoadFailureClass::Other,
        },
        LoadPlatform::Apple => match domain {
            Some(WEBKIT_ERROR_DOMAIN) => match code {
                // Frame load interrupted by a newer navigation; plug-in
                // handled load. Neither is the page failing.
                102 | 204 => return None,
                _ => LoadFailureClass::Other,
            },
            Some(NS_URL_ERROR_DOMAIN) | None => match code {
                // NSURLErrorCancelled — a navigation replaced by another.
                -999 => return None,
                // bad URL, cannot find host, DNS lookup failed, unsupported URL
                -1000 | -1003 | -1006 | -1002 => LoadFailureClass::NotFound,
                -1001 => LoadFailureClass::Timeout,
                -1004 => LoadFailureClass::Refused,
                // not connected, connection lost, roaming off, data not
                // allowed, cannot load from network
                -1009 | -1005 | -1018 | -1020 | -2000 => LoadFailureClass::Offline,
                // secure connection failed, certificate untrusted / expired /
                // not yet valid / unknown root / rejected / required
                -1206..=-1200 => LoadFailureClass::Certificate,
                _ => LoadFailureClass::Other,
            },
            Some(_) => LoadFailureClass::Other,
        },
        LoadPlatform::Probe => match code {
            probe_code::DNS => LoadFailureClass::NotFound,
            probe_code::REFUSED => LoadFailureClass::Refused,
            probe_code::TIMEOUT => LoadFailureClass::Timeout,
            probe_code::TLS => LoadFailureClass::Certificate,
            probe_code::CONNECT => LoadFailureClass::Offline,
            _ => LoadFailureClass::Other,
        },
    };
    Some(failure(class))
}

fn failure(class: LoadFailureClass) -> LoadFailure {
    LoadFailure {
        class,
        reason_key: reason_key(class).to_owned(),
        auto_retry: retry_delay_ms(class, 1).is_some(),
    }
}

/// The corpus key of each class's sentence. Offline, timeout and refused
/// share one: to a person all three are "the network — retrying", and the
/// class still drives the retry schedule. The generic line is the one the
/// panel always had (`connect.browser.loadFailed`).
pub fn reason_key(class: LoadFailureClass) -> &'static str {
    match class {
        LoadFailureClass::Offline | LoadFailureClass::Timeout | LoadFailureClass::Refused => {
            "explore.loadOffline"
        }
        LoadFailureClass::NotFound => "explore.loadNotFound",
        LoadFailureClass::Certificate => "explore.loadCertificate",
        LoadFailureClass::Other => "connect.browser.loadFailed",
    }
}

/// The wait before automatic attempt `attempt` (1-based), or `None` to stop.
///
/// A network that is coming back usually does within seconds, so three
/// quick attempts cover it; a name that does not resolve is a typo, and a
/// certificate that is wrong is a warning — neither heals by asking again.
pub fn retry_delay_ms(class: LoadFailureClass, attempt: u32) -> Option<u32> {
    match class {
        LoadFailureClass::Offline | LoadFailureClass::Timeout | LoadFailureClass::Refused => {
            match attempt {
                1 => Some(2_000),
                2 => Some(5_000),
                3 => Some(10_000),
                _ => None,
            }
        }
        LoadFailureClass::Other => (attempt == 1).then_some(3_000),
        LoadFailureClass::NotFound | LoadFailureClass::Certificate => None,
    }
}

/// What a shell observed when a load finished, read from the page itself in
/// ONE script (`{href, title, icon}`) so the three cannot come from two
/// documents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadFinished {
    /// The document's own `location.href`, not the view's current URL.
    pub url: String,
    pub title: String,
    pub icon: Option<String>,
    /// The main frame failed during this load (an engine error page is what
    /// finished, not the site).
    pub main_frame_failed: bool,
    /// The main document's HTTP status, where the platform reports it.
    pub http_status: Option<u16>,
}

/// A visit for Recents (`browser_history`'s `VisitRecorded`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    pub url: String,
    pub title: Option<String>,
    pub favicon: Option<String>,
}

/// The visit a finished load is, or `None` when it is not one: a failed
/// load, an error status (4xx/5xx), or anything that is not an http(s) page
/// (the engines' error documents live at `chrome-error://`, `about:`,
/// `data:`).
pub fn visit_to_record(load: LoadFinished) -> Option<Visit> {
    if load.main_frame_failed || load.http_status.is_some_and(|status| status >= 400) {
        return None;
    }
    if !is_web_url(&load.url) {
        return None;
    }
    let title = load.title.trim();
    Some(Visit {
        url: load.url,
        title: (!title.is_empty()).then(|| title.to_owned()),
        favicon: load.icon.filter(|icon| is_web_url(icon)),
    })
}

fn is_web_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

/// The letter a site's avatar shows when it has no icon: the first ASCII
/// letter or digit of the host once a leading `www.`, `app.` or `m.` is
/// dropped (only when a dotted name remains, so `app.io` stays "A"),
/// upper-cased; `?` when there is none.
pub fn site_letter(host: &str) -> String {
    let host = host.trim().to_ascii_lowercase();
    let name = ["www.", "app.", "m."]
        .iter()
        .find_map(|prefix| host.strip_prefix(prefix).filter(|rest| rest.contains('.')))
        .unwrap_or(&host);
    name.chars()
        .find(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned())
}

// ---------------------------------------------------------------------------
// The load watch (spec 079 US3 / R4; moved into the core by spec 082, RD4)
// ---------------------------------------------------------------------------
//
// wry 0.56 reports a load only when it COMMITS (`Started`) and when it
// finishes, and implements no `didFail*`: a navigation that fails is silent,
// and WKWebView keeps drawing the page it was on. So the desktop showed
// nothing at all on a bad network — no progress after Go, no failure, no
// reason, no retry (the 079 audit's F3–F5, "worse" than both phones).
//
// The watch is the shell's own account of a load, decided here with no UI in
// it so the tests can drive every turn:
//
// - **Asked** — `navigate`, reload, Retry: loading from that instant, not from
//   the engine's commit.
// - **No commit in 3 s** — probe the address natively (HEAD, 5 s) and feed its
//   error through [`classify`] with the probe's codes. A probe that fails is
//   the failure panel, with its reason.
// - **A probe that answers** — the site is up and slow: keep waiting, and call
//   it a timeout at 20 s.
// - **A commit at any time** clears everything: the page is there.
// - **Retry** — automatic for the network classes, on [`retry_delay_ms`]'s
//   schedule, only while the page is in front; the panel stays up, saying it
//   is trying again, until a commit takes its place. A wrong name or a bad
//   certificate is never retried by itself.
//
// Every timer and every probe carries the load's number; an answer for an
// older load is dropped, which is how a new address wins over a retry of the
// old one.

/// How long a load may go without committing before the desktop probes the
/// address.
pub const WATCHDOG_MS: u32 = 3_000;
/// The desktop probe's own budget.
pub const PROBE_BUDGET_MS: u32 = 5_000;
/// A load that has not committed by now is given up on, on every client —
/// unless the engine shows it is getting somewhere ([`ENGINE_LIVE_PROGRESS`]).
pub const GIVE_UP_MS: u32 = 20_000;
/// Engine progress above this means the site is answering. WebKit starts a
/// provisional load at 0.1, so anything past 0.15 is the page arriving.
pub const ENGINE_LIVE_PROGRESS: f64 = 0.15;

/// Who asked for a load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// An address, a link, a tab, a reload: a new load.
    Navigation,
    /// The panel's Retry: the count starts over, the panel stays.
    Retry,
    /// The schedule's own attempt: the panel stays, the count goes on.
    AutoRetry,
}

/// The shell's account of the page's current load.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LoadWatch {
    /// The address asked for last — what a retry loads again.
    pub url: Option<String>,
    /// The load's number: every timer and probe carries it.
    pub generation: u64,
    pub requested_at_ms: f64,
    /// Asked for, not committed yet.
    pub loading: bool,
    /// Committed, not finished: the hairline stays until the page is done.
    pub committed: bool,
    pub probing: bool,
    /// The failure the panel shows. Stays through a retry.
    pub failure: Option<LoadFailure>,
    /// A retry is running under the panel.
    pub retrying: bool,
    /// Automatic attempts since the last commit or the last Retry.
    pub attempt: u32,
    /// An automatic attempt came due while the page was not in front.
    pub retry_due: bool,
    /// How the next reported request was asked for (set by the page just
    /// before it tells the engine to load).
    pub next_asked: Option<Asked>,
}

/// What a probe's answer means for the load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Probed {
    /// For another load, or one already committed: nothing.
    Ignored,
    /// The site answered: wait for the engine until this instant (ms, the
    /// clock `requested` was given).
    WaitUntil(f64),
    /// The failure panel, with its reason.
    Failed,
}

impl LoadWatch {
    /// The engine was asked to load `url`. Returns the load's number when a
    /// watchdog should run — never for `about:blank` and the like, which are
    /// the wallet's own pages and cannot fail on a network.
    pub fn requested(&mut self, url: &str, now_ms: f64) -> Option<u64> {
        let asked = self.next_asked.take().unwrap_or(Asked::Navigation);
        self.generation += 1;
        self.probing = false;
        self.committed = false;
        self.retry_due = false;
        if !is_web_url(url) {
            self.url = None;
            self.loading = false;
            self.failure = None;
            self.retrying = false;
            self.attempt = 0;
            return None;
        }
        self.url = Some(url.to_owned());
        self.requested_at_ms = now_ms;
        self.loading = true;
        match asked {
            Asked::Navigation => {
                self.failure = None;
                self.retrying = false;
                self.attempt = 0;
            }
            Asked::Retry => {
                self.retrying = self.failure.is_some();
                self.attempt = 0;
            }
            Asked::AutoRetry => self.retrying = self.failure.is_some(),
        }
        Some(self.generation)
    }

    /// A document committed at `url`: the page is there, whatever was asked —
    /// unless it is not a web page while a web page is being waited on.
    /// WKWebView commits an empty `about:blank` when the first load of a
    /// fresh view is refused (seen on the Mac, spec 079): that is the engine
    /// giving up, not the site arriving.
    pub fn committed(&mut self, url: &str) {
        if self.ignores(url) {
            return;
        }
        self.loading = false;
        self.committed = true;
        self.probing = false;
        self.failure = None;
        self.retrying = false;
        self.attempt = 0;
        self.retry_due = false;
    }

    /// The load of `url` finished — the engine's blank page finishing is not
    /// the site's load finishing (see [`Self::committed`]).
    pub fn finished(&mut self, url: &str) {
        if self.ignores(url) {
            return;
        }
        self.committed = false;
        self.loading = false;
        self.probing = false;
    }

    /// A document that is not a web page while a web page is being waited
    /// on (or has failed): the engine giving up, not the site.
    fn ignores(&self, url: &str) -> bool {
        !is_web_url(url) && self.url.is_some() && (self.loading || self.failure.is_some())
    }

    /// [`WATCHDOG_MS`] after load `generation` was asked for: the address to
    /// probe, if it still has not committed.
    pub fn watchdog(&mut self, generation: u64) -> Option<String> {
        if generation != self.generation || !self.loading || self.probing {
            return None;
        }
        self.probing = true;
        self.url.clone()
    }

    /// The probe of load `generation` answered: `Ok` — the site is reachable;
    /// `Err(code)` — one of [`probe_code`]'s, or anything else for "other".
    pub fn probed(&mut self, generation: u64, answer: Result<(), i64>) -> Probed {
        if generation != self.generation || !self.loading {
            return Probed::Ignored;
        }
        self.probing = false;
        match answer {
            Ok(()) => Probed::WaitUntil(self.requested_at_ms + f64::from(GIVE_UP_MS)),
            Err(code) => {
                self.fail(classify(LoadPlatform::Probe, code, None, false));
                Probed::Failed
            }
        }
    }

    /// [`GIVE_UP_MS`] after load `generation` was asked for, with a site that
    /// answered the probe: still nothing committed is a timeout.
    pub fn give_up(&mut self, generation: u64) -> bool {
        if generation != self.generation || !self.loading {
            return false;
        }
        self.fail(classify(
            LoadPlatform::Probe,
            probe_code::TIMEOUT,
            None,
            false,
        ));
        true
    }

    fn fail(&mut self, failure: Option<LoadFailure>) {
        self.loading = false;
        self.probing = false;
        self.retrying = false;
        // `None` would be "not a failure" — never the probe's case, but the
        // classifier is the one to say so.
        if let Some(failure) = failure {
            self.failure = Some(failure);
        }
    }

    /// After a failure: the wait (ms) before the next automatic attempt, with
    /// the load it belongs to — `None` when the class is never retried by
    /// itself or the schedule has run out. Counts the attempt.
    pub fn schedule_retry(&mut self) -> Option<(u64, u32)> {
        let failure = self.failure.as_ref()?;
        if !failure.auto_retry || self.loading {
            return None;
        }
        let wait = retry_delay_ms(failure.class, self.attempt + 1)?;
        self.attempt += 1;
        Some((self.generation, wait))
    }

    /// An automatic attempt for load `generation` came due. The address to
    /// load again when the page is in front; otherwise it waits for the page
    /// to come back ([`Self::take_due`]).
    pub fn retry_fired(&mut self, generation: u64, in_front: bool) -> Option<String> {
        if generation != self.generation || self.failure.is_none() || self.loading {
            return None;
        }
        if !in_front {
            self.retry_due = true;
            return None;
        }
        self.next_asked = Some(Asked::AutoRetry);
        self.url.clone()
    }

    /// The page is in front again: an attempt that came due while it was not.
    pub fn take_due(&mut self) -> Option<String> {
        if !std::mem::take(&mut self.retry_due) || self.failure.is_none() || self.loading {
            return None;
        }
        self.next_asked = Some(Asked::AutoRetry);
        self.url.clone()
    }

    /// The panel's Retry: the address to load again.
    pub fn retry(&mut self) -> Option<String> {
        if self.loading {
            return None;
        }
        self.next_asked = Some(Asked::Retry);
        self.url.clone()
    }

    /// The hairline: from the moment a load is asked for until it finishes or
    /// fails.
    #[must_use]
    pub fn busy(&self) -> bool {
        self.loading || self.committed
    }
}

/// The host of an address as typed, for the panel's small line: userinfo
/// dropped, the port kept, nothing normalised.
#[must_use]
pub fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .rsplit('@')
        .next()
        .unwrap_or_default()
        .to_owned()
}
