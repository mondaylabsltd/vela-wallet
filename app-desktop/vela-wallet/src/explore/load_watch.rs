//! Whether the page on screen is loading, failed, or trying again (spec 079
//! US3, research R4).
//!
//! wry 0.56 reports a load only when it COMMITS (`Started`) and when it
//! finishes, and implements no `didFail*`: a navigation that fails is silent,
//! and WKWebView keeps drawing the page it was on. So the desktop showed
//! nothing at all on a bad network — no progress after Go, no failure, no
//! reason, no retry (the audit's F3–F5, "worse" than both phones).
//!
//! The watch is the shell's own account of a load, decided here with no gpui in
//! it so the tests can drive every turn:
//!
//! - **Asked** — `navigate`, reload, Retry: loading from that instant, not from
//!   the engine's commit.
//! - **No commit in 3 s** — probe the address natively (HEAD, 5 s) and feed its
//!   error through the core's classifier (`browser_load::classify` with the
//!   probe's codes). A probe that fails is the failure panel, with its reason.
//! - **A probe that answers** — the site is up and slow: keep waiting, and call
//!   it a timeout at 20 s.
//! - **A commit at any time** clears everything: the page is there.
//! - **Retry** — automatic for the network classes, on the core's schedule,
//!   only while the page is in front; the panel stays up, saying it is trying
//!   again, until a commit takes its place. A wrong name or a bad certificate
//!   is never retried by itself.
//!
//! Every timer and every probe carries the load's number; an answer for an
//! older load is dropped, which is how a new address wins over a retry of the
//! old one.

use std::time::Duration;

use vela_core::app::browser_load::{self, LoadFailure, LoadPlatform, probe_code};

/// How long a load may go without committing before the address is probed.
pub const WATCHDOG: Duration = Duration::from_secs(3);
/// The probe's own budget.
pub const PROBE_BUDGET: Duration = Duration::from_secs(5);
/// A site that answers the probe but never commits is a timeout after this.
pub const GIVE_UP: Duration = Duration::from_secs(20);

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
    /// The site answered: wait for the engine until this instant.
    WaitUntil(f64),
    /// The failure panel, with its reason.
    Failed,
}

fn is_web(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
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
        if !is_web(url) {
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
    /// fresh view is refused (seen on this Mac, spec 079): that is the engine
    /// giving up, not the site arriving.
    pub fn committed(&mut self, url: &str) {
        if !is_web(url) && self.url.is_some() && (self.loading || self.failure.is_some()) {
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

    /// The load finished.
    pub fn finished(&mut self) {
        self.committed = false;
        self.loading = false;
        self.probing = false;
    }

    /// Three seconds after load `generation` was asked for: the address to
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
            Ok(()) => Probed::WaitUntil(self.requested_at_ms + GIVE_UP.as_secs_f64() * 1000.),
            Err(code) => {
                self.fail(browser_load::classify(
                    LoadPlatform::Probe,
                    code,
                    None,
                    false,
                ));
                Probed::Failed
            }
        }
    }

    /// Twenty seconds after load `generation` was asked for, with a site that
    /// answered the probe: still nothing committed is a timeout.
    pub fn give_up(&mut self, generation: u64) -> bool {
        if generation != self.generation || !self.loading {
            return false;
        }
        self.fail(browser_load::classify(
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

    /// After a failure: the wait before the next automatic attempt, with the
    /// load it belongs to — `None` when the class is never retried by itself
    /// or the schedule has run out. Counts the attempt.
    pub fn schedule_retry(&mut self) -> Option<(u64, Duration)> {
        let failure = self.failure.as_ref()?;
        if !failure.auto_retry || self.loading {
            return None;
        }
        let wait = browser_load::retry_delay_ms(failure.class, self.attempt + 1)?;
        self.attempt += 1;
        Some((self.generation, Duration::from_millis(u64::from(wait))))
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

/// The host of an address, for the panel's small line.
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

/// The probe's code for a transport error (`browser_load::probe_code`).
#[must_use]
pub fn probe_code_of(error: &ureq::Error) -> i64 {
    match error {
        ureq::Error::HostNotFound => probe_code::DNS,
        ureq::Error::Timeout(_) => probe_code::TIMEOUT,
        ureq::Error::Tls(_) | ureq::Error::Rustls(_) => probe_code::TLS,
        ureq::Error::ConnectionFailed => probe_code::CONNECT,
        ureq::Error::Io(io) => io_code(io),
        _ => 0,
    }
}

fn io_code(io: &std::io::Error) -> i64 {
    use std::io::ErrorKind;
    match io.kind() {
        ErrorKind::ConnectionRefused => probe_code::REFUSED,
        ErrorKind::TimedOut => probe_code::TIMEOUT,
        ErrorKind::NetworkUnreachable
        | ErrorKind::HostUnreachable
        | ErrorKind::NetworkDown
        | ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::NotConnected
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof => probe_code::CONNECT,
        // `std` surfaces a resolver failure as an uncategorized error
        // carrying the libc sentence (`executor::proxy`'s note).
        _ => {
            let text = io.to_string();
            if text.contains("failed to lookup address information")
                || text.contains("nodename nor servname")
                || text.contains("Name or service not known")
                || text.contains("No such host is known")
                || text.contains("No address associated with hostname")
            {
                probe_code::DNS
            } else {
                0
            }
        }
    }
}

/// Ask whether `url` answers at all: one HEAD over the app's own route out,
/// within `budget`. **Blocks** — run it off the frame. Any HTTP status is an
/// answer: the site is up, whatever it thinks of a HEAD.
pub fn probe(url: &str, budget: Duration) -> Result<(), i64> {
    let (tx, rx) = std::sync::mpsc::channel();
    let target = url.to_owned();
    let spawned = std::thread::Builder::new()
        .name("vela-load-probe".to_owned())
        .spawn(move || {
            let answer = crate::executor::proxy::with_candidates_for(&target, budget, |agent| {
                agent.head(&target).call()
            });
            let _ = tx.send(match answer {
                Ok(_) => Ok(()),
                Err(failure) => match failure.error {
                    ureq::Error::StatusCode(_) => Ok(()),
                    error => Err(probe_code_of(&error)),
                },
            });
        });
    if spawned.is_err() {
        return Err(0);
    }
    // The whole chain of routes gets the one budget, not one each.
    rx.recv_timeout(budget).unwrap_or(Err(probe_code::TIMEOUT))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::browser_load::LoadFailureClass;

    const SITE: &str = "https://app.example/swap";

    fn asked(watch: &mut LoadWatch, how: Asked, url: &str, now: f64) -> Option<u64> {
        watch.next_asked = Some(how);
        watch.requested(url, now)
    }

    /// Progress from the request, not from the commit; and the wallet's own
    /// blank page is never watched.
    #[test]
    fn loading_starts_when_the_load_is_asked_for() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 1_000.);
        assert!(generation.is_some());
        assert!(watch.loading && watch.busy());
        watch.committed(SITE);
        assert!(!watch.loading && watch.busy(), "committed, still finishing");
        watch.finished();
        assert!(!watch.busy());
        assert_eq!(watch.requested("about:blank", 2_000.), None);
        assert!(!watch.busy() && watch.url.is_none());
    }

    /// No commit in three seconds: the address is probed, once.
    #[test]
    fn a_silent_load_is_probed_once() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 0.).unwrap_or_default();
        assert_eq!(watch.watchdog(generation).as_deref(), Some(SITE));
        assert_eq!(watch.watchdog(generation), None, "one probe at a time");
        // A commit before the watchdog: nothing to probe.
        let mut quick = LoadWatch::default();
        let generation = quick.requested(SITE, 0.).unwrap_or_default();
        quick.committed(SITE);
        assert_eq!(quick.watchdog(generation), None);
    }

    /// Each probe failure is the core's class, with its sentence and whether
    /// it retries by itself.
    #[test]
    fn a_failed_probe_is_classified_by_the_core() {
        for (code, class, key, auto) in [
            (
                probe_code::DNS,
                LoadFailureClass::NotFound,
                "explore.loadNotFound",
                false,
            ),
            (
                probe_code::REFUSED,
                LoadFailureClass::Refused,
                "explore.loadOffline",
                true,
            ),
            (
                probe_code::TIMEOUT,
                LoadFailureClass::Timeout,
                "explore.loadOffline",
                true,
            ),
            (
                probe_code::TLS,
                LoadFailureClass::Certificate,
                "explore.loadCertificate",
                false,
            ),
            (
                probe_code::CONNECT,
                LoadFailureClass::Offline,
                "explore.loadOffline",
                true,
            ),
            (
                0,
                LoadFailureClass::Other,
                "connect.browser.loadFailed",
                true,
            ),
        ] {
            let mut watch = LoadWatch::default();
            let generation = watch.requested(SITE, 0.).unwrap_or_default();
            watch.watchdog(generation);
            assert_eq!(watch.probed(generation, Err(code)), Probed::Failed);
            let failure = watch
                .failure
                .clone()
                .unwrap_or_else(|| unreachable!("failed"));
            assert_eq!(failure.class, class, "code {code}");
            assert_eq!(failure.reason_key, key);
            assert_eq!(failure.auto_retry, auto);
            assert!(!watch.busy(), "the panel, not the hairline");
        }
    }

    /// A site that answers is slow, not down: wait to 20 s, then a timeout.
    #[test]
    fn a_reachable_site_gets_twenty_seconds() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 5_000.).unwrap_or_default();
        watch.watchdog(generation);
        assert_eq!(watch.probed(generation, Ok(())), Probed::WaitUntil(25_000.));
        assert!(watch.loading && watch.failure.is_none(), "still waiting");
        assert!(watch.give_up(generation));
        assert_eq!(
            watch.failure.as_ref().map(|failure| failure.class),
            Some(LoadFailureClass::Timeout)
        );
        // …unless it committed in the meantime.
        let mut late = LoadWatch::default();
        let generation = late.requested(SITE, 0.).unwrap_or_default();
        late.watchdog(generation);
        late.probed(generation, Ok(()));
        late.committed(SITE);
        assert!(!late.give_up(generation));
        assert!(late.failure.is_none());
    }

    /// A commit at any time clears the panel — even after a probe failed:
    /// the engine got there after all.
    #[test]
    fn a_commit_clears_a_failure() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 0.).unwrap_or_default();
        watch.watchdog(generation);
        watch.probed(generation, Err(probe_code::CONNECT));
        assert!(watch.failure.is_some());
        watch.committed(SITE);
        assert!(watch.failure.is_none() && !watch.retrying && watch.attempt == 0);
    }

    /// The engine's own blank page is not the site: a refused first load
    /// commits `about:blank`, and the watch keeps waiting (and probes).
    #[test]
    fn an_engine_blank_page_is_not_the_site_arriving() {
        let mut watch = LoadWatch::default();
        let generation = watch
            .requested("http://127.0.0.1:9/", 0.)
            .unwrap_or_default();
        watch.committed("about:blank");
        assert!(watch.loading, "still waiting on the site");
        assert_eq!(
            watch.watchdog(generation).as_deref(),
            Some("http://127.0.0.1:9/")
        );
        watch.probed(generation, Err(probe_code::REFUSED));
        watch.committed("about:blank");
        assert!(
            watch.failure.is_some(),
            "the panel stays over the blank page"
        );
        watch.committed("http://127.0.0.1:9/");
        assert!(watch.failure.is_none());
    }

    /// The network classes retry by themselves on the core's schedule (2 s,
    /// 5 s, 10 s), the panel staying up and saying so; a wrong name or a bad
    /// certificate never does.
    #[test]
    fn network_failures_retry_on_the_schedule_with_the_panel_up() {
        let mut watch = LoadWatch::default();
        let mut generation = watch.requested(SITE, 0.).unwrap_or_default();
        let mut waits = Vec::new();
        for _ in 0..4 {
            watch.watchdog(generation);
            watch.probed(generation, Err(probe_code::CONNECT));
            let Some((due, wait)) = watch.schedule_retry() else {
                break;
            };
            waits.push(wait.as_millis());
            let url = watch.retry_fired(due, true).unwrap_or_default();
            generation = watch.requested(&url, 0.).unwrap_or_default();
            assert!(watch.retrying, "the panel stays through the attempt");
            assert!(watch.failure.is_some(), "never the engine's own page");
        }
        assert_eq!(waits, vec![2_000, 5_000, 10_000]);

        for code in [probe_code::DNS, probe_code::TLS] {
            let mut watch = LoadWatch::default();
            let generation = watch.requested(SITE, 0.).unwrap_or_default();
            watch.watchdog(generation);
            watch.probed(generation, Err(code));
            assert_eq!(watch.schedule_retry(), None, "code {code} is not retried");
        }
    }

    /// Retries run only while the page is in front; one that came due while
    /// it was not runs when it comes back.
    #[test]
    fn a_retry_waits_for_the_page_to_be_in_front() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 0.).unwrap_or_default();
        watch.watchdog(generation);
        watch.probed(generation, Err(probe_code::TIMEOUT));
        let (due, _) = watch.schedule_retry().unwrap_or_default();
        assert_eq!(watch.retry_fired(due, false), None);
        assert!(watch.retry_due);
        assert_eq!(watch.take_due().as_deref(), Some(SITE));
        assert_eq!(watch.take_due(), None, "once");
    }

    /// A new address wins: every answer for the old load is dropped.
    #[test]
    fn a_new_address_abandons_the_old_load() {
        let mut watch = LoadWatch::default();
        let old = watch.requested(SITE, 0.).unwrap_or_default();
        watch.watchdog(old);
        let new = watch
            .requested("https://other.example/", 1_000.)
            .unwrap_or_default();
        assert_eq!(watch.probed(old, Err(probe_code::DNS)), Probed::Ignored);
        assert!(watch.failure.is_none());
        assert!(!watch.give_up(old));
        assert_eq!(watch.retry_fired(old, true), None);
        assert_eq!(
            watch.watchdog(new).as_deref(),
            Some("https://other.example/")
        );
        // A new navigation over a failure clears the panel; a Retry keeps it.
        watch.probed(new, Err(probe_code::CONNECT));
        let url = watch.retry().unwrap_or_default();
        asked(&mut watch, Asked::Retry, &url, 2_000.);
        assert!(watch.retrying && watch.failure.is_some());
        watch.requested(SITE, 3_000.);
        assert!(!watch.retrying && watch.failure.is_none());
    }

    /// The manual Retry starts the count over.
    #[test]
    fn retry_resets_the_count() {
        let mut watch = LoadWatch::default();
        let generation = watch.requested(SITE, 0.).unwrap_or_default();
        watch.watchdog(generation);
        watch.probed(generation, Err(probe_code::CONNECT));
        watch.schedule_retry();
        watch.schedule_retry();
        assert_eq!(watch.attempt, 2);
        let url = watch.retry().unwrap_or_default();
        watch.requested(&url, 1.);
        assert_eq!(watch.attempt, 0);
        assert_eq!(watch.retry(), None, "not while it is loading");
    }

    #[test]
    fn transport_errors_map_to_the_probe_codes() {
        use std::io::{Error, ErrorKind};
        assert_eq!(probe_code_of(&ureq::Error::HostNotFound), probe_code::DNS);
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::from(ErrorKind::ConnectionRefused))),
            probe_code::REFUSED
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::from(ErrorKind::NetworkUnreachable))),
            probe_code::CONNECT
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Io(Error::other(
                "failed to lookup address information: nodename nor servname provided"
            ))),
            probe_code::DNS
        );
        assert_eq!(
            probe_code_of(&ureq::Error::Tls("bad cert")),
            probe_code::TLS
        );
        assert_eq!(
            probe_code_of(&ureq::Error::ConnectionFailed),
            probe_code::CONNECT
        );
        assert_eq!(probe_code_of(&ureq::Error::RedirectFailed), 0);
        assert_eq!(
            host_of("https://user@app.example:8443/x?y#z"),
            "app.example:8443"
        );
    }

    /// A real probe against this machine: a closed port is refused.
    #[test]
    fn the_probe_hears_a_refused_port() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        // The listener is dropped: nothing listens there now.
        let refused = probe(&format!("http://127.0.0.1:{closed}/"), PROBE_BUDGET);
        assert_eq!(refused, Err(probe_code::REFUSED));
    }
}
