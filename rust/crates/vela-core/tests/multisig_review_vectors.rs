//! The state-diff guard on real chain answers: each vector under
//! `tests/vectors/multisig-review/` is what the core asked
//! (`reviewReads`) and what anvil on the Gnosis fork answered — the
//! `debug_traceCall` prestate diff, the `simulateAndRevert` + SafeStateProbe
//! revert payload, `eth_simulateV1` — for a 1.4.1 Safe, owners 0x1111…,
//! 0x2222…, 1 of 2. `-probe` vectors had the trace answered "not supported"
//! (as Arc answers it), `-probe-only` the simulation too.
//!
//! Re-capture with the fork harness up (`scripts/onchain/multisig-fork`):
//! the scratch script `live-review.mjs` drives the wasm's `multisigCall`.
#![cfg(feature = "crux")]

use serde_json::{json, Value};
use vela_core::multisig::call_json;

fn run(name: &str) -> (Value, Value) {
    let path = format!(
        "{}/tests/vectors/multisig-review/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let out = call_json(
        "multisigReview",
        &json!({ "input": honest(&v["input"]) }).to_string(),
    )
    .unwrap();
    (serde_json::from_str(&out).unwrap(), v["expect"].clone())
}

/// R22: the captured answers as an honest network gives a corroborated read
/// — each one witnessed alike by a second, independent node (anvil answered
/// once; a second honest node answers the same).
fn honest(input: &Value) -> Value {
    let mut input = input.clone();
    for key in ["trace", "probe"] {
        if input[key].is_object() {
            input[key]["witness"] = input[key].clone();
        }
    }
    if let Some(reply) = input["simReply"].as_str() {
        let mut v: Value = serde_json::from_str(reply).unwrap();
        if v.get("result").is_some() || v.get("error").is_some() {
            v["witness"] = v.clone();
        }
        input["simReply"] = json!(v.to_string());
    }
    input
}

#[test]
fn a_declared_add_owner_passes_on_both_sources() {
    for (name, source) in [
        ("fork-add-owner-trace", "trace"),
        ("fork-add-owner-probe", "probe"),
    ] {
        let (got, expect) = run(name);
        assert_eq!(got["state"], expect["state"], "{name}");
        assert_eq!(got["state"]["source"], json!(source), "{name}");
        assert_eq!(got["state"]["verdict"]["kind"], json!("declared"), "{name}");
        assert_eq!(got["sign"]["allowed"], json!(true), "{name}");
    }
}

#[test]
fn bybit_slot_zero_write_is_refused_on_both_sources() {
    for name in ["fork-bybit-trace", "fork-bybit-probe"] {
        let (got, expect) = run(name);
        assert_eq!(got["state"], expect["state"], "{name}");
        let writes = &got["state"]["verdict"]["writes"];
        assert_eq!(writes[0]["kind"], json!("singleton"), "{name}");
        assert_eq!(
            writes[0]["after"],
            json!("0xBAd00000000000000000000000000000000000ba"),
            "{name}"
        );
        assert_eq!(got["tone"], json!("refused"), "{name}");
        assert_eq!(
            got["sign"],
            json!({ "allowed": false, "anyway": false }),
            "{name}"
        );
        // A delegatecall: refused as one, and nothing said about what moves
        // — never "No asset changes" under the refusal.
        assert_eq!(got["refused"]["why"], json!("delegatecall"), "{name}");
        assert_eq!(got["simulation"], expect["simulation"], "{name}");
        assert_eq!(got["simulation"]["kind"], json!("withheld"), "{name}");
        assert!(got["simulation"]["no_change_key"].is_null(), "{name}");
        assert!(
            !got["concerns"]
                .as_array()
                .unwrap()
                .iter()
                .any(|c| c["kind"] == "unsimulated"),
            "{name}"
        );
    }
}

#[test]
fn what_moves_from_the_probe_where_nothing_else_answers() {
    let (got, expect) = run("fork-transfer-probe-only");
    assert_eq!(got["simulationSource"], json!("probe"));
    assert_eq!(got["simulationSource"], expect["simulationSource"]);
    assert_eq!(
        got["simulation"]["deltas"][0]["delta"],
        json!("-100000000000000000")
    );
    assert_eq!(got["simulation"]["deltas"][0]["kind"], json!("native"));
    assert_eq!(got["state"]["verdict"]["kind"], json!("unchanged"));
}

/// Arc Testnet (`rpc.testnet.arc.network`, 2026-10-29): no trace, no
/// eth_simulateV1 — the probe alone, on a multisig deployed nowhere yet: the
/// override puts it in place from its verified genesis (the singleton's code
/// read from Arc and checked against its hash).
#[test]
fn arc_undeployed_the_probe_puts_the_safe_in_place() {
    let (got, expect) = run("arc-undeployed-add-owner-probe");
    assert_eq!(got["state"], expect["state"]);
    assert_eq!(
        got["state"],
        json!({ "source": "probe", "verdict": { "kind": "declared" } })
    );
    // 0.01 USDC out of a Safe holding none: it would fail — the sheet's danger.
    let (got, _) = run("arc-undeployed-transfer-probe");
    assert_eq!(got["state"]["verdict"]["kind"], json!("reverts"));
    assert_eq!(got["simulation"]["kind"], json!("reverts"));
    assert_eq!(got["sign"], json!({ "allowed": true, "anyway": true }));
}
