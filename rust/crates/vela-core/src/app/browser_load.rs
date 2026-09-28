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
