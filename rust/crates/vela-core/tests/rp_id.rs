//! §35 (review 0-1 follow-up): the relying party a build's passkeys live
//! under — the shared vector `tests/vectors/rules/rp-id.json`, replayed through
//! the core's rule and its JSON door (what uniffi and the wasm crate expose).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use vela_core::service_defaults::{
    relying_party_id, relying_party_json, web_relying_party_id, RpIdError, LOCAL_RP_ID,
    RELEASE_RP_ID, TEST_RP_DOMAIN, TEST_RP_DOMAIN_UNSET,
};

const VECTOR: &str = include_str!("vectors/rules/rp-id.json");

fn vector() -> Value {
    serde_json::from_str(VECTOR).unwrap()
}

#[test]
fn every_native_case_is_the_cores_answer() {
    let v = vector();
    assert_eq!(v["release"], RELEASE_RP_ID);
    assert_eq!(v["local"], LOCAL_RP_ID);
    // Owner, 2026-11-14: a device test build uses the production relying
    // party unless it names another (the vector's expectations assume that).
    assert_eq!(TEST_RP_DOMAIN, Some(RELEASE_RP_ID));
    for case in v["native"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let ask = &case["ask"];
        let answer = relying_party_id(
            ask["testBuild"].as_bool().unwrap(),
            ask["simulator"].as_bool().unwrap(),
            ask["testDomain"].as_str(),
        );
        let door: Value = serde_json::from_str(&relying_party_json(&ask.to_string())).unwrap();
        match case["rpId"].as_str() {
            Some(rp_id) => {
                assert_eq!(answer.as_deref(), Ok(rp_id), "{name}");
                assert_eq!(door["rpId"], rp_id, "{name}: the JSON door");
            }
            None => {
                let kind = case["error"].as_str().unwrap();
                let error = answer.expect_err(name);
                assert_eq!(door["error"]["kind"], kind, "{name}: the JSON door");
                match (&error, kind) {
                    (RpIdError::TestDomainUnset { message }, "test_domain_unset") => {
                        assert_eq!(message, TEST_RP_DOMAIN_UNSET);
                    }
                    (RpIdError::TestDomainInvalid { message, .. }, "test_domain_invalid") => {
                        assert!(message.contains("TEST_RP_DOMAIN"), "{message}");
                    }
                    other => panic!("{name}: {other:?}"),
                }
            }
        }
    }
}

#[test]
fn every_web_case_is_the_cores_answer() {
    for case in vector()["web"].as_array().unwrap() {
        assert_eq!(
            web_relying_party_id(
                case["host"].as_str().unwrap(),
                case["packaged"].as_bool().unwrap()
            ),
            case["rpId"].as_str().unwrap(),
            "{}",
            case["name"]
        );
    }
}

/// The words say what to do, and the door fails closed on an ask it cannot
/// read: no relying party at all, never a guess.
#[test]
fn an_unreadable_ask_says_how_to_set_it_and_yields_no_relying_party() {
    assert!(TEST_RP_DOMAIN_UNSET.contains("TEST_RP_DOMAIN"));
    assert!(TEST_RP_DOMAIN_UNSET.contains("docs/security/hardening.md"));
    let door: Value = serde_json::from_str(&relying_party_json("not json")).unwrap();
    assert!(door.get("rpId").is_none());
    assert_eq!(door["error"]["kind"], "test_domain_unset");
}
