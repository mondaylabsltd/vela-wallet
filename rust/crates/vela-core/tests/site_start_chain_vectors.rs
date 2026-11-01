//! dapp-17, r9, r10 — where a new site starts. The shared vectors
//! (`tests/vectors/site-start-chain.json`) are answered by the core's rule
//! here and by the extension worker's twin (`extension/lib/protocol.js`,
//! replayed in `src/lib/dapp/protocol.test.ts`): the worker cannot run the
//! core, so the two are held to one file and cannot drift.

#![cfg(feature = "crux")]

use serde_json::Value;
use vela_core::app::network_space::{
    site_may_switch_to, site_start_chain_for, site_switch_refused, sites_moved_by_space_switch,
    switch_refusal_message, switch_refusal_status_key, NetworkSpace, SiteChain, SwitchRefused,
};

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

/// Spec 105's line (coordinator, r10): a page moves a site only to a network
/// of the space in front — the core's rule, as the in-app browser asks it.
#[test]
fn every_switch_vector_holds_the_space() {
    let doc = vectors();
    let switches = doc["switches"].as_array().cloned().unwrap_or_default();
    assert!(switches.len() >= 4, "the switch vectors went missing");
    for case in switches {
        let name = case["name"].as_str().unwrap_or("?");
        let wanted = chain(&case["chain"]).unwrap_or_default();
        let space = chains(&case["space_chain_ids"]);
        let allowed = site_may_switch_to(&space, wanted);
        assert_eq!(Some(allowed), case["allowed"].as_bool(), "{name}");
        // r10 follow-up: why not, in the page's sentence and the status
        // line's words — the extension worker's `switchRefusal` says the
        // same sentence (`protocol.test.ts`).
        let refused = site_switch_refused(&space, &chains(&case["known"]), wanted);
        let expected = match case["refused"].as_str() {
            None => None,
            Some("otherSpace") => Some(SwitchRefused::OtherSpace),
            Some("unknown") => Some(SwitchRefused::Unknown),
            Some(other) => unreachable!("{name}: refused {other}"),
        };
        assert_eq!(refused, expected, "{name}");
        if let Some(refused) = refused {
            let front = NetworkSpace::parse(case["space"].as_str());
            assert_eq!(
                Some(switch_refusal_message(refused, front, wanted).as_str()),
                case["message"].as_str(),
                "{name}"
            );
            assert_eq!(
                Some(switch_refusal_status_key(refused, front)),
                case["status_key"].as_str(),
                "{name}"
            );
        }
    }
}

fn sites(value: &Value) -> Vec<SiteChain> {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| unreachable!("sites: {e}"))
}

/// r10 follow-up (the owner's spec-105 line): what a space switch does to
/// the sites — the core's rule, as the in-app browser runs it; the worker's
/// `spaceSwitchMoves` replays the same cases.
#[test]
fn every_space_switch_vector_moves_what_it_says() {
    let doc = vectors();
    let cases = doc["space_switches"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(cases.len() >= 6, "the space-switch vectors went missing");
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?");
        let moves = sites_moved_by_space_switch(
            &chains(&case["space_chain_ids"]),
            chain(&case["last_used"]),
            &chains(&case["account_chains"]),
            &sites(&case["sites"]),
        );
        assert_eq!(moves, sites(&case["moves"]), "{name}");
    }
}

/// The page's sentence IS the corpus's English — what the extension's copy
/// is held to through the vectors above.
#[cfg(feature = "i18n-en")]
#[test]
fn the_refusal_sentences_are_the_corpus_english() {
    use vela_core::app::network_space::{switch_refusal_key, SPACE_MOVED_KEY};
    use vela_core::i18n::{I18n, Options, Var};
    let i18n = I18n::embedded().unwrap_or_else(|e| unreachable!("{e:?}"));
    let vars = [("chainId", Var::Str("5042002"))];
    let opts = Options {
        vars: &vars,
        ..Options::default()
    };
    for key in [
        SPACE_MOVED_KEY,
        "componentsUi.browserStatus.reason.otherSpaceTestnet",
        "componentsUi.browserStatus.reason.otherSpaceMainnet",
    ] {
        assert!(i18n.exists(key, &Options::default()), "{key}");
    }
    for (refused, front) in [
        (SwitchRefused::Unknown, NetworkSpace::Mainnet),
        (SwitchRefused::OtherSpace, NetworkSpace::Mainnet),
        (SwitchRefused::OtherSpace, NetworkSpace::Testnet),
    ] {
        assert!(i18n.exists(
            switch_refusal_status_key(refused, front),
            &Options::default()
        ));
        let words = i18n
            .t(switch_refusal_key(refused, front), &opts)
            .unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(
            words,
            switch_refusal_message(refused, front, 5_042_002),
            "{refused:?} {front:?}"
        );
    }
}
