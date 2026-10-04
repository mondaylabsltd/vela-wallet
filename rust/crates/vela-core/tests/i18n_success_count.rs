//! Issue #409: the line under "Wallet created" agrees with its count.
//!
//! It read "Any of your 1 keys can sign in on its own." for a one-key wallet —
//! a single `{{count}}` sentence written for several keys. The key is now
//! plural (`successMessage_one` / `_few` / `_many` / `_other`), and the form is
//! chosen HERE, by the language's CLDR rule, for every shell that passes the
//! count. These tests pin what each shell then draws, for every founding-set
//! size a wallet can have (1..=7), in all fifteen languages.
//!
//! Run with `--features i18n-all` (the workspace test command does).

use vela_core::i18n::{plural_category, Catalog, Category, Count, I18n, Options, Var, SUPPORTED};
use vela_core::safe::MAX_MULTI_KEYS;

const KEY: &str = "onboarding.create.successMessage";

fn engine(lang: &str) -> I18n {
    let en = match Catalog::embedded("en") {
        Ok(c) => c,
        Err(e) => unreachable!("i18n-all must be on for this test: {e}"),
    };
    let mut engine = match I18n::new(en) {
        Ok(e) => e,
        Err(e) => unreachable!("en catalog must construct: {e}"),
    };
    if lang != "en" {
        match Catalog::embedded(lang) {
            Ok(c) => {
                engine.load_catalog(c);
            }
            Err(e) => unreachable!("{lang} must be compiled in: {e}"),
        }
    }
    engine.change_language(lang);
    engine
}

/// What a shell draws for a wallet of `keys` founding keys.
fn line(engine: &I18n, key: &str, keys: usize) -> String {
    #[allow(clippy::cast_precision_loss, clippy::allow_attributes)]
    let opts = Options {
        count: Some(Count::Num(keys as f64)),
        ..Options::default()
    };
    match engine.t(key, &opts) {
        Ok(text) => text,
        Err(e) => unreachable!("t() is total for a numeric count: {e}"),
    }
}

#[test]
fn english_says_your_key_for_one_and_any_of_n_for_several() {
    let en = engine("en");
    assert_eq!(
        line(&en, KEY, 1),
        "Your key can sign in on its own. The contract deploys with your first transaction."
    );
    assert_eq!(
        line(&en, KEY, 2),
        "Any of your 2 keys can sign in on its own. The contract deploys with your first transaction."
    );
    assert_eq!(
        line(&en, KEY, 7),
        "Any of your 7 keys can sign in on its own. The contract deploys with your first transaction."
    );
}

/// Every language, every founding-set size: a sentence of its own (never the
/// key, never a placeholder, never English standing in), and — where the
/// language HAS a singular — a one-key sentence that is not the several-keys
/// sentence with a 1 written into it, which was the bug.
#[test]
fn every_language_has_a_sentence_for_every_wallet_size() {
    let en = engine("en");
    for lang in SUPPORTED {
        let engine = engine(lang);
        for keys in 1..=MAX_MULTI_KEYS {
            let text = line(&engine, KEY, keys);
            assert_ne!(text, KEY, "{lang}/{keys}: the key echoed");
            assert!(!text.contains("{{"), "{lang}/{keys}: unfilled: {text}");
            if lang != "en" {
                assert_ne!(
                    text,
                    line(&en, KEY, keys),
                    "{lang}/{keys}: fell back to English"
                );
            }
        }

        let one = line(&engine, KEY, 1);
        let has_singular = plural_category(lang, 1.0) == Category::One;
        let several_with_a_one = line(&engine, &format!("{KEY}_other"), 1);
        if has_singular {
            assert_ne!(
                one, several_with_a_one,
                "{lang}: one key reads as the several-keys sentence with a 1 in it"
            );
        } else {
            // zh, zh-TW, zh-HK, ja, ko, vi, id: CLDR gives these ONE form for
            // every count, so the one sentence must read right at one key and
            // at seven — it cannot carry the number ("1 把密钥都能…" was this
            // same bug in Chinese).
            for keys in 2..=MAX_MULTI_KEYS {
                assert_eq!(
                    line(&engine, KEY, keys),
                    one,
                    "{lang}: a language with no singular must not say how many"
                );
            }
        }
    }
}

/// Russian has four forms, and 2–4 is `few`: a shell that picked
/// `count == 1 ? _one : _other` itself would be choosing a different form from
/// the one CLDR does. Through the core, each count lands on its own category.
#[test]
fn russian_counts_land_on_their_own_categories() {
    let ru = engine("ru");
    assert_eq!(plural_category("ru", 2.0), Category::Few);
    assert_eq!(plural_category("ru", 5.0), Category::Many);
    assert!(line(&ru, KEY, 1).starts_with("Ваш ключ"));
    assert!(line(&ru, KEY, 3).contains("из 3 ключей"));
    assert!(line(&ru, KEY, 6).contains("из 6 ключей"));
}

/// A shell that interpolates `{{count}}` WITHOUT passing it as the plural
/// count — a text variable, as Android and iOS did — gets no form at all: the
/// base key has no value of its own any more. This is the failure each shell
/// had to be moved off, pinned so it stays loud rather than quietly English.
#[test]
fn a_count_passed_as_text_selects_no_form() {
    let en = engine("en");
    let vars = [("count", Var::Str("1"))];
    let opts = Options {
        vars: &vars,
        ..Options::default()
    };
    assert_eq!(en.t(KEY, &opts).ok().as_deref(), Some(KEY));
}
