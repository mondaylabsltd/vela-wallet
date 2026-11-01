//! A passkey prompt's failure, said in the person's language (round 10,
//! real-device S3) — `vectors/rules/passkey-errors.json`, replayed through the
//! core's rule; the shells replay the same file through their exports.

use serde_json::Value;
use vela_core::app::passkey_error::{
    passkey_error_words, passkey_error_words_json, PasskeyErrorWords,
};

const VECTORS: &str = include_str!("vectors/rules/passkey-errors.json");

#[test]
fn every_platform_error_is_said_by_a_corpus_key() {
    let root: Value = serde_json::from_str(VECTORS).expect("vectors parse");
    let cases = root["cases"].as_array().expect("cases");
    assert!(cases.len() >= 30, "the corpus covers every platform");
    for case in cases {
        let name = case["name"].as_str().expect("name");
        let text = |key: &str| case[key].as_str().expect(key).to_owned();
        let code = case["code"]
            .as_i64()
            .map(|c| i32::try_from(c).expect("i32"));
        let words = passkey_error_words(&text("platform"), &text("domain"), code, &text("message"));
        let got = serde_json::to_value(&words).expect("serializes");
        assert_eq!(got, case["expect"], "{name}");

        // The JSON door answers the same.
        let facts = serde_json::json!({
            "platform": case["platform"],
            "domain": case["domain"],
            "code": case["code"],
            "message": case["message"],
        });
        let via_json: PasskeyErrorWords =
            serde_json::from_str(&passkey_error_words_json(&facts.to_string())).expect("words");
        assert_eq!(via_json, words, "{name}: json");
    }
}

/// The words are never the platform's: every key resolves in every locale.
#[cfg(feature = "i18n-all")]
#[test]
fn every_key_is_in_every_locale() {
    let root: Value = serde_json::from_str(VECTORS).expect("vectors parse");
    let mut keys: Vec<String> = root["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .map(|case| case["expect"]["key"].as_str().expect("key").to_owned())
        .collect();
    keys.sort();
    keys.dedup();
    for locale in [
        "en", "de", "es-MX", "fr", "id", "it", "ja", "ko", "pt-BR", "ru", "tr", "vi", "zh",
        "zh-HK", "zh-TW",
    ] {
        let raw = std::fs::read_to_string(format!(
            "{}/i18n/locales/{locale}/onboarding.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("locale file");
        let doc: Value = serde_json::from_str(&raw).expect("locale json");
        for key in &keys {
            let mut at = &doc;
            for part in key.split('.') {
                at = &at[part];
            }
            assert!(
                at.as_str().is_some_and(|s| !s.trim().is_empty()),
                "{locale}: {key}"
            );
        }
    }
}
