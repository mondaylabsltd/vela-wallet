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
//! - `reached = true` — any answer from a server, whatever its status;
//! - `reached = false` — a call that never reached one (the socket could not
//!   open, the connection was refused or dropped before an answer).
//!
//! A timeout is neither and is not fed: a slow server is not a missing
//! network.
//!
//! On [`NetEdge::CameBack`] a shell resets its failed pages' attempt counts
//! and retries the one in front (`browser_load::retry_when_network_returns`),
//! clears transient logo misses (`remote_mark`) and forces a balance read.

use serde::{Deserialize, Serialize};

/// Consecutive calls that never reached a server before the app counts
/// itself offline. Two are a blip.
pub const MISSES_BEFORE_OFFLINE: u32 = 3;

/// The count so far. A fresh app is online with no misses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetHealth {
    /// Calls in a row that never reached a server.
    pub misses: u32,
    pub online: bool,
}

impl Default for NetHealth {
    fn default() -> Self {
        Self {
            misses: 0,
            online: true,
        }
    }
}

/// A change the shells act on. Nothing is said while the state holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetEdge {
    /// The [`MISSES_BEFORE_OFFLINE`]th miss in a row.
    WentOffline,
    /// The first call answered after going offline.
    CameBack,
}

/// One call's outcome: the next state, and the edge it crossed, if any.
#[must_use]
pub fn net_health_step(state: NetHealth, reached: bool) -> (NetHealth, Option<NetEdge>) {
    if reached {
        let edge = (!state.online).then_some(NetEdge::CameBack);
        let next = NetHealth {
            misses: 0,
            online: true,
        };
        return (next, edge);
    }
    let misses = state.misses.saturating_add(1);
    let went_offline = state.online && misses >= MISSES_BEFORE_OFFLINE;
    let next = NetHealth {
        misses,
        online: state.online && !went_offline,
    };
    (next, went_offline.then_some(NetEdge::WentOffline))
}
