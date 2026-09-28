//! Page-load rules (spec 079): the failure classifier, the retry schedule,
//! the visit rule and the avatar letter — every shell's browser uses these,
//! so every row the device pass or the audit named is pinned here.

#![cfg(feature = "crux")]

use vela_core::app::browser_load::{
    classify, probe_code, reason_key, retry_delay_ms, site_letter, visit_to_record,
    LoadFailureClass as C, LoadFinished, LoadPlatform as P,
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
