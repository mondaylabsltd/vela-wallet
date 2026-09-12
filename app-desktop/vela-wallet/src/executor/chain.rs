//! The four chain reads the submit path needs, through the pool.
//!
//! **Ported from** the RPC helpers of
//! `app-web/vela-wallet/src/lib/services/safe-transaction.ts` @ `origin/main`
//! (`isDeployed`, `getNonce`, `incrementNonceCache`, `fetchRawGasSignals`,
//! `verifyChainReady`). Small, and load-bearing in one specific way each:
//!
//! - **Deployment status is correctness-critical.** It decides whether the
//!   operation carries `initCode`. A transient failure guessed as "deployed"
//!   ships an empty initCode for a fresh account (AA20, every new user's first
//!   send fails); guessed as "undeployed" it attaches initCode to a live one
//!   (AA10). Neither guess is safe, so an INDETERMINATE read is an error the
//!   caller retries, and only a definitive answer is trusted.
//! - **A nonce failure is not permission to sign nonce 0.** A deployed wallet
//!   that signed `0x0` would burn a passkey prompt on an op the relay rejects
//!   (AA25). The error is typed so the caller can fail before signing.
//! - **The gas signals are handed over raw.** The tip-inclusive price rule
//!   (`derive_chain_gas_price`) is `fee_policy`'s; feeding it a derived value
//!   would apply the rule twice.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::fee_policy::{
    ChainGasPrice, GasSignals, derive_chain_gas_price, is_tempo_chain,
};
use vela_core::primitives::{abi_encode_address, function_selector, to_hex};
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::parse_hex_quantity;

use crate::executor::pool;

/// `NONCE_CACHE_TTL` (`safe-transaction.ts:2375`).
const NONCE_TTL: Duration = Duration::from_secs(10);
/// `GAS_PRICE_CACHE_TTL` (`safe-transaction.ts:2482`).
const GAS_PRICE_TTL: Duration = Duration::from_secs(15);

const INDETERMINATE_DEPLOYMENT: &str = "Could not verify the account deployment status — the network may be unstable. Please try again.";
const INDETERMINATE_NONCE: &str =
    "Could not fetch the account nonce — the network may be unstable. Please try again.";

/// Once deployed, always deployed (irreversible), so only `true` is cached.
static DEPLOYED: Mutex<Option<HashMap<String, ()>>> = Mutex::new(None);
static NONCES: Mutex<Option<HashMap<String, (String, Instant)>>> = Mutex::new(None);
static GAS_PRICES: Mutex<Option<HashMap<u32, (ChainGasPrice, Instant)>>> = Mutex::new(None);
static CHAIN_READY: Mutex<Option<HashMap<u32, ()>>> = Mutex::new(None);

fn key(chain_id: u32, address: &str) -> String {
    format!("{chain_id}:{}", address.to_lowercase())
}

/// Whether `address` has code (`isDeployed`). `Err` is an INDETERMINATE
/// read — an unreachable chain, an RPC error, or a non-string result — and
/// the caller must retry rather than guess.
pub fn is_deployed(address: &str, chain_id: u32) -> Result<bool, String> {
    let cache_key = key(chain_id, address);
    if let Ok(cache) = DEPLOYED.lock()
        && cache
            .as_ref()
            .is_some_and(|map| map.contains_key(&cache_key))
    {
        return Ok(true);
    }
    let body = pool::call(chain_id, "eth_getCode", json!([address, "latest"]))
        .map_err(|_| INDETERMINATE_DEPLOYMENT.to_owned())?;
    if body.get("error").is_some() {
        return Err(INDETERMINATE_DEPLOYMENT.to_owned());
    }
    let Some(code) = body.get("result").and_then(Value::as_str) else {
        return Err(INDETERMINATE_DEPLOYMENT.to_owned());
    };
    let deployed = code != "0x" && code.len() > 2;
    if deployed && let Ok(mut cache) = DEPLOYED.lock() {
        cache.get_or_insert_with(HashMap::new).insert(cache_key, ());
    }
    Ok(deployed)
}

/// `EntryPoint.getNonce(safe, 0)` (`getNonce`), as the RPC spelled it, cached
/// 10 s. `Err` is a genuine RPC failure: a fresh account answers a valid
/// zero word, never an error.
pub fn nonce(safe: &str, chain_id: u32) -> Result<String, String> {
    let cache_key = key(chain_id, safe);
    if let Ok(cache) = NONCES.lock()
        && let Some((nonce, at)) = cache.as_ref().and_then(|map| map.get(&cache_key))
        && at.elapsed() < NONCE_TTL
    {
        return Ok(nonce.clone());
    }
    let mut data = function_selector("getNonce(address,uint192)").map_err(|e| e.to_string())?;
    data.extend(abi_encode_address(safe).map_err(|e| e.to_string())?);
    data.extend([0u8; 32]);
    let body = pool::call(
        chain_id,
        "eth_call",
        json!([{ "to": ENTRY_POINT, "data": to_hex(&data, true) }, "latest"]),
    )
    .map_err(|_| INDETERMINATE_NONCE.to_owned())?;
    if body.get("error").is_some() {
        return Err(INDETERMINATE_NONCE.to_owned());
    }
    let Some(nonce) = body.get("result").and_then(Value::as_str) else {
        return Err(INDETERMINATE_NONCE.to_owned());
    };
    if let Ok(mut cache) = NONCES.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(cache_key, (nonce.to_owned(), Instant::now()));
    }
    Ok(nonce.to_owned())
}

/// `incrementNonceCache`: after a submit, so a concurrent send does not
/// reuse the nonce. A missing or stale entry is left for the next read.
pub fn bump_nonce(safe: &str, chain_id: u32) {
    let cache_key = key(chain_id, safe);
    if let Ok(mut cache) = NONCES.lock()
        && let Some(map) = cache.as_mut()
        && let Some((nonce, _)) = map.get(&cache_key)
        && let Ok(current) = parse_hex_quantity(Some(nonce))
    {
        map.insert(
            cache_key,
            (format!("0x{:x}", current.saturating_add(1)), Instant::now()),
        );
    }
}

/// The three chain price signals, raw, as decimal strings (`fetchRawGasSignals`).
/// `None` is a failed or skipped read; only the core turns an absent
/// `eth_gasPrice` into the 5-gwei default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawGasSignals {
    pub eth_gas_price: Option<String>,
    pub base_fee: Option<String>,
    pub priority_fee: Option<String>,
}

fn quantity_text(value: Option<&Value>) -> Option<String> {
    parse_hex_quantity(Some(value?.as_str()?))
        .ok()
        .map(|n| n.to_string())
}

pub fn raw_gas_signals(chain_id: u32, want_tip: bool) -> RawGasSignals {
    let gas_price = pool::call(chain_id, "eth_gasPrice", json!([])).ok();
    let block = pool::call(chain_id, "eth_getBlockByNumber", json!(["latest", false])).ok();
    let tip = want_tip
        .then(|| pool::call(chain_id, "eth_maxPriorityFeePerGas", json!([])).ok())
        .flatten();
    RawGasSignals {
        eth_gas_price: quantity_text(gas_price.as_ref().and_then(|b| b.get("result"))),
        base_fee: quantity_text(
            block
                .as_ref()
                .and_then(|b| b.pointer("/result/baseFeePerGas")),
        ),
        priority_fee: quantity_text(tip.as_ref().and_then(|b| b.get("result"))),
    }
}

/// `getGasPrices`: the derived network price, cached 15 s, with the 5-gwei
/// default when even `eth_gasPrice` is absent. Used by the send-time
/// fallback quote and Tempo's reimbursement; the confirm-screen quote is
/// `fee_policy`'s and reads the raw signals instead.
pub fn chain_gas_price(chain_id: u32) -> ChainGasPrice {
    if let Ok(cache) = GAS_PRICES.lock()
        && let Some((price, at)) = cache.as_ref().and_then(|map| map.get(&chain_id))
        && at.elapsed() < GAS_PRICE_TTL
    {
        return *price;
    }
    let want_tip = !is_tempo_chain(chain_id);
    let signals = raw_gas_signals(chain_id, want_tip);
    let parse = |text: &Option<String>| text.as_deref().and_then(|t| t.parse::<u128>().ok());
    let derived = match parse(&signals.eth_gas_price) {
        Some(eth_gas_price) => {
            let tip_measured = want_tip && signals.priority_fee.is_some();
            let derived = derive_chain_gas_price(&GasSignals {
                eth_gas_price,
                base_fee: parse(&signals.base_fee).unwrap_or(0),
                priority_fee: parse(&signals.priority_fee).unwrap_or(0),
                tip_measured: Some(tip_measured),
            });
            if derived.gas_price > 0 {
                Some(derived)
            } else {
                None
            }
        }
        None => None,
    };
    let price = derived.unwrap_or(ChainGasPrice {
        gas_price: 5_000_000_000,
        base_fee: 5_000_000_000,
        priority_fee: 0,
        tip_measured: false,
    });
    if let Ok(mut cache) = GAS_PRICES.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(chain_id, (price, Instant::now()));
    }
    price
}

/// Forget the cached price before a submit — stale prices on a volatile
/// chain are "gas price too low" rejections (`_gasPriceCache.delete`).
pub fn forget_gas_price(chain_id: u32) {
    if let Ok(mut cache) = GAS_PRICES.lock()
        && let Some(map) = cache.as_mut()
    {
        map.remove(&chain_id);
    }
}

/// `verifyChainReady`: the EntryPoint exists here, once per chain.
pub fn verify_chain_ready(chain_id: u32) -> Result<(), String> {
    if let Ok(cache) = CHAIN_READY.lock()
        && cache
            .as_ref()
            .is_some_and(|map| map.contains_key(&chain_id))
    {
        return Ok(());
    }
    if !is_deployed(ENTRY_POINT, chain_id)? {
        return Err("This network is not ready yet. Required smart contracts (EntryPoint) are not deployed. Please activate this network in Settings → Transaction Services.".to_owned());
    }
    if let Ok(mut cache) = CHAIN_READY.lock() {
        cache.get_or_insert_with(HashMap::new).insert(chain_id, ());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bumped_nonce_is_one_more_and_stays_hex() {
        if let Ok(mut cache) = NONCES.lock() {
            cache.get_or_insert_with(HashMap::new).insert(
                key(31337, "0xAbC"),
                (
                    "0x0000000000000000000000000000000000000000000000000000000000000007".to_owned(),
                    Instant::now(),
                ),
            );
        }
        bump_nonce("0xabc", 31337);
        let now = NONCES.lock().ok().and_then(|cache| {
            cache
                .as_ref()
                .and_then(|map| map.get(&key(31337, "0xabc")).cloned())
        });
        assert_eq!(now.map(|(nonce, _)| nonce).as_deref(), Some("0x8"));
        // A chain nobody read is left alone.
        bump_nonce("0xdef", 31337);
    }

    // -- live (`cargo test executor::chain -- --ignored --test-threads=1`) --

    const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    /// The golden Safe is deployed on Gnosis and has a nonce; the EntryPoint
    /// is there; the gas signals carry a tip (Gnosis's whole price).
    #[test]
    #[ignore = "real network"]
    fn live_the_golden_safe_reads_on_gnosis() {
        verify_chain_ready(100).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(is_deployed(GOLDEN, 100).unwrap_or_else(|e| unreachable!("{e}")));
        let nonce = nonce(GOLDEN, 100).unwrap_or_else(|e| unreachable!("{e}"));
        println!("golden nonce: {nonce}");
        assert!(parse_hex_quantity(Some(&nonce)).is_ok());
        let signals = raw_gas_signals(100, true);
        println!("gas signals: {signals:?}");
        assert!(signals.eth_gas_price.is_some());
        let price = chain_gas_price(100);
        println!("derived: {price:?}");
        assert!(price.gas_price > 0);
    }
}
