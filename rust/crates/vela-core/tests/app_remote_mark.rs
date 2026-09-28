//! Logo misses (spec 082 T035, RE10): a missing, refused or broken image is
//! remembered for the session; a throttled, failing or unreached one for a
//! minute. Every status class is pinned.

#![cfg(feature = "crux")]

use vela_core::app::remote_mark::{
    mark_miss_of_status, mark_miss_ttl_ms, MarkMiss, MARK_MISS_TRANSIENT_MS,
};

#[test]
fn every_status_class_maps_to_its_miss() {
    for (status, want) in [
        (404, MarkMiss::NotFound),
        (410, MarkMiss::NotFound),
        (401, MarkMiss::Refused),
        (403, MarkMiss::Refused),
        (200, MarkMiss::NotAnImage),
        (204, MarkMiss::NotAnImage),
        (429, MarkMiss::Throttled),
        (408, MarkMiss::ServerError),
        (500, MarkMiss::ServerError),
        (502, MarkMiss::ServerError),
        (503, MarkMiss::ServerError),
        (599, MarkMiss::ServerError),
        (0, MarkMiss::Unknown),
        (100, MarkMiss::Unknown),
        (301, MarkMiss::Unknown),
        (400, MarkMiss::Unknown),
        (451, MarkMiss::Unknown),
        (600, MarkMiss::Unknown),
    ] {
        assert_eq!(mark_miss_of_status(status), want, "{status}");
    }
}

/// What asking again will not fix is remembered for the session.
#[test]
fn a_missing_refused_or_broken_image_is_remembered_for_the_session() {
    for miss in [MarkMiss::NotFound, MarkMiss::Refused, MarkMiss::NotAnImage] {
        assert_eq!(mark_miss_ttl_ms(miss), None, "{miss:?}");
    }
}

/// W20: a logo lost to a bad minute comes back after one.
#[test]
fn a_miss_that_may_heal_is_remembered_for_a_minute() {
    assert_eq!(MARK_MISS_TRANSIENT_MS, 60_000);
    for miss in [
        MarkMiss::Throttled,
        MarkMiss::ServerError,
        MarkMiss::Transport,
        MarkMiss::Unknown,
    ] {
        assert_eq!(mark_miss_ttl_ms(miss), Some(60_000), "{miss:?}");
    }
    // From the status straight to the wait.
    assert_eq!(mark_miss_ttl_ms(mark_miss_of_status(503)), Some(60_000));
    assert_eq!(mark_miss_ttl_ms(mark_miss_of_status(404)), None);
}

/// The kinds cross the FFI as snake_case words.
#[test]
fn the_kinds_travel_as_snake_case_words() {
    for (miss, word) in [
        (MarkMiss::NotFound, "not_found"),
        (MarkMiss::Refused, "refused"),
        (MarkMiss::NotAnImage, "not_an_image"),
        (MarkMiss::Throttled, "throttled"),
        (MarkMiss::ServerError, "server_error"),
        (MarkMiss::Transport, "transport"),
        (MarkMiss::Unknown, "unknown"),
    ] {
        assert_eq!(
            serde_json::to_value(miss).unwrap_or_default(),
            serde_json::Value::String(word.to_owned())
        );
        assert_eq!(
            serde_json::from_value::<MarkMiss>(serde_json::Value::String(word.to_owned())).ok(),
            Some(miss)
        );
    }
}
