//! Network health (spec 082 T034, RE3; round 2 T192, RJ14): Android's
//! `NetHealthTest.kt`, moved with the rule — and, since G53, counted by
//! source: misses from one chain are that chain's notice, never "offline".
//! Offline needs three misses in a row from at least two sources and nothing
//! answering for ten seconds; one answer is back; only the two changes are
//! ever said.

#![cfg(feature = "crux")]

use vela_core::app::net_health::{
    net_health_step, NetEdge, NetHealth, MISSES_BEFORE_OFFLINE, OFFLINE_QUIET_MS,
};

const T0: f64 = 1_759_000_000_000.0;
const GNOSIS: u32 = 100;
const ETHEREUM: u32 = 1;
const BASE: u32 = 8453;

/// One call: reached or not, from which chain (`None` = no chain), at which
/// second after `T0`.
type Call = (bool, Option<u32>, f64);

/// Feeds `calls` from a fresh app; the final state and every edge said.
fn run(calls: &[Call]) -> (NetHealth, Vec<Option<NetEdge>>) {
    run_from(NetHealth::default(), calls)
}

fn run_from(mut state: NetHealth, calls: &[Call]) -> (NetHealth, Vec<Option<NetEdge>>) {
    let mut edges = Vec::new();
    for &(reached, source, at_s) in calls {
        let (next, edge) = net_health_step(state, reached, source, T0 + at_s * 1_000.0);
        state = next;
        edges.push(edge);
    }
    (state, edges)
}

fn miss(source: u32, at_s: f64) -> Call {
    (false, Some(source), at_s)
}

fn reach(source: u32, at_s: f64) -> Call {
    (true, Some(source), at_s)
}

#[test]
fn a_fresh_app_is_online() {
    let fresh = NetHealth::default();
    assert_eq!(fresh.misses, 0);
    assert!(fresh.online);
    assert!(fresh.sources.is_empty());
    assert_eq!(fresh.last_reach_ms, None);
    assert_eq!(MISSES_BEFORE_OFFLINE, 3);
    assert_eq!(OFFLINE_QUIET_MS, 10_000.0);
}

/// G53, the T181 log: one faulted chain (Gnosis) given up on ten times in a
/// row is that chain's notice. The app is not offline.
#[test]
fn ten_misses_from_one_chain_are_never_offline() {
    let calls: Vec<Call> = (0..10).map(|i| miss(GNOSIS, f64::from(i) * 5.0)).collect();
    let (state, edges) = run(&calls);
    assert!(state.online, "one source is a chain notice");
    assert!(edges.iter().all(Option::is_none), "{edges:?}");
    assert_eq!(state.misses, 10);
    assert_eq!(state.sources, vec![GNOSIS]);
}

/// The T181 shape: chains 100 and 1 giving up while Base answers between
/// them. Every answer resets the run, so nothing flaps.
#[test]
fn misses_interleaved_with_answers_from_another_chain_never_flap() {
    let mut calls = Vec::new();
    for i in 0..12 {
        let at = f64::from(i) * 4.0;
        calls.push(miss(GNOSIS, at));
        calls.push(miss(ETHEREUM, at + 1.0));
        calls.push(reach(BASE, at + 2.0));
    }
    let (state, edges) = run(&calls);
    assert!(state.online);
    assert!(edges.iter().all(Option::is_none), "{edges:?}");
}

/// Two chains and a quiet network are "offline" only once nothing has
/// answered for ten seconds: a burst right after an answer is not enough.
#[test]
fn misses_from_two_chains_go_offline_only_after_ten_quiet_seconds() {
    let (state, edges) = run(&[
        reach(BASE, 0.0),
        miss(GNOSIS, 1.0),
        miss(ETHEREUM, 2.0),
        miss(GNOSIS, 3.0),
    ]);
    assert!(state.online, "three misses 3 s after an answer are a blip");
    assert!(edges.iter().all(Option::is_none));

    let (state, edges) = run_from(state, &[miss(ETHEREUM, 10.5)]);
    assert!(!state.online);
    assert_eq!(edges, vec![Some(NetEdge::WentOffline)]);
}

/// Three misses over two chains with nothing answering for 10 s: offline,
/// once; more misses say nothing; one answer is back, once.
#[test]
fn offline_then_back_once_each() {
    let (state, edges) = run(&[
        reach(BASE, 0.0),
        miss(GNOSIS, 4.0),
        miss(ETHEREUM, 8.0),
        miss(GNOSIS, 12.0),
        miss(ETHEREUM, 16.0),
        (true, Some(GNOSIS), 20.0),
        reach(BASE, 21.0),
    ]);
    assert_eq!(
        edges,
        vec![
            None,
            None,
            None,
            Some(NetEdge::WentOffline),
            None,
            Some(NetEdge::CameBack),
            None,
        ]
    );
    assert!(state.online);
    assert_eq!(state.misses, 0);
    assert!(state.sources.is_empty(), "an answer ends the run");
    assert_eq!(state.last_reach_ms, Some(T0 + 21_000.0));
}

/// A miss with no chain (a probe, a relay call) counts as its own source.
#[test]
fn a_miss_with_no_source_counts_as_its_own() {
    let (state, edges) = run(&[
        (false, None, 0.0),
        (false, None, 5.0),
        (false, None, 11.0),
    ]);
    assert!(!state.online, "three unattributed misses are three sources");
    assert_eq!(edges.last(), Some(&Some(NetEdge::WentOffline)));

    let (state, _) = run(&[
        miss(GNOSIS, 0.0),
        miss(GNOSIS, 5.0),
        (false, None, 11.0),
    ]);
    assert!(!state.online, "a chain and an unattributed miss are two");
}

/// A fresh app that never reached anything counts its quiet from the first
/// miss of the run.
#[test]
fn a_fresh_app_counts_its_quiet_from_the_first_miss() {
    let (state, _) = run(&[miss(GNOSIS, 0.0), miss(ETHEREUM, 1.0), miss(GNOSIS, 2.0)]);
    assert!(state.online, "2 s of misses on a fresh app is not offline");
    let (state, edges) = run_from(state, &[miss(ETHEREUM, 10.0)]);
    assert!(!state.online);
    assert_eq!(edges, vec![Some(NetEdge::WentOffline)]);
}

/// A count that has run for a very long offline stretch does not wrap.
#[test]
fn the_count_saturates() {
    let long = NetHealth {
        misses: u32::MAX,
        online: false,
        ..NetHealth::default()
    };
    let (state, edge) = net_health_step(long, false, Some(GNOSIS), T0);
    assert_eq!(state.misses, u32::MAX);
    assert!(!state.online && edge.is_none());
}

/// The shells hand the state across as JSON; the edge is a snake_case word,
/// and a state saved before round 2 still reads.
#[test]
fn the_state_and_edges_travel_as_json() {
    let json = serde_json::to_value(NetHealth::default()).unwrap_or_default();
    assert_eq!(
        json,
        serde_json::json!({
            "misses": 0,
            "online": true,
            "sources": [],
            "unsourced": 0,
            "last_reach_ms": null,
            "run_started_ms": null,
        })
    );
    let old: NetHealth =
        serde_json::from_str(r#"{"misses":2,"online":true}"#).unwrap_or_default();
    assert_eq!(old.misses, 2);
    assert!(old.sources.is_empty());
    assert_eq!(
        serde_json::to_string(&NetEdge::WentOffline).unwrap_or_default(),
        r#""went_offline""#
    );
    assert_eq!(
        serde_json::to_string(&NetEdge::CameBack).unwrap_or_default(),
        r#""came_back""#
    );
}
