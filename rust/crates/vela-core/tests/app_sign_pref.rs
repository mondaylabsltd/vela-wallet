//! Rules of the stored Trusted Signer page (spec 071), one test per rule. The
//! machine's job is refusing to invent a preference, and refusing to store a
//! page a browser could not sign on.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::sign_pref::{
    Event, SignPref, SignPrefOperation as Op, SignPrefShellResult as Res,
};
use vela_core::trusted_signer::DEFAULT_SIGNER_URL;

type Sut = DomainDriver<SignPref>;

fn stored(url: Option<&str>) -> Res {
    Res::Stored {
        signer_url: url.map(str::to_owned),
    }
}

fn loaded(url: Option<&str>) -> Sut {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::Refresh), vec![Op::ReadStored]);
    sut.resolve(stored(url));
    sut
}

#[test]
fn nothing_stored_is_the_official_page() {
    let view = loaded(None).view();
    assert_eq!(view.signer_url, DEFAULT_SIGNER_URL);
    assert!(view.signer_url_is_default);
    assert!(view.signer_uses_wallet_passkeys);
    assert_eq!(view.signer_url_error, None);
}

#[test]
fn a_stored_page_is_read_back() {
    let view = loaded(Some("https://sign.example/cs/")).view();
    assert_eq!(view.signer_url, "https://sign.example/cs/");
    assert!(!view.signer_url_is_default);
    assert!(
        !view.signer_uses_wallet_passkeys,
        "a page elsewhere cannot use getvela.app passkeys"
    );
}

#[test]
fn a_page_this_build_does_not_accept_reads_as_unset_and_is_not_rewritten() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    let ops = sut.resolve(stored(Some("http://192.168.1.4/")));
    assert!(ops.is_empty(), "nothing is written back: {ops:?}");
    assert_eq!(sut.view().signer_url, DEFAULT_SIGNER_URL);
}

/// Founder, 2026-09-26: how a signature is routed is the account's sign-in,
/// not a stored default. A shell that still reports the old `vela.signMethod`
/// beside the page is read for the page alone.
#[test]
fn a_stored_default_method_from_before_is_ignored() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    let old: Res = serde_json::from_value(serde_json::json!({
        "type": "stored",
        "method": "security_key",
        "signer_url": "https://sign.example/",
    }))
    .unwrap_or_else(|_| unreachable!());
    assert!(sut.resolve(old).is_empty());
    assert_eq!(sut.view().signer_url, "https://sign.example/");
}

#[test]
fn a_usable_page_is_normalised_and_stored() {
    let mut sut = loaded(None);
    let ops = sut.dispatch(Event::SignerUrlSubmitted {
        text: " localhost:8140 ".into(),
    });
    // A bare host is https; loopback over https is still loopback.
    assert_eq!(
        ops,
        vec![Op::WriteSignerUrl {
            url: Some("https://localhost:8140/".into())
        }]
    );
    let ops = sut.dispatch(Event::SignerUrlSubmitted {
        text: "http://127.0.0.1:8140".into(),
    });
    assert_eq!(
        ops,
        vec![Op::WriteSignerUrl {
            url: Some("http://127.0.0.1:8140/".into())
        }]
    );
    assert_eq!(sut.view().signer_url, "http://127.0.0.1:8140/");
}

#[test]
fn a_page_a_browser_could_not_sign_on_is_refused_and_the_old_one_stands() {
    let mut sut = loaded(Some("https://sign.example/"));
    assert!(sut
        .dispatch(Event::SignerUrlSubmitted {
            text: "http://sign.example/".into()
        })
        .is_empty());
    let view = sut.view();
    assert_eq!(view.signer_url_error.as_deref(), Some("insecure"));
    assert_eq!(view.signer_url, "https://sign.example/");
    assert!(sut
        .dispatch(Event::SignerUrlSubmitted {
            text: "not a url at all".into()
        })
        .is_empty());
    assert_eq!(sut.view().signer_url_error.as_deref(), Some("invalid"));
    // The next good submit clears the complaint.
    sut.dispatch(Event::SignerUrlSubmitted {
        text: "https://other.example".into(),
    });
    assert_eq!(sut.view().signer_url_error, None);
}

#[test]
fn the_official_page_typed_back_in_and_a_reset_both_mean_the_default() {
    let mut sut = loaded(Some("https://sign.example/"));
    assert_eq!(
        sut.dispatch(Event::SignerUrlSubmitted {
            text: DEFAULT_SIGNER_URL.into()
        }),
        vec![Op::WriteSignerUrl { url: None }]
    );
    assert!(sut.view().signer_url_is_default);
    let mut sut = loaded(Some("https://sign.example/"));
    assert_eq!(
        sut.dispatch(Event::SignerUrlReset),
        vec![Op::WriteSignerUrl { url: None }]
    );
    assert!(sut.view().signer_url_is_default);
}

#[test]
fn a_read_that_lands_after_a_choice_is_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.dispatch(Event::SignerUrlSubmitted {
        text: "https://mine.example".into(),
    });
    // The read's answer arrives after the choice — the person's choice wins.
    sut.resolve_matching(
        |op| matches!(op, Op::ReadStored),
        stored(Some("https://sign.example/")),
    );
    assert_eq!(sut.view().signer_url, "https://mine.example/");
}
