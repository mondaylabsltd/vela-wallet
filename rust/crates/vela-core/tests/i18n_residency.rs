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
/// the same day the Trusted Signer's two cross-device channels were cut and 23
/// strings went with them, which put `ja` + `en` back at 128,800 — under the
/// original figure. A budget at twice the measurement would not fire until the
/// corpus DOUBLED, which is not a warning, so it goes back to where it warns.
///
/// 2026-09-27 (078): the in-app report learned to carry screenshots — ten new
/// sentences and four rewritten ones, +1,783 bytes of `ja` + `en` JSON — and the
/// runtime route measured 136,702, over a line it had cleared by 426 bytes. The
/// words are what the founder asked for, so the line moves to just above the new
/// measurement (~1 KB of room), which keeps it a warning rather than a ceiling
/// nobody hears. Moving it is the owner's call; this is the reviewed proposal.
///
/// 2026-09-27 (078, later): the signing sheet's descriptor words — "Approve",
/// "Amount", "Spender", "Unlimited" and the rest — were English on a Chinese
/// sheet; the founder asked for them translated. 29 new words (19 more reused
/// from the old app's keys) put the runtime route at 137,801, 101 bytes over.
/// Same rule as the morning's move: the words were asked for, so the line goes
/// to ~1 KB above the new measurement and stays a warning. Approved by the
/// owner the same day: 「同意呀，i18n_residency 可以增加预算」.
///
/// 2026-10-02: the owner raised it to 140,800 for the beta-readiness
/// branches, which each make this identical edit; later the same day, to
/// 141,800 (096's order-terms warning, "All" and "Order" among them).
///
/// 2026-10-03 (098): the founder asked for the wallet to stop sending silently into a relay that
/// cannot serve the chain, and for a funding sheet that resumes by itself. The
/// "relay can't reach this network" sheet (7 strings) and the sheet's balance,
/// network, scan and watching lines (4) put the runtime route at 143,429 after
/// `en` and `ja` were tightened. Same rule: ~1 KB above the measurement.
/// Approved by the owner the same day, with the reduction claim below:
/// 「i18n 体积预算上调 统一」.
///
/// 2026-10-04 (099): the dApp browser's layers in words — the tab status,
/// each request's reason, why the slide is shut, "the relay is sending it" —
/// 51 strings in every locale, which fit at 144,350 only after `en` and `ja`
/// were cut to the bone and one line dropped, with 50 bytes left. Same rule:
/// ~1 KB above the measurement. Approved by the owner: 「Text budget 提高预算」.
///
/// 2026-10-20 (103): Settings → AI agent — 17 strings, `en` and `ja` already
/// tightened — put the runtime route at 145,819, 419 bytes over, and more of
/// the agent's words are coming (the in-wallet approval, the agent's status).
/// The owner raised the line by half rather than by ~1 KB per step, knowing it
/// warns later for it: 「预算可以加50%」 (145,400 × 1.5).
///
/// 2026-10-29 (round-4 multisig, seven branches merged): member changes read
/// in full, token provenance, executions watched, the fee before the passkey,
/// Settings' member changes, invites and presence, the copy pass and the
/// connect page's outside owners — 290 leaves in every locale — put the
/// runtime route at 246,278, 28,178 over; each branch alone fitted. A raise
/// to 247,300 was proposed and never approved.
///
/// 2026-10-29 (round 5): instead, the words that were over moved out of what
/// every reader holds. `home.multisig` — the multisig screens and the connect
/// page an outside owner signs on, 527 leaves, about a third of `ja` + `en` —
/// is drawn only by the web app, which resolves it at build time; no native
/// shell reads one of its keys. It is now an on-demand namespace
/// (`scripts/gen-i18n.mjs` `ON_DEMAND`): out of the compiled tables and the
/// main runtime catalog, its own `assets/i18n/multisig/<lng>.json`, loaded by
/// the screens that draw it. With it out the runtime route measured 179,075
/// (compiled in: 174,117), so the line goes back to the owner's last approved
/// figure, 218,100, with ~39 KB of headroom — and it warns again when the
/// corpus everyone carries grows, not when a feature some never open does.
const SC005_BUDGET: usize = 218_100;

/// The cold-start reduction SC-005 claims, against the July baseline above.
///
/// It was 86%; every word added since July has eaten into it, and 098's put the
/// measurement at 85.94%. Moved with the budget, the same way and approved with
/// it: ~1.4 KB of room above the measurement. The sentence SC-005 requires — one language plus
/// `en` — is asserted separately and does not move.
///
/// 2026-10-20 (103): moved with the budget again, to the reduction the new
/// budget itself allows (100 − 218,100 / 990,499 ≈ 77.98%), so the two checks
/// guard one line rather than the percentage firing first.
///
/// 2026-10-29 (round 5): back with the budget to the reduction it allows
/// (100 − 218,100 / 990,499 ≈ 77.98%).
const SC005_REDUCTION_PERCENT: f64 = 77.9;

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
        saved >= SC005_REDUCTION_PERCENT,
        "reduction {saved:.2}% is below the >={SC005_REDUCTION_PERCENT}% SC-005 claims"
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

/// The corpus subtrees that load on demand (`scripts/gen-i18n.mjs`'s
/// `ON_DEMAND`): not in the main runtime catalog, not compiled in.
const ON_DEMAND: [&[&str]; 1] = [&["home", "multisig"]];

fn prune(map: &mut serde_json::Map<String, serde_json::Value>, path: &[&str]) {
    match path {
        [] => {}
        [last] => {
            map.remove(*last);
        }
        [first, rest @ ..] => {
            if let Some(serde_json::Value::Object(child)) = map.get_mut(*first) {
                prune(child, rest);
            }
        }
    }
}

/// The generated catalogs agree with the split: the main one carries none of
/// an on-demand namespace, and that namespace's own catalog carries it, for
/// every locale the residency is measured in.
#[test]
fn on_demand_namespaces_are_out_of_the_main_catalog_and_in_their_own() {
    let assets = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../assets/i18n");
    let read = |p: std::path::PathBuf| -> serde_json::Value {
        let raw = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| unreachable!("{} must be readable: {e}", p.display()));
        serde_json::from_str(&raw).unwrap_or_else(|e| unreachable!("{}: {e}", p.display()))
    };
    for lng in ["en", "ja", "de", "ru"] {
        let main = read(assets.join(format!("{lng}.json")));
        let own = read(assets.join("multisig").join(format!("{lng}.json")));
        for path in ON_DEMAND {
            let at =
                |v: &serde_json::Value| path.iter().try_fold(v.clone(), |v, k| v.get(*k).cloned());
            assert!(
                at(&main).is_none(),
                "{lng}: {} is still in the main catalog",
                path.join(".")
            );
            assert!(
                at(&own).is_some_and(|v| v.as_object().is_some_and(|o| !o.is_empty())),
                "{lng}: the on-demand catalog has no {}",
                path.join(".")
            );
        }
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
        // The on-demand namespaces leave the main catalog (`ON_DEMAND` in
        // `scripts/gen-i18n.mjs`): what every reader holds is the rest.
        for path in ON_DEMAND {
            prune(&mut merged, path);
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
