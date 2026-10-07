//! Machine — RPC/Bundler endpoint pool decisions (spec `017`, inventory
//! `### rpc_pool (P2)`).
//!
//! The decision core of `src/services/rpc-pool.ts` (26-1035): six-tier source
//! priority scoring, EMA latency penalty, exponential failure cooldown, the
//! temp-1h / permanent-24h ban system, the four-way error classification
//! (permanent auth / transient / rate-limit / getLogs range-cap), the
//! 3-pass full-pool sweep with jittered backoff, chain-level failure
//! classification (failed vs rate-limited), and the all-banned self-rescue.
//! The shell keeps the fetch execution: it holds the request payloads keyed by
//! `call_id`, performs [`RpcOperation::JsonRpcPost`], and reports transport
//! outcomes back — this core only ever decides *which URL next and why*.
//!
//! ```text
//! CallRequested ─► (pool stale? LoadPoolConfig) ─► pass 0: score-sorted sweep
//!    │ per endpoint: Post ─► outcome ─┬─ success/range-cap ─► Conclude
//!    │                                ├─ ban-class ─► temp/perma ban, next url
//!    │                                └─ transient/429/… ─► next url
//!    └─ pass swept clean ─► DrawJitter ─► StartBackoff ─► next pass (≤3 rpc, ≤2 bundler)
//!                                          └─ passes exhausted ─► Failed{rate_limited}
//! ```
//!
//! Inventory invariants ①–⑧, each pinned by a test:
//!
//! - ① banned endpoints are never selected (`endpointScore` -Infinity,
//!   `rpc-pool.ts:396-423`) — here banned URLs are excluded from selection
//!   entirely, see the unification note below.
//! - ② an endpoint that reported the wrong `eth_chainId` is never handed to
//!   the bundler via `X-Rpc-Url` (`:673-681`) — for BOTH consumers of that
//!   header: the JSON-RPC sweep ([`Event::CallRequested`] with
//!   [`RpcKind::Bundler`]) and the REST leg
//!   ([`Event::BestRpcUrlRequested`] / `getChainRpcUrl`, which feeds
//!   `/v1/account` and the `/v1/sponsor` treasury transfer).
//! - ③ account-info/sponsor resolve to the SAME bundler the pool would submit
//!   to — Tempo's gas reimbursement is paid to that bundler's per-Safe EOA;
//!   reading it from a different bundler reimburses the wrong EOA and the op
//!   is rejected (`:957-974`). [`Event::BundlerBaseRequested`] answers with
//!   the top non-banned bundler endpoint, `/{chainId}` suffix stripped.
//! - ④ rate-limiting is a transient, self-healing condition — the chain is
//!   marked `rate_limited` so the UI keeps the cached balance and NEVER shows
//!   the "swap in your own RPC" banner for it (`:576-630`). Only a chain
//!   whose nodes ANSWERED, if only to say "slow down": a final pass in which
//!   any node could not be reached at all is a failed chain even when another
//!   node throttled (spec 092 — behind a blocking network, twenty-two dead
//!   BNB Chain nodes and one public node answering 429 had hidden the chain
//!   from the "can't reach" list for good).
//! - ⑤ a `getLogs` range/size cap is request-specific: the endpoint is healthy
//!   but capped, so the answer goes back to the caller to split the range —
//!   no failover, no ban, and the endpoint records a *success* (`:484-519`,
//!   `:768-779`).
//! - ⑥ when every endpoint of a pool is banned, that pool's next pass proceeds
//!   over the bans and the failure counters reset, so the chain can recover
//!   (`:745-755`). The rescue is scoped to the pass: unlike TS it does not
//!   delete the ban entries, because those entries are also what
//!   [`bundler_eligible_urls`] reads, and an endpoint nothing else will touch
//!   must not become the one named to the bundler in `X-Rpc-Url`.
//! - ⑦ a permanent ban requires zero successes AND ≥ 6 failures, and expires
//!   after 24h to allow recovery from transient outages (`:58-185`).
//! - ⑧ the two ban truths (`EndpointStats.banned` vs `banMap`) are unified —
//!   see below.
//!
//! # Ban-truth unification (invariant ⑧)
//!
//! In TS the ban concept lives in two places with different lifecycles:
//! `banMap` (TTL'd, persisted) and the per-endpoint `banned` flag (set at
//! runtime, only ever cleared by the all-banned rescue or an app restart —
//! it survives even after the `banMap` entry expires, because collection
//! filters `isBanned` at init while scoring reads the stale flag). This core
//! models a SINGLE truth: the ban map with its TTLs, re-checked live at every
//! selection. Documented consequences of the unification:
//!
//! - a temp ban expiring restores the endpoint at the next selection — no
//!   pool refresh or rescue needed (in TS the stale `banned` flag could pin
//!   the endpoint at -Infinity until restart);
//! - banned endpoints are excluded from the sweep entirely (in TS a
//!   runtime-banned endpoint still sat at the end of the score-sorted list
//!   and could be tried as a last resort within the same pass);
//! - banned endpoints do not take part in the fastest-RPC ping race (TS
//!   pinged even -Infinity endpoints, and one could have won the race into
//!   the `X-Rpc-Url` header).
//!
//! # Ported quirks (kept verbatim, see inventory open questions)
//!
//! - `sawRateLimit` is a per-pass local in TS: only the FINAL pass's
//!   observations classify the chain as rate-limited vs hard-failed. Ported
//!   verbatim — two rate-limited passes followed by a network-error pass
//!   classify as a hard failure.
//! - An empty pool still burns all backoff passes before failing, exactly as
//!   the TS loop over zero endpoints does.
//! - `fastestRpcCache`'s comment says "for 60s" but `FASTEST_RPC_TTL_MS` is
//!   3_600_000 — one hour is the shipped behavior (same class of stale
//!   comment as fee_policy's `// 150`). One hour here.
//! - The first-number extraction in the range-cap parser is greedy across
//!   commas and picks up a stray `k`/`m` after whitespace ("got 5000, max
//!   100" → `5000` then the `m` of "max" → 5_000_000_000). Ported verbatim.
//! - A non-finite jitter value produces a 0ms delay (JS `setTimeout(NaN)`
//!   fires immediately).
//!
//! # Deliberate deviations (each doc'd where it lives)
//!
//! - Invariant ② over the code path: TS's all-pings-failed fallback hands the
//!   score-sorted first URL *unverified* — including one that just REPORTED a
//!   wrong chain id. Here an endpoint that reported a wrong chain is excluded
//!   even from the fallback; if every endpoint did, no `X-Rpc-Url` is sent at
//!   all (fail-closed).
//! - `saveBans` calls are collapsed to one [`RpcOperation::PersistBans`] per
//!   decision (TS wrote the whole map twice back-to-back on a perma-ban).
//! - The dev fault-injection hook at the head of `poolRpcCall` has no core
//!   counterpart — tests inject outcomes directly, which is the same power.
//! - `isUsingBuiltinBundler` and `probeRpcChainId` (settings validation) stay
//!   in the shell: both are pure functions of shell-owned config, not pool
//!   decisions (the probe unification belongs to network_admin).
//! - `getChainRpcUrl` does NOT stay in the shell, and the distinction is the
//!   one above: it is not a question about config, it is the pool's ranking
//!   ([`Event::BestRpcUrlRequested`]). Answering it shell-side means answering
//!   it from collection order alone — no EMA latency, no failure cooldown, no
//!   wrong-chain memory — which is materially weaker than the TypeScript pool
//!   it replaced, on a value that decides a treasury transfer.
//!
//! # Spec 082 additions
//!
//! - Did a POST leave the device? [`RpcTransportOutcome::NotConnected`] (routed
//!   like `Network`) and [`may_have_delivered`]; both concluding verdicts carry
//!   the call's sticky OR, which is what lets a submit say "not sent" only
//!   when that is true (RA1).
//! - `unreached_chains`: one clean first pass of transport failures, no
//!   rate-limit signal, is enough for the browser's chain notice (RF1).
//! - [`OPTIONAL_METHODS`]: a node that does not serve `eth_simulateV1` answers
//!   so; that is never a ban or a chain fault (RG7).
//! - [`is_log_range_error`]: the one reading of an `eth_getLogs` range limit,
//!   shared with `tx_tracker`'s find-event (ruling 8); rate-limit wording on
//!   `eth_getLogs` is never mistaken for one (T180).
//!
//! Time never originates here: every shell result carries `now_ms` (epoch
//! milliseconds, f64). Randomness never originates here: the backoff jitter
//! is drawn by the shell via [`RpcOperation::DrawJitter`]. The two global
//! failure sets consumed by balance_dashboard and bug-report become
//! [`RpcPoolView`] fields (view push, not module-global reads).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants — every value mirrors the TS source it is named after
// ---------------------------------------------------------------------------

/// Pool config re-read interval — `POOL_REFRESH_MS` (`rpc-pool.ts:61`).
pub const POOL_REFRESH_MS: f64 = 600_000.0;
/// Temporary ban cooldown — `TEMP_BAN_TTL_MS` (`rpc-pool.ts:69`).
pub const TEMP_BAN_TTL_MS: f64 = 3_600_000.0;
/// Permanent bans expire after 24h to allow recovery from transient outages —
/// `PERMA_BAN_TTL_MS` (`rpc-pool.ts:70`). Half of invariant ⑦.
pub const PERMA_BAN_TTL_MS: f64 = 86_400_000.0;
/// Never-succeeded failure threshold for a permanent ban —
/// `PERMA_BAN_MIN_FAILURES` (`rpc-pool.ts:71`). The other half of ⑦.
pub const PERMA_BAN_MIN_FAILURES: u64 = 6;
/// Expired-ban prune throttle — `BAN_PRUNE_INTERVAL_MS` (`rpc-pool.ts:96`).
pub const BAN_PRUNE_INTERVAL_MS: f64 = 300_000.0;
/// Full-pool sweep passes for an RPC call — `MAX_RPC_ATTEMPTS`
/// (`rpc-pool.ts:716`). Bounded to fit the 18s per-chain race upstream.
pub const MAX_RPC_ATTEMPTS: u32 = 3;
/// Bundler calls retry exactly once (`rpc-pool.ts:918-924`).
pub const MAX_BUNDLER_ATTEMPTS: u32 = 2;
/// RPC inter-pass backoff: `backoffWithJitter(attempt, 300, 1500)` —
/// "~0–300ms, ~0–600ms, …" (`rpc-pool.ts:832`).
pub const RPC_BACKOFF_BASE_MS: u32 = 300;
pub const RPC_BACKOFF_CAP_MS: u32 = 1_500;
/// Bundler retry backoff: `backoffWithJitter(0, 1000, 1000)` (`rpc-pool.ts:920`).
pub const BUNDLER_BACKOFF_BASE_MS: u32 = 1_000;
pub const BUNDLER_BACKOFF_CAP_MS: u32 = 1_000;
/// Read RPC timeout — `NET_TIMEOUTS.rpcRead` (`net.ts:25`); `poolRpcCall`
/// always posts with this shorter timeout (`rpc-pool.ts:765`).
pub const RPC_READ_TIMEOUT_MS: u32 = 8_000;
/// How long a read waits on one endpoint before a shell also asks the
/// endpoint this machine will ask next — hedged reads (spec 083 D3b,
/// hand-off H6; see [`is_hedged_read`], [`early_verdict`] and
/// [`RpcPoolView::pending_urls`]). Without it a black-holed node holds every
/// read for the whole [`RPC_READ_TIMEOUT_MS`] before the next node is asked.
pub const HEDGE_AFTER_MS: u32 = 1_500;
/// Bundler JSON-RPC timeout — `NET_TIMEOUTS.bundlerRpc` (`net.ts:27`).
pub const BUNDLER_RPC_TIMEOUT_MS: u32 = 15_000;
/// Per-endpoint `eth_chainId` ping timeout — `NET_TIMEOUTS.rpcPing` (`net.ts:29`).
pub const PING_TIMEOUT_MS: u32 = 3_000;
/// Fastest-RPC winner cache TTL — `FASTEST_RPC_TTL_MS = 3_600_000`
/// (`rpc-pool.ts:641`; the "for 60s" comment there is stale — 1h shipped).
pub const FASTEST_RPC_TTL_MS: f64 = 3_600_000.0;
/// Failure cooldown: 30s · 2^(n-1), capped at 300s (`rpc-pool.ts:414`).
pub const COOLDOWN_BASE_MS: f64 = 30_000.0;
pub const COOLDOWN_CAP_MS: f64 = 300_000.0;

/// Score subtracted while an endpoint is inside its failure cooldown —
/// "effectively disabled during cooldown" (`rpc-pool.ts:416`).
const COOLDOWN_SCORE_PENALTY: f64 = 50_000.0;
/// Post-cooldown per-failure penalty (`rpc-pool.ts:418`).
const CONSECUTIVE_FAILURE_PENALTY: f64 = 200.0;
/// Latency penalty: -1 per 10ms above 200ms, capped at 200 (`rpc-pool.ts:402-406`).
const LATENCY_FREE_MS: f64 = 200.0;
const LATENCY_PENALTY_CAP: f64 = 200.0;
/// Reliability bonus: +1 per success, capped at 50 (`rpc-pool.ts:408-410`).
const RELIABILITY_BONUS_CAP: u64 = 50;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// Which pool a call routes through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcKind {
    Rpc,
    Bundler,
}

/// Endpoint source tier (`EndpointStats['source']`, `rpc-pool.ts:37`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcSource {
    /// Per-network override.
    User,
    /// Configured key (Alchemy/dRPC/Ankr).
    Provider,
    /// Vela built-in (CHAINS table).
    Default,
    /// Vela curated public fallback.
    Public,
    /// ethereum-data chain-index (first few).
    Builtin,
    /// Chain-index extras — only when everything else is exhausted.
    Fallback,
}

/// `SOURCE_PRIORITY` (`rpc-pool.ts:387-394`), verbatim.
pub fn source_priority(source: RpcSource) -> f64 {
    match source {
        RpcSource::User => 10_000.0,
        RpcSource::Provider => 9_000.0,
        RpcSource::Default => 1_000.0,
        RpcSource::Public => 500.0,
        RpcSource::Builtin => 100.0,
        RpcSource::Fallback => 10.0,
    }
}

/// One collected endpoint, as the shell's config layer hands it in. The shell
/// no longer filters bans at collection (it cannot — bans are core state);
/// the core filters at selection, which IS the single-truth unification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RpcEndpointSeed {
    pub url: String,
    pub source: RpcSource,
}

/// One ban — `BanEntry` (`rpc-pool.ts:73`), persisted under `vela.rpc.banned`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RpcBanEntry {
    pub url: String,
    pub banned_at_ms: f64,
    pub permanent: bool,
}

/// Is this ban still in force at `now_ms`? Temp bans last 1h, permanent bans
/// 24h (`isBanned`, `rpc-pool.ts:119-135`).
pub fn is_ban_active(entry: &RpcBanEntry, now_ms: f64) -> bool {
    let ttl = if entry.permanent {
        PERMA_BAN_TTL_MS
    } else {
        TEMP_BAN_TTL_MS
    };
    now_ms - entry.banned_at_ms < ttl
}

/// The JSON-RPC `error` member, as far as classification needs it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RpcErrorInfo {
    pub code: Option<i32>,
    pub message: Option<String>,
}

/// What the shell's fetch observed, mechanically. HTTP status and body shape
/// are transport facts the shell knows; everything *meaningful* about them
/// (ban vs cool-down vs deliver) is decided here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcTransportOutcome {
    /// HTTP 2xx with a JSON object body; the shell holds the body and reports
    /// only the `error` member (if any) for classification.
    Response { error: Option<RpcErrorInfo> },
    /// Any non-2xx status (401/403/404 → ban, 429 → rate-limited cooldown,
    /// the rest → plain failover — all decided here).
    HttpError { status: u16 },
    /// 2xx but not JSON (`Non-JSON response` / `Invalid response`).
    NonJson,
    /// The per-request timeout elapsed.
    Timeout,
    /// A socket failure after the request may have been written (a reset,
    /// a closed connection mid-answer), or a failure the shell cannot place.
    Network,
    /// Nothing left the device (spec 082 RA1): DNS, a refused connection,
    /// the TLS handshake, the proxy's CONNECT, a connect or resolve timeout.
    /// Routed exactly like [`Self::Network`]; it differs only in
    /// [`may_have_delivered`], which is what lets a submit say "not sent".
    NotConnected,
}

/// Could this one POST have reached the endpoint and been acted on, with its
/// answer lost on the way back (spec 082 RA1)? True for a timeout, a socket
/// failure after the write, a body that is not JSON and an HTTP 5xx; false
/// when nothing left the device ([`RpcTransportOutcome::NotConnected`]), for
/// any JSON answer (the endpoint said what it did) and for an HTTP 4xx (the
/// request was refused before any handler ran).
pub fn may_have_delivered(outcome: &RpcTransportOutcome) -> bool {
    match outcome {
        RpcTransportOutcome::Timeout
        | RpcTransportOutcome::Network
        | RpcTransportOutcome::NonJson => true,
        RpcTransportOutcome::HttpError { status } => (500..=599).contains(status),
        RpcTransportOutcome::NotConnected | RpcTransportOutcome::Response { .. } => false,
    }
}

/// How a routed call ends — delivered to the shell so it can settle the
/// promise it is holding for `call_id`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcCallVerdict {
    /// Use the JSON body held for (call_id, url). Includes valid execution
    /// errors like "execution reverted" — those are answers, not faults.
    /// `maybe_delivered` is the OR of [`may_have_delivered`] over every POST
    /// of this call — an earlier endpoint may have acted on it too.
    Respond {
        url: String,
        #[serde(default)]
        maybe_delivered: bool,
    },
    /// `eth_getLogs` range/size cap (invariant ⑤): the caller splits the
    /// block range. `max_span` is the endpoint's stated block span, or 0 for
    /// "halve" (a result-count cap or a range error with no usable number).
    RangeCap { url: String, max_span: f64 },
    /// Every endpoint failed every pass. For RPC calls `rate_limited`
    /// distinguishes the self-healing transient condition (invariant ④).
    /// `maybe_delivered` as on [`Self::Respond`]: false only when no POST of
    /// the call can have been acted on (spec 082 RA1).
    Failed {
        rate_limited: bool,
        #[serde(default)]
        maybe_delivered: bool,
    },
    /// Answer to [`Event::BundlerBaseRequested`]: the REST base of the
    /// bundler the pool would submit to (invariant ③). `None` ⇒ every
    /// bundler endpoint is banned or the pool is empty — use the built-in
    /// base (`getActiveBundlerBaseUrl`'s fallback; the built-in URL is shell
    /// config).
    BundlerBase { base_url: Option<String> },
    /// Answer to [`Event::BestRpcUrlRequested`] (`getChainRpcUrl`): the RPC
    /// endpoint this chain's pool would reach for first. `None` ⇒ nothing
    /// eligible (empty pool, everything banned, or every candidate has
    /// reported another chain's id) — the caller sends no `X-Rpc-Url` header
    /// and the fork simulator declines to run, both of which are the
    /// fail-closed side (invariant ②).
    BestRpcUrl { url: Option<String> },
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. The shell owns fetch, storage
/// and the clock; every answer carries `now_ms` where time matters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcOperation {
    /// Collect the endpoint lists for one chain (user override, provider
    /// keys, defaults, curated public, chain index — all shell config;
    /// `collectRpcUrls`/`collectBundlerUrls` stay there). Do NOT filter
    /// banned URLs — bans are this core's state.
    LoadPoolConfig { chain_id: u32 },
    /// POST the JSON-RPC payload the shell holds for `call_id` to `url`.
    /// The decision core carries no params — only what routing needs.
    JsonRpcPost {
        call_id: String,
        url: String,
        method: String,
        /// Bundler calls only: the verified same-chain RPC URL the shell sends
        /// the relay as `x-vela-rpc-url` (invariant ②; spec 098 §5 — before
        /// 081 the shells used `X-Rpc-Url`, a name the relay never read).
        /// `None` ⇒ send no header. Never set on a call to an RPC provider.
        x_rpc_url: Option<String>,
        timeout_ms: u32,
    },
    /// Lightweight `eth_chainId` ping for the fastest-RPC race.
    ProbeChainId {
        chain_id: u32,
        url: String,
        timeout_ms: u32,
    },
    /// Draw one uniform random in [0,1). Jitter randomness is injected by
    /// the shell — never generated in the core.
    DrawJitter { call_id: String },
    /// Sleep before the next full-pool pass.
    StartBackoff { call_id: String, delay_ms: u32 },
    /// Persist the whole ban map to `vela.rpc.banned` (`saveBans`; the TS
    /// double-write on a perma-ban is collapsed to one).
    PersistBans { entries: Vec<RpcBanEntry> },
    /// A routed call (or base query) concluded — settle the caller.
    Conclude {
        call_id: String,
        verdict: RpcCallVerdict,
    },
}

/// What the shell observed. Every result self-identifies (call_id / chain /
/// url) so a stale answer is dropped by construction, and every time-bearing
/// result carries `now_ms`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum RpcShellResult {
    PoolConfig {
        chain_id: u32,
        rpc_endpoints: Vec<RpcEndpointSeed>,
        bundler_endpoints: Vec<RpcEndpointSeed>,
        now_ms: f64,
    },
    PostOutcome {
        call_id: String,
        url: String,
        outcome: RpcTransportOutcome,
        latency_ms: f64,
        now_ms: f64,
    },
    /// `reported` is the decimal chain id parsed from the probe's hex result;
    /// `None` = unreachable / not valid JSON-RPC / non-finite.
    ChainIdProbed {
        chain_id: u32,
        url: String,
        reported: Option<u32>,
        latency_ms: f64,
        now_ms: f64,
    },
    Jitter {
        call_id: String,
        value: f64,
    },
    BackoffElapsed {
        call_id: String,
        now_ms: f64,
    },
    Persisted,
    Concluded,
}

impl Operation for RpcOperation {
    type Output = RpcShellResult;
}

#[effect]
pub enum RpcEffect {
    Render(RenderOperation),
    Shell(RpcOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "RpcEvent"))]
pub enum Event {
    /// Startup: the persisted ban map (`loadBans`). Honored once — a second
    /// load never clobbers runtime bans (the TS flag, `banLoaded`).
    BansLoaded { entries: Vec<RpcBanEntry> },
    /// A JSON-RPC (or bundler) call wants routing. The shell holds the
    /// payload; `method` is what routing needs (the getLogs range-cap rule).
    /// `call_id` must be fresh — a duplicate id is dropped (fail-closed).
    CallRequested {
        call_id: String,
        chain_id: u32,
        kind: RpcKind,
        method: String,
        now_ms: f64,
    },
    /// Which bundler REST base `/v1/account` and `/v1/sponsor` must use —
    /// the same bundler the pool would submit to (invariant ③,
    /// `getActiveBundlerBaseUrl`).
    BundlerBaseRequested {
        call_id: String,
        chain_id: u32,
        now_ms: f64,
    },
    /// Which RPC URL this chain's pool would reach for first (`getChainRpcUrl`).
    ///
    /// The answer rides `X-Rpc-Url` on the bundler's REST endpoints
    /// (`/v1/account`, and `/v1/sponsor` — a real treasury transfer) and seeds
    /// the local fork simulator, so it is the SECOND consumer of that header
    /// after [`RpcOperation::JsonRpcPost`]. It is therefore answered from the
    /// same score-sorted, ban-filtered ranking the sweep uses, minus every
    /// endpoint that has reported another chain's id — invariant ② governs
    /// both consumers or it governs neither.
    BestRpcUrlRequested {
        call_id: String,
        chain_id: u32,
        now_ms: f64,
    },
    /// Provider keys / service endpoints changed: every pool re-reads config
    /// on next use, and cached fastest-RPC winners are dropped
    /// (`invalidateAllPools`).
    InvalidateAll,
    /// One chain's endpoints changed: reload now and drop its cached winner —
    /// it may point at the endpoint the user just replaced and is handed to
    /// the bundler via `X-Rpc-Url` for up to an hour (`refreshPool`).
    RefreshChain { chain_id: u32 },
    /// Internal: an effect resolved. `attempt` is captured at request time;
    /// this machine never abandons the whole session (bans and stats are
    /// facts, not UI state), so staleness is enforced by construction — every
    /// result names its call/chain/url and is dropped when they no longer
    /// match.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: RpcShellResult,
    },
}

// ---------------------------------------------------------------------------
// Pure scoring / classification — line-by-line ports, pub for the shell's
// synchronous URL-order queries and for the rule tests
// ---------------------------------------------------------------------------

/// Live per-endpoint stats (`EndpointStats` minus the `banned` flag — the ban
/// map is the single truth, invariant ⑧).
#[derive(Clone, Debug, PartialEq)]
pub struct RpcEndpointStats {
    pub url: String,
    pub source: RpcSource,
    /// Exponential moving average, 0 = no sample yet.
    pub avg_latency_ms: f64,
    pub consecutive_failures: u32,
    pub last_failure_at_ms: f64,
    pub total_calls: u64,
    pub total_failures: u64,
}

impl RpcEndpointStats {
    pub fn new(url: String, source: RpcSource) -> Self {
        RpcEndpointStats {
            url,
            source,
            avg_latency_ms: 0.0,
            consecutive_failures: 0,
            last_failure_at_ms: 0.0,
            total_calls: 0,
            total_failures: 0,
        }
    }
}

/// Failure cooldown: 30s · 2^(n-1), capped at 300s (`rpc-pool.ts:414`).
pub fn cooldown_ms(consecutive_failures: u32) -> f64 {
    if consecutive_failures == 0 {
        return 0.0;
    }
    (COOLDOWN_BASE_MS * 2f64.powf(f64::from(consecutive_failures - 1))).min(COOLDOWN_CAP_MS)
}

/// `endpointScore` (`rpc-pool.ts:396-423`) minus the banned branch — banned
/// URLs never reach scoring here (invariant ①/⑧).
pub fn endpoint_score(stats: &RpcEndpointStats, now_ms: f64) -> f64 {
    let mut score = source_priority(stats.source);

    // Latency penalty: -1 per 10ms above 200ms (guard against NaN/Infinity).
    let latency = if stats.avg_latency_ms.is_finite() {
        stats.avg_latency_ms
    } else {
        0.0
    };
    if latency > LATENCY_FREE_MS {
        score -= ((latency - LATENCY_FREE_MS) / 10.0).min(LATENCY_PENALTY_CAP);
    }

    // Reliability bonus.
    let successes = stats.total_calls.saturating_sub(stats.total_failures);
    score += successes.min(RELIABILITY_BONUS_CAP) as f64;

    // Failure penalty with cooldown.
    if stats.consecutive_failures > 0 {
        if now_ms - stats.last_failure_at_ms < cooldown_ms(stats.consecutive_failures) {
            score -= COOLDOWN_SCORE_PENALTY; // effectively disabled during cooldown
        } else {
            score -= f64::from(stats.consecutive_failures) * CONSECUTIVE_FAILURE_PENALTY;
        }
    }

    score
}

/// `recordSuccess` (`rpc-pool.ts:530-539`): EMA 0.7/0.3, first sample taken
/// as-is.
pub fn record_success(stats: &mut RpcEndpointStats, latency_ms: f64) {
    stats.total_calls = stats.total_calls.saturating_add(1);
    stats.consecutive_failures = 0;
    if stats.avg_latency_ms == 0.0 {
        stats.avg_latency_ms = latency_ms;
    } else {
        stats.avg_latency_ms = stats.avg_latency_ms * 0.7 + latency_ms * 0.3;
    }
}

/// `recordFailure` (`rpc-pool.ts:541-546`).
pub fn record_failure(stats: &mut RpcEndpointStats, now_ms: f64) {
    stats.total_calls = stats.total_calls.saturating_add(1);
    stats.total_failures = stats.total_failures.saturating_add(1);
    stats.consecutive_failures = stats.consecutive_failures.saturating_add(1);
    stats.last_failure_at_ms = now_ms;
}

/// `maybePermaBan`'s condition (`rpc-pool.ts:147-154`): the endpoint has
/// NEVER succeeded and has failed ≥ 6 times (invariant ⑦).
pub fn qualifies_for_perma_ban(total_calls: u64, total_failures: u64) -> bool {
    total_calls.saturating_sub(total_failures) == 0 && total_failures >= PERMA_BAN_MIN_FAILURES
}

/// The selection function — score-sorted non-banned URLs, best first
/// (`getSortedEndpoints` with the ban truth applied; invariant ①). Pub so a
/// shell fast path can take the URL order for the same stats snapshot the
/// machine holds. The sort is stable: equal scores keep collection order,
/// which is the cold-start source-priority tiebreak.
pub fn select_urls(
    endpoints: &[RpcEndpointStats],
    bans: &[RpcBanEntry],
    now_ms: f64,
) -> Vec<String> {
    let mut eligible: Vec<&RpcEndpointStats> = endpoints
        .iter()
        .filter(|ep| {
            !bans
                .iter()
                .any(|ban| ban.url == ep.url && is_ban_active(ban, now_ms))
        })
        .collect();
    eligible.sort_by(|a, b| endpoint_score(b, now_ms).total_cmp(&endpoint_score(a, now_ms)));
    eligible.into_iter().map(|ep| ep.url.clone()).collect()
}

/// `isPermanentRpcError` (`rpc-pool.ts:430-448`): the endpoint requires an
/// API key / auth / paid plan — it can never serve us, so ban and fail over.
pub fn is_permanent_rpc_error(error: &RpcErrorInfo) -> bool {
    // JS `!error?.message` — absent AND empty-string messages both bail.
    let Some(message) = error.message.as_deref().filter(|m| !m.is_empty()) else {
        return false;
    };
    let msg = message.to_lowercase();
    [
        "unauthorized",
        "api key",
        "authenticate",
        "forbidden",
        "payment required",
        "exceeded",
        "subscription",
        // Restricted public nodes that reject topic-only getLogs and demand a
        // contract address / paid plan.
        "specify an address",
        "dedicated full node",
    ]
    .iter()
    .any(|needle| msg.contains(needle))
}

/// `isTransientServerError` (`rpc-pool.ts:455-470`): server-side trouble —
/// fail over without banning. Execution errors (revert/gas) are valid
/// answers and are excluded FIRST, even when a server-error code rides along.
pub fn is_transient_server_error(error: &RpcErrorInfo) -> bool {
    let msg = error.message.as_deref().unwrap_or("").to_lowercase();
    if msg.contains("revert") || msg.contains("gas") || msg.contains("execution") {
        return false;
    }
    if let Some(code) = error.code {
        if code == -32603 || (-32099..=-32000).contains(&code) {
            return true;
        }
    }
    [
        "internal error",
        "server error",
        "service unavailable",
        "temporarily unavailable",
        "too many request",
    ]
    .iter()
    .any(|needle| msg.contains(needle))
}

/// `isRateLimitSignal` (`rpc-pool.ts:576-591`): classifies a chain's failure
/// as transient rate-limiting (invariant ④). Never changes the ban/failover
/// decision — that stays with the checks above.
pub fn is_rate_limit_signal(error: &RpcErrorInfo) -> bool {
    let msg = error.message.as_deref().unwrap_or("").to_lowercase();
    if [
        "rate limit",
        "rate-limit",
        "too many request",
        "usage limit",
        "quota",
        "exceeded",
    ]
    .iter()
    .any(|needle| msg.contains(needle))
    {
        return true;
    }
    matches!(error.code, Some(-32005) | Some(-32001) | Some(-32029))
}

/// `getLogsRangeCap` (`rpc-pool.ts:484-519`), invariant ⑤'s classifier.
/// `None` = not a range error; `Some(0)` = range/result cap with no usable
/// number (caller halves); `Some(n)` = the endpoint's stated max block span.
/// f64 because the TS value is a JS number (huge digit runs go to Infinity
/// and fall back to 0, exactly as `Number.isFinite` does).
pub fn get_logs_range_cap(error: &RpcErrorInfo) -> Option<f64> {
    let message = error.message.as_deref().filter(|m| !m.is_empty())?;
    let msg = message.to_lowercase();

    // Result-count caps: narrow the span, but the number is a result count,
    // not a block span — signal "halve".
    if msg.contains("result")
        && (msg.contains("more than")
            || msg.contains("exceed")
            || msg.contains("limit")
            || msg.contains("too many"))
    {
        return Some(0.0);
    }

    // Block-span caps, worded many different ways across providers.
    let is_range_error = msg.contains("block range")
        || msg.contains("block height")
        || msg.contains("too many blocks")
        || msg.contains("range is too")
        || msg.contains("range too")
        || msg.contains("range limit")
        || msg.contains("limited to")
        || (msg.contains("range")
            && (msg.contains("exceed")
                || msg.contains("large")
                || msg.contains("wide")
                || msg.contains("maximum")
                // dRPC's free tier: "ranges over 10000 blocks are not
                // supported on free plan" (answered for a 500-block window on
                // Gnosis, 2026-09-28; 100 blocks pass) — spec 082 T180.
                || msg.contains("blocks")));
    if !is_range_error {
        return None;
    }

    // First `(\d[\d,_]*)\s*([km])?` match — ported verbatim, including the
    // greedy comma run and the stray-suffix quirk (see module doc).
    if let Some((digits, suffix)) = first_number_token(&msg) {
        let cleaned: String = digits.chars().filter(char::is_ascii_digit).collect();
        if let Ok(mut n) = cleaned.parse::<f64>() {
            match suffix {
                Some('k') => n *= 1_000.0,
                Some('m') => n *= 1_000_000.0,
                _ => {}
            }
            if n.is_finite() && n > 0.0 {
                return Some(n);
            }
        }
    }
    // A span written as its bounds — "eth_getLogs is limited to 0 - 50
    // blocks range" (a Gnosis endpoint, 2026-10-07). The first number is the
    // lower bound, 0, which read as "no number"; the incoming scan then
    // retried a window barely narrower, was refused again, and never found a
    // receipt on that chain (issue #443). The upper bound is the span.
    if let Some(upper) = range_upper_bound(&msg) {
        return Some(upper);
    }
    Some(0.0)
}

/// The `M` of the first `N - M` in `msg` (hyphen or en dash, spaces either
/// side optional) when it is a positive number; `None` otherwise.
fn range_upper_bound(msg: &str) -> Option<f64> {
    let chars: Vec<char> = msg.chars().collect();
    let digits_at = |mut i: usize| -> (String, usize) {
        let mut out = String::new();
        while let Some(c) = chars.get(i) {
            if c.is_ascii_digit() || *c == ',' || *c == '_' {
                out.push(*c);
                i += 1;
            } else {
                break;
            }
        }
        (out, i)
    };
    let mut i = 0;
    while i < chars.len() {
        if !chars[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let (_, mut j) = digits_at(i);
        while chars.get(j).is_some_and(|c| c.is_whitespace()) {
            j += 1;
        }
        if chars.get(j).is_some_and(|c| *c == '-' || *c == '–') {
            j += 1;
            while chars.get(j).is_some_and(|c| c.is_whitespace()) {
                j += 1;
            }
            let (upper, _) = digits_at(j);
            let cleaned: String = upper.chars().filter(char::is_ascii_digit).collect();
            return cleaned
                .parse::<f64>()
                .ok()
                .filter(|n| n.is_finite() && *n > 0.0);
        }
        i = j.max(i + 1);
    }
    None
}

/// Methods a healthy node may simply not offer (spec 082 RG7). A JSON error
/// to one is the node's answer — "not served here" — never a fault of the
/// endpoint or of the chain: no ban, no score change, no chain notice or
/// banner. A rate-limit signal on one still fails over.
pub const OPTIONAL_METHODS: &[&str] = &["eth_simulateV1"];

/// Is `method` one of [`OPTIONAL_METHODS`]?
pub fn is_optional_method(method: &str) -> bool {
    OPTIONAL_METHODS.contains(&method)
}

/// Does this JSON-RPC error say the `eth_getLogs` window was too wide — a
/// block-range limit or a result cap (spec 082 T180, ruling 8)? The one rule
/// both readers of that answer use: the pool, which answers it to the caller
/// instead of banning the endpoint or classifying the chain (invariant ⑤),
/// and `tx_tracker`'s find-event, which halves its window on it. It is
/// [`get_logs_range_cap`] as a predicate, so the two can never disagree.
pub fn is_log_range_error(error: &RpcErrorInfo) -> bool {
    get_logs_range_cap(error).is_some()
}

/// Rate-limit WORDS, without the error codes [`is_rate_limit_signal`] also
/// counts: `-32005` is "limit exceeded" to EIP-1474 and a result cap to many
/// log providers ("query returned more than 10000 results"), so on
/// `eth_getLogs` only the wording may overrule a range error.
fn has_rate_limit_wording(error: &RpcErrorInfo) -> bool {
    let msg = error.message.as_deref().unwrap_or("").to_lowercase();
    [
        "rate limit",
        "rate-limit",
        "too many request",
        "usage limit",
        "quota",
    ]
    .iter()
    .any(|needle| msg.contains(needle))
}

/// The `(\d[\d,_]*)\s*([km])?` scan: first digit anywhere, greedy digits/
/// commas/underscores, optional whitespace, optional k/m suffix.
fn first_number_token(msg: &str) -> Option<(String, Option<char>)> {
    let chars: Vec<char> = msg.chars().collect();
    let start = chars.iter().position(char::is_ascii_digit)?;
    let mut digits = String::new();
    let mut i = start;
    while i < chars.len() {
        let Some(c) = chars.get(i) else { break };
        if c.is_ascii_digit() || *c == ',' || *c == '_' {
            digits.push(*c);
            i += 1;
        } else {
            break;
        }
    }
    while chars.get(i).is_some_and(|c| c.is_whitespace()) {
        i += 1;
    }
    let suffix = chars.get(i).copied().filter(|c| *c == 'k' || *c == 'm');
    Some((digits, suffix))
}

/// `backoffWithJitter` (`net.ts:163-166`): full jitter (AWS) —
/// `floor(random · min(cap, base · 2^attempt))`. The random is injected by
/// the shell; a non-finite value yields 0 (JS `setTimeout(NaN)` fires
/// immediately), and out-of-range values are clamped into [0, 1].
pub fn backoff_with_jitter_ms(attempt: u32, base_ms: u32, cap_ms: u32, random01: f64) -> u32 {
    let ceiling = f64::from(cap_ms).min(f64::from(base_ms) * 2f64.powf(f64::from(attempt)));
    let random = if random01.is_finite() {
        random01.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let delay = (random * ceiling).floor();
    if delay.is_finite() && delay > 0.0 {
        delay.min(f64::from(u32::MAX)) as u32
    } else {
        0
    }
}

/// `getActiveBundlerBaseUrl`'s suffix strip: bundler pools store JSON-RPC
/// URLs as `${base}/${chainId}`; the REST base drops that suffix
/// (`rpc-pool.ts:969-974`, regex `/${chainId}/?$`).
pub fn strip_chain_suffix(url: &str, chain_id: u32) -> String {
    let with_slash = format!("/{chain_id}/");
    let bare = format!("/{chain_id}");
    if let Some(stripped) = url.strip_suffix(&with_slash) {
        stripped.to_owned()
    } else if let Some(stripped) = url.strip_suffix(&bare) {
        stripped.to_owned()
    } else {
        url.to_owned()
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Work parked behind a pool load (`initInFlight` dedupes concurrent inits;
/// callers queue rather than double-load).
#[derive(Clone, Debug)]
enum PendingWork {
    Call { call_id: String },
    Base { call_id: String, chain_id: u32 },
    BestRpc { call_id: String, chain_id: u32 },
}

/// Both pools of one chain — `poolInitAt` is keyed by chain in TS because
/// `initPool` always collects RPC and bundler lists together.
#[derive(Debug, Default)]
struct ChainPool {
    rpc: Vec<RpcEndpointStats>,
    bundler: Vec<RpcEndpointStats>,
    loaded_at_ms: Option<f64>,
    loading: bool,
    pending: Vec<PendingWork>,
}

impl ChainPool {
    fn list(&self, kind: RpcKind) -> &Vec<RpcEndpointStats> {
        match kind {
            RpcKind::Rpc => &self.rpc,
            RpcKind::Bundler => &self.bundler,
        }
    }

    fn list_mut(&mut self, kind: RpcKind) -> &mut Vec<RpcEndpointStats> {
        match kind {
            RpcKind::Rpc => &mut self.rpc,
            RpcKind::Bundler => &mut self.bundler,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::enum_variant_names)] // every variant is one kind of waiting — that is the state
enum SessionState {
    /// Queued behind a pool load.
    WaitingPool,
    /// A bundler call waiting on the fastest-RPC ping race.
    WaitingProbe,
    /// One endpoint attempt in flight.
    WaitingPost {
        url: String,
    },
    WaitingJitter,
    WaitingBackoff,
}

/// One routed call in flight.
#[derive(Debug)]
struct CallSession {
    chain_id: u32,
    kind: RpcKind,
    method: String,
    /// 0-based sweep pass (`attempt` in `poolRpcCall`).
    pass: u32,
    /// Remaining URLs of the current pass, best first.
    queue: VecDeque<String>,
    /// Whether any endpoint of the CURRENT pass reported rate-limiting.
    /// Per-pass on purpose — only the final pass classifies the chain
    /// (ported verbatim, see module doc).
    saw_rate_limit: bool,
    /// Whether any endpoint of the CURRENT pass could not be reached at all
    /// ([`is_transport_failure`]). Per pass, like `saw_rate_limit`: a final
    /// pass with one is not "rate-limited" — invariant ④ is for nodes that
    /// answered (spec 092).
    saw_unreached: bool,
    /// The verified same-chain RPC URL for `X-Rpc-Url` (bundler calls).
    x_rpc_url: Option<String>,
    state: SessionState,
    /// Sticky OR of [`may_have_delivered`] over every POST of this call
    /// (spec 082 RA1) — carried on the verdict, never reset between passes.
    maybe_delivered: bool,
    /// First pass only (spec 082 RF1): how many endpoints were tried, and
    /// whether every one of them failed on transport. A pass that swept clean
    /// this way, with no rate-limit signal, puts the chain in
    /// `unreached_chains` without waiting for the later passes.
    first_pass_tried: u32,
    first_pass_all_transport: bool,
}

/// The fastest-RPC ping race (`pickFastestRpcUrl`), run at most once per
/// chain per TTL window; concurrent bundler calls join as waiters.
#[derive(Debug)]
struct ProbeRace {
    outstanding: BTreeSet<String>,
    /// Correct-chain reporters in arrival order (url, latency).
    matches: Vec<(String, f64)>,
    /// URLs that REPORTED a wrong chain — excluded even from the fallback
    /// (invariant ② over the TS code path, see module doc).
    wrong_chain: BTreeSet<String>,
    /// Score-sorted candidates at race start — the TS fallback order.
    ranked: Vec<String>,
    waiters: Vec<String>,
}

#[derive(Debug, Clone)]
struct FastestPick {
    url: String,
    at_ms: f64,
}

#[derive(Default)]
pub struct Model {
    pools: BTreeMap<u32, ChainPool>,
    /// The ONE ban truth (invariant ⑧). Keyed by URL, global across chains,
    /// mirrored to `vela.rpc.banned` via [`RpcOperation::PersistBans`].
    bans: BTreeMap<String, RpcBanEntry>,
    bans_loaded: bool,
    last_ban_prune_ms: Option<f64>,
    /// Chains where ALL RPC endpoints failed on the last attempt. Cleared on
    /// success. Consumed via the view (was `getFailedRpcChains`).
    failed_chains: BTreeSet<u32>,
    /// The transient subset hint: failing (at least partly) due to
    /// rate-limiting — the UI must NOT nag the user to swap RPCs for these
    /// (invariant ④; was `getRateLimitedChains`).
    rate_limited_chains: BTreeSet<u32>,
    /// Chains whose RPC endpoints ALL failed on transport in one call's first
    /// pass, with no rate-limit signal (spec 082 RF1). Earlier than
    /// `failed_chains` (one pass, not three) and cleared by the same usable
    /// answer. The browser's chain notice reads `failed ∪ unreached`; the home
    /// banner keeps `failed_chains` alone.
    unreached_chains: BTreeSet<u32>,
    /// Cached fastest-RPC winner per chain, 1h TTL (`fastestRpcCache`).
    fastest: BTreeMap<u32, FastestPick>,
    /// Per chain, the URLs that answered `eth_chainId` with a DIFFERENT id.
    ///
    /// [`ProbeRace::wrong_chain`] knows this too, but only for the length of one
    /// race — it dies with the race that learned it. Invariant ② has to outlive
    /// that: [`Event::BestRpcUrlRequested`] hands its answer to `/v1/sponsor`,
    /// which spends the treasury based on what that endpoint reports about the
    /// Safe, and it runs nowhere near a race. Proof, never suspicion: only a
    /// positive disagreeing id lands here (a timeout or a refusal is
    /// "unverified" and changes nothing), which is why it can safely exclude.
    /// Cleared wherever `fastest` is, since a config change can put a different
    /// node behind the same URL.
    ///
    /// Read by BOTH consumers of `X-Rpc-Url`, which is the whole point of
    /// keeping it: [`answer_best_rpc_url`] for the REST leg, and
    /// [`begin_bundler_call`] for the JSON-RPC leg, where it filters the
    /// candidate set BEFORE the single-candidate short-circuit. A pool that
    /// has shrunk to one endpoint is exactly when the two legs would otherwise
    /// disagree — the REST leg answering `None`, the JSON-RPC leg submitting a
    /// UserOp through the condemned node.
    wrong_chain: BTreeMap<u32, BTreeSet<String>>,
    probes: BTreeMap<u32, ProbeRace>,
    calls: BTreeMap<String, CallSession>,
    /// Captured into every request. Never bumped: no event abandons the
    /// resident pool state — staleness is enforced per result by
    /// construction (see [`Event::ShellCompleted`]).
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RpcPoolView {
    /// Chains whose whole RPC pool failed on the last attempt — the
    /// stale-balance notice source. Sorted ascending.
    pub failed_chains: Vec<u32>,
    /// The transient subset: failing due to rate-limiting. A chain listed
    /// here must keep the cached balance and NEVER show the "swap in your
    /// own RPC" banner (invariant ④).
    pub rate_limited_chains: Vec<u32>,
    /// The ban map as persisted (expired entries may linger until the next
    /// prune — check [`is_ban_active`] with a current timestamp).
    pub banned: Vec<RpcBanEntry>,
    /// Chains one call's first pass could not reach at all — every endpoint
    /// failed on transport, no rate-limit signal (spec 082 RF1). Sorted
    /// ascending. The chain notice is `chain ∈ failed_chains ∪
    /// unreached_chains ∧ chain ∉ rate_limited_chains`; the home "fix your
    /// RPC" banner still reads `failed_chains` only.
    #[serde(default)]
    pub unreached_chains: Vec<u32>,
    /// Per routed call, the endpoints its current pass has still to try, in
    /// the order it will try them (the endpoint it is waiting on is already
    /// off the list). For a shell that asks the next endpoint early — the
    /// desktop's hedged reads, spec 083 H6 — so the early ask goes where this
    /// machine goes next, past its cooldowns and 429 back-offs, never around
    /// them. Shell-local: never serialized, never in the bindings.
    #[serde(skip)]
    #[cfg_attr(feature = "bindings", ts(skip))]
    pub pending_urls: BTreeMap<String, Vec<String>>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct RpcPool;

impl App for RpcPool {
    type Event = Event;
    type Model = Model;
    type ViewModel = RpcPoolView;
    type Effect = RpcEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<RpcEffect, Event> {
        match event {
            Event::BansLoaded { entries } => {
                if model.bans_loaded {
                    return Command::done();
                }
                model.bans_loaded = true;
                for entry in entries {
                    // Stored entries overwrite in-memory ones (`banMap.set`).
                    model.bans.insert(entry.url.clone(), entry);
                }
                render()
            }
            Event::CallRequested {
                call_id,
                chain_id,
                kind,
                method,
                now_ms,
            } => {
                if model.calls.contains_key(&call_id) {
                    // A duplicate correlation id could cross two callers'
                    // answers — fail closed, drop the second request.
                    return Command::done();
                }
                model.calls.insert(
                    call_id.clone(),
                    CallSession {
                        chain_id,
                        kind,
                        method,
                        pass: 0,
                        queue: VecDeque::new(),
                        saw_rate_limit: false,
                        saw_unreached: false,
                        x_rpc_url: None,
                        state: SessionState::WaitingPool,
                        maybe_delivered: false,
                        first_pass_tried: 0,
                        first_pass_all_transport: true,
                    },
                );
                ensure_pool_then(model, chain_id, PendingWork::Call { call_id }, now_ms)
            }
            Event::BundlerBaseRequested {
                call_id,
                chain_id,
                now_ms,
            } => ensure_pool_then(
                model,
                chain_id,
                PendingWork::Base { call_id, chain_id },
                now_ms,
            ),
            Event::BestRpcUrlRequested {
                call_id,
                chain_id,
                now_ms,
            } => ensure_pool_then(
                model,
                chain_id,
                PendingWork::BestRpc { call_id, chain_id },
                now_ms,
            ),
            Event::InvalidateAll => {
                for pool in model.pools.values_mut() {
                    pool.loaded_at_ms = None;
                }
                // After a provider-key change the old winner would otherwise
                // ride `X-Rpc-Url` for up to an hour.
                model.fastest.clear();
                model.wrong_chain.clear();
                Command::done()
            }
            Event::RefreshChain { chain_id } => {
                model.fastest.remove(&chain_id);
                model.wrong_chain.remove(&chain_id);
                let pool = model.pools.entry(chain_id).or_default();
                pool.loaded_at_ms = None;
                if pool.loading {
                    return Command::done();
                }
                pool.loading = true;
                request(model, RpcOperation::LoadPoolConfig { chain_id })
            }
            Event::ShellCompleted { attempt: _, result } => accept(model, result),
        }
    }

    fn view(&self, model: &Model) -> RpcPoolView {
        RpcPoolView {
            failed_chains: model.failed_chains.iter().copied().collect(),
            rate_limited_chains: model.rate_limited_chains.iter().copied().collect(),
            banned: model.bans.values().cloned().collect(),
            unreached_chains: model.unreached_chains.iter().copied().collect(),
            pending_urls: model
                .calls
                .iter()
                .filter(|(_, session)| !session.queue.is_empty())
                .map(|(call_id, session)| {
                    (call_id.clone(), session.queue.iter().cloned().collect())
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: RpcShellResult) -> Command<RpcEffect, Event> {
    match result {
        RpcShellResult::PoolConfig {
            chain_id,
            rpc_endpoints,
            bundler_endpoints,
            now_ms,
        } => {
            let Some(pool) = model.pools.get_mut(&chain_id) else {
                return Command::done();
            };
            if !pool.loading {
                // Unsolicited config — dropped by construction.
                return Command::done();
            }
            pool.loading = false;
            pool.loaded_at_ms = Some(now_ms);
            pool.rpc = merge_endpoints(std::mem::take(&mut pool.rpc), rpc_endpoints);
            pool.bundler = merge_endpoints(std::mem::take(&mut pool.bundler), bundler_endpoints);
            let pending = std::mem::take(&mut pool.pending);
            let mut commands = Vec::new();
            for work in pending {
                commands.push(start_work(model, work, now_ms));
            }
            Command::all(commands)
        }
        RpcShellResult::PostOutcome {
            call_id,
            url,
            outcome,
            latency_ms,
            now_ms,
        } => handle_outcome(model, &call_id, &url, outcome, latency_ms, now_ms),
        RpcShellResult::ChainIdProbed {
            chain_id,
            url,
            reported,
            latency_ms,
            now_ms,
        } => handle_probe(model, chain_id, &url, reported, latency_ms, now_ms),
        RpcShellResult::Jitter { call_id, value } => {
            let Some(session) = model.calls.get_mut(&call_id) else {
                return Command::done();
            };
            if session.state != SessionState::WaitingJitter {
                return Command::done();
            }
            let (base_ms, cap_ms) = backoff_params(session.kind);
            let delay_ms = backoff_with_jitter_ms(session.pass, base_ms, cap_ms, value);
            session.state = SessionState::WaitingBackoff;
            request(model, RpcOperation::StartBackoff { call_id, delay_ms })
        }
        RpcShellResult::BackoffElapsed { call_id, now_ms } => {
            let Some(session) = model.calls.get_mut(&call_id) else {
                return Command::done();
            };
            if session.state != SessionState::WaitingBackoff {
                return Command::done();
            }
            session.pass = session.pass.saturating_add(1);
            start_pass(model, &call_id, now_ms)
        }
        RpcShellResult::Persisted | RpcShellResult::Concluded => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Pool lifecycle
// ---------------------------------------------------------------------------

/// `ensurePool`: fresh within 10 minutes → run the work now; otherwise queue
/// it behind one (deduplicated) config load.
fn ensure_pool_then(
    model: &mut Model,
    chain_id: u32,
    work: PendingWork,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    let pool = model.pools.entry(chain_id).or_default();
    let fresh = pool
        .loaded_at_ms
        .is_some_and(|loaded| now_ms - loaded < POOL_REFRESH_MS);
    if fresh {
        return start_work(model, work, now_ms);
    }
    pool.pending.push(work);
    if pool.loading {
        return Command::done(); // `initInFlight` dedupe
    }
    pool.loading = true;
    request(model, RpcOperation::LoadPoolConfig { chain_id })
}

/// `mergeEndpoints` (`rpc-pool.ts:254-280`): keep stats for URLs that stay,
/// update their source in case it changed, drop URLs no longer collected.
/// Empty URLs and duplicates are skipped (`add`'s guard); banned URLs are
/// KEPT — the ban truth is applied at selection, not membership (invariant ⑧).
fn merge_endpoints(
    existing: Vec<RpcEndpointStats>,
    seeds: Vec<RpcEndpointSeed>,
) -> Vec<RpcEndpointStats> {
    let mut by_url: BTreeMap<String, RpcEndpointStats> = existing
        .into_iter()
        .map(|stats| (stats.url.clone(), stats))
        .collect();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut merged = Vec::new();
    for seed in seeds {
        if seed.url.is_empty() || !seen.insert(seed.url.clone()) {
            continue;
        }
        if let Some(mut prev) = by_url.remove(&seed.url) {
            prev.source = seed.source;
            merged.push(prev);
        } else {
            merged.push(RpcEndpointStats::new(seed.url, seed.source));
        }
    }
    merged
}

fn start_work(model: &mut Model, work: PendingWork, now_ms: f64) -> Command<RpcEffect, Event> {
    match work {
        PendingWork::Call { call_id } => start_call(model, &call_id, now_ms),
        PendingWork::Base { call_id, chain_id } => {
            answer_bundler_base(model, &call_id, chain_id, now_ms)
        }
        PendingWork::BestRpc { call_id, chain_id } => {
            answer_best_rpc_url(model, &call_id, chain_id, now_ms)
        }
    }
}

fn start_call(model: &mut Model, call_id: &str, now_ms: f64) -> Command<RpcEffect, Event> {
    let Some(session) = model.calls.get(call_id) else {
        return Command::done();
    };
    match session.kind {
        RpcKind::Rpc => start_pass(model, call_id, now_ms),
        RpcKind::Bundler => begin_bundler_call(model, call_id, now_ms),
    }
}

// ---------------------------------------------------------------------------
// Fastest-RPC pick (X-Rpc-Url) — `pickFastestRpcUrl`
// ---------------------------------------------------------------------------

/// **The** predicate behind every `X-Rpc-Url` this core will name for a chain,
/// as a set: the chain's RPC endpoints in [`select_urls`] order (six-tier
/// source priority, EMA latency penalty, failure cooldown, and the single ban
/// truth re-checked live against `now_ms`), minus every endpoint that has
/// PROVED it serves another chain (invariant ②).
///
/// Both legs that can put a URL in front of the bundler read it — the JSON-RPC
/// leg through [`begin_bundler_call`] and the REST leg through
/// [`answer_best_rpc_url`] — so "may this endpoint be handed to the bundler"
/// has exactly one answer per chain, whichever door the question comes in by.
/// It is deliberately a SET rather than four scattered checks: the empty case,
/// the single-candidate case, the cached-fastest case and the race seed are all
/// phrased against it, and none of them can grow a private notion of eligible.
///
/// What that unification cost when it was missing: the `fastest` short-circuit
/// re-checked `wrong_chain` but not the ban map, so for the whole
/// `FASTEST_RPC_TTL_MS` (one hour) after a race, `eth_sendUserOperation` rode a
/// header naming an endpoint the pool had banned on a 401 — while
/// `getChainRpcUrl`, asked about the very same chain in the very same second,
/// correctly refused to name it. One pool, two opinions.
///
/// Emptiness is an honest answer, never a reason to relax: the caller omits the
/// header rather than fall back to a URL this predicate has already rejected.
fn bundler_eligible_urls(model: &Model, chain_id: u32, now_ms: f64) -> Vec<String> {
    let bans: Vec<RpcBanEntry> = model.bans.values().cloned().collect();
    let wrong = model.wrong_chain.get(&chain_id);
    model
        .pools
        .get(&chain_id)
        .map(|pool| select_urls(pool.list(RpcKind::Rpc), &bans, now_ms))
        .unwrap_or_default()
        .into_iter()
        .filter(|candidate| !wrong.is_some_and(|set| set.contains(candidate)))
        .collect()
}

fn begin_bundler_call(model: &mut Model, call_id: &str, now_ms: f64) -> Command<RpcEffect, Event> {
    let Some(session) = model.calls.get(call_id) else {
        return Command::done();
    };
    let chain_id = session.chain_id;

    // The candidate set IS the predicate (see `bundler_eligible_urls`), applied
    // here once — not re-derived at the exits below, where the single-candidate
    // short-circuit used to skip invariant ② and the cached-fastest
    // short-circuit used to skip the ban map, both submitting a UserOp with
    // `X-Rpc-Url` pointing at a node the pool had already condemned (and which
    // `answer_best_rpc_url` was refusing to name for the very same chain).
    // Cleared with `fastest` on `RefreshChain` / `InvalidateAll`, so a config
    // change re-admits the URL.
    let ranked = bundler_eligible_urls(model, chain_id, now_ms);

    if ranked.is_empty() {
        // Nothing left to vouch for — every endpoint banned, or every endpoint
        // proved to be another chain's. No header is the honest answer;
        // falling back to an endpoint this predicate rejected is the one thing
        // invariant ② forbids (same fail-closed exit as `handle_probe`'s
        // fallback). Note there is deliberately no all-banned RESCUE here: the
        // sweep may rescue a chain's pool to keep it reachable at all, but a
        // rescued endpoint has no business being named to the bundler until it
        // has earned its way back through a selection — which is exactly why
        // `select_with_rescue` no longer deletes the ban entries this predicate
        // reads. The sweep's rescue and this refusal used to contradict each
        // other in the same file.
        return set_x_and_sweep(model, call_id, None, now_ms);
    }
    if ranked.len() == 1 {
        // Single eligible endpoint: used directly, and — ported — NOT cached.
        let only = ranked.first().cloned();
        return set_x_and_sweep(model, call_id, only, now_ms);
    }
    if let Some(pick) = model.fastest.get(&chain_id) {
        // A cached winner came from `race.matches` — an endpoint that reported
        // THIS chain's id — but that was up to an hour ago, and everything the
        // pool has learned since is a reason it may no longer be eligible: a
        // 401 that banned it, a rate-limit ban, a wrong-chain report from a
        // later race, or a config change that dropped it from the pool. So the
        // winner is not trusted for being a winner; it is re-asked the ONE
        // question, by membership in the set above. A winner that no longer
        // belongs falls through to a fresh race rather than riding the header.
        let eligible = ranked.iter().any(|candidate| candidate == &pick.url);
        if now_ms - pick.at_ms < FASTEST_RPC_TTL_MS && eligible {
            let url = Some(pick.url.clone());
            return set_x_and_sweep(model, call_id, url, now_ms);
        }
    }

    // Join a running race, or start one.
    if let Some(race) = model.probes.get_mut(&chain_id) {
        race.waiters.push(call_id.to_owned());
        if let Some(session) = model.calls.get_mut(call_id) {
            session.state = SessionState::WaitingProbe;
        }
        return Command::done();
    }
    let outstanding: BTreeSet<String> = ranked.iter().cloned().collect();
    model.probes.insert(
        chain_id,
        ProbeRace {
            outstanding,
            matches: Vec::new(),
            wrong_chain: BTreeSet::new(),
            ranked: ranked.clone(),
            waiters: vec![call_id.to_owned()],
        },
    );
    if let Some(session) = model.calls.get_mut(call_id) {
        session.state = SessionState::WaitingProbe;
    }
    let mut commands = Vec::new();
    for url in ranked {
        commands.push(request(
            model,
            RpcOperation::ProbeChainId {
                chain_id,
                url,
                timeout_ms: PING_TIMEOUT_MS,
            },
        ));
    }
    Command::all(commands)
}

fn set_x_and_sweep(
    model: &mut Model,
    call_id: &str,
    x_rpc_url: Option<String>,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    if let Some(session) = model.calls.get_mut(call_id) {
        session.x_rpc_url = x_rpc_url;
    }
    start_pass(model, call_id, now_ms)
}

fn handle_probe(
    model: &mut Model,
    chain_id: u32,
    url: &str,
    reported: Option<u32>,
    latency_ms: f64,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    let Some(race) = model.probes.get_mut(&chain_id) else {
        return Command::done();
    };
    if !race.outstanding.remove(url) {
        return Command::done();
    }
    let mut proved_wrong_chain = false;
    match reported {
        Some(id) if id == chain_id => race.matches.push((url.to_owned(), latency_ms)),
        // Invariant ②: a fast endpoint on the WRONG chain must never be
        // handed to the bundler as this chain — remembered so it cannot even
        // win the fallback below.
        Some(_) => {
            race.wrong_chain.insert(url.to_owned());
            proved_wrong_chain = true;
        }
        // Timeout / network / invalid answer — skipped silently.
        None => {}
    }
    if proved_wrong_chain {
        // The same fact, kept past this race: `getChainRpcUrl` hands `X-Rpc-Url`
        // to `/v1/sponsor` and `/v1/account` without ever running a race, and it
        // must be bound by invariant ② too.
        model
            .wrong_chain
            .entry(chain_id)
            .or_default()
            .insert(url.to_owned());
    }
    let Some(race) = model.probes.get_mut(&chain_id) else {
        return Command::done();
    };
    if !race.outstanding.is_empty() {
        return Command::done();
    }
    let Some(race) = model.probes.remove(&chain_id) else {
        return Command::done();
    };

    // A probe race is up to `PING_TIMEOUT_MS` wide, and everything the pool
    // learned inside that window still binds: a 401/403 ban landed by the
    // JSON-RPC leg, a rate-limit ban, another chain's proof from a concurrent
    // race, a `RefreshChain` that dropped the endpoint from the pool. Winning a
    // race is not a licence. So the race results are not consulted directly —
    // they are intersected with [`bundler_eligible_urls`], the same single
    // predicate `begin_bundler_call` and `answer_best_rpc_url` ask, re-derived
    // HERE against the pool as it stands now that the race has landed.
    //
    // This was the last door into `X-Rpc-Url` still deciding for itself:
    // an endpoint banned mid-race rode the header on `eth_sendUserOperation`
    // while `getChainRpcUrl`, asked about the same chain in the same instant,
    // correctly refused to name it. One pool, two opinions, again.
    let eligible = bundler_eligible_urls(model, chain_id, now_ms);
    let is_eligible = |url: &str| eligible.iter().any(|candidate| candidate == url);

    let winner = if race.matches.is_empty() {
        // TS falls back to the score-sorted first endpoint unverified; here a
        // wrong-chain REPORTER is excluded even from the fallback, and so is
        // anything the predicate no longer admits. If nothing survives, the
        // header is omitted entirely (fail-closed — invariant ② over
        // `rpc-pool.ts:689-692`). Fallback winners are not cached, as in TS.
        race.ranked
            .iter()
            .find(|candidate| !race.wrong_chain.contains(*candidate) && is_eligible(candidate))
            .cloned()
    } else {
        // Lowest latency wins; ties keep arrival order (the TS stable sort).
        // Endpoints that verified this chain but have since become ineligible
        // are simply not candidates — and if that empties the field, the
        // answer is no header, NOT a demotion to the unverified fallback.
        let mut best: Option<&(String, f64)> = None;
        for entry in race.matches.iter().filter(|(url, _)| is_eligible(url)) {
            match best {
                Some(current) if entry.1 >= current.1 => {}
                _ => best = Some(entry),
            }
        }
        let winner = best.map(|(url, _)| url.clone());
        // Only a verified AND still-eligible winner earns the hour-long cache;
        // a condemned endpoint must not be parked there to be re-litigated.
        if let Some(url) = &winner {
            model.fastest.insert(
                chain_id,
                FastestPick {
                    url: url.clone(),
                    at_ms: now_ms,
                },
            );
        }
        winner
    };

    let mut commands = Vec::new();
    for waiter in race.waiters {
        if let Some(session) = model.calls.get_mut(&waiter) {
            session.x_rpc_url = winner.clone();
        }
        commands.push(start_pass(model, &waiter, now_ms));
    }
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------

fn backoff_params(kind: RpcKind) -> (u32, u32) {
    match kind {
        RpcKind::Rpc => (RPC_BACKOFF_BASE_MS, RPC_BACKOFF_CAP_MS),
        RpcKind::Bundler => (BUNDLER_BACKOFF_BASE_MS, BUNDLER_BACKOFF_CAP_MS),
    }
}

fn max_passes(kind: RpcKind) -> u32 {
    match kind {
        RpcKind::Rpc => MAX_RPC_ATTEMPTS,
        RpcKind::Bundler => MAX_BUNDLER_ATTEMPTS,
    }
}

/// Begin one full-pool pass: prune expired bans (throttled), select the
/// score-sorted eligible URLs — rescuing an all-banned pool (invariant ⑥) —
/// and try the first endpoint.
fn start_pass(model: &mut Model, call_id: &str, now_ms: f64) -> Command<RpcEffect, Event> {
    let Some(session) = model.calls.get(call_id) else {
        return Command::done();
    };
    let (chain_id, kind) = (session.chain_id, session.kind);

    let pruned = prune_expired_bans(model, now_ms);
    let urls = select_with_rescue(model, chain_id, kind, now_ms);

    if let Some(session) = model.calls.get_mut(call_id) {
        session.queue = urls.into_iter().collect();
        // Per-pass on purpose: only the final pass classifies the chain
        // (ported verbatim, see module doc).
        session.saw_rate_limit = false;
        session.saw_unreached = false;
    }

    let mut commands = Vec::new();
    // Only the prune changes the ban record. The rescue deliberately does not
    // (see `select_with_rescue`), so there is nothing to persist and nothing
    // for a ban-watching surface to re-read: an endpoint a pass had to fall
    // back onto is still a banned endpoint everywhere else.
    if pruned {
        let entries: Vec<RpcBanEntry> = model.bans.values().cloned().collect();
        commands.push(request(model, RpcOperation::PersistBans { entries }));
        commands.push(render());
    }
    commands.push(next_endpoint(model, call_id, now_ms));
    Command::all(commands)
}

/// Selection with the all-banned self-rescue (`rpc-pool.ts:745-755`,
/// invariant ⑥): when every member of THIS pool is banned, this pass proceeds
/// anyway — over the bans, in score order — and the consecutive-failure
/// counters reset, so a chain whose whole pool stands condemned (a permanent
/// 401 on its only endpoint, say) is still reachable and can still recover.
///
/// The rescue is scoped to the pass that needed it, and that scope is the whole
/// point. It used to `model.bans.remove(..)`, which is a GLOBAL edit: one
/// ordinary `eth_call` on a dead pool deleted yesterday's permanent ban, and
/// the endpoint was thereby re-admitted to [`bundler_eligible_urls`] — named to
/// the bundler in `X-Rpc-Url` by both legs, having answered nothing.
/// [`begin_bundler_call`] says in so many words that a rescued endpoint has no
/// business being named to the bundler until it has earned its way back through
/// a selection; the code did the exact opposite of its own comment. Now the ban
/// record stands (so every other reader of the pool still sees the truth) and
/// merely stops binding the one leg that has nothing else left.
///
/// Consequence, deliberately: the rescue writes nothing, so it persists
/// nothing — there is no longer a "rescued" outcome for the caller to react to.
/// A ban survives the rescue and a restart alike, and the endpoint is re-tried
/// on every pass of a pool that has no alternative, which is where
/// `rpc-pool.ts` also ends up (its cleared ban is re-applied by the next 401).
fn select_with_rescue(model: &mut Model, chain_id: u32, kind: RpcKind, now_ms: f64) -> Vec<String> {
    let Some(pool) = model.pools.get_mut(&chain_id) else {
        return Vec::new();
    };
    let list = pool.list_mut(kind);
    let bans: Vec<RpcBanEntry> = model.bans.values().cloned().collect();
    let urls = select_urls(list, &bans, now_ms);
    if !urls.is_empty() || list.is_empty() {
        return urls;
    }
    for endpoint in list.iter_mut() {
        endpoint.consecutive_failures = 0;
    }
    // No bans handed in: this pass, and only this pass, proceeds over them.
    select_urls(list, &[], now_ms)
}

/// `pruneExpiredBans` (`rpc-pool.ts:98-116`): throttled sweep; persists only
/// when something was removed. (The core checks ban expiry live at every
/// selection, so this is persistence hygiene, not a decision.)
fn prune_expired_bans(model: &mut Model, now_ms: f64) -> bool {
    let due = model
        .last_ban_prune_ms
        .is_none_or(|last| now_ms - last >= BAN_PRUNE_INTERVAL_MS);
    if !due {
        return false;
    }
    model.last_ban_prune_ms = Some(now_ms);
    let before = model.bans.len();
    model.bans.retain(|_, entry| is_ban_active(entry, now_ms));
    before != model.bans.len()
}

/// Re-ask [`bundler_eligible_urls`] about the header this session is carrying,
/// immediately before it goes out.
///
/// `session.x_rpc_url` is chosen ONCE — at [`set_x_and_sweep`] or when a probe
/// race lands — and a bundler call is then up to two passes of several POSTs
/// each. Everything the pool learns in between is a reason the chosen endpoint
/// may no longer be eligible, and until this existed none of it was consulted
/// again: an endpoint that won the race and was banned a moment later (403 on
/// a concurrent `eth_call`) kept riding the header for the rest of the call,
/// while `getChainRpcUrl`, asked about the same chain in the same second,
/// refused to name it. The sixth door into `X-Rpc-Url`, and the last one that
/// still decided for itself.
///
/// It is a *re-ask*, not a second opinion: the predicate is the same single
/// function all the other doors ask. An endpoint that is still eligible is
/// kept (so a healthy call does not wander between nodes); one that is not is
/// replaced by the best endpoint that is; and when nothing qualifies the header
/// is dropped rather than sent knowingly wrong — the same honest failure
/// `begin_bundler_call` and `handle_probe` end in.
///
/// A header that is deliberately ABSENT stays absent. `begin_bundler_call` and
/// `handle_probe` both have exits that mean "nothing here has earned the
/// header"; promoting a URL here would quietly overrule them.
fn revalidated_x_rpc_url(
    model: &mut Model,
    call_id: &str,
    chain_id: u32,
    now_ms: f64,
) -> Option<String> {
    let held = model.calls.get(call_id)?.x_rpc_url.clone()?;
    let eligible = bundler_eligible_urls(model, chain_id, now_ms);
    let next = if eligible.contains(&held) {
        Some(held)
    } else {
        eligible.into_iter().next()
    };
    // Written back so the session, the POST and any later pass all name the
    // same endpoint — one truth per session, as with the pool's one ban truth.
    if let Some(session) = model.calls.get_mut(call_id) {
        session.x_rpc_url = next.clone();
    }
    next
}

/// Try the next endpoint of the current pass, or close the pass: another
/// jittered-backoff pass while attempts remain, else the chain-level verdict.
fn next_endpoint(model: &mut Model, call_id: &str, now_ms: f64) -> Command<RpcEffect, Event> {
    let Some(session) = model.calls.get_mut(call_id) else {
        return Command::done();
    };
    if let Some(url) = session.queue.pop_front() {
        session.state = SessionState::WaitingPost { url: url.clone() };
        let method = session.method.clone();
        let (kind, chain_id) = (session.kind, session.chain_id);
        let (x_rpc_url, timeout_ms) = match kind {
            RpcKind::Rpc => (None, RPC_READ_TIMEOUT_MS),
            RpcKind::Bundler => (
                revalidated_x_rpc_url(model, call_id, chain_id, now_ms),
                BUNDLER_RPC_TIMEOUT_MS,
            ),
        };
        return request(
            model,
            RpcOperation::JsonRpcPost {
                call_id: call_id.to_owned(),
                url,
                method,
                x_rpc_url,
                timeout_ms,
            },
        );
    }

    // Pass swept clean.
    if session.pass == 0
        && session.kind == RpcKind::Rpc
        && session.first_pass_tried > 0
        && session.first_pass_all_transport
        && !session.saw_rate_limit
        && !is_optional_method(&session.method)
    {
        // RF1: nothing on this chain answered at all — say so after one pass
        // (endpoints × 8 s), not three. `failed_chains` keeps its meaning.
        let chain_id = session.chain_id;
        model.unreached_chains.insert(chain_id);
    }
    let Some(session) = model.calls.get_mut(call_id) else {
        return Command::done();
    };
    if session.pass + 1 < max_passes(session.kind) {
        session.state = SessionState::WaitingJitter;
        return request(
            model,
            RpcOperation::DrawJitter {
                call_id: call_id.to_owned(),
            },
        );
    }
    conclude_failed(model, call_id)
}

/// Every endpoint failed every pass — classify the chain (RPC only:
/// rate-limited → keep cached balances and stay quiet; hard failure → the
/// fix-your-RPC banner may show) and settle the caller.
///
/// Rate-limited means the final pass's nodes answered, if only to throttle:
/// one node saying 429 while the others could not be reached at all is a
/// chain that cannot be read, not a busy one (spec 092, invariant ④).
fn conclude_failed(model: &mut Model, call_id: &str) -> Command<RpcEffect, Event> {
    let Some(session) = model.calls.remove(call_id) else {
        return Command::done();
    };
    let throttled = session.saw_rate_limit && !session.saw_unreached;
    let rate_limited = match session.kind {
        // Spec 082 RG7: a method a node may simply not offer never says
        // anything about the chain — neither banner nor notice.
        RpcKind::Rpc if is_optional_method(&session.method) => throttled,
        RpcKind::Rpc => {
            model.failed_chains.insert(session.chain_id);
            if throttled {
                model.rate_limited_chains.insert(session.chain_id);
            } else {
                model.rate_limited_chains.remove(&session.chain_id);
            }
            throttled
        }
        // The bundler path never classifies chains (`poolBundlerCall`).
        RpcKind::Bundler => false,
    };
    Command::all([
        request(
            model,
            RpcOperation::Conclude {
                call_id: call_id.to_owned(),
                verdict: RpcCallVerdict::Failed {
                    rate_limited,
                    maybe_delivered: session.maybe_delivered,
                },
            },
        ),
        render(),
    ])
}

// ---------------------------------------------------------------------------
// Outcome routing — the four-way classification applied
// ---------------------------------------------------------------------------

/// What one endpoint attempt means for the route.
enum Route {
    /// A usable answer (including valid execution errors) — deliver it.
    Success,
    /// getLogs range/size cap — deliver to the caller to split (invariant ⑤).
    RangeCap(f64),
    /// Permanently unusable here (auth / API key / HTTP 401/403/404) — ban
    /// and fail over. `rate_limit_signal` mirrors the TS quirk that only
    /// RESPONSE-classified permanent errors feed `sawRateLimit`.
    Ban { rate_limit_signal: bool },
    /// Transient server error — fail over without banning.
    Transient { rate_limit_signal: bool },
    /// HTTP 429 — scoring cooldown only, never a hard ban.
    RateLimited429,
    /// Timeout / network / non-JSON / other HTTP status.
    PlainFailure,
    /// An optional method's JSON error (spec 082 RG7): the node answered that
    /// it does not serve this — delivered as an answer, with no score change,
    /// no ban and no chain classification.
    NotServed,
}

/// Nothing usable came back from the endpoint at all — the RF1 notion of
/// "unreached": no socket, no answer in time, a reset, a body that is not
/// JSON, or a gateway's 5xx in front of a dead node. A JSON answer (even an
/// error), a ban-class status and a 429 all mean something answered.
fn is_transport_failure(outcome: &RpcTransportOutcome) -> bool {
    match outcome {
        RpcTransportOutcome::Timeout
        | RpcTransportOutcome::Network
        | RpcTransportOutcome::NotConnected
        | RpcTransportOutcome::NonJson => true,
        RpcTransportOutcome::HttpError { status } => (500..=599).contains(status),
        RpcTransportOutcome::Response { .. } => false,
    }
}

fn classify_response_error(error: &RpcErrorInfo) -> Route {
    if is_permanent_rpc_error(error) {
        Route::Ban {
            rate_limit_signal: is_rate_limit_signal(error),
        }
    } else if is_transient_server_error(error) {
        Route::Transient {
            rate_limit_signal: is_rate_limit_signal(error),
        }
    } else {
        // Execution errors ("revert", "out of gas") are valid responses.
        Route::Success
    }
}

/// What one endpoint's outcome means for the route — the four-way
/// classification, applied.
fn route_of(kind: RpcKind, method: &str, outcome: &RpcTransportOutcome) -> Route {
    match outcome {
        RpcTransportOutcome::Response { error: None } => Route::Success,
        RpcTransportOutcome::Response { error: Some(error) } => {
            // The range check MUST come before the permanent/transient checks:
            // these errors often carry "exceed" or a -32000 code that would
            // otherwise (wrongly) ban or fail over the endpoint
            // (`rpc-pool.ts:768-772`). RPC calls only, as in TS. Rate-limit
            // wording goes first (spec 082 T180): a throttled node is not a
            // range limit, and the caller must not shrink its window for it.
            // Nor is it a ban (contract §2): the find-event reads `eth_getLogs`
            // every tick while an op may have been sent, and "exceeded" would
            // otherwise ban a throttled node for an hour for every method.
            if kind == RpcKind::Rpc && method == "eth_getLogs" {
                if has_rate_limit_wording(error) {
                    Route::Transient {
                        rate_limit_signal: true,
                    }
                } else if let Some(cap) = get_logs_range_cap(error) {
                    Route::RangeCap(cap)
                } else if is_rate_limit_signal(error) {
                    Route::Transient {
                        rate_limit_signal: true,
                    }
                } else {
                    classify_response_error(error)
                }
            } else if kind == RpcKind::Rpc && is_optional_method(method) {
                // Spec 082 RG7: plan words or `-32603 "method handler
                // crashed"` from a node that cannot simulate must not ban it
                // for `eth_call`, nor raise Arbitrum's notice. Throttling still
                // fails over, without a ban.
                if is_rate_limit_signal(error) {
                    Route::Transient {
                        rate_limit_signal: true,
                    }
                } else {
                    Route::NotServed
                }
            } else {
                classify_response_error(error)
            }
        }
        RpcTransportOutcome::HttpError {
            status: 401 | 403 | 404,
        } => Route::Ban {
            // The HttpBanError catch never feeds `sawRateLimit` (ported).
            rate_limit_signal: false,
        },
        RpcTransportOutcome::HttpError { status: 429 } => Route::RateLimited429,
        RpcTransportOutcome::HttpError { .. }
        | RpcTransportOutcome::NonJson
        | RpcTransportOutcome::Timeout
        | RpcTransportOutcome::Network
        | RpcTransportOutcome::NotConnected => Route::PlainFailure,
    }
}

/// The reads a shell may hedge (spec 083 H6). Each answers from chain state
/// and changes nothing, so asking two nodes costs one request and never a
/// second effect. Anything that writes — `eth_sendRawTransaction`, which a
/// dApp's page sends through the same pool — is absent, and so are the
/// filter methods, whose ids live on one node only.
const HEDGED_READS: &[&str] = &[
    "eth_blockNumber",
    "eth_call",
    "eth_chainId",
    "eth_createAccessList",
    "eth_estimateGas",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByHash",
    "eth_getBlockByNumber",
    "eth_getBlockReceipts",
    "eth_getBlockTransactionCountByHash",
    "eth_getBlockTransactionCountByNumber",
    "eth_getCode",
    "eth_getLogs",
    "eth_getProof",
    "eth_getStorageAt",
    "eth_getTransactionByBlockHashAndIndex",
    "eth_getTransactionByBlockNumberAndIndex",
    "eth_getTransactionByHash",
    "eth_getTransactionCount",
    "eth_getTransactionReceipt",
    "eth_maxPriorityFeePerGas",
    "eth_simulateV1",
    "eth_syncing",
    "net_version",
    "web3_clientVersion",
];

/// Whether a shell may ask the next endpoint early for this call (spec 083
/// H6): a read on the list above, through the RPC pool. Never a write, never
/// a filter, never anything bound for the bundler.
pub fn is_hedged_read(kind: RpcKind, method: &str) -> bool {
    kind == RpcKind::Rpc && HEDGED_READS.contains(&method)
}

/// The verdict an outcome may hand the caller BEFORE this machine reaches
/// that endpoint in its own order, or `None` when it must wait for it.
///
/// For a shell that asks the next endpoint while the current one is still
/// silent (desktop hedged reads, spec 083 H6). An early answer has to be one
/// every healthy node gives alike, because it beats the preferred node's
/// answer rather than following that node's failure: a result, or an
/// execution revert (code 3, or a message naming the revert) — facts about
/// chain state. Anything else this machine would still conclude with is a
/// fact about the NODE that answered: a getLogs range cap, a -32601 "method
/// not found", a missing trace or simulate method. Handed over early, a
/// weaker public node's cap or gap would replace the preferred node's real
/// answer whenever that node was merely slower than the hedge delay; such
/// an outcome waits, and is this machine's to conclude with only when it
/// reaches that endpoint — after the endpoints before it have failed.
///
/// Always a subset of what [`handle_outcome`] concludes with: an outcome the
/// machine would ban or fail over (an "execution reverted: … exceeded" that
/// reads as permanent, say) is never an answer here either.
pub fn early_verdict(
    kind: RpcKind,
    method: &str,
    url: &str,
    outcome: &RpcTransportOutcome,
) -> Option<RpcCallVerdict> {
    let node_independent = match outcome {
        RpcTransportOutcome::Response { error: None } => true,
        RpcTransportOutcome::Response { error: Some(error) } => {
            error.code == Some(3)
                || error
                    .message
                    .as_deref()
                    .is_some_and(|message| message.to_lowercase().contains("revert"))
        }
        _ => false,
    };
    match route_of(kind, method, outcome) {
        // A hedged read changes nothing on any node (`HEDGED_READS`), so no
        // POST of it delivered anything (spec 082 RA1).
        Route::Success if node_independent => Some(RpcCallVerdict::Respond {
            url: url.to_owned(),
            maybe_delivered: false,
        }),
        _ => None,
    }
}

fn handle_outcome(
    model: &mut Model,
    call_id: &str,
    url: &str,
    outcome: RpcTransportOutcome,
    latency_ms: f64,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    let (kind, method, chain_id) = {
        let Some(session) = model.calls.get(call_id) else {
            return Command::done();
        };
        match &session.state {
            SessionState::WaitingPost { url: expected } if expected == url => {}
            _ => return Command::done(),
        }
        (session.kind, session.method.clone(), session.chain_id)
    };

    // Facts about this POST that outlive the route (spec 082 RA1, RF1).
    if let Some(session) = model.calls.get_mut(call_id) {
        session.maybe_delivered |= may_have_delivered(&outcome);
        session.saw_unreached |= is_transport_failure(&outcome);
        if session.pass == 0 {
            session.first_pass_tried = session.first_pass_tried.saturating_add(1);
            session.first_pass_all_transport &= is_transport_failure(&outcome);
        }
    }

    let route = route_of(kind, &method, &outcome);
    let maybe_delivered = model
        .calls
        .get(call_id)
        .is_some_and(|session| session.maybe_delivered);

    match route {
        Route::Success => {
            touch_success(model, chain_id, kind, url, latency_ms);
            clear_chain_failure(model, chain_id, kind);
            conclude(
                model,
                call_id,
                RpcCallVerdict::Respond {
                    url: url.to_owned(),
                    maybe_delivered,
                },
            )
        }
        Route::NotServed => {
            // An answer about the method, not the endpoint: the chain is
            // reachable, the endpoint's record is untouched.
            clear_chain_failure(model, chain_id, kind);
            conclude(
                model,
                call_id,
                RpcCallVerdict::Respond {
                    url: url.to_owned(),
                    maybe_delivered,
                },
            )
        }
        Route::RangeCap(max_span) => {
            // Invariant ⑤: request-specific — the endpoint records a SUCCESS,
            // the chain is healthy, and the caller splits the range. No
            // failover (the next endpoint usually has the same cap), no ban.
            touch_success(model, chain_id, kind, url, latency_ms);
            clear_chain_failure(model, chain_id, kind);
            conclude(
                model,
                call_id,
                RpcCallVerdict::RangeCap {
                    url: url.to_owned(),
                    max_span,
                },
            )
        }
        Route::Ban { rate_limit_signal } => {
            if kind == RpcKind::Rpc && rate_limit_signal {
                if let Some(session) = model.calls.get_mut(call_id) {
                    session.saw_rate_limit = true;
                }
            }
            touch_failure(model, chain_id, kind, url, now_ms);
            ban_url(model, chain_id, kind, url, now_ms);
            let entries: Vec<RpcBanEntry> = model.bans.values().cloned().collect();
            Command::all([
                request(model, RpcOperation::PersistBans { entries }),
                render(),
                next_endpoint(model, call_id, now_ms),
            ])
        }
        Route::Transient { rate_limit_signal } => {
            if kind == RpcKind::Rpc && rate_limit_signal {
                if let Some(session) = model.calls.get_mut(call_id) {
                    session.saw_rate_limit = true;
                }
            }
            touch_failure(model, chain_id, kind, url, now_ms);
            next_endpoint(model, call_id, now_ms)
        }
        Route::RateLimited429 => {
            // 429: back off this endpoint briefly (scoring cooldown), never a
            // hard ban — and remember the transient signal (invariant ④).
            if kind == RpcKind::Rpc {
                if let Some(session) = model.calls.get_mut(call_id) {
                    session.saw_rate_limit = true;
                }
            }
            touch_failure(model, chain_id, kind, url, now_ms);
            next_endpoint(model, call_id, now_ms)
        }
        Route::PlainFailure => {
            touch_failure(model, chain_id, kind, url, now_ms);
            next_endpoint(model, call_id, now_ms)
        }
    }
}

fn conclude(
    model: &mut Model,
    call_id: &str,
    verdict: RpcCallVerdict,
) -> Command<RpcEffect, Event> {
    model.calls.remove(call_id);
    Command::all([
        request(
            model,
            RpcOperation::Conclude {
                call_id: call_id.to_owned(),
                verdict,
            },
        ),
        render(),
    ])
}

/// On any usable RPC answer the chain is healthy again — both failure sets
/// clear (`rpc-pool.ts:800-803`). Bundler calls never touch them.
fn clear_chain_failure(model: &mut Model, chain_id: u32, kind: RpcKind) {
    if kind == RpcKind::Rpc {
        model.failed_chains.remove(&chain_id);
        model.rate_limited_chains.remove(&chain_id);
        model.unreached_chains.remove(&chain_id);
    }
}

fn stats_mut<'model>(
    model: &'model mut Model,
    chain_id: u32,
    kind: RpcKind,
    url: &str,
) -> Option<&'model mut RpcEndpointStats> {
    let pool = model.pools.get_mut(&chain_id)?;
    pool.list_mut(kind)
        .iter_mut()
        .find(|endpoint| endpoint.url == url)
}

fn touch_success(model: &mut Model, chain_id: u32, kind: RpcKind, url: &str, latency_ms: f64) {
    if let Some(stats) = stats_mut(model, chain_id, kind, url) {
        record_success(stats, latency_ms);
    }
}

fn touch_failure(model: &mut Model, chain_id: u32, kind: RpcKind, url: &str, now_ms: f64) {
    if let Some(stats) = stats_mut(model, chain_id, kind, url) {
        record_failure(stats, now_ms);
    }
}

/// Temp-ban a URL, upgrading to permanent when it has never succeeded and
/// failed ≥ 6 times (`tempBan` + `maybePermaBan`; the two `saveBans` writes
/// are collapsed into the caller's single `PersistBans`).
fn ban_url(model: &mut Model, chain_id: u32, kind: RpcKind, url: &str, now_ms: f64) {
    let permanent = stats_mut(model, chain_id, kind, url)
        .map(|stats| qualifies_for_perma_ban(stats.total_calls, stats.total_failures))
        .unwrap_or(false);
    model.bans.insert(
        url.to_owned(),
        RpcBanEntry {
            url: url.to_owned(),
            banned_at_ms: now_ms,
            permanent,
        },
    );
}

// ---------------------------------------------------------------------------
// Bundler REST base (invariant ③)
// ---------------------------------------------------------------------------

/// `getActiveBundlerBaseUrl`: the REST base of the bundler the pool would
/// submit `eth_sendUserOperation` to — its highest-scored non-banned
/// endpoint, `/{chainId}` suffix stripped. `None` = fall back to the
/// built-in base (shell config). No rescue here, as in TS.
fn answer_bundler_base(
    model: &mut Model,
    call_id: &str,
    chain_id: u32,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    let base_url = {
        let bans: Vec<RpcBanEntry> = model.bans.values().cloned().collect();
        model
            .pools
            .get(&chain_id)
            .and_then(|pool| {
                select_urls(pool.list(RpcKind::Bundler), &bans, now_ms)
                    .into_iter()
                    .next()
            })
            .map(|url| strip_chain_suffix(&url, chain_id))
    };
    request(
        model,
        RpcOperation::Conclude {
            call_id: call_id.to_owned(),
            verdict: RpcCallVerdict::BundlerBase { base_url },
        },
    )
}

// ---------------------------------------------------------------------------
// Best RPC URL for the bundler's REST leg / the fork simulator (invariant ②)
// ---------------------------------------------------------------------------

/// `getChainRpcUrl`: the RPC endpoint this chain's pool would reach for first.
///
/// The first member of [`bundler_eligible_urls`] — the SAME predicate the
/// JSON-RPC leg's `X-Rpc-Url` is chosen by, deliberately not a second
/// re-statement of it. The bundler will read the Safe's code, nonce and balance
/// through this URL to decide a treasury transfer, and it reaches that decision
/// through whichever of the two legs the shell happened to call; the two must
/// not be able to disagree about which endpoints are usable. `None` when
/// nothing survives; the shell then omits the header rather than guessing
/// (fail-closed, exactly as [`handle_probe`]'s fallback does).
fn answer_best_rpc_url(
    model: &mut Model,
    call_id: &str,
    chain_id: u32,
    now_ms: f64,
) -> Command<RpcEffect, Event> {
    let url = bundler_eligible_urls(model, chain_id, now_ms)
        .into_iter()
        .next();
    request(
        model,
        RpcOperation::Conclude {
            call_id: call_id.to_owned(),
            verdict: RpcCallVerdict::BestRpcUrl { url },
        },
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Issue one operation whose answer routes back through `ShellCompleted`.
fn request(model: &mut Model, operation: RpcOperation) -> Command<RpcEffect, Event> {
    let attempt = model.attempt;
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

impl super::SplitEffect for RpcEffect {
    type Op = RpcOperation;
    fn into_shell(self) -> Option<crux_core::Request<RpcOperation>> {
        match self {
            RpcEffect::Render(_) => None,
            RpcEffect::Shell(request) => Some(request),
        }
    }
}
