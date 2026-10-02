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
    dapp_spelling, decide_popup_request, granted_to_signed_in, is_connect_method,
    is_insecure_public_origin, is_secure_context, is_signing_method, offers_wallet, origin_of,
    popup_origin_refusal, resolve_granted, settle_on_close, DappPermissions, DpermGrant,
    DpermOperation as Op, DpermPopupDecision, DpermPopupOutcome, DpermPopupView,
    DpermRejectReason as Reason, DpermRespondPayload as Payload, DpermShellResult as Res, Event,
};

type Sut = DomainDriver<DappPermissions>;

const T0: f64 = 1_754_700_000_000.0;
const ORIGIN: &str = "https://dapp.example";
const A1: &str = "0x1111111111111111111111111111111111111111";
const A2: &str = "0x2222222222222222222222222222222222222222";
const A3: &str = "0x3333333333333333333333333333333333333333";
/// A mixed-case account: its EIP-55 spelling differs from its lower case.
const V: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
const V_LOWER: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";

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
// Spec 088 FR-004 — who is offered the wallet at all: a secure context
// ---------------------------------------------------------------------------

#[test]
fn secure_context_is_https_or_exact_loopback() {
    for origin in [
        "https://dapp.example",
        "https://dapp.example:8443",
        "HTTPS://DAPP.EXAMPLE",
        "http://localhost",
        "http://localhost:5173",
        "http://dev.localhost:3000",
        "http://127.0.0.1",
        "http://127.0.0.1:8137",
        "http://127.12.0.9",
        "http://[::1]",
        "http://[::1]:8137",
    ] {
        assert!(is_secure_context(origin), "{origin}");
    }
    for origin in [
        "http://dapp.example",
        // A private LAN page may load, but it is reachable by anyone on that
        // network — no provider there.
        "http://192.168.1.4:8137",
        "http://10.0.0.1",
        "http://dev.local",
        "http://[fd12::1]",
        // Names that only start like loopback are public FQDNs.
        "http://127.0.0.1.evil.com",
        "http://localhost.evil.com",
        "http://127.0.0.256",
        "ws://localhost",
        "file:///etc/hosts",
        "not a url",
        "",
    ] {
        assert!(!is_secure_context(origin), "{origin}");
    }
}

// ---------------------------------------------------------------------------
// Spec 091 — who is offered the wallet, with Settings' debug mode
// ---------------------------------------------------------------------------

/// The one rule the gate and the injected script follow. Debug mode adds
/// http on this device's own network — matched exactly — and nothing else.
#[test]
fn offers_wallet_table() {
    // (origin, offered with debug mode off, offered with it on)
    let table: &[(&str, bool, bool)] = &[
        // Secure contexts: always.
        ("https://dapp.example", true, true),
        ("https://192.168.1.5:3000", true, true),
        ("http://localhost:5173", true, true),
        ("http://dev.localhost", true, true),
        ("http://127.0.0.1:8137", true, true),
        ("http://[::1]:3000", true, true),
        // This device's own network: only in debug mode.
        ("http://192.168.1.5:3000", false, true),
        ("http://192.168.0.1", false, true),
        ("http://10.0.0.1", false, true),
        ("http://10.255.255.255", false, true),
        ("http://172.16.0.1", false, true),
        ("http://172.31.255.255", false, true),
        ("http://169.254.1.1", false, true),
        ("http://foo.local", false, true),
        ("http://FOO.LOCAL:8080", false, true),
        ("http://[fd00::1]", false, true),
        ("http://[fd00::1]:3000", false, true),
        ("http://[fc12:3456::1]", false, true),
        ("http://[fe80::2]", false, true),
        // Public http: never, debug mode or not.
        ("http://dapp.example", false, false),
        ("http://10.0.0.1.evil.com", false, false),
        ("http://192.168.1.5.nip.io", false, false),
        ("http://127.0.0.1.evil.com", false, false),
        ("http://foo.local.evil.com", false, false),
        ("http://local", false, false),
        ("http://172.15.0.1", false, false),
        ("http://172.32.0.1", false, false),
        ("http://192.169.0.1", false, false),
        ("http://169.253.0.1", false, false),
        ("http://11.0.0.1", false, false),
        ("http://8.8.8.8", false, false),
        ("http://999.1.1.1", false, false),
        ("http://192.168.1", false, false),
        ("http://192.168.1.5.6", false, false),
        ("http://[2001:db8::1]", false, false),
        ("http://[fe81::1]", false, false),
        ("http://[fd0::1]", false, false),
        ("http://[::ffff:192.168.1.5]", false, false),
        // Not a page at all.
        ("ws://192.168.1.5", false, false),
        ("ftp://10.0.0.1", false, false),
        ("file:///etc/hosts", false, false),
        ("not a url", false, false),
        ("", false, false),
    ];
    for &(origin, off, on) in table {
        assert_eq!(
            offers_wallet(origin, false),
            off,
            "{origin} without debug mode"
        );
        assert_eq!(offers_wallet(origin, true), on, "{origin} in debug mode");
        // Off is spec 088 to the letter.
        assert_eq!(
            offers_wallet(origin, false),
            is_secure_context(origin),
            "{origin}"
        );
        // What debug mode adds never reaches past the insecure-signing
        // exemption.
        if on && !off {
            assert!(!is_insecure_public_origin(origin), "{origin}");
        }
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

/// `signed_in` is the account the session settled on — `None` when nobody is.
fn popup(
    method: &str,
    grant: Option<DpermGrant>,
    signed_in: Option<&str>,
    pinned: Option<&str>,
) -> Event {
    Event::PopupRequest {
        method: method.to_owned(),
        grant,
        signed_in: signed_in.map(str::to_owned),
        pinned_address: pinned.map(str::to_owned),
        origin: None,
        params_json: None,
    }
}

/// The question as the extension's surface asks it since spec 089: the origin
/// and the params come with it, and the core reads the address the request
/// names itself.
fn popup_with(
    method: &str,
    grant: Option<DpermGrant>,
    origin: &str,
    params: serde_json::Value,
) -> Event {
    Event::PopupRequest {
        method: method.to_owned(),
        grant,
        signed_in: Some(A1.to_owned()),
        pinned_address: None,
        origin: Some(origin.to_owned()),
        params_json: Some(params.to_string()),
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
    let verdict = ask(&mut sut, popup("personal_sign", None, Some(A1), None));
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::NotConnected,
        },
    );
    assert!(verdict.granted.is_empty());

    // …and the connect methods do not leak one either: they ask the user.
    let verdict = ask(&mut sut, popup("eth_requestAccounts", None, Some(A1), None));
    assert_eq!(verdict.outcome, DpermPopupOutcome::Consent);
}

/// Spec 096 F11: the consent names the account a Connect shares — the one
/// signed in, which the grant is pinned to — and nothing when it is not a
/// consent or nobody is signed in.
#[test]
fn the_consent_names_the_account_it_would_share() {
    let mut sut = Sut::new();
    let lower = A1.to_ascii_lowercase();
    let verdict = ask(
        &mut sut,
        popup("eth_requestAccounts", None, Some(&lower), None),
    );
    assert_eq!(verdict.outcome, DpermPopupOutcome::Consent);
    assert_eq!(
        verdict.consent_address,
        Some(vela_core::app::dapp_permissions::dapp_spelling(A1)),
        "the dApp's spelling, as the grant will answer it"
    );
    for signed_in in [None, Some("")] {
        let verdict = ask(
            &mut sut,
            popup("eth_requestAccounts", None, signed_in, None),
        );
        assert_eq!(verdict.consent_address, None, "{signed_in:?}");
    }
    let verdict = ask(&mut sut, popup("personal_sign", None, Some(A1), None));
    assert_eq!(verdict.consent_address, None, "a refusal names no account");
}

/// The forward is pinned to the GRANT's address (invariant ⑨) — which is the
/// signed-in account, because a grant for any other account is not answered.
#[test]
fn popup_event_forwards_the_granted_address() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup("eth_sendTransaction", Some(grant(A1)), Some(A1), None),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A1.to_owned(),
        },
    );
    assert_eq!(verdict.granted, vec![A1.to_owned()]);
}

/// Spec 086, issue #315: signed in to A2, a site granted to A1 — an account the
/// device still holds — learns nothing of A1. It used to: the grant was
/// answered for any HELD account, so a re-pin the wallet missed (a sign-in in a
/// fresh document, a side panel open across the switch) handed the site the
/// previous account on connect, on reconnect and on every signature.
#[test]
fn popup_event_never_answers_a_grant_for_an_account_that_is_not_signed_in() {
    let mut sut = Sut::new();
    // Connect: the person is ASKED, and connects the account they are in —
    // never handed the old one, never handed A2 without a word either.
    for method in ["eth_requestAccounts", "wallet_requestPermissions"] {
        let verdict = ask(&mut sut, popup(method, Some(grant(A1)), Some(A2), None));
        assert_eq!(verdict.outcome, DpermPopupOutcome::Consent, "{method}");
        assert!(verdict.granted.is_empty(), "{method}");
    }
    // A signature: refused as not connected, never signed by A1.
    for method in [
        "eth_sendTransaction",
        "personal_sign",
        "eth_signTypedData_v4",
    ] {
        let verdict = ask(&mut sut, popup(method, Some(grant(A1)), Some(A2), None));
        assert_eq!(
            verdict.outcome,
            DpermPopupOutcome::Reject {
                code: 4100,
                reason: Reason::NotConnected,
            },
            "{method}"
        );
    }
    // Pinning A1 explicitly changes nothing: the grant is not answered at all.
    let verdict = ask(
        &mut sut,
        popup("personal_sign", Some(grant(A1)), Some(A2), Some(A1)),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::NotConnected,
        },
    );
}

/// Signed out, a site learns nothing — whatever it was granted. (The grant
/// stays: signing back in lines it back up, `session.rs`.)
#[test]
fn popup_event_tells_nobody_anything_while_signed_out() {
    let mut sut = Sut::new();
    for signed_in in [None, Some("")] {
        let verdict = ask(
            &mut sut,
            popup("eth_requestAccounts", Some(grant(A1)), signed_in, None),
        );
        assert_eq!(verdict.outcome, DpermPopupOutcome::Consent, "{signed_in:?}");
        assert!(verdict.granted.is_empty());
        let verdict = ask(
            &mut sut,
            popup("eth_sendTransaction", Some(grant(A1)), signed_in, None),
        );
        assert_eq!(
            verdict.outcome,
            DpermPopupOutcome::Reject {
                code: 4100,
                reason: Reason::NotConnected,
            },
            "{signed_in:?}"
        );
    }
}

/// The pure rule, case by case — and the matrix its JavaScript twin in the
/// service worker is pinned against (`instant.test.ts`).
#[test]
fn granted_to_signed_in_answers_only_the_signed_in_account() {
    let stored = grant(A1);
    assert_eq!(
        granted_to_signed_in(Some(&stored), Some(A1)),
        vec![A1.to_owned()]
    );
    assert!(granted_to_signed_in(Some(&stored), Some(A2)).is_empty());
    assert!(granted_to_signed_in(Some(&stored), None).is_empty());
    assert!(granted_to_signed_in(Some(&stored), Some("")).is_empty());
    assert!(granted_to_signed_in(None, Some(A1)).is_empty());
    // Any case matches, and the grant comes back as stored.
    let stored = grant(V_LOWER);
    assert_eq!(
        granted_to_signed_in(Some(&stored), Some(V)),
        vec![V_LOWER.to_owned()]
    );
    let stored = grant(V);
    assert_eq!(
        granted_to_signed_in(Some(&stored), Some(V_LOWER)),
        vec![V.to_owned()]
    );
}

/// A request pinning an address other than the granted one is refused 4100 —
/// and the verdict is a REFUSAL, never a forward carrying some other signer.
#[test]
fn popup_event_refuses_a_stale_pinned_address() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup("personal_sign", Some(grant(A1)), Some(A1), Some(A2)),
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
        popup("personal_sign", Some(grant(A1)), Some(A1), Some(&upper)),
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
        popup("eth_requestAccounts", Some(grant(A1)), Some(A1), None),
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
        popup("wallet_requestPermissions", Some(grant(A1)), Some(A1), None),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Respond {
            payload: Payload::Permissions { granted: true },
        },
    );
}

/// Spec 089: a transaction naming another account in its `from` is refused
/// 4100 — the window's by-shape guess read only top-level strings, so a
/// transaction from A2 was signed by A1, the grant, instead of refused. The
/// core reads the address by `dapp_rpc::requested_address`, the rule every
/// in-app browser's sign gate reads.
#[test]
fn popup_event_refuses_a_transaction_from_another_account() {
    let mut sut = Sut::new();
    for method in ["eth_sendTransaction", "wallet_sendCalls"] {
        let verdict = ask(
            &mut sut,
            popup_with(
                method,
                Some(grant(A1)),
                ORIGIN,
                serde_json::json!([{ "from": A2, "to": A3, "value": "0x1" }]),
            ),
        );
        assert_eq!(
            verdict.outcome,
            DpermPopupOutcome::Reject {
                code: 4100,
                reason: Reason::StaleAuthorizedAddress,
            },
            "{method} from another account"
        );
        // The granted account, in any case, and a request that names nobody,
        // both go to signing, pinned to the grant.
        for params in [
            serde_json::json!([{ "from": A1.to_uppercase().replace("0X", "0x"), "to": A3 }]),
            serde_json::json!([{ "to": A3, "value": "0x1" }]),
        ] {
            let verdict = ask(
                &mut sut,
                popup_with(method, Some(grant(A1)), ORIGIN, params),
            );
            assert_eq!(
                verdict.outcome,
                DpermPopupOutcome::ForwardToSigning {
                    granted_address: A1.to_owned(),
                },
                "{method}"
            );
        }
    }
}

/// Spec 089: `personal_sign`'s account is its SECOND param. A message that is
/// itself twenty bytes of hex is a message, not an account — the by-shape
/// guess took it for one and refused a request for the right account.
#[test]
fn popup_event_reads_personal_signs_account_by_position() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup_with(
            "personal_sign",
            Some(grant(A1)),
            ORIGIN,
            serde_json::json!([A3, A1]),
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A1.to_owned(),
        },
    );
    let verdict = ask(
        &mut sut,
        popup_with(
            "personal_sign",
            Some(grant(A1)),
            ORIGIN,
            serde_json::json!(["0x68656c6c6f", A2]),
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::StaleAuthorizedAddress,
        },
    );
}

/// Spec 089: a signature asked for by a public plain-http origin is refused
/// 4100 before anything else — the in-app browsers' sign gate. Loopback and
/// LAN origins keep working (local dApps), and connecting is not refused.
#[test]
fn popup_event_refuses_a_signature_from_an_insecure_public_origin() {
    let mut sut = Sut::new();
    let insecure = "http://dapp.example";
    for method in [
        "personal_sign",
        "eth_sendTransaction",
        "eth_signTypedData_v4",
    ] {
        let verdict = ask(
            &mut sut,
            popup_with(method, Some(grant(A1)), insecure, serde_json::json!([])),
        );
        assert_eq!(
            verdict.outcome,
            DpermPopupOutcome::Reject {
                code: 4100,
                reason: Reason::InsecureOrigin,
            },
            "{method}"
        );
    }
    // Not connected AND insecure: the origin is what is said.
    let verdict = ask(
        &mut sut,
        popup_with("personal_sign", None, insecure, serde_json::json!([])),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Reject {
            code: 4100,
            reason: Reason::InsecureOrigin,
        },
    );
    // Connecting from it is a person's decision, as in an in-app browser.
    let verdict = ask(
        &mut sut,
        popup_with("eth_requestAccounts", None, insecure, serde_json::json!([])),
    );
    assert_eq!(verdict.outcome, DpermPopupOutcome::Consent);
    // https, loopback and the LAN sign as before.
    for origin in [
        "https://dapp.example",
        "http://localhost:5173",
        "http://127.0.0.1:8080",
        "http://192.168.1.20:3000",
    ] {
        let verdict = ask(
            &mut sut,
            popup_with(
                "personal_sign",
                Some(grant(A1)),
                origin,
                serde_json::json!(["0x68656c6c6f", A1]),
            ),
        );
        assert_eq!(
            verdict.outcome,
            DpermPopupOutcome::ForwardToSigning {
                granted_address: A1.to_owned(),
            },
            "{origin}"
        );
    }
}

#[test]
fn the_origin_rule_is_asked_only_of_signatures() {
    assert_eq!(
        popup_origin_refusal("personal_sign", Some("http://dapp.example")),
        Some(Reason::InsecureOrigin)
    );
    assert_eq!(
        popup_origin_refusal("eth_signTypedData_v2", Some("http://dapp.example")),
        Some(Reason::InsecureOrigin),
        "anything dapp_rpc calls a signature"
    );
    assert_eq!(
        popup_origin_refusal("eth_requestAccounts", Some("http://dapp.example")),
        None
    );
    assert_eq!(
        popup_origin_refusal("personal_sign", Some("https://dapp.example")),
        None
    );
    assert_eq!(
        popup_origin_refusal("personal_sign", None),
        None,
        "no origin, no rule"
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
        popup("personal_sign", Some(grant(A2)), Some(A2), None),
    );
    assert_eq!(
        first.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: A2.to_owned(),
        },
    );
    let second = ask(&mut sut, popup("personal_sign", None, Some(A2), None));
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

// ---------------------------------------------------------------------------
// Spec 082 RG10 — one address spelling toward dApps (L-D6)
// ---------------------------------------------------------------------------

/// `dapp_spelling` is EIP-55 for any case, and leaves anything that is not an
/// address exactly as it came — it changes how an account is spelled, never
/// which one, and never invents one.
#[test]
fn dapp_spelling_is_eip55_or_the_input_unchanged() {
    assert_eq!(dapp_spelling(V_LOWER), V);
    assert_eq!(dapp_spelling(&V.to_uppercase().replacen("0X", "0x", 1)), V);
    assert_eq!(dapp_spelling(V), V);
    assert_eq!(dapp_spelling(A1), A1, "no letters, nothing to change");
    for not_an_address in [
        "",
        "0x",
        "0x1234",
        "vitalik.eth",
        "0xZZ",
        "0X1111111111111111111111111111111111111111",
    ] {
        assert_eq!(dapp_spelling(not_an_address), not_an_address);
    }
}

/// A connect approved for a lower-case account writes the grant, the audit row
/// and the answer in EIP-55 — the spelling every later event uses.
#[test]
fn a_lower_case_grant_is_written_eip55() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::PopupApproved {
        origin: ORIGIN.to_owned(),
        request_id: "r1".to_owned(),
        method: "eth_requestAccounts".to_owned(),
        address: V_LOWER.to_owned(),
        chain_id: 1,
        now_ms: T0,
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteGrant {
                grant: DpermGrant {
                    origin: ORIGIN.to_owned(),
                    address: V.to_owned(),
                    chain_id: 1,
                    granted_at_ms: T0,
                },
            },
            Op::SaveConnectionRecord {
                address: V.to_owned(),
                chain_id: 1,
                origin: ORIGIN.to_owned(),
            },
            accounts("r1", &[V]),
        ]
    );
}

/// An account switch re-pins the site in EIP-55, whatever case the wallet
/// named the account in.
#[test]
fn an_account_switch_answers_eip55() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(switch(
        Some(grant(A1)),
        Some(vec![A1.to_owned(), V_LOWER.to_owned()]),
        V_LOWER,
        ORIGIN,
    ));
    assert_eq!(
        ops,
        vec![Op::WriteGrant {
            grant: DpermGrant {
                origin: ORIGIN.to_owned(),
                address: V.to_owned(),
                chain_id: 8453,
                granted_at_ms: T0 + 2_000.0,
            },
        }]
    );
}

/// `resolve_granted` is unchanged: it matches in any case and returns the
/// grant as stored (its JavaScript twin in the worker cannot checksum).
#[test]
fn resolve_granted_still_matches_case_insensitively() {
    let stored = grant(V_LOWER);
    assert_eq!(
        resolve_granted(Some(&stored), Some(&[V.to_owned()])),
        vec![V_LOWER.to_owned()]
    );
    let stored = grant(V);
    assert_eq!(
        resolve_granted(Some(&stored), Some(&[V_LOWER.to_owned()])),
        vec![V.to_owned()]
    );
    assert!(resolve_granted(Some(&stored), Some(&[A1.to_owned()])).is_empty());
}

/// A grant stored lower-case before 082 is answered in EIP-55 by the request
/// window too — the connect answer and the forward — while a pinned address
/// in either case still matches it.
#[test]
fn the_popup_answers_an_old_lower_case_grant_in_eip55() {
    let mut sut = Sut::new();
    let verdict = ask(
        &mut sut,
        popup("eth_requestAccounts", Some(grant(V_LOWER)), Some(V), None),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::Respond {
            payload: Payload::Accounts {
                addresses: vec![V.to_owned()],
            },
        }
    );
    assert_eq!(verdict.granted, vec![V.to_owned()]);

    let verdict = ask(
        &mut sut,
        popup(
            "eth_sendTransaction",
            Some(grant(V_LOWER)),
            Some(V_LOWER),
            Some(V_LOWER),
        ),
    );
    assert_eq!(
        verdict.outcome,
        DpermPopupOutcome::ForwardToSigning {
            granted_address: V.to_owned(),
        }
    );
}
