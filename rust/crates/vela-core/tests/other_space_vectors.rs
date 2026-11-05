//! r13-1: where an account lives when it is not in the space in front
//! (`network_space::lives_elsewhere`) — the one rule every shell's home
//! draws its other-space note from. Replays `vectors/multisig/other-space.json`
//! (the web replays it through the wasm too).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use vela_core::app::network_space::{lives_elsewhere_json, Elsewhere};

#[test]
fn the_other_space_vectors_hold() {
    let path = format!(
        "{}/tests/vectors/multisig/other-space.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 6);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let args = serde_json::json!({
            "chain_ids": case["chain_ids"],
            "here": case["here"],
            "other": case["other"],
            "front": case["front"],
        });
        let got: Option<Elsewhere> =
            serde_json::from_str(&lives_elsewhere_json(&args.to_string())).unwrap();
        let want: Option<Elsewhere> = serde_json::from_value(case["expect"].clone()).unwrap();
        assert_eq!(got, want, "{name}");
    }
}
