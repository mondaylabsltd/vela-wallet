//! The Ethereum-mainnet Chainlink feeds — the last rung of the price ladder.
//!
//! **Ported from** `src/services/price-service.ts` @ `e85febf9` (FR-006).
//!
//! One `aggregate3` on chain 1 reads every native coin's USD feed at once. It is
//! the rung below a DEX quote and below the chain's own local feed, and it
//! exists because a chain with no liquid pool and no local feed still has a coin
//! somebody holds.
//!
//! **These are proxy addresses.** The aggregator behind each one is replaced
//! over time; the proxy is not, which is why hard-coding them is safe and
//! hard-coding an aggregator would not be.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::executor::abi::{self, Call3};
use crate::executor::pool;

/// Feeds live on Ethereum mainnet; this is the chain they are read from.
const FEED_CHAIN: u32 = 1;
const CACHE_TTL: Duration = Duration::from_secs(3 * 60);

/// Symbol → feed proxy on Ethereum mainnet. All USD feeds, all 8 decimals.
const FEEDS: &[(&str, &str)] = &[
    ("ETH", "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
    // MEASURED DEAD, 2026-09-04: `eth_getCode` at this address on Ethereum
    // mainnet answers `0x`, so `latestRoundData()` returns nothing and BNB
    // never appears in the map. Kept verbatim because it is the ported table
    // (FR-006) and because BNB is not left unpriced by it: chain 56's OWN feed
    // (`NATIVE_CHAINLINK_FEEDS`) answers, and that is the rung above this one.
    // What is lost is the fallback, on the day BSC's local feed also fails.
    ("BNB", "0x14e613AC691a42F21B17a6Dc7232f070FF175d25"),
    ("MATIC", "0x7bAC85A8a13A4BcD8abb3eB7d6b4d632c5a57676"),
    ("AVAX", "0xFF3EEb22B5E3dE6e705b44749C2559d704923FD7"),
    ("DAI", "0xAed0c38402a5d19df6E4c03F4E2DceD6e29c1ee9"),
];

/// Where the wallet's name for a coin differs from Chainlink's feed key.
/// Polygon's coin is POL and its feed is still MATIC; Gnosis's is xDAI and its
/// feed is DAI.
const ALIASES: &[(&str, &str)] = &[("POL", "MATIC"), ("XDAI", "DAI")];

type Cache = Mutex<Option<(HashMap<String, f64>, Instant)>>;

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// Drop the cached prices — after a state directory is swapped underneath a
/// test, or an endpoint is edited.
pub fn invalidate() {
    if let Ok(mut cache) = cache().lock() {
        *cache = None;
    }
}

/// Every feed's USD price, from cache or from one batched read.
///
/// A failed read answers the **last good map** rather than an empty one: these
/// prices move by fractions of a percent in three minutes, and a stale ETH price
/// is a far better answer than no price at all — the alternative is a portfolio
/// total that drops the coin entirely.
#[must_use]
pub fn prices() -> HashMap<String, f64> {
    if let Ok(cache) = cache().lock() {
        if let Some((prices, at)) = cache.as_ref() {
            if at.elapsed() < CACHE_TTL {
                return prices.clone();
            }
        }
    }

    let calls: Vec<Call3> = FEEDS
        .iter()
        .map(|(_, feed)| Call3 {
            target: (*feed).to_owned(),
            call_data: abi::enc_latest_round(),
        })
        .collect();
    let data = abi::enc_aggregate3(&calls);
    let answered = pool::call(
        FEED_CHAIN,
        "eth_call",
        json!([{ "to": abi::MULTICALL3, "data": data }, "latest"]),
    )
    .ok()
    .and_then(|body| {
        body.get("result")
            .and_then(Value::as_str)
            .map(str::to_owned)
    });

    let Some(raw) = answered else {
        return last_good();
    };
    let results = abi::dec_aggregate3(&raw);
    if results.len() != FEEDS.len() {
        return last_good();
    }

    let mut prices = HashMap::new();
    for (index, (symbol, _)) in FEEDS.iter().enumerate() {
        let Some(result) = results.get(index) else {
            continue;
        };
        if !result.success {
            continue;
        }
        if let Some(usd) = abi::dec_chainlink_usd(&result.data) {
            prices.insert((*symbol).to_owned(), usd);
        }
    }
    // A batch where every feed reverted is a bad read, not a world without
    // prices. Keeping the previous map is the difference between a wallet that
    // is a few minutes stale and one that says a coin is worthless.
    if prices.is_empty() {
        return last_good();
    }
    if let Ok(mut cache) = cache().lock() {
        *cache = Some((prices.clone(), Instant::now()));
    }
    prices
}

fn last_good() -> HashMap<String, f64> {
    cache()
        .lock()
        .ok()
        .and_then(|cache| cache.as_ref().map(|(prices, _)| prices.clone()))
        .unwrap_or_default()
}

/// One coin's price out of the map, through the aliases.
#[must_use]
pub fn resolve(native_symbol: &str, prices: &HashMap<String, f64>) -> Option<f64> {
    let upper = native_symbol.to_uppercase();
    // A chain whose gas IS a stablecoin (Tempo's "USD") is pegged, and there is
    // no feed for it because there is nothing to measure.
    if upper == "USD" {
        return Some(1.0);
    }
    if let Some(price) = prices.get(&upper) {
        return Some(*price);
    }
    ALIASES
        .iter()
        .find(|(from, _)| *from == upper)
        .and_then(|(_, to)| prices.get(*to))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, f64)]) -> HashMap<String, f64> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
    }

    /// The aliases, and the peg that has no feed.
    #[test]
    fn a_coin_resolves_through_its_alias_and_a_stable_gas_coin_is_one_dollar() {
        let prices = map(&[("ETH", 3_000.0), ("MATIC", 0.42), ("DAI", 1.0)]);
        assert_eq!(resolve("ETH", &prices), Some(3_000.0));
        // Polygon's coin is POL; its feed is still MATIC.
        assert_eq!(resolve("POL", &prices), Some(0.42));
        // Gnosis's coin is xDAI, and case must not decide.
        assert_eq!(resolve("xDAI", &prices), Some(1.0));
        assert_eq!(resolve("XDAI", &prices), Some(1.0));
        // Tempo's gas is a stablecoin — pegged, with nothing to read.
        assert_eq!(resolve("USD", &HashMap::new()), Some(1.0));
        // And a coin with no feed is None, never a substituted 1.
        assert_eq!(resolve("MON", &prices), None);
    }

    /// The live batch.
    #[test]
    #[ignore = "reads Ethereum mainnet"]
    fn the_mainnet_feeds_answer_with_plausible_prices() {
        crate::executor::storage::tests::with_temp_state("chainlink-live", || {
            invalidate();
            let prices = prices();
            for (symbol, usd) in &prices {
                println!("  {symbol} = ${usd:.2}");
            }
            let eth = prices
                .get("ETH")
                .copied()
                .unwrap_or_else(|| unreachable!("no ETH feed: {prices:?}"));
            // A band, not a figure. The price moves; what must hold is that a
            // decode error cannot pass for a price.
            assert!(
                eth > 100.0 && eth < 100_000.0,
                "implausible ETH price {eth}"
            );
            let dai = prices
                .get("DAI")
                .copied()
                .unwrap_or_else(|| unreachable!("no DAI feed"));
            assert!(dai > 0.9 && dai < 1.1, "DAI is pegged; got {dai}");
        });
    }
}
