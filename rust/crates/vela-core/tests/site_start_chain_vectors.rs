//! dapp-17, r9, r10 — where a new site starts. The shared vectors
//! (`tests/vectors/site-start-chain.json`) are answered by the core's rule
//! here and by the extension worker's twin (`extension/lib/protocol.js`,
//! replayed in `src/lib/dapp/protocol.test.ts`): the worker cannot run the
//! core, so the two are held to one file and cannot drift.

#![cfg(feature = "crux")]

use serde_json::Value;
use vela_core::app::network_space::site_start_chain_for;

fn vectors() -> Value {
    let path = format!(
        "{}/tests/vectors/site-start-chain.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| unreachable!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{path}: {e}"))
}

fn chain(value: &Value) -> Option<u32> {
    value.as_u64().and_then(|id| u32::try_from(id).ok())
}

fn chains(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .map(|list| list.iter().filter_map(chain).collect())
        .unwrap_or_default()
}

#[test]
fn every_vector_starts_where_written() {
    let doc = vectors();
    let cases = doc["cases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 10, "the vectors went missing");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let start = site_start_chain_for(
            &chains(&case["space_chain_ids"]),
            chain(&case["last_used"]),
            &chains(&case["account_chains"]),
        );
        assert_eq!(Some(start), chain(&case["start"]), "{name}");
    }
}
