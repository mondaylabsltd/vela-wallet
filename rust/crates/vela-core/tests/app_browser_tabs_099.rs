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

// ---------------------------------------------------------------------------
// Where Explore lands, and where an opened site goes (spec 099 navigation)
// ---------------------------------------------------------------------------

mod navigation {
    use vela_core::app::browser_tabs::{
        explore_landing, explore_landing_json, lit_tab, lit_tab_json, open_target,
        open_target_json, waiting_tab, waiting_tab_json, ExploreEntry, ExploreLanding,
        ExploreOpenKind, ExploreOpenTarget,
    };
    use vela_core::app::dapp_browser::{DbrConsentView, DbrSigningView, DbrView};
    use vela_core::app::explore_sites::{ExploreTab, ExploreView, TABS_CAP};

    const UNISWAP: &str = "https://app.uniswap.org/#/swap";
    const AAVE: &str = "https://app.aave.com/";

    /// A strip: `(id, url)` per tab, the selection and the recency order.
    fn strip(
        tabs: &[(&str, Option<&str>)],
        selected: Option<&str>,
        recent: &[&str],
    ) -> ExploreView {
        ExploreView {
            tabs: tabs
                .iter()
                .map(|(id, url)| ExploreTab {
                    id: (*id).to_owned(),
                    url: url.map(str::to_owned),
                    title: String::new(),
                    host: String::new(),
                })
                .collect(),
            selected_tab: selected.map(str::to_owned),
            recent_tabs: recent.iter().map(|id| (*id).to_owned()).collect(),
            ready: true,
            ..ExploreView::default()
        }
    }

    fn tab(id: &str) -> ExploreLanding {
        ExploreLanding::Tab { id: id.to_owned() }
    }

    fn load(id: &str) -> ExploreOpenTarget {
        ExploreOpenTarget::Load { id: id.to_owned() }
    }

    fn resume(id: &str) -> ExploreOpenTarget {
        ExploreOpenTarget::Resume { id: id.to_owned() }
    }

    /// Ported from the desktop (`browser_host.rs`, G2 / spec 082 RD6), the
    /// rule it pinned now the core's: launched with a restored Uniswap tab,
    /// the home shows, the tab waits unlit, and an address entered over the
    /// home opens a NEW tab, leaving the restored one intact. A start-page
    /// tab selected is lit, and it is where the address goes; a page on
    /// screen is lit, and the address bar loads in it.
    #[test]
    fn a_restored_tab_waits_unlit_and_enter_opens_a_new_one() {
        let view = strip(&[("t1", Some(UNISWAP))], Some("t1"), &["t1"]);
        assert_eq!(
            lit_tab(&view, None, false),
            None,
            "nothing lit over the home"
        );
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            ExploreOpenTarget::NewTab,
            "a new tab"
        );

        let view = strip(&[("t1", Some(UNISWAP)), ("t2", None)], Some("t2"), &["t2"]);
        assert_eq!(lit_tab(&view, None, false).as_deref(), Some("t2"));
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t2")
        );

        assert_eq!(lit_tab(&view, Some("t1"), true).as_deref(), Some("t1"));
        assert_eq!(
            open_target(&view, Some("t1"), true, AAVE, ExploreOpenKind::Address),
            load("t1")
        );
    }

    /// Explore chosen from another section, or again while it is up, shows
    /// its home — the dApp in the selected tab is kept, not landed in.
    #[test]
    fn entering_explore_lands_on_its_home_with_the_dapp_kept() {
        let view = strip(&[("t1", Some(UNISWAP))], Some("t1"), &["t1"]);
        assert_eq!(
            explore_landing(&view, ExploreEntry::Section, None),
            ExploreLanding::Home
        );
        assert_eq!(
            explore_landing(&view, ExploreEntry::Reselect, None),
            ExploreLanding::Home
        );
        // An empty strip, or a launch before the mirror: the home.
        assert_eq!(
            explore_landing(&ExploreView::default(), ExploreEntry::Section, None),
            ExploreLanding::Home
        );
    }

    /// A page opened from outside (deep link, scan, external page, launch
    /// URL) lands in its tab — the selected one once the open is in the view.
    /// A selected start-page tab has no page to land in.
    #[test]
    fn a_page_opened_from_outside_lands_in_its_tab() {
        let view = strip(
            &[("t1", Some(UNISWAP)), ("t2", Some(AAVE))],
            Some("t2"),
            &["t2"],
        );
        assert_eq!(
            explore_landing(&view, ExploreEntry::PageOpened, None),
            tab("t2")
        );
        let view = strip(&[("t1", Some(UNISWAP)), ("t2", None)], Some("t2"), &["t2"]);
        assert_eq!(
            explore_landing(&view, ExploreEntry::PageOpened, None),
            ExploreLanding::Home
        );
        assert_eq!(
            explore_landing(&ExploreView::default(), ExploreEntry::PageOpened, None),
            ExploreLanding::Home
        );
    }

    /// A tab with a request waiting on the person is where Explore lands,
    /// whatever brought it up (the desktop's RJ18 signing column comes back
    /// with its tab). A tab the strip lacks, or one with no page, is no
    /// reason to leave the home.
    #[test]
    fn a_request_waiting_on_the_person_brings_its_tab() {
        let view = strip(
            &[("t1", Some(UNISWAP)), ("t2", Some(AAVE)), ("t3", None)],
            Some("t2"),
            &["t2", "t1"],
        );
        for entry in [
            ExploreEntry::Section,
            ExploreEntry::Reselect,
            ExploreEntry::PageOpened,
        ] {
            assert_eq!(
                explore_landing(&view, entry, Some("t1")),
                tab("t1"),
                "{entry:?}"
            );
        }
        assert_eq!(
            explore_landing(&view, ExploreEntry::Section, Some("t-gone")),
            ExploreLanding::Home
        );
        assert_eq!(
            explore_landing(&view, ExploreEntry::Section, Some("t3")),
            ExploreLanding::Home
        );
    }

    /// Opening a site from the home never replaces a live dApp: with the
    /// dApp's tab selected, a picked site and a typed address both get a new
    /// tab; with a start-page tab selected, it gets its first page.
    #[test]
    fn opening_from_the_home_never_replaces_a_live_dapp() {
        let view = strip(&[("t1", Some(UNISWAP))], Some("t1"), &["t1"]);
        for kind in [ExploreOpenKind::Site, ExploreOpenKind::Address] {
            assert_eq!(
                open_target(&view, Some("t1"), false, AAVE, kind),
                ExploreOpenTarget::NewTab,
                "{kind:?}"
            );
        }
        let view = strip(
            &[("t1", Some(UNISWAP)), ("t2", None)],
            Some("t2"),
            &["t2", "t1"],
        );
        for kind in [ExploreOpenKind::Site, ExploreOpenKind::Address] {
            assert_eq!(
                open_target(&view, Some("t1"), false, AAVE, kind),
                load("t2"),
                "{kind:?}"
            );
        }
    }

    /// A site picked from the home that a tab is already on comes back as
    /// that tab, as it was left — not a second copy, and not a reload over
    /// it. The same site is the same origin however the address is spelled
    /// (path, case, default port); another scheme or port is another site.
    #[test]
    fn a_site_already_open_is_resumed_not_opened_twice() {
        let view = strip(
            &[("t1", Some(UNISWAP)), ("t2", None)],
            Some("t2"),
            &["t2", "t1"],
        );
        for same in [
            "https://app.uniswap.org/",
            "https://app.uniswap.org/pools?chain=base",
            "https://APP.Uniswap.org:443/#/swap",
        ] {
            assert_eq!(
                open_target(&view, None, false, same, ExploreOpenKind::Site),
                resume("t1"),
                "{same}"
            );
        }
        for other in [
            "http://app.uniswap.org/",
            "https://app.uniswap.org:8443/",
            "https://uniswap.org/",
        ] {
            assert_eq!(
                open_target(&view, None, false, other, ExploreOpenKind::Site),
                load("t2"),
                "{other}"
            );
        }
    }

    /// Two tabs on the site: the one used most recently; recency unknown
    /// (restored at launch), the first in the strip.
    #[test]
    fn of_two_tabs_on_a_site_the_most_recent_is_resumed() {
        let tabs = [
            ("t1", Some("https://app.uniswap.org/#/pools")),
            ("t2", Some(AAVE)),
            ("t3", Some(UNISWAP)),
        ];
        let view = strip(&tabs, Some("t2"), &["t2", "t3", "t1"]);
        assert_eq!(
            open_target(&view, None, false, UNISWAP, ExploreOpenKind::Site),
            resume("t3")
        );
        let view = strip(&tabs, Some("t2"), &["t2"]);
        assert_eq!(
            open_target(&view, None, false, UNISWAP, ExploreOpenKind::Site),
            resume("t1")
        );
    }

    /// An address names a page: it never switches to a tab already on its
    /// site (the typed path would be dropped) and never loads there (the
    /// live dApp would be replaced).
    #[test]
    fn a_typed_address_never_switches_to_a_same_origin_tab() {
        let view = strip(&[("t1", Some(UNISWAP))], Some("t1"), &["t1"]);
        assert_eq!(
            open_target(
                &view,
                None,
                false,
                "https://app.uniswap.org/pools",
                ExploreOpenKind::Address
            ),
            ExploreOpenTarget::NewTab
        );
    }

    /// The same address handed in again — a launch URL, a deep link, a scan
    /// — is the page a tab already shows: that tab, resumed as it was left,
    /// not one more tab on it (iPhone pass 2026-10-09: nineteen identical
    /// tabs). Exactly as written: another path, query, fragment or a missing
    /// slash is another page, and takes the address rules; the most recently
    /// used of two tabs on it is the one.
    #[test]
    fn an_address_a_tab_already_shows_resumes_that_tab() {
        const PAGE: &str = "http://127.0.0.1:8137/";
        let view = strip(
            &[("t1", Some(PAGE)), ("t2", Some(UNISWAP))],
            Some("t2"),
            &["t2", "t1"],
        );
        assert_eq!(
            open_target(&view, None, false, PAGE, ExploreOpenKind::Address),
            resume("t1")
        );
        // From outside, the shell's own tab in front does not make it "on a page".
        assert_eq!(
            open_target(&view, Some("t2"), false, PAGE, ExploreOpenKind::Address),
            resume("t1")
        );
        for other in [
            "http://127.0.0.1:8137",
            "http://127.0.0.1:8137/?x=1",
            "http://127.0.0.1:8137/#a",
            "http://127.0.0.1:8137/sign",
        ] {
            assert_eq!(
                open_target(&view, None, false, other, ExploreOpenKind::Address),
                ExploreOpenTarget::NewTab,
                "{other}"
            );
        }
        let twice = strip(
            &[
                ("t1", Some(PAGE)),
                ("t2", Some(UNISWAP)),
                ("t3", Some(PAGE)),
            ],
            Some("t2"),
            &["t2", "t3", "t1"],
        );
        assert_eq!(
            open_target(&twice, None, false, PAGE, ExploreOpenKind::Address),
            resume("t3")
        );
        // On a page, its own bar still loads in it.
        assert_eq!(
            open_target(&view, Some("t2"), true, PAGE, ExploreOpenKind::Address),
            load("t2")
        );
        assert_eq!(
            open_target_json(
                &serde_json::to_string(&view).expect("the view encodes"),
                None,
                false,
                PAGE,
                "address"
            )
            .as_deref(),
            Some(r#"{"type":"resume","id":"t1"}"#)
        );
    }

    /// On a page, the address bar loads in that page, whatever was asked —
    /// the person is in it and typed over its address.
    #[test]
    fn on_a_page_an_open_loads_in_that_page() {
        let view = strip(
            &[("t1", Some(UNISWAP)), ("t2", Some(AAVE))],
            Some("t2"),
            &["t2", "t1"],
        );
        for kind in [ExploreOpenKind::Site, ExploreOpenKind::Address] {
            assert_eq!(
                open_target(&view, Some("t2"), true, UNISWAP, kind),
                load("t2"),
                "{kind:?}"
            );
        }
        // "On a page" with no page shown is the home: a new tab for a page
        // no tab is on, and the tab already on it for one that is.
        assert_eq!(
            open_target(
                &view,
                None,
                true,
                "https://app.uniswap.org/pools",
                ExploreOpenKind::Address
            ),
            ExploreOpenTarget::NewTab
        );
        assert_eq!(
            open_target(&view, None, true, UNISWAP, ExploreOpenKind::Address),
            resume("t1")
        );
    }

    /// A full strip (TABS_CAP) has no room for a new tab — the explore
    /// machine drops that `tab_opened`, so "new tab" would be an open that
    /// silently does nothing. An open tab takes the address instead, and
    /// never the dApp the person just left (the selected tab) while another
    /// will do: a start-page tab first, the one used most recently of them;
    /// else the tab used longest ago. The other rules still come first: a
    /// picked site already open is resumed, a selected start-page tab gets
    /// its first page, and on a page the address loads there. One short of
    /// the cap is a new tab.
    #[test]
    fn a_full_strip_never_gives_up_the_dapp_just_left() {
        let ids: Vec<String> = (1..=TABS_CAP).map(|i| format!("t{i}")).collect();
        // Every tab on a page but `start_pages`; t1 on Uniswap. Recency:
        // `recent` as given, the rest in strip order after it.
        let full = |selected: &str, start_pages: &[&str], recent: &[&str]| {
            let tabs: Vec<(&str, Option<&str>)> = ids
                .iter()
                .map(|id| {
                    let url = if start_pages.contains(&id.as_str()) {
                        None
                    } else if id == "t1" {
                        Some(UNISWAP)
                    } else {
                        Some("https://example.org/")
                    };
                    (id.as_str(), url)
                })
                .collect();
            strip(&tabs, Some(selected), recent)
        };

        // The dApp just left is t7, and every tab is known: t24 was used
        // longest ago.
        let mut recent: Vec<&str> = vec!["t7", "t3"];
        recent.extend(
            ids.iter()
                .map(String::as_str)
                .filter(|id| !["t7", "t3", "t24"].contains(id)),
        );
        recent.push("t24");
        let view = full("t7", &[], &recent);
        for kind in [ExploreOpenKind::Site, ExploreOpenKind::Address] {
            assert_eq!(
                open_target(&view, None, false, AAVE, kind),
                load("t24"),
                "{kind:?}: the tab used longest ago, never the dApp just left"
            );
            assert_eq!(
                open_target(&view, Some("t3"), false, AAVE, kind),
                load("t24"),
                "{kind:?}: over the home, the tab last shown is no reason"
            );
        }
        // Recency that knows only the selected tab (a launch): the others
        // follow in strip order, so the last of the strip is the oldest.
        let view = full("t7", &[], &["t7"]);
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t24")
        );
        // The tab used longest ago is the selected one: the next oldest.
        let view = full("t24", &[], &["t1", "t2"]);
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t23")
        );
        // A start-page tab has nothing to lose: it goes first, the one used
        // most recently of several.
        let view = full("t7", &["t5", "t12"], &["t7", "t12", "t5"]);
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t12")
        );
        let view = full("t7", &["t5", "t12"], &["t7"]);
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t5"),
            "recency unknown: strip order"
        );
        // Rule 2 before rule 5: a picked site already open comes back.
        let view = full("t7", &[], &["t7"]);
        assert_eq!(
            open_target(&view, None, false, UNISWAP, ExploreOpenKind::Site),
            resume("t1")
        );
        // Rule 3 before rule 5: a selected start-page tab gets the page.
        let view = full("t9", &["t9", "t5"], &["t5"]);
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t9")
        );
        // Rule 1: on a page, that page.
        let view = full("t7", &[], &["t7"]);
        assert_eq!(
            open_target(&view, Some("t3"), true, AAVE, ExploreOpenKind::Address),
            load("t3")
        );
        // A selection that names no tab (a shell's own copy): the tab used
        // most recently stands in for the one in front; the oldest goes.
        let mut stale = full("t7", &[], &["t5", "t24"]);
        stale.selected_tab = Some("gone".to_owned());
        assert_eq!(
            open_target(&stale, None, false, AAVE, ExploreOpenKind::Address),
            load("t23"),
            "t24 was used, so the strip's last unknown tab is the oldest"
        );
        // One short of the cap there is still room: a new tab.
        let mut room = full("t7", &[], &["t7"]);
        room.tabs.pop();
        assert_eq!(
            open_target(&room, None, false, AAVE, ExploreOpenKind::Address),
            ExploreOpenTarget::NewTab
        );
        // The same answer over the wire.
        let json = serde_json::to_string(&full("t7", &[], &["t7"])).expect("a view serializes");
        assert_eq!(
            open_target_json(&json, None, false, AAVE, "address").as_deref(),
            Some(r#"{"type":"load","id":"t24"}"#)
        );
    }

    /// The selected tab is given up only when it is the only tab there is —
    /// a shell's copy of a strip the machine never publishes (one tab, yet
    /// full), the one way rule 5 can have no other.
    #[test]
    fn a_full_strip_of_one_tab_gives_up_the_selected_one() {
        let mut view = strip(&[("t1", Some(UNISWAP))], Some("t1"), &["t1"]);
        // Duplicated past the cap under one id: there is no other tab.
        view.tabs = std::iter::repeat_n(view.tabs[0].clone(), TABS_CAP).collect();
        assert_eq!(
            open_target(&view, None, false, AAVE, ExploreOpenKind::Address),
            load("t1")
        );
    }

    /// The consent, then the signature, then the add-network sheet: the tab
    /// in front of the person. Nothing open, nothing waiting.
    #[test]
    fn the_waiting_tab_is_the_sheet_in_front_of_the_person() {
        let mut dapp = DbrView::default();
        assert_eq!(waiting_tab(&dapp), None);
        dapp.adding_network = Some(DbrSigningView {
            tab: "t3".to_owned(),
            id: "1".to_owned(),
        });
        assert_eq!(waiting_tab(&dapp).as_deref(), Some("t3"));
        dapp.signing = Some(DbrSigningView {
            tab: "t2".to_owned(),
            id: "2".to_owned(),
        });
        assert_eq!(waiting_tab(&dapp).as_deref(), Some("t2"));
        dapp.consent = Some(DbrConsentView {
            tab: "t1".to_owned(),
            origin: "https://app.uniswap.org".to_owned(),
            methods: vec!["eth_requestAccounts".to_owned()],
            address: None,
            chain_id: 1,
        });
        assert_eq!(waiting_tab(&dapp).as_deref(), Some("t1"));
        // The same answer over JSON, from the whole view.
        let json = serde_json::to_string(&dapp).expect("a view serializes");
        assert_eq!(waiting_tab_json(&json).as_deref(), Some("t1"));
    }

    /// The wire the phones speak: an `ExploreView` JSON — the whole view, or
    /// the shell's own re-encoded copy with only the strip — bare words for
    /// the two questions, tagged JSON back. Input that does not read is
    /// `None`, never a guess.
    #[test]
    fn the_json_wrappers_speak_the_wire() {
        let view = strip(&[("t1", Some(UNISWAP)), ("t2", None)], Some("t1"), &["t1"]);
        let whole = serde_json::to_string(&view).expect("a view serializes");
        let partial = format!(
            r#"{{"tabs":[{{"id":"t1","url":"{UNISWAP}","title":"","host":""}},{{"id":"t2","title":"","host":""}}],"selected_tab":"t1"}}"#
        );
        for json in [&whole, &partial] {
            assert_eq!(
                explore_landing_json(json, "section", None).as_deref(),
                Some(r#"{"type":"home"}"#)
            );
            assert_eq!(
                explore_landing_json(json, "page_opened", None).as_deref(),
                Some(r#"{"type":"tab","id":"t1"}"#)
            );
            assert_eq!(
                explore_landing_json(json, "reselect", Some("t1")).as_deref(),
                Some(r#"{"type":"tab","id":"t1"}"#)
            );
            assert_eq!(
                open_target_json(json, Some("t1"), false, AAVE, "site").as_deref(),
                Some(r#"{"type":"new_tab"}"#)
            );
            assert_eq!(
                open_target_json(json, None, false, UNISWAP, "site").as_deref(),
                Some(r#"{"type":"resume","id":"t1"}"#)
            );
            assert_eq!(
                open_target_json(json, Some("t1"), true, AAVE, "address").as_deref(),
                Some(r#"{"type":"load","id":"t1"}"#)
            );
            assert_eq!(lit_tab_json(json, Some("t1"), true).as_deref(), Some("t1"));
            assert_eq!(lit_tab_json(json, None, false), None);
        }
        assert_eq!(explore_landing_json(&whole, "browsing", None), None);
        assert_eq!(explore_landing_json(&whole, "\"section\"", None), None);
        assert_eq!(open_target_json(&whole, None, false, AAVE, "typed"), None);
        assert_eq!(explore_landing_json("not json", "section", None), None);
        assert_eq!(
            open_target_json(r#"{"tabs":"t1"}"#, None, false, AAVE, "site"),
            None
        );
        assert_eq!(waiting_tab_json("not json"), None);
        assert_eq!(
            waiting_tab_json(r#"{"signing":{"tab":"t2","id":"9"}}"#).as_deref(),
            Some("t2")
        );
    }
}
