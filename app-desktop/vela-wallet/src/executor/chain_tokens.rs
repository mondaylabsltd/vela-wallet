//! What a chain holds: its stablecoins, its wrapped native coin, its DEX.
//!
//! **Ported from** `src/services/chain-tokens.ts` @ `117ef753` (FR-006).
//!
//! The balance fetch needs three things per chain that no core owns and no
//! chain announces: which contracts are that chain's stablecoins, which one
//! wraps its native coin, and which DEX can be asked for a price. They come
//! from the ethereum-data index the settings screen already points at, with a
//! built-in DEX table that wins over whatever the index says.
//!
//! **The override direction is deliberate.** A wrong stablecoin address costs a
//! balance row nobody holds; a wrong quoter address costs a *price*, and a price
//! is multiplied by somebody's whole holding before it reaches the total. So the
//! DEX contracts are pinned here and the remote index supplies only the token
//! lists.

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use vela_core::app::network_admin::DEFAULT_ETHEREUM_DATA_URL;

use crate::executor::{proxy, storage};

/// The index is a slow-moving document; 30 minutes is the web's TTL.
const CACHE_TTL: Duration = Duration::from_secs(30 * 60);
const FETCH_TIMEOUT: Duration = Duration::from_secs(8);

/// One of a chain's curated stablecoins. `kind` is the index's `"native"` /
/// `"bridge"` distinction, which only orders the quote-token preference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StableToken {
    pub symbol: String,
    pub kind: String,
    pub contract: String,
}

/// How to ask this chain for a price.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DexInfo {
    /// `uniswap-v3` reads `quoterV2`; `solidly` reads `router`. Anything else
    /// has no encoder here and falls through to Chainlink.
    pub protocol: &'static str,
    pub quoter_v2: Option<&'static str>,
    pub router: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChainTokenData {
    pub native_name: String,
    pub native_symbol: String,
    pub native_decimals: u32,
    pub stables: Vec<StableToken>,
    pub wrapped_native: Option<String>,
    pub dex: Option<DexInfo>,
}

const fn uniswap_v3(quoter: &'static str) -> DexInfo {
    DexInfo {
        protocol: "uniswap-v3",
        quoter_v2: Some(quoter),
        router: None,
    }
}

const fn solidly(router: &'static str) -> DexInfo {
    DexInfo {
        protocol: "solidly",
        quoter_v2: None,
        router: Some(router),
    }
}

/// The most mainstream DEX per chain, pinned. Never the remote index's.
fn builtin_dex(chain_id: u32) -> Option<DexInfo> {
    // The canonical Uniswap V3 QuoterV2, deployed at the same address on every
    // chain Uniswap ships to.
    const UNI_QUOTER: &str = "0x61fFE014bA17989E743c5F6cB21bF9697530B21e";
    Some(match chain_id {
        1 | 137 | 42161 | 10 | 130 | 143 | 480 => uniswap_v3(UNI_QUOTER),
        // PancakeSwap V3.
        56 => uniswap_v3("0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997"),
        // Aerodrome — Base's deepest liquidity by TVL.
        8453 => solidly("0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43"),
        // Uniswap V3 on Avalanche, whose quoter is NOT the canonical address.
        // Trader Joe's liquidity-book has no encoder here.
        43114 => uniswap_v3("0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F"),
        // SushiSwap V3 — the best available V3 quoter on Gnosis.
        100 => uniswap_v3("0xb1E835Dc2785b52265711e17fCCb0fd018226a6e"),
        // Not listed, deliberately: Tempo (4217) and Arc (5042) have no DEX to
        // quote against. Arc does not need one — its native coin is USDC and
        // the core's peg prices it (spec 060).
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/// What the index said about one chain's document — three answers, not two
/// (PR 2 polish). "No such document" and "could not ask" were one `None`,
/// and a chain whose money lives only in the registry's stablecoins (Tempo:
/// no native coin) read as "answered, holds nothing" — $0.00 — when the
/// index was simply out of reach.
#[derive(Clone, Debug, PartialEq)]
pub enum IndexDoc {
    /// A 2xx with a body that parses.
    Doc(ChainTokenData),
    /// HTTP 404 from a server that answered: definitively no document for
    /// this chain.
    Absent,
    /// Not read: no answer (network, timeout, TLS), a 5xx, a 429 or any
    /// other non-2xx, or a body that does not parse. Says nothing about the
    /// chain — never cached.
    Unread,
}

impl IndexDoc {
    /// The document's data, when there is one.
    #[must_use]
    pub fn data(&self) -> Option<&ChainTokenData> {
        match self {
            Self::Doc(data) => Some(data),
            Self::Absent | Self::Unread => None,
        }
    }
}

/// Only an answer is cached: a document, or the index saying it has none.
type Cache = Mutex<HashMap<u32, (Option<ChainTokenData>, Instant)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Forget every cached chain document — after the endpoint is edited, or a
/// state directory is swapped underneath a test.
pub fn invalidate() {
    if let Ok(mut cache) = cache().lock() {
        cache.clear();
    }
}

/// This chain's document as the index answered, from cache or from the
/// index. A document and a 404 are cached for [`CACHE_TTL`] — a chain the
/// index does not carry would otherwise re-fetch on every refresh, forever;
/// an unread one never is: the next read asks again.
///
/// Neither a 404 nor an unread index is a reason to skip the chain: the
/// balance read still reads the native coin, it just reads no ERC-20s and
/// prices nothing from a DEX — except on a chain with no native coin, where
/// an unread index means nothing was read (`balances::not_read`).
#[must_use]
pub fn fetch_doc(chain_id: u32) -> IndexDoc {
    fetch_doc_with(chain_id, || fetch_uncached(chain_id))
}

/// [`fetch_doc`] with the request supplied — the seam the tests use.
fn fetch_doc_with(chain_id: u32, ask: impl FnOnce() -> IndexDoc) -> IndexDoc {
    if let Some(data) = cached_answer(chain_id) {
        return data.map_or(IndexDoc::Absent, IndexDoc::Doc);
    }
    let fetched = ask();
    let answer = match &fetched {
        IndexDoc::Doc(data) => Some(Some(data.clone())),
        IndexDoc::Absent => Some(None),
        IndexDoc::Unread => None,
    };
    if let Some(answer) = answer
        && let Ok(mut cache) = cache().lock()
    {
        cache.insert(chain_id, (answer, Instant::now()));
    }
    fetched
}

/// The index's cached answer for `chain_id` while it is fresh: `Some(Some)`
/// a document, `Some(None)` the index has none, `None` nothing cached.
fn cached_answer(chain_id: u32) -> Option<Option<ChainTokenData>> {
    let cache = cache().lock().ok()?;
    let (data, at) = cache.get(&chain_id)?;
    (at.elapsed() < CACHE_TTL).then(|| data.clone())
}

/// The cached document for `chain_id`, if a read this half hour found one —
/// never a request. For what is told after a balance round, which has
/// just asked for every chain it read: a chain whose index was unread then
/// has no facts now, rather than another wait for the same index.
#[must_use]
pub fn cached(chain_id: u32) -> Option<ChainTokenData> {
    cached_answer(chain_id).flatten()
}

fn fetch_uncached(chain_id: u32) -> IndexDoc {
    let url = format!("{}/chains/eip155-{chain_id}.json", data_base());
    let answer = proxy::with_routes(&url, FETCH_TIMEOUT, |agent| agent.get(&url).call())
        .map_err(|failure| failure.error)
        .and_then(|mut response| {
            let mut body = String::new();
            response
                .body_mut()
                .as_reader()
                .read_to_string(&mut body)
                .map(|_| body)
                .map_err(ureq::Error::Io)
        });
    classify(chain_id, answer)
}

/// The index's answer, as one of the three (PR 2 polish). `StatusCode` is
/// the only error that means the server answered (`registry::classify`), and
/// of those only a 404 says the document does not exist; a 5xx or a 429 is a
/// server that could not say, and a timeout or a refused connection never
/// reached one.
fn classify(chain_id: u32, answer: Result<String, ureq::Error>) -> IndexDoc {
    match answer {
        Ok(body) => serde_json::from_str::<Value>(&body)
            .map_or(IndexDoc::Unread, |raw| IndexDoc::Doc(parse(chain_id, &raw))),
        Err(ureq::Error::StatusCode(404)) => IndexDoc::Absent,
        Err(_) => IndexDoc::Unread,
    }
}

/// Where the index lives: the configured endpoint, or its default. The same
/// resolution `network_admin` does, and it must stay the same one — two
/// answers here is a settings screen that changes one caller's host.
///
/// Shared with `clear_signing`, which fetches ERC-7730 descriptors from the
/// same host, for exactly that reason.
///
/// Remembered until the wallet file is next written, or moves: `marks` builds
/// every logo URL from it, once per row per frame, and reading it from disk
/// each time was a fifth of every scrolling frame.
pub(crate) fn data_base() -> String {
    static LAST: Mutex<Option<(u64, std::path::PathBuf, String)>> = Mutex::new(None);
    let generation = storage::generation();
    let path = storage::path().unwrap_or_default();
    if let Ok(last) = LAST.lock()
        && let Some((seen, at, url)) = last.as_ref()
        && *seen == generation
        && *at == path
    {
        return url.clone();
    }
    let url = read_data_base();
    if let Ok(mut last) = LAST.lock() {
        *last = Some((generation, path, url.clone()));
    }
    url
}

fn read_data_base() -> String {
    storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("ethereumDataURL")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| DEFAULT_ETHEREUM_DATA_URL.to_owned())
}

fn parse(chain_id: u32, raw: &Value) -> ChainTokenData {
    let native = raw.get("nativeCurrency");
    let decimals = native
        .and_then(|n| n.get("decimals"))
        .and_then(Value::as_u64)
        .and_then(|d| u32::try_from(d).ok())
        .filter(|d| *d <= 255)
        .unwrap_or(18);
    let stables = raw
        .get("stables")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(StableToken {
                        symbol: item.get("symbol").and_then(Value::as_str)?.to_owned(),
                        kind: item
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                        contract: item.get("contract").and_then(Value::as_str)?.to_owned(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    ChainTokenData {
        native_name: native
            .and_then(|n| n.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("Ether")
            .to_owned(),
        native_symbol: native
            .and_then(|n| n.get("symbol"))
            .and_then(Value::as_str)
            .unwrap_or("ETH")
            .to_owned(),
        native_decimals: decimals,
        stables,
        wrapped_native: raw
            .get("wrappedNativeToken")
            .and_then(Value::as_str)
            .map(str::to_owned),
        dex: builtin_dex(chain_id),
    }
}

/// The stablecoin a custom token's price should be quoted against first: native
/// USDC, then any USDC, then USDT, then whatever the chain has.
///
/// It only ORDERS the attempts, and it matters because
/// `first_grouped_quote_price` takes the FIRST group that answers rather than
/// the best. The native-coin path does not use it: there the core takes the
/// maximum across every stable, so order changes nothing.
#[must_use]
pub fn pick_quote_token(stables: &[StableToken]) -> Option<&StableToken> {
    stables
        .iter()
        .find(|s| s.symbol == "USDC" && s.kind == "native")
        .or_else(|| stables.iter().find(|s| s.symbol == "USDC"))
        .or_else(|| stables.iter().find(|s| s.symbol == "USDT"))
        .or_else(|| stables.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The index's shape, including the fields it is allowed to omit.
    #[test]
    fn the_index_document_parses_and_its_gaps_have_answers() {
        let raw = json!({
            "nativeCurrency": { "name": "xDAI", "symbol": "xDAI", "decimals": 18 },
            "stables": [
                { "symbol": "USDC", "type": "native", "contract": "0xaaa" },
                { "symbol": "WXDAI", "type": "native", "contract": "0xbbb" },
                // A malformed entry must drop, not poison the list.
                { "symbol": "BROKEN" },
            ],
            "wrappedNativeToken": "0xccc",
            "dex": { "dex": "whatever", "protocol": "curve", "contracts": {} },
        });
        let data = parse(100, &raw);
        assert_eq!(data.native_symbol, "xDAI");
        assert_eq!(data.native_decimals, 18);
        assert_eq!(data.stables.len(), 2);
        assert_eq!(data.wrapped_native.as_deref(), Some("0xccc"));
        // The built-in DEX wins over the index's — a wrong quoter is a wrong
        // price, and a wrong price is multiplied by a whole holding.
        let dex = data.dex.unwrap_or_else(|| unreachable!("no dex"));
        assert_eq!(dex.protocol, "uniswap-v3");
        assert_eq!(
            dex.quoter_v2,
            Some("0xb1E835Dc2785b52265711e17fCCb0fd018226a6e")
        );

        // An empty document still answers, with defaults rather than a panic.
        let empty = parse(999_999, &json!({}));
        assert_eq!(empty.native_symbol, "ETH");
        assert_eq!(empty.native_decimals, 18);
        assert!(empty.stables.is_empty());
        assert_eq!(empty.dex, None);
    }

    fn stable(symbol: &str, kind: &str) -> StableToken {
        StableToken {
            symbol: symbol.to_owned(),
            kind: kind.to_owned(),
            contract: format!("0x{symbol}"),
        }
    }

    /// The quote-token preference, in its stated order.
    #[test]
    fn the_quote_token_prefers_native_usdc_then_any_usdc_then_usdt() {
        let all = vec![
            stable("USDT", "native"),
            stable("USDC", "bridge"),
            stable("USDC", "native"),
        ];
        let picked = pick_quote_token(&all).unwrap_or_else(|| unreachable!("a stable"));
        assert_eq!(
            (picked.symbol.as_str(), picked.kind.as_str()),
            ("USDC", "native")
        );

        let bridged = vec![stable("USDT", "native"), stable("USDC", "bridge")];
        assert_eq!(
            pick_quote_token(&bridged).map(|s| s.symbol.as_str()),
            Some("USDC")
        );

        let no_usdc = vec![stable("DAI", "native"), stable("USDT", "native")];
        assert_eq!(
            pick_quote_token(&no_usdc).map(|s| s.symbol.as_str()),
            Some("USDT")
        );

        let neither = vec![stable("DAI", "native")];
        assert_eq!(
            pick_quote_token(&neither).map(|s| s.symbol.as_str()),
            Some("DAI")
        );
        assert!(pick_quote_token(&[]).is_none());
    }

    /// An out-of-range `decimals` is the index being wrong about a scale, and a
    /// wrong scale renders an amount at the wrong magnitude.
    #[test]
    fn an_impossible_decimals_falls_back_rather_than_scaling_by_it() {
        let raw = json!({ "nativeCurrency": { "decimals": 999 } });
        assert_eq!(parse(1, &raw).native_decimals, 18);
        let negative = json!({ "nativeCurrency": { "decimals": -2 } });
        assert_eq!(parse(1, &negative).native_decimals, 18);
    }

    /// The host is remembered between frames, and the settings write that
    /// moves it is what makes it forgotten: the next frame's logos come from
    /// the new host, not from the one the wallet started with.
    #[test]
    fn the_remembered_data_host_follows_a_settings_write() {
        storage::tests::with_temp_state("chain-tokens-host", || {
            assert_eq!(data_base(), DEFAULT_ETHEREUM_DATA_URL);
            let mut fields = serde_json::Map::new();
            fields.insert("ethereumDataURL".to_owned(), json!("https://data.example"));
            if storage::merge_value(storage::KEY_SERVICE_ENDPOINTS, fields).is_err() {
                unreachable!("could not save the data endpoint");
            }
            assert_eq!(data_base(), "https://data.example");
        });
    }

    /// Every chain with a pinned DEX names contracts for the protocol it says
    /// it speaks — a `uniswap-v3` entry with no quoter would encode nothing and
    /// price nothing, silently.
    #[test]
    fn every_pinned_dex_carries_the_contract_its_protocol_reads() {
        for chain_id in [1, 56, 137, 42161, 10, 8453, 43114, 100, 130, 143, 480] {
            let dex = builtin_dex(chain_id)
                .unwrap_or_else(|| unreachable!("chain {chain_id} lost its DEX"));
            match dex.protocol {
                "uniswap-v3" => assert!(dex.quoter_v2.is_some(), "chain {chain_id}"),
                "solidly" => assert!(dex.router.is_some(), "chain {chain_id}"),
                other => unreachable!("chain {chain_id} speaks {other}, which has no encoder"),
            }
        }
        assert_eq!(builtin_dex(4217), None, "Tempo has no DEX to quote");
        assert_eq!(
            builtin_dex(5042),
            None,
            "Arc has no DEX; its coin is pegged"
        );
    }

    /// PR 2 polish: the index's answer is one of three. A document; a 404
    /// from a server that answered — no document for this chain; and
    /// everything else — a 5xx, a 429, a timeout, a refused connection, a
    /// body that does not parse — is unread, which says nothing about the
    /// chain.
    #[test]
    fn the_index_answer_is_a_document_absent_or_unread() {
        let doc = classify(
            100,
            Ok(r#"{"nativeCurrency":{"symbol":"xDAI"}}"#.to_owned()),
        );
        assert!(matches!(&doc, IndexDoc::Doc(data) if data.native_symbol == "xDAI"));
        assert_eq!(
            classify(4217, Err(ureq::Error::StatusCode(404))),
            IndexDoc::Absent
        );
        for unread in [
            Err(ureq::Error::StatusCode(500)),
            Err(ureq::Error::StatusCode(503)),
            Err(ureq::Error::StatusCode(429)),
            Err(ureq::Error::StatusCode(403)),
            Err(ureq::Error::Timeout(ureq::Timeout::Global)),
            Err(ureq::Error::ConnectionFailed),
            Err(ureq::Error::Io(std::io::Error::other("reset"))),
            Ok("<html>gateway</html>".to_owned()),
        ] {
            let case = format!("{unread:?}");
            assert_eq!(classify(4217, unread), IndexDoc::Unread, "{case}");
        }
    }

    /// PR 2 polish: a document and a 404 are answers, kept for the half
    /// hour; an unread index is asked again on the next read — cached, it
    /// held a chain at "not read" for thirty minutes after the network came
    /// back. Answered without the network, on chain ids no other test uses.
    #[test]
    fn only_an_answer_is_cached() {
        let asked = &std::cell::Cell::new(0);
        let ask = |answer: IndexDoc| {
            move || {
                asked.set(asked.get() + 1);
                answer
            }
        };
        // Unread, twice: asked twice — and then the answer is kept.
        assert_eq!(
            fetch_doc_with(990_001, ask(IndexDoc::Unread)),
            IndexDoc::Unread
        );
        assert_eq!(
            fetch_doc_with(990_001, ask(IndexDoc::Unread)),
            IndexDoc::Unread
        );
        assert_eq!(asked.get(), 2, "an unread index is asked again");
        assert_eq!(cached(990_001), None);
        let data = parse(990_001, &json!({}));
        assert_eq!(
            fetch_doc_with(990_001, ask(IndexDoc::Doc(data.clone()))),
            IndexDoc::Doc(data.clone())
        );
        assert_eq!(
            fetch_doc_with(990_001, ask(IndexDoc::Unread)),
            IndexDoc::Doc(data.clone()),
            "the document stands"
        );
        assert_eq!(asked.get(), 3);
        assert_eq!(cached(990_001), Some(data));
        // A 404 is kept too: the index has no such document.
        assert_eq!(
            fetch_doc_with(990_002, ask(IndexDoc::Absent)),
            IndexDoc::Absent
        );
        assert_eq!(
            fetch_doc_with(990_002, ask(IndexDoc::Unread)),
            IndexDoc::Absent
        );
        assert_eq!(asked.get(), 4);
        assert_eq!(cached(990_002), None, "no document to tell");
    }

    /// The live index, for one chain.
    #[test]
    #[ignore = "reaches the ethereum-data index"]
    fn gnosis_answers_with_stablecoins_and_a_wrapped_coin() {
        crate::executor::storage::tests::with_temp_state("chain-tokens-live", || {
            invalidate();
            let data = fetch_doc(100)
                .data()
                .cloned()
                .unwrap_or_else(|| unreachable!("no index document"));
            println!(
                "  chain 100: {} / {} stables / wrapped {:?}",
                data.native_symbol,
                data.stables.len(),
                data.wrapped_native
            );
            for token in &data.stables {
                println!("    {} ({}) {}", token.symbol, token.kind, token.contract);
            }
            assert!(!data.stables.is_empty(), "Gnosis has stablecoins");
            assert!(data.wrapped_native.is_some(), "Gnosis wraps xDAI");
            // A second read must not go out again.
            let again = fetch_doc(100).data().cloned();
            assert_eq!(again.as_ref(), Some(&data));
        });
    }
}
