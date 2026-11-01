//! The incoming-funds scan, as one rule for every shell (r9-17 / r8-13).
//!
//! A receipt — "Received +0.6" in Activity — showed on the web and nowhere
//! else for three rounds, on a multisig most of all. Each shell chose the
//! networks to scan for itself (`activity.ts`, iOS `scanChains`, desktop
//! `scan_chains`, Android `IncomingScan.kt`), each ran its own cadence, and
//! every one of them looked back only [`LIVE_SCAN_BLOCKS`] blocks from the
//! head, so a payment older than that when the scan first ran was never
//! seen. Here the core decides all three; a shell gathers facts and executes:
//!
//! - **Networks** ([`scan_plan`]): where the account holds something, ∪ where
//!   it lives (a multisig's networks — its record's, where it is deployed —
//!   passed in as facts), and, when neither says anything yet, the space's
//!   defaults by the rule `activity.ts` had (the main payment chains when the
//!   space has any of them, else the networks the person added and the
//!   space's first six). The native-balance watch's networks come with it.
//! - **Windows** ([`scan_windows`]): the live window at the head, as before,
//!   plus a bounded backfill of what came BEFORE — down to
//!   [`backfill_floor`], never below a known deploy block, in chunks a
//!   capped endpoint serves, a few a poll, newest first, and never a span
//!   already covered (the coverage is kept per account and network by the
//!   shell, [`TrustScanCursor`], so a restart resumes and nothing is scanned
//!   twice). `token_trust` runs these windows on its `ScanRequested`.
//! - **Cadence** ([`next_scan_ms`]): when the next scan is due, from the facts
//!   of the page (shown or not, touched lately or not) — the owner's round-5
//!   numbers, which lived in the web's `incoming-cadence.ts`.
//!
//! The shared vectors are `tests/vectors/incoming-scan.json`; every shell
//! replays them through its door.
//!
//! Pure: no I/O, no clock.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// The live window at the head, in blocks — `token_trust`'s
/// `LIVE_SCAN_BLOCKS` (`transfer-monitor.ts:54`). The scan reads
/// `[head - LIVE_SCAN_BLOCKS, head]` on every poll, covered or not: an
/// endpoint's newest logs can lag its head, and a repeat costs one de-duped
/// read.
pub const LIVE_SCAN_BLOCKS: u64 = 100;

/// Main payment chains a wallet holding nothing yet is watched on
/// (`activity.ts:393`; `token_trust::DEFAULT_MONITOR_CHAINS`).
pub const DEFAULT_MONITOR_CHAINS: [u32; 6] = [1, 56, 137, 42_161, 8_453, 100];

/// How many of the space's networks stand in for a wallet holding nothing in
/// a space without the main payment chains (`activity.ts`
/// `NATIVE_WATCH_FALLBACK`, home-5 / r4D-18).
pub const SPACE_FALLBACK_CHAINS: usize = 6;

/// How far back the backfill reaches: one week before the scan.
///
/// Rationale (best practice, r9-17): a wallet's history is the explorer's
/// job; what the scan owes a person is the payments that reached an account
/// around the time it came to this device — a multisig funded before its
/// members added it, a payment made while the app was closed. A week covers
/// "I sent it to you the other day" and a long weekend away; going further
/// would cost every account on every network a deep log sweep that public
/// endpoints throttle, for receipts the explorer link already shows.
pub const BACKFILL_LOOKBACK_SECS: u64 = 7 * 24 * 60 * 60;

/// The most blocks one account's backfill reads on one network, whatever the
/// block time — the cost bound. 400 000 blocks is a week of a 1.5 s chain;
/// a faster chain (Arbitrum's 0.25 s, BSC's 0.75 s) gets less than a week
/// rather than a sweep of millions of blocks. At [`BACKFILL_CHUNK_BLOCKS`]
/// that is 40 reads, [`BACKFILL_CHUNKS_PER_POLL`] a poll.
pub const BACKFILL_MAX_BLOCKS: u64 = 400_000;

/// One backfill read's span. 10 000 blocks is the span the common public
/// endpoints serve (`eth_getLogs` range caps sit at 10 000 or above on most;
/// an endpoint that caps lower says so, and the scan shrinks its chunk to
/// that cap — `token_trust`'s `RangeCapped`).
pub const BACKFILL_CHUNK_BLOCKS: u64 = 10_000;

/// Backfill reads per network per poll — each is two `eth_getLogs` (the
/// transfers, the wallet's own `SafeReceived`). Four keeps a poll's extra
/// cost to eight reads a network while a week of a 12 s chain (50 400
/// blocks) is done in two polls.
pub const BACKFILL_CHUNKS_PER_POLL: usize = 4;

/// Covered spans kept per account and network. Coverage grows at its edges
/// and gaps close as the backfill runs, so a handful is plenty; past it the
/// oldest span goes (it is scanned again — a cost, never a loss).
pub const MAX_COVERED_SPANS: usize = 16;

/// Where a shell keeps an account's [`TrustScanCursor`] list: this prefix +
/// the account's lower-case address, the list as JSON. Under `vela.scan…`,
/// so the storage page files it with the scan caches
/// (`storage_catalog`): clearing it costs a re-read of the week, never a
/// receipt.
pub const CURSOR_KEY_PREFIX: &str = "vela.scanCursors.";

/// The block time assumed for a network whose time is not in the table:
/// 2 s, the common L2 cadence (Base, OP, Polygon). Wrong in either direction
/// only moves how much of the week [`BACKFILL_MAX_BLOCKS`] lets in.
pub const DEFAULT_BLOCK_MS: u64 = 2_000;

/// While the person is at the page (round 5, `incoming-cadence.ts`).
pub const CADENCE_ACTIVE_MS: u32 = 10_000;
/// Shown, nobody at it.
pub const CADENCE_IDLE_MS: u32 = 30_000;
/// Hidden: a background page still hears of money within a minute.
pub const CADENCE_HIDDEN_MS: u32 = 60_000;
/// An interaction this recent counts as "at the page".
pub const CADENCE_ACTIVE_WINDOW_MS: f64 = 120_000.0;

// ---------------------------------------------------------------------------
// Networks
// ---------------------------------------------------------------------------

/// A network an account lives on — a multisig's (its record's networks,
/// where it is deployed). A Vela account lives on every network and passes
/// none.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustAccountNetwork {
    pub chain_id: u32,
    /// The block its contract was deployed in, when known: the backfill
    /// never reads below it (nothing can have reached it before).
    #[serde(default)]
    pub deploy_block: Option<f64>,
}

/// What a shell knows when it scans an account — facts, no choice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustScanFacts {
    /// Networks the account holds a balance on (the balance read's chains).
    #[serde(default)]
    pub held_chain_ids: Vec<u32>,
    /// Networks the account lives on: a multisig's. Empty for a Vela account.
    #[serde(default)]
    pub account_networks: Vec<TrustAccountNetwork>,
    /// The networks of the space in front, in the list's order
    /// (`network_space::space_chain_ids`). Empty: no space is known, and
    /// every network counts as in it.
    #[serde(default)]
    pub space_chain_ids: Vec<u32>,
    /// The networks the person added (custom networks), any space.
    #[serde(default)]
    pub added_chain_ids: Vec<u32>,
}

/// The networks to scan, and where to watch the native balance.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct IncomingScanPlan {
    /// The receipt scan's networks (`token_trust`'s scan set), in order.
    pub scan_chain_ids: Vec<u32>,
    /// home-5: the networks whose native balance is read beside the scan — a
    /// coin paid to an address with no code leaves no log to find. A shell
    /// without that watch ignores it.
    pub native_watch_chain_ids: Vec<u32>,
}

fn dedupe(ids: impl IntoIterator<Item = u32>) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    ids.into_iter()
        .filter(|id| *id > 0 && seen.insert(*id))
        .collect()
}

/// The networks an account's incoming scan reads (module doc). For a Vela
/// account with balances this is exactly what `activity.ts` scanned; a
/// multisig adds the networks it lives on, so its first receipt on a network
/// where it holds nothing yet is seen.
#[must_use]
pub fn scan_plan(facts: &TrustScanFacts) -> IncomingScanPlan {
    let space = &facts.space_chain_ids;
    let in_space = |id: &u32| space.is_empty() || space.contains(id);
    let added: Vec<u32> = dedupe(facts.added_chain_ids.iter().copied().filter(in_space));
    let space_fallback = dedupe(
        added
            .iter()
            .copied()
            .chain(space.iter().copied().take(SPACE_FALLBACK_CHAINS)),
    );
    let lives_on = facts.account_networks.iter().map(|n| n.chain_id);
    let known = dedupe(facts.held_chain_ids.iter().copied().chain(lives_on));

    enum From {
        Known,
        Defaults,
        Space,
    }
    let (scan, from) = if !known.is_empty() {
        (known, From::Known)
    } else if DEFAULT_MONITOR_CHAINS.iter().any(in_space) {
        (DEFAULT_MONITOR_CHAINS.to_vec(), From::Defaults)
    } else {
        (space_fallback.clone(), From::Space)
    };

    let native = match from {
        // r4a-17: a wallet holding nothing in a space without the main
        // payment chains may get its first coin on any of its networks.
        From::Space => dedupe(space.iter().copied()),
        From::Known | From::Defaults => dedupe(
            scan.iter()
                .copied()
                .filter(in_space)
                .chain(added.iter().copied()),
        ),
    };
    let native_watch_chain_ids = if native.is_empty() {
        space_fallback
    } else {
        native
    };
    IncomingScanPlan {
        scan_chain_ids: scan,
        native_watch_chain_ids,
    }
}

/// [`scan_plan`] over JSON — the door for the shells (`TrustScanFacts` in,
/// `IncomingScanPlan` out). Unreadable facts are no facts.
#[must_use]
pub fn scan_plan_json(facts_json: &str) -> String {
    let facts: TrustScanFacts = serde_json::from_str(facts_json).unwrap_or_default();
    serde_json::to_string(&scan_plan(&facts)).unwrap_or_else(|_| "{}".to_owned())
}

// ---------------------------------------------------------------------------
// Windows
// ---------------------------------------------------------------------------

/// A span of blocks, both ends included.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustBlockSpan {
    pub from_block: f64,
    pub to_block: f64,
}

impl TrustBlockSpan {
    #[must_use]
    pub fn of(from: u64, to: u64) -> Self {
        Self {
            from_block: from as f64,
            to_block: to as f64,
        }
    }

    /// As whole blocks; `None` for a span that is not one (negative, not
    /// whole, beyond 2^53, or upside down).
    #[must_use]
    pub fn blocks(&self) -> Option<(u64, u64)> {
        let whole = |v: f64| {
            (v.is_finite() && v >= 0.0 && v.fract() == 0.0 && v <= 9_007_199_254_740_991.0)
                .then_some(v as u64)
        };
        let (from, to) = (whole(self.from_block)?, whole(self.to_block)?);
        (from <= to).then_some((from, to))
    }
}

/// What one account's scan has read on one network: the spans covered, so
/// a later poll — after a restart too — reads only what is still open. The
/// shell keeps it per account (`token_trust`'s `TrustView::cursors` after a
/// poll) and hands it back with the next `ScanRequested`; two copies merge
/// (a span read is read, whoever read it).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustScanCursor {
    pub chain_id: u32,
    /// Sorted, merged, oldest first; at most [`MAX_COVERED_SPANS`].
    #[serde(default)]
    pub covered: Vec<TrustBlockSpan>,
}

/// A network's block time in ms, as far as the core knows it: Ethereum and
/// its 12 s-slot testnets, Gnosis' 5 s, BSC's 0.75 s, Arbitrum's 0.25 s; the
/// rest [`DEFAULT_BLOCK_MS`].
#[must_use]
pub fn block_ms(chain_id: u32) -> u64 {
    match chain_id {
        1 | 11_155_111 | 17_000 | 560_048 => 12_000,
        100 | 10_200 => 5_000,
        56 | 97 => 750,
        42_161 | 421_614 => 250,
        _ => DEFAULT_BLOCK_MS,
    }
}

/// How many blocks back from the head the backfill reaches on `chain_id`:
/// [`BACKFILL_LOOKBACK_SECS`] of its blocks, at most [`BACKFILL_MAX_BLOCKS`].
#[must_use]
pub fn backfill_lookback_blocks(chain_id: u32) -> u64 {
    (BACKFILL_LOOKBACK_SECS * 1_000 / block_ms(chain_id).max(1)).min(BACKFILL_MAX_BLOCKS)
}

/// The lowest block the scan reads on `chain_id` with the head at `latest`:
/// a week of blocks back (bounded), and never below the account's deploy
/// block when it is known.
#[must_use]
pub fn backfill_floor(chain_id: u32, latest: u64, deploy_block: Option<u64>) -> u64 {
    latest
        .saturating_sub(backfill_lookback_blocks(chain_id))
        .max(deploy_block.unwrap_or(0))
}

/// One poll's reads on one network.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct IncomingScanWindows {
    /// The live window at the head — read on every poll.
    pub live: TrustBlockSpan,
    /// This poll's backfill reads, newest first, each at most the chunk,
    /// none overlapping what is covered or the live window.
    pub backfill: Vec<TrustBlockSpan>,
    /// [`backfill_floor`] for this head.
    pub floor: f64,
}

/// Merge `span` into `covered`, keeping it sorted, overlap- and
/// adjacency-merged, and at most [`MAX_COVERED_SPANS`] long (the oldest go).
#[must_use]
pub fn merge_covered(covered: &[(u64, u64)], span: (u64, u64)) -> Vec<(u64, u64)> {
    let mut all: Vec<(u64, u64)> = covered.to_vec();
    all.push(span);
    all.retain(|(from, to)| from <= to);
    all.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(all.len());
    for (from, to) in all {
        match out.last_mut() {
            Some(last) if from <= last.1.saturating_add(1) => last.1 = last.1.max(to),
            _ => out.push((from, to)),
        }
    }
    if out.len() > MAX_COVERED_SPANS {
        out.drain(..out.len() - MAX_COVERED_SPANS);
    }
    out
}

/// One poll's windows on `chain_id` (module doc): the live window
/// `[latest - LIVE_SCAN_BLOCKS, latest]`, then up to `budget` backfill reads
/// of at most `chunk` blocks over what `covered` (the spans already read)
/// and the live window leave open between [`backfill_floor`] and the head —
/// newest first, so a payment made while the app was closed lands before
/// last week's.
#[must_use]
pub fn scan_windows(
    chain_id: u32,
    latest: u64,
    deploy_block: Option<u64>,
    covered: &[(u64, u64)],
    chunk: u64,
    budget: usize,
) -> IncomingScanWindows {
    let chunk = chunk.max(1);
    let live_from = latest.saturating_sub(LIVE_SCAN_BLOCKS);
    let floor = backfill_floor(chain_id, latest, deploy_block);
    let taken = merge_covered(covered, (live_from, latest));
    // The open gaps between the floor and the head, newest first. `top` is
    // the highest block not yet ruled out (`None`: nothing is left below).
    let mut gaps: Vec<(u64, u64)> = Vec::new();
    let mut top = Some(latest);
    for &(from, to) in taken.iter().rev() {
        let Some(t) = top.filter(|t| *t >= floor) else {
            break;
        };
        if to < t {
            let low = (to + 1).max(floor);
            if low <= t {
                gaps.push((low, t));
            }
        }
        top = from.checked_sub(1).map(|below| below.min(t));
    }
    if let Some(t) = top.filter(|t| *t >= floor) {
        gaps.push((floor, t));
    }
    let mut backfill = Vec::new();
    'gaps: for (low, high) in gaps {
        let mut top = high;
        loop {
            if backfill.len() >= budget {
                break 'gaps;
            }
            let from = top.saturating_sub(chunk - 1).max(low);
            backfill.push(TrustBlockSpan::of(from, top));
            if from <= low {
                break;
            }
            top = from - 1;
        }
    }
    IncomingScanWindows {
        live: TrustBlockSpan::of(live_from, latest),
        backfill,
        floor: floor as f64,
    }
}

/// The question [`scan_windows_json`] answers — the vectors' shape.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct IncomingScanWindowsAsk {
    pub chain_id: u32,
    pub latest: f64,
    #[serde(default)]
    pub deploy_block: Option<f64>,
    #[serde(default)]
    pub covered: Vec<TrustBlockSpan>,
    /// The read span; absent: [`BACKFILL_CHUNK_BLOCKS`].
    #[serde(default)]
    pub chunk: Option<f64>,
    /// Reads this poll; absent: [`BACKFILL_CHUNKS_PER_POLL`].
    #[serde(default)]
    pub budget: Option<u32>,
}

/// [`scan_windows`] over JSON (`IncomingScanWindowsAsk` in,
/// `IncomingScanWindows` out) — the door the shells replay the vectors
/// through. An unreadable ask answers an empty plan at block 0.
#[must_use]
pub fn scan_windows_json(ask_json: &str) -> String {
    let ask: IncomingScanWindowsAsk = serde_json::from_str(ask_json).unwrap_or_default();
    let whole = |v: f64| {
        TrustBlockSpan {
            from_block: 0.0,
            to_block: v,
        }
        .blocks()
        .map(|(_, v)| v)
    };
    let latest = whole(ask.latest).unwrap_or(0);
    let deploy = ask.deploy_block.and_then(whole);
    let covered: Vec<(u64, u64)> = ask
        .covered
        .iter()
        .filter_map(TrustBlockSpan::blocks)
        .collect();
    let chunk = ask
        .chunk
        .and_then(whole)
        .filter(|c| *c > 0)
        .unwrap_or(BACKFILL_CHUNK_BLOCKS);
    let budget = ask.budget.map_or(BACKFILL_CHUNKS_PER_POLL, |b| b as usize);
    let windows = scan_windows(ask.chain_id, latest, deploy, &covered, chunk, budget);
    serde_json::to_string(&windows).unwrap_or_else(|_| "{}".to_owned())
}

// ---------------------------------------------------------------------------
// Cadence
// ---------------------------------------------------------------------------

/// The wait before the next scan, for the page as it stands (round 5): 10 s
/// while someone is at it (touched in the last two minutes), 30 s shown and
/// idle, 60 s hidden.
#[must_use]
pub fn next_scan_ms(visible: bool, since_interaction_ms: f64) -> u32 {
    if !visible {
        return CADENCE_HIDDEN_MS;
    }
    // A figure that is no figure (NaN, negative) is "not lately".
    if (0.0..CADENCE_ACTIVE_WINDOW_MS).contains(&since_interaction_ms) {
        CADENCE_ACTIVE_MS
    } else {
        CADENCE_IDLE_MS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_span_reads_back_as_whole_blocks_or_not_at_all() {
        assert_eq!(TrustBlockSpan::of(3, 9).blocks(), Some((3, 9)));
        let upside_down = TrustBlockSpan {
            from_block: 9.0,
            to_block: 3.0,
        };
        assert_eq!(upside_down.blocks(), None);
        let half = TrustBlockSpan {
            from_block: 0.5,
            to_block: 3.0,
        };
        assert_eq!(half.blocks(), None);
    }

    #[test]
    fn the_kept_coverage_is_a_scan_cache() {
        let key = format!("{CURSOR_KEY_PREFIX}0xabc");
        let item = crate::storage_catalog::item_of_key(&key).map(|item| item.id);
        assert_eq!(item, Some("scan"));
        assert!(crate::storage_catalog::is_cache_key(&key));
    }

    #[test]
    fn coverage_merges_overlaps_and_neighbours() {
        let merged = merge_covered(&[(10, 20), (30, 40)], (21, 29));
        assert_eq!(merged, vec![(10, 40)]);
        let apart = merge_covered(&[(10, 20)], (25, 30));
        assert_eq!(apart, vec![(10, 20), (25, 30)]);
    }

    #[test]
    fn coverage_keeps_the_newest_spans() {
        let many: Vec<(u64, u64)> = (0..MAX_COVERED_SPANS as u64)
            .map(|i| (i * 10, i * 10 + 1))
            .collect();
        let merged = merge_covered(&many, (1_000, 1_001));
        assert_eq!(merged.len(), MAX_COVERED_SPANS);
        assert_eq!(merged.first(), Some(&(10, 11)));
        assert_eq!(merged.last(), Some(&(1_000, 1_001)));
    }
}
