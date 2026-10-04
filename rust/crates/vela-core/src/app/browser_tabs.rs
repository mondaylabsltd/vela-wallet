//! Pure policy — which browser tabs keep a live engine (spec 099 R2).
//!
//! A live tab is a webview with its page running: switching to it shows the
//! page as it was left, inputs, scroll, sockets and all. A suspended tab is
//! only its URL and title: selecting it loads the page again. Every client
//! keeps one engine per tab (the desktop since 099, the phones since 044/053),
//! and every client asks this function which ones to let go, so "which tab
//! reloaded, and why" is the same answer everywhere.
//!
//! ```text
//! explore_sites ─ tabs, selected, recent ─┐
//! dapp_browser  ─ busy tabs ──────────────┼─► plan_engines ─► suspend [ids]
//! shell         ─ live engines, pressure ─┘
//! ```
//!
//! The rule, in order:
//!
//! 1. The selected tab is kept. It is what the person is looking at.
//! 2. A busy tab is kept — a request open, a read in flight, a consent or a
//!    signature from it. Suspending it would answer its page 4900 for nothing
//!    the person did.
//! 3. Other live tabs are kept most-recently-used first while fewer than the
//!    cap are kept: [`LIVE_TABS_CAP`], or one under memory pressure.
//! 4. Every other live tab is suspended. A tab with no engine has nothing to
//!    suspend, and nothing here ever wakes one: the shell wakes the selected
//!    tab, and only that.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// Most engines kept alive at once, the selected and busy tabs included —
/// unless more than this are busy, which are all kept regardless.
pub const LIVE_TABS_CAP: usize = 6;

/// What [`plan_engines`] decides from. Ids are explore tab ids.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct EngineInput {
    /// Every open tab, in strip order (`ExploreView.tabs`).
    pub tabs: Vec<String>,
    /// `ExploreView.selected_tab`.
    pub selected: Option<String>,
    /// Most recently used first (`ExploreView.recent_tabs`).
    #[serde(default)]
    pub recent: Vec<String>,
    /// Tabs `dapp_browser` reports busy (`DbrTabView.busy`).
    #[serde(default)]
    pub busy: Vec<String>,
    /// Tabs whose engine exists now — the shell's own fact.
    #[serde(default)]
    pub live: Vec<String>,
    /// The system asked the app to free memory.
    #[serde(default)]
    pub pressure: bool,
}

/// The engines to let go of now.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct EnginePlan {
    /// Live tabs to suspend: drop the engine, keep the URL and title.
    pub suspend: Vec<String>,
}

/// The live-engine rule (module doc).
#[must_use]
pub fn plan_engines(input: &EngineInput) -> EnginePlan {
    let cap = if input.pressure { 1 } else { LIVE_TABS_CAP };
    let is_live = |id: &String| input.live.contains(id) && input.tabs.contains(id);

    let mut kept: Vec<&String> = Vec::new();
    if let Some(selected) = input.selected.as_ref().filter(|id| is_live(id)) {
        kept.push(selected);
    }
    for id in input.tabs.iter().filter(|id| is_live(id)) {
        if input.busy.contains(id) && !kept.contains(&id) {
            kept.push(id);
        }
    }

    // Most recent first; a live tab recency does not know (a shell that lost
    // track, a tab restored at launch) after those, in strip order.
    let by_recency = input.recent.iter().filter(|id| is_live(id)).chain(
        input
            .tabs
            .iter()
            .filter(|id| is_live(id) && !input.recent.contains(id)),
    );
    for id in by_recency {
        if kept.len() >= cap {
            break;
        }
        if !kept.contains(&id) {
            kept.push(id);
        }
    }

    EnginePlan {
        suspend: input
            .tabs
            .iter()
            .filter(|id| is_live(id) && !kept.contains(id))
            .cloned()
            .collect(),
    }
}

/// [`plan_engines`] over JSON, for the shells that hold their views as JSON
/// (UniFFI, wasm): an [`EngineInput`] in, an [`EnginePlan`] out. `None` for
/// input that is not one.
#[must_use]
pub fn plan_engines_json(input_json: &str) -> Option<String> {
    let input: EngineInput = serde_json::from_str(input_json).ok()?;
    serde_json::to_string(&plan_engines(&input)).ok()
}
