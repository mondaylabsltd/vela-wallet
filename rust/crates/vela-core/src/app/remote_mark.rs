//! Which logo a token or a network wears, and how long a remote logo that
//! failed to load stays failed (spec 082, RE10). Everything here is pure.
//!
//! ## Which logo
//!
//! One rule for every circle that stands for a coin or a network, on all four
//! shells. It used to be written five times (the web's `tokens-model.ts` +
//! `flows/marks.ts`, the desktop's `marks.rs`, Android's `Marks.kt`, iOS's
//! `Marks.swift`), and the copies had begun to disagree about which mark a
//! surface asks for. The founder's rulings of 2026-09-05 and 2026-09-12:
//!
//! 1. Logos come from the person's chain-data endpoint (the built-in one
//!    when the setting is empty): `{base}/chainlogos/eip155-{id}.png` and
//!    `{base}/assets/eip155-{chain}/{address}/logo.png`. The drawn
//!    three-letter glyph is the whole fallback: a mark is never a blank
//!    circle.
//! 2. A native coin wears its OWN home chain's logo: ETH on Base is still
//!    Ethereum's.
//! 3. The chain badge is hidden when it would repeat the token: ETH on
//!    Ethereum, XDAI on Gnosis, BNB on BNB Chain.
//! 4. A token's logo is its asset entry under the EIP-55 checksummed address,
//!    with the lowercase path as the second candidate; logo URLs an index
//!    already named come before both.
//!
//! **The kind rule.** Anything that names a NETWORK (a network row or fact, a
//! notice that locks a chain, a receive row, the QR code's centre, a chip)
//! wears [`chain_mark`]; anything that names a COIN wears [`token_mark`].
//! Never a token's logo on a network row, and never rule 2 on one: the
//! network row of ETH sent on Base wears Base's logo.
//!
//! ## How long a miss lasts
//!
//! A miss is classed from its HTTP status: a missing or refused image
//! is remembered for the session, a throttled, broken or unreached one for a
//! short while, so a logo lost to a bad minute comes back (W20: on the iPhone
//! one bad minute left every token as a lettermark until the app was killed).
//! A miss is still remembered at all because a token with no logo would
//! otherwise be fetched again on every scroll — one 404 per row per frame.
//!
//! The shells keep the expiry next to each miss; a transient miss (one with a
//! TTL) is also forgotten on `net_health`'s `CameBack`. The web's
//! `<img onerror>` has no status, so its misses are [`MarkMiss::Unknown`].

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::contacts::is_address;
use super::network_admin::DEFAULT_ETHEREUM_DATA_URL;
use crate::primitives::checksum_address;

/// How long a transient miss is remembered before the logo is asked for
/// again.
pub const MARK_MISS_TRANSIENT_MS: u32 = 60_000;

/// Why a logo did not arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkMiss {
    /// 404 or 410: there is no image there.
    NotFound,
    /// 401 or 403: the server will not give it to us.
    Refused,
    /// The server answered with success and what came back does not draw.
    NotAnImage,
    /// 429: asked too often — it will answer later.
    Throttled,
    /// 5xx or 408: the server is having a bad minute.
    ServerError,
    /// No answer at all: the connection failed, dropped or timed out.
    Transport,
    /// Anything else, and a miss with no status (the web's `onerror`).
    Unknown,
}

/// The class of a miss whose response carried HTTP `status`.
#[must_use]
pub fn mark_miss_of_status(status: u16) -> MarkMiss {
    match status {
        404 | 410 => MarkMiss::NotFound,
        401 | 403 => MarkMiss::Refused,
        429 => MarkMiss::Throttled,
        408 | 500..=599 => MarkMiss::ServerError,
        // The request succeeded; the bytes were the problem.
        200..=299 => MarkMiss::NotAnImage,
        _ => MarkMiss::Unknown,
    }
}

/// How long `miss` is remembered: `None` for the session (asking again will
/// not help), `Some(ms)` for a miss that may heal.
#[must_use]
pub fn mark_miss_ttl_ms(miss: MarkMiss) -> Option<u32> {
    match miss {
        MarkMiss::NotFound | MarkMiss::Refused | MarkMiss::NotAnImage => None,
        MarkMiss::Throttled | MarkMiss::ServerError | MarkMiss::Transport | MarkMiss::Unknown => {
            Some(MARK_MISS_TRANSIENT_MS)
        }
    }
}

// ---------------------------------------------------------------------------
// Which logo
// ---------------------------------------------------------------------------

/// How many characters of a ticker the drawn glyph keeps.
pub const GLYPH_CHARS: usize = 3;

/// What a circle standing for a token or a network wears.
///
/// The shell draws `glyph` in the circle, then the first of `logo_urls` that
/// loads over it (a miss falls through to the next candidate, and a miss is
/// remembered as [`mark_miss_ttl_ms`] says). The corner badge is drawn only
/// when `badge_chain_id` is set: the chain's colour dot, with
/// `badge_logo_url` over it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MarkView {
    /// The drawn fallback, always present: the ticker's first three
    /// characters, upper-cased ("ETH", "USD" for USDC, "PAT" for pathUSD).
    pub glyph: String,
    /// Logo candidates, best first. Empty means the glyph alone.
    pub logo_urls: Vec<String>,
    /// The chain the corner badge names. `None` means no badge at all: a
    /// native coin on its own home chain, or a network drawn as itself.
    pub badge_chain_id: Option<u32>,
    /// The badge chain's logo, drawn over its dot. `None` whenever there is
    /// no badge.
    pub badge_logo_url: Option<String>,
}

/// Chain id 0 names no network: no logo URL is ever built on it. The callers'
/// rule is to pass the token's or the request's chain instead; this keeps a
/// caller that forgot from asking the endpoint for `eip155-0` on every row.
const NO_CHAIN: u32 = 0;

/// The endpoint root: the person's ethereum-data URL without its trailing
/// slashes, or the built-in endpoint when they left the field empty.
fn data_root(ethereum_data_url: &str) -> &str {
    let root = ethereum_data_url.trim().trim_end_matches('/');
    if root.is_empty() {
        DEFAULT_ETHEREUM_DATA_URL
    } else {
        root
    }
}

/// The ticker's drawn glyph: its first [`GLYPH_CHARS`] characters,
/// upper-cased.
fn glyph_of(ticker: &str) -> String {
    ticker
        .chars()
        .take(GLYPH_CHARS)
        .collect::<String>()
        .to_uppercase()
}

/// A network's logo on the person's chain-data endpoint:
/// `{base}/chainlogos/eip155-{chain_id}.png`. An empty `ethereum_data_url`
/// means the built-in endpoint ([`DEFAULT_ETHEREUM_DATA_URL`]). `None` only for
/// chain id 0, which names no network.
#[must_use]
pub fn chain_logo_url(ethereum_data_url: &str, chain_id: u32) -> Option<String> {
    (chain_id != NO_CHAIN).then(|| {
        format!(
            "{}/chainlogos/eip155-{chain_id}.png",
            data_root(ethereum_data_url)
        )
    })
}

/// Which chain's logo a native coin wears: the coin's home chain (ETH → 1,
/// BNB → 56, POL and MATIC → 137, AVAX → 43114, XDAI → 100), whatever chain it
/// sits on; any other coin wears the logo of `chain_id`, the chain it is on.
/// The symbol is compared case-insensitively ("xDAI" is XDAI).
#[must_use]
pub fn native_coin_logo_chain_id(symbol: &str, chain_id: u32) -> u32 {
    match symbol.to_uppercase().as_str() {
        "ETH" => 1,
        "BNB" => 56,
        "POL" | "MATIC" => 137,
        "AVAX" => 43114,
        "XDAI" => 100,
        _ => chain_id,
    }
}

/// An ERC-20's own logo candidates: the asset entry under the checksummed
/// address, then the lowercase one when that is a different path (the index
/// holds both spellings and neither is guaranteed). A malformed address, or
/// chain 0, has none: the glyph is drawn rather than a guess.
fn asset_logo_urls(root: &str, chain_id: u32, address: &str) -> Vec<String> {
    if chain_id == NO_CHAIN || !is_address(address) {
        return Vec::new();
    }
    let lower = address.to_ascii_lowercase();
    let checksummed = checksum_address(&lower).unwrap_or_else(|_| lower.clone());
    let mut urls = vec![format!(
        "{root}/assets/eip155-{chain_id}/{checksummed}/logo.png"
    )];
    if checksummed != lower {
        urls.push(format!("{root}/assets/eip155-{chain_id}/{lower}/logo.png"));
    }
    urls
}

/// A coin's mark: `chain_id` is the chain it is held or sent on, `symbol` its
/// ticker, `token_address` its contract (`None` = the chain's native coin;
/// an empty or malformed address is a contract this rule cannot place, so it
/// gets no derived logo), and `named` the logo URLs an index already gave for
/// it (the core's `logo_urls`), which are tried first.
///
/// `logo_urls` = `named` (blank entries dropped, duplicates removed), then the
/// derived candidates not already named: a native coin's home-chain logo
/// ([`native_coin_logo_chain_id`]), or an ERC-20's asset entry, checksummed
/// then lowercase. The badge names `chain_id`, except for a native coin on
/// its own home chain, where it would repeat the coin.
#[must_use]
pub fn token_mark(
    ethereum_data_url: &str,
    chain_id: u32,
    symbol: &str,
    token_address: Option<&str>,
    named: &[String],
) -> MarkView {
    let mut logo_urls: Vec<String> = Vec::new();
    for url in named {
        let url = url.trim();
        if !url.is_empty() && !logo_urls.iter().any(|seen| seen == url) {
            logo_urls.push(url.to_owned());
        }
    }
    let derived = match token_address {
        None => chain_logo_url(
            ethereum_data_url,
            native_coin_logo_chain_id(symbol, chain_id),
        )
        .into_iter()
        .collect(),
        Some(address) => asset_logo_urls(data_root(ethereum_data_url), chain_id, address),
    };
    for url in derived {
        if !logo_urls.contains(&url) {
            logo_urls.push(url);
        }
    }
    let at_home =
        token_address.is_none() && native_coin_logo_chain_id(symbol, chain_id) == chain_id;
    let badge_chain_id = (!at_home).then_some(chain_id);
    MarkView {
        glyph: glyph_of(symbol),
        logo_urls,
        badge_chain_id,
        badge_logo_url: badge_chain_id.and_then(|id| chain_logo_url(ethereum_data_url, id)),
    }
}

/// A NETWORK drawn as itself (a network row or fact, a notice that locks a
/// chain, a receive row, the QR code's centre, a chip): its own logo, the
/// glyph of its native coin's ticker, and never a badge. Never the native
/// coin's home-chain rule either: Base is Base's logo, not Ethereum's.
#[must_use]
pub fn chain_mark(ethereum_data_url: &str, chain_id: u32, native_symbol: &str) -> MarkView {
    MarkView {
        glyph: glyph_of(native_symbol),
        logo_urls: chain_logo_url(ethereum_data_url, chain_id)
            .into_iter()
            .collect(),
        badge_chain_id: None,
        badge_logo_url: None,
    }
}
