//! The only place the `balance_dashboard` machine touches the outside world.
//!
//! Seven operations: the multi-chain fetch (delegated whole to
//! [`crate::executor::balances`]), the 24-hour total cache, a retry timer and
//! the privacy flag.
//!
//! ## The TTL is the shell's, the write gate is the core's
//!
//! The core's own words on `ReadBalanceCache`: "The shell applies the 24h TTL —
//! absent or expired answers `None`." And on `WriteBalanceCache`: "The CORE
//! decides when this may happen — the complete-results-only write gate."
//!
//! That split is worth respecting exactly. Caching a total assembled from a
//! partial fetch is how a wallet remembers a number that was never true; the
//! core refuses to ask for that write, and this file never writes uninvited.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;
use serde_json::{Value, json};

use vela_core::app::balance_dashboard::{
    AUTO_REFRESH_MS, BalanceCacheEntry, BalanceDashboard, BalanceOperation, BalanceShellResult,
    BalanceToken, Event,
};

use crate::executor::{balances, pool, storage};
use crate::resident::{self, Answer, Machine};

/// `vela.balanceCache` — `{ address: { usd, at } }`, the Expo bytes.
const CACHE_KEY: &str = "vela.balanceCache";
/// 24 hours (`balance-cache.ts:11`).
const CACHE_TTL_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// `vela.balanceHidden` — `'1'` / `'0'`, best effort.
const PRIVACY_KEY: &str = "vela.balanceHidden";

/// A cached total, if one is present and still inside the TTL.
pub(crate) fn cached_usd(address: &str, now_ms: f64) -> Option<f64> {
    let entry = storage::read_value(CACHE_KEY).ok().flatten()?;
    let record = entry.get(address)?;
    let at = record.get("at").and_then(Value::as_f64)?;
    // Expired reads as absent, not as stale-but-usable: the hero would rather
    // show a skeleton than a figure from a different day.
    (now_ms - at <= CACHE_TTL_MS).then(|| record.get("usd").and_then(Value::as_f64))?
}

// ---------------------------------------------------------------------------
// The rows the home last showed (`holdings-cache.ts`, home-1/home-3)
// ---------------------------------------------------------------------------

/// `vela.holdingsCache` — `{ "<address>#<space>": { at, tokens } }`, the
/// web's bytes. Unlike the total, the testnet space keeps its rows too: they
/// are coins, not a value.
pub const HOLDINGS_KEY: &str = "vela.holdingsCache";

fn holdings_key(address: &str, space: &str) -> String {
    format!("{}#{space}", address.trim().to_lowercase())
}

/// Keep the rows a settled round left for `address` in `space` — told back
/// once ([`Event::HoldingsRestored`]) the next time the account or space is
/// in front, so the Assets list opens on what was last known instead of a
/// skeleton that lasts as long as the slowest network. Best effort.
pub fn save_holdings(address: &str, space: &str, tokens: &[BalanceToken], now_ms: f64) {
    let mut map = match storage::read_value(HOLDINGS_KEY) {
        Ok(Some(Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    };
    map.insert(
        holdings_key(address, space),
        json!({ "at": now_ms, "tokens": tokens }),
    );
    let _ = storage::write_value(HOLDINGS_KEY, Value::Object(map));
}

/// Every stored entry's rows, as the core reads them: a row only in its
/// shape, an entry only with its time.
fn stored_holdings() -> Vec<(String, f64, Vec<BalanceToken>)> {
    let Ok(Some(Value::Object(map))) = storage::read_value(HOLDINGS_KEY) else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(key, entry)| {
            let at = entry.get("at").and_then(Value::as_f64)?;
            let tokens = entry
                .get("tokens")
                .and_then(Value::as_array)?
                .iter()
                .filter_map(|row| serde_json::from_value::<BalanceToken>(row.clone()).ok())
                .collect();
            Some((key, at, tokens))
        })
        .collect()
}

/// The rows last kept for `address` in `space`, or none when there are none
/// or they are a day old.
#[must_use]
pub fn load_holdings(address: &str, space: &str, now_ms: f64) -> Vec<BalanceToken> {
    let wanted = holdings_key(address, space);
    stored_holdings()
        .into_iter()
        .find(|(key, _, _)| *key == wanted)
        .filter(|(_, at, _)| now_ms - at <= CACHE_TTL_MS)
        .map(|(_, _, tokens)| tokens)
        .unwrap_or_default()
}

/// Networks where a last-known holding is more than zero — any account,
/// either space, however old the reading; a spam token is not money. The
/// balances' evidence for `shown_networks::settle`.
#[must_use]
pub fn held_chain_ids() -> Vec<u32> {
    let mut ids: Vec<u32> = Vec::new();
    for (_, _, tokens) in stored_holdings() {
        for token in tokens {
            if !token.spam
                && token.balance.bytes().any(|b| (b'1'..=b'9').contains(&b))
                && !ids.contains(&token.chain_id)
            {
                ids.push(token.chain_id);
            }
        }
    }
    ids
}

/// [`Event::HoldingsRestored`] for `address` in the space in front, when
/// rows were kept for it.
fn restored_holdings(address: &str) -> Option<Event> {
    if address.trim().is_empty() {
        return None;
    }
    let space = crate::network_space::current();
    let tokens = load_holdings(address, space.as_str(), crate::executor::now_ms());
    (!tokens.is_empty()).then(|| Event::HoldingsRestored {
        address: address.to_owned(),
        tokens,
    })
}

pub(crate) fn write_cached_usd(address: &str, usd: f64, now_ms: f64) {
    let mut map = match storage::read_value(CACHE_KEY) {
        Ok(Some(Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    };
    map.insert(address.to_owned(), json!({ "usd": usd, "at": now_ms }));
    let _ = storage::write_value(CACHE_KEY, Value::Object(map));
}

/// One address's last settled round.
#[derive(Clone, Debug)]
pub struct Settled {
    /// Every token the round read — zero balances included (the core drops
    /// those from the display, not from the list), and the rows carried over
    /// for chains that did not answer.
    pub tokens: Vec<BalanceToken>,
    /// The chains that did not answer.
    pub failed_chain_ids: Vec<u32>,
    /// Which round this is: rises by one at every settle, for any address.
    pub round: u64,
    pub at_ms: f64,
}

/// The last settled round per address (lower-cased) — what a chain that
/// does not answer falls back to. The web's in-memory `tokenCache`.
static LAST_SETTLED: Mutex<Option<HashMap<String, Settled>>> = Mutex::new(None);
static ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The previous snapshot's holdings on chains that did not answer, kept
/// alongside the ones that did (issue #196, the web's
/// `carryOverUnansweredChains`, commit 72dee30a).
///
/// The core's streaming merge already holds a chain's last value while it is
/// in flight, but the settled list replaces everything — so one round in
/// which Gnosis timed out removed its tokens from the Assets list and their
/// value from the total. A chain that ANSWERED stays authoritative: tokens it
/// no longer reports really have been spent. The failed chains are still
/// reported to the core, which keeps treating the round as partial and never
/// promotes a total carrying stale holdings to last-known-good.
fn carry_over_unanswered(
    previous: &[BalanceToken],
    mut fresh: Vec<BalanceToken>,
    failed_chain_ids: &[u32],
) -> Vec<BalanceToken> {
    let carried: Vec<BalanceToken> = previous
        .iter()
        .filter(|token| failed_chain_ids.contains(&token.chain_id))
        .cloned()
        .collect();
    if carried.is_empty() {
        return fresh;
    }
    fresh.extend(carried);
    // The fetch's own deterministic order, so a carried row sits where it
    // would have had its chain answered.
    fresh.sort_by(|a, b| {
        a.chain_id
            .cmp(&b.chain_id)
            .then_with(|| a.token_address.is_some().cmp(&b.token_address.is_some()))
            .then_with(|| a.symbol.cmp(&b.symbol))
    });
    fresh
}

/// What this address last settled to — empty for one never fetched in this
/// run. Read at the START of a fetch, so switching back to an account shows
/// what it held a moment ago instead of an empty strip while every chain is
/// asked again (the web's `tokenCache`, which answers the same switch from
/// memory).
fn last_settled(address: &str) -> Vec<BalanceToken> {
    settled(address)
        .map(|round| round.tokens)
        .unwrap_or_default()
}

/// This address's last settled round, or `None` for one not settled in this
/// run.
///
/// The send picker's list (`SendHost::answer_tokens`): the very round the
/// Assets column was drawn from, whole — zero balances included, because a
/// locked payment request is matched against every token read. One fetch,
/// two screens; the picker used to run a second fan-out of its own, which
/// kept it blank for as long as the slowest chain took and could disagree
/// with the column beside it.
///
/// Only the space in front's holdings (spec 105): a round read before a
/// switch stands in for none of the other space's.
pub fn settled(address: &str) -> Option<Settled> {
    let mut round = LAST_SETTLED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .and_then(|snapshots| snapshots.get(&address.to_lowercase()).cloned())?;
    round
        .tokens
        .retain(|token| crate::network_space::in_space(token.chain_id));
    Some(round)
}

/// Settle one round against the last one for this address, and remember it.
///
/// r15-4: the first round of a run has no round of this run to fall back
/// to, and `LAST_SETTLED` lives in memory only — so a chain that failed
/// then lost its rows from the settle, and from the holdings cache saved
/// from it: a refused Arc on the first round after a launch wrote Dana's
/// 10.53 USDC out of `vela.holdingsCache`, and the next launch claimed an
/// empty wallet. `kept` is what the disk last kept for this account (the
/// rows the core was told at boot, `HoldingsRestored`), asked only when no
/// round of this run settled — the same rule the core's own settle keeps
/// for a failed chain's restored rows.
fn settle_with_carry_over(
    address: &str,
    fresh: Vec<BalanceToken>,
    failed_chain_ids: &[u32],
    kept: impl FnOnce() -> Vec<BalanceToken>,
) -> Vec<BalanceToken> {
    let key = address.to_lowercase();
    let mut guard = LAST_SETTLED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let snapshots = guard.get_or_insert_with(HashMap::new);
    let from_disk;
    let previous = match snapshots.get(&key) {
        Some(round) => round.tokens.as_slice(),
        None if failed_chain_ids.is_empty() => &[][..],
        None => {
            from_disk = kept();
            from_disk.as_slice()
        }
    };
    let tokens = carry_over_unanswered(previous, fresh, failed_chain_ids);
    snapshots.insert(
        key,
        Settled {
            tokens: tokens.clone(),
            failed_chain_ids: failed_chain_ids.to_vec(),
            round: ROUND.fetch_add(1, Ordering::SeqCst) + 1,
            at_ms: crate::executor::now_ms(),
        },
    );
    tokens
}

/// One round's answer for the core: what was read, the chains that did not
/// answer — and of those, the ones the pool saw only rate limits on, the
/// ones whose read never left the app (PR 2 note 11), and the ones whose
/// token list could not be loaded while their node answered (PR 3 note 4).
fn settled_result(
    address: &str,
    pull: bool,
    read_chain_ids: Vec<u32>,
    fetched: balances::Fetched,
) -> BalanceShellResult {
    let balances::Fetched {
        tokens,
        failed,
        internal,
        registry,
    } = fetched;
    // A chain that did not answer keeps what the last round knew it held;
    // it is still reported as failed below.
    let space = crate::network_space::current();
    let tokens = settle_with_carry_over(address, tokens, &failed, || {
        load_holdings(address, space.as_str(), crate::executor::now_ms())
    });
    // The rows this round leaves on the home, kept for the next visit
    // (home-1, home-3): what the space in front holds, more than nothing.
    let shown: Vec<BalanceToken> = tokens
        .iter()
        .filter(|token| crate::network_space::in_space(token.chain_id))
        .filter(|token| token.balance.bytes().any(|b| (b'1'..=b'9').contains(&b)))
        .cloned()
        .collect();
    save_holdings(address, space.as_str(), &shown, crate::executor::now_ms());
    // r15-15: the pool's verdicts are asked within a budget. A pool thread
    // held up (every worker busy under a refused or rate-limiting network,
    // a backoff slept inline) used to hold this settle — and the hero's
    // "Checking…" — for as long as its backlog lasted, long past the
    // round's own deadline. Unanswered, a failed chain is said as failed
    // (not "busy"), and the pool's word lands with the next round.
    let budget = Duration::from_millis(pool::SETTLE_ASK_MS);
    let limited = if failed.iter().all(|chain| internal.contains(chain)) {
        // Nothing left the app: the pool has no verdict to give, and asking
        // it would be one more call into what just failed.
        Vec::new()
    } else {
        pool::rate_limited_chains_within(budget).unwrap_or_else(|| {
            crate::diag::vlog!(
                "balances",
                "the pool did not say which networks are rate-limited within {} ms; settling without it",
                pool::SETTLE_ASK_MS
            );
            Vec::new()
        })
    };
    // Owner decision 2026-10-28 (WIRE-AUDIT A8): the failed chains read
    // through the person's own node alone, and the chains public nodes served
    // because theirs is not answering — the pool's lists, as the web's
    // `getOwnNodeUnreachedChains` / `getFallbackChains`.
    // Each chain's standing is the core's one reading of the pool's lists
    // (`rpc_pool::node_standing`).
    // Unanswered within the budget: no chain is said to be read through the
    // person's own node or public nodes this round (the next round says it).
    let own = pool::own_node_chains_within(budget).unwrap_or_default();
    let own_node_chain_ids = failed_own_node(&failed, &own);
    BalanceShellResult::FetchSettled {
        address: address.to_owned(),
        pull,
        tokens,
        // Of the chains that failed, the ones the pool saw only rate limits
        // on: "still loading", not "broken" — nobody is sent to swap RPCs
        // over a busy provider (the web's `getRateLimitedChains`, 078 W-08).
        // A chain whose node answered this round (the token list is what
        // did not) is not one the pool is being throttled on.
        rate_limited_chain_ids: failed
            .iter()
            .copied()
            .filter(|chain| {
                limited.contains(chain) && !internal.contains(chain) && !registry.contains(chain)
            })
            .collect(),
        failed_chain_ids: failed,
        read_chain_ids,
        // Of the chains that failed, the ones whose read never left the app:
        // never "Can't reach …" (`BalanceView::internal_key`).
        internal_chain_ids: internal,
        // Of the chains that failed, the ones whose node answered and whose
        // token list did not (PR 3 note 4): "can't load its token list",
        // and no RPC fix offered for an endpoint that is working.
        registry_chain_ids: registry,
        own_node_chain_ids,
        fallback_chain_ids: own.falling_back(),
        read_at_ms: None,
        now_ms: crate::executor::now_ms(),
    }
}

/// The failed chains that were read through the person's own node alone —
/// those whose standing is `OwnNodeUnreached`, by the core's reading.
fn failed_own_node(failed: &[u32], own: &pool::OwnNodeChains) -> Vec<u32> {
    own.standing_as(
        failed,
        vela_core::app::rpc_pool::NodeStanding::OwnNodeUnreached,
    )
}

impl Machine for BalanceDashboard {
    const LABEL: &'static str = "balance_dashboard";

    fn boot_event(cx: &App) -> Event {
        // The core resets its state and paints the hero from cache on this
        // event, so it is the boot: an address, or the empty string, which is
        // what "no account yet" looks like on the way from Welcome.
        // Whose wallet: the multisig shown, else the session's account.
        let address = crate::multisig::shown::address(cx);
        // perf-10: opened on a screen that draws no balance of its own
        // (Settings, the address book), the account is NOTED — its cached
        // total read for the switcher and the privacy mask — and no network
        // is read until the home is shown ([`shown`]).
        let noted = NOTED_ONLY.load(Ordering::SeqCst) && !address.is_empty();
        BOOTED_NOTED.store(noted, Ordering::SeqCst);
        if noted {
            return Event::AccountNoted { address };
        }
        Event::AccountChanged { address }
    }

    /// The rows the home showed last time for this account in the space in
    /// front (home-1, home-3) — the web's `#restoreRows` after
    /// `account_changed`: the core lets them stand until each chain answers.
    fn boot_followups(cx: &App) -> Vec<Event> {
        // r10-1: the networks each multisig here lives on, so the stamp
        // speaks only for them (`Event::LivesOn`, kept per address).
        let mut out = lives_on_known();
        out.extend(restored_holdings(&crate::multisig::shown::address(cx)));
        out
    }

    fn perform(operation: &BalanceOperation) -> Answer<BalanceShellResult, Self::Event> {
        match operation {
            BalanceOperation::FetchTokens {
                address,
                force,
                pull,
                generation,
            } => {
                let (address, pull, generation) = (address.clone(), *pull, *generation);
                // `force` bypasses a 5-minute shell TTL this cut does not keep:
                // every fetch is live. Recorded rather than silently ignored —
                // adding the TTL later changes no core rule, because the core
                // already tells us when it wants one bypassed.
                let _ = force;
                // Streaming, because this is the one operation the core says
                // streams: "while in flight the shell streams
                // `Event::ChainAssetsArrived` snapshots; the operation itself
                // settles exactly once". `FetchAccountAssets` below is the
                // same fan-out and deliberately does NOT stream — it fills a
                // switcher row for somebody else's account, and its snapshots
                // would be merged into the active one.
                let previous = last_settled(&address);
                Answer::Streaming(Box::new(move |sink| {
                    let sink = sink.clone();
                    // What this account held last time, first: the core merges
                    // it like any chain's snapshot, each chain then replaces
                    // its own rows as it answers, and the settle below stays
                    // the authority. Without it the strip went blank on every
                    // switch back until the first chain answered. Only within
                    // this run — nothing about holdings is written to disk.
                    if !previous.is_empty() {
                        sink.send(Event::ChainAssetsArrived {
                            address: address.clone(),
                            generation,
                            tokens: previous,
                        });
                    }
                    let streamed_for = address.clone();
                    let arrived: std::sync::Arc<balances::ChainSink> =
                        std::sync::Arc::new(move |tokens| {
                            // The fetch's own address and generation ride
                            // along so the core can drop a stream that
                            // belongs to an account (or a space) the person
                            // has already switched away from (its invariant
                            // ⑤; D3).
                            sink.send(Event::ChainAssetsArrived {
                                address: streamed_for.clone(),
                                generation,
                                tokens,
                            });
                        });
                    // Counted while it reads: a "network came back" joins it
                    // rather than starting a second round beside it (G53).
                    let _reading = RoundGuard::enter();
                    // A fault inside the round itself — a panic between the
                    // fan-out and the settle — is the app's, said as that
                    // (PR 2 note 11), never left as a fetch that never ends.
                    // The panic hook has the report for the failure sheet
                    // all the same (spec 038).
                    let settled = crate::panic_report::guarded(|| {
                        let read_chain_ids = balances::chain_ids();
                        let fetched = balances::fetch_all_streaming(&address, &arrived);
                        settled_result(&address, pull, read_chain_ids, fetched)
                    });
                    settled.unwrap_or(BalanceShellResult::FetchErrored {
                        address,
                        pull,
                        internal: true,
                    })
                }))
            }

            BalanceOperation::FetchAccountAssets { address } => {
                let address = address.clone();
                Answer::Blocking(Box::new(move || {
                    let (tokens, failed) = balances::fetch_all(&address);
                    BalanceShellResult::AccountAssetsFetched {
                        address,
                        // Best effort: a row that could not be read keeps its
                        // cached value rather than showing a wrong one.
                        tokens: failed.is_empty().then_some(tokens),
                    }
                }))
            }

            // The saved totals are the mainnet space's (spec 105): a
            // testnet's coins have no value to remember, and the mainnet's
            // never stand in for them.
            BalanceOperation::ReadBalanceCache { address } => {
                Answer::Now(BalanceShellResult::CachedTotalLoaded {
                    address: address.clone(),
                    usd: (!crate::network_space::testnet())
                        .then(|| cached_usd(address, crate::executor::now_ms()))
                        .flatten(),
                })
            }

            BalanceOperation::ReadBalanceCacheMany { addresses } => {
                let now = crate::executor::now_ms();
                let testnet = crate::network_space::testnet();
                Answer::Now(BalanceShellResult::CachedBalancesLoaded {
                    balances: addresses
                        .iter()
                        .filter(|_| !testnet)
                        .filter_map(|address| {
                            cached_usd(address, now).map(|usd| BalanceCacheEntry {
                                address: address.clone(),
                                usd,
                            })
                        })
                        .collect(),
                })
            }

            BalanceOperation::WriteBalanceCache { address, usd } => {
                if !crate::network_space::testnet() {
                    write_cached_usd(address, *usd, crate::executor::now_ms());
                }
                Answer::Now(BalanceShellResult::BalanceCacheWritten)
            }

            BalanceOperation::StartRetryTimer { ms, timer_id } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                BalanceShellResult::RetryElapsed {
                    timer_id: *timer_id,
                },
            ),

            BalanceOperation::WritePrivacy { hidden } => {
                let _ = storage::write_value(
                    PRIVACY_KEY,
                    Value::String(if *hidden { "1" } else { "0" }.to_owned()),
                );
                Answer::Now(BalanceShellResult::PrivacyWritten)
            }
        }
    }
}

/// The person's stored answer to "hide my balance", read at boot.
///
/// `WritePrivacy` has been writing this file since spec 030 and **nothing has
/// ever read it back**: the flag survived a restart on disk and not on screen.
/// First-write-wins is the core's (its invariant ⑧), which is why this is an
/// event rather than a field set before boot.
fn hydrate_privacy() -> Option<Event> {
    let stored = storage::read_value(PRIVACY_KEY).ok().flatten()?;
    let hidden = match &stored {
        Value::String(text) => text == "1",
        Value::Bool(flag) => *flag,
        _ => return None,
    };
    Some(Event::PrivacyHydrated { hidden })
}

static TICKING: AtomicBool = AtomicBool::new(false);
/// Set when something invalidated what the hero is showing, from a place with
/// no `cx` to say so directly (a `perform` runs without one). Drained by the
/// page on its next frame — which is the same frame the change caused.
static INVALIDATED: AtomicBool = AtomicBool::new(false);

/// The network came back (spec 082 RE3) — a reason to read again that a
/// round already reading satisfies, unlike [`INVALIDATED`], whose change (a
/// token added, holdings moved by a landing) a round begun before it may
/// have missed.
static NETWORK_BACK: AtomicBool = AtomicBool::new(false);
/// Balance rounds reading right now.
static ROUNDS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Counts one round while it reads; the drop un-counts it on any exit.
struct RoundGuard(());

impl RoundGuard {
    fn enter() -> Self {
        ROUNDS.fetch_add(1, Ordering::SeqCst);
        Self(())
    }
}

impl Drop for RoundGuard {
    fn drop(&mut self) {
        ROUNDS.fetch_sub(1, Ordering::SeqCst);
    }
}

/// "What you are showing is out of date." Cheap, idempotent, and safe to call
/// from an operation.
pub fn invalidate() {
    INVALIDATED.store(true, Ordering::SeqCst);
}

/// The network came back: read again — unless a round is already reading,
/// which is that read (spec 082 RJ14, G53). Every "came back" used to force
/// a second round beside the one out, and with the flap the pool carried two
/// or three rounds at once: the relaunch the device pass read as "24 networks
/// unavailable" over an empty list while one chain was faulted.
pub fn network_back() {
    NETWORK_BACK.store(true, Ordering::SeqCst);
}

/// Drain the flags. `true` means somebody should force a read.
pub fn take_invalidation() -> bool {
    let changed = INVALIDATED.swap(false, Ordering::SeqCst);
    let back = NETWORK_BACK.swap(false, Ordering::SeqCst);
    if back && !changed && ROUNDS.load(Ordering::SeqCst) > 0 {
        crate::diag::vlog!(
            "balances",
            "network back: the round already reading covers it"
        );
        return false;
    }
    changed || back
}

/// The flag is process-wide: the tests that raise and drain it take turns.
#[cfg(test)]
pub fn invalidation_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Boot the hero and keep it honest for the life of the process.
///
/// Until this existed the desktop dispatched exactly one balance event ever —
/// `AccountChanged` at boot — so **the total on screen was the total at launch**.
/// Money could arrive, a send could settle, and the figure would not move until
/// the app was restarted. The core names the two cadences it does not own
/// (`AUTO_REFRESH_MS`, and `AppFocused` when the window comes back) and both are
/// wired here.
///
/// `force: false` is deliberate: the core drops an unforced refresh while the
/// window is in the background, which is exactly what an interval poll should
/// do and what the web's own timer relies on.
pub fn start_ticks(cx: &mut App) {
    let _ = resident::resident::<BalanceDashboard>(cx);
    if let Some(event) = hydrate_privacy() {
        resident::resident::<BalanceDashboard>(cx)
            .update(cx, |resident, cx| resident.dispatch(event, cx));
    }
    if TICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a ten-minute constant in f64 ms is exactly representable"
    )]
    let period = Duration::from_millis(AUTO_REFRESH_MS as u64);
    // r12-3: the pool's out-of-reach set, to the balance machine whenever it
    // moved (`pool_unreached`) — looked at every second, from whatever read
    // found it (the incoming scan, a send, a balance read). The machine
    // decides what to read again; this only carries the set.
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(OUT_OF_REACH_LOOK_MS))
                .await;
            if let Some(loss) = crate::executor::pool::take_out_of_reach_change() {
                crate::diag::vlog!(
                    "balances",
                    "pool out of reach: {:?}, silent {:?}",
                    loss.chain_ids,
                    loss.silent
                );
                let balance = cx.update(resident::resident::<BalanceDashboard>);
                balance.update(cx, |resident, cx| {
                    // r13-4: a SILENT network is named at once — the web's
                    // timing, not a second read's 18 s later.
                    resident.dispatch(pool_unreached(loss), cx);
                });
            }
        }
    })
    .detach();
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(period).await;
            // Re-fetched each tick: a sign-out drops every resident, and the
            // next sign-in's hero is the one that must get the ticks.
            let balance = cx.update(resident::resident::<BalanceDashboard>);
            balance.update(cx, |resident, cx| {
                resident.dispatch(
                    Event::RefreshRequested {
                        force: false,
                        pull: false,
                    },
                    cx,
                );
            });
        }
    })
    .detach();
}

/// One event into the hero, from a screen that made something stale.
pub fn dispatch(event: Event, cx: &mut App) {
    resident::resident::<BalanceDashboard>(cx).update(cx, |resident, cx| {
        resident.dispatch(event, cx);
    });
}

/// The networks each account here lives on, as last told (r10-1).
static LIVES_ON: Mutex<Option<std::collections::BTreeMap<String, Vec<u32>>>> = Mutex::new(None);

/// [`Event::LivesOn`] for every account whose networks are known — a
/// multisig's, from its record (`chain_ids`).
fn lives_on_known() -> Vec<Event> {
    LIVES_ON
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .flatten()
        .map(|(address, chain_ids)| Event::LivesOn {
            address: address.clone(),
            chain_ids: chain_ids.clone(),
        })
        .collect()
}

/// Keep the networks `address` lives on; whether that changed what was kept.
pub fn remember_lives_on(address: &str, chain_ids: Vec<u32>) -> bool {
    let mut guard = LIVES_ON
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let map = guard.get_or_insert_with(Default::default);
    let address = address.to_ascii_lowercase();
    if map.get(&address) == Some(&chain_ids) {
        return false;
    }
    map.insert(address, chain_ids);
    true
}

/// The networks `address` lives on, as last told (a multisig's; none for a
/// Vela account) — the incoming scan's `account_networks` (§9.4).
#[must_use]
pub fn lives_on_of(address: &str) -> Vec<u32> {
    LIVES_ON
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .and_then(|map| map.get(&address.to_ascii_lowercase()).cloned())
        .unwrap_or_default()
}

/// r10-1 (docs/multisig-machines.md §9.3): `address` lives on `chain_ids` —
/// a multisig's networks — so the hero's "Updated" moves only when they
/// answered. Kept for the next boot, and told to the running machine when
/// it changed. Never boots the balances: a machine not booted yet hears it
/// with its boot.
pub fn lives_on(address: &str, chain_ids: Vec<u32>, cx: &mut App) {
    let address = address.to_ascii_lowercase();
    if !remember_lives_on(&address, chain_ids.clone()) {
        return;
    }
    if let Some(resident) = resident::existing::<BalanceDashboard>(cx) {
        resident.update(cx, |resident, cx| {
            resident.dispatch(Event::LivesOn { address, chain_ids }, cx);
        });
    }
}

/// The window opened on a screen that draws no balance of its own: the next
/// boot only notes the account ([`Event::AccountNoted`], perf-10).
static NOTED_ONLY: AtomicBool = AtomicBool::new(false);
/// The running machine was booted on [`Event::AccountNoted`] and has not
/// been told the home is shown yet.
static BOOTED_NOTED: AtomicBool = AtomicBool::new(false);

/// The window is opening on Settings or the address book, not the home:
/// the balances note the account and read no network until it is shown.
pub fn note_only() {
    NOTED_ONLY.store(true, Ordering::SeqCst);
}

/// The home is on screen. An account only noted so far is read now —
/// [`Event::AccountChanged`] for the same account, as the web's wallet page
/// says it — once; every later frame is nothing.
pub fn shown(cx: &mut App) {
    // Shown before anything booted the balances: they boot reading.
    NOTED_ONLY.store(false, Ordering::SeqCst);
    if !BOOTED_NOTED.swap(false, Ordering::SeqCst) {
        return;
    }
    let address = crate::multisig::shown::address(cx);
    if !address.is_empty() {
        dispatch(Event::AccountChanged { address }, cx);
    }
}

/// The space in front changed (spec 105): [`Event::SpaceChanged`] — the
/// core resets for the same account, reads this space's cached total and
/// forces a read — then the rows this space last showed for it.
pub fn space_switched(cx: &mut App) {
    dispatch(Event::SpaceChanged, cx);
    let address = crate::multisig::shown::address(cx);
    if let Some(event) = restored_holdings(&address) {
        dispatch(event, cx);
    }
}

/// A forced read, for the moments a screen KNOWS the answer changed — a token
/// added, a network added, an RPC repaired. The web forces on exactly these,
/// and forcing matters: those all happen while somebody is looking.
pub fn refresh(cx: &mut App) {
    dispatch(
        Event::RefreshRequested {
            force: true,
            pull: false,
        },
        cx,
    );
}

/// How often the pool's out-of-reach set is looked at for a change (r12-3).
const OUT_OF_REACH_LOOK_MS: u64 = 1_000;

/// r12-3 / r13-4: the pool's loss as the balance machine hears it — its
/// out-of-reach networks and, of them, the silent ones (the web's
/// `pool_unreached {chain_ids, silent}`).
fn pool_unreached(loss: crate::executor::pool::PoolLoss) -> Event {
    Event::PoolUnreached {
        chain_ids: loss.chain_ids,
        silent: loss.silent,
    }
}

/// When the balances are read again after a send was handed off (R12 S4),
/// in ms from the hand-off: once the operation has had time to land, and
/// once more for a slow bundle.
pub const AFTER_SEND_READS_MS: [u64; 2] = [5_000, 15_000];

/// A send left this account (R12 S4): Assets kept the figure from before it
/// while the sent row was pending — the tracker's `HoldingsMoved` comes with
/// the receipt, and a receipt held (or a window not painting) left the
/// figure the pre-send one for minutes. The chain is read again, forced, at
/// [`AFTER_SEND_READS_MS`] — what it says then is what Assets says, never a
/// figure the shell worked out.
pub fn after_send(cx: &mut App) {
    cx.spawn(async move |cx| {
        let mut waited = 0;
        for at in AFTER_SEND_READS_MS {
            cx.background_executor()
                .timer(Duration::from_millis(at.saturating_sub(waited)))
                .await;
            waited = at;
            cx.update(refresh);
        }
    })
    .detach();
}

/// The read the PERSON asked for — the hero's "↻ Updated <ago>" (issue 462),
/// what the phone's pull-to-refresh sends. `pull: true` is what makes it
/// theirs: the core holds `BalanceView.refreshing` from this dispatch until
/// this round settles or errors, and the control turns for exactly that
/// long. The forced reads above stay `pull: false` — a token added is not a
/// press, and must not spin a control nobody touched.
pub fn pull(cx: &mut App) {
    dispatch(
        Event::RefreshRequested {
            force: true,
            pull: true,
        },
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perform(operation: BalanceOperation) -> BalanceShellResult {
        match BalanceDashboard::perform(&operation) {
            Answer::Now(result) => result,
            _ => unreachable!("this operation is local"),
        }
    }

    /// r10-1 (§9.3): the shared stamp vectors, replayed through this shell's
    /// own host of the machine — events dispatched, each answer resolving
    /// the outstanding operation of its kind, the stamp read off the view as
    /// the hero draws it.
    #[test]
    fn the_balance_stamp_vectors_hold_here() {
        use crate::core_host::{CoreHost, Pending};
        const VECTORS: &str =
            include_str!("../../../../rust/crates/vela-core/tests/vectors/balance-stamp.json");
        let file: Value = serde_json::from_str(VECTORS).unwrap_or(Value::Null);
        let cases = file["cases"].as_array().cloned().unwrap_or_default();
        assert!(cases.len() >= 8, "the vectors are read");
        for case in cases {
            let name = case["name"].as_str().unwrap_or_default().to_owned();
            let address = case["address"].as_str().unwrap_or_default().to_owned();
            let mut host = CoreHost::<BalanceDashboard>::new();
            let mut out: Vec<Pending<BalanceOperation>> =
                host.dispatch(Event::AccountChanged { address });
            for (i, step) in case["steps"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                if let Some(event) = step.get("event") {
                    let event: Event = serde_json::from_value(event.clone())
                        .unwrap_or_else(|e| unreachable!("{name} step {i}: {e}"));
                    out.extend(host.dispatch(event));
                } else {
                    let answer: BalanceShellResult = serde_json::from_value(step["answer"].clone())
                        .unwrap_or_else(|e| unreachable!("{name} step {i}: {e}"));
                    let fetch = matches!(answer, BalanceShellResult::FetchSettled { .. });
                    let at = out
                        .iter()
                        .position(|p| {
                            if fetch {
                                matches!(p.operation, BalanceOperation::FetchTokens { .. })
                            } else {
                                matches!(p.operation, BalanceOperation::ReadBalanceCache { .. })
                            }
                        })
                        .unwrap_or_else(|| unreachable!("{name} step {i}: nothing to answer"));
                    let id = out.remove(at).id;
                    out.extend(host.resolve(id, answer));
                }
                if let Some(stamp) = step.get("stamp") {
                    assert_eq!(
                        host.view().last_refreshed_at_ms,
                        stamp.as_f64(),
                        "{name}, step {i}"
                    );
                }
            }
        }
    }

    /// r13-4 (§17): `balance-pool-unreached.json` replayed through this
    /// shell's host, every `pool_unreached` carried as the pool hands it on
    /// ([`crate::executor::pool::PoolLoss`] → [`pool_unreached`]) — a silent
    /// network named at once, a merely lost one only read again.
    #[test]
    fn the_pool_unreached_vectors_hold_through_the_pools_loss() {
        use crate::core_host::{CoreHost, Pending};
        use crate::executor::pool::PoolLoss;
        const VECTORS: &str = include_str!(
            "../../../../rust/crates/vela-core/tests/vectors/balance-pool-unreached.json"
        );
        let file: Value = serde_json::from_str(VECTORS).unwrap_or(Value::Null);
        let cases = file["cases"].as_array().cloned().unwrap_or_default();
        assert!(
            cases
                .iter()
                .any(|c| c["name"].as_str().unwrap_or_default().starts_with("r13-4")),
            "the silent cases are read"
        );
        let ids = |v: &Value| -> Vec<u32> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_u64().and_then(|x| u32::try_from(x).ok()))
                        .collect()
                })
                .unwrap_or_default()
        };
        for case in cases {
            let name = case["name"].as_str().unwrap_or_default().to_owned();
            let address = case["address"].as_str().unwrap_or_default().to_owned();
            let mut host = CoreHost::<BalanceDashboard>::new();
            let mut out: Vec<Pending<BalanceOperation>> =
                host.dispatch(Event::AccountChanged { address });
            for (i, step) in case["steps"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                if let Some(event) = step.get("event") {
                    let event: Event = if event["type"] == "pool_unreached" {
                        pool_unreached(PoolLoss {
                            chain_ids: ids(&event["chain_ids"]),
                            silent: ids(&event["silent"]),
                        })
                    } else {
                        serde_json::from_value(event.clone())
                            .unwrap_or_else(|e| unreachable!("{name} step {i}: {e}"))
                    };
                    out.extend(host.dispatch(event));
                } else {
                    let answer: BalanceShellResult = serde_json::from_value(step["answer"].clone())
                        .unwrap_or_else(|e| unreachable!("{name} step {i}: {e}"));
                    let fetch = matches!(answer, BalanceShellResult::FetchSettled { .. });
                    let at = out
                        .iter()
                        .position(|p| {
                            if fetch {
                                matches!(p.operation, BalanceOperation::FetchTokens { .. })
                            } else {
                                matches!(p.operation, BalanceOperation::ReadBalanceCache { .. })
                            }
                        })
                        .unwrap_or_else(|| unreachable!("{name} step {i}: nothing to answer"));
                    let id = out.remove(at).id;
                    out.extend(host.resolve(id, answer));
                }
                let Some(expect) = step.get("expect") else {
                    continue;
                };
                let view = host.view();
                if let Some(want) = expect.get("unreachable") {
                    let got: Vec<u32> = view
                        .unreachable_networks
                        .iter()
                        .map(|n| n.chain_id)
                        .collect();
                    assert_eq!(got, ids(want), "{name}, step {i}: unreachable");
                }
                if let Some(live) = expect.get("live").and_then(Value::as_bool) {
                    assert_eq!(view.live_key.is_some(), live, "{name}, step {i}: live");
                }
            }
        }
    }

    /// The privacy flag survives a restart on SCREEN, not only on disk.
    ///
    /// `WritePrivacy` has written this file since spec 030 and nothing read it
    /// back, so hiding your balance lasted exactly as long as the process. Both
    /// spellings are accepted because both clients have written it: the Expo
    /// app's `'1'`/`'0'` strings, and a boolean is what a JSON store would hold
    /// if one ever wrote it that way.
    #[test]
    fn the_stored_privacy_choice_is_read_back_at_boot() {
        storage::tests::with_temp_state("balance-privacy-hydrate", || {
            assert!(hydrate_privacy().is_none(), "nothing stored, nothing said");

            if storage::write_value(PRIVACY_KEY, Value::String("1".to_owned())).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: true })
            ));

            if storage::write_value(PRIVACY_KEY, Value::String("0".to_owned())).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: false })
            ));

            if storage::write_value(PRIVACY_KEY, Value::Bool(true)).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: true })
            ));
        });
    }

    /// Hiding the balance outlives the process.
    ///
    /// Two halves that were both present and never met: the core has always
    /// asked for `WritePrivacy`, and nothing read the file back. This drives
    /// the real machine through the tap, performs the write it asks for, and
    /// then reads it the way a fresh launch does.
    ///
    /// Only the privacy operation is performed — `AccountChanged` also asks for
    /// a twelve-chain fetch, and a unit test has no business making one.
    #[test]
    fn hiding_the_balance_survives_a_restart() {
        use crate::core_host::CoreHost;

        storage::tests::with_temp_state("balance-privacy-roundtrip", || {
            let mut host = CoreHost::<BalanceDashboard>::new();
            let _ = host.dispatch(Event::AccountChanged {
                address: "0xabc".to_owned(),
            });

            let toggle = |host: &mut CoreHost<BalanceDashboard>| {
                for next in host.dispatch(Event::PrivacyToggled) {
                    if matches!(next.operation, BalanceOperation::WritePrivacy { .. }) {
                        let result = perform(next.operation.clone());
                        let _ = host.resolve(next.id, result);
                    }
                }
            };

            toggle(&mut host);
            assert!(host.view().hidden, "the tap hid it");
            assert!(
                matches!(
                    hydrate_privacy(),
                    Some(Event::PrivacyHydrated { hidden: true })
                ),
                "and the next launch would start hidden"
            );

            toggle(&mut host);
            assert!(!host.view().hidden, "the second tap shows it again");
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: false })
            ));
        });
    }

    /// Invalidation is a one-shot: the frame that drains it forces one read,
    /// and the frame after it does not force another.
    #[test]
    fn an_invalidation_is_drained_once() {
        let _turn = invalidation_turn();
        let _ = take_invalidation();
        assert!(!take_invalidation(), "nothing to drain");
        invalidate();
        invalidate();
        assert!(take_invalidation(), "one drain reports it");
        assert!(!take_invalidation(), "and the next frame does not re-read");
    }

    /// Spec 082 RJ14 (G53): the network coming back reads again — unless a
    /// round is already reading, which is that read. A change the round may
    /// have missed (a token added, holdings moved) still forces its own.
    #[test]
    fn a_network_back_joins_the_round_already_reading() {
        let _turn = invalidation_turn();
        let _ = take_invalidation();

        network_back();
        assert!(take_invalidation(), "nothing reading: read");

        {
            let _reading = RoundGuard::enter();
            network_back();
            assert!(!take_invalidation(), "the round out is the read");
            assert!(!take_invalidation(), "and it is not owed later either");

            network_back();
            invalidate();
            assert!(take_invalidation(), "a change is never folded into it");
        }

        // The round ended (the guard is gone on any exit): the next one reads.
        network_back();
        assert!(take_invalidation());
    }

    /// A total written today comes back; one written two days ago does not.
    ///
    /// Expiry reads as **absent**, not as a stale figure: the hero would rather
    /// show a skeleton than yesterday's number presented as today's.
    #[test]
    fn a_cached_total_expires_rather_than_going_stale() {
        storage::tests::with_temp_state("balance-cache-ttl", || {
            const ADDR: &str = "0xabc";
            let now = 1_756_000_000_000.0;

            assert_eq!(cached_usd(ADDR, now), None, "nothing cached yet");

            write_cached_usd(ADDR, 42.5, now);
            assert_eq!(cached_usd(ADDR, now), Some(42.5));
            assert_eq!(
                cached_usd(ADDR, now + CACHE_TTL_MS - 1.0),
                Some(42.5),
                "still inside the window"
            );
            assert_eq!(
                cached_usd(ADDR, now + CACHE_TTL_MS + 1.0),
                None,
                "expired must read as absent, not as stale"
            );
        });
    }

    /// The stored shape is the one every other client reads.
    #[test]
    fn the_cache_uses_the_shared_key_and_shape() {
        storage::tests::with_temp_state("balance-cache-shape", || {
            write_cached_usd("0xabc", 12.25, 1_756_000_000_000.0);
            let raw = storage::read_value(CACHE_KEY)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("nothing written"));
            let record = raw
                .get("0xabc")
                .unwrap_or_else(|| unreachable!("the address is the key"));
            assert_eq!(record.get("usd").and_then(Value::as_f64), Some(12.25));
            assert!(record.get("at").is_some(), "`at` is what the TTL reads");
        });
    }

    /// The switcher only ever offers rows it can vouch for.
    #[test]
    fn only_unexpired_rows_reach_the_switcher() {
        storage::tests::with_temp_state("balance-cache-many", || {
            // The REAL clock, because the operation reads it: `ReadBalanceCache*`
            // takes `now` from the shell, which is the right design and means a
            // test seeding a fixed past timestamp writes two expired rows and
            // proves nothing. (It cost me one confusing red.)
            let now = crate::executor::now_ms();
            write_cached_usd("0xfresh", 1.0, now);
            write_cached_usd("0xstale", 2.0, now - CACHE_TTL_MS - 1.0);

            let result = perform(BalanceOperation::ReadBalanceCacheMany {
                addresses: vec![
                    "0xfresh".to_owned(),
                    "0xstale".to_owned(),
                    "0xnone".to_owned(),
                ],
            });
            match result {
                BalanceShellResult::CachedBalancesLoaded { balances } => {
                    assert_eq!(balances.len(), 1, "only the fresh row may be offered");
                    assert_eq!(balances[0].address, "0xfresh");
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    fn held(chain_id: u32, symbol: &str, contract: Option<&str>) -> BalanceToken {
        BalanceToken {
            chain_id,
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            balance: "1".to_owned(),
            decimals: 18,
            token_address: contract.map(str::to_owned),
            price_usd: Some(1.0),
            spam: false,
        }
    }

    /// Issue #196: a chain that did not answer keeps the holdings the last
    /// round saw; a chain that answered is authoritative, even with nothing.
    #[test]
    fn a_failing_chain_keeps_its_tokens_and_an_answering_one_is_believed() {
        let previous = vec![
            held(100, "xDAI", None),
            held(100, "USDC", Some("0xusdc")),
            held(8453, "ETH", None),
            held(137, "POL", None),
        ];
        // Gnosis failed; Base answered with nothing (spent); Polygon answered.
        let fresh = vec![held(137, "POL", None)];
        let settled = carry_over_unanswered(&previous, fresh, &[100]);
        let seen: Vec<(u32, &str)> = settled
            .iter()
            .map(|t| (t.chain_id, t.symbol.as_str()))
            .collect();
        assert_eq!(
            seen,
            vec![(100, "xDAI"), (100, "USDC"), (137, "POL")],
            "Gnosis kept, Base's spent ETH gone, and the fetch's own order"
        );

        // Nothing failed: the fresh list, untouched.
        let fresh = vec![held(137, "POL", None)];
        assert_eq!(carry_over_unanswered(&previous, fresh.clone(), &[]), fresh);
        // Nothing known before: nothing to carry.
        assert!(carry_over_unanswered(&[], Vec::new(), &[100]).is_empty());
    }

    /// r15-4: the first round after a launch has no round of this run to
    /// fall back to — the rows kept on disk (`vela.holdingsCache`) stand in.
    /// Arc did not answer: its 10.53 USDC stays in what is settled (and so
    /// in what is saved for the next launch), never dropped by a round
    /// that could not read it. A chain that answered is still believed.
    #[test]
    fn the_first_round_keeps_a_failed_chains_rows_from_the_disk() {
        let address = "0xFirstRoundKeepsArc";
        let mut arc = held(
            5_042_002,
            "USDC",
            Some("0x3600000000000000000000000000000000000000"),
        );
        arc.balance = "10.53".to_owned();
        let kept = vec![arc.clone(), held(8453, "ETH", None)];
        let settled =
            settle_with_carry_over(address, vec![held(100, "xDAI", None)], &[5_042_002], || {
                kept.clone()
            });
        let seen: Vec<(u32, &str)> = settled
            .iter()
            .map(|t| (t.chain_id, t.symbol.as_str()))
            .collect();
        assert_eq!(
            seen,
            vec![(100, "xDAI"), (5_042_002, "USDC")],
            "Arc kept from the disk; Base answered (nothing) and is believed"
        );
        assert_eq!(settled[1].balance, "10.53");
        // Once a round of this run settled, it is the authority, not the disk.
        let again = settle_with_carry_over(address, Vec::new(), &[5_042_002], || {
            unreachable!("the round of this run stands in")
        });
        assert_eq!(again.len(), 1);
    }

    /// The snapshot a failing round falls back to is the last SETTLED one for
    /// that address — including what an earlier failing round carried.
    #[test]
    fn a_second_failing_round_still_has_the_first_rounds_tokens() {
        let address = "0xCarryOverTest";
        let first = settle_with_carry_over(address, vec![held(100, "xDAI", None)], &[], Vec::new);
        assert_eq!(first.len(), 1);
        let second = settle_with_carry_over(address, Vec::new(), &[100], Vec::new);
        assert_eq!(second.len(), 1, "Gnosis failed: its xDAI stays");
        let third = settle_with_carry_over(&address.to_lowercase(), Vec::new(), &[100], Vec::new);
        assert_eq!(third.len(), 1, "the same account, however it is cased");
        let answered = settle_with_carry_over(address, Vec::new(), &[], Vec::new);
        assert!(answered.is_empty(), "Gnosis answered with nothing: spent");
    }

    /// Switching back to an account starts from what it last settled to —
    /// the seed a fetch sends before any chain answers — whatever the case of
    /// the address, and nothing for an account never fetched in this run.
    #[test]
    fn a_fetch_starts_from_what_the_account_last_held() {
        let address = "0xSwitchBackTest";
        assert!(last_settled(address).is_empty(), "never fetched");
        let _ = settle_with_carry_over(address, vec![held(100, "xDAI", None)], &[], Vec::new);
        assert_eq!(last_settled(address).len(), 1);
        assert_eq!(last_settled(&address.to_uppercase()).len(), 1);
        assert!(last_settled("0xSomebodyElse").is_empty());
    }

    /// The round the send picker answers from: every token read (zero
    /// balances included — a locked payment request matches against them),
    /// the chains that did not answer, and a round number that moves at every
    /// settle, which is what tells an open picker there is a newer list.
    #[test]
    fn a_settled_round_is_what_the_send_picker_reads() {
        let address = "0xPickerRoundTest";
        assert!(settled(address).is_none(), "not settled in this run");

        let mut empty = held(
            100,
            "USDC",
            Some("0x2a22f9c3b484c3629090feed35f17ff8f88f76f0"),
        );
        empty.balance = "0".to_owned();
        let _ = settle_with_carry_over(
            address,
            vec![held(100, "xDAI", None), empty],
            &[8453],
            Vec::new,
        );
        let Some(first) = settled(&address.to_uppercase()) else {
            unreachable!("just settled");
        };
        assert_eq!(first.tokens.len(), 2, "the zero-balance row is kept");
        assert_eq!(first.failed_chain_ids, vec![8453]);

        let _ = settle_with_carry_over(address, vec![held(100, "xDAI", None)], &[], Vec::new);
        let Some(second) = settled(address) else {
            unreachable!("just settled");
        };
        assert!(
            second.round > first.round,
            "a newer round is a newer number"
        );
        assert!(second.failed_chain_ids.is_empty());
    }

    /// PR 3 note 4, end to end from the executor's settle to the core's
    /// view: Tempo's node answered and its token list did not load. The
    /// settle reports it in `registry_chain_ids` (and never as rate-limited),
    /// and the core then says the token list — not "Can't reach Tempo" — and
    /// offers its row no RPC fix. A chain that really is down beside it is
    /// still the network's, and still fixable.
    #[test]
    fn a_token_list_miss_is_reported_and_the_core_says_so() {
        use crate::core_host::CoreHost;
        use vela_core::app::balance_dashboard::{
            BalanceDashboard, TOKEN_LIST_UNREACHABLE, UNREACHABLE_MANY, UnreachableCause,
        };
        const TEMPO: u32 = vela_core::app::fee_policy::TEMPO_CHAIN_IDS[0];
        let view_of = |address: &str, fetched: balances::Fetched| {
            let mut host = CoreHost::<BalanceDashboard>::new();
            let mut pending = host.dispatch(Event::AccountChanged {
                address: address.to_owned(),
            });
            while let Some(effect) = pending.pop() {
                let result = match &effect.operation {
                    BalanceOperation::ReadBalanceCache { address } => {
                        BalanceShellResult::CachedTotalLoaded {
                            address: address.clone(),
                            usd: None,
                        }
                    }
                    BalanceOperation::FetchTokens { address, pull, .. } => {
                        settled_result(address, *pull, vec![56, 100, TEMPO], fetched.clone())
                    }
                    _ => continue,
                };
                pending.extend(host.resolve(effect.id, result));
            }
            host.view()
        };

        let fetched = balances::Fetched {
            tokens: vec![held(100, "xDAI", None)],
            failed: vec![TEMPO],
            internal: Vec::new(),
            registry: vec![TEMPO],
        };
        match settled_result(
            "0xTokenListSettle",
            false,
            vec![100, TEMPO],
            fetched.clone(),
        ) {
            BalanceShellResult::FetchSettled {
                failed_chain_ids,
                registry_chain_ids,
                rate_limited_chain_ids,
                internal_chain_ids,
                ..
            } => {
                assert_eq!(failed_chain_ids, vec![TEMPO]);
                assert_eq!(registry_chain_ids, vec![TEMPO]);
                assert!(rate_limited_chain_ids.is_empty() && internal_chain_ids.is_empty());
            }
            other => unreachable!("wrong variant: {other:?}"),
        }

        let view = view_of("0xTokenListOne", fetched);
        assert_eq!(
            view.unreachable_key.as_deref(),
            Some(TOKEN_LIST_UNREACHABLE)
        );
        let rows: Vec<_> = view
            .unreachable_networks
            .iter()
            .map(|row| (row.chain_id, row.cause, row.rpc_fixable))
            .collect();
        assert_eq!(rows, vec![(TEMPO, UnreachableCause::TokenList, false)]);

        // BNB Chain really is down beside it: counted together, told apart.
        let view = view_of(
            "0xTokenListTwo",
            balances::Fetched {
                tokens: vec![held(100, "xDAI", None)],
                failed: vec![56, TEMPO],
                internal: Vec::new(),
                registry: vec![TEMPO],
            },
        );
        assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_MANY));
        let fixable = |chain_id: u32| {
            view.unreachable_networks
                .iter()
                .find(|row| row.chain_id == chain_id)
                .map(|row| row.rpc_fixable)
        };
        assert_eq!(fixable(56), Some(true));
        assert_eq!(fixable(TEMPO), Some(false));
    }

    /// Privacy persists as the '1'/'0' string the other clients wrote.
    #[test]
    fn privacy_persists_as_the_shared_flag() {
        storage::tests::with_temp_state("balance-privacy", || {
            perform(BalanceOperation::WritePrivacy { hidden: true });
            assert_eq!(
                storage::read_value(PRIVACY_KEY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str),
                Some("1")
            );
            perform(BalanceOperation::WritePrivacy { hidden: false });
            assert_eq!(
                storage::read_value(PRIVACY_KEY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str),
                Some("0")
            );
        });
    }

    /// WIRE-AUDIT A8: of the chains that failed, the ones read through the
    /// person's own node alone — never a chain that answered.
    #[test]
    fn own_node_rows_are_the_failed_chains_read_through_the_own_node() {
        let own = |unreached: &[u32]| pool::OwnNodeChains::of(&[], &[100, 8453], unreached);
        assert_eq!(
            failed_own_node(&[1, 100, 137], &own(&[100, 8453])),
            vec![100]
        );
        assert!(failed_own_node(&[], &own(&[100])).is_empty());
        assert!(failed_own_node(&[1], &own(&[])).is_empty());
    }

    /// The breakdown's "using public nodes" chains are the core's
    /// `FallingBack` standing — never an own-only chain that cannot be
    /// read, where nobody else is asked.
    #[test]
    fn public_nodes_serve_only_a_chain_the_core_says_is_falling_back() {
        use vela_core::app::rpc_pool::NodeStanding;
        let own = pool::OwnNodeChains::of(&[10, 100], &[100], &[100]);
        assert_eq!(own.falling_back(), vec![10]);
        assert_eq!(own.standing(100), NodeStanding::OwnNodeUnreached);
        assert_eq!(own.standing(1), NodeStanding::Pooled);
    }
}
