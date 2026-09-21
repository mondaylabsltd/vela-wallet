//! The fee's inputs, held still for 15 s (issue 212; the desktop's since 069).
//!
//! The fee session re-samples on every quote run — the warm-up, the form, a
//! recipient edit, Max, the Continue pre-check — and since spec 069 up to
//! three sessions price the same send at once, one per speed. Uncached, each
//! of those reads the chain again: editing the RECIPIENT re-rolled the gas
//! price, and three tiers priced on three different readings could not be
//! compared at all. So a measurement is good for 15 s per chain (the window
//! `chain_gas_price` has always used), exactly as the web's
//! `fetchRawGasSignals` / `fetchRawBundlerQuote` hold theirs, and:
//!
//! - only a REAL, COMPLETE measurement is kept — a failed or zero
//!   `eth_gasPrice`, a missing block or tip leg, or an absent or zero relay
//!   quote is never pinned, so one hiccup cannot own the next 15 seconds;
//! - [`invalidate`] is called by the explicit refresh, by a quote that
//!   settled as failed, and at the start of every submit, so each of those
//!   measures again;
//! - an invalidate bumps the chain's epoch, so a read that was already in
//!   flight when somebody asked for a fresh one cannot land its older answer
//!   in the cache.
//!
//! Nothing here judges a number: what the core is handed is still the raw
//! reading, and every rule about it stays `fee_policy`'s.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use vela_core::app::fee_policy::{FeeBundlerQuote, FeeTier};

use crate::executor::chain::{self, RawGasSignals};
use crate::executor::relay;

/// The same window as `chain_gas_price` and the web's `FEE_SIGNALS_CACHE_TTL`.
const TTL: Duration = Duration::from_secs(15);

struct Held<T> {
    at: Instant,
    value: T,
}

/// A cache: absent until first used, then one entry per key.
type Cache<K, T> = Mutex<Option<HashMap<K, Held<T>>>>;

/// Keyed by chain and whether the tip was asked for.
static GAS: Cache<(u32, bool), RawGasSignals> = Mutex::new(None);
/// Keyed by the tier's debug name: `FeeTier` is not `Hash`, and a cache is no
/// reason to widen a wire type's derives.
static QUOTES: Cache<(u32, String), FeeBundlerQuote> = Mutex::new(None);
static EPOCH: Mutex<Option<HashMap<u32, u64>>> = Mutex::new(None);

fn epoch(chain_id: u32) -> u64 {
    EPOCH
        .lock()
        .ok()
        .and_then(|map| map.as_ref().and_then(|m| m.get(&chain_id).copied()))
        .unwrap_or(0)
}

fn fresh<K: std::hash::Hash + Eq, T: Clone>(cache: &Cache<K, T>, key: &K) -> Option<T> {
    let guard = cache.lock().ok()?;
    let held = guard.as_ref()?.get(key)?;
    (held.at.elapsed() < TTL).then(|| held.value.clone())
}

fn keep<K: std::hash::Hash + Eq, T>(cache: &Mutex<Option<HashMap<K, Held<T>>>>, key: K, value: T) {
    if let Ok(mut guard) = cache.lock() {
        guard.get_or_insert_with(HashMap::new).insert(
            key,
            Held {
                at: Instant::now(),
                value,
            },
        );
    }
}

/// `eth_gasPrice` ∥ the latest block's base fee ∥ the tip, raw — held 15 s.
pub fn gas_signals(chain_id: u32, want_tip: bool) -> RawGasSignals {
    if let Some(signals) = fresh(&GAS, &(chain_id, want_tip)) {
        return signals;
    }
    let started = epoch(chain_id);
    let (signals, complete) = chain::read_gas_signals(chain_id, want_tip);
    if complete && epoch(chain_id) == started {
        keep(&GAS, (chain_id, want_tip), signals.clone());
    }
    signals
}

/// One tier of the relay's gas price, raw — held 15 s. `None` and a zero cap
/// ("the relay did not answer", which the core rejects as degenerate) are
/// never held.
pub fn bundler_quote(chain_id: u32, tier: FeeTier) -> Option<FeeBundlerQuote> {
    let key = (chain_id, format!("{tier:?}"));
    if let Some(quote) = fresh(&QUOTES, &key) {
        return Some(quote);
    }
    let started = epoch(chain_id);
    let quote = relay::raw_bundler_quote(chain_id, tier);
    if let Some(quote) = &quote
        && quote
            .max_fee_per_gas
            .parse::<u128>()
            .is_ok_and(|cap| cap > 0)
        && epoch(chain_id) == started
    {
        keep(&QUOTES, key, quote.clone());
    }
    quote
}

/// Forget this chain's held readings, so the next quote run measures again.
pub fn invalidate(chain_id: u32) {
    if let Ok(mut map) = EPOCH.lock() {
        *map.get_or_insert_with(HashMap::new)
            .entry(chain_id)
            .or_insert(0) += 1;
    }
    if let Ok(mut guard) = GAS.lock()
        && let Some(map) = guard.as_mut()
    {
        map.retain(|(chain, _), _| *chain != chain_id);
    }
    if let Ok(mut guard) = QUOTES.lock()
        && let Some(map) = guard.as_mut()
    {
        map.retain(|(chain, _), _| *chain != chain_id);
    }
}
