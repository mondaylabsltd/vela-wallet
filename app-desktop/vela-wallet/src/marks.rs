//! Which logo a token or a chain wears (the founder's rulings of 2026-09-05
//! and 2026-09-12, ported from the web's `flows/marks.ts` and Android's
//! `Marks.kt`).
//!
//! Four rules, and the desktop had none of them — it drew the three-letter
//! glyph everywhere, so one network could not be told from another at a
//! glance (issue 201):
//!
//! 1. Logos come from the chain-data endpoint, with the drawn glyph as the
//!    whole fallback — never a blank circle.
//! 2. A native coin wears its OWN chain's logo: ETH on Base is still
//!    Ethereum's.
//! 3. The chain badge is hidden when it would repeat the token (ETH on
//!    Ethereum, XDAI on Gnosis).
//! 4. A token's logo is `assets/eip155-<chain>/<checksummed>/logo.png`, with
//!    the lowercase path as the second candidate.
use gpui::SharedString;

use crate::executor::chain_tokens::data_base;

/// The endpoint's logo for one chain.
pub fn chain_logo_url(chain_id: u32) -> Option<SharedString> {
    let root = data_base();
    let root = root.trim_end_matches('/');
    if root.is_empty() {
        return None;
    }
    Some(SharedString::from(format!(
        "{root}/chainlogos/eip155-{chain_id}.png"
    )))
}

/// Rule 2 — which chain's logo a native coin wears.
fn native_coin_chain_id(symbol: &str, fallback: u32) -> u32 {
    match symbol.to_uppercase().as_str() {
        "ETH" => 1,
        "BNB" => 56,
        "POL" | "MATIC" => 137,
        "AVAX" => 43114,
        "XDAI" => 100,
        _ => fallback,
    }
}

/// Rule 3 — the badge's chain, or `None` when it would repeat the token.
fn badge_chain_id(chain_id: u32, symbol: &str, token_address: Option<&str>) -> Option<u32> {
    if token_address.is_none() && native_coin_chain_id(symbol, chain_id) == chain_id {
        None
    } else {
        Some(chain_id)
    }
}

/// Rule 4 — a token's own logo candidates, in order.
fn token_logo_urls(chain_id: u32, symbol: &str, token_address: Option<&str>) -> Vec<SharedString> {
    let Some(address) = token_address else {
        return chain_logo_url(native_coin_chain_id(symbol, chain_id))
            .into_iter()
            .collect();
    };
    let root = data_base();
    let root = root.trim_end_matches('/').to_owned();
    if root.is_empty() || !is_address(address) {
        return Vec::new();
    }
    let lower = address.to_lowercase();
    let checksummed =
        vela_core::primitives::checksum_address(&lower).unwrap_or_else(|_| lower.clone());
    let mut urls = vec![SharedString::from(format!(
        "{root}/assets/eip155-{chain_id}/{checksummed}/logo.png"
    ))];
    if lower != checksummed {
        urls.push(SharedString::from(format!(
            "{root}/assets/eip155-{chain_id}/{lower}/logo.png"
        )));
    }
    urls
}

fn is_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A token's logo candidates and its badge — the shape every mark carries.
///
/// `Default` is "no logos": the drawn glyph and the coloured dot, which is
/// what the fixtures (and any surface the endpoint cannot price) show.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Logos {
    pub logo_urls: Vec<SharedString>,
    pub badge_logo: Option<SharedString>,
    pub badge_hidden: bool,
}

/// The rules applied: named URLs the API already gave first, then ours.
pub fn token_logos(
    chain_id: u32,
    symbol: &str,
    token_address: Option<&str>,
    named: &[String],
) -> Logos {
    let badge = badge_chain_id(chain_id, symbol, token_address);
    let mut urls: Vec<SharedString> = named
        .iter()
        .filter(|url| !url.is_empty())
        .map(|url| SharedString::from(url.clone()))
        .collect();
    for url in token_logo_urls(chain_id, symbol, token_address) {
        if !urls.contains(&url) {
            urls.push(url);
        }
    }
    Logos {
        logo_urls: urls,
        badge_logo: badge.and_then(chain_logo_url),
        badge_hidden: badge.is_none(),
    }
}

/// A chain drawn as itself: its own logo, no badge.
pub fn chain_logos(chain_id: u32) -> Logos {
    Logos {
        logo_urls: chain_logo_url(chain_id).into_iter().collect(),
        badge_logo: None,
        badge_hidden: true,
    }
}
