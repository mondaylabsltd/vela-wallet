//! Machine — the browser's own memory: favourites and open tabs.
//!
//! ```text
//! Start ─► ReadExplore ─► Loaded{doc} ─► ready mirror
//!   FavoriteAdded ─► dedupe by ORIGIN ─┬─ known ─► refresh title/url in place
//!                                      └─ new ─► append (cap) ─► WriteExplore
//!   PageLoaded ─► a favourite still named by its host ─► the site's title
//!   SystemGroupHiddenSet/Tab* ─► pure edit ─► WriteExplore
//! ```
//!
//! ## A favourite's name
//!
//! The site's last good title, or its host — never an engine's error page
//! (issue #329: Android pinned app.uniswap.org as "网页无法打开"). Pinning
//! takes the core's [`super::browser_load::pinned_title`]; a favourite with
//! no title yet takes the title of the site's next good load
//! ([`Event::PageLoaded`]); and a document from before that rule has every
//! name nobody chose replaced by its host on hydration ([`NAME_RULE`],
//! issue #425 — #329's fix named new favourites, and left the stored error
//! page on the tile).
//!
//! ## Why this machine exists
//!
//! Spec 022 drew a start page with a favourites grid, custom groups and a tab
//! strip, and every client rendered them from fixtures — the web says so in
//! its own loader ("the vocabulary still ships for the gallery"), and the
//! desktop said the same thing in `explore/fixtures.rs`. Nothing anywhere
//! owned the RULES: where a favourite lives, who owns a tab. Four shells
//! drawing the same picture from four sets of mock data is not a feature;
//! this is the machine that makes it one, and the founder asked for it on
//! 2026-09-08.
//!
//! ## No custom groups (issue #465)
//!
//! The start page has two sections, Favorites and Recent dApps, each of
//! which can be hidden and neither deleted ([`ExploreSystemGroup`]). The
//! custom groups spec 022 drew are gone: the phones could only make empty
//! ones, and every site a group listed was already a favourite (a group was
//! a view over the favourites, never a container). A document written before
//! this rule may still carry a `groups` list — serde ignores it on read, and
//! the next write drops it; every favourite is kept.
//!
//! ## What it deliberately does not hold
//!
//! No page content, no cookies, no scroll position, no per-site settings —
//! url, title and host, exactly what [`super::browser_history`] keeps, for the
//! same reason: this document is a convenience, and a convenience that leaks
//! browsing detail is not one.
//!
//! ## One document, one key
//!
//! `vela.explore` holds the favourites, the tabs and which sections are
//! hidden. They are edited TOGETHER — a site is favourited from a tab — and
//! separate keys can half-write one without the other. The write is whole
//! and best-effort, like the history's; the mirror stays authoritative.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// The favourites grid is a grid a person reads at a glance. Past this many a
/// tile is not a shortcut any more, and the cap is the honest way to say so —
/// the same argument the history's own cap makes.
pub const FAVORITES_CAP: usize = 24;

/// The rule a document's favourite names were made under
/// ([`ExploreDoc::name_rule`]). `1`: pinned under
/// [`super::browser_load::pinned_title`] — the site's last good title or its
/// host — and titled only by a good load ([`Event::PageLoaded`]).
pub const NAME_RULE: u32 = 1;

/// Open tabs. A browser that lets a page open tabs without bound is a browser
/// a page can wedge; this shell's tabs are opened by a PERSON, and two dozen
/// is well past what anyone arranges on purpose.
pub const TABS_CAP: usize = 24;

/// Resume rows on the home ([`ExploreView::resumable`]). A glance, not a
/// second tab switcher: past three the switcher, one tap away, is the list.
pub const RESUME_SHOWN: usize = 3;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One pinned site.
///
/// `origin` is the identity (a favourite is a SITE, not a page); `url` is
/// where it opens, which may be deeper than the origin because that is where
/// the person actually works.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreSite {
    pub origin: String,
    pub url: String,
    pub host: String,
    /// What the tile says: the site's last good title, else its host, until
    /// somebody renames it — and a rename is kept forever after, because a
    /// person who named a tile meant it and a page can change its `<title>`
    /// at will. Never an engine's error page (see the module's "A
    /// favourite's name").
    pub name: String,
    /// `true` once a person renamed it: a later visit must not overwrite it.
    pub renamed: bool,
    pub added_ms: f64,
}

/// One open tab.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreTab {
    pub id: String,
    /// `None` is the start page — the tab every window has when it has no
    /// site open, drawn with the wallet's own mark rather than a favicon.
    pub url: Option<String>,
    pub title: String,
    pub host: String,
}

/// What a batch close takes (spec 099): [`tabs_closed_by`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TabCloseScope {
    /// Every tab but this one ("close other tabs").
    Others { keep: String },
    /// Every tab to the right of this one in the strip.
    Right { of: String },
    /// Every tab.
    All,
}

/// The two sections the start page always has. They can be hidden, never
/// deleted — the only groups there are (issue 465).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreSystemGroup {
    Favorites,
    Recent,
}

/// The stored document, whole.
///
/// A document from before issue 465 may also carry `groups` (custom groups,
/// each a list of favourite origins). It is not read — no
/// `deny_unknown_fields` — and the next write leaves it out; the favourites
/// those groups listed are all still here.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreDoc {
    #[serde(default)]
    pub favorites: Vec<ExploreSite>,
    #[serde(default)]
    pub tabs: Vec<ExploreTab>,
    /// The id of the selected tab. An id that no tab carries reads as "the
    /// first one", so a truncated or hand-edited document cannot leave the
    /// strip with nothing selected.
    #[serde(default)]
    pub selected_tab: Option<String>,
    /// Which system groups are hidden.
    #[serde(default)]
    pub hidden_system: Vec<ExploreSystemGroup>,
    /// The rule the favourites' names were made under ([`NAME_RULE`]).
    ///
    /// Absent (`0`) in every document written before issue 425, and those
    /// names nobody can vouch for: Android v0.9.5 named a favourite after
    /// whatever title its WebView held — the engine's own error page
    /// ("网页无法打开") when the site had failed to load (issue 329) — and
    /// iOS and the desktop after the page before's. Issue 329's fix named NEW
    /// favourites by the rule and left the stored ones as they were, so the
    /// Xiaomi's Uniswap tile still read "网页无法打开" in v0.9.6 while the
    /// site loaded fine. Hydration replaces every such name nobody chose
    /// with its host; the site's next good load titles it.
    #[serde(default)]
    pub name_rule: u32,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// The KV vocabulary on `vela.explore`. The shell owns the key and the
/// storage handle; the core decides what the document must become.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExploreOperation"))]
pub enum ExploreOperation {
    /// KV get. An unreadable or absent document answers `Loaded { doc: None }`
    /// — an empty start page, never a failure a person has to dismiss.
    ReadExplore,
    /// KV set — best effort, like the history's write. The mirror stays
    /// authoritative, so a swallowed storage error costs the next launch's
    /// memory and nothing in this one.
    WriteExplore { doc: ExploreDoc },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExploreShellResult"))]
pub enum ExploreShellResult {
    Loaded {
        doc: Option<ExploreDoc>,
    },
    /// A best-effort write acknowledged. Never changes state.
    Written,
}

impl Operation for ExploreOperation {
    type Output = ExploreShellResult;
}

#[effect]
pub enum ExploreEffect {
    Render(RenderOperation),
    Shell(ExploreOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExploreEvent"))]
pub enum Event {
    /// The browser surface came up — hydrate the mirror once.
    Start,
    /// Pin a site. `url` is where it opens; `now_ms` is the shell's clock
    /// (this core owns none).
    FavoriteAdded {
        url: String,
        title: Option<String>,
        now_ms: f64,
    },
    /// Unpin.
    FavoriteRemoved {
        origin: String,
    },
    /// A page loaded without failing — the core's own visit
    /// ([`super::browser_load::visit_to_record`]: never an engine's error
    /// page, never an HTTP error). A favourite of that site still named by
    /// its host — no title was known when it was pinned, or its stored name
    /// could not be trusted ([`ExploreDoc::name_rule`]) — takes this title.
    /// A favourite with a title, or a name a person chose, keeps it: a tile
    /// does not change its name with every page of the site.
    PageLoaded {
        url: String,
        title: Option<String>,
    },
    /// A person names a tile. Empty or blank is a no-op rather than a way to
    /// erase a name into a title the site controls.
    FavoriteRenamed {
        origin: String,
        name: String,
    },
    /// Hide or show one of the two sections. They cannot be deleted.
    SystemGroupHiddenSet {
        group: ExploreSystemGroup,
        hidden: bool,
    },
    /// A new tab, selected on open (that is what opening one means).
    TabOpened {
        url: Option<String>,
        title: Option<String>,
        now_ms: f64,
    },
    /// The tab moved to a page. Its title follows unless the page gave none.
    TabNavigated {
        id: String,
        url: String,
        title: Option<String>,
    },
    TabSelected {
        id: String,
    },
    /// Close one. A closed selected tab hands the selection to a neighbour;
    /// closing the last tab leaves the strip empty and nothing selected,
    /// which is the start page — see [`close_tab`].
    TabClosed {
        id: String,
    },
    /// Close several at once (spec 099, the browser's "close other tabs" /
    /// "close tabs to the right" / "close all tabs") — the ids
    /// [`tabs_closed_by`] names for a scope. One write; the selection follows
    /// [`close_tabs`].
    TabsClosed {
        ids: Vec<String>,
    },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: ExploreShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Fresh,
    Hydrating,
    Ready,
}

#[derive(Default)]
pub struct Model {
    doc: ExploreDoc,
    phase: Phase,
    attempt: u64,
    /// Tab ids, most recently used first (spec 099 R2) — what
    /// [`super::browser_tabs::plan_engines`] keeps alive. Not stored: at
    /// launch only the selected tab has a page, so it starts as that one.
    recent: Vec<String>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreView {
    pub favorites: Vec<ExploreSite>,
    pub tabs: Vec<ExploreTab>,
    /// Always a tab that exists, whenever there is one at all.
    pub selected_tab: Option<String>,
    pub favorites_hidden: bool,
    pub recent_hidden: bool,
    /// The grid is full: the shell draws no "add" affordance rather than one
    /// that refuses. A control that cannot work is worse than an absent one.
    pub favorites_full: bool,
    /// The strip is full, for the same reason.
    pub tabs_full: bool,
    /// Every tab id, most recently used first (spec 099 R2): the order
    /// [`super::browser_tabs::plan_engines`] keeps engines alive in.
    #[serde(default)]
    pub recent_tabs: Vec<String>,
    /// The home's resume rows (spec 099 navigation): the tabs that have a
    /// page, most recently used first ([`Self::recent_tabs`]), then the ones
    /// recency does not know in strip order, at most [`RESUME_SHOWN`]. A
    /// start-page tab is never one — there is nothing in it to go back to.
    ///
    /// One tap on a row is `tab_selected` and that tab's page as it was left
    /// (a live engine, no load). The section draws only while this has a
    /// row; its header counts every open tab (`tabs`, what the switcher
    /// holds — the plural `explore.openTabs_*`, `{{count}}` = `tabs.len()`),
    /// and its action opens the switcher.
    #[serde(default)]
    pub resumable: Vec<ExploreTab>,
    /// The mirror is live. Before this, a screen shows nothing rather than an
    /// empty start page it would have to correct a frame later.
    pub ready: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ExploreSites;

impl App for ExploreSites {
    type Event = Event;
    type Model = Model;
    type ViewModel = ExploreView;
    type Effect = ExploreEffect;

    #[allow(clippy::too_many_lines, reason = "one arm per event, each small")]
    fn update(&self, event: Event, model: &mut Model) -> Command<ExploreEffect, Event> {
        // Every mutation needs the store it is editing. A mutation before
        // hydration is DROPPED rather than applied to an empty document that
        // is about to be replaced — the fail-closed choice `browser_history`
        // made for the same reason.
        if !matches!(event, Event::Start | Event::ShellCompleted { .. })
            && model.phase != Phase::Ready
        {
            return Command::done();
        }

        match event {
            Event::Start => {
                if model.phase != Phase::Fresh {
                    return Command::done();
                }
                model.attempt += 1;
                model.phase = Phase::Hydrating;
                request(model, ExploreOperation::ReadExplore)
            }

            Event::FavoriteAdded { url, title, now_ms } => {
                let Some((origin, host)) = parse_web_origin(&url) else {
                    return Command::done();
                };
                if let Some(known) = model.doc.favorites.iter_mut().find(|s| s.origin == origin) {
                    // Already pinned: refresh where it opens, and its name
                    // only if nobody has named it. No reorder — the grid is
                    // the person's arrangement, not a recency list.
                    known.url = url;
                    if !known.renamed {
                        if let Some(title) = non_blank(title) {
                            known.name = title;
                        }
                    }
                    return persist(model);
                }
                if model.doc.favorites.len() >= FAVORITES_CAP {
                    return Command::done();
                }
                let name = non_blank(title).unwrap_or_else(|| host.clone());
                model.doc.favorites.push(ExploreSite {
                    origin,
                    url,
                    host,
                    name,
                    renamed: false,
                    added_ms: now_ms,
                });
                persist(model)
            }

            Event::PageLoaded { url, title } => {
                let (Some((origin, _)), Some(title)) = (parse_web_origin(&url), non_blank(title))
                else {
                    return Command::done();
                };
                let Some(site) = model
                    .doc
                    .favorites
                    .iter_mut()
                    // The same site however the page spelled its host.
                    .find(|site| site.origin.eq_ignore_ascii_case(&origin) && untitled(site))
                else {
                    return Command::done();
                };
                site.name = title;
                persist(model)
            }

            Event::FavoriteRemoved { origin } => {
                model.doc.favorites.retain(|s| s.origin != origin);
                persist(model)
            }

            Event::FavoriteRenamed { origin, name } => {
                let Some(name) = non_blank(Some(name)) else {
                    return Command::done();
                };
                let Some(site) = model.doc.favorites.iter_mut().find(|s| s.origin == origin) else {
                    return Command::done();
                };
                site.name = name;
                site.renamed = true;
                persist(model)
            }

            Event::SystemGroupHiddenSet { group, hidden } => {
                model.doc.hidden_system.retain(|g| *g != group);
                if hidden {
                    model.doc.hidden_system.push(group);
                }
                persist(model)
            }

            Event::TabOpened { url, title, now_ms } => {
                if model.doc.tabs.len() >= TABS_CAP {
                    return Command::done();
                }
                let (host, url) = match url.as_deref().and_then(parse_web_origin) {
                    Some((_, host)) => (host, url),
                    // Not a web address: the start page, whatever was passed.
                    None => (String::new(), None),
                };
                let id = unique_tab_id(&model.doc.tabs, now_ms);
                let title = non_blank(title).unwrap_or_else(|| host.clone());
                model.doc.tabs.push(ExploreTab {
                    id: id.clone(),
                    url,
                    title,
                    host,
                });
                // Opening a tab selects it. That is what opening one is.
                model.doc.selected_tab = Some(id.clone());
                used(model, id);
                persist(model)
            }

            Event::TabNavigated { id, url, title } => {
                let Some((_, host)) = parse_web_origin(&url) else {
                    return Command::done();
                };
                let Some(tab) = model.doc.tabs.iter_mut().find(|t| t.id == id) else {
                    return Command::done();
                };
                // A page with no title of its own is its host, never the
                // title of the page this tab used to be on.
                tab.title = non_blank(title).unwrap_or_else(|| host.clone());
                tab.url = Some(url);
                tab.host = host;
                persist(model)
            }

            Event::TabSelected { id } => {
                if !model.doc.tabs.iter().any(|t| t.id == id) {
                    return Command::done();
                }
                model.doc.selected_tab = Some(id.clone());
                used(model, id);
                persist(model)
            }

            Event::TabClosed { id } => close_tab(model, &id),

            Event::TabsClosed { ids } => close_tabs(model, &ids),

            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> ExploreView {
        let doc = &model.doc;
        ExploreView {
            favorites: doc.favorites.clone(),
            tabs: doc.tabs.clone(),
            // Never an id nothing carries: a strip with tabs always has one
            // of them selected.
            selected_tab: selected_or_first(doc),
            favorites_hidden: doc.hidden_system.contains(&ExploreSystemGroup::Favorites),
            recent_hidden: doc.hidden_system.contains(&ExploreSystemGroup::Recent),
            favorites_full: doc.favorites.len() >= FAVORITES_CAP,
            tabs_full: doc.tabs.len() >= TABS_CAP,
            recent_tabs: model
                .recent
                .iter()
                .filter(|id| doc.tabs.iter().any(|tab| &tab.id == *id))
                .cloned()
                .collect(),
            resumable: resumable(doc, &model.recent),
            ready: model.phase == Phase::Ready,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: ExploreShellResult) -> Command<ExploreEffect, Event> {
    match (model.phase, result) {
        (Phase::Hydrating, ExploreShellResult::Loaded { doc }) => {
            // A document from before issue #465 may carry custom groups; they
            // were never read into the model, and the next write leaves them
            // out. Every favourite they listed is in `favorites` regardless.
            model.doc = doc.unwrap_or_default();
            model.recent = selected_or_first(&model.doc).into_iter().collect();
            model.phase = Phase::Ready;
            // The repaired names are written back once, so the next launch
            // reads a document made under the rule.
            if repair_names(&mut model.doc) {
                persist(model)
            } else {
                render()
            }
        }
        _ => Command::done(),
    }
}

/// [`ExploreDoc::name_rule`]: a document from before the rule has every name
/// nobody chose replaced by its host — it may be an engine's error page, and
/// nothing stored can tell which. A rename is the person's word and stays.
/// `true` when a name changed, i.e. the document must be written back.
fn repair_names(doc: &mut ExploreDoc) -> bool {
    if doc.name_rule >= NAME_RULE {
        return false;
    }
    doc.name_rule = NAME_RULE;
    let mut changed = false;
    for site in &mut doc.favorites {
        if !site.renamed && !site.host.is_empty() && site.name != site.host {
            site.name.clone_from(&site.host);
            changed = true;
        }
    }
    changed
}

/// A favourite with no title of its own yet: named by its host, and not by a
/// person (who may have chosen the host on purpose).
fn untitled(site: &ExploreSite) -> bool {
    !site.renamed && site.name == site.host
}

// ---------------------------------------------------------------------------
// Pure policy
// ---------------------------------------------------------------------------

/// Close a tab, keeping the two things a strip must never lose.
///
/// A closed SELECTED tab hands selection to its right-hand neighbour, or to
/// the left when it was the last one — the convention every browser follows,
/// and the one a person's hand already expects. Closing the final tab leaves
/// the strip empty and nothing selected: the shell draws its start page then,
/// which is what a browser with no tabs is.
fn close_tab(model: &mut Model, id: &str) -> Command<ExploreEffect, Event> {
    let Some(index) = model.doc.tabs.iter().position(|t| t.id == id) else {
        return Command::done();
    };
    let was_selected = selected_or_first(&model.doc).as_deref() == Some(id);
    model.doc.tabs.remove(index);
    model.recent.retain(|recent| recent != id);
    if was_selected {
        model.doc.selected_tab = model
            .doc
            .tabs
            .get(index)
            .or_else(|| {
                index
                    .checked_sub(1)
                    .and_then(|left| model.doc.tabs.get(left))
            })
            .map(|tab| tab.id.clone());
        // The tab that takes over is the one in use now.
        if let Some(next) = model.doc.selected_tab.clone() {
            used(model, next);
        }
    }
    persist(model)
}

/// [`ExploreView::resumable`]: the tabs with a page, most recently used
/// first, then strip order, capped at [`RESUME_SHOWN`].
fn resumable(doc: &ExploreDoc, recent: &[String]) -> Vec<ExploreTab> {
    by_recency(&doc.tabs, recent)
        .filter(|tab| tab.url.is_some())
        .take(RESUME_SHOWN)
        .cloned()
        .collect()
}

/// The strip's tabs most recently used first, then the ones `recent` does
/// not know — restored at launch, never selected since — in strip order. The
/// order the resume rows are drawn in, and a picked site's tab is found in
/// ([`super::browser_tabs::open_target`]).
pub(crate) fn by_recency<'a>(
    tabs: &'a [ExploreTab],
    recent: &'a [String],
) -> impl Iterator<Item = &'a ExploreTab> + 'a {
    let known = recent
        .iter()
        .filter_map(|id| tabs.iter().find(|tab| &tab.id == id));
    let rest = tabs.iter().filter(|tab| !recent.contains(&tab.id));
    known.chain(rest)
}

/// `id` is the tab in use now: first in the recency order.
fn used(model: &mut Model, id: String) {
    model.recent.retain(|recent| recent != &id);
    model.recent.insert(0, id);
}

/// Close several tabs in one write (spec 099).
///
/// A selection that survives stays. One that is closed moves to the nearest
/// surviving tab to its RIGHT in the old strip, else to its left — the single
/// close's rule, so "close tabs to the right" of a tab left of the selected
/// one lands on that tab, and "close other tabs" lands on the one kept.
/// Nothing left: nothing selected, the start page.
fn close_tabs(model: &mut Model, ids: &[String]) -> Command<ExploreEffect, Event> {
    if !ids
        .iter()
        .any(|id| model.doc.tabs.iter().any(|tab| &tab.id == id))
    {
        return Command::done();
    }
    let selected = selected_or_first(&model.doc);
    let before: Vec<String> = model.doc.tabs.iter().map(|tab| tab.id.clone()).collect();
    model.doc.tabs.retain(|tab| !ids.contains(&tab.id));
    model.recent.retain(|recent| !ids.contains(recent));
    let survives = |id: &String| !ids.contains(id) && before.contains(id);
    let selection_closed = selected.as_ref().is_some_and(|id| ids.contains(id));
    if selection_closed {
        let at = selected
            .as_ref()
            .and_then(|id| before.iter().position(|tab| tab == id))
            .unwrap_or(0);
        let next = before[at..]
            .iter()
            .find(|id| survives(id))
            .or_else(|| before[..at].iter().rev().find(|id| survives(id)))
            .cloned();
        model.doc.selected_tab = next.clone();
        if let Some(next) = next {
            used(model, next);
        }
    }
    persist(model)
}

/// Which tabs a batch close takes (spec 099) — Chrome's three, decided once
/// for every client: every tab but `keep`; every tab right of `of` in the
/// strip; every tab. An id the strip does not carry closes nothing.
#[must_use]
pub fn tabs_closed_by(tabs: &[ExploreTab], scope: &TabCloseScope) -> Vec<String> {
    match scope {
        TabCloseScope::Others { keep } => {
            if !tabs.iter().any(|tab| &tab.id == keep) {
                return Vec::new();
            }
            tabs.iter()
                .filter(|tab| &tab.id != keep)
                .map(|tab| tab.id.clone())
                .collect()
        }
        TabCloseScope::Right { of } => tabs
            .iter()
            .position(|tab| &tab.id == of)
            .map(|at| tabs[at + 1..].iter().map(|tab| tab.id.clone()).collect())
            .unwrap_or_default(),
        TabCloseScope::All => tabs.iter().map(|tab| tab.id.clone()).collect(),
    }
}

/// [`tabs_closed_by`] over JSON (UniFFI): the strip's tabs and a
/// [`TabCloseScope`] in, the ids out. `None` for input that does not read.
#[must_use]
pub fn tabs_closed_by_json(tabs_json: &str, scope_json: &str) -> Option<String> {
    let tabs: Vec<ExploreTab> = serde_json::from_str(tabs_json).ok()?;
    let scope: TabCloseScope = serde_json::from_str(scope_json).ok()?;
    serde_json::to_string(&tabs_closed_by(&tabs, &scope)).ok()
}

/// The selected tab, or the first — an id nothing carries selects nothing,
/// and a strip with tabs always has one of them selected.
fn selected_or_first(doc: &ExploreDoc) -> Option<String> {
    doc.selected_tab
        .as_ref()
        .filter(|id| doc.tabs.iter().any(|tab| &&tab.id == id))
        .cloned()
        .or_else(|| doc.tabs.first().map(|tab| tab.id.clone()))
}

/// Trimmed, or `None`. Blank is not a name.
fn non_blank(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn unique_tab_id(tabs: &[ExploreTab], now_ms: f64) -> String {
    #[allow(clippy::cast_possible_truncation, reason = "an id, not an instant")]
    let stamp = now_ms as i64;
    let mut id = format!("t-{stamp}");
    let mut suffix = 1;
    while tabs.iter().any(|t| t.id == id) {
        id = format!("t-{stamp}-{suffix}");
        suffix += 1;
    }
    id
}

/// `scheme://host[:port]` and the host, for the shapes a browser meets.
/// `None` for anything without a web authority — `about:blank`, a file, an
/// empty string. Mirrors [`super::browser_history`]'s parse, deliberately:
/// two answers to "what site is this" is one too many.
fn parse_web_origin(url: &str) -> Option<(String, String)> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.is_empty() {
        return None;
    }
    Some((
        format!("{scheme}://{authority}"),
        authority.to_ascii_lowercase(),
    ))
}

fn request(model: &mut Model, operation: ExploreOperation) -> Command<ExploreEffect, Event> {
    let attempt = model.attempt;
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

/// Mirror the model back to the store and redraw. Best effort: the write is
/// issued and its answer changes nothing.
fn persist(model: &mut Model) -> Command<ExploreEffect, Event> {
    let doc = model.doc.clone();
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(ExploreOperation::WriteExplore { doc })
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

/// The three lines every machine writes so the bridge and the test driver can
/// tell a shell request from a render without knowing this domain.
impl super::SplitEffect for ExploreEffect {
    type Op = ExploreOperation;
    fn into_shell(self) -> Option<crux_core::Request<ExploreOperation>> {
        match self {
            ExploreEffect::Render(_) => None,
            ExploreEffect::Shell(request) => Some(request),
        }
    }
}
