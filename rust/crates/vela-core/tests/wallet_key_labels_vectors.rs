//! r10 follow-up #5b — what a key row is called
//! (`tests/vectors/wallet-keys/labels.json`).
//!
//! The keys view never draws a blank label and never the account's name: a
//! key's own name, else "Key n" by its place in the founding order. The same
//! file is replayed through the wasm door and the web's words in
//! `app-web/vela-wallet/src/lib/services/wallet-keys.test.ts`.

use serde_json::Value;
use vela_core::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};
use vela_core::wallet_keys::{step_json, KeysStep, KEY_N_KEY};

const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

fn read(path: &str) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| unreachable!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{path}: {e}"))
}

fn vectors() -> Value {
    read(&format!(
        "{}/tests/vectors/wallet-keys/labels.json",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// The web's verbatim recording of the registry on both chains.
fn recorded() -> Value {
    read(&format!(
        "{}/../../../app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json",
        env!("CARGO_MANIFEST_DIR")
    ))
}

/// "Key {{n}}" in English, from the corpus itself.
fn key_n_en() -> String {
    let en = read(&format!(
        "{}/i18n/locales/en/settingsModals.json",
        env!("CARGO_MANIFEST_DIR")
    ));
    en["settingsModals"]["keys"]["keyN"]
        .as_str()
        .unwrap_or_else(|| unreachable!("en keyN"))
        .to_owned()
}

fn answer(fixture: &Value, request: LookupRequest) -> LookupAnswer {
    let LookupRequest::EthCall {
        id, chain_id, data, ..
    } = request
    else {
        unreachable!("the keys walk asks only eth_call")
    };
    let body = fixture["answers"][chain_id.to_string()][&data].as_str();
    LookupAnswer {
        id,
        outcome: if body.is_some() {
            LookupOutcome::Ok
        } else {
            LookupOutcome::Failed
        },
        body: body.map(str::to_owned),
    }
}

/// The walk to its end, through the JSON door the bindings use — answered
/// from the recording, or asked with no address (nobody to ask).
fn walk(device_json: &str, chains: &str) -> Vec<Value> {
    let fixture = recorded();
    let address = if chains == "recorded" { SAFE } else { "" };
    let mut answers: Vec<LookupAnswer> = Vec::new();
    for _ in 0..32 {
        let transcript = serde_json::to_string(&answers).unwrap_or_default();
        let step: KeysStep =
            serde_json::from_str(&step_json(address, device_json, &transcript, ""))
                .unwrap_or_else(|e| unreachable!("{e}"));
        match step {
            KeysStep::Ask { requests } => {
                answers.extend(requests.into_iter().map(|r| answer(&fixture, r)));
            }
            KeysStep::Done { keys, .. } => {
                return keys
                    .iter()
                    .map(|row| serde_json::to_value(row).unwrap_or_default())
                    .collect();
            }
        }
    }
    unreachable!("the walk never finished")
}

#[test]
fn every_row_is_called_as_the_vectors_say() {
    let doc = vectors();
    assert_eq!(doc["label_key"].as_str(), Some(KEY_N_KEY));
    let key_n = key_n_en();
    let first_key = recorded()["publicKey"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let cases = doc["cases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 6, "the vectors went missing");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let device = case["device"].to_string().replace("@recorded", &first_key);
        let rows = walk(&device, case["chains"].as_str().unwrap_or("none"));
        let got: Vec<Value> = rows
            .iter()
            .map(|row| serde_json::json!([row["name"], row["label_key"], row["label_n"]]))
            .collect();
        assert_eq!(Value::Array(got), case["rows"], "{name}");
        // What an English row says: its name, else its label.
        let said: Vec<String> = rows
            .iter()
            .map(|row| match row["name"].as_str().unwrap_or_default() {
                "" => key_n.replace("{{n}}", &row["label_n"].to_string()),
                own => own.to_owned(),
            })
            .collect();
        assert_eq!(serde_json::json!(said), case["say_en"], "{name}");
        // Never blank, never the account's name.
        let account = case["device"]["account_name"].as_str().unwrap_or("").trim();
        for line in &said {
            assert!(!line.trim().is_empty(), "{name}: a blank label");
            assert!(
                account.is_empty() || line != account,
                "{name}: the account's name"
            );
        }
    }
}
