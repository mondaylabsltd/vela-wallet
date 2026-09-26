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

use vela_core::app::fee_policy::{FeeBundlerQuote, FeeCall, FeeGasOutcome, FeeTier};

use crate::executor::chain::{self, RawGasSignals};
use crate::executor::relay;
use crate::executor::single_flight::SingleFlight;

/// The core's window (`fee_policy::FEE_SIGNALS_CACHE_TTL_MS`), the one every
/// shell's fee-signal cache holds a reading for.
const TTL: Duration =
    Duration::from_millis(vela_core::app::fee_policy::FEE_SIGNALS_CACHE_TTL_MS as u64);

struct Held<T> {
    at: Instant,
    value: T,
}

/// A cache: absent until first used, then one entry per key.
type Cache<K, T> = Mutex<Option<HashMap<K, Held<T>>>>;

/// Keyed by chain and whether the tip was asked for.
static GAS: Cache<(u32, bool), RawGasSignals> = Mutex::new(None);
/// Keyed by the relay's tier name: `FeeTier` is not `Hash`, and a cache is no
/// reason to widen a wire type's derives.
static QUOTES: Cache<(u32, String), FeeBundlerQuote> = Mutex::new(None);
static EPOCH: Mutex<Option<HashMap<u32, u64>>> = Mutex::new(None);

/// One exact operation: the chain, the account, whether it is deployed, and
/// every call byte for byte.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SimKey {
    chain_id: u32,
    account: String,
    deployed: bool,
    calls: Vec<(String, String, String)>,
}

static SIMULATIONS: Cache<SimKey, FeeGasOutcome> = Mutex::new(None);

const SIMULATION_TTL: Duration =
    Duration::from_millis(vela_core::app::fee_policy::SIMULATION_CACHE_TTL_MS as u64);

fn epoch(chain_id: u32) -> u64 {
    EPOCH
        .lock()
        .ok()
        .and_then(|map| map.as_ref().and_then(|m| m.get(&chain_id).copied()))
        .unwrap_or(0)
}

fn fresh<K: std::hash::Hash + Eq, T: Clone>(cache: &Cache<K, T>, key: &K) -> Option<T> {
    fresh_for(cache, key, TTL)
}

fn fresh_for<K: std::hash::Hash + Eq, T: Clone>(
    cache: &Cache<K, T>,
    key: &K,
    ttl: Duration,
) -> Option<T> {
    let guard = cache.lock().ok()?;
    let held = guard.as_ref()?.get(key)?;
    (held.at.elapsed() < ttl).then(|| held.value.clone())
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

/// `eth_gasPrice` ∥ the latest block's base fee ∥ the tip, raw — held 15 s,
/// and read once for every session asking at the same moment.
pub fn gas_signals(chain_id: u32, want_tip: bool) -> RawGasSignals {
    if let Some(signals) = fresh(&GAS, &(chain_id, want_tip)) {
        return signals;
    }
    static IN_FLIGHT: SingleFlight<(u32, bool), RawGasSignals> = SingleFlight::new();
    IN_FLIGHT.run((chain_id, want_tip), || {
        let started = epoch(chain_id);
        let (signals, complete) = chain::read_gas_signals(chain_id, want_tip);
        if complete && epoch(chain_id) == started {
            keep(&GAS, (chain_id, want_tip), signals.clone());
        }
        signals
    })
}

/// One tier of the relay's gas price, raw — held 15 s. `None` and a zero cap
/// ("the relay did not answer", which the core rejects as degenerate) are
/// never held.
///
/// The relay answers every tier in one response, so one read fills every
/// tier's row: the session for the speed in force and the previews for the
/// other two share it, and the three rows settle together instead of one
/// request apiece landing one after another.
pub fn bundler_quote(chain_id: u32, tier: FeeTier) -> Option<FeeBundlerQuote> {
    let wanted = relay::tier_name(tier);
    if let Some(quote) = fresh(&QUOTES, &(chain_id, wanted.to_owned())) {
        return Some(quote);
    }
    static IN_FLIGHT: SingleFlight<u32, Option<Vec<(&'static str, FeeBundlerQuote)>>> =
        SingleFlight::new();
    let rows = IN_FLIGHT.run(chain_id, || {
        let started = epoch(chain_id);
        let rows = relay::raw_bundler_quotes(chain_id);
        if epoch(chain_id) == started {
            for (name, quote) in rows.iter().flatten() {
                if vela_core::app::fee_policy::bundler_quote_cacheable(&quote.max_fee_per_gas) {
                    keep(&QUOTES, (chain_id, (*name).to_owned()), quote.clone());
                }
            }
        }
        rows
    })?;
    rows.into_iter()
        .find_map(|(name, quote)| (name == wanted).then_some(quote))
}

/// The relay's simulation of one exact operation, held
/// `fee_policy::SIMULATION_CACHE_TTL_MS` and read once for every session
/// asking at the same moment. Nothing the simulation measures depends on the
/// speed, so the speed in force and both previews price the same operation
/// off ONE simulation, settled together. Only a real estimate is held — a
/// refusal or a missing context is asked again next time.
pub fn simulation(
    chain_id: u32,
    account: &str,
    deployed: bool,
    calls: &[FeeCall],
    simulate: impl FnOnce() -> FeeGasOutcome,
) -> FeeGasOutcome {
    let key = SimKey {
        chain_id,
        account: account.to_lowercase(),
        deployed,
        calls: calls
            .iter()
            .map(|call| (call.to.to_lowercase(), call.value.clone(), call.data.to_lowercase()))
            .collect(),
    };
    if let Some(outcome) = fresh_for(&SIMULATIONS, &key, SIMULATION_TTL) {
        return outcome;
    }
    static IN_FLIGHT: SingleFlight<SimKey, FeeGasOutcome> = SingleFlight::new();
    IN_FLIGHT.run(key.clone(), || {
        let started = epoch(chain_id);
        let outcome = simulate();
        if matches!(outcome, FeeGasOutcome::Estimated { .. }) && epoch(chain_id) == started {
            keep(&SIMULATIONS, key, outcome.clone());
        }
        outcome
    })
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
    if let Ok(mut guard) = SIMULATIONS.lock()
        && let Some(map) = guard.as_mut()
    {
        map.retain(|key, _| key.chain_id != chain_id);
    }
}
