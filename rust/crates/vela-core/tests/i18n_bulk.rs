//! The bulk translation calls (core-perf-1): a shell asks once per screen or
//! once per language, not once per label, and gets what [`I18n::t`] gives.
//!
//! `cargo test -p vela-core --features i18n-all --test i18n_bulk`

use vela_core::i18n::{Catalog, I18n, Options};

fn engine(lang: &str) -> I18n {
    let mut engine = I18n::embedded().expect("embedded en");
    if lang != "en" {
        engine.load_catalog(Catalog::embedded(lang).expect("embedded catalog"));
    }
    engine.change_language(lang);
    engine
}

#[test]
fn t_many_answers_what_t_answers_in_order() {
    let engine = engine("ja");
    let opts = Options::default();
    let keys = ["about.footer", "no.such.key", "about.debugMode"];
    let many = engine.t_many(&keys, &opts).expect("t_many");
    let one: Vec<String> = keys
        .iter()
        .map(|key| engine.t(key, &opts).expect("t"))
        .collect();
    assert_eq!(many, one);
    assert_eq!(many[1], "no.such.key", "a miss is the key, as t says");
}

#[test]
fn a_language_snapshot_is_every_plain_label_as_t_says_it() {
    for lang in ["en", "ja", "de"] {
        let engine = engine(lang);
        let snapshot = engine.plain_strings("");
        assert!(snapshot.len() > 1_000, "{lang}: {} strings", snapshot.len());
        let opts = Options::default();
        for (key, value) in &snapshot {
            assert_eq!(&engine.t(key, &opts).expect("t"), value, "{lang} {key}");
        }
    }
}

#[test]
fn a_snapshot_can_be_taken_for_one_screen() {
    let engine = engine("en");
    let common = engine.plain_strings("about.");
    assert!(!common.is_empty());
    assert!(common.iter().all(|(key, _)| key.starts_with("about.")));
}
