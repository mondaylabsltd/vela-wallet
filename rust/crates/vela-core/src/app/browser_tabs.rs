//! Pure policy — the browser's tabs: which keep a live engine (spec 099 R2),
//! what Explore shows when somebody enters it, and which tab an opened site
//! goes into. Every client asks these functions, so the four agree.
//!
//! ## Live engines
//!
//! A live tab is a webview with its page running: switching to it shows the
//! page as it was left, inputs, scroll, sockets and all. A suspended tab is
//! only its URL and title: selecting it loads the page again. Every client
//! keeps one engine per tab (the desktop since 099, the phones since 044/053),
//! and every client asks [`plan_engines`] which ones to let go, so "which tab
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
//!
//! ## Where Explore lands
//!
//! Leaving a dApp for the wallet and coming back put the person straight back
//! inside the dApp on both phones, and the desktop re-showed whatever page it
//! had last: each shell decided for itself, and they had drifted apart.
//! [`explore_landing`] decides it once.
//!
//! ```text
//! entry ─ section / reselect ─────────────► Home
//!       ─ page_opened ─ selected has a page ► Tab{selected}   (else Home)
//! waiting ─ a tab with a page ─────────────► Tab{waiting}     (any entry)
//! ```
//!
//! - Explore chosen from another section, or chosen again while it is up →
//!   its home. Nothing is closed: the dApp's tab stays — live, unlit in the
//!   strip, first in [`ExploreView::resumable`] — and one tap resumes it with
//!   no reload.
//! - Explore brought up by a page opened from outside — a deep link, a scan,
//!   the external-page sheet, a launch URL → that page's tab.
//! - A tab with a request waiting on the person ([`waiting_tab`]) → that tab,
//!   whatever brought Explore up: a request is never shown beside a home page
//!   it did not come from (the desktop's RJ18 signing column comes back WITH
//!   its tab).
//!
//! Only ENTERING Explore asks. A pick inside it — a resume row, a tab in the
//! switcher, a site opened from the home — shows its own tab.
//!
//! ## Where an opened site goes
//!
//! [`open_target`], the desktop's spec 082 RD6 rule moved here and extended
//! so that opening a site from Explore's home never replaces a live dApp:
//!
//! 1. On a page (the address bar of the page on screen) → that tab loads it.
//! 2. Over the home, a PICKED site with a tab already on its origin → that
//!    tab, resumed as it was left.
//! 3. Over the home, the selected tab when it is a start-page tab → it gets
//!    its first page.
//! 4. Otherwise a NEW tab, so a tab waiting unlit — a dApp left for the
//!    wallet, a tab restored at launch — is left intact.
//! 5. Unless the strip is full ([`TABS_CAP`]): the explore machine refuses a
//!    tab past the cap, so a new tab would be an open that silently does
//!    nothing. An open tab takes the address instead, and never the dApp
//!    the person just left while another will do: a start-page tab (the
//!    one used most recently first) — nothing in it is lost; else the tab
//!    used longest ago that is not the selected one; the selected tab only
//!    when no other is there. The shell selects the tab it is given.
//!
//! A site PICKED (a favourite, a recent dApp, a featured tile) and an address
//! TYPED differ in rule 2 only. A site is an origin — the explore machine and
//! the history both key it so — so a tab already on that origin IS the site,
//! and picking it brings that tab back rather than a second copy of it. An
//! address names a page: switching to a same-origin tab would drop the path
//! somebody typed, and loading it there would replace the live dApp, so it
//! takes rule 3 or 4. A page handed in from outside (deep link, scan, launch
//! URL, the external-page sheet) is an address, and is never "on a page".

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::dapp_browser::DbrView;
use super::dapp_permissions::origin_of;
use super::explore_sites::{ExploreTab, ExploreView, TABS_CAP};

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

// ---------------------------------------------------------------------------
// Where Explore lands
// ---------------------------------------------------------------------------

/// What brought Explore up — [`explore_landing`]'s question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreEntry {
    /// Explore chosen from another section (the tab bar, the sidebar) — the
    /// first time after a launch included.
    Section,
    /// Explore chosen again while it is already up: a re-tap while browsing
    /// is the way back to its home.
    Reselect,
    /// Explore came up because a page was opened from outside: a deep link,
    /// a scan, the external-page sheet, a launch URL. Asked once that open's
    /// `tab_opened` / `tab_navigated` is in the view, so the selected tab is
    /// the page.
    PageOpened,
}

/// What Explore shows on entry ([`explore_landing`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreLanding {
    /// The start page: search, the resume rows, favourites, recents. Every
    /// tab stays as it was — this closes nothing and loads nothing.
    Home,
    /// This tab's page. Always a tab of the strip that has a page.
    Tab { id: String },
}

/// The landing rule (module doc, "Where Explore lands"). `waiting` is the tab
/// whose request waits on the person ([`waiting_tab`], or the desktop's kept
/// signing column); a tab the strip does not carry, or one with no page, is
/// no reason to leave the home.
#[must_use]
pub fn explore_landing(
    view: &ExploreView,
    entry: ExploreEntry,
    waiting: Option<&str>,
) -> ExploreLanding {
    let has_page = |id: &&str| {
        view.tabs
            .iter()
            .any(|tab| tab.id == *id && tab.url.is_some())
    };
    let tab = |id: &str| ExploreLanding::Tab { id: id.to_owned() };
    if let Some(waiting) = waiting.filter(has_page) {
        return tab(waiting);
    }
    match entry {
        ExploreEntry::Section | ExploreEntry::Reselect => ExploreLanding::Home,
        ExploreEntry::PageOpened => view
            .selected_tab
            .as_deref()
            .filter(has_page)
            .map_or(ExploreLanding::Home, tab),
    }
}

/// The tab whose request is in front of the person — the consent, the
/// signature, or the add-network sheet the browser machine holds, in that
/// order. `None` while nothing waits.
#[must_use]
pub fn waiting_tab(dapp: &DbrView) -> Option<String> {
    first_waiting(
        dapp.consent.as_ref().map(|consent| consent.tab.as_str()),
        dapp.signing.as_ref().map(|signing| signing.tab.as_str()),
        dapp.adding_network
            .as_ref()
            .map(|adding| adding.tab.as_str()),
    )
}

/// The order [`waiting_tab`] reads the three sheets in.
fn first_waiting(
    consent: Option<&str>,
    signing: Option<&str>,
    adding_network: Option<&str>,
) -> Option<String> {
    consent.or(signing).or(adding_network).map(str::to_owned)
}

// ---------------------------------------------------------------------------
// Where an opened site goes
// ---------------------------------------------------------------------------

/// How an open was asked for — the one difference between the two is
/// [`open_target`]'s rule 2 (module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreOpenKind {
    /// An address: typed into a bar, or handed in from outside (deep link,
    /// scan, launch URL, the external-page sheet). It names a page.
    Address,
    /// A site picked from the home: a favourite, a recent dApp, a featured
    /// tile. It names a site — an origin.
    Site,
}

/// Where an open goes ([`open_target`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreOpenTarget {
    /// Load the address in this tab: the page on screen, a start-page tab's
    /// first page, or the tab a full strip gives up. The shell sends
    /// `tab_selected` when it is not the selected tab, then `tab_navigated`.
    Load { id: String },
    /// Show this tab as it was left — it is already on the site. A live
    /// engine comes to the front with no load; a suspended one loads its
    /// own address, as any tab switch does. The shell sends `tab_selected`.
    Resume { id: String },
    /// A new tab onto the address (`tab_opened`): nothing open is replaced.
    /// Never answered for a full strip ([`TABS_CAP`]) — the machine would
    /// drop that `tab_opened` — which gets [`Self::Load`] of the tab it can
    /// spare instead (module doc, rule 5).
    NewTab,
}

/// The tab the strip lights (spec 082 RD6): the one whose page is on screen
/// while a page shows (`on_page`, `shown`); over the home, the selected tab
/// only when it IS a start-page tab. A tab waiting unlit — restored at
/// launch, or left for the wallet — is not "this tab" under a home page.
#[must_use]
pub fn lit_tab(view: &ExploreView, shown: Option<&str>, on_page: bool) -> Option<String> {
    if on_page {
        return shown.map(str::to_owned);
    }
    let selected = view.selected_tab.as_deref()?;
    view.tabs
        .iter()
        .find(|tab| tab.id == selected && tab.url.is_none())
        .map(|tab| tab.id.clone())
}

/// The open rule (module doc, "Where an opened site goes"): `url` asked for
/// as `kind`, with `shown` the tab whose page is in the view and `on_page`
/// whether that page is on screen (the address bar of a page) rather than
/// the home.
#[must_use]
pub fn open_target(
    view: &ExploreView,
    shown: Option<&str>,
    on_page: bool,
    url: &str,
    kind: ExploreOpenKind,
) -> ExploreOpenTarget {
    if on_page {
        if let Some(shown) = shown {
            return ExploreOpenTarget::Load {
                id: shown.to_owned(),
            };
        }
    }
    if kind == ExploreOpenKind::Site {
        if let Some(id) = same_site(view, url) {
            return ExploreOpenTarget::Resume { id };
        }
    }
    if let Some(id) = lit_tab(view, None, false) {
        return ExploreOpenTarget::Load { id };
    }
    if view.tabs.len() >= TABS_CAP {
        if let Some(id) = spare_tab(view) {
            return ExploreOpenTarget::Load { id };
        }
    }
    ExploreOpenTarget::NewTab
}

/// The tab a full strip gives an open (rule 5). Never the one in front — the
/// dApp the person just left — while any other will do: a start-page tab,
/// the one used most recently first, has nothing in it to lose; else the tab
/// used longest ago (the last in the resume rows' order, [`by_recency`]);
/// the one in front only when it is the only tab there is.
///
/// [`by_recency`]: super::explore_sites::by_recency
fn spare_tab(view: &ExploreView) -> Option<String> {
    let front = in_front(view);
    let by_use: Vec<&ExploreTab> =
        super::explore_sites::by_recency(&view.tabs, &view.recent_tabs).collect();
    let start_page = by_use.iter().find(|tab| tab.url.is_none());
    let longest_ago = || {
        by_use
            .iter()
            .rev()
            .find(|tab| Some(tab.id.as_str()) != front.as_deref())
    };
    start_page
        .or_else(longest_ago)
        .map(|tab| tab.id.clone())
        .or(front)
}

/// The tab in front: the selected one, else — a view whose selection names
/// no tab, which the machine never publishes but a shell's re-encoded copy
/// might — the one used most recently.
fn in_front(view: &ExploreView) -> Option<String> {
    let selected = view
        .selected_tab
        .as_deref()
        .filter(|id| view.tabs.iter().any(|tab| tab.id == *id));
    selected.map(str::to_owned).or_else(|| {
        super::explore_sites::by_recency(&view.tabs, &view.recent_tabs)
            .next()
            .map(|tab| tab.id.clone())
    })
}

/// The tab already on `url`'s origin, most recently used first, then in
/// strip order. Origins as the connection rule reads them
/// ([`origin_of`]): the site a tab is connected as is the site it is.
fn same_site(view: &ExploreView, url: &str) -> Option<String> {
    let origin = origin_of(url)?;
    let on_origin = |tab: &&ExploreTab| {
        tab.url
            .as_deref()
            .and_then(origin_of)
            .is_some_and(|its| its == origin)
    };
    super::explore_sites::by_recency(&view.tabs, &view.recent_tabs)
        .find(on_origin)
        .map(|tab| tab.id.clone())
}

// ---------------------------------------------------------------------------
// Over JSON (UniFFI)
// ---------------------------------------------------------------------------

/// What the JSON wrappers read of an `ExploreView`: the strip, its selection
/// and its recency, each defaulting when absent — a shell that re-encodes
/// its own copy of the view (fields it never decoded, a renamed one gone) is
/// still read, rather than answered with nothing.
#[derive(Deserialize)]
struct StripJson {
    #[serde(default)]
    tabs: Vec<ExploreTab>,
    #[serde(default)]
    selected_tab: Option<String>,
    #[serde(default)]
    recent_tabs: Vec<String>,
}

fn strip_of(view_json: &str) -> Option<ExploreView> {
    let strip: StripJson = serde_json::from_str(view_json).ok()?;
    Some(ExploreView {
        tabs: strip.tabs,
        selected_tab: strip.selected_tab,
        recent_tabs: strip.recent_tabs,
        ..ExploreView::default()
    })
}

/// A unit enum from its bare wire word (`"section"`, `"site"`), the way a
/// shell already holds it.
fn word<T: serde::de::DeserializeOwned>(word: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(word.to_owned())).ok()
}

/// [`explore_landing`] over JSON: an `ExploreView` JSON, the entry's word
/// (`"section"`, `"reselect"`, `"page_opened"`) and the waiting tab in, an
/// [`ExploreLanding`] JSON out (`{"type":"home"}`, `{"type":"tab","id":…}`).
/// `None` for input that does not read.
#[must_use]
pub fn explore_landing_json(view_json: &str, entry: &str, waiting: Option<&str>) -> Option<String> {
    let view = strip_of(view_json)?;
    let entry: ExploreEntry = word(entry)?;
    serde_json::to_string(&explore_landing(&view, entry, waiting)).ok()
}

/// [`open_target`] over JSON: an `ExploreView` JSON, the shown tab, whether
/// its page is on screen, the address and the kind's word (`"address"`,
/// `"site"`) in, an [`ExploreOpenTarget`] JSON out (`{"type":"load","id":…}`,
/// `{"type":"resume","id":…}`, `{"type":"new_tab"}`). `None` for input that
/// does not read.
#[must_use]
pub fn open_target_json(
    view_json: &str,
    shown: Option<&str>,
    on_page: bool,
    url: &str,
    kind: &str,
) -> Option<String> {
    let view = strip_of(view_json)?;
    let kind: ExploreOpenKind = word(kind)?;
    serde_json::to_string(&open_target(&view, shown, on_page, url, kind)).ok()
}

/// [`lit_tab`] over JSON: an `ExploreView` JSON, the shown tab and whether
/// its page is on screen in, the lit tab's id out. `None` when nothing is lit
/// or the view does not read.
#[must_use]
pub fn lit_tab_json(view_json: &str, shown: Option<&str>, on_page: bool) -> Option<String> {
    lit_tab(&strip_of(view_json)?, shown, on_page)
}

/// What [`waiting_tab`] reads of a `DbrView`: the three sheets' tabs.
#[derive(Deserialize)]
struct SheetsJson {
    #[serde(default)]
    consent: Option<TabOnly>,
    #[serde(default)]
    signing: Option<TabOnly>,
    #[serde(default)]
    adding_network: Option<TabOnly>,
}

#[derive(Deserialize)]
struct TabOnly {
    tab: String,
}

impl TabOnly {
    fn of(sheet: Option<&Self>) -> Option<&str> {
        sheet.map(|sheet| sheet.tab.as_str())
    }
}

/// [`waiting_tab`] over JSON: a `DbrView` JSON in, the waiting tab's id out.
/// `None` while nothing waits, or for a view that does not read.
#[must_use]
pub fn waiting_tab_json(dapp_view_json: &str) -> Option<String> {
    let sheets: SheetsJson = serde_json::from_str(dapp_view_json).ok()?;
    first_waiting(
        TabOnly::of(sheets.consent.as_ref()),
        TabOnly::of(sheets.signing.as_ref()),
        TabOnly::of(sheets.adding_network.as_ref()),
    )
}
