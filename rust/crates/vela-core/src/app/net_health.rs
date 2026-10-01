//! Whether the network is there, from the answers the app already gets (spec 082, RE3).
//!
//! Pure. Each read either reached a node or did not; a run of misses is
//! "went offline", the first answer after it is "came back", and the shells
//! retry what failed on that edge. Android's `NetHealth.kt` (spec 047 FR-006)
//! moves here so every shell counts the same way.
//!
//! Answered by what the calls did, never by what the radio claims: a
//! connectivity check says "connected" behind a captive portal and "no
//! network" on a working VPN, and a proxy node that hangs leaves the path
//! "satisfied" (the China case, where NWPathMonitor alone never fires). So a
//! shell feeds [`net_health_step`] with:
//!
//! - `reached = true` — any answer from a server, whatever its status (a
//!   throttled 429 included);
//! - `reached = false` — a call that never reached one. Which calls count is
//!   the shell's unit of work:
//!   - one HTTP request (Android's executor, spec 047): the socket could not
//!     open, or the connection was refused or dropped before an answer. That
//!     one request timing out is not fed — a slow server is not a missing
//!     network;
//!   - one pooled read (the iPhone's `RpcPool`, the desktop's
//!     `executor/pool.rs`; RE3, T113, T069): every endpoint swept and none
//!     answered, and not throttled. This
//!     one IS fed even when each endpoint only timed out: a hanging proxy
//!     node refuses nothing — it only times out — and is exactly the case
//!     that must still go offline and come back.
//!
//! On [`NetEdge::CameBack`] a shell resets its failed pages' attempt counts
//! and retries the one in front (`browser_load::retry_when_network_returns`),
//! clears transient logo misses (`remote_mark`) and forces a balance read.
//!
//! Spec 082 round 2 (RJ14, G53): the count is of sources, not calls. Each
//! miss names where it came from (`source`: the chain id of a pooled read;
//! `None` for a call with no chain, which counts as a source of its own).
//! "Went offline" needs [`MISSES_BEFORE_OFFLINE`] misses in a row from at
//! least two distinct sources **and** nothing reached for
//! [`OFFLINE_QUIET_MS`]; any answer resets the run. A chain whose nodes are
//! all failing while the others answer is that chain's notice, never
//! "offline" — the desktop's log had ten `net: offline` / `came back` lines in
//! two minutes, each "back" 0.2–0.9 s after "offline", with chains 1, 100 and
//! 480 giving up while the rest answered. A source count alone would still
//! have flapped there; the quiet window is what stops it.

use serde::{Deserialize, Serialize};

/// Consecutive calls that never reached a server before the app counts
/// itself offline. Two are a blip.
pub const MISSES_BEFORE_OFFLINE: u32 = 3;

/// Distinct sources a run of misses must come from before it can be
/// "offline" (RJ14): one failing chain is that chain's notice.
pub const SOURCES_BEFORE_OFFLINE: usize = 2;

/// How long nothing may have answered before a run of misses is "offline"
/// (RJ14), in milliseconds.
pub const OFFLINE_QUIET_MS: f64 = 10_000.0;

/// The count so far. A fresh app is online with no misses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NetHealth {
    /// Calls in a row that never reached a server.
    pub misses: u32,
    pub online: bool,
    /// The distinct chains the current run of misses came from, in the order
    /// first seen. Cleared by any answer.
    #[serde(default)]
    pub sources: Vec<u32>,
    /// Misses in the current run that named no chain — each its own source.
    #[serde(default)]
    pub unsourced: u32,
    /// When a call last reached a server (epoch ms); `None` = not yet.
    #[serde(default)]
    pub last_reach_ms: Option<f64>,
    /// When the current run of misses began (epoch ms) — the quiet window's
    /// start for an app that has not reached anything yet.
    #[serde(default)]
    pub run_started_ms: Option<f64>,
}

impl Default for NetHealth {
    fn default() -> Self {
        Self {
            misses: 0,
            online: true,
            sources: Vec::new(),
            unsourced: 0,
            last_reach_ms: None,
            run_started_ms: None,
        }
    }
}

impl NetHealth {
    /// Distinct sources of the current run of misses.
    #[must_use]
    pub fn distinct_sources(&self) -> usize {
        self.sources
            .len()
            .saturating_add(usize::try_from(self.unsourced).unwrap_or(usize::MAX))
    }
}

/// A change the shells act on. Nothing is said while the state holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetEdge {
    /// The run of misses reached [`MISSES_BEFORE_OFFLINE`], from at least
    /// [`SOURCES_BEFORE_OFFLINE`] sources, with nothing answering for
    /// [`OFFLINE_QUIET_MS`].
    WentOffline,
    /// The first call answered after going offline.
    CameBack,
}

/// One call's outcome: the next state, and the edge it crossed, if any.
///
/// `source` is the chain the call read (`None` = no chain; counted as a
/// source of its own); `now_ms` the shell's clock when the call ended.
#[must_use]
pub fn net_health_step(
    state: NetHealth,
    reached: bool,
    source: Option<u32>,
    now_ms: f64,
) -> (NetHealth, Option<NetEdge>) {
    if reached {
        let edge = (!state.online).then_some(NetEdge::CameBack);
        let next = NetHealth {
            misses: 0,
            online: true,
            sources: Vec::new(),
            unsourced: 0,
            last_reach_ms: Some(now_ms),
            run_started_ms: None,
        };
        return (next, edge);
    }
    let mut next = state;
    next.misses = next.misses.saturating_add(1);
    if next.run_started_ms.is_none() {
        next.run_started_ms = Some(now_ms);
    }
    match source {
        Some(chain) if !next.sources.contains(&chain) => next.sources.push(chain),
        Some(_) => {}
        None => next.unsourced = next.unsourced.saturating_add(1),
    }
    let quiet_since = next.last_reach_ms.or(next.run_started_ms).unwrap_or(now_ms);
    let went_offline = next.online
        && next.misses >= MISSES_BEFORE_OFFLINE
        && next.distinct_sources() >= SOURCES_BEFORE_OFFLINE
        && now_ms - quiet_since >= OFFLINE_QUIET_MS;
    if went_offline {
        next.online = false;
    }
    (next, went_offline.then_some(NetEdge::WentOffline))
}
