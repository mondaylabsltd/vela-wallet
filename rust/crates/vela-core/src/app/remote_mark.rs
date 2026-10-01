//! How long a remote logo that failed to load stays failed (spec 082, RE10).
//!
//! Pure. A miss is classed from its HTTP status: a missing or refused image
//! is remembered for the session, a throttled, broken or unreached one for a
//! short while, so a logo lost to a bad minute comes back (W20: on the iPhone
//! one bad minute left every token as a lettermark until the app was killed).
//! A miss is still remembered at all because a token with no logo would
//! otherwise be fetched again on every scroll — one 404 per row per frame.
//!
//! The shells keep the expiry next to each miss; a transient miss (one with a
//! TTL) is also forgotten on `net_health`'s `CameBack`. The web's
//! `<img onerror>` has no status, so its misses are [`MarkMiss::Unknown`].

use serde::{Deserialize, Serialize};

/// How long a transient miss is remembered before the logo is asked for
/// again.
pub const MARK_MISS_TRANSIENT_MS: u32 = 60_000;

/// Why a logo did not arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkMiss {
    /// 404 or 410: there is no image there.
    NotFound,
    /// 401 or 403: the server will not give it to us.
    Refused,
    /// The server answered with success and what came back does not draw.
    NotAnImage,
    /// 429: asked too often — it will answer later.
    Throttled,
    /// 5xx or 408: the server is having a bad minute.
    ServerError,
    /// No answer at all: the connection failed, dropped or timed out.
    Transport,
    /// Anything else, and a miss with no status (the web's `onerror`).
    Unknown,
}

/// The class of a miss whose response carried HTTP `status`.
#[must_use]
pub fn mark_miss_of_status(status: u16) -> MarkMiss {
    match status {
        404 | 410 => MarkMiss::NotFound,
        401 | 403 => MarkMiss::Refused,
        429 => MarkMiss::Throttled,
        408 | 500..=599 => MarkMiss::ServerError,
        // The request succeeded; the bytes were the problem.
        200..=299 => MarkMiss::NotAnImage,
        _ => MarkMiss::Unknown,
    }
}

/// How long `miss` is remembered: `None` for the session (asking again will
/// not help), `Some(ms)` for a miss that may heal.
#[must_use]
pub fn mark_miss_ttl_ms(miss: MarkMiss) -> Option<u32> {
    match miss {
        MarkMiss::NotFound | MarkMiss::Refused | MarkMiss::NotAnImage => None,
        MarkMiss::Throttled | MarkMiss::ServerError | MarkMiss::Transport | MarkMiss::Unknown => {
            Some(MARK_MISS_TRANSIENT_MS)
        }
    }
}
