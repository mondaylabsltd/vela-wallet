//! Machine — balance aggregation & display policy (spec `017`, inventory
//! `### balance_dashboard (P2)`).
//!
//! App-resident, per active account. Everything that decides *what number the
//! hero may show* lives here; the shell keeps the transports, the 5-minute
//! token cache, the per-chain 18s cap and every timer.
//!
//! ```text
//! AccountChanged ─► reset ─► ReadBalanceCache ∥ FetchTokens ──► stream:
//!     ChainAssetsArrived (merge per chain, slow chains keep last value)
//!     └► FetchSettled ─┬─ complete ─► WriteBalanceCache + budget reset
//!                      └─ partial ──► silent retry ×3 [1500,4000,8000]ms
//!                                     └─ exhausted ─► notice allowed
//! ```
//!
//! The display rule (`useHomeController.ts:167-188`): never a confidently-wrong
//! smaller number — a partial live sum renders as `max(live, cached)`; nothing
//! known at all renders as a skeleton, never a fake `$0` that later jumps.
//!
//! Faithful port of the TypeScript sources — behavior aligned line by line:
//!
//! - `src/screens/wallet/useHomeController.ts:46-54, 79-91, 167-188, 259-479`
//!   — the whole balance/retry/notice/switcher policy
//! - `src/screens/wallet/HomeScreen.tsx:95-139, 264-278` — skeleton vs `$0`,
//!   notice wording split, banner exclusion of rate-limited chains
//! - `src/services/balance-cache.ts` — per-account USD cache (24h TTL; the
//!   TTL check itself runs in the shell executor, which owns the clock)
//! - `src/services/wallet-api.ts:55-220` — fetch semantics (execution stays in
//!   the shell; the *write gate* and the merge/sort/filter decisions are here)
//! - `src/services/wallet-api.ts:289-427` — native-coin pricing: deepest-pool
//!   selection and the DEX↔Chainlink sanity band, as pure functions
//! - `src/hooks/use-balance-privacy.ts` — the hand-the-phone-over privacy
//!   model, hydrate race included
//! - `src/services/rpc-pool.ts:156-185` — consumed classification: the shell
//!   snapshots the failed / rate-limited chain sets into `FetchSettled` (once
//!   the `rpc_pool` machine lands, its view feeds the same fields)
//!
//! Ported quirks, kept verbatim (each doc-commented at the site):
//!
//! - The account-change reset (`useHomeController.ts:399-416`) does NOT clear
//!   `rateLimitedChainIds` or `lastRefreshedAt` — both survive a switch.
//! - `openSwitcher` (`:473`) pokes the CURRENT `displayTotal` into the
//!   persisted cache — including a partial `max(live, cached)` value, or `0`
//!   when the balance is still unknown. This is in tension with invariant ⑥
//!   (cache stores complete totals only) but is today's shipped behavior.
//! - The retry-delay lookup falls back to 8000ms past the table
//!   (`PARTIAL_RETRY_DELAYS_MS[...] ?? 8000`, `:357`).
//!
//! Deliberate strictness where JS has no Rust equivalent (all fail-closed):
//!
//! - `parseFloat`'s `Infinity`-literal form parses as 0 here (a balance string
//!   is never that word; a numeric-prefix scan covers everything
//!   `formatRawBalance` can emit).
//! - A malformed DEX quote amount is skipped (cannot price), where the TS
//!   decode layer could not produce one at all.
//! - Per-account switcher refresh results arriving after an account switch are
//!   dropped by the attempt tag (TS would still apply them to the — by then
//!   closed — modal's rows; dropping is the safe side of the same behavior).
//! - The privacy hydrate collapses "key missing" and "read failed" into one
//!   `PrivacyHydrated { hidden: false }` — observably identical, since hydrate
//!   is single-shot and a later toggle overrides either way.
//!
//! Left in the shell on purpose: the ≤1-account tap-copies-address branch of
//! `openSwitcher` (clipboard), the 650ms minimum pull-spinner hold, all
//! formatting, and the 10min/10s polling cadences (they arrive as
//! [`Event::RefreshRequested`] / [`Event::AppFocused`]).

use std::collections::BTreeSet;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants — every value mirrors the TS source it is named after
// ---------------------------------------------------------------------------

/// Silent force-refetch budget for an incomplete result
/// (`useHomeController.ts:53`).
pub const MAX_PARTIAL_RETRIES: u32 = 3;
/// Escalating backoff between the silent retries (`useHomeController.ts:54`).
pub const PARTIAL_RETRY_DELAYS_MS: [u32; 3] = [1_500, 4_000, 8_000];
/// The `?? 8000` past the end of the table (`useHomeController.ts:357`).
pub const FALLBACK_RETRY_DELAY_MS: u32 = 8_000;
/// Aggregate poll cadence — the SHELL owns this timer and feeds
/// [`Event::RefreshRequested`] (`useHomeController.ts:46`).
pub const AUTO_REFRESH_MS: f64 = 600_000.0;
/// Activity-tab live poll cadence — also shell-owned (`useHomeController.ts:47`).
pub const LIVE_POLL_MS: f64 = 10_000.0;
/// Persisted per-account total TTL (`balance-cache.ts:11`). The shell executor
/// applies it on read (it owns the clock); the core owns the WRITE gate.
pub const BALANCE_CACHE_TTL_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// Quote-token decimals fallback when the `decimals()` read failed
/// (`wallet-api.ts:360, 379`).
pub const DEFAULT_QUOTE_DECIMALS: u32 = 6;

// ---------------------------------------------------------------------------
// Pure value math — `models/types.ts` helpers, f64 verbatim (open question 5:
// the display pipeline stays f64 so totals are bit-identical to today's)
// ---------------------------------------------------------------------------

/// `tokenBalanceDouble` (`models/types.ts:64-66`): `parseFloat(balance) || 0`.
/// Ports `parseFloat`'s longest-numeric-prefix scan (sign, digits, dot,
/// exponent); `NaN` and `±0` collapse to `0` exactly as `|| 0` does. The
/// `Infinity` literal form is NOT ported (fail-closed to 0 — see module doc).
pub fn token_balance_double(balance: &str) -> f64 {
    let s = balance.trim_start();
    let bytes = s.as_bytes();
    let mut i = 0usize;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut frac_digits = 0usize;
    if i < bytes.len() && bytes[i] == b'.' {
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        frac_digits = j - (i + 1);
        if int_digits > 0 || frac_digits > 0 {
            i = j;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return 0.0; // no mantissa digit → parseFloat gives NaN → `|| 0`
    }
    if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < bytes.len() && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            i = j; // exponent only consumed when it has digits, as parseFloat
        }
    }
    let parsed = s[..i].parse::<f64>().unwrap_or(0.0);
    if parsed == 0.0 {
        0.0 // -0 is falsy in JS: `-0 || 0` → 0
    } else {
        parsed
    }
}

/// Chains whose native asset is itself an ERC-20 (spec 038, the founder's
/// Celo report): the chain data names that contract as the "wrapped" native,
/// but nothing is wrapped — `balanceOf` there and the native balance are ONE
/// balance, and a shell that lists both counts the holding twice (CELO 6.96 +
/// WCELO 6.96). Every shell's balance walk asks here before adding the
/// wrapped slot; the address still serves the native price quote.
#[must_use]
pub fn native_token_as_erc20(chain_id: u32) -> Option<&'static str> {
    match chain_id {
        // Celo mainnet — the GoldToken.
        42220 => Some("0x471ece3750da237f93b8e339c536989b8978a438"),
        // Celo Alfajores.
        44787 => Some("0xf194afdf50b03e69bd7d057c1aa9e10c9954e4c9"),
        _ => None,
    }
}

/// Whether the chain's "wrapped" native at `address` is the native itself.
#[must_use]
pub fn wrapped_native_is_the_native(chain_id: u32, address: &str) -> bool {
    native_token_as_erc20(chain_id).is_some_and(|known| known.eq_ignore_ascii_case(address))
}

/// `tokenUsdValue` (`models/types.ts:68-70`): balance × (price ?? 0).
pub fn token_usd_value(token: &BalanceToken) -> f64 {
    token_balance_double(&token.balance) * token.price_usd.unwrap_or(0.0)
}

fn live_total(tokens: &[BalanceToken]) -> f64 {
    tokens.iter().map(token_usd_value).sum()
}

/// A HELD token with no price source (`useHomeController.ts:172`).
fn has_unpriced(tokens: &[BalanceToken]) -> bool {
    tokens
        .iter()
        .any(|t| token_balance_double(&t.balance) > 0.0 && t.price_usd.is_none())
}

// ---------------------------------------------------------------------------
// Native-coin pricing — `wallet-api.ts:289-427`, previously untested money
// display rules, ported as pure functions the shell executor prices with
// ---------------------------------------------------------------------------

/// One stable's DEX quotes for 1 native coin (`nativeQuoteGroups`,
/// `wallet-api.ts:304-312`). Each stable is its OWN group so its own decimals
/// normalize the amount — USDC (6) and DAI (18) must never be compared under
/// one shared scale.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeQuoteGroup {
    /// Successful quote outputs in THIS stable's base units, decimal strings
    /// (the shell decodes the multicall; failed calls are simply absent).
    pub amounts_out: Vec<String>,
    /// This stable's `decimals()` read; `None` = the read failed → default 6
    /// (`wallet-api.ts:379`).
    pub quote_decimals: Option<u32>,
}

/// `extractBestPrice` (`wallet-api.ts:580-598`): the deepest-pool price within
/// ONE quote token. For a fixed input, a more-liquid pool returns more output
/// (less slippage), so the max is the least-distorted price — this dodges a
/// broken/near-empty pool quoting a garbage low value. Zero outputs are
/// skipped (`amountOut > 0n`).
pub fn best_group_price(group: &NativeQuoteGroup) -> Option<f64> {
    let decimals = group.quote_decimals.unwrap_or(DEFAULT_QUOTE_DECIMALS);
    let scale = 10f64.powi(i32::try_from(decimals).unwrap_or(i32::MAX));
    let mut best: Option<f64> = None;
    for amount in &group.amounts_out {
        // `Number(bigint)` rounds to the nearest f64; `str::parse` on the same
        // decimal digits rounds identically. Malformed input cannot price.
        let Ok(value) = amount.trim().parse::<f64>() else {
            continue;
        };
        if value <= 0.0 || !value.is_finite() {
            continue;
        }
        let price = value / scale;
        if best.is_none_or(|b| price > b) {
            best = Some(price);
        }
    }
    best
}

/// The cross-stable max (`wallet-api.ts:374-381`) — the X Layer WOKB case:
/// one stable's pool can be near-empty and quote OKB at ~$5 while another
/// holds the liquid pool (~$81); taking the first would lock in the junk.
pub fn best_native_dex_price(groups: &[NativeQuoteGroup]) -> Option<f64> {
    let mut best: Option<f64> = None;
    for group in groups {
        if let Some(price) = best_group_price(group) {
            if best.is_none_or(|b| price > b) {
                best = Some(price);
            }
        }
    }
    best
}

/// `firstGroupedQuotePrice` (`wallet-api.ts:648-662`): the first usable price
/// across quote groups, each scaled by ITS OWN quote token's decimals.
///
/// The CUSTOM-token counterpart of [`best_native_dex_price`], and deliberately
/// a different rule. The native path takes the maximum because every group
/// prices the same coin and the deepest pool is the least distorted. A custom
/// token's groups are tried in a stated ORDER — the preferred stablecoin first
/// (`pickQuoteToken`), then the rest in chain order — so the first pool that
/// answers is the one the caller asked for, and taking a maximum would silently
/// promote whichever stable happened to quote highest.
///
/// The rule this replaced took the first surviving quote out of a FLAT list
/// that mixed quote tokens and divided it by ONE token's `decimals()`. On any
/// chain whose stablecoin list holds both a 6-decimal (USDC/USDT) and an
/// 18-decimal (DAI/WXDAI) entry, a custom token with no USDC pool but a live
/// DAI pool was priced 10^12 times too high — and that number is what the
/// portfolio total, the sort order and the ingest valuation all consume.
///
/// `quote_decimals: None` falls back to [`DEFAULT_QUOTE_DECIMALS`] — the SAME
/// group's fallback, never a neighbour's real value. A zero amount does not
/// price: a zero-output quote is a dead pool, not a free token.
#[must_use]
pub fn first_grouped_quote_price(groups: &[NativeQuoteGroup]) -> Option<f64> {
    for group in groups {
        let decimals = group.quote_decimals.unwrap_or(DEFAULT_QUOTE_DECIMALS);
        let scale = 10f64.powi(i32::try_from(decimals).unwrap_or(i32::MAX));
        for amount in &group.amounts_out {
            // The TS calls `BigInt(raw)`, which THROWS on a malformed string
            // and would abort the whole lookup. Nothing malformed can reach it
            // there (the decoder only emits digits), so skipping is the same
            // behaviour on every input that actually occurs — and a safer one
            // on the input that does not.
            let Ok(value) = amount.trim().parse::<f64>() else {
                continue;
            };
            if value > 0.0 && value.is_finite() {
                return Some(value / scale);
            }
        }
    }
    None
}

/// Where the chosen native price came from — mirrors `nativePriceSource`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativePriceSource {
    Dex,
    /// The DEX price failed the sanity band → Chainlink wins.
    ChainlinkSanity,
    ChainlinkLocal,
    ChainlinkEth,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativePrice {
    pub price: f64,
    pub source: NativePriceSource,
}

/// The source ladder + sanity band (`wallet-api.ts:396-417`): DEX preferred,
/// but a DEX price deviating beyond ratio (0.5, 2.0) against the best
/// Chainlink read means low liquidity → prefer Chainlink. `chainlink_local`
/// carries the `Number.isFinite(usd) && usd > 0` decode gate
/// (`wallet-api.ts:388-390`) with it; the Ethereum-mainnet fallback is
/// deliberately ungated, verbatim.
pub fn choose_native_price(
    dex: Option<f64>,
    chainlink_local: Option<f64>,
    chainlink_eth: Option<f64>,
) -> Option<NativePrice> {
    let chainlink_local = chainlink_local.filter(|p| p.is_finite() && *p > 0.0);
    let chainlink_best = chainlink_local.or(chainlink_eth);
    if let (Some(dex_price), Some(cl_price)) = (dex, chainlink_best) {
        let ratio = dex_price / cl_price;
        if ratio > 0.5 && ratio < 2.0 {
            return Some(NativePrice {
                price: dex_price,
                source: NativePriceSource::Dex,
            });
        }
        return Some(NativePrice {
            price: cl_price,
            source: NativePriceSource::ChainlinkSanity,
        });
    }
    if let Some(dex_price) = dex {
        return Some(NativePrice {
            price: dex_price,
            source: NativePriceSource::Dex,
        });
    }
    if let Some(price) = chainlink_local {
        return Some(NativePrice {
            price,
            source: NativePriceSource::ChainlinkLocal,
        });
    }
    chainlink_eth.map(|price| NativePrice {
        price,
        source: NativePriceSource::ChainlinkEth,
    })
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One held asset, as the shell maps it from `APIToken`. `balance` stays a
/// human decimal string (never a JSON number); `chain_id` replaces the
/// network-slug lookup table, which is shell master data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BalanceToken {
    pub chain_id: u32,
    pub symbol: String,
    pub name: String,
    pub balance: String,
    pub decimals: u32,
    /// `None` = the chain's native coin.
    pub token_address: Option<String>,
    pub price_usd: Option<f64>,
    pub spam: bool,
}

/// One switcher row / cache entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BalanceCacheEntry {
    pub address: String,
    pub usd: f64,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Sentences, not I/O — the shell
/// owns transports, the 5-minute token cache, the per-chain 18s cap and every
/// timer; the core owns when to ask and what to believe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BalanceOperation"))]
pub enum BalanceOperation {
    /// The full multi-chain fetch for the active account (`fetchTokens`).
    /// `force` bypasses the shell's 5-minute TTL — the manual-pull rule
    /// (invariant ⑨, `useHomeController.ts:205-217`). While in flight the
    /// shell streams [`Event::ChainAssetsArrived`] snapshots; the operation
    /// itself settles exactly once (`FetchSettled`/`FetchErrored`), echoing
    /// `address` and `pull`.
    FetchTokens {
        address: String,
        force: bool,
        /// True only for the pull-to-refresh gesture — drives the spinner.
        pull: bool,
    },
    /// One account's assets for a switcher row (`fetchTokens(acc.address)`,
    /// `useHomeController.ts:459` — TTL-cached, never forced, never streams).
    FetchAccountAssets { address: String },
    /// Read the persisted total. The shell applies the 24h TTL — absent or
    /// expired answers `None` (`balance-cache.ts:53-59`).
    ReadBalanceCache { address: String },
    /// Batch read for the switcher; only TTL-valid entries come back
    /// (`balance-cache.ts:62-73`).
    ReadBalanceCacheMany { addresses: Vec<String> },
    /// Persist a total. The CORE decides when this may happen — the complete-
    /// results-only write gate (invariant ⑥) plus the verbatim switcher poke.
    WriteBalanceCache { address: String, usd: f64 },
    /// One-shot timer for a silent partial retry. `timer_id` comes back in
    /// [`BalanceShellResult::RetryElapsed`]; only the live id fires.
    StartRetryTimer { ms: u32, timer_id: u32 },
    /// Persist `vela.balanceHidden` ('1'/'0', best effort —
    /// `use-balance-privacy.ts:35-40`).
    WritePrivacy { hidden: bool },
}

/// What the shell observed. Every account-scoped variant carries `address` so
/// a slow answer for a previous account is dropped by construction, and every
/// time-bearing variant carries `now_ms` (epoch milliseconds, f64) — the core
/// has no clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BalanceShellResult"))]
pub enum BalanceShellResult {
    /// The fetch finished (`useHomeController.ts:318-339`). `tokens` is the
    /// full final result; `failed_chain_ids` / `rate_limited_chain_ids` are
    /// the rpc-pool classification snapshot taken at settle
    /// (`rpc-pool.ts:156-185`). Echoes the request's `pull` flag.
    FetchSettled {
        address: String,
        pull: bool,
        tokens: Vec<BalanceToken>,
        failed_chain_ids: Vec<u32>,
        rate_limited_chain_ids: Vec<u32>,
        now_ms: f64,
    },
    /// The fetch itself threw (`useHomeController.ts:367`) — keep last-known
    /// everything, just close the skeleton.
    FetchErrored {
        address: String,
        pull: bool,
    },
    /// A switcher row's assets; `None` = per-account best-effort failure
    /// (`useHomeController.ts:457-463`) — the row keeps its cached value.
    AccountAssetsFetched {
        address: String,
        tokens: Option<Vec<BalanceToken>>,
    },
    /// `None` = missing or expired — the hero keeps its skeleton
    /// (`useHomeController.ts:412-414` only commits non-null).
    CachedTotalLoaded {
        address: String,
        usd: Option<f64>,
    },
    CachedBalancesLoaded {
        balances: Vec<BalanceCacheEntry>,
    },
    BalanceCacheWritten,
    RetryElapsed {
        timer_id: u32,
    },
    PrivacyWritten,
}

impl Operation for BalanceOperation {
    type Output = BalanceShellResult;
}

#[effect]
pub enum BalanceEffect {
    Render(RenderOperation),
    Shell(BalanceOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "BalanceEvent"))]
pub enum Event {
    /// The active account changed (or was first resolved). Resets the balance
    /// state, paints the hero from cache, and starts a fetch — the port of
    /// `useHomeController.ts:399-416` plus the focus-effect reload. Bumps the
    /// account generation: every in-flight answer for the old account dies.
    AccountChanged {
        address: String,
    },
    /// A refresh trigger. The shell feeds these for the 10-minute aggregate
    /// poll and the 10-second activity poll (`force: false, pull: false`),
    /// the pull gesture (`force: true, pull: true` — invariant ⑨: a user pull
    /// MUST re-hit RPC), and the detail sheet's retry (`force: true,
    /// pull: false`). A non-forced tick while backgrounded is dropped — the
    /// `isAppActive()` gate (`useHomeController.ts:375, 383`).
    RefreshRequested {
        force: bool,
        pull: bool,
    },
    /// Mid-fetch stream: the accumulated, USD-sorted snapshot of every chain
    /// that has finished so far — exactly what `onProgress` delivers
    /// (`useHomeController.ts:324-331`). Chains not in the snapshot keep
    /// their previous tokens, so the total never drops to $0 mid-refresh
    /// (invariant ④). Tagged with `address`: a stale account's stream can
    /// never paint the new account (invariant ⑤).
    ChainAssetsArrived {
        address: String,
        tokens: Vec<BalanceToken>,
    },
    /// Home gained focus / app foregrounded — reload, exactly as
    /// `useFocusEffect`'s `loadData()` does.
    AppFocused,
    /// Backgrounded: polls stop biting (invariant ⑨'s second half). An
    /// in-flight fetch is NOT aborted, as today.
    AppBackgrounded,
    /// The eye tap. Flips and persists immediately; marks the store touched so
    /// a slower hydrate can never overwrite the user's choice (invariant ⑧).
    PrivacyToggled,
    /// The shell's boot-time read of `vela.balanceHidden`. First-write-wins
    /// against [`Event::PrivacyToggled`] (`use-balance-privacy.ts:25-33`).
    PrivacyHydrated {
        hidden: bool,
    },
    /// The RPC-fix modal saved a working endpoint for this chain — drop it
    /// from the failed set and reload (`HomeScreen.tsx:337-340, 363-366`).
    FixChainResolved {
        chain_id: u32,
    },
    /// The account switcher was tapped open. `addresses` is the full roster
    /// (context the caller holds — the session machine owns accounts). The
    /// ≤1-account tap-copies-address branch stays in the shell (clipboard).
    /// Cache first, refresh after: the modal must show numbers the instant it
    /// opens (invariant ⑩, `useHomeController.ts:470-479`).
    SwitcherOpened {
        addresses: Vec<String>,
    },
    SwitcherClosed,
    /// Internal: an effect resolved. `attempt` is the account generation
    /// captured when the request was made; an older attempt belongs to a
    /// previous account and is dropped — this IS the `addressRef` check
    /// (invariant ⑤), enforced by construction.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: BalanceShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Model {
    /// The active account — the ONLY address any result may paint for.
    address: Option<String>,
    /// Account generation. Bumped exclusively by [`Event::AccountChanged`].
    attempt: u64,
    /// Live tokens, merged per chain, USD-sorted, zero balances filtered.
    tokens: Vec<BalanceToken>,
    failed_chain_ids: Vec<u32>,
    /// NOT cleared on account change — ported verbatim (the reset effect at
    /// `useHomeController.ts:399-416` never touches it).
    rate_limited_chain_ids: Vec<u32>,
    /// Last-known-good complete total (the `max(live, cached)` floor).
    cached_total: Option<f64>,
    /// First fetch for this account has settled (either way) — skeleton off.
    bootstrapped: bool,
    /// Survives account switches — ported verbatim.
    last_refreshed_at_ms: Option<f64>,
    partial_retries_left: u32,
    /// The armed retry timer's id; a `RetryElapsed` for any other id is a
    /// cancelled timer's late echo and is ignored (`clearTimeout`).
    live_timer: Option<u32>,
    timer_seq: u32,
    notice_allowed: bool,
    /// Outstanding pull-gesture fetches; the spinner shows while > 0.
    pending_pulls: u32,
    /// A fetch (silent or pulled) is out. While it is, the FIGURE holds at
    /// `last_settled_total` even as chains stream in (#188): the number a
    /// person reads must not climb $62 → $98 as seven networks answer one by
    /// one. The token LIST still merges per chain — that is what "streaming"
    /// is for.
    fetch_in_flight: bool,
    /// The figure the last settle produced (complete or partial), seeded from
    /// the cache. Shown while `fetch_in_flight`.
    last_settled_total: Option<f64>,
    /// The first fetch threw with nothing known — no tokens, no cache. The home
    /// says "unreachable", never $0.00 (spec 038 finding 15).
    errored_without_data: bool,
    hidden: bool,
    /// Hydrate OR toggle — whichever lands first wins
    /// (`use-balance-privacy.ts:19`).
    privacy_touched: bool,
    /// Inverted so `Default` (false) means active, matching a fresh screen.
    backgrounded: bool,
    switcher_open: bool,
    switcher_roster: Vec<String>,
    /// `displayTotal` captured at the moment the switcher was tapped — the
    /// closure capture in `openSwitcher` (`useHomeController.ts:470-479`).
    /// Doubles as the "an open is pending" latch.
    switcher_pinned_total: Option<f64>,
    switcher_pending: u32,
    switcher_balances: Vec<BalanceCacheEntry>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// Which notice line the hero shows once the retry budget is exhausted
/// (`HomeScreen.tsx:121-126`): failed chains are transient ("still updating"
/// is honest); an unpriced held token won't resolve on its own, so promising
/// an update would lie.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum BalanceNotice {
    StillUpdating,
    Unpriced,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BalanceSwitcherView {
    pub open: bool,
    pub loading: bool,
    pub balances: Vec<BalanceCacheEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct BalanceView {
    pub address: Option<String>,
    /// The ONE number the hero may render, in USD (the display-currency
    /// machine owns conversion). `None` while the skeleton shows — never a
    /// fake $0 (invariant ②) — AND while privacy hides: the fiat value is
    /// withheld by construction, not masked downstream (invariant ⑧).
    pub display_total_usd: Option<f64>,
    pub balance_unknown: bool,
    pub balance_partial: bool,
    /// Nothing could be read and nothing is known: the first fetch failed
    /// with no cache to fall back on. A skeleton and a reason, never a zero.
    pub unreachable: bool,
    /// `Some` only when partial AND the silent retries are exhausted
    /// (invariant ③).
    pub notice: Option<BalanceNotice>,
    /// Every money surface (feed amounts, holdings, switcher, receipt toast)
    /// masks on this together — a leak in one defeats the mask everywhere.
    pub hidden: bool,
    pub refreshing: bool,
    pub last_refreshed_at_ms: Option<f64>,
    /// USD-sorted holdings for the Assets tab.
    pub tokens: Vec<BalanceToken>,
    /// The detail sheet's "couldn't be priced" list — held, no price source,
    /// spam excluded (`useHomeController.ts:176-179`).
    pub unpriced_tokens: Vec<BalanceToken>,
    pub failed_chain_ids: Vec<u32>,
    pub rate_limited_chain_ids: Vec<u32>,
    /// Failed minus rate-limited (invariant ⑦): a rate limit lifts on its
    /// own, so the "fix your RPC" banner must never nag for it — the balance
    /// quietly stays on cache (`HomeScreen.tsx:133-139`).
    pub banner_chain_ids: Vec<u32>,
    /// `tokens.length === 0 && (cachedTotal ?? 0) > 0` (`HomeScreen.tsx:271`).
    pub holdings_loading: bool,
    pub cached_total_usd: Option<f64>,
    pub switcher: BalanceSwitcherView,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct BalanceDashboard;

impl App for BalanceDashboard {
    type Event = Event;
    type Model = Model;
    type ViewModel = BalanceView;
    type Effect = BalanceEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<BalanceEffect, Event> {
        match event {
            Event::AccountChanged { address } => account_changed(model, address),
            Event::RefreshRequested { force, pull } => {
                if model.address.is_none() {
                    return Command::done();
                }
                // The interval polls check `isAppActive()` before loading
                // (`useHomeController.ts:375, 383`); manual paths don't.
                if !force && model.backgrounded {
                    return Command::done();
                }
                begin_fetch(model, force, pull)
            }
            Event::ChainAssetsArrived { address, tokens } => {
                chain_assets_arrived(model, &address, tokens)
            }
            Event::AppFocused => {
                model.backgrounded = false;
                if model.address.is_some() {
                    begin_fetch(model, false, false)
                } else {
                    Command::done()
                }
            }
            Event::AppBackgrounded => {
                model.backgrounded = true;
                Command::done()
            }
            Event::PrivacyToggled => {
                model.privacy_touched = true;
                model.hidden = !model.hidden;
                request(
                    model,
                    BalanceOperation::WritePrivacy {
                        hidden: model.hidden,
                    },
                )
            }
            Event::PrivacyHydrated { hidden } => {
                if model.privacy_touched {
                    // A toggle raced the read — the user's tap wins
                    // (`use-balance-privacy.ts:29`).
                    return Command::done();
                }
                model.privacy_touched = true;
                model.hidden = hidden;
                render()
            }
            Event::FixChainResolved { chain_id } => {
                model.failed_chain_ids.retain(|id| *id != chain_id);
                if model.address.is_some() {
                    begin_fetch(model, false, false)
                } else {
                    render()
                }
            }
            Event::SwitcherOpened { addresses } => switcher_opened(model, addresses),
            Event::SwitcherClosed => {
                model.switcher_open = false;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A previous account's answer — dropping it IS the
                    // `addressRef` check (invariant ⑤).
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> BalanceView {
        let partial = balance_partial(model);
        let total = display_total(model);
        // Nothing known yet → skeleton, never a fake $0 (invariant ②,
        // `useHomeController.ts:186-188`).
        let unknown =
            model.tokens.is_empty() && model.cached_total.is_none() && !model.bootstrapped;
        // Never mid-refresh: before prices arrive every token is unpriced,
        // and "some tokens couldn't be priced" flashing while chains are still
        // answering was #188's second half.
        let notice = if partial && model.notice_allowed && !model.fetch_in_flight {
            Some(if model.failed_chain_ids.is_empty() {
                BalanceNotice::Unpriced
            } else {
                BalanceNotice::StillUpdating
            })
        } else {
            None
        };
        let banner_chain_ids = model
            .failed_chain_ids
            .iter()
            .copied()
            .filter(|id| !model.rate_limited_chain_ids.contains(id))
            .collect();
        let unpriced_tokens = model
            .tokens
            .iter()
            .filter(|t| !t.spam && token_balance_double(&t.balance) > 0.0 && t.price_usd.is_none())
            .cloned()
            .collect();
        BalanceView {
            address: model.address.clone(),
            display_total_usd: if model.hidden || unknown {
                None
            } else {
                Some(total)
            },
            balance_unknown: unknown,
            balance_partial: partial,
            unreachable: model.errored_without_data
                && model.tokens.is_empty()
                && model.cached_total.is_none(),
            notice,
            hidden: model.hidden,
            refreshing: model.pending_pulls > 0,
            last_refreshed_at_ms: model.last_refreshed_at_ms,
            tokens: model.tokens.clone(),
            unpriced_tokens,
            failed_chain_ids: model.failed_chain_ids.clone(),
            rate_limited_chain_ids: model.rate_limited_chain_ids.clone(),
            banner_chain_ids,
            holdings_loading: model.tokens.is_empty() && model.cached_total.unwrap_or(0.0) > 0.0,
            cached_total_usd: model.cached_total,
            switcher: BalanceSwitcherView {
                open: model.switcher_open,
                loading: model.switcher_pending > 0,
                balances: model.switcher_balances.clone(),
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

fn account_changed(model: &mut Model, address: String) -> Command<BalanceEffect, Event> {
    if model.address.as_deref() == Some(address.as_str()) {
        // Same dependency value — the reset effect would not re-run.
        return Command::done();
    }
    model.attempt += 1;
    model.address = Some(address.clone());
    // The reset block (`useHomeController.ts:399-416`) — note what it does
    // NOT reset: `rateLimitedChainIds` and `lastRefreshedAt` survive the
    // switch, ported verbatim.
    model.tokens.clear();
    model.failed_chain_ids.clear();
    model.cached_total = None;
    model.bootstrapped = false;
    model.partial_retries_left = MAX_PARTIAL_RETRIES;
    model.notice_allowed = false;
    model.live_timer = None; // drop any pending retry from the old account
    model.pending_pulls = 0; // stale pull settles are dropped by attempt
    model.fetch_in_flight = false;
    model.last_settled_total = None;
    model.errored_without_data = false;
    model.switcher_open = false;
    model.switcher_roster.clear();
    model.switcher_pinned_total = None;
    model.switcher_pending = 0;
    requests(
        model,
        vec![
            // Paint the hero instantly from the cached total (never a $0
            // flash, `:412-414`) …
            BalanceOperation::ReadBalanceCache {
                address: address.clone(),
            },
            // … while the focus-effect reload fetches live data.
            BalanceOperation::FetchTokens {
                address,
                force: false,
                pull: false,
            },
        ],
    )
}

fn begin_fetch(model: &mut Model, force: bool, pull: bool) -> Command<BalanceEffect, Event> {
    let Some(address) = model.address.clone() else {
        return Command::done();
    };
    if pull {
        model.pending_pulls = model.pending_pulls.saturating_add(1);
    }
    model.fetch_in_flight = true;
    request(
        model,
        BalanceOperation::FetchTokens {
            address,
            force,
            pull,
        },
    )
}

/// The streaming merge (`useHomeController.ts:326-330`): chains present in
/// the snapshot replace their previous tokens; chains still in flight keep
/// their last value — the total never drops to $0 mid-refresh (invariant ④).
fn chain_assets_arrived(
    model: &mut Model,
    address: &str,
    tokens: Vec<BalanceToken>,
) -> Command<BalanceEffect, Event> {
    if model.address.as_deref() != Some(address) {
        return Command::done(); // stale stream for a previous account (⑤)
    }
    let fresh: BTreeSet<u32> = tokens.iter().map(|t| t.chain_id).collect();
    let mut merged: Vec<BalanceToken> = model
        .tokens
        .iter()
        .filter(|t| !fresh.contains(&t.chain_id))
        .cloned()
        .collect();
    merged.extend(
        tokens
            .into_iter()
            .filter(|t| token_balance_double(&t.balance) > 0.0),
    );
    sort_by_usd_desc(&mut merged);
    model.tokens = merged;
    render()
}

fn switcher_opened(model: &mut Model, addresses: Vec<String>) -> Command<BalanceEffect, Event> {
    let Some(address) = model.address.clone() else {
        return Command::done();
    };
    // Captured NOW, exactly as `openSwitcher`'s closure captures
    // `displayTotal` — a settle landing before the cache read answers must
    // not move the pinned row.
    let total = display_total(model);
    model.switcher_roster = addresses.clone();
    model.switcher_pinned_total = Some(total);
    requests(
        model,
        vec![
            // Ported verbatim (`useHomeController.ts:473`): the CURRENT
            // displayTotal is poked into the persisted cache so the active
            // row matches the hero — even when that value is a partial
            // `max(live, cached)`, or 0 while still unknown. In tension with
            // invariant ⑥; today's shipped behavior.
            BalanceOperation::WriteBalanceCache {
                address,
                usd: total,
            },
            BalanceOperation::ReadBalanceCacheMany { addresses },
        ],
    )
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: BalanceShellResult) -> Command<BalanceEffect, Event> {
    match result {
        BalanceShellResult::FetchSettled {
            address,
            pull,
            tokens,
            failed_chain_ids,
            rate_limited_chain_ids,
            now_ms,
        } => {
            if model.address.as_deref() != Some(address.as_str()) {
                return Command::done(); // belt to the attempt check's braces
            }
            if pull {
                model.pending_pulls = model.pending_pulls.saturating_sub(1);
            }
            // The final result replaces everything (`setTokens(result)`,
            // `:335`): a chain that answered nothing this round drops out of
            // the live list — the failed-chain record plus `max(live,cached)`
            // is what keeps the total honest.
            let mut live: Vec<BalanceToken> = tokens
                .into_iter()
                .filter(|t| token_balance_double(&t.balance) > 0.0)
                .collect();
            sort_by_usd_desc(&mut live);
            model.tokens = live;
            model.failed_chain_ids = failed_chain_ids;
            model.last_refreshed_at_ms = Some(now_ms);
            model.rate_limited_chain_ids = rate_limited_chain_ids;
            model.fetch_in_flight = false;
            model.errored_without_data = false;
            // The figure this settle produced, under the same rule the screen
            // reads — held through the NEXT refresh (#188).
            model.last_settled_total = Some(display_total(model));

            let unpriced = has_unpriced(&model.tokens);
            let partial = !model.failed_chain_ids.is_empty() || unpriced;
            let mut operations = Vec::new();
            if !partial {
                // Invariant ⑥: only a COMPLETE total may become the new
                // last-known-good — a partial write would poison the
                // `max(live, cached)` floor (`:342-348`).
                let usd = live_total(&model.tokens);
                operations.push(BalanceOperation::WriteBalanceCache {
                    address: address.clone(),
                    usd,
                });
                model.cached_total = Some(usd);
                // The switcher's row for THIS account is this figure too (spec
                // 038, the founder's "10,289 here, 10,315 there"): the row used
                // to keep whatever its own per-account fetch had found at some
                // other instant, so the hero and the account list disagreed by
                // however far prices had moved in between. One settle, one
                // number, every screen.
                upsert_balance(&mut model.switcher_balances, &address, usd);
            }
            // `clearTimeout` at every settle (`:352`): a previously armed
            // retry may no longer fire.
            model.live_timer = None;
            if !partial {
                // A clean result resets the budget so a later hiccup gets
                // its own grace (`:353-355`).
                model.partial_retries_left = MAX_PARTIAL_RETRIES;
                model.notice_allowed = false;
            } else if model.partial_retries_left > 0 {
                // Invariant ③: silent force-retries with escalating backoff
                // before the notice is allowed to show (`:356-362`).
                let index = MAX_PARTIAL_RETRIES.saturating_sub(model.partial_retries_left) as usize;
                let ms = PARTIAL_RETRY_DELAYS_MS
                    .get(index)
                    .copied()
                    .unwrap_or(FALLBACK_RETRY_DELAY_MS);
                model.partial_retries_left -= 1;
                model.notice_allowed = false;
                model.timer_seq = model.timer_seq.wrapping_add(1);
                model.live_timer = Some(model.timer_seq);
                operations.push(BalanceOperation::StartRetryTimer {
                    ms,
                    timer_id: model.timer_seq,
                });
            } else {
                // Retries exhausted and still incomplete — now the notice is
                // honest (`:363-366`).
                model.notice_allowed = true;
            }
            model.bootstrapped = true;
            requests(model, operations)
        }

        BalanceShellResult::FetchErrored { address, pull } => {
            if model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            if pull {
                model.pending_pulls = model.pending_pulls.saturating_sub(1);
            }
            // `catch { /* keep last-known tokens + total */ }` then
            // `setBootstrapped(true)` (`:367-369`).
            model.bootstrapped = true;
            model.fetch_in_flight = false;
            // Nothing known at all: the home must say so rather than show a
            // settled-looking $0.00 (spec 038 finding 15).
            model.errored_without_data = model.tokens.is_empty() && model.cached_total.is_none();
            render()
        }

        BalanceShellResult::CachedTotalLoaded { address, usd } => {
            if model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            // Only a present value commits (`if (v != null)`, `:413`).
            if let Some(usd) = usd {
                model.cached_total = Some(usd);
                if model.last_settled_total.is_none() {
                    model.last_settled_total = Some(usd);
                }
            }
            render()
        }

        BalanceShellResult::RetryElapsed { timer_id } => {
            if model.live_timer != Some(timer_id) {
                return Command::done(); // a cancelled timer's late echo
            }
            model.live_timer = None;
            // `loadDataRef.current?.(true)` — forced, but not a pull: the
            // spinner never shows for a silent retry (`:360-362`).
            begin_fetch(model, true, false)
        }

        BalanceShellResult::CachedBalancesLoaded { balances } => {
            let Some(pinned) = model.switcher_pinned_total.take() else {
                return Command::done(); // no open pending
            };
            model.switcher_balances = balances;
            if let Some(address) = model.address.clone() {
                // `balances.set(address, displayTotal)` (`:475`) — the active
                // row always matches what the hero showed at open.
                upsert_balance(&mut model.switcher_balances, &address, pinned);
            }
            // Invariant ⑩: open NOW, with cached numbers on every row …
            model.switcher_open = true;
            // … then refresh every account in the background (`:478`).
            let roster = std::mem::take(&mut model.switcher_roster);
            model.switcher_pending = u32::try_from(roster.len()).unwrap_or(u32::MAX);
            requests(
                model,
                roster
                    .into_iter()
                    .map(|address| BalanceOperation::FetchAccountAssets { address })
                    .collect(),
            )
        }

        BalanceShellResult::AccountAssetsFetched { address, tokens } => {
            model.switcher_pending = model.switcher_pending.saturating_sub(1);
            match tokens {
                Some(tokens) => {
                    let usd: f64 = tokens.iter().map(token_usd_value).sum();
                    upsert_balance(&mut model.switcher_balances, &address, usd);
                    requests(
                        model,
                        vec![BalanceOperation::WriteBalanceCache { address, usd }],
                    )
                }
                // Per-account best effort — the row keeps its cached value.
                None => render(),
            }
        }

        // Best-effort write acks. Neither may change state.
        BalanceShellResult::BalanceCacheWritten | BalanceShellResult::PrivacyWritten => {
            Command::done()
        }
    }
}

// ---------------------------------------------------------------------------
// Derivations — `useHomeController.ts:167-188`, f64 verbatim
// ---------------------------------------------------------------------------

fn balance_partial(model: &Model) -> bool {
    !model.failed_chain_ids.is_empty() || (!model.tokens.is_empty() && has_unpriced(&model.tokens))
}

/// The display rule (invariants ① and the cache fallback): no live data →
/// cached; partial → `max(live, cached)` — never the confident undercount;
/// otherwise the live sum.
fn display_total(model: &Model) -> f64 {
    // #188: while a fetch is out the figure holds at the last settle. Live
    // replaces it once, at settle. (A first fetch with nothing settled and
    // nothing cached falls through: there is nothing to hold.)
    if model.fetch_in_flight {
        if let Some(held) = model.last_settled_total {
            return held;
        }
    }
    let live = live_total(&model.tokens);
    let has_live = !model.tokens.is_empty();
    match model.cached_total {
        Some(cached) if !has_live => cached,
        Some(cached) if balance_partial(model) => live.max(cached),
        _ => live,
    }
}

/// `.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a))` — stable in both
/// runtimes; the totals contain no NaN by construction.
fn sort_by_usd_desc(tokens: &mut [BalanceToken]) {
    tokens.sort_by(|a, b| {
        token_usd_value(b)
            .partial_cmp(&token_usd_value(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn upsert_balance(rows: &mut Vec<BalanceCacheEntry>, address: &str, usd: f64) {
    match rows.iter_mut().find(|row| row.address == address) {
        Some(row) => row.usd = usd,
        None => rows.push(BalanceCacheEntry {
            address: address.to_owned(),
            usd,
        }),
    }
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue one operation whose answer must match the current account generation.
fn request(model: &Model, operation: BalanceOperation) -> Command<BalanceEffect, Event> {
    requests(model, vec![operation])
}

/// Issue operations whose answers must match the current account generation.
fn requests(model: &Model, operations: Vec<BalanceOperation>) -> Command<BalanceEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<BalanceEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for BalanceEffect {
    type Op = BalanceOperation;
    fn into_shell(self) -> Option<crux_core::Request<BalanceOperation>> {
        match self {
            BalanceEffect::Render(_) => None,
            BalanceEffect::Shell(request) => Some(request),
        }
    }
}
