//! Rules of Settings → Signing pages (spec 102), one test per rule. The
//! machine keeps a list of pages a person trusts; the official page is always
//! first; nothing a browser could not sign on is stored; and 071's single
//! "Trusted Signer page" is imported once.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::signing_pages::{
    Event, SigningPages, SigningPagesOperation as Op, SigningPagesShellResult as Res,
};
use vela_core::signing_venue::SigningPage;
use vela_core::trusted_signer::DEFAULT_SIGNER_URL;

type Sut = DomainDriver<SigningPages>;

fn page(url: &str, name: &str) -> SigningPage {
    SigningPage::new(url.to_owned(), name.to_owned())
}

fn stored(pages: Option<&str>, legacy: Option<&str>) -> Res {
    Res::Stored {
        pages_json: pages.map(str::to_owned),
        legacy_url: legacy.map(str::to_owned),
    }
}

fn loaded(pages: Option<&str>) -> Sut {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::Refresh), vec![Op::ReadStored]);
    assert!(sut.resolve(stored(pages, None)).is_empty());
    sut
}

#[test]
fn nothing_stored_is_the_official_page_alone() {
    let view = loaded(None).view();
    assert!(view.loaded);
    assert_eq!(view.pages.len(), 1);
    assert_eq!(view.pages[0].url, DEFAULT_SIGNER_URL);
    assert!(view.pages[0].official);
    assert_eq!(view.pages[0].domain, "getvela.app");
    assert!(view.saved.is_empty());
}

/// Each row says which domain's keys the page can use (R1), so a person sees
/// before choosing it which accounts it can sign for.
#[test]
fn stored_pages_come_back_after_the_official_one_with_their_domains() {
    let view = loaded(Some(
        r#"[{"url":"https://sign.example.com/","name":"Mine"},{"url":"http://localhost:8140/"}]"#,
    ))
    .view();
    let rows: Vec<(&str, &str, &str, bool)> = view
        .pages
        .iter()
        .map(|row| {
            (
                row.url.as_str(),
                row.name.as_str(),
                row.domain.as_str(),
                row.official,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            (DEFAULT_SIGNER_URL, "", "getvela.app", true),
            (
                "https://sign.example.com/",
                "Mine",
                "sign.example.com",
                false
            ),
            ("http://localhost:8140/", "", "localhost", false),
        ]
    );
    assert_eq!(view.saved.len(), 2);
}

/// What this build would not store is not offered either: an insecure page,
/// the official page again, a repeat, something that is not a list.
#[test]
fn a_stored_entry_this_build_would_refuse_is_dropped_on_read() {
    let view = loaded(Some(
        r#"[{"url":"http://192.168.1.4/"},{"url":"https://sign.getvela.app"},
            {"url":"https://Sign.Example.com"},{"url":"https://sign.example.com/"},42]"#,
    ))
    .view();
    assert_eq!(view.saved, vec![page("https://sign.example.com/", "")]);
    assert_eq!(loaded(Some("not json")).view().saved, vec![]);
}

#[test]
fn a_usable_page_is_normalised_named_and_stored() {
    let mut sut = loaded(None);
    let ops = sut.dispatch(Event::PageAdded {
        url: " localhost:8140 ".into(),
        name: "  Home  ".into(),
    });
    assert_eq!(
        ops,
        vec![Op::WritePages {
            pages: vec![page("https://localhost:8140/", "Home")],
            remove_legacy_url: false,
        }]
    );
    assert_eq!(sut.view().add_error, None);
}

/// Nothing a browser would not sign on is stored, and the list stands.
#[test]
fn an_unusable_or_repeated_page_is_refused_and_nothing_is_stored() {
    let mut sut = loaded(Some(r#"[{"url":"https://sign.example.com/"}]"#));
    for (url, why) in [
        ("http://192.168.1.4/", "insecure"),
        ("not a url", "invalid"),
        ("https://sign.example.com", "duplicate"),
        ("sign.getvela.app", "duplicate"),
    ] {
        let ops = sut.dispatch(Event::PageAdded {
            url: url.into(),
            name: String::new(),
        });
        assert!(ops.is_empty(), "{url}: {ops:?}");
        assert_eq!(sut.view().add_error.as_deref(), Some(why), "{url}");
    }
    assert_eq!(sut.view().saved.len(), 1);
}

#[test]
fn a_saved_page_is_renamed_and_removed_but_never_the_official_one() {
    let mut sut = loaded(Some(r#"[{"url":"https://sign.example.com/","name":"A"}]"#));
    assert_eq!(
        sut.dispatch(Event::PageRenamed {
            url: "https://SIGN.example.com".into(),
            name: "B".into(),
        }),
        vec![Op::WritePages {
            pages: vec![page("https://sign.example.com/", "B")],
            remove_legacy_url: false,
        }]
    );
    sut.resolve(Res::Written);
    // The official page is not in the list to rename or remove.
    assert!(sut
        .dispatch(Event::PageRenamed {
            url: DEFAULT_SIGNER_URL.into(),
            name: "Mine".into(),
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::PageRemoved {
            url: DEFAULT_SIGNER_URL.into(),
        })
        .is_empty());
    assert_eq!(
        sut.dispatch(Event::PageRemoved {
            url: "https://sign.example.com/".into(),
        }),
        vec![Op::WritePages {
            pages: vec![],
            remove_legacy_url: false,
        }]
    );
    assert_eq!(sut.view().pages.len(), 1);
}

/// Edits before the list is read would replace pages they never saw.
#[test]
fn nothing_is_written_before_the_list_is_read() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::PageAdded {
            url: "https://sign.example.com".into(),
            name: String::new(),
        })
        .is_empty());
    assert!(!sut.view().loaded);
    sut.dispatch(Event::Refresh);
    assert!(sut
        .dispatch(Event::PageRemoved {
            url: "https://sign.example.com".into(),
        })
        .is_empty());
}

/// 071's one page is imported once: added when it is a usable page not yet
/// here, and the old key removed either way — so a page the person later
/// removes does not come back from it.
#[test]
fn the_071_page_is_imported_once_and_its_key_removed() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    let ops = sut.resolve(stored(None, Some("https://sign.example.com/cs/")));
    assert_eq!(
        ops,
        vec![Op::WritePages {
            pages: vec![page("https://sign.example.com/cs/", "")],
            remove_legacy_url: true,
        }]
    );
    // The official page stored there is not a page to import — but the key
    // still goes.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    assert_eq!(
        sut.resolve(stored(
            Some(r#"[{"url":"https://a.example/"}]"#),
            Some(DEFAULT_SIGNER_URL)
        )),
        vec![Op::WritePages {
            pages: vec![page("https://a.example/", "")],
            remove_legacy_url: true,
        }]
    );
    // Already saved: not added twice.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    assert_eq!(
        sut.resolve(stored(
            Some(r#"[{"url":"https://a.example/","name":"A"}]"#),
            Some("a.example")
        )),
        vec![Op::WritePages {
            pages: vec![page("https://a.example/", "A")],
            remove_legacy_url: true,
        }]
    );
}

/// A late answer to a superseded read changes nothing.
#[test]
fn a_superseded_read_is_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None, None));
    sut.dispatch(Event::PageAdded {
        url: "https://a.example".into(),
        name: String::new(),
    });
    // Two writes later, the first write's acknowledgement arrives — inert.
    sut.dispatch(Event::PageAdded {
        url: "https://b.example".into(),
        name: String::new(),
    });
    assert!(sut.resolve(Res::Written).is_empty());
    assert_eq!(sut.view().saved.len(), 2);
}

/// The wire names the shells write and read.
#[test]
fn the_wire_is_stable() {
    let op = serde_json::to_value(Op::WritePages {
        pages: vec![page("https://a.example/", "A")],
        remove_legacy_url: false,
    })
    .unwrap_or_default();
    assert_eq!(
        op,
        serde_json::json!({
            "type": "write_pages",
            "pages": [{ "url": "https://a.example/", "name": "A" }],
        })
    );
    let res: Res = serde_json::from_value(serde_json::json!({ "type": "stored" }))
        .unwrap_or_else(|_| unreachable!());
    assert_eq!(res, stored(None, None));
}
