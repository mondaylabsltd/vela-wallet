//! Rules of the browser's own memory — one test per rule the machine states.
//!
//! The rules here were WRITTEN rather than ported: spec 022 drew favourites,
//! groups and tabs, and no client owned what they mean (custom groups are
//! gone since issue #465; the two sections can still be hidden). Each test is the
//! statement of one of those decisions, so a later change has to argue with a
//! sentence rather than with a diff.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::browser_load::{visit_to_record, LoadFinished};
use vela_core::app::explore_sites::{tabs_closed_by, TabCloseScope};
use vela_core::app::explore_sites::{
    Event, ExploreDoc, ExploreOperation as Op, ExploreShellResult as Res, ExploreSites,
    ExploreSystemGroup, ExploreTab, ExploreView, FAVORITES_CAP, NAME_RULE, RESUME_SHOWN,
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

// ---------------------------------------------------------------------------
// Hydration
// ---------------------------------------------------------------------------

/// An absent document is an empty start page, never a failure.
#[test]
fn a_wallet_that_has_never_browsed_starts_empty_and_ready() {
    let sut = ready(None);
    let view = sut.view();
    assert!(view.ready);
    assert!(view.favorites.is_empty() && view.tabs.is_empty());
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

/// A `vela.explore` document written before issue #465, with custom groups —
/// a named one listing a favourite (desktop's "Move to group…") and an empty
/// one a phone made — in the shape the shells store.
const PRE_465_DOC: &str = r#"{
    "favorites": [
        {"origin": "https://curve.fi", "url": "https://curve.fi/",
         "host": "curve.fi", "name": "Curve", "renamed": false,
         "added_ms": 1759051384000.0},
        {"origin": "https://polymarket.com", "url": "https://polymarket.com/",
         "host": "polymarket.com", "name": "Polymarket", "renamed": false,
         "added_ms": 1759051385000.0}
    ],
    "groups": [
        {"id": "g-1759051390000", "name": "Trading",
         "members": ["https://curve.fi", "https://gone.example"],
         "hidden": false, "created_ms": 1759051390000.0},
        {"id": "g-1759051391000", "name": "New group", "members": [],
         "hidden": true, "created_ms": 1759051391000.0}
    ],
    "tabs": [],
    "selected_tab": null,
    "hidden_system": ["recent"],
    "name_rule": 1
}"#;

/// Issue #465: custom groups are gone. A document that still carries them
/// reads — on the wire the shell answers with, too — keeps every favourite
/// and the hidden sections, and the next write leaves `groups` out.
#[test]
fn a_document_from_before_465_keeps_every_favourite_and_loses_its_groups() {
    let stored: ExploreDoc = serde_json::from_str(PRE_465_DOC).expect("the pre-#465 shape reads");
    let answer: Res = serde_json::from_str(&format!(r#"{{"type":"loaded","doc":{PRE_465_DOC}}}"#))
        .expect("the shell's answer reads with the old groups in it");
    assert_eq!(
        answer,
        Res::Loaded {
            doc: Some(stored.clone())
        }
    );

    let mut sut = ready(Some(stored));
    let view = sut.view();
    let origins: Vec<&str> = view
        .favorites
        .iter()
        .map(|site| site.origin.as_str())
        .collect();
    assert_eq!(origins, vec!["https://curve.fi", "https://polymarket.com"]);
    assert!(view.recent_hidden, "a hidden section stays hidden");
    assert!(!view.favorites_hidden);

    // The next write is the document without groups.
    let doc = written(sut.dispatch(Event::FavoriteRenamed {
        origin: "https://curve.fi".to_owned(),
        name: "Stables".to_owned(),
    }));
    let json = serde_json::to_value(&doc).expect("the document serialises");
    assert!(
        json.get("groups").is_none(),
        "groups were written back: {json}"
    );
    assert_eq!(doc.favorites.len(), 2, "every favourite is kept");
    assert_eq!(doc.hidden_system, vec![ExploreSystemGroup::Recent]);
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

/// Un-pinning takes the tile away.
#[test]
fn unpinning_a_site_removes_its_tile() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://curve.fi/", Some("Curve"), T0);
    let doc = written(sut.dispatch(Event::FavoriteRemoved {
        origin: "https://curve.fi".to_owned(),
    }));
    assert!(doc.favorites.is_empty());
    assert!(sut.view().favorites.is_empty());
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

// ---------------------------------------------------------------------------
// A favourite's name — issue #425
// ---------------------------------------------------------------------------

/// The `vela.explore` document the Xiaomi carried from v0.9.5 into v0.9.6,
/// in the shape Android's serializer writes it: app.uniswap.org pinned while
/// it had failed to load, under the WebView's error page's title (#329) —
/// with no `name_rule`, because no build before the fix wrote one.
const XIAOMI_V095_DOC: &str = r#"{
    "favorites": [
        {"origin": "https://app.uniswap.org", "url": "https://app.uniswap.org/",
         "host": "app.uniswap.org", "name": "网页无法打开", "renamed": false,
         "added_ms": 1759051383000.0},
        {"origin": "https://curve.fi", "url": "https://curve.fi/",
         "host": "curve.fi", "name": "My stables", "renamed": true,
         "added_ms": 1759051384000.0}
    ],
    "groups": [],
    "tabs": [{"id": "t-1759051380000", "url": "https://app.uniswap.org/",
              "title": "Uniswap Interface", "host": "app.uniswap.org"}],
    "selected_tab": "t-1759051380000",
    "hidden_system": []
}"#;

/// The visit a finished load is, by the core's own rule — what a shell
/// dispatches as [`Event::PageLoaded`] (and records in Recents).
fn page_loaded(url: &str, title: &str, failed: bool) -> Option<Event> {
    visit_to_record(LoadFinished {
        url: url.to_owned(),
        title: title.to_owned(),
        icon: None,
        main_frame_failed: failed,
        http_status: None,
    })
    .map(|visit| Event::PageLoaded {
        url: visit.url,
        title: visit.title,
    })
}

/// Issue #425, the exact case: #345 (issue #329) named NEW favourites by the
/// site's last good title or its host, and read the stored ones as they were
/// — so the Uniswap tile pinned under v0.9.5 still read "网页无法打开" in
/// v0.9.6, while Recents, right under it, named the site properly. A document
/// from before the rule has every name nobody chose replaced by its host on
/// hydration, and written back once; a person's rename stays.
#[test]
fn a_favourite_stored_before_the_rule_under_an_error_page_is_named_by_its_host() {
    let stored: ExploreDoc = serde_json::from_str(XIAOMI_V095_DOC).expect("the v0.9.5 shape reads");
    assert_eq!(
        stored.name_rule, 0,
        "a document from before the rule says so"
    );

    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    let doc = written(sut.resolve(Res::Loaded { doc: Some(stored) }));

    let view = sut.view();
    assert!(view.ready);
    assert_eq!(
        view.favorites[0].name, "app.uniswap.org",
        "never the error page"
    );
    assert_eq!(
        view.favorites[1].name, "My stables",
        "a person's name is theirs"
    );
    assert_eq!(
        doc.name_rule, NAME_RULE,
        "written back under the rule, once"
    );
    assert_eq!(doc.favorites[0].name, "app.uniswap.org");
    assert_eq!(doc.favorites[1].name, "My stables");
    // Nothing else in the document moved.
    assert_eq!(doc.tabs[0].title, "Uniswap Interface");
    assert_eq!(doc.selected_tab.as_deref(), Some("t-1759051380000"));

    // The next launch reads a document made under the rule: nothing to write.
    let ready_again = ready(Some(doc));
    assert_eq!(ready_again.view().favorites[0].name, "app.uniswap.org");
}

/// …and the site's next good load names it — "Uniswap | Trade Crypto on
/// DeFi's…", the title Recents already showed. An engine's error page is no
/// load (the core's visit rule), so it can never be the one that names it.
#[test]
fn the_sites_next_good_load_names_a_favourite_its_host_stands_in_for() {
    let stored: ExploreDoc = serde_json::from_str(XIAOMI_V095_DOC).expect("the v0.9.5 shape reads");
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    written(sut.resolve(Res::Loaded { doc: Some(stored) }));

    // The error page again: failed, or read at its chrome-error:// address.
    assert!(page_loaded("https://app.uniswap.org/", "网页无法打开", true).is_none());
    assert!(page_loaded("chrome-error://chromewebdata/", "网页无法打开", false).is_none());

    let good = page_loaded(
        "https://app.uniswap.org/swap",
        "Uniswap | Trade Crypto on DeFi's Leading Exchange",
        false,
    )
    .expect("a page that loaded is a visit");
    let doc = written(sut.dispatch(good));
    assert_eq!(
        doc.favorites[0].name,
        "Uniswap | Trade Crypto on DeFi's Leading Exchange"
    );
    assert_eq!(
        doc.favorites[1].name, "My stables",
        "a rename is never retitled"
    );
}

/// A favourite named by its host — pinned while its site had failed (the
/// core's `pinned_title` gave no title) — takes the site's title from its
/// first good load. One with a title keeps it: a tile does not change its
/// name with every page of the site; nor does one a person named, even with
/// the host itself; nor does a load of another site, or one with no title.
#[test]
fn only_a_favourite_still_named_by_its_host_takes_a_loads_title() {
    let mut sut = ready(None);
    favorite(&mut sut, "https://app.uniswap.org/", None, T0);
    favorite(&mut sut, "https://curve.fi/", Some("Curve"), T0);
    favorite(&mut sut, "https://aave.com/", None, T0);
    written(sut.dispatch(Event::FavoriteRenamed {
        origin: "https://aave.com".to_owned(),
        name: "aave.com".to_owned(),
    }));
    assert_eq!(sut.view().favorites[0].name, "app.uniswap.org");

    // Another site's load, a load with no title, a blank title: nothing.
    for (url, title) in [
        ("https://app.sushi.com/", Some("Sushi")),
        ("https://app.uniswap.org/", None),
        ("https://app.uniswap.org/", Some("   ")),
    ] {
        let event = Event::PageLoaded {
            url: url.to_owned(),
            title: title.map(str::to_owned),
        };
        assert!(sut.dispatch(event).is_empty(), "{url} {title:?} wrote");
    }

    // The site's own load, however the page spelled its host.
    let doc = written(sut.dispatch(Event::PageLoaded {
        url: "https://APP.uniswap.org/positions".to_owned(),
        title: Some("  Uniswap Interface ".to_owned()),
    }));
    assert_eq!(doc.favorites[0].name, "Uniswap Interface");

    // Titled now: a later page of the site does not rename it.
    for (url, title) in [
        ("https://app.uniswap.org/explore", "Explore | Uniswap"),
        ("https://curve.fi/dex", "Curve DEX"),
        ("https://aave.com/", "Aave - Open Source Liquidity Protocol"),
    ] {
        let event = Event::PageLoaded {
            url: url.to_owned(),
            title: Some(title.to_owned()),
        };
        assert!(sut.dispatch(event).is_empty(), "{url} renamed its tile");
    }
    let names: Vec<String> = sut
        .view()
        .favorites
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["Uniswap Interface", "Curve", "aave.com"]);
}

/// A load before the store is known changes nothing — it is dropped like
/// every edit, and the next one names the tile.
#[test]
fn a_load_before_hydration_is_dropped() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::PageLoaded {
            url: "https://app.uniswap.org/".to_owned(),
            title: Some("Uniswap".to_owned()),
        })
        .is_empty());
}

/// A document written under the rule is read as it is: its names were made
/// by `pinned_title`, so a good title is kept and nothing is written.
#[test]
fn a_document_made_under_the_rule_keeps_its_names() {
    let mut sut = ready(None);
    let doc = favorite(
        &mut sut,
        "https://app.uniswap.org/",
        Some("Uniswap Interface"),
        T0,
    );
    assert_eq!(doc.name_rule, NAME_RULE, "every write carries the rule");
    let again = ready(Some(doc));
    assert_eq!(again.view().favorites[0].name, "Uniswap Interface");
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
// The two sections (the only groups since issue #465)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Recency (spec 099 R2) — the order browser_tabs keeps engines alive in
// ---------------------------------------------------------------------------

/// Opening and selecting a tab put it first; closing one forgets it, and the
/// tab that takes over from a closed selected one is the one in use now.
#[test]
fn recency_follows_the_person() {
    let mut sut = ready(None);
    for (i, host) in ["a", "b", "c"].iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "three test timestamps")]
        let at = T0 + i as f64;
        written(sut.dispatch(Event::TabOpened {
            url: Some(format!("https://{host}.example/")),
            title: None,
            now_ms: at,
        }));
    }
    let ids: Vec<String> = sut.view().tabs.iter().map(|t| t.id.clone()).collect();
    assert_eq!(
        sut.view().recent_tabs,
        vec![ids[2].clone(), ids[1].clone(), ids[0].clone()]
    );

    written(sut.dispatch(Event::TabSelected { id: ids[0].clone() }));
    assert_eq!(
        sut.view().recent_tabs,
        vec![ids[0].clone(), ids[2].clone(), ids[1].clone()]
    );

    // Closing the selected tab selects its right-hand neighbour, which is
    // the tab in use now.
    written(sut.dispatch(Event::TabClosed { id: ids[0].clone() }));
    assert_eq!(sut.view().selected_tab.as_deref(), Some(ids[1].as_str()));
    assert_eq!(sut.view().recent_tabs, vec![ids[1].clone(), ids[2].clone()]);
}

/// At launch only the selected tab has had a page this session.
#[test]
fn recency_starts_at_the_selected_tab() {
    let tab = |id: &str| ExploreTab {
        id: id.to_owned(),
        url: Some(format!("https://{id}.example/")),
        title: id.to_owned(),
        host: format!("{id}.example"),
    };
    let sut = ready(Some(ExploreDoc {
        tabs: vec![tab("t-1"), tab("t-2"), tab("t-3")],
        selected_tab: Some("t-2".to_owned()),
        ..ExploreDoc::default()
    }));
    assert_eq!(sut.view().recent_tabs, vec!["t-2".to_owned()]);
}

// ---------------------------------------------------------------------------
// Batch close (spec 099) — Chrome's three
// ---------------------------------------------------------------------------

fn five_tabs() -> (Sut, Vec<String>) {
    let mut sut = ready(None);
    for (i, host) in ["a", "b", "c", "d", "e"].iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "five test timestamps")]
        let at = T0 + i as f64;
        written(sut.dispatch(Event::TabOpened {
            url: Some(format!("https://{host}.example/")),
            title: None,
            now_ms: at,
        }));
    }
    let ids = sut.view().tabs.iter().map(|t| t.id.clone()).collect();
    (sut, ids)
}

/// The three scopes name exactly the tabs they say; an id the strip does not
/// carry closes nothing.
#[test]
fn each_scope_names_its_tabs() {
    let (sut, ids) = five_tabs();
    let tabs = sut.view().tabs;
    assert_eq!(
        tabs_closed_by(
            &tabs,
            &TabCloseScope::Others {
                keep: ids[2].clone()
            }
        ),
        vec![
            ids[0].clone(),
            ids[1].clone(),
            ids[3].clone(),
            ids[4].clone()
        ]
    );
    assert_eq!(
        tabs_closed_by(&tabs, &TabCloseScope::Right { of: ids[2].clone() }),
        vec![ids[3].clone(), ids[4].clone()]
    );
    assert!(tabs_closed_by(&tabs, &TabCloseScope::Right { of: ids[4].clone() }).is_empty());
    assert_eq!(tabs_closed_by(&tabs, &TabCloseScope::All), ids);
    assert!(tabs_closed_by(
        &tabs,
        &TabCloseScope::Others {
            keep: "t-gone".to_owned()
        }
    )
    .is_empty());
}

/// "Close other tabs" on a tab that is not selected: that tab is the one in
/// use after, in one write.
#[test]
fn closing_others_selects_the_one_kept() {
    let (mut sut, ids) = five_tabs();
    let closing = tabs_closed_by(
        &sut.view().tabs,
        &TabCloseScope::Others {
            keep: ids[1].clone(),
        },
    );
    let doc = written(sut.dispatch(Event::TabsClosed { ids: closing }));
    assert_eq!(doc.tabs.len(), 1);
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[1].as_str()));
    assert_eq!(sut.view().recent_tabs, vec![ids[1].clone()]);
}

/// "Close tabs to the right" of a tab left of the selected one: the
/// selection moves left, to the nearest tab that survives — that tab.
#[test]
fn closing_to_the_right_moves_a_closed_selection_left() {
    let (mut sut, ids) = five_tabs();
    assert_eq!(sut.view().selected_tab.as_deref(), Some(ids[4].as_str()));
    let closing = tabs_closed_by(
        &sut.view().tabs,
        &TabCloseScope::Right { of: ids[1].clone() },
    );
    let doc = written(sut.dispatch(Event::TabsClosed { ids: closing }));
    assert_eq!(
        doc.tabs.iter().map(|t| t.id.clone()).collect::<Vec<_>>(),
        ids[..2].to_vec()
    );
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[1].as_str()));
}

/// A selection that survives stays where it is; closing all leaves the start
/// page; closing nothing writes nothing.
#[test]
fn a_surviving_selection_stays_and_all_leaves_the_start_page() {
    let (mut sut, ids) = five_tabs();
    written(sut.dispatch(Event::TabSelected { id: ids[0].clone() }));
    let doc = written(sut.dispatch(Event::TabsClosed {
        ids: vec![ids[2].clone(), ids[3].clone()],
    }));
    assert_eq!(doc.selected_tab.as_deref(), Some(ids[0].as_str()));
    assert!(sut
        .dispatch(Event::TabsClosed {
            ids: vec!["t-gone".to_owned()]
        })
        .is_empty());
    let all = tabs_closed_by(&sut.view().tabs, &TabCloseScope::All);
    let doc = written(sut.dispatch(Event::TabsClosed { ids: all }));
    assert!(doc.tabs.is_empty());
    assert_eq!(sut.view().selected_tab, None);
}

// ---------------------------------------------------------------------------
// The home's resume rows (spec 099 navigation)
// ---------------------------------------------------------------------------

fn ids_of(tabs: &[ExploreTab]) -> Vec<String> {
    tabs.iter().map(|tab| tab.id.clone()).collect()
}

/// The tabs with a page, the one used last first. A start-page tab is never
/// a row — there is nothing in it to go back to.
#[test]
fn the_resume_rows_are_the_tabs_with_a_page_most_recent_first() {
    let mut sut = ready(None);
    for (i, url) in [
        Some("https://a.example/"),
        Some("https://b.example/"),
        None,
        Some("https://c.example/"),
    ]
    .iter()
    .enumerate()
    {
        #[allow(clippy::cast_precision_loss, reason = "four test timestamps")]
        let at = T0 + i as f64;
        written(sut.dispatch(Event::TabOpened {
            url: url.map(str::to_owned),
            title: None,
            now_ms: at,
        }));
    }
    let ids = ids_of(&sut.view().tabs);
    assert_eq!(
        ids_of(&sut.view().resumable),
        vec![ids[3].clone(), ids[1].clone(), ids[0].clone()]
    );

    written(sut.dispatch(Event::TabSelected { id: ids[0].clone() }));
    assert_eq!(
        ids_of(&sut.view().resumable),
        vec![ids[0].clone(), ids[3].clone(), ids[1].clone()]
    );

    // The start-page tab gets a page: it is a row now, and the first.
    written(sut.dispatch(Event::TabSelected { id: ids[2].clone() }));
    written(sut.dispatch(Event::TabNavigated {
        id: ids[2].clone(),
        url: "https://d.example/".to_owned(),
        title: None,
    }));
    let rows = sut.view().resumable;
    assert_eq!(rows.len(), RESUME_SHOWN);
    assert_eq!(rows[0].id, ids[2]);
    assert_eq!(rows[0].host, "d.example", "a row carries what it draws");
}

/// At most [`RESUME_SHOWN`]: the switcher, one tap away, is the full list.
#[test]
fn the_resume_rows_stop_at_three() {
    let (sut, ids) = five_tabs();
    assert_eq!(
        ids_of(&sut.view().resumable),
        vec![ids[4].clone(), ids[3].clone(), ids[2].clone()]
    );
}

/// At launch recency knows only the selected tab: it comes first, then the
/// restored tabs in strip order.
#[test]
fn restored_tabs_resume_from_the_selected_one_then_in_strip_order() {
    let tab = |id: &str, url: Option<&str>| ExploreTab {
        id: id.to_owned(),
        url: url.map(str::to_owned),
        title: id.to_owned(),
        host: String::new(),
    };
    let sut = ready(Some(ExploreDoc {
        tabs: vec![
            tab("t-1", Some("https://a.example/")),
            tab("t-2", None),
            tab("t-3", Some("https://c.example/")),
            tab("t-4", Some("https://d.example/")),
        ],
        selected_tab: Some("t-3".to_owned()),
        ..ExploreDoc::default()
    }));
    assert_eq!(
        ids_of(&sut.view().resumable),
        vec!["t-3".to_owned(), "t-1".to_owned(), "t-4".to_owned()]
    );
}

/// A closed tab leaves the rows with its engine; closing them all leaves
/// none, and the home draws no section.
#[test]
fn a_closed_tab_is_no_longer_a_row() {
    let (mut sut, ids) = five_tabs();
    written(sut.dispatch(Event::TabClosed { id: ids[3].clone() }));
    assert_eq!(
        ids_of(&sut.view().resumable),
        vec![ids[4].clone(), ids[2].clone(), ids[1].clone()]
    );
    let all = tabs_closed_by(&sut.view().tabs, &TabCloseScope::All);
    written(sut.dispatch(Event::TabsClosed { ids: all }));
    assert!(sut.view().resumable.is_empty());
}

/// A view written before the rows existed still reads — every shell decodes
/// the view it is sent, and an older one simply has no rows.
#[test]
fn a_view_without_resume_rows_still_reads() {
    let view: ExploreView = serde_json::from_str(
        r#"{"favorites":[],"groups":[],"tabs":[],"selected_tab":null,
            "favorites_hidden":false,"recent_hidden":false,"favorites_full":false,
            "tabs_full":false,"ready":true}"#,
    )
    .expect("an older view reads");
    assert!(view.resumable.is_empty() && view.recent_tabs.is_empty());
}
