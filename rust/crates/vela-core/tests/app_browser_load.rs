//! Page-load rules (spec 079): the failure classifier, the retry schedule,
//! the visit rule and the avatar letter — every shell's browser uses these,
//! so every row the device pass or the audit named is pinned here. Spec 082
//! (RD4) moved the desktop's load watch in with its tests.

#![cfg(feature = "crux")]

use vela_core::app::browser_load::same_address;
use vela_core::app::browser_load::{
    address_bar, classify, host_of, probe_code, reason_key, retry_delay_ms,
    retry_when_network_returns, should_give_up, site_label, site_letter, stalled, visit_to_record,
    AddressBar, Asked, BarLock, EngineSample, EngineVerdict, LoadFailureClass as C, LoadFinished,
    LoadPlatform as P, LoadWatch, Probed, RetryAction, SiteLabel, ENGINE_LIVE_PROGRESS, GIVE_UP_MS,
};
use vela_core::app::browser_load::{pinned_title, Visit};

fn class(platform: P, code: i64, domain: Option<&str>) -> Option<C> {
    classify(platform, code, domain, false).map(|failure| failure.class)
}

#[test]
fn android_codes_map_to_their_class() {
    for (code, want) in [
        (-2, C::NotFound),  // ERROR_HOST_LOOKUP
        (-12, C::NotFound), // ERROR_BAD_URL
        (-10, C::NotFound), // ERROR_UNSUPPORTED_SCHEME
        (-6, C::Refused),   // ERROR_CONNECT
        (-8, C::Timeout),   // ERROR_TIMEOUT
        (-7, C::Offline),   // ERROR_IO
        (-1, C::Offline),   // ERROR_UNKNOWN — net::ERR_EMPTY_RESPONSE on the Xiaomi
        (-11, C::Certificate),
        (-5, C::Proxy),  // ERROR_PROXY_AUTHENTICATION
        (-15, C::Other), // ERROR_TOO_MANY_REQUESTS
    ] {
        assert_eq!(class(P::Android, code, None), Some(want), "android {code}");
    }
}

/// Spec 083: WebView2's own error statuses (Windows) map like Android's
/// codes do — the same person-facing class for the same network fault.
#[test]
fn webview2_statuses_map_to_their_class() {
    for (code, want) in [
        (0, C::Other), // UNKNOWN
        (1, C::Certificate),
        (2, C::Certificate),
        (3, C::Certificate),
        (4, C::Certificate),
        (5, C::Certificate),
        (6, C::Offline), // SERVER_UNREACHABLE
        (7, C::Timeout),
        (8, C::Offline), // ERR_EMPTY_RESPONSE, seen on the device pass
        (9, C::Offline),
        (10, C::Offline),
        (11, C::Offline),
        (12, C::Refused),
        (13, C::NotFound),
        (15, C::Other), // REDIRECT_FAILED
        (16, C::Other),
        (17, C::Other),
        (18, C::Other),
        (99, C::Other),
    ] {
        assert_eq!(
            class(P::WebView2, code, None),
            Some(want),
            "webview2 {code}"
        );
    }
    assert_eq!(
        class(P::WebView2, 14, None),
        None,
        "a replaced navigation is not a failure"
    );
    assert_eq!(class(P::WebView2, 8, None), class(P::Android, -1, None));
    for code in 1..=5 {
        let failure = classify(P::WebView2, code, None, false);
        assert!(failure.is_some_and(|f| !f.auto_retry && f.reason_key == "explore.loadCertificate"));
    }
    assert_eq!(
        serde_json::to_string(&P::WebView2).ok().as_deref(),
        Some("\"webview2\"")
    );
}

#[test]
fn apple_codes_map_to_their_class() {
    let url = Some("NSURLErrorDomain");
    for (code, want) in [
        // 082 G31: every -1000 seen was the per-app proxy refusing CONNECT.
        (-1000, C::Offline),
        (-1002, C::NotFound),
        (-1003, C::NotFound),
        (-1006, C::NotFound),
        (-1001, C::Timeout),
        (-1004, C::Refused),
        (-1009, C::Offline),
        (-1005, C::Offline),
        (-1018, C::Offline),
        (-1020, C::Offline),
        (-2000, C::Offline),
        (-1200, C::Certificate),
        (-1202, C::Certificate),
        (-1206, C::Certificate),
        (-1011, C::Other),
    ] {
        assert_eq!(class(P::Apple, code, url), Some(want), "apple {code}");
    }
    // A domain the shell did not name is read as NSURLErrorDomain.
    assert_eq!(class(P::Apple, -1009, None), Some(C::Offline));
    assert_eq!(
        class(P::Apple, 101, Some("WebKitErrorDomain")),
        Some(C::Other)
    );
    assert_eq!(class(P::Apple, 7, Some("SomethingElse")), Some(C::Other));
}

/// 082 RD9: the proxy is named only when it could not be used; a proxy that
/// answered the CONNECT spoke for the host.
#[test]
fn apple_proxy_failures_name_the_proxy() {
    let cf = Some("kCFErrorDomainCFNetwork");
    for (code, want) in [
        (306, C::Proxy),   // kCFErrorHTTPProxyConnectionFailure
        (307, C::Proxy),   // kCFErrorHTTPBadProxyCredentials
        (308, C::Proxy),   // kCFErrorPACFileError
        (309, C::Proxy),   // kCFErrorPACFileAuth
        (310, C::Proxy),   // kCFErrorHTTPSProxyConnectionFailure
        (311, C::Refused), // unexpected response to CONNECT: the proxy answered
        (305, C::Other),
        (312, C::Other),
    ] {
        assert_eq!(class(P::Apple, code, cf), Some(want), "cfnetwork {code}");
    }
    // The same numbers in NSURLErrorDomain mean nothing of the kind.
    assert_eq!(
        class(P::Apple, 306, Some("NSURLErrorDomain")),
        Some(C::Other)
    );
}

#[test]
fn a_cancelled_or_interrupted_navigation_is_not_a_failure() {
    assert_eq!(
        classify(P::Apple, -999, Some("NSURLErrorDomain"), false),
        None
    );
    assert_eq!(
        classify(P::Apple, 102, Some("WebKitErrorDomain"), false),
        None
    );
    assert_eq!(
        classify(P::Apple, 204, Some("WebKitErrorDomain"), false),
        None
    );
}

#[test]
fn the_certificate_callback_wins_over_any_code() {
    let failure = classify(P::Android, -1, None, true);
    assert_eq!(
        failure.map(|f| (f.class, f.auto_retry)),
        Some((C::Certificate, false))
    );
}

#[test]
fn the_desktop_probe_codes_map_to_their_class() {
    for (code, want) in [
        (probe_code::DNS, C::NotFound),
        (probe_code::REFUSED, C::Refused),
        (probe_code::TIMEOUT, C::Timeout),
        (probe_code::TLS, C::Certificate),
        (probe_code::CONNECT, C::Offline),
        (probe_code::PROXY, C::Proxy),
        (0, C::Other),
    ] {
        assert_eq!(class(P::Probe, code, None), Some(want), "probe {code}");
    }
}

#[test]
fn every_failure_carries_its_sentence_and_retry_verdict() {
    let failure = classify(P::Android, -8, None, false);
    assert_eq!(
        failure.map(|f| (f.reason_key, f.auto_retry)),
        Some(("explore.loadOffline".to_owned(), true))
    );
    // Offline, timeout and refused share one sentence; the class still
    // drives the retry schedule.
    for (c, key) in [
        (C::Offline, "explore.loadOffline"),
        (C::Timeout, "explore.loadOffline"),
        (C::Refused, "explore.loadOffline"),
        (C::NotFound, "explore.loadNotFound"),
        (C::Certificate, "explore.loadCertificate"),
        (C::Other, "connect.browser.loadFailed"),
        (C::Proxy, "explore.loadProxy"),
    ] {
        assert_eq!(reason_key(c), key);
    }
    assert_eq!(
        classify(P::Probe, probe_code::PROXY, None, false).map(|f| (f.reason_key, f.auto_retry)),
        Some(("explore.loadProxy".to_owned(), true))
    );
}

#[test]
fn network_failures_retry_three_times_on_a_growing_wait() {
    for c in [C::Offline, C::Timeout, C::Refused, C::Proxy] {
        let schedule: Vec<_> = (1..=4).map(|attempt| retry_delay_ms(c, attempt)).collect();
        assert_eq!(
            schedule,
            vec![Some(2_000), Some(5_000), Some(10_000), None],
            "{c:?}"
        );
    }
    assert_eq!(retry_delay_ms(C::Other, 1), Some(3_000));
    assert_eq!(retry_delay_ms(C::Other, 2), None);
}

#[test]
fn a_typo_and_a_bad_certificate_never_retry() {
    for attempt in 1..=3 {
        assert_eq!(retry_delay_ms(C::NotFound, attempt), None);
        assert_eq!(retry_delay_ms(C::Certificate, attempt), None);
    }
    assert_eq!(
        classify(P::Android, -2, None, false).map(|f| f.auto_retry),
        Some(false)
    );
}

fn finished(url: &str, title: &str) -> LoadFinished {
    LoadFinished {
        url: url.to_owned(),
        title: title.to_owned(),
        icon: Some("https://app.uniswap.org/favicon.png".to_owned()),
        main_frame_failed: false,
        http_status: None,
    }
}

#[test]
fn a_page_that_loaded_is_a_visit_with_its_own_fields() {
    let visit = visit_to_record(finished(
        "https://app.uniswap.org/#/swap",
        "  Uniswap Interface ",
    ));
    let visit = visit.map(|v| (v.url, v.title, v.favicon));
    assert_eq!(
        visit,
        Some((
            "https://app.uniswap.org/#/swap".to_owned(),
            Some("Uniswap Interface".to_owned()),
            Some("https://app.uniswap.org/favicon.png".to_owned())
        ))
    );
}

#[test]
fn a_failed_load_is_never_a_visit() {
    // The Xiaomi recorded "网页无法打开" — the engine's error page — for 127.0.0.1:8137.
    let mut load = finished("http://127.0.0.1:8137/", "网页无法打开");
    load.main_frame_failed = true;
    assert_eq!(visit_to_record(load), None);
}

#[test]
fn an_error_status_is_never_a_visit() {
    for status in [404, 500, 503] {
        let mut load = finished("https://example.com/missing", "Not Found");
        load.http_status = Some(status);
        assert_eq!(visit_to_record(load), None, "{status}");
    }
    let mut ok = finished("https://example.com/", "Example");
    ok.http_status = Some(200);
    assert!(visit_to_record(ok).is_some());
}

#[test]
fn engine_documents_are_never_visits() {
    for url in [
        "about:blank",
        "chrome-error://chromewebdata/",
        "data:text/html,x",
        "file:///sdcard/x.html",
        "",
    ] {
        assert_eq!(visit_to_record(finished(url, "x")), None, "{url}");
    }
}

#[test]
fn an_empty_title_and_a_non_web_icon_are_left_out() {
    let mut load = finished("https://example.com/", "   ");
    load.icon = Some("data:image/png;base64,AAAA".to_owned());
    let visit = visit_to_record(load).map(|v| (v.title, v.favicon));
    assert_eq!(visit, Some((None, None)));
}

#[test]
fn the_letter_skips_the_subdomain_everyone_puts_in_front() {
    for (host, want) in [
        ("app.uniswap.org", "U"),
        ("www.example.com", "E"),
        ("m.x.io", "X"),
        ("APP.Aave.com", "A"),
        ("app.io", "A"), // nothing dotted would remain: the prefix is the name
        ("bscscan.com", "B"),
        ("127.0.0.1", "1"),
        ("127.0.0.1:8137", "1"),
        ("xn--fiqs8s.com", "X"),
        ("", "?"),
        ("...", "?"),
    ] {
        assert_eq!(site_letter(host), want, "{host}");
    }
}

// ---------------------------------------------------------------------------
// The load watch — ported from the desktop's `explore/load_watch.rs` tests
// (spec 082 T029); the probe's own transport tests stay with the probe.
// ---------------------------------------------------------------------------

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
    watch.finished(SITE);
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
        (probe_code::DNS, C::NotFound, "explore.loadNotFound", false),
        (probe_code::REFUSED, C::Refused, "explore.loadOffline", true),
        (probe_code::TIMEOUT, C::Timeout, "explore.loadOffline", true),
        (
            probe_code::TLS,
            C::Certificate,
            "explore.loadCertificate",
            false,
        ),
        (probe_code::CONNECT, C::Offline, "explore.loadOffline", true),
        (probe_code::PROXY, C::Proxy, "explore.loadProxy", true),
        (0, C::Other, "connect.browser.loadFailed", true),
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
        Some(C::Timeout)
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
    watch.finished("about:blank");
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
        waits.push(wait);
        let RetryAction::Load(url) = watch.retry_fired(due, true) else {
            unreachable!("in front, the engine idle: a load");
        };
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
    assert_eq!(watch.retry_fired(due, false), RetryAction::NotInFront);
    assert!(watch.retry_due);
    assert_eq!(watch.take_due(), RetryAction::Load(SITE.to_owned()));
    assert_eq!(watch.take_due(), RetryAction::Nothing, "once");
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
    assert_eq!(watch.retry_fired(old, true), RetryAction::Nothing);
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

/// The panel's small line: userinfo dropped, the port kept (from the
/// desktop's transport test, whose ureq half stays with the probe).
#[test]
fn the_host_line_drops_userinfo_and_keeps_the_port() {
    assert_eq!(
        host_of("https://user@app.example:8443/x?y#z"),
        "app.example:8443"
    );
}

// ---------------------------------------------------------------------------
// The engine's own account first (spec 082 T030, RD3, RD7)
// ---------------------------------------------------------------------------

fn sample(loading: bool, progress: f64, url: &str) -> EngineSample {
    EngineSample {
        loading,
        progress,
        url: Some(url.to_owned()),
    }
}

/// A watch whose load of `SITE` the engine has picked up (WebKit's first
/// provisional progress, 0.1).
fn engine_on_site() -> (LoadWatch, u64) {
    let mut watch = LoadWatch::default();
    let generation = watch.requested(SITE, 0.).unwrap_or_default();
    assert_eq!(
        watch.engine(&sample(true, 0.1, SITE), 250.),
        EngineVerdict::Nothing,
        "the wallet's own load"
    );
    assert!(watch.engine_loading);
    (watch, generation)
}

/// W7: the probe gave up at ~8 s while WebKit was still on the page; the
/// retry due at ~10 s must not send a second request for it.
#[test]
fn a_retry_never_restarts_a_load_the_engine_is_still_on() {
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::TIMEOUT)),
        Probed::Failed,
        "the panel shows fast"
    );
    let (due, wait) = watch.schedule_retry().unwrap_or_default();
    assert_eq!(wait, 2_000);
    assert_eq!(
        watch.retry_fired(due, true),
        RetryAction::EngineStillLoading
    );
    assert_eq!(
        watch.schedule_retry(),
        Some((due, 2_000)),
        "the attempt was not spent"
    );
    // Brought back to the front while the engine is still on it: the same.
    assert_eq!(watch.retry_fired(due, false), RetryAction::NotInFront);
    assert_eq!(watch.take_due(), RetryAction::EngineStillLoading);
    // The engine gives up too (the panel is already up), and the next
    // attempt is a real load.
    watch.schedule_retry();
    assert_eq!(
        watch.engine(&sample(false, 0.1, SITE), 30_000.),
        EngineVerdict::Nothing
    );
    assert_eq!(
        watch.retry_fired(due, true),
        RetryAction::Load(SITE.to_owned())
    );
}

/// The engine stopping with no commit IS the failure; the probe only says
/// why — and a site that answers the probe is "other", not "wait".
#[test]
fn the_engine_stopping_without_a_commit_is_the_failure() {
    let (mut watch, generation) = engine_on_site();
    assert_eq!(
        watch.engine(&sample(false, 0.1, SITE), 1_200.),
        EngineVerdict::StoppedWithoutCommit {
            generation,
            url: SITE.to_owned()
        }
    );
    assert!(watch.probing && watch.loading);
    assert_eq!(watch.watchdog(generation), None, "the probe is running");
    assert_eq!(watch.probed(generation, Ok(())), Probed::Failed);
    assert_eq!(
        watch.failure.as_ref().map(|failure| failure.class),
        Some(C::Other)
    );

    // A probe that fails gives its own class.
    let (mut watch, generation) = engine_on_site();
    watch.engine(&sample(false, 0.1, SITE), 1_200.);
    watch.probed(generation, Err(probe_code::DNS));
    assert_eq!(
        watch.failure.as_ref().map(|failure| failure.class),
        Some(C::NotFound)
    );

    // Stopped while the watchdog's probe is out: that probe classifies it.
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.engine(&sample(false, 0.1, SITE), 3_500.),
        EngineVerdict::Nothing
    );
    assert_eq!(watch.probed(generation, Ok(())), Probed::Failed);

    // WebKit's blank page after a refused first load is the engine stopping.
    let (mut watch, generation) = engine_on_site();
    assert!(matches!(
        watch.engine(&sample(true, 0.1, "about:blank"), 600.),
        EngineVerdict::StoppedWithoutCommit { generation: g, .. } if g == generation
    ));

    // A load that committed and then finished is not a failure.
    let (mut watch, _) = engine_on_site();
    watch.committed(SITE);
    assert_eq!(
        watch.engine(&sample(false, 1.0, SITE), 900.),
        EngineVerdict::Nothing
    );
    watch.finished(SITE);
    assert!(watch.failure.is_none() && !watch.busy());
}

/// A stop is the failure only while the engine stays stopped: when it takes
/// the load up again before the probe answers, a site that answers is
/// "wait", as for any live load — never an "other" panel over a page that
/// is arriving.
#[test]
fn a_stop_the_engine_takes_back_is_not_the_failure() {
    let (mut watch, generation) = engine_on_site();
    assert!(matches!(
        watch.engine(&sample(false, 0.1, SITE), 500.),
        EngineVerdict::StoppedWithoutCommit { .. }
    ));
    assert_eq!(
        watch.engine(&sample(true, 0.3, SITE), 750.),
        EngineVerdict::Nothing,
        "the wallet's own load, taken up again"
    );
    assert!(!watch.engine_stopped, "no longer stopped");
    assert_eq!(
        watch.probed(generation, Ok(())),
        Probed::WaitUntil(f64::from(GIVE_UP_MS))
    );
    assert!(watch.loading && watch.failure.is_none());
    // Stopping again is the failure again, and its probe classifies it.
    let EngineVerdict::StoppedWithoutCommit { .. } =
        watch.engine(&sample(false, 0.3, SITE), 4_000.)
    else {
        unreachable!("stopped again with no commit");
    };
    assert_eq!(watch.probed(generation, Ok(())), Probed::Failed);
    assert_eq!(
        watch.failure.as_ref().map(|failure| failure.class),
        Some(C::Other)
    );
}

/// W18: a load the page started (a link, a form, a script) is watched like
/// any other — hairline, watchdog, probe, panel — but only retried by hand.
#[test]
fn a_load_the_page_started_is_watched_and_retried_by_hand_only() {
    let (mut watch, _) = engine_on_site();
    watch.committed(SITE);
    watch.engine(&sample(false, 1.0, SITE), 900.);
    watch.finished(SITE);

    let pool = "https://app.example/pool";
    let EngineVerdict::PageStarted { generation, url } =
        watch.engine(&sample(true, 0.1, pool), 5_000.)
    else {
        unreachable!("an unasked load");
    };
    assert_eq!(url, pool);
    assert!(watch.page_initiated && watch.loading && watch.busy());
    assert_eq!(watch.requested_at_ms, 5_000.);
    assert_eq!(watch.watchdog(generation).as_deref(), Some(pool));
    watch.probed(generation, Err(probe_code::CONNECT));
    assert_eq!(
        watch.failure.as_ref().map(|failure| failure.class),
        Some(C::Offline)
    );
    assert_eq!(watch.schedule_retry(), None, "manual Retry only");

    // The person's Retry is the wallet's own load: the schedule applies again.
    let again = watch.retry().unwrap_or_default();
    assert_eq!(again, pool);
    let generation = watch.requested(&again, 9_000.).unwrap_or_default();
    assert!(!watch.page_initiated);
    watch.watchdog(generation);
    watch.probed(generation, Err(probe_code::CONNECT));
    assert!(watch.schedule_retry().is_some());
}

/// Only a NEW web address with nothing of the wallet's under way is a page
/// load: not the wallet's own request, not the same address starting again,
/// not the engine's blank page.
#[test]
fn the_wallet_s_own_loads_are_never_page_loads() {
    let (watch, generation) = engine_on_site();
    assert_eq!(watch.generation, generation, "the request, not a new load");
    assert!(!watch.page_initiated);

    let mut settled = LoadWatch::default();
    settled.requested(SITE, 0.);
    settled.committed(SITE);
    settled.finished(SITE);
    assert_eq!(
        settled.engine(&sample(true, 0.1, SITE), 1_000.),
        EngineVerdict::Nothing,
        "the same address starting again"
    );
    settled.engine(&sample(false, 1.0, SITE), 1_500.);
    assert_eq!(
        settled.engine(&sample(true, 0.1, "about:blank"), 2_000.),
        EngineVerdict::Nothing
    );
    // Committed and still finishing (a poll that missed the start): the
    // wallet's load, whatever the address says after a redirect.
    let mut finishing = LoadWatch::default();
    finishing.requested("http://app.example/", 0.);
    finishing.committed("https://app.example/");
    assert_eq!(
        finishing.engine(&sample(true, 0.8, "https://app.example/"), 300.),
        EngineVerdict::Nothing
    );
}

/// DX5: a site whose page is arriving is never cut at 20 s; one stuck at
/// WebKit's first 0.1 still is.
#[test]
fn a_page_that_is_arriving_is_never_given_up_on() {
    let (mut watch, generation) = engine_on_site();
    watch.engine(&sample(true, 0.4, SITE), 8_000.);
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Ok(())),
        Probed::WaitUntil(f64::from(GIVE_UP_MS))
    );
    assert!(!watch.give_up(generation), "progress 0.4 at 20 s");
    assert!(watch.loading && watch.failure.is_none());

    let (mut stuck, generation) = engine_on_site();
    stuck.engine(&sample(true, ENGINE_LIVE_PROGRESS, SITE), 8_000.);
    stuck.watchdog(generation);
    stuck.probed(generation, Ok(()));
    assert!(stuck.give_up(generation), "never rose above the threshold");
    assert_eq!(
        stuck.failure.as_ref().map(|failure| failure.class),
        Some(C::Timeout)
    );
}

/// The probe's TLS stack is not WebKit's: while the engine is still loading,
/// a certificate verdict waits for the engine; after it stops, it stands.
#[test]
fn a_certificate_verdict_waits_for_a_loading_engine() {
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::TLS)),
        Probed::Deferred
    );
    assert!(watch.loading && !watch.probing && watch.failure.is_none());
    // The engine committed after all: nothing was ever shown.
    watch.committed(SITE);
    assert!(watch.failure.is_none());

    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    watch.probed(generation, Err(probe_code::TLS));
    let EngineVerdict::StoppedWithoutCommit { .. } =
        watch.engine(&sample(false, 0.1, SITE), 6_000.)
    else {
        unreachable!("stopped with no commit");
    };
    assert_eq!(
        watch.probed(generation, Err(probe_code::TLS)),
        Probed::Failed
    );
    assert_eq!(
        watch.failure.as_ref().map(|failure| failure.class),
        Some(C::Certificate)
    );
    // Without an engine to ask (Windows), the probe's verdict stands at once.
    let mut probe_only = LoadWatch::default();
    let generation = probe_only.requested(SITE, 0.).unwrap_or_default();
    probe_only.watchdog(generation);
    assert_eq!(
        probe_only.probed(generation, Err(probe_code::TLS)),
        Probed::Failed
    );
}

/// A deferred verdict is not a pass: the 20 s give-up still ends a load
/// that WebKit holds at its first step (RD3, RE2), so the probe's TLS
/// answer never leaves a hairline running until WebKit's own minute. A page
/// that is arriving is still never cut.
#[test]
fn a_deferred_verdict_still_gives_up_at_twenty_seconds() {
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::TLS)),
        Probed::Deferred
    );
    watch.engine(&sample(true, 0.1, SITE), 19_000.);
    assert!(watch.give_up(generation), "held at 0.1 for 20 s: given up");
    assert_eq!(watch.failure, Some(stalled()));

    let (mut arriving, generation) = engine_on_site();
    arriving.watchdog(generation);
    arriving.probed(generation, Err(probe_code::TLS));
    arriving.engine(&sample(true, 0.5, SITE), 9_000.);
    assert!(!arriving.give_up(generation), "the page is arriving");
    assert!(arriving.loading && arriving.failure.is_none());
}

// ---------------------------------------------------------------------------
// The phones' watchdog and the network coming back (spec 082 T031, RE2, RE3)
// ---------------------------------------------------------------------------

/// One give-up rule on every client: 20 s with nothing committed and the
/// page not arriving.
#[test]
fn a_load_is_given_up_at_twenty_seconds_only_if_nothing_is_arriving() {
    assert_eq!(GIVE_UP_MS, 20_000);
    assert!(!should_give_up(19_000, false, 0.1), "19 s");
    assert!(should_give_up(20_000, false, 0.1), "20 s, no commit, 0.1");
    assert!(should_give_up(45_000, false, 0.0));
    assert!(!should_give_up(20_000, true, 0.1), "committed");
    assert!(!should_give_up(20_000, false, 0.2), "progress 0.2");
    assert!(
        should_give_up(20_000, false, ENGINE_LIVE_PROGRESS),
        "WebKit's own first step is not the page arriving"
    );
}

/// A given-up load is a timeout: the network sentence, retried on the
/// network schedule — the same failure the desktop's watch gives.
#[test]
fn a_stalled_load_is_a_timeout_that_retries() {
    let failure = stalled();
    assert_eq!(failure.class, C::Timeout);
    assert_eq!(failure.reason_key, "explore.loadOffline");
    assert!(failure.auto_retry);

    let mut watch = LoadWatch::default();
    let generation = watch.requested(SITE, 0.).unwrap_or_default();
    watch.watchdog(generation);
    watch.probed(generation, Ok(()));
    watch.give_up(generation);
    assert_eq!(watch.failure, Some(stalled()));
}

/// When the network comes back, every class it can heal is loaded again; a
/// typo and a wrong certificate are not.
#[test]
fn only_what_the_network_can_heal_is_retried_when_it_returns() {
    for class in [C::Offline, C::Timeout, C::Refused, C::Other, C::Proxy] {
        assert!(retry_when_network_returns(class), "{class:?}");
    }
    for class in [C::Certificate, C::NotFound] {
        assert!(!retry_when_network_returns(class), "{class:?}");
    }
}

// ---------------------------------------------------------------------------
// The address bar and a site named once (spec 082 T033, RE1, RE7)
// ---------------------------------------------------------------------------

fn bar(url: &str, host: &str, lock: BarLock) -> AddressBar {
    AddressBar {
        url: url.to_owned(),
        host: host.to_owned(),
        lock,
    }
}

/// G28: a page may start a load of any address; until it commits, the bar
/// keeps naming the document on screen.
#[test]
fn a_pending_load_never_renames_the_page_on_screen() {
    let jumper = "https://jumper.exchange/?fromChain=100";
    let uniswap = "https://app.uniswap.org/#/swap";
    assert_eq!(
        address_bar(Some(jumper), Some(uniswap), None),
        bar(jumper, "jumper.exchange", BarLock::Closed)
    );
    // After the commit it is the new page's, with its own lock.
    assert_eq!(
        address_bar(Some(uniswap), None, None),
        bar(uniswap, "app.uniswap.org", BarLock::Closed)
    );
}

/// A failure panel names the failed host, with no lock: nothing from that
/// host is on screen — whatever is committed or pending behind it.
#[test]
fn a_failure_names_the_failed_host_without_a_lock() {
    assert_eq!(
        address_bar(
            Some("https://jumper.exchange/"),
            Some("https://app.uniswap.org/"),
            Some("https://app.uniswap.org/#/swap"),
        ),
        bar(
            "https://app.uniswap.org/#/swap",
            "app.uniswap.org",
            BarLock::None
        )
    );
    assert_eq!(
        address_bar(None, None, Some("http://127.0.0.1:9/")),
        bar("http://127.0.0.1:9/", "127.0.0.1:9", BarLock::None)
    );
}

/// The lock is the signing rule's: https, and http on loopback or a private
/// network, are closed; public http is open.
#[test]
fn the_lock_follows_the_origin_rule() {
    for (url, host, lock) in [
        (
            "https://app.uniswap.org/",
            "app.uniswap.org",
            BarLock::Closed,
        ),
        ("http://127.0.0.1:8137/", "127.0.0.1:8137", BarLock::Closed),
        ("http://localhost:5173/", "localhost:5173", BarLock::Closed),
        (
            "http://192.168.50.9:8137/x",
            "192.168.50.9:8137",
            BarLock::Closed,
        ),
        ("http://10.0.0.8/", "10.0.0.8", BarLock::Closed),
        ("http://example.com/", "example.com", BarLock::Open),
        (
            "http://10.0.0.1.evil.com/",
            "10.0.0.1.evil.com",
            BarLock::Open,
        ),
    ] {
        assert_eq!(
            address_bar(Some(url), None, None),
            bar(url, host, lock),
            "{url}"
        );
    }
}

/// The host is the origin's: a non-default port kept, a default one and any
/// userinfo dropped, lower-cased.
#[test]
fn the_host_keeps_a_non_default_port() {
    assert_eq!(
        address_bar(Some("http://127.0.0.1:8137/"), None, None).host,
        "127.0.0.1:8137"
    );
    assert_eq!(
        address_bar(Some("https://App.Example:443/x"), None, None).host,
        "app.example"
    );
    assert_eq!(
        address_bar(Some("https://user@app.example:8443/"), None, None).host,
        "app.example:8443"
    );
}

/// An empty tab shows where it is going, with no lock; a tab with nothing
/// shows nothing. The engine's blank page is not a document.
#[test]
fn an_empty_tab_names_its_pending_load() {
    let pending = "https://app.uniswap.org/";
    let going = bar(pending, "app.uniswap.org", BarLock::None);
    assert_eq!(address_bar(None, Some(pending), None), going);
    assert_eq!(address_bar(Some("about:blank"), Some(pending), None), going);
    assert_eq!(address_bar(Some("  "), Some(pending), None), going);
    assert_eq!(address_bar(None, None, None), bar("", "", BarLock::None));
    assert_eq!(
        address_bar(Some("about:blank"), Some("about:blank"), None),
        bar("", "", BarLock::None)
    );
}

fn label(name: &str, host_line: Option<&str>) -> SiteLabel {
    SiteLabel {
        name: name.to_owned(),
        host_line: host_line.map(str::to_owned),
    }
}

/// A name that is its host is said once; a real name sits over its host.
#[test]
fn a_site_named_by_its_host_is_said_once() {
    assert_eq!(
        site_label("127.0.0.1:8137", "127.0.0.1:8137"),
        label("127.0.0.1:8137", None)
    );
    assert_eq!(
        site_label("Uniswap", "app.uniswap.org"),
        label("Uniswap", Some("app.uniswap.org"))
    );
    assert_eq!(
        site_label("APP.Uniswap.ORG", "app.uniswap.org"),
        label("app.uniswap.org", None),
        "ASCII case is not a second name"
    );
    assert_eq!(
        site_label("  ", "app.uniswap.org"),
        label("app.uniswap.org", None)
    );
    assert_eq!(site_label("Uniswap", ""), label("Uniswap", None));
}

// ---------------------------------------------------------------------------
// Spec 086 (issue #329): the title a page is pinned under
// ---------------------------------------------------------------------------

/// What a shell reads from the document when a load finishes, made a visit
/// by the core's own rule — the only door a title has into `pinned_title`.
fn visit_of(url: &str, title: &str, failed: bool) -> Option<Visit> {
    visit_to_record(LoadFinished {
        url: url.to_owned(),
        title: title.to_owned(),
        icon: None,
        main_frame_failed: failed,
        http_status: None,
    })
}

/// Issue #329: app.uniswap.org failed to load and was pinned as "网页无法打开",
/// the WebView's own error page. That page is never a visit — failed, or read
/// at its `chrome-error://` address — so it never names a favourite.
#[test]
fn an_engine_error_page_never_names_a_favourite() {
    let pinned = "https://app.uniswap.org/";
    let error_page = visit_of("chrome-error://chromewebdata/", "网页无法打开", false);
    assert_eq!(
        error_page, None,
        "an engine document is no visit, failed flag or not"
    );
    assert_eq!(pinned_title(pinned, error_page.as_ref()), None);
    let failed = visit_of(pinned, "网页无法打开", true);
    assert_eq!(pinned_title(pinned, failed.as_ref()), None);
    // …and the favourite is then its host (`explore_sites`: no title → host).
}

/// The site's last good title survives a later failure of the same site.
#[test]
fn a_site_keeps_its_last_good_title_under_a_failure() {
    let good = visit_of("https://app.uniswap.org/swap", "Uniswap Interface", false);
    assert_eq!(
        pinned_title("https://app.uniswap.org/", good.as_ref()),
        Some("Uniswap Interface".to_owned()),
        "same origin, another path: the same site"
    );
    assert_eq!(
        pinned_title("https://APP.Uniswap.org/explore", good.as_ref()),
        Some("Uniswap Interface".to_owned()),
        "the host's case is not another site"
    );
}

/// The page before is another site: its title never names this one — the
/// failed address, or a load that has no title yet, is pinned by its host.
#[test]
fn another_site_s_title_never_names_a_favourite() {
    let before = visit_of("https://bscscan.com/", "BscScan", false);
    assert_eq!(
        pinned_title("https://app.uniswap.org/", before.as_ref()),
        None
    );
    assert_eq!(
        pinned_title("http://bscscan.com/", before.as_ref()),
        None,
        "another scheme"
    );
    assert_eq!(
        pinned_title("https://bscscan.com:8443/", before.as_ref()),
        None,
        "another port"
    );
    assert_eq!(
        pinned_title("https://app.uniswap.org/", None),
        None,
        "nothing loaded yet"
    );
}

/// A visit with no title, a blank one, or an address that is no web page
/// gives no title — never an empty name.
#[test]
fn no_title_is_no_title() {
    let untitled = visit_of("https://app.uniswap.org/", "   ", false);
    assert_eq!(
        pinned_title("https://app.uniswap.org/", untitled.as_ref()),
        None
    );
    let good = visit_of("https://app.uniswap.org/", "  Uniswap Interface  ", false);
    assert_eq!(
        pinned_title("https://app.uniswap.org/", good.as_ref()),
        Some("Uniswap Interface".to_owned())
    );
    assert_eq!(pinned_title("about:blank", good.as_ref()), None);
    assert_eq!(pinned_title("", good.as_ref()), None);
}

// ---------------------------------------------------------------------------
// Spec 082 round 2 (T190): the retry race, one address spelling, hung loads
// ---------------------------------------------------------------------------

/// RJ8: one comparison of addresses everywhere the watch compares them.
#[test]
fn same_address_ignores_what_does_not_change_the_page() {
    let same = [
        ("https://app.uniswap.org", "https://app.uniswap.org/"),
        ("HTTPS://App.Uniswap.ORG/", "https://app.uniswap.org"),
        (
            "https://app.uniswap.org:443/swap",
            "https://app.uniswap.org/swap",
        ),
        ("http://127.0.0.1:80/", "http://127.0.0.1"),
        (
            "https://app.uniswap.org/swap/",
            "https://app.uniswap.org/swap",
        ),
        (
            "https://app.uniswap.org/swap#top",
            "https://app.uniswap.org/swap",
        ),
        ("https://a.test/x?q=1", "https://a.test/x?q=1#frag"),
    ];
    for (a, b) in same {
        assert!(same_address(a, b), "{a} = {b}");
        assert!(same_address(b, a), "{b} = {a}");
    }
    let different = [
        ("https://app.uniswap.org", "http://app.uniswap.org"),
        ("https://app.uniswap.org:8443", "https://app.uniswap.org"),
        (
            "https://app.uniswap.org/Swap",
            "https://app.uniswap.org/swap",
        ),
        ("https://a.test/x?q=1", "https://a.test/x?q=2"),
        ("https://a.test/x?q=1", "https://a.test/x"),
        ("https://a.test/x//", "https://a.test/x"),
        ("https://a.test", "https://b.test"),
    ];
    for (a, b) in different {
        assert!(!same_address(a, b), "{a} ≠ {b}");
    }
}

/// One failed attempt of the wallet's own load at `url`: the engine picks it
/// up (spelled `engine_url`), stops with no commit, and the probe fails.
fn fail_once(watch: &mut LoadWatch, generation: u64, engine_url: &str, at: f64) {
    watch.engine(&sample(true, 0.1, engine_url), at + 100.);
    let stopped = watch.engine(&sample(false, 0.1, engine_url), at + 1_000.);
    assert!(
        matches!(stopped, EngineVerdict::StoppedWithoutCommit { generation: g, .. } if g == generation),
        "{stopped:?}"
    );
    assert_eq!(
        watch.probed(generation, Err(probe_code::CONNECT)),
        Probed::Failed
    );
}

/// DX14 (G43): the engine reports `https://app.uniswap.org/` for a typed
/// `https://app.uniswap.org`, and its poll sees the retry start before the
/// wallet's own `requested`. That is the wallet's own attempt — never a load
/// the page started: the count goes on, three attempts at +2, +5 and +10 s,
/// and the panel stays up between them.
#[test]
fn the_wallet_s_own_retry_is_never_taken_for_a_page_load() {
    const TYPED: &str = "https://app.uniswap.org";
    const ENGINE: &str = "https://app.uniswap.org/";
    let mut watch = LoadWatch::default();
    let mut generation = watch.requested(TYPED, 0.).unwrap_or_default();
    let mut now = 0.;
    fail_once(&mut watch, generation, ENGINE, now);
    let mut waits = Vec::new();
    while let Some((due, wait)) = watch.schedule_retry() {
        waits.push(wait);
        now += 1_000. + f64::from(wait);
        let RetryAction::Load(url) = watch.retry_fired(due, true) else {
            panic!("attempt {} is a load", waits.len());
        };
        assert_eq!(url, TYPED);
        // The engine is seen starting BEFORE the wallet reports the request.
        assert_eq!(
            watch.engine(&sample(true, 0.1, ENGINE), now),
            EngineVerdict::Nothing,
            "the wallet's own attempt, not a page load"
        );
        assert!(!watch.page_initiated);
        assert_eq!(
            watch.attempt,
            u32::try_from(waits.len()).unwrap_or_default()
        );
        assert!(watch.failure.is_some(), "the panel stays up");
        generation = watch.requested(&url, now).unwrap_or_default();
        assert!(watch.retrying && !watch.page_initiated);
        fail_once(&mut watch, generation, ENGINE, now);
    }
    assert_eq!(
        waits,
        vec![2_000, 5_000, 10_000],
        "three attempts, no restart"
    );
}

/// The panel's own Retry, seen starting by the engine first, is the same.
#[test]
fn the_panel_s_retry_seen_first_by_the_engine_is_the_wallet_s() {
    let mut watch = LoadWatch::default();
    let generation = watch.requested(SITE, 0.).unwrap_or_default();
    fail_once(&mut watch, generation, SITE, 0.);
    let url = watch.retry().unwrap_or_default();
    assert_eq!(
        watch.engine(&sample(true, 0.1, SITE), 5_000.),
        EngineVerdict::Nothing
    );
    assert!(!watch.page_initiated);
    watch.requested(&url, 5_100.);
    assert!(watch.retrying, "the panel stays, saying it is trying again");
}

/// L2 (G44): WebKit's provisional load hangs at 0.1. The probe says why
/// quickly, and the attempts that fall due while WebKit is still on it are
/// given back — until the load is 20 s old: the next one is a real load,
/// which cancels the hung one, instead of waiting for WebKit's own minute.
#[test]
fn a_hung_provisional_load_is_replaced_after_twenty_seconds() {
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::PROXY)),
        Probed::Failed
    );
    let mut now = 5_000.;
    let mut given_back = 0;
    loop {
        let (due, wait) = watch.schedule_retry().unwrap_or_default();
        now += f64::from(wait);
        // WebKit is still on it, still at 0.1.
        watch.engine(&sample(true, 0.1, SITE), now);
        match watch.retry_fired(due, true) {
            RetryAction::EngineStillLoading => {
                assert!(now < f64::from(GIVE_UP_MS), "given back at {now}");
                given_back += 1;
            }
            RetryAction::Load(url) => {
                assert_eq!(url, SITE);
                assert!(now >= f64::from(GIVE_UP_MS), "loaded again at {now}");
                break;
            }
            other => panic!("{other:?} at {now}"),
        }
    }
    assert!(given_back > 0, "the young load was protected");
}

/// DX1: a 9 s proxy latency keeps WebKit at 0.1 until it commits; the
/// attempts before that are all given back — one `loadRequest`, never a
/// restart.
#[test]
fn a_slow_load_is_never_restarted_before_it_commits() {
    let (mut watch, generation) = engine_on_site();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::TIMEOUT)),
        Probed::Failed
    );
    let (due, wait) = watch.schedule_retry().unwrap_or_default();
    watch.engine(&sample(true, 0.1, SITE), 8_000. + f64::from(wait) - 2_000.);
    assert_eq!(
        watch.retry_fired(due, true),
        RetryAction::EngineStillLoading
    );
    watch.committed(SITE);
    assert!(watch.failure.is_none() && watch.attempt == 0);
    assert_eq!(watch.retry_fired(due, true), RetryAction::Nothing);
}

/// A live load past 20 s (the page arriving) is still never replaced.
#[test]
fn a_live_load_is_never_replaced() {
    let (mut watch, generation) = engine_on_site();
    watch.engine(&sample(true, 0.5, SITE), 4_000.);
    watch.watchdog(generation);
    watch.probed(generation, Err(probe_code::TIMEOUT));
    let (due, _) = watch.schedule_retry().unwrap_or_default();
    watch.engine(&sample(true, 0.6, SITE), 25_000.);
    assert_eq!(
        watch.retry_fired_at(due, true, 25_000.),
        RetryAction::EngineStillLoading
    );
}

// ---------------------------------------------------------------------------
// Spec 083: WebView2's own error page and its navigation failure (ported from
// the desktop's load watch when 082 moved the watch into the core)
// ---------------------------------------------------------------------------

/// Spec 083 W3: WebView2's own error page is not the site arriving; the
/// navigation's failure puts Vela's panel up at once, with its reason.
#[test]
fn an_engine_error_page_is_not_a_commit() {
    let mut watch = LoadWatch::default();
    watch.requested(SITE, 0.);
    watch.error_page();
    assert!(watch.engine_page && watch.loading && watch.failure.is_none());
    assert!(watch.engine_failed(SITE, 8, false), "a retry is booked");
    assert!(!watch.engine_page && !watch.loading);
    assert_eq!(watch.failure.as_ref().map(|f| f.class), Some(C::Offline));
    assert!(watch.schedule_retry().is_some());
}

/// The probe failed first; WebView2's own timeout ~40 s later keeps the
/// panel and books nothing twice.
#[test]
fn the_engines_late_timeout_keeps_the_panel() {
    let mut watch = LoadWatch::default();
    let generation = watch.requested(SITE, 0.).unwrap_or_default();
    watch.watchdog(generation);
    assert_eq!(
        watch.probed(generation, Err(probe_code::TIMEOUT)),
        Probed::Failed
    );
    let before = watch.failure.clone();
    assert!(!watch.engine_failed(SITE, 7, false));
    assert_eq!(watch.failure, before);
}

/// A replaced navigation is not a failure; a bad certificate is final.
#[test]
fn a_replaced_navigation_changes_nothing_and_a_bad_certificate_is_final() {
    let mut watch = LoadWatch::default();
    watch.requested(SITE, 0.);
    assert!(!watch.engine_failed(SITE, 14, false));
    assert!(watch.loading && watch.failure.is_none());
    assert!(watch.engine_failed(SITE, 2, true));
    assert_eq!(
        watch.failure.as_ref().map(|f| f.class),
        Some(C::Certificate)
    );
    assert_eq!(watch.schedule_retry(), None, "never retried by itself");
}

/// A link to a dead host (not a load the wallet asked for) names its own
/// address, so Retry loads that.
#[test]
fn a_failed_link_names_its_own_address() {
    let mut watch = LoadWatch::default();
    watch.requested(SITE, 0.);
    watch.committed(SITE);
    watch.finished(SITE);
    assert!(watch.engine_failed("https://dead.example/", 13, false));
    assert_eq!(watch.url.as_deref(), Some("https://dead.example/"));
    assert_eq!(watch.failure.as_ref().map(|f| f.class), Some(C::NotFound));
}

/// Spec 083 FR-005: the bar names the load asked for until a page commits,
/// and keeps naming it under a failure; never the wallet's blank page.
#[test]
fn the_bar_names_the_load_asked_for_until_a_page_commits() {
    let mut watch = LoadWatch::default();
    assert_eq!(watch.named_url(), None);
    let generation = watch.requested(SITE, 0.).unwrap_or_default();
    assert_eq!(watch.named_url(), Some(SITE));
    watch.committed(SITE);
    assert_eq!(watch.named_url(), None, "a page stands for itself now");
    let generation2 = watch.requested(SITE, 1_000.).unwrap_or_default();
    assert_ne!(generation, generation2);
    assert_eq!(watch.watchdog(generation2).as_deref(), Some(SITE));
    assert_eq!(
        watch.probed(generation2, Err(probe_code::DNS)),
        Probed::Failed
    );
    assert_eq!(
        watch.named_url(),
        Some(SITE),
        "the failed address stays named"
    );
    assert_eq!(watch.requested("about:blank", 2_000.), None);
}
