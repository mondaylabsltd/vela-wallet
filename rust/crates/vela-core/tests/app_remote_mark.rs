//! Logo misses (spec 082 T035, RE10): a missing, refused or broken image is
//! remembered for the session; a throttled, failing or unreached one for a
//! minute. Every status class is pinned.
//!
//! Which logo a mark wears (DESIGN L, 2026-10-08): the one rule the four
//! shells used to copy. `tests/vectors/marks.json` pins the same rule for
//! every binding surface (`conformance.rs`, verify-web, smoke-kotlin,
//! smoke-swift); these tests pin the reasons.

#![cfg(feature = "crux")]

use vela_core::app::network_admin::{BUILTIN_CHAINS, DEFAULT_ETHEREUM_DATA_URL};
use vela_core::app::remote_mark::{
    chain_logo_url, chain_mark, mark_miss_of_status, mark_miss_ttl_ms, native_coin_logo_chain_id,
    token_mark, MarkMiss, MarkView, MARK_MISS_TRANSIENT_MS,
};

#[test]
fn every_status_class_maps_to_its_miss() {
    for (status, want) in [
        (404, MarkMiss::NotFound),
        (410, MarkMiss::NotFound),
        (401, MarkMiss::Refused),
        (403, MarkMiss::Refused),
        (200, MarkMiss::NotAnImage),
        (204, MarkMiss::NotAnImage),
        (429, MarkMiss::Throttled),
        (408, MarkMiss::ServerError),
        (500, MarkMiss::ServerError),
        (502, MarkMiss::ServerError),
        (503, MarkMiss::ServerError),
        (599, MarkMiss::ServerError),
        (0, MarkMiss::Unknown),
        (100, MarkMiss::Unknown),
        (301, MarkMiss::Unknown),
        (400, MarkMiss::Unknown),
        (451, MarkMiss::Unknown),
        (600, MarkMiss::Unknown),
    ] {
        assert_eq!(mark_miss_of_status(status), want, "{status}");
    }
}

/// What asking again will not fix is remembered for the session.
#[test]
fn a_missing_refused_or_broken_image_is_remembered_for_the_session() {
    for miss in [MarkMiss::NotFound, MarkMiss::Refused, MarkMiss::NotAnImage] {
        assert_eq!(mark_miss_ttl_ms(miss), None, "{miss:?}");
    }
}

/// W20: a logo lost to a bad minute comes back after one.
#[test]
fn a_miss_that_may_heal_is_remembered_for_a_minute() {
    assert_eq!(MARK_MISS_TRANSIENT_MS, 60_000);
    for miss in [
        MarkMiss::Throttled,
        MarkMiss::ServerError,
        MarkMiss::Transport,
        MarkMiss::Unknown,
    ] {
        assert_eq!(mark_miss_ttl_ms(miss), Some(60_000), "{miss:?}");
    }
    // From the status straight to the wait.
    assert_eq!(mark_miss_ttl_ms(mark_miss_of_status(503)), Some(60_000));
    assert_eq!(mark_miss_ttl_ms(mark_miss_of_status(404)), None);
}

/// The kinds cross the FFI as snake_case words.
#[test]
fn the_kinds_travel_as_snake_case_words() {
    for (miss, word) in [
        (MarkMiss::NotFound, "not_found"),
        (MarkMiss::Refused, "refused"),
        (MarkMiss::NotAnImage, "not_an_image"),
        (MarkMiss::Throttled, "throttled"),
        (MarkMiss::ServerError, "server_error"),
        (MarkMiss::Transport, "transport"),
        (MarkMiss::Unknown, "unknown"),
    ] {
        assert_eq!(
            serde_json::to_value(miss).unwrap_or_default(),
            serde_json::Value::String(word.to_owned())
        );
        assert_eq!(
            serde_json::from_value::<MarkMiss>(serde_json::Value::String(word.to_owned())).ok(),
            Some(miss)
        );
    }
}

// ---------------------------------------------------------------------------
// Which logo
// ---------------------------------------------------------------------------

const BASE: &str = "https://data.example";

fn chain_logo(id: u32) -> String {
    format!("{BASE}/chainlogos/eip155-{id}.png")
}

/// Rule 2: a native coin is its coin's chain, wherever it sits; anything else
/// is the chain it is on. The symbol's case does not matter.
#[test]
fn a_native_coin_wears_its_home_chains_logo() {
    for (symbol, home) in [
        ("ETH", 1),
        ("BNB", 56),
        ("POL", 137),
        ("MATIC", 137),
        ("AVAX", 43114),
        ("XDAI", 100),
        ("xDai", 100),
        ("eth", 1),
    ] {
        assert_eq!(native_coin_logo_chain_id(symbol, 8453), home, "{symbol}");
    }
    for symbol in ["OKB", "pathUSD", "CELO", "S", "MNT", ""] {
        assert_eq!(native_coin_logo_chain_id(symbol, 4217), 4217, "{symbol}");
    }
}

/// Rule 3: the badge is gone exactly when it would repeat the coin.
#[test]
fn the_badge_hides_only_on_a_native_coins_home_chain() {
    let at_home = token_mark(BASE, 100, "XDAI", None, &[]);
    assert_eq!(at_home.logo_urls, vec![chain_logo(100)]);
    assert_eq!(at_home.badge_chain_id, None);
    assert_eq!(at_home.badge_logo_url, None);

    let away = token_mark(BASE, 42161, "ETH", None, &[]);
    assert_eq!(away.logo_urls, vec![chain_logo(1)]);
    assert_eq!(away.badge_chain_id, Some(42161));
    assert_eq!(away.badge_logo_url, Some(chain_logo(42161)));

    // A contract token always names its chain, even on Ethereum, even when
    // its ticker reads like a native coin's.
    let token = token_mark(BASE, 1, "ETH", Some(&format!("0x{}", "a".repeat(40))), &[]);
    assert_eq!(token.badge_chain_id, Some(1));
    assert!(token
        .logo_urls
        .iter()
        .all(|url| url.contains("/assets/eip155-1/")));
}

/// Every chain Vela ships: its native coin on it wears one picture, no badge,
/// except where the coin's home is elsewhere (ETH on an L2, BNB on opBNB).
#[test]
fn every_shipped_chains_native_coin_has_a_logo_and_a_truthful_badge() {
    for chain in BUILTIN_CHAINS {
        let id = chain.chain_id;
        let mark = token_mark(BASE, id, chain.native_symbol, None, &[]);
        let home = native_coin_logo_chain_id(chain.native_symbol, id);
        assert_eq!(
            mark.logo_urls,
            vec![chain_logo(home)],
            "{}",
            chain.display_name
        );
        if home == id {
            assert_eq!(mark.badge_chain_id, None, "{}", chain.display_name);
        } else {
            assert_eq!(mark.badge_chain_id, Some(id), "{}", chain.display_name);
            assert_eq!(mark.badge_logo_url, Some(chain_logo(id)));
        }
    }
}

/// Rule 4: checksummed first, lowercase second, whatever case came in; one
/// path when the two spellings are the same (an address of digits only).
#[test]
fn an_erc20_asks_the_checksummed_then_the_lowercase_path() {
    let mark = token_mark(
        BASE,
        1,
        "USDC",
        Some("0xA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48"),
        &[],
    );
    assert_eq!(
        mark.logo_urls,
        vec![
            format!("{BASE}/assets/eip155-1/0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48/logo.png"),
            format!("{BASE}/assets/eip155-1/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48/logo.png"),
        ]
    );
    assert_eq!(mark.glyph, "USD");

    let digits = "0x0000000000000000000000000000000000001010";
    assert_eq!(
        token_mark(BASE, 137, "POL", Some(digits), &[]).logo_urls,
        vec![format!("{BASE}/assets/eip155-137/{digits}/logo.png")]
    );
}

/// What this rule cannot place gets the glyph, never a guessed logo: a
/// malformed or empty address is not the native coin.
#[test]
fn a_malformed_address_draws_the_glyph_not_a_guess() {
    for address in [
        "",
        "0x1234",
        "not-an-address",
        "0XA0B86991C6218B36C1D19D4A2E9EB0CE3606EB48",
        " 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
        "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb4g",
    ] {
        let mark = token_mark(BASE, 1, "ETH", Some(address), &[]);
        assert!(mark.logo_urls.is_empty(), "{address:?}");
        assert_eq!(mark.badge_chain_id, Some(1), "{address:?}");
        assert_eq!(mark.glyph, "ETH");
    }
}

/// URLs an index named win, once each; blanks never become a candidate.
#[test]
fn named_urls_come_first_once_each() {
    let checksummed =
        format!("{BASE}/assets/eip155-1/0xdAC17F958D2ee523a2206206994597C13D831ec7/logo.png");
    let named = vec![
        "https://cdn.example/usdt.png".to_owned(),
        String::new(),
        " https://cdn.example/usdt.png ".to_owned(),
        "   ".to_owned(),
        checksummed.clone(),
    ];
    let mark = token_mark(
        BASE,
        1,
        "USDT",
        Some("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
        &named,
    );
    assert_eq!(
        mark.logo_urls,
        vec![
            "https://cdn.example/usdt.png".to_owned(),
            checksummed,
            format!("{BASE}/assets/eip155-1/0xdac17f958d2ee523a2206206994597c13d831ec7/logo.png"),
        ]
    );
}

/// The endpoint is the person's, read as given; an empty setting is the
/// built-in endpoint, and trailing slashes never double up.
#[test]
fn the_endpoint_is_the_persons_or_the_built_in_one() {
    assert_eq!(
        chain_logo_url("", 56),
        Some(format!(
            "{DEFAULT_ETHEREUM_DATA_URL}/chainlogos/eip155-56.png"
        ))
    );
    assert_eq!(chain_logo_url("  ", 56), chain_logo_url("", 56));
    assert_eq!(
        chain_logo_url("https://data.example//", 56),
        Some(chain_logo(56))
    );
    assert_eq!(
        token_mark("", 8453, "ETH", None, &[]).badge_logo_url,
        Some(format!(
            "{DEFAULT_ETHEREUM_DATA_URL}/chainlogos/eip155-8453.png"
        ))
    );
}

/// Chain 0 is no network: nothing is ever asked of `eip155-0`.
#[test]
fn chain_zero_asks_for_nothing() {
    assert_eq!(chain_logo_url(BASE, 0), None);
    let usdc = token_mark(
        BASE,
        0,
        "USDC",
        Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        &[],
    );
    assert!(usdc.logo_urls.is_empty());
    assert_eq!(usdc.badge_logo_url, None);
    assert!(chain_mark(BASE, 0, "ETH").logo_urls.is_empty());
    let all = [
        usdc,
        token_mark(BASE, 0, "ETH", None, &[]),
        chain_mark(BASE, 0, ""),
    ];
    for mark in all {
        assert!(mark.logo_urls.iter().all(|url| !url.contains("eip155-0")));
    }
}

/// The kind rule: a network is drawn as itself. ETH sent on Base puts the
/// Ethereum logo on the COIN and the Base logo on the NETWORK row.
#[test]
fn a_network_wears_its_own_logo_and_never_a_badge() {
    let network = chain_mark(BASE, 8453, "ETH");
    assert_eq!(
        network,
        MarkView {
            glyph: "ETH".to_owned(),
            logo_urls: vec![chain_logo(8453)],
            badge_chain_id: None,
            badge_logo_url: None,
        }
    );
    assert_ne!(
        network.logo_urls,
        token_mark(BASE, 8453, "ETH", None, &[]).logo_urls
    );
    for chain in BUILTIN_CHAINS {
        let mark = chain_mark(BASE, chain.chain_id, chain.native_symbol);
        assert_eq!(mark.logo_urls, vec![chain_logo(chain.chain_id)]);
        assert_eq!(mark.badge_chain_id, None);
        assert_eq!(mark.badge_logo_url, None);
    }
}

/// The glyph every shell drew by hand: three characters, upper-cased.
#[test]
fn the_glyph_is_the_tickers_first_three_characters_upper_cased() {
    for (ticker, glyph) in [
        ("ETH", "ETH"),
        ("USDC", "USD"),
        ("pathUSD", "PAT"),
        ("xDAI", "XDA"),
        ("op", "OP"),
        ("", ""),
        ("USD₮0", "USD"),
        ("₮ab", "₮AB"),
    ] {
        assert_eq!(
            token_mark(BASE, 1, ticker, Some("0x1"), &[]).glyph,
            glyph,
            "{ticker}"
        );
        assert_eq!(chain_mark(BASE, 1, ticker).glyph, glyph, "{ticker}");
    }
}

/// The shape the shells decode: snake_case fields, `null` for no badge.
#[test]
fn a_mark_travels_as_snake_case_json() {
    let json = serde_json::to_value(token_mark(BASE, 1, "ETH", None, &[])).unwrap_or_default();
    assert_eq!(
        json,
        serde_json::json!({
            "glyph": "ETH",
            "logo_urls": [chain_logo(1)],
            "badge_chain_id": null,
            "badge_logo_url": null,
        })
    );
}
