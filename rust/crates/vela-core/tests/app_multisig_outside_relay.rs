//! The relay token's rules through the one door a shell asks them by
//! (`multisigLinkCall("relay.<rule>", …)`): the decision cases of the web's
//! `external/relay.test.ts`, ported. The transport keeps only the socket,
//! the timers and the fetches; every decision below is the core's.

#![cfg(feature = "crux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::{json, Value};
use vela_core::multisig_link::call_json;

fn ask(rule: &str, args: Value) -> Value {
    serde_json::from_str(&call_json(&format!("relay.{rule}"), &args.to_string()).unwrap()).unwrap()
}

const NOW: f64 = 1_800_000_000.0;
const YEAR: f64 = NOW + 365.0 * 24.0 * 3600.0;

#[test]
fn only_the_public_relay_asks_for_a_token() {
    let c = ask("constants", json!({}));
    let public = c["defaultRelay"].as_str().unwrap();
    assert_eq!(public, "wss://relay.peerconnect.dev");
    assert_eq!(ask("needsToken", json!({ "relay": public })), json!(true));
    assert_eq!(
        ask("needsToken", json!({ "relay": format!("{public}/") })),
        json!(true)
    );
    assert_eq!(
        ask("needsToken", json!({ "relay": "ws://127.0.0.1:8792" })),
        json!(false),
        "a profile's own relay gets none"
    );
}

#[test]
fn a_kept_token_is_used_until_it_is_about_to_expire_and_the_trial_only_when_asked() {
    let kept = json!({ "token": "t-site", "expiresAt": YEAR, "source": "site" });
    assert_eq!(
        ask("usable", json!({ "kept": kept, "nowS": NOW })),
        json!({ "token": "t-site", "source": "site" })
    );
    // About to expire (inside the ten-minute margin): replaced.
    let old = json!({ "token": "t-old", "expiresAt": NOW + 60.0 });
    assert_eq!(
        ask("usable", json!({ "kept": old, "nowS": NOW })),
        Value::Null
    );
    // The trial's asked for: the site's kept one does not do.
    assert_eq!(
        ask(
            "usable",
            json!({ "kept": kept, "nowS": NOW, "trial": true })
        ),
        Value::Null
    );
    let trial = json!({ "token": "t-trial", "expiresAt": NOW + 7200.0, "source": "trial" });
    assert_eq!(
        ask(
            "usable",
            json!({ "kept": trial, "nowS": NOW, "trial": true })
        )["source"],
        json!("trial")
    );
    // Nothing kept.
    assert_eq!(
        ask("usable", json!({ "kept": null, "nowS": NOW })),
        Value::Null
    );
    // A token kept before sources were written is the site's.
    let unsourced = json!({ "token": "t-1", "expiresAt": YEAR });
    assert_eq!(
        ask("usable", json!({ "kept": unsourced, "nowS": NOW }))["source"],
        json!("site")
    );
}

#[test]
fn what_a_token_endpoint_answered() {
    let body = json!({ "token": "t-site", "expires_at": YEAR, "relay_url": "wss://x" }).to_string();
    assert_eq!(
        ask("tokenAnswer", json!({ "status": 200, "body": body }))["ok"]["token"],
        json!("t-site")
    );
    assert_eq!(
        ask(
            "tokenAnswer",
            json!({ "status": 503, "body": "{\"error\":\"not_configured\"}" })
        ),
        json!({ "err": "HTTP 503" }),
        "the site cannot issue one: the trial's is asked for"
    );
    assert_eq!(
        ask("tokenAnswer", json!({ "status": 200, "body": "{}" })),
        json!({ "err": "no token in the answer" })
    );
    assert_eq!(
        ask("tokenAnswer", json!({ "status": null, "body": "" })),
        json!({ "err": "no answer" })
    );
}

#[test]
fn no_token_is_said_with_what_each_place_answered() {
    let c = ask("constants", json!({}));
    assert_eq!(
        ask(
            "noTokenDetail",
            json!({ "endpoint": c["tokenEndpoint"], "site": "HTTP 429", "trial": "HTTP 429" })
        ),
        json!("getvela.app: HTTP 429 · peerconnect.dev: HTTP 429")
    );
    // Only the trial was asked (the last chance before stopping).
    assert_eq!(
        ask(
            "noTokenDetail",
            json!({ "endpoint": c["tokenEndpoint"], "site": null, "trial": "no answer" })
        ),
        json!("peerconnect.dev: no answer")
    );
}

#[test]
fn only_the_refused_token_is_forgotten() {
    let kept = json!({ "token": "t-1", "expiresAt": YEAR });
    assert_eq!(
        ask("forgets", json!({ "kept": kept, "refused": "t-other" })),
        json!(false)
    );
    assert_eq!(
        ask("forgets", json!({ "kept": kept, "refused": "t-1" })),
        json!(true)
    );
    assert_eq!(
        ask("forgets", json!({ "kept": kept, "refused": null })),
        json!(true),
        "none named: the kept one goes"
    );
}

/// r4C-1: a token the relay refuses is never followed by another at once —
/// 2 s, then 4 s, a fresh one from the site, then the trial's once, then
/// stopped until Retry. The count is the transport's, handed in and back.
#[test]
fn a_connection_refused_by_the_relay_waits_falls_back_to_the_trial_once_then_stops() {
    let c = ask("constants", json!({}));
    let backoff = c["backoffMs"].as_array().unwrap().clone();
    let mut state = Value::Null;
    let mut on = |event: &str, source: Option<&str>| -> Value {
        let answer = ask(
            "refusals",
            json!({ "state": state, "on": event, "source": source }),
        );
        state = answer["state"].clone();
        answer["before"].clone()
    };
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "go", "trial": false })
    );
    on("issued", Some("site"));
    on("unauthorized", None);
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "wait", "ms": backoff[0] })
    );
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "go", "trial": false })
    );
    on("issued", Some("site"));
    on("unauthorized", None);
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "wait", "ms": backoff[1] })
    );
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "go", "trial": true }),
        "the trial's, once"
    );
    on("issued", Some("trial"));
    on("unauthorized", None);
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "stop" }),
        "no loop: said as failed"
    );
    // Retry: asked again, from the site.
    on("retried", None);
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "go", "trial": false })
    );
    // On the relay: the refusals are forgotten.
    on("issued", Some("site"));
    on("unauthorized", None);
    on("beforeToken", None);
    on("accepted", None);
    on("unauthorized", None);
    assert_eq!(
        on("beforeToken", None),
        json!({ "kind": "wait", "ms": backoff[0] }),
        "counted afresh"
    );
}

/// r4C-4: getting no token at all backs off to 30 s, then stops.
#[test]
fn getting_no_token_backs_off_to_30_s_then_stops() {
    let steps: Vec<Value> = (0..=5)
        .map(|tries| ask("noTokenStep", json!({ "tries": tries })))
        .collect();
    assert_eq!(steps[0], json!({ "kind": "wait", "ms": 2000 }));
    assert_eq!(steps[1], json!({ "kind": "wait", "ms": 4000 }));
    assert_eq!(steps[4], json!({ "kind": "wait", "ms": 30000 }));
    assert_eq!(steps[5], json!({ "kind": "stop" }));
}

#[test]
fn up_once_on_the_relay_and_unreachable_when_a_token_does_not_get_it_there() {
    let c = ask("constants", json!({}));
    let after = c["unreachableAfterMs"].as_f64().unwrap();
    let relay = c["defaultRelay"].clone();
    let offline = ask(
        "onState",
        json!({ "status": { "kind": "connecting" }, "state": "offline", "offlineMs": after + 10.0, "relay": relay }),
    );
    assert_eq!(
        offline["status"],
        json!({ "kind": "retrying", "why": "unreachable", "detail": "relay.peerconnect.dev", "at": null })
    );
    assert_eq!(offline["accepted"], json!(false));
    // Not long enough yet: still connecting.
    assert_eq!(
        ask(
            "onState",
            json!({ "status": { "kind": "connecting" }, "state": "offline", "offlineMs": 0, "relay": relay })
        )["status"],
        json!({ "kind": "connecting" })
    );
    let up = ask(
        "onState",
        json!({ "status": offline["status"], "state": "waiting", "relay": relay }),
    );
    assert_eq!(up["status"], json!({ "kind": "up" }));
    assert_eq!(up["accepted"], json!(true), "its refusals are forgotten");
    // Up, then offline again: connecting, not up.
    assert_eq!(
        ask(
            "onState",
            json!({ "status": { "kind": "up" }, "state": "offline", "relay": relay })
        )["status"],
        json!({ "kind": "connecting" })
    );
    // Closed: what was said stands.
    let failed = json!({ "kind": "failed", "why": "refused", "detail": "HTTP 4403" });
    assert_eq!(
        ask(
            "onState",
            json!({ "status": failed, "state": "closed", "relay": relay })
        )["status"],
        failed
    );
}

#[test]
fn a_pages_way_onto_the_relay_is_the_worst_of_its_rooms() {
    let failed = json!({ "kind": "failed", "why": "noToken", "detail": "x" });
    assert_eq!(
        ask(
            "worst",
            json!({ "statuses": [{ "kind": "up" }, failed, { "kind": "connecting" }] })
        ),
        failed
    );
    assert_eq!(
        ask("worst", json!({ "statuses": [] })),
        json!({ "kind": "up" })
    );
}
