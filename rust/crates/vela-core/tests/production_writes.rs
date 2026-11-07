//! §28 — a test build never writes to production: the shared vector
//! `vectors/rules/production-writes.json`, replayed through the core's own
//! rule. Each shell replays the same file through its door to the rule (its
//! guard test), so the vector is the contract and this is its first reader.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use vela_core::service_defaults::{
    self, is_production_host, opt_in, service_defaults, write_verdict, write_verdict_json,
    BuildFacts, WriteVerdict, ALLOW_PRODUCTION_WRITES, REFUSED_KEY,
};

const VECTOR: &str = include_str!("vectors/rules/production-writes.json");

fn vector() -> Value {
    serde_json::from_str(VECTOR).expect("the vector is JSON")
}

#[test]
fn the_defaults_are_the_vectors() {
    let v = vector();
    let defaults = serde_json::to_value(service_defaults()).unwrap();
    assert_eq!(
        defaults, v["defaults"],
        "the vector's defaults are the core's"
    );
    assert_eq!(v["refusedKey"], REFUSED_KEY);
    assert_eq!(v["optInName"], ALLOW_PRODUCTION_WRITES);
    // Every default's host is a production host: the guard covers what the
    // shells ship pointing at.
    for (name, url) in defaults.as_object().unwrap() {
        let host = service_defaults::url_host(url.as_str().unwrap()).unwrap();
        assert!(is_production_host(&host), "{name}: {host}");
    }
}

#[test]
fn every_build_fact_makes_a_test_build() {
    for case in vector()["builds"].as_array().unwrap() {
        let facts: BuildFacts = serde_json::from_value(case["facts"].clone()).unwrap();
        assert_eq!(
            facts.is_test_build(),
            case["testBuild"].as_bool().unwrap(),
            "{}",
            case["about"]
        );
    }
}

#[test]
fn the_opt_in_reads_as_the_vector_says() {
    for pair in vector()["optIn"].as_array().unwrap() {
        assert_eq!(
            opt_in(pair[0].as_str()),
            pair[1].as_bool().unwrap(),
            "{:?}",
            pair[0]
        );
    }
}

#[test]
fn every_case_answers_as_the_vector_says() {
    let cases = vector()["cases"].as_array().unwrap().clone();
    assert!(cases.len() >= 20);
    for case in &cases {
        let about = case["about"].as_str().unwrap();
        let verdict = write_verdict(
            case["testBuild"].as_bool().unwrap(),
            case["allowProductionWrites"].as_bool().unwrap(),
            case["method"].as_str().unwrap(),
            case["url"].as_str().unwrap(),
            case["body"].as_str(),
        );
        match case["verdict"].as_str().unwrap() {
            "allowed" => assert_eq!(verdict, WriteVerdict::Allowed, "{about}"),
            "refused" => match &verdict {
                WriteVerdict::Refused { host, key, .. } => {
                    assert_eq!(host, case["host"].as_str().unwrap(), "{about}");
                    assert_eq!(key, REFUSED_KEY, "{about}");
                }
                WriteVerdict::Allowed => panic!("{about}: allowed"),
            },
            other => panic!("{about}: unknown verdict {other}"),
        }
        // The JSON door (uniffi, wasm) answers the same.
        let mut ask = case.clone();
        ask.as_object_mut().unwrap().remove("verdict");
        let answered: Value = serde_json::from_str(&write_verdict_json(&ask.to_string())).unwrap();
        assert_eq!(answered["verdict"], case["verdict"], "{about} (json)");
    }
}

#[test]
fn a_release_build_is_never_refused() {
    // Whatever it sends, wherever it sends it: the rule is for test builds.
    for case in vector()["cases"].as_array().unwrap() {
        let verdict = write_verdict(
            false,
            false,
            case["method"].as_str().unwrap(),
            case["url"].as_str().unwrap(),
            case["body"].as_str(),
        );
        assert_eq!(verdict, WriteVerdict::Allowed, "{}", case["about"]);
    }
}
