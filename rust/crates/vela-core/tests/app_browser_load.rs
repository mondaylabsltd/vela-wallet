//! Page-load rules (spec 079): the failure classifier, the retry schedule,
//! the visit rule and the avatar letter — every shell's browser uses these,
//! so every row the device pass or the audit named is pinned here. Spec 082
//! (RD4) moved the desktop's load watch in with its tests.

#![cfg(feature = "crux")]

use vela_core::app::browser_load::{
    classify, host_of, probe_code, reason_key, retry_delay_ms, retry_when_network_returns,
    should_give_up, site_letter, stalled, visit_to_record, Asked, EngineSample, EngineVerdict,
    LoadFailureClass as C, LoadFinished, LoadPlatform as P, LoadWatch, Probed, RetryAction,
    ENGINE_LIVE_PROGRESS, GIVE_UP_MS,
};

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
        (-15, C::Other), // ERROR_TOO_MANY_REQUESTS
    ] {
        assert_eq!(class(P::Android, code, None), Some(want), "android {code}");
    }
}

#[test]
fn apple_codes_map_to_their_class() {
    let url = Some("NSURLErrorDomain");
    for (code, want) in [
        (-1000, C::NotFound),
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
    ] {
        assert_eq!(reason_key(c), key);
    }
}

#[test]
fn network_failures_retry_three_times_on_a_growing_wait() {
    for c in [C::Offline, C::Timeout, C::Refused] {
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
    for class in [C::Offline, C::Timeout, C::Refused, C::Other] {
        assert!(retry_when_network_returns(class), "{class:?}");
    }
    for class in [C::Certificate, C::NotFound] {
        assert!(!retry_when_network_returns(class), "{class:?}");
    }
}
