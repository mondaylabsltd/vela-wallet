//! Machine — the browser's own memory: favourites, groups and open tabs.
//!
//! ```text
//! Start ─► ReadExplore ─► Loaded{doc} ─► ready mirror
//!   FavoriteAdded ─► dedupe by ORIGIN ─┬─ known ─► refresh title/url in place
//!                                      └─ new ─► append (cap) ─► WriteExplore
//!   Group*/Tab* ─► pure edit ─► WriteExplore
//! ```
//!
//! ## Why this machine exists
//!
//! Spec 022 drew a start page with a favourites grid, custom groups and a tab
//! strip, and every client rendered them from fixtures — the web says so in
//! its own loader ("the vocabulary still ships for the gallery"), and the
//! desktop said the same thing in `explore/fixtures.rs`. Nothing anywhere
//! owned the RULES: where a favourite lives, what a group is, who owns a tab.
//! Four shells drawing the same picture from four sets of mock data is not a
//! feature; this is the machine that makes it one, and the founder asked for
//! it on 2026-09-08.
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
//! `vela.explore` holds all three lists. They are edited TOGETHER — a site is
//! favourited from a tab, a favourite is dropped into a group — and three
//! keys can half-write a membership whose site no longer exists. The write is
//! whole and best-effort, like the history's; the mirror stays authoritative.

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

/// Open tabs. A browser that lets a page open tabs without bound is a browser
/// a page can wedge; this shell's tabs are opened by a PERSON, and two dozen
/// is well past what anyone arranges on purpose.
pub const TABS_CAP: usize = 24;

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
    /// What the tile says. The page's own title until somebody renames it —
    /// and a rename is kept forever after, because a person who named a tile
    /// meant it and a page can change its `<title>` at will.
    pub name: String,
    /// `true` once a person renamed it: a later visit must not overwrite it.
    pub renamed: bool,
    pub added_ms: f64,
}

/// A named collection of favourites.
///
/// Membership is by ORIGIN, and a site may be in several groups — a group is
/// a VIEW over the favourites, never a container that owns them. Which is why
/// deleting one keeps its sites (the rule `contacts` already settled for
/// contact groups, so a person meets one behaviour, not two).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreGroup {
    pub id: String,
    pub name: String,
    /// Origins, in the order the person added them.
    pub members: Vec<String>,
    /// A hidden group keeps everything and draws nothing. System groups can
    /// only ever be hidden — never deleted (spec 022's own model: "system
    /// groups: hideable, never deletable").
    pub hidden: bool,
    pub created_ms: f64,
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

/// The two system groups the start page always has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExploreSystemGroup {
    Favorites,
    Recent,
}

/// The stored document, whole.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreDoc {
    #[serde(default)]
    pub favorites: Vec<ExploreSite>,
    #[serde(default)]
    pub groups: Vec<ExploreGroup>,
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
    /// Unpin. Its group memberships go with it — a group must never list a
    /// site the favourites no longer have.
    FavoriteRemoved {
        origin: String,
    },
    /// A person names a tile. Empty or blank is a no-op rather than a way to
    /// erase a name into a title the site controls.
    FavoriteRenamed {
        origin: String,
        name: String,
    },
    GroupCreated {
        name: String,
        now_ms: f64,
    },
    GroupRenamed {
        id: String,
        name: String,
    },
    /// Delete a group. Its sites STAY favourited (see [`ExploreGroup`]).
    GroupDeleted {
        id: String,
    },
    GroupHiddenSet {
        id: String,
        hidden: bool,
    },
    /// Hide or show one of the two system groups. They cannot be deleted.
    SystemGroupHiddenSet {
        group: ExploreSystemGroup,
        hidden: bool,
    },
    /// Membership. Adding a site that is not favourited favourites nothing —
    /// a group is a view over the favourites, so the shell pins first.
    GroupMemberAdded {
        id: String,
        origin: String,
    },
    GroupMemberRemoved {
        id: String,
        origin: String,
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
    /// Close one. The strip is never left empty and never left with nothing
    /// selected — see [`close_tab`].
    TabClosed {
        id: String,
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
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// A group with its sites resolved, ready to draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreGroupView {
    pub id: String,
    pub name: String,
    pub hidden: bool,
    /// Resolved from the favourites, in membership order. A member whose site
    /// is gone is dropped here rather than drawn as a blank row.
    pub sites: Vec<ExploreSite>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExploreView {
    pub favorites: Vec<ExploreSite>,
    pub groups: Vec<ExploreGroupView>,
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

            Event::FavoriteRemoved { origin } => {
                model.doc.favorites.retain(|s| s.origin != origin);
                // …and out of every group. A membership pointing at nothing
                // is a row that cannot be drawn and a count that lies.
                for group in &mut model.doc.groups {
                    group.members.retain(|member| member != &origin);
                }
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

            Event::GroupCreated { name, now_ms } => {
                let Some(name) = non_blank(Some(name)) else {
                    return Command::done();
                };
                model.doc.groups.push(ExploreGroup {
                    // The shell's clock is the only id source this core has,
                    // and a collision would merge two groups, so it is made
                    // unique against what is already there.
                    id: unique_id(&model.doc.groups, now_ms),
                    name,
                    members: Vec::new(),
                    hidden: false,
                    created_ms: now_ms,
                });
                persist(model)
            }

            Event::GroupRenamed { id, name } => {
                let Some(name) = non_blank(Some(name)) else {
                    return Command::done();
                };
                let Some(group) = model.doc.groups.iter_mut().find(|g| g.id == id) else {
                    return Command::done();
                };
                group.name = name;
                persist(model)
            }

            Event::GroupDeleted { id } => {
                let before = model.doc.groups.len();
                model.doc.groups.retain(|g| g.id != id);
                if model.doc.groups.len() == before {
                    return Command::done();
                }
                // The sites stay favourited. Deleting a shelf is not
                // throwing away the books on it.
                persist(model)
            }

            Event::GroupHiddenSet { id, hidden } => {
                let Some(group) = model.doc.groups.iter_mut().find(|g| g.id == id) else {
                    return Command::done();
                };
                group.hidden = hidden;
                persist(model)
            }

            Event::SystemGroupHiddenSet { group, hidden } => {
                model.doc.hidden_system.retain(|g| *g != group);
                if hidden {
                    model.doc.hidden_system.push(group);
                }
                persist(model)
            }

            Event::GroupMemberAdded { id, origin } => {
                // A group lists favourites. An origin nobody pinned is not
                // one, and adding it would put a row in a group that the
                // grid above cannot show.
                if !model.doc.favorites.iter().any(|s| s.origin == origin) {
                    return Command::done();
                }
                let Some(group) = model.doc.groups.iter_mut().find(|g| g.id == id) else {
                    return Command::done();
                };
                if group.members.contains(&origin) {
                    return Command::done();
                }
                group.members.push(origin);
                persist(model)
            }

            Event::GroupMemberRemoved { id, origin } => {
                let Some(group) = model.doc.groups.iter_mut().find(|g| g.id == id) else {
                    return Command::done();
                };
                let before = group.members.len();
                group.members.retain(|member| member != &origin);
                if group.members.len() == before {
                    return Command::done();
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
                model.doc.selected_tab = Some(id);
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
                model.doc.selected_tab = Some(id);
                persist(model)
            }

            Event::TabClosed { id } => close_tab(model, &id),

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
        let groups = doc
            .groups
            .iter()
            .map(|group| ExploreGroupView {
                id: group.id.clone(),
                name: group.name.clone(),
                hidden: group.hidden,
                sites: group
                    .members
                    .iter()
                    .filter_map(|origin| {
                        doc.favorites.iter().find(|s| &s.origin == origin).cloned()
                    })
                    .collect(),
            })
            .collect();
        ExploreView {
            favorites: doc.favorites.clone(),
            groups,
            tabs: doc.tabs.clone(),
            // Never an id nothing carries: a strip with tabs always has one
            // of them selected.
            selected_tab: selected_or_first(doc),
            favorites_hidden: doc.hidden_system.contains(&ExploreSystemGroup::Favorites),
            recent_hidden: doc.hidden_system.contains(&ExploreSystemGroup::Recent),
            favorites_full: doc.favorites.len() >= FAVORITES_CAP,
            tabs_full: doc.tabs.len() >= TABS_CAP,
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
            model.doc = doc.unwrap_or_default();
            // A stored document is data, not a promise: memberships that name
            // sites it does not carry are dropped here rather than drawn as
            // blank rows for the rest of the session.
            let known: Vec<String> = model
                .doc
                .favorites
                .iter()
                .map(|site| site.origin.clone())
                .collect();
            for group in &mut model.doc.groups {
                group.members.retain(|member| known.contains(member));
            }
            model.phase = Phase::Ready;
            render()
        }
        _ => Command::done(),
    }
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
    }
    persist(model)
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

/// `g-<ms>`, made unique against the groups that exist.
///
/// The shell's clock is the only id source, and two groups created inside one
/// millisecond would otherwise share an id — after which renaming one renames
/// the other.
fn unique_id(groups: &[ExploreGroup], now_ms: f64) -> String {
    #[allow(clippy::cast_possible_truncation, reason = "an id, not an instant")]
    let stamp = now_ms as i64;
    let mut id = format!("g-{stamp}");
    let mut suffix = 1;
    while groups.iter().any(|g| g.id == id) {
        id = format!("g-{stamp}-{suffix}");
        suffix += 1;
    }
    id
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
