//! Network health (spec 082 T034, RE3): Android's `NetHealthTest.kt`, moved
//! with the rule — three calls that never reached a server are offline, one
//! answered call is back, and only the two changes are ever said.

#![cfg(feature = "crux")]

use vela_core::app::net_health::{net_health_step, NetEdge, NetHealth, MISSES_BEFORE_OFFLINE};

/// Feeds `outcomes` from a fresh app; the final state and every edge said.
fn run(outcomes: &[bool]) -> (NetHealth, Vec<Option<NetEdge>>) {
    let mut state = NetHealth::default();
    let mut edges = Vec::new();
    for &reached in outcomes {
        let (next, edge) = net_health_step(state, reached);
        state = next;
        edges.push(edge);
    }
    (state, edges)
}

#[test]
fn a_fresh_app_is_online() {
    assert_eq!(
        NetHealth::default(),
        NetHealth {
            misses: 0,
            online: true
        }
    );
    assert_eq!(MISSES_BEFORE_OFFLINE, 3);
}

/// `two misses are a blip, the third is offline, one answer is back`.
#[test]
fn two_misses_are_a_blip_the_third_is_offline_one_answer_is_back() {
    let (state, edges) = run(&[false, false]);
    assert!(state.online, "two misses are a blip");
    assert_eq!(edges, vec![None, None]);

    let (state, edges) = run(&[false, false, false]);
    assert!(!state.online);
    assert_eq!(edges, vec![None, None, Some(NetEdge::WentOffline)]);

    let (state, edges) = run(&[false, false, false, true]);
    assert_eq!(
        state,
        NetHealth {
            misses: 0,
            online: true
        }
    );
    assert_eq!(edges.last(), Some(&Some(NetEdge::CameBack)));
}

/// `an answer in between resets the count`.
#[test]
fn an_answer_in_between_resets_the_count() {
    let (state, edges) = run(&[false, false, true, false, false]);
    assert!(state.online);
    assert_eq!(state.misses, 2);
    assert!(edges.iter().all(Option::is_none));
}

/// Only the two changes are said: nothing while online, nothing for more
/// misses once offline, and "back" once.
#[test]
fn no_edge_while_the_state_holds() {
    let (_, edges) = run(&[true, true, true]);
    assert!(edges.iter().all(Option::is_none), "no edge while online");

    let (state, edges) = run(&[false, false, false, false, false, true, true]);
    assert_eq!(
        edges,
        vec![
            None,
            None,
            Some(NetEdge::WentOffline),
            None,
            None,
            Some(NetEdge::CameBack),
            None,
        ]
    );
    assert!(state.online && state.misses == 0);
}

/// A count that has run for a very long offline stretch does not wrap.
#[test]
fn the_count_saturates() {
    let (state, edge) = net_health_step(
        NetHealth {
            misses: u32::MAX,
            online: false,
        },
        false,
    );
    assert_eq!(state.misses, u32::MAX);
    assert!(!state.online && edge.is_none());
}

/// The shells hand the state across as JSON; the edge is a snake_case word.
#[test]
fn the_state_and_edges_travel_as_json() {
    let json = serde_json::to_string(&NetHealth::default()).unwrap_or_default();
    assert_eq!(json, r#"{"misses":0,"online":true}"#);
    assert_eq!(
        serde_json::to_string(&NetEdge::WentOffline).unwrap_or_default(),
        r#""went_offline""#
    );
    assert_eq!(
        serde_json::to_string(&NetEdge::CameBack).unwrap_or_default(),
        r#""came_back""#
    );
}
