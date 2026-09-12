//! Rules of the browser's own memory — one test per rule the machine states.
//!
//! The rules here were WRITTEN rather than ported: spec 022 drew favourites,
//! groups and tabs, and no client owned what they mean. Each test is the
//! statement of one of those decisions, so a later change has to argue with a
//! sentence rather than with a diff.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::explore_sites::{
    Event, ExploreDoc, ExploreGroup, ExploreOperation as Op, ExploreShellResult as Res,
    ExploreSites, ExploreSystemGroup, ExploreTab, FAVORITES_CAP,
};

type Sut = DomainDriver<ExploreSites>;

const T0: f64 = 1_757_000_000_000.0;

/// Start the machine and hydrate it with `stored`.
fn ready(stored: Option<ExploreDoc>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Start);
    assert_eq!(ops, vec![Op::ReadExplore]);
    let ops = sut.resolve(Res::Loaded { doc: stored });
    assert!(ops.is_empty(), "hydration only renders");
    sut
}

/// The single document a mutation must have written.
fn written(ops: Vec<Op>) -> ExploreDoc {
    match ops.as_slice() {
        [Op::WriteExplore { doc }] => doc.clone(),
        other => panic!("expected exactly one WriteExplore, got {other:?}"),
    }
}

fn favorite(sut: &mut Sut, url: &str, title: Option<&str>, at: f64) -> ExploreDoc {
    written(sut.dispatch(Event::FavoriteAdded {
        url: url.to_owned(),
        title: title.map(str::to_owned),
        now_ms: at,
    }))
}

fn group(sut: &mut Sut, name: &str, at: f64) -> String {
    let doc = written(sut.dispatch(Event::GroupCreated {
        name: name.to_owned(),
        now_ms: at,
    }));
    doc.groups
        .last()
        .map(|g| g.id.clone())
        .expect("the group was created")
}

// ---------------------------------------------------------------------------
// Hydration
// ---------------------------------------------------------------------------

/// An absent document is an empty start page, never a failure.
#[test]
fn a_wallet_that_has_never_browsed_starts_empty_and_ready() {
    let sut = ready(None);
    let view = sut.view();
    assert!(view.ready);
    assert!(view.favorites.is_empty() && view.groups.is_empty() && view.tabs.is_empty());
    assert!(!view.favorites_full);
}

/// Nothing may be edited before the store is known.
///
/// Fail-closed: an edit applied to an empty document that is about to be
/// replaced would be lost, and the person would watch their favourite appear
/// and then vanish.
#[test]
fn edits_before_hydration_are_dropped() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::FavoriteAdded {
            url: "https://app.uniswap.org/swap".to_owned(),
            title: Some("Uniswap".to_owned()),
            now_ms: T0,
        })
        .is_empty());
    assert!(sut.view().favorites.is_empty());
}

/// A stored membership naming a site the document no longer carries is
/// dropped at hydration — a group must never draw a blank row.
#[test]
fn hydration_drops_memberships_whose_site_is_gone() {
    let doc = ExploreDoc {
        favorites: Vec::new(),
        groups: vec![ExploreGroup {
            id: "g-1".to_owned(),
            name: "Trading".to_owned(),
            members: vec!["https://gone.example".to_owned()],
            hidden: false,
            created_ms: T0,
        }],
        ..ExploreDoc::default()
    };
    let sut = ready(Some(doc));
    assert!(sut.view().groups[0].sites.is_empty());
}

// ---------------------------------------------------------------------------
// Favourites
// ---------------------------------------------------------------------------

/// A favourite is a SITE, and pinning one twice does not make two tiles.
///
/// The second pin refreshes where it opens — a person who pins from a deeper
/// page means that page — and never reorders the grid, which is their
/// arrangement rather than a recency list.
#[test]
fn a_site_pinned_twice_is_one_tile_that_follows_the_person() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://app.uniswap.org/", Some("Uniswap"), T0);
    favorite(&mut sut, "https://polymarket.com/", Some("Polymarket"), T0);
    let doc = favorite(
        &mut sut,
        "https://app.uniswap.org/swap?chain=gnosis",
        Some("Swap · Uniswap"),
        T0 + 1.0,
    );

    assert_eq!(doc.favorites.len(), 2, "the same site made a second tile");
    assert_eq!(doc.favorites[0].origin, "https://app.uniswap.org");
    assert_eq!(
        doc.favorites[0].url, "https://app.uniswap.org/swap?chain=gnosis",
        "it must open where the person pinned it from"
    );
    assert_eq!(doc.favorites[0].name, "Swap · Uniswap");
    assert_eq!(
        doc.favorites[1].origin, "https://polymarket.com",
        "re-pinning reordered a grid the person arranged"
    );
}

/// A name a person typed outlives every later visit.
///
/// A page can change its own `<title>` whenever it likes; a person who named
/// a tile meant it.
#[test]
fn a_renamed_tile_is_never_renamed_by_the_page() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://app.uniswap.org/", Some("Uniswap"), T0);
    written(sut.dispatch(Event::FavoriteRenamed {
        origin: "https://app.uniswap.org".to_owned(),
        name: "  My swap  ".to_owned(),
    }));

    let doc = favorite(
        &mut sut,
        "https://app.uniswap.org/",
        Some("Uniswap Interface"),
        T0 + 1.0,
    );
    assert_eq!(doc.favorites[0].name, "My swap", "trimmed, and kept");

    // Blank is not a name: it must not erase one back to what the site says.
    assert!(sut
        .dispatch(Event::FavoriteRenamed {
            origin: "https://app.uniswap.org".to_owned(),
            name: "   ".to_owned(),
        })
        .is_empty());
    assert_eq!(sut.view().favorites[0].name, "My swap");
}

/// Un-pinning takes the site out of every group it was in.
#[test]
fn unpinning_a_site_removes_it_from_its_groups() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://curve.fi/", Some("Curve"), T0);
    let id = group(&mut sut, "Trading", T0);
    written(sut.dispatch(Event::GroupMemberAdded {
        id: id.clone(),
        origin: "https://curve.fi".to_owned(),
    }));
    assert_eq!(sut.view().groups[0].sites.len(), 1);

    let doc = written(sut.dispatch(Event::FavoriteRemoved {
        origin: "https://curve.fi".to_owned(),
    }));
    assert!(doc.favorites.is_empty());
    assert!(
        doc.groups[0].members.is_empty(),
        "a group kept a membership pointing at nothing"
    );
}

/// Only a real web address can be pinned, and the grid has an end.
#[test]
fn what_cannot_be_pinned() {
    let mut sut = ready(None);
    for url in ["about:blank", "file:///Users/me/x.html", "", "https://"] {
        assert!(
            sut.dispatch(Event::FavoriteAdded {
                url: url.to_owned(),
                title: Some("nope".to_owned()),
                now_ms: T0,
            })
            .is_empty(),
            "{url} was pinned"
        );
    }

    for i in 0..FAVORITES_CAP {
        favorite(&mut sut, &format!("https://s{i}.example/"), None, T0);
    }
    assert!(
        sut.view().favorites_full,
        "the view must say the grid is full"
    );
    assert!(sut
        .dispatch(Event::FavoriteAdded {
            url: "https://one-too-many.example/".to_owned(),
            title: None,
            now_ms: T0,
        })
        .is_empty());
    assert_eq!(sut.view().favorites.len(), FAVORITES_CAP);
}

/// A page with no title of its own is its host, not a blank tile.
#[test]
fn an_untitled_page_is_pinned_under_its_host() {
    let mut sut = ready(None);
    let doc = favorite(&mut sut, "https://App.Uniswap.ORG/swap", None, T0);
    assert_eq!(doc.favorites[0].name, "app.uniswap.org");
    assert_eq!(
        doc.favorites[0].origin, "https://App.Uniswap.ORG",
        "the origin keeps the url's own spelling; the host is the lowercased one"
    );
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

/// Deleting a group keeps its sites — the rule contacts already settled.
///
/// A group is a shelf, not a box: throwing the shelf away does not throw away
/// the books. A person meets one behaviour across the wallet, not two.
#[test]
fn deleting_a_group_keeps_its_sites() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://curve.fi/", Some("Curve"), T0);
    let id = group(&mut sut, "Trading", T0);
    written(sut.dispatch(Event::GroupMemberAdded {
        id: id.clone(),
        origin: "https://curve.fi".to_owned(),
    }));

    let doc = written(sut.dispatch(Event::GroupDeleted { id }));
    assert!(doc.groups.is_empty());
    assert_eq!(doc.favorites.len(), 1, "the favourite went with the shelf");
}

/// A group lists FAVOURITES. An origin nobody pinned is not one.
#[test]
fn a_group_cannot_hold_a_site_that_is_not_pinned() {
    let mut sut = ready(None);
    let id = group(&mut sut, "Trading", T0);
    assert!(
        sut.dispatch(Event::GroupMemberAdded {
            id: id.clone(),
            origin: "https://never-pinned.example".to_owned(),
        })
        .is_empty(),
        "a group took a site the grid above cannot show"
    );
    // And the same site twice is one membership.
    favorite(&mut sut, "https://curve.fi/", Some("Curve"), T0);
    written(sut.dispatch(Event::GroupMemberAdded {
        id: id.clone(),
        origin: "https://curve.fi".to_owned(),
    }));
    assert!(sut
        .dispatch(Event::GroupMemberAdded {
            id,
            origin: "https://curve.fi".to_owned(),
        })
        .is_empty());
    assert_eq!(sut.view().groups[0].sites.len(), 1);
}

/// Two groups made in the same millisecond are two groups.
///
/// The shell's clock is the only id source this core has, and a shared id
/// would mean renaming one renames the other.
#[test]
fn groups_made_in_the_same_millisecond_keep_their_own_identities() {
    let mut sut = ready(None);
    let first = group(&mut sut, "Trading", T0);
    let second = group(&mut sut, "Prediction", T0);
    assert_ne!(first, second);

    written(sut.dispatch(Event::GroupRenamed {
        id: second,
        name: "Markets".to_owned(),
    }));
    let view = sut.view();
    assert_eq!(view.groups[0].name, "Trading");
    assert_eq!(view.groups[1].name, "Markets");
}

/// A system group can be hidden and can never be deleted.
///
/// There is no `SystemGroupDeleted` event at all — the rule is expressed by
/// the protocol rather than by a check, which is the only way it cannot be
/// forgotten. This pins the half that exists.
#[test]
fn system_groups_hide_rather_than_disappear() {
    let mut sut = ready(None);
    let doc = written(sut.dispatch(Event::SystemGroupHiddenSet {
        group: ExploreSystemGroup::Recent,
        hidden: true,
    }));
    assert_eq!(doc.hidden_system, vec![ExploreSystemGroup::Recent]);
    assert!(sut.view().recent_hidden && !sut.view().favorites_hidden);

    // Hiding twice is still hidden once.
    let doc = written(sut.dispatch(Event::SystemGroupHiddenSet {
        group: ExploreSystemGroup::Recent,
        hidden: true,
    }));
    assert_eq!(doc.hidden_system.len(), 1);

    let doc = written(sut.dispatch(Event::SystemGroupHiddenSet {
        group: ExploreSystemGroup::Recent,
        hidden: false,
    }));
    assert!(doc.hidden_system.is_empty());
}

/// A blank name creates and renames nothing.
#[test]
fn a_group_needs_a_name() {
    let mut sut = ready(None);
    assert!(sut
        .dispatch(Event::GroupCreated {
            name: "   ".to_owned(),
            now_ms: T0,
        })
        .is_empty());
    let id = group(&mut sut, "Trading", T0);
    assert!(sut
        .dispatch(Event::GroupRenamed {
            id,
            name: "\t\n".to_owned(),
        })
        .is_empty());
    assert_eq!(sut.view().groups[0].name, "Trading");
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// Opening a tab selects it — that is what opening one means.
#[test]
fn a_new_tab_is_the_selected_one() {
    let mut sut = ready(None);
    written(sut.dispatch(Event::TabOpened {
        url: Some("https://a.example/".to_owned()),
        title: Some("A".to_owned()),
        now_ms: T0,
    }));
    let doc = written(sut.dispatch(Event::TabOpened {
        url: Some("https://b.example/".to_owned()),
        title: None,
        now_ms: T0 + 1.0,
    }));
    assert_eq!(doc.tabs.len(), 2);
    assert_eq!(doc.selected_tab.as_deref(), Some(doc.tabs[1].id.as_str()));
    assert_eq!(
        doc.tabs[1].title, "b.example",
        "an untitled page is its host"
    );
}

/// Closing the selected tab hands selection to its right-hand neighbour, and
/// to the left when it was the last — the convention a person's hand expects.
#[test]
fn closing_the_selected_tab_selects_a_neighbour() {
    let mut sut = ready(None);
    for (i, host) in ["a", "b", "c"].iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "three test timestamps")]
        let at = T0 + i as f64;
        written(sut.dispatch(Event::TabOpened {
            url: Some(format!("https://{host}.example/")),
            title: Some((*host).to_owned()),
            now_ms: at,
        }));
    }
    let ids: Vec<String> = sut.view().tabs.iter().map(|t| t.id.clone()).collect();

    // Select the middle one and close it: selection moves right, to "c".
    written(sut.dispatch(Event::TabSelected { id: ids[1].clone() }));
    let doc = written(sut.dispatch(Event::TabClosed { id: ids[1].clone() }));
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[2].as_str()));

    // Close the last one: selection falls back to the left.
    let doc = written(sut.dispatch(Event::TabClosed { id: ids[2].clone() }));
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[0].as_str()));

    // Close the final tab: nothing is open and nothing is selected — which is
    // what a browser showing its start page is.
    let doc = written(sut.dispatch(Event::TabClosed { id: ids[0].clone() }));
    assert!(doc.tabs.is_empty());
    assert_eq!(sut.view().selected_tab, None);
}

/// Closing a tab that is not selected leaves the selection alone.
#[test]
fn closing_another_tab_does_not_move_the_person() {
    let mut sut = ready(None);
    for host in ["a", "b"] {
        written(sut.dispatch(Event::TabOpened {
            url: Some(format!("https://{host}.example/")),
            title: Some(host.to_owned()),
            now_ms: T0,
        }));
    }
    let ids: Vec<String> = sut.view().tabs.iter().map(|t| t.id.clone()).collect();
    written(sut.dispatch(Event::TabSelected { id: ids[1].clone() }));
    let doc = written(sut.dispatch(Event::TabClosed { id: ids[0].clone() }));
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[1].as_str()));
}

/// A tab that navigates takes the new page's title, or its host — never the
/// title of the page it used to be on.
#[test]
fn a_navigated_tab_never_keeps_the_last_pages_name() {
    let mut sut = ready(None);
    written(sut.dispatch(Event::TabOpened {
        url: Some("https://a.example/".to_owned()),
        title: Some("Aaa".to_owned()),
        now_ms: T0,
    }));
    let id = sut.view().tabs[0].id.clone();
    let doc = written(sut.dispatch(Event::TabNavigated {
        id: id.clone(),
        url: "https://b.example/page".to_owned(),
        title: None,
    }));
    assert_eq!(doc.tabs[0].title, "b.example");
    assert_eq!(doc.tabs[0].url.as_deref(), Some("https://b.example/page"));

    // A navigation to something that is not a web page changes nothing.
    assert!(sut
        .dispatch(Event::TabNavigated {
            id,
            url: "about:blank".to_owned(),
            title: Some("blank".to_owned()),
        })
        .is_empty());
    assert_eq!(sut.view().tabs[0].host, "b.example");
}

/// A selection naming a tab that is gone reads as the first tab.
///
/// A hand-edited or truncated document must not leave the strip with nothing
/// selected while it still has tabs.
#[test]
fn a_stale_selection_falls_back_to_the_first_tab() {
    let doc = ExploreDoc {
        tabs: vec![
            ExploreTab {
                id: "t-1".to_owned(),
                url: Some("https://a.example/".to_owned()),
                title: "A".to_owned(),
                host: "a.example".to_owned(),
            },
            ExploreTab {
                id: "t-2".to_owned(),
                url: None,
                title: String::new(),
                host: String::new(),
            },
        ],
        selected_tab: Some("t-gone".to_owned()),
        ..ExploreDoc::default()
    };
    let sut = ready(Some(doc));
    assert_eq!(sut.view().selected_tab.as_deref(), Some("t-1"));
}
