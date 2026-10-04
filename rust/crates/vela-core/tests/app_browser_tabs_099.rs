//! Which tabs keep a live engine (spec 099 R2) — one test per rule of
//! `browser_tabs::plan_engines`.

#![cfg(feature = "crux")]

use vela_core::app::browser_tabs::{plan_engines, EngineInput, EnginePlan, LIVE_TABS_CAP};

fn ids(n: usize) -> Vec<String> {
    (1..=n).map(|i| format!("t-{i}")).collect()
}

fn input(tabs: &[String], selected: &str, recent: &[&str], live: &[String]) -> EngineInput {
    EngineInput {
        tabs: tabs.to_vec(),
        selected: Some(selected.to_owned()),
        recent: recent.iter().map(|s| (*s).to_owned()).collect(),
        busy: Vec::new(),
        live: live.to_vec(),
        pressure: false,
    }
}

fn suspended(plan: &EnginePlan) -> Vec<&str> {
    plan.suspend.iter().map(String::as_str).collect()
}

/// Up to the cap, nothing is let go.
#[test]
fn under_the_cap_every_engine_stays() {
    let tabs = ids(LIVE_TABS_CAP);
    let plan = plan_engines(&input(&tabs, "t-1", &[], &tabs));
    assert!(plan.suspend.is_empty());
}

/// Past the cap, the least recently used go first.
#[test]
fn past_the_cap_the_least_recently_used_go() {
    let tabs = ids(8);
    let recent = ["t-8", "t-7", "t-6", "t-5", "t-4", "t-3", "t-2", "t-1"];
    let plan = plan_engines(&input(&tabs, "t-8", &recent, &tabs));
    assert_eq!(suspended(&plan), vec!["t-1", "t-2"]);
}

/// The selected tab is kept even when recency has never heard of it.
#[test]
fn the_selected_tab_is_never_suspended() {
    let tabs = ids(8);
    let recent = ["t-8", "t-7", "t-6", "t-5", "t-4", "t-3", "t-2"];
    let plan = plan_engines(&input(&tabs, "t-1", &recent, &tabs));
    assert!(!plan.suspend.iter().any(|id| id == "t-1"));
    assert_eq!(plan.suspend.len(), 8 - LIVE_TABS_CAP);
}

/// A busy tab is kept however old — suspending it would answer its page 4900
/// for nothing the person did — even past the cap.
#[test]
fn a_busy_tab_is_never_suspended() {
    let tabs = ids(9);
    let recent = [
        "t-9", "t-8", "t-7", "t-6", "t-5", "t-4", "t-3", "t-2", "t-1",
    ];
    let mut sut = input(&tabs, "t-9", &recent, &tabs);
    sut.busy = vec!["t-1".to_owned(), "t-2".to_owned()];
    let plan = plan_engines(&sut);
    assert!(!plan.suspend.iter().any(|id| id == "t-1" || id == "t-2"));
    // Kept: t-9 (selected), t-1, t-2 (busy), then t-8, t-7, t-6 by recency.
    assert_eq!(suspended(&plan), vec!["t-3", "t-4", "t-5"]);

    // More busy tabs than the cap: all of them stay.
    sut.busy = tabs.clone();
    assert!(plan_engines(&sut).suspend.is_empty());
}

/// Under memory pressure only the selected tab and busy tabs stay.
#[test]
fn pressure_keeps_only_what_must_stay() {
    let tabs = ids(4);
    let mut sut = input(&tabs, "t-2", &["t-2", "t-1", "t-3", "t-4"], &tabs);
    sut.pressure = true;
    sut.busy = vec!["t-4".to_owned()];
    assert_eq!(suspended(&plan_engines(&sut)), vec!["t-1", "t-3"]);
}

/// A tab with no engine has nothing to suspend; an engine for a tab the strip
/// no longer has is the shell's to drop with the tab, not a suspension.
#[test]
fn only_live_tabs_of_the_strip_are_named() {
    let tabs = ids(8);
    let live = vec!["t-1".to_owned(), "t-8".to_owned(), "t-gone".to_owned()];
    let plan = plan_engines(&input(&tabs, "t-8", &["t-8"], &live));
    assert!(plan.suspend.is_empty());
}

/// Tabs recency does not know come after the ones it does, in strip order.
#[test]
fn unknown_recency_comes_last_in_strip_order() {
    let tabs = ids(8);
    let plan = plan_engines(&input(&tabs, "t-8", &["t-8", "t-1"], &tabs));
    // Kept: t-8, t-1, then t-2..t-5 in strip order.
    assert_eq!(suspended(&plan), vec!["t-6", "t-7"]);
}
