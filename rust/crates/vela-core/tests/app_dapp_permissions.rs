//! Rules of the dApp grant store and the request window's entries into it —
//! one test per rule (inventory `### dapp_permissions (P2)`, invariants ②, ③,
//! ⑤, ⑨, narrowed by spec 070 T063).
//!
//! The in-app browser's consent flow is NOT tested here any more: spec 070 moved
//! every browser onto `dapp_browser`, which owns those rules and their tests
//! (`app_dapp_browser.rs`). What is left is the grant store, the window's three
//! entries, and the pure helpers other machines import from this one.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::dapp_permissions::{
    decide_popup_request, is_connect_method, is_insecure_public_origin, is_signing_method,
    origin_of, settle_on_close, DappPermissions, DpermGrant, DpermOperation as Op,
    DpermPopupDecision, DpermPopupOutcome, DpermPopupView, DpermRejectReason as Reason,
    DpermRespondPayload as Payload, DpermShellResult as Res, Event,
};

type Sut = DomainDriver<DappPermissions>;

const T0: f64 = 1_754_700_000_000.0;
const ORIGIN: &str = "https://dapp.example";
const A1: &str = "0x1111111111111111111111111111111111111111";
const A2: &str = "0x2222222222222222222222222222222222222222";
const A3: &str = "0x3333333333333333333333333333333333333333";

fn grant(address: &str) -> DpermGrant {
    DpermGrant {
        origin: ORIGIN.to_owned(),
        address: address.to_owned(),
        chain_id: 8453,
        granted_at_ms: T0,
    }
}

fn accounts(id: &str, addresses: &[&str]) -> Op {
    Op::Respond {
        id: id.to_owned(),
        payload: Payload::Accounts {
            addresses: addresses.iter().map(|a| (*a).to_owned()).collect(),
        },
    }
}

// ---------------------------------------------------------------------------
// The request window's approve (spec 070 T063)
// ---------------------------------------------------------------------------

/// The window authors the same connection the browser path used to, without
/// pretending to be a browser.
///
/// It used to get here by dispatching `provider_request`, answering the grant
/// read, checking a consent sheet had opened for its origin and only then
/// approving — five events through a model with no tab, no document and no
/// navigation in it. The three operations are what the window actually needs:
/// the grant, the row that gives the person a trail, and the answer.
#[test]
fn the_popup_authors_a_connection_in_one_event() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::PopupApproved {
        origin: ORIGIN.to_owned(),
        request_id: "r1".to_owned(),
        method: "eth_requestAccounts".to_owned(),
        address: A1.to_owned(),
        chain_id: 1,
        now_ms: T0 + 1_000.0,
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteGrant {
                grant: DpermGrant {
                    origin: ORIGIN.to_owned(),
                    address: A1.to_owned(),
                    chain_id: 1,
                    granted_at_ms: T0 + 1_000.0,
                },
            },
            Op::SaveConnectionRecord {
                address: A1.to_owned(),
                chain_id: 1,
                origin: ORIGIN.to_owned(),
            },
            accounts("r1", &[A1]),
        ],
        "the grant, the audit row, the answer — and no page events, because \
         this window has no document to push them into"
    );
}

/// `wallet_requestPermissions` is answered in its own shape — the one thing
/// about the window's answer that is not the address.
#[test]
fn the_popup_answers_a_permissions_request_in_its_own_shape() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::PopupApproved {
        origin: ORIGIN.to_owned(),
        request_id: "r9".to_owned(),
        method: "wallet_requestPermissions".to_owned(),
        address: A1.to_owned(),
        chain_id: 100,
        now_ms: T0,
    });
    assert!(
        matches!(
            ops.last(),
            Some(Op::Respond {
                payload: Payload::Permissions { granted: true },
                ..
            })
        ),
        "permissions, not an address list: {ops:?}"
    );
}

/// Nothing is authored without the two facts the grant is made of. A window
/// that lost its origin or its account writes no grant rather than a grant
/// nobody can read back.
#[test]
fn the_popup_authors_nothing_without_an_origin_and_an_account() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::PopupApproved {
            origin: String::new(),
            request_id: "r1".to_owned(),
            method: "eth_requestAccounts".to_owned(),
            address: A1.to_owned(),
            chain_id: 1,
            now_ms: T0,
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::PopupApproved {
            origin: ORIGIN.to_owned(),
            request_id: "r1".to_owned(),
            method: "eth_requestAccounts".to_owned(),
            address: String::new(),
            chain_id: 1,
            now_ms: T0,
        })
        .is_empty());
}

// ---------------------------------------------------------------------------
// The wallet switched account — what one connected site hears (070 T063)
// ---------------------------------------------------------------------------

/// A grant whose account the wallet still holds follows the active account —
/// and keeps the chain it was made on, which is an audit fact, not a preference.
#[test]
fn an_account_switch_repins_a_connected_site_without_rewriting_its_chain() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::PopupAccountSwitch {
        origin: ORIGIN.to_owned(),
        grant: Some(grant(A1)),
        current_addresses: Some(vec![A1.to_owned(), A2.to_owned()]),
        active_address: A2.to_owned(),
        now_ms: T0 + 2_000.0,
    });
    assert_eq!(
        ops,
        vec![Op::WriteGrant {
            grant: DpermGrant {
                origin: ORIGIN.to_owned(),
                address: A2.to_owned(),
                chain_id: 8453,
                granted_at_ms: T0 + 2_000.0,
            },
        }],
        "re-pinned to the new address, on the chain the site connected on"
    );
    // The shell's ack changes nothing and asks for nothing.
    assert!(sut.resolve(Res::Ack).is_empty());
}

/// Invariant ⑨'s other half: a grant whose own account was DELETED from the
/// wallet is physically removed, not silently re-pinned to whoever is active.
#[test]
fn an_account_switch_removes_a_grant_whose_account_left_the_wallet() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::PopupAccountSwitch {
        origin: ORIGIN.to_owned(),
        grant: Some(grant(A3)), // A3 is not in [A1, A2]
        current_addresses: Some(vec![A1.to_owned(), A2.to_owned()]),
        active_address: A1.to_owned(),
        now_ms: T0 + 2_000.0,
    });
    assert_eq!(
        ops,
        vec![Op::RemoveGrant {
            origin: ORIGIN.to_owned(),
        }],
        "removed, never re-pinned to an account the site never authorized"
    );
}

fn switch(
    grant: Option<DpermGrant>,
    addresses: Option<Vec<String>>,
    active: &str,
    origin: &str,
) -> Event {
    Event::PopupAccountSwitch {
        origin: origin.to_owned(),
        grant,
        current_addresses: addresses,
        active_address: active.to_owned(),
        now_ms: T0 + 2_000.0,
    }
}

/// Invariant ②: a transient empty account list is NOT evidence that the
/// granted account is gone. A switch on a cold read re-pins the site to the
/// account the wallet says is active — it never removes the grant, which would
/// log every open dApp out on app launch.
#[test]
fn an_account_switch_on_a_cold_read_never_removes_a_grant() {
    for addresses in [None, Some(Vec::new())] {
        let mut sut = Sut::new();
        let ops = sut.dispatch(switch(Some(grant(A3)), addresses, A1, ORIGIN));
        assert_eq!(
            ops,
            vec![Op::WriteGrant {
                grant: DpermGrant {
                    origin: ORIGIN.to_owned(),
                    address: A1.to_owned(),
                    chain_id: 8453,
                    granted_at_ms: T0 + 2_000.0,
                },
            }],
            "a cold read re-pins; it must never revoke (invariant ②)"
        );
    }
}

/// Everything the switch must NOT write.
#[test]
fn an_account_switch_writes_nothing_it_cannot_justify() {
    let mut sut = Sut::new();
    // The same address in another casing is not a change.
    let upper = A1.to_uppercase().replace("0X", "0x");
    assert!(sut
        .dispatch(switch(
            Some(grant(&upper)),
            Some(vec![A1.to_owned()]),
            A1,
            ORIGIN
        ))
        .is_empty());
    // A site that never connected, and the two facts a write is made of.
    assert!(sut
        .dispatch(switch(None, Some(vec![A1.to_owned()]), A1, ORIGIN))
        .is_empty());
    assert!(sut
        .dispatch(switch(Some(grant(A1)), Some(vec![A2.to_owned()]), A2, ""))
        .is_empty());
    assert!(sut
        .dispatch(switch(
            Some(grant(A1)),
            Some(vec![A2.to_owned()]),
            "",
            ORIGIN
        ))
        .is_empty());
    assert!(sut.outstanding().is_empty());
}

// ---------------------------------------------------------------------------
// ⑤ — a window that goes away settles 4900, never 4001
// ---------------------------------------------------------------------------

/// The code is the whole point: a dApp reads 4001 as "the user said no, nothing
/// happened" and re-sends, double-spending a request that may already have
/// landed. A window closing is not a person saying no.
#[test]
fn a_window_that_goes_away_settles_unknown_pending() {
    let (code, reason) = settle_on_close();
    assert_eq!(code, 4900);
    assert_ne!(code, 4001, "a dApp retries 4001 — double-spend risk");
    assert_eq!(reason, Reason::BrowserClosed);
    assert_eq!(reason.code(), code, "the reason and the code cannot drift");
}

// ---------------------------------------------------------------------------
// ③ — an insecure public origin, with fully-anchored IP exemptions
// ---------------------------------------------------------------------------

#[test]
fn insecure_origin_classification_table() {
    // insecure
    for origin in [
        "http://dapp.example",
        "http://10.0.0.1.evil.com", // a registrable public FQDN, not a private IP
        "http://999.1.1.1",         // not a valid quad → hostname → public
        "http://172.32.0.1",        // just past the 172.16–31 private block
        "not a url",
        "",
    ] {
        assert!(is_insecure_public_origin(origin), "{origin}");
    }
    // exempt / secure
    for origin in [
        "https://dapp.example",
        "http://localhost",
        "http://dev.local",
        "http://127.0.0.1",
        "http://10.0.0.1",
        "http://192.168.0.10",
        "http://172.31.255.255",
        "http://169.254.1.1",
        "http://[::1]",
        "http://[fd12::1]",
        "http://[fe80::2]",
    ] {
        assert!(!is_insecure_public_origin(origin), "{origin}");
    }
}

// ---------------------------------------------------------------------------
// The request window's decision (`web-request.tsx:169-193`)
// ---------------------------------------------------------------------------

#[test]
fn popup_connect_paths() {
    let granted = vec![A1.to_owned()];
    assert_eq!(
        decide_popup_request("eth_requestAccounts", &granted, None),
        DpermPopupDecision::Respond(Payload::Accounts {
            addresses: vec![A1.to_owned()],
        }),
    );
    assert_eq!(
        decide_popup_request("wallet_requestPermissions", &granted, None),
        DpermPopupDecision::Respond(Payload::Permissions { granted: true }),
    );
    assert_eq!(
        decide_popup_request("eth_requestAccounts", &[], None),
        DpermPopupDecision::Consent,
    );
}

/// The window REFUSES non-connect requests from a never-connected origin
/// (4100) — it does not forward reads the way a browser does. The difference
/// is a rule, and it is explicit.
#[test]
fn popup_requires_connection_first() {
    assert_eq!(
        decide_popup_request("personal_sign", &[], None),
        DpermPopupDecision::Reject(Reason::NotConnected),
    );
    assert_eq!(Reason::NotConnected.code(), 4100);
}

#[test]
fn popup_pinned_address_must_match_the_grant() {
    let granted = vec![A1.to_owned()];
    assert_eq!(
        decide_popup_request("personal_sign", &granted, Some(A2)),
        DpermPopupDecision::Reject(Reason::StaleAuthorizedAddress),
    );
    // Case-insensitive match, and no pin at all, both forward.
    let upper = A1.to_uppercase().replace("0X", "0x");
    assert_eq!(
        decide_popup_request("personal_sign", &granted, Some(&upper)),
        DpermPopupDecision::ForwardToSigning,
    );
    assert_eq!(
        decide_popup_request("eth_sendTransaction", &granted, None),
        DpermPopupDecision::ForwardToSigning,
    );
}

// ---------------------------------------------------------------------------
// The window's question, as a DISPATCHABLE decision (`Event::PopupRequest`)
//
// The rules above are the pure function's; these are the same rules reached
// the only way a shell can reach them. Every one of them is a fund-safety
// rule, so the projection has to be asserted, not assumed.
// ---------------------------------------------------------------------------

fn popup(
    method: &str,
    grant: Option<DpermGrant>,
    addresses: Option<&[&str]>,
    pinned: Option<&str>,
) -> Event {
    Event::PopupRequest {
        method: method.to_owned(),
        grant,
        current_addresses: addresses.map(|a| a.iter().map(|s| (*s).to_owned()).collect()),
        pinned_address: pinned.map(str::to_owned),
    }
}

/// Ask the verdict and read it back. Every popup question must be answered
/// with NO shell operation — the window owns its own grant I/O and its own
/// surface; this core is asked the question and nothing else.
fn ask(sut: &mut Sut, event: Event) -> DpermPopupView {
    let ops = sut.dispatch(event);
    assert!(
        ops.is_empty(),
        "a popup question asks the shell for nothing"
    );
    sut.view().popup.expect("a popup verdict")
}

/// A never-connected origin gets NO address — 4100, never a forward.
#[test]
fn popup_event_refuses_an_unconnected_origin() {
    let mut sut = Sut::new();
    let verdict = ask(&mut sut, popup("personal_sign", None, Some(&[A1]), None));
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::NotConnected,
        },
    );
    assert!(verdict.granted.is_empty());

    // …and the connect methods do not leak one either: they ask the user.
    let verdict = ask(
        &mut sut,
        popup("eth_requestAccounts", None, Some(&[A1]), None),
    );
    assert_eq!(verdict.outcome, DpermPopupOutcome::Consent);
}

/// The forward is pinned to the GRANT's address, never the wallet's active
/// account (invariant ⑨). A2 is first in the wallet here; A1 is the grant.
#[test]
fn popup_event_forwards_the_granted_address_not_the_active_account() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup(
            "eth_sendTransaction",
            Some(grant(A1)),
            Some(&[A2, A1]),
            None,
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A1.to_owned(),
        },
    );

    // A grant whose account was deleted from the wallet exposes nothing.
    let verdict = ask(
        &mut sut,
        popup(
            "eth_sendTransaction",
            Some(grant(A3)),
            Some(&[A1, A2]),
            None,
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::NotConnected,
        },
    );

    // Cold read (addresses not known yet): the grant is TRUSTED, not revoked
    // (invariant ②) — a transient empty list must not log the origin out.
    let verdict = ask(
        &mut sut,
        popup("eth_sendTransaction", Some(grant(A1)), None, None),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A1.to_owned(),
        },
    );
}

/// A request pinning an address other than the granted one is refused 4100 —
/// and the verdict is a REFUSAL, never a forward carrying some other signer.
#[test]
fn popup_event_refuses_a_stale_pinned_address() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup("personal_sign", Some(grant(A1)), Some(&[A1, A2]), Some(A2)),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::StaleAuthorizedAddress,
        },
    );

    // The same address in another case still forwards, pinned to the grant.
    let upper = A1.to_uppercase().replace("0X", "0x");
    let verdict = ask(
        &mut sut,
        popup(
            "personal_sign",
            Some(grant(A1)),
            Some(&[A1, A2]),
            Some(&upper),
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A1.to_owned(),
        },
    );
}

/// Connect on an already-granted origin answers immediately, in the shape the
/// method asks for — no second prompt on revisit.
#[test]
fn popup_event_connect_on_a_granted_origin_answers_without_a_prompt() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup(
            "eth_requestAccounts",
            Some(grant(A1)),
            Some(&[A1, A2]),
            None,
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Respond {
            payload: Payload::Accounts {
                addresses: vec![A1.to_owned()],
            },
        },
    );
    let verdict = ask(
        &mut sut,
        popup(
            "wallet_requestPermissions",
            Some(grant(A1)),
            Some(&[A1, A2]),
            None,
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Respond {
            payload: Payload::Permissions { granted: true },
        },
    );
}

/// The question is PURE: it authors no operation, persists nothing, and the
/// only thing it leaves behind is its own answer — which the next question
/// replaces. A window that asks must never change what another window's grant
/// store says.
#[test]
fn the_popup_question_authors_nothing() {
    let mut sut = Sut::new();
    let first = ask(
        &mut sut,
        popup("personal_sign", Some(grant(A2)), Some(&[A1, A2]), None),
    );
    assert_eq!(
        first.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A2.to_owned(),
        },
    );
    let second = ask(
        &mut sut,
        popup("personal_sign", None, Some(&[A1, A2]), None),
    );
    assert_eq!(
        second.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::NotConnected,
        },
        "the verdict is replaced, never merged with the last one"
    );
    assert!(
        sut.outstanding().is_empty(),
        "no store write, no respond, nothing in flight"
    );
}

// ---------------------------------------------------------------------------
// Method sets + origin helpers — the single point
// ---------------------------------------------------------------------------

#[test]
fn method_sets_are_the_single_point() {
    for method in [
        "eth_sendTransaction",
        "personal_sign",
        "eth_sign",
        "eth_signTypedData",
        "eth_signTypedData_v1",
        "eth_signTypedData_v3",
        "eth_signTypedData_v4",
        "wallet_sendCalls",
    ] {
        assert!(is_signing_method(method), "{method}");
    }
    assert!(!is_signing_method("eth_call"));
    assert!(!is_signing_method("wallet_switchEthereumChain"));

    assert!(is_connect_method("eth_requestAccounts"));
    assert!(is_connect_method("wallet_requestPermissions"));
    assert!(!is_connect_method("eth_accounts"));
}

#[test]
fn origin_of_normalizes_like_the_url_constructor() {
    assert_eq!(
        origin_of("https://Dapp.Example/Swap?x=1#y").as_deref(),
        Some("https://dapp.example"),
    );
    assert_eq!(
        origin_of("https://dapp.example:443/x").as_deref(),
        Some("https://dapp.example"),
        "default port stripped",
    );
    assert_eq!(
        origin_of("http://dapp.example:8080/x").as_deref(),
        Some("http://dapp.example:8080"),
        "non-default port kept",
    );
    assert_eq!(
        origin_of("http://[::1]:8545/rpc").as_deref(),
        Some("http://[::1]:8545"),
    );
    assert_eq!(origin_of(""), None);
    assert_eq!(origin_of("not a url"), None);
    assert_eq!(
        origin_of("javascript:alert(1)"),
        None,
        "non-http(s) never becomes an origin"
    );
}
