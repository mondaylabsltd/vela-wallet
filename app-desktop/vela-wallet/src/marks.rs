//! Which logo a token or a network wears: the core's one rule
//! (`vela_core::app::remote_mark`), on the person's chain-data endpoint.
//!
//! The rule used to be written here, and once more in each of the other three
//! shells, and the copies had begun to disagree (2026-10-08 survey). It now
//! lives in vela-core: which URLs a mark tries, in what order, which chain's
//! logo a native coin wears, and when the corner badge is hidden. This module
//! only reads the endpoint and turns the core's `MarkView` into the shape the
//! desktop draws.
//!
//! The kind rule, the core's too: anything that names a NETWORK (a network
//! row or fact, a notice that locks a chain, a receive row, the QR code's
//! centre, a chip, the add-network wizard) wears [`chain_logos`]; anything
//! that names a COIN wears [`token_logos`]. Never a token's logo on a network
//! row, and never the native coin's home chain there: the network row of ETH
//! sent on Base wears Base's logo.
use gpui::SharedString;
use vela_core::app::remote_mark::{self, GLYPH_CHARS, MarkView};

use crate::executor::chain_tokens::data_base;

/// A mark's logo candidates and its badge, as the desktop draws them.
///
/// `Default` is "no logos": the drawn glyph and the coloured dot, which is
/// what the fixtures (and any surface the endpoint cannot price) show.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Logos {
    /// Best first. The drawing tries each in turn and keeps the first that
    /// loads; the glyph stays under them all.
    pub logo_urls: Vec<SharedString>,
    pub badge_logo: Option<SharedString>,
    /// The core's `badge_chain_id == None`: no badge at all. The dot's colour
    /// is the shell's tint of the token's chain, which is the chain the
    /// badge names whenever there is one.
    pub badge_hidden: bool,
}

impl From<MarkView> for Logos {
    fn from(mark: MarkView) -> Self {
        Self {
            logo_urls: mark.logo_urls.into_iter().map(SharedString::from).collect(),
            badge_logo: mark.badge_logo_url.map(SharedString::from),
            badge_hidden: mark.badge_chain_id.is_none(),
        }
    }
}

/// The endpoint's logo for one network. `None` for chain 0, which names none.
pub fn chain_logo_url(chain_id: u32) -> Option<SharedString> {
    chain_logo_url_on(&data_base(), chain_id)
}

/// A coin's logos: `chain_id` is the chain it is held or sent on,
/// `token_address` its contract (`None` = the chain's own coin, never `""`),
/// and `named` the logo URLs the core already gave for it, tried first.
pub fn token_logos(
    chain_id: u32,
    symbol: &str,
    token_address: Option<&str>,
    named: &[String],
) -> Logos {
    token_logos_on(&data_base(), chain_id, symbol, token_address, named)
}

/// A network drawn as itself: its own logo, never a badge.
pub fn chain_logos(chain_id: u32) -> Logos {
    chain_logos_on(&data_base(), chain_id)
}

/// The drawn fallback a mark's circle carries: the ticker's first three
/// characters, upper-cased — the core's `MarkView.glyph`, which the vectors
/// below hold this to.
pub fn glyph(ticker: &str) -> SharedString {
    SharedString::from(
        ticker
            .chars()
            .take(GLYPH_CHARS)
            .collect::<String>()
            .to_uppercase(),
    )
}

fn chain_logo_url_on(ethereum_data_url: &str, chain_id: u32) -> Option<SharedString> {
    remote_mark::chain_logo_url(ethereum_data_url, chain_id).map(SharedString::from)
}

fn token_logos_on(
    ethereum_data_url: &str,
    chain_id: u32,
    symbol: &str,
    token_address: Option<&str>,
    named: &[String],
) -> Logos {
    remote_mark::token_mark(ethereum_data_url, chain_id, symbol, token_address, named).into()
}

fn chain_logos_on(ethereum_data_url: &str, chain_id: u32) -> Logos {
    // The glyph is drawn from the mark's own ticker (`glyph`), so the native
    // symbol the core would letter it with is not needed here.
    remote_mark::chain_mark(ethereum_data_url, chain_id, "").into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The core's conformance file, the one every shell replays.
    const VECTORS: &str = include_str!("../../../rust/crates/vela-core/tests/vectors/marks.json");

    fn text<'a>(value: &'a Value, key: &str, name: &str) -> &'a str {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_else(|| unreachable!("{name}: `{key}` is not a string"))
    }

    fn chain_of(input: &Value, name: &str) -> u32 {
        input
            .get("chain_id")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .unwrap_or_else(|| unreachable!("{name}: no chain_id"))
    }

    fn strings(value: &Value, key: &str, name: &str) -> Vec<String> {
        value
            .get(key)
            .and_then(Value::as_array)
            .unwrap_or_else(|| unreachable!("{name}: `{key}` is not a list"))
            .iter()
            .map(|item| {
                item.as_str()
                    .unwrap_or_else(|| unreachable!("{name}: `{key}` holds a non-string"))
                    .to_owned()
            })
            .collect()
    }

    /// What the desktop must draw for one expected `MarkView`.
    fn drawn(expect: &Value, name: &str) -> Logos {
        Logos {
            logo_urls: strings(expect, "logo_urls", name)
                .into_iter()
                .map(SharedString::from)
                .collect(),
            badge_logo: expect
                .get("badge_logo_url")
                .and_then(Value::as_str)
                .map(|url| SharedString::from(url.to_owned())),
            badge_hidden: expect.get("badge_chain_id").is_none_or(Value::is_null),
        }
    }

    /// Every case in `marks.json`, through the functions the desktop draws
    /// with: the candidates in the core's order, the badge and its logo, and
    /// the glyph the circles letter themselves with.
    #[test]
    fn every_mark_vector_replays_through_the_desktop_marks() {
        let suite: Value =
            serde_json::from_str(VECTORS).unwrap_or_else(|e| unreachable!("marks.json: {e}"));
        assert_eq!(suite.get("suite").and_then(Value::as_str), Some("marks"));
        let cases = suite
            .get("cases")
            .and_then(Value::as_array)
            .unwrap_or_else(|| unreachable!("marks.json has no cases"));
        let (mut tokens, mut chains, mut urls) = (0, 0, 0);
        for case in cases {
            let name = text(case, "name", "a case");
            let input = case
                .get("input")
                .unwrap_or_else(|| unreachable!("{name}: no input"));
            let expect = case
                .get("expect")
                .unwrap_or_else(|| unreachable!("{name}: no expect"));
            let base = text(input, "ethereum_data_url", name);
            let chain_id = chain_of(input, name);
            match text(case, "fn", name) {
                "token_mark" => {
                    tokens += 1;
                    let symbol = text(input, "symbol", name);
                    let address = input.get("token_address").and_then(Value::as_str);
                    let named = strings(input, "named", name);
                    assert_eq!(
                        token_logos_on(base, chain_id, symbol, address, &named),
                        drawn(expect, name),
                        "{name}"
                    );
                    assert_eq!(
                        glyph(symbol).as_ref(),
                        text(expect, "glyph", name),
                        "{name}"
                    );
                    // The desktop tints the dot by the token's own chain; that
                    // is right only while the core's badge names that chain.
                    if let Some(badge) = expect.get("badge_chain_id").and_then(Value::as_u64) {
                        assert_eq!(badge, u64::from(chain_id), "{name}: the badge's chain");
                    }
                }
                "chain_mark" => {
                    chains += 1;
                    let native = text(input, "native_symbol", name);
                    let logos = chain_logos_on(base, chain_id);
                    assert_eq!(logos, drawn(expect, name), "{name}");
                    assert!(logos.badge_hidden, "{name}: a network wears no badge");
                    assert_eq!(
                        glyph(native).as_ref(),
                        text(expect, "glyph", name),
                        "{name}"
                    );
                }
                "chain_logo_url" => {
                    urls += 1;
                    assert_eq!(
                        chain_logo_url_on(base, chain_id).as_deref(),
                        expect.get("value").and_then(Value::as_str),
                        "{name}"
                    );
                }
                other => unreachable!("{name}: `{other}` is a function this replay does not know"),
            }
        }
        assert!(
            tokens > 0 && chains > 0 && urls > 0,
            "the file covers all three functions ({tokens}, {chains}, {urls})"
        );
        assert_eq!(tokens + chains + urls, cases.len());
    }

    /// The kind rule at the call the network rows make: ETH sent on Base
    /// is a Base row, so it wears Base's logo — the token's rule would have
    /// put Ethereum's there.
    #[test]
    fn a_network_wears_its_own_logo_never_its_coins() {
        let base = "https://data.example";
        let network = chain_logos_on(base, 8453);
        assert_eq!(
            network.logo_urls,
            vec![SharedString::from(
                "https://data.example/chainlogos/eip155-8453.png"
            )]
        );
        assert!(network.badge_hidden && network.badge_logo.is_none());
        let coin = token_logos_on(base, 8453, "ETH", None, &[]);
        assert_eq!(
            coin.logo_urls,
            vec![SharedString::from(
                "https://data.example/chainlogos/eip155-1.png"
            )]
        );
        assert!(!coin.badge_hidden);
    }

    /// Chain 0 names no network: nothing is asked of the endpoint for it.
    #[test]
    fn chain_zero_asks_for_nothing() {
        assert_eq!(chain_logo_url_on("", 0), None);
        assert!(chain_logos_on("", 0).logo_urls.is_empty());
        let usdc = token_logos_on(
            "",
            0,
            "USDC",
            Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"),
            &[],
        );
        assert!(usdc.logo_urls.is_empty() && usdc.badge_logo.is_none());
    }
}
