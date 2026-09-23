//! SC-005: only the language in use gets loaded.
//!
//! The claim this file exists to *measure* rather than assert: a cold start in
//! Japanese holds `ja` + `en`, not the whole corpus, and a language switch does not
//! monotonically grow memory.
//!
//! Baseline to beat: `src/i18n/resources.ts` statically imports all 240 files and
//! spreads them into one object, so every user carries **990,499 bytes** of strings
//! regardless of which language they read. SC-005's requirement is that a cold
//! start loads one language plus `en` — see [`SC005_BUDGET`] for what the
//! number below guards now, and what it stopped guarding.
//!
//! Run with `cargo test -p vela-core --features i18n-all --test i18n_residency -- --nocapture`
//! to see the numbers rather than just the pass.

use vela_core::i18n::{Catalog, I18n};

/// The pre-feature cost: every locale resident on every device.
const CORPUS_BYTES: usize = 990_499;
/// The bloat guard for `ja` + `en`.
///
/// What SC-005 REQUIRES is the sentence, not the number: "cold start in any
/// single language loads at most that language plus the `en` fallback". The
/// number is an early warning that the corpus is growing, and it is only worth
/// having while it sits close to today's measurement.
///
/// It was doubled to 270,690 on the owner's word that morning, because three
/// sentences for "remove one wallet from this device" had gone over it. Later
/// the same day the Clear Signer's two cross-device channels were cut and 23
/// strings went with them, which put `ja` + `en` back at 128,800 — under the
/// original figure. A budget at twice the measurement would not fire until the
/// corpus DOUBLED, which is not a warning, so it goes back to where it warns.
const SC005_BUDGET: usize = 135_345;

fn engine_with(active: &str) -> I18n {
    let en = match Catalog::embedded("en") {
        Ok(c) => c,
        Err(e) => unreachable!("i18n-en feature must be on for this test: {e}"),
    };
    let mut engine = match I18n::new(en) {
        Ok(e) => e,
        Err(e) => unreachable!("en catalog must construct: {e}"),
    };
    if active != "en" {
        match Catalog::embedded(active) {
            Ok(c) => {
                engine.load_catalog(c);
            }
            Err(e) => unreachable!("{active} catalog must be compiled in: {e}"),
        }
    }
    engine.change_language(active);
    engine
}

#[test]
fn cold_start_holds_only_the_active_language_and_the_fallback() {
    let engine = engine_with("ja");

    let resident = engine.resident_bytes();
    println!("cold start (ja):");
    println!("  resident         {resident:>9} bytes");
    println!("  SC-005 budget    {SC005_BUDGET:>9} bytes");
    println!("  whole corpus     {CORPUS_BYTES:>9} bytes  (what the app loads today)");
    #[allow(clippy::cast_precision_loss, clippy::allow_attributes)]
    let saved = 100.0 - (resident as f64 / CORPUS_BYTES as f64) * 100.0;
    println!("  reduction        {saved:>8.1}%");

    assert!(
        resident <= SC005_BUDGET,
        "resident {resident} exceeds the SC-005 budget of {SC005_BUDGET}"
    );
    assert!(
        saved >= 86.0,
        "reduction {saved:.1}% is below the >=86% SC-005 claims"
    );
    assert_eq!(engine.resident_locales(), vec!["ja", "en"]);
}

#[test]
fn switching_language_does_not_accumulate_catalogs() {
    let mut engine = engine_with("ja");
    let after_ja = engine.resident_bytes();

    // Loading `de` DISPLACES `ja` — the API hands the dropped catalog back, so the
    // release is observable rather than something the caller has to trust.
    let displaced = match Catalog::embedded("de") {
        Ok(c) => engine.load_catalog(c),
        Err(e) => unreachable!("de must be compiled in: {e}"),
    };
    engine.change_language("de");
    assert!(
        displaced.is_some(),
        "loading a second locale must displace the first"
    );
    assert_eq!(
        displaced.as_ref().map(Catalog::lang),
        Some("ja"),
        "the displaced catalog must be the one that was active"
    );

    let after_de = engine.resident_bytes();
    println!("after switch ja -> de:");
    println!("  ja + en          {after_ja:>9} bytes");
    println!("  de + en          {after_de:>9} bytes");

    assert_eq!(engine.resident_locales(), vec!["de", "en"]);
    assert!(
        !engine.is_resident("ja"),
        "ja must be gone, not merely inactive"
    );
    // The point of SC-005: switching is a REPLACEMENT, never an accumulation.
    assert!(
        after_de <= SC005_BUDGET,
        "residency after a switch ({after_de}) must still fit the budget"
    );
}

#[test]
fn releasing_returns_to_the_fallback_alone() {
    let mut engine = engine_with("ru");
    let with_ru = engine.resident_bytes();

    let released = engine.release_catalog("ru");
    assert!(released.is_some(), "the active catalog must be releasable");
    let en_only = engine.resident_bytes();

    println!("release:");
    println!("  ru + en          {with_ru:>9} bytes");
    println!("  en only          {en_only:>9} bytes");

    assert!(en_only < with_ru, "releasing must actually free bytes");
    assert_eq!(engine.resident_locales(), vec!["en"]);
}

#[test]
fn the_pinned_fallback_cannot_be_released() {
    let mut engine = engine_with("fr");
    // `en` is a field, not a slot, so this is not expressible — it returns None
    // rather than leaving the engine without a fallback. FR-013 is a type-level
    // guarantee here, not a runtime check anyone could forget.
    assert!(engine.release_catalog("en").is_none());
    assert!(engine.is_resident("en"));

    engine.release_catalog("fr");
    assert!(
        engine.is_resident("en"),
        "en survives releasing the active locale"
    );
}

#[test]
fn residency_is_bounded_at_two_under_any_sequence() {
    const SEQUENCE: [&str; 9] = ["ja", "de", "ru", "fr", "zh-TW", "it", "ko", "en", "pt-BR"];
    // The byte bound is the largest PAIR this sequence can legitimately hold —
    // measured, not remembered. It was a flat 140,000, which says nothing about
    // residency and everything about how long the Russian corpus happened to be
    // on the day it was written: Cyrillic is two bytes a letter, `ru` + `en`
    // grew past it with eleven ordinary sentences (issues 204-206), and nothing
    // had leaked. A third catalog left resident WOULD exceed the largest pair,
    // by a whole catalog — which is the failure this loop exists to catch.
    // SC-005's own budget (`ja` + `en`) is asserted, unchanged, in
    // `cold_start_holds_only_the_active_language_and_the_fallback`.
    let largest_pair = SEQUENCE
        .iter()
        .map(|lng| engine_with(lng).resident_bytes())
        .max()
        .unwrap_or(0);

    let mut engine = engine_with("en");
    // Hammer every transition the API allows; residency must never exceed two, and
    // `en` must always be one of them. This is the structural invariant FR-012 and
    // FR-013 encode — there is nowhere for a third catalog to go.
    for lng in SEQUENCE {
        if let Ok(c) = Catalog::embedded(lng) {
            engine.load_catalog(c);
        }
        engine.change_language(lng);
        let resident = engine.resident_locales();
        assert!(
            resident.len() <= 2,
            "{lng}: {} catalogs resident",
            resident.len()
        );
        assert!(resident.contains(&"en"), "{lng}: en is not resident");
        assert!(
            engine.resident_bytes() <= largest_pair,
            "{lng}: residency grew to {} — more than any two catalogs ({largest_pair})",
            engine.resident_bytes()
        );
        engine.release_catalog(lng);
    }
}

/// The runtime-JSON route (FR-015) is what the web build actually uses, so its
/// residency is the number that matters for the browser — and it must land inside
/// the same budget as the compiled-in route.
#[test]
fn runtime_json_catalogs_fit_the_same_budget() {
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/locales");
    // Merge a locale the way the generated asset does, so this reads the corpus
    // rather than a fixture that could drift from it.
    let read = |lng: &str| -> String {
        let mut merged = serde_json::Map::new();
        let core = std::fs::read_to_string(repo.join(format!("{lng}.json")))
            .unwrap_or_else(|e| unreachable!("corpus must be readable: {e}"));
        if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(&core) {
            merged.extend(m);
        }
        for ns in [
            "home",
            "send",
            "receive",
            "assets",
            "addToken",
            "tokenDetail",
            "history",
            "onboarding",
            "connect",
            "about",
            "clearSigning",
            "componentsTx",
            "componentsUi",
            "settingsModals",
            "contacts",
            "explore",
        ] {
            let raw = std::fs::read_to_string(repo.join(lng).join(format!("{ns}.json")))
                .unwrap_or_else(|e| unreachable!("corpus must be readable: {e}"));
            if let Ok(serde_json::Value::Object(m)) = serde_json::from_str(&raw) {
                merged.extend(m);
            }
        }
        serde_json::Value::Object(merged).to_string()
    };

    let en = match Catalog::from_json("en", read("en").as_bytes()) {
        Ok(c) => c,
        Err(e) => unreachable!("en must parse: {e}"),
    };
    let mut engine = match I18n::new(en) {
        Ok(e) => e,
        Err(e) => unreachable!("{e}"),
    };
    match Catalog::from_json("ja", read("ja").as_bytes()) {
        Ok(c) => {
            engine.load_catalog(c);
        }
        Err(e) => unreachable!("ja must parse: {e}"),
    }
    engine.change_language("ja");

    let resident = engine.resident_bytes();
    println!("runtime-JSON route (what the web build uses):");
    println!("  ja + en          {resident:>9} bytes");
    assert!(
        resident <= SC005_BUDGET,
        "runtime residency {resident} exceeds the SC-005 budget of {SC005_BUDGET}"
    );

    // And it must actually resolve — a small catalog that returns nothing would
    // pass every byte assertion above.
    let opts = vela_core::i18n::Options::default();
    let got = engine.t("common.cancel", &opts);
    assert_eq!(
        got.as_deref(),
        Ok("キャンセル"),
        "runtime catalog must resolve"
    );
}
