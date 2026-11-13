//! Spec 102 R8 — the surface. On the Safari Web Extension, whose pages cannot
//! use a `getvela.app` passkey (WebKit: `SecurityError`, "The requested RPID
//! did not match the origin", measured on iOS 26.2), every signature and
//! every key ceremony goes to a page; every other surface answers as R3 and R7
//! always did.
//!
//! 1. The shared vectors (`tests/vectors/signing-route/surface.json`, also
//!    read by the web through the wasm): per account record, method and
//!    surface, the route.
//! 2. R3 on a surface: the page a ceremony runs on.
//! 3. Creating a wallet on the Safari surface: every key minted and confirmed
//!    on the official page, the unit filed under `getvela.app`, and the
//!    wallet an ordinary `getvela.app` one (Vela's sheet as its venue — the
//!    same keys the apps use).
//! 4. Signing in on the Safari surface: on the official page.
//! 5. A shell that never says its surface is the standard one, and a surface
//!    is said before the first key or not at all.

#![cfg(feature = "crux")]

mod support;

use serde_json::Value;
use support::{Driver, NOW};
use vela_core::app::create_wallet::{CreateWallet, Event as CreateEvent, ACK_COUNT};
use vela_core::app::login::{Event as LoginEvent, Login};
use vela_core::app::shell::{ShellOperation, ShellResult};
use vela_core::app::KeyMethod;
use vela_core::signing_venue::{ceremony_page_on, SigningSurface};

const OFFICIAL: &str = "https://sign.getvela.app/";

fn vectors() -> Value {
    let path = format!(
        "{}/tests/vectors/signing-route/surface.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| unreachable!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{path}: {e}"))
}

fn surface(name: &str) -> SigningSurface {
    serde_json::from_value(Value::String(name.to_owned()))
        .unwrap_or_else(|e| unreachable!("{name}: {e}"))
}

#[test]
fn every_vector_routes_as_written() {
    let doc = vectors();
    let accounts = &doc["accounts"];
    let cases = doc["cases"].as_array().cloned().unwrap_or_default();
    assert!(cases.len() >= 100, "the vectors are all there");
    let mut safari_in_vela = 0;
    for case in cases {
        let account = accounts[case["account"].as_str().unwrap_or_default()].to_string();
        let method = case["method"].as_str().unwrap_or_default();
        let on = surface(case["surface"].as_str().unwrap_or_default());
        let got: Value = vela_core::app::signing_route_json_on(&account, method, on)
            .map(|json| serde_json::from_str(&json).unwrap_or_default())
            .unwrap_or(Value::Null);
        assert_eq!(got, case["route"], "{case}");
        if on == SigningSurface::SafariExtension && got["type"] == "in_vela" {
            safari_in_vela += 1;
        }
        // The standard surface is the door every shell used before R8.
        if on == SigningSurface::Standard {
            let before: Value = vela_core::app::signing_route_json(&account, method)
                .map(|json| serde_json::from_str(&json).unwrap_or_default())
                .unwrap_or(Value::Null);
            assert_eq!(got, before, "{case}");
        }
    }
    assert_eq!(safari_in_vela, 0, "nothing is signed in Vela on Safari");
}

#[test]
fn every_ceremony_page_vector() {
    for case in vectors()["ceremony_page"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let page = case["page"].as_str();
        let on = surface(case["surface"].as_str().unwrap_or_default());
        assert_eq!(
            ceremony_page_on(page, on).as_deref(),
            case["expect"].as_str(),
            "{case}"
        );
    }
}

/// A member's approval follows the same rule through its own door.
#[test]
fn an_approval_on_safari_goes_to_the_page() {
    let doc = vectors();
    let account = doc["accounts"]["in_vela"].to_string();
    let route: Value = vela_core::trusted_signer::approval::approval_route_json_on(
        &account,
        SigningSurface::SafariExtension,
    )
    .map(|json| serde_json::from_str(&json).unwrap_or_default())
    .unwrap_or_default();
    assert_eq!(route, serde_json::json!({"type": "page", "url": OFFICIAL}));
    assert_eq!(
        vela_core::trusted_signer::approval::approval_route_json(&account),
        Some(r#"{"type":"in_vela"}"#.to_owned())
    );
}

// ---------------------------------------------------------------------------
// The machines
// ---------------------------------------------------------------------------

const GROUP_SEED: &str = "5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";
const GROUP_KEY: &str = "04feedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed\
feedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed01";

/// The form filled and the group key minted, on `on` — the key list, empty.
fn create_on(on: Option<SigningSurface>) -> Driver<CreateWallet> {
    let mut sut = Driver::<CreateWallet>::new();
    sut.dispatch(CreateEvent::Start);
    if let Some(surface) = on {
        assert!(
            sut.dispatch(CreateEvent::SurfaceKnown { surface })
                .is_empty(),
            "saying the surface asks nothing of the shell"
        );
    }
    sut.dispatch(CreateEvent::NameChanged {
        name: "Ann".to_owned(),
    });
    for index in 0..ACK_COUNT {
        sut.dispatch(CreateEvent::AckToggled { index });
    }
    sut.dispatch(CreateEvent::Submit);
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::GroupKeyGenerated {
        seed_hex: GROUP_SEED.to_owned(),
        group_public_key_hex: GROUP_KEY.to_owned(),
    });
    sut
}

fn first_key(sut: &mut Driver<CreateWallet>, origin: Option<&str>) -> Vec<ShellOperation> {
    let mut asked = sut.dispatch(CreateEvent::AddKey {
        name: String::new(),
        method: KeyMethod::Platform,
    });
    let mut registration = support::registration("credential-1");
    registration.signer_origin = origin.map(str::to_owned);
    asked.extend(sut.resolve(ShellResult::PasskeyRegistered {
        registration,
        now_iso: NOW.to_owned(),
    }));
    asked
}

/// On the Safari surface a wallet's keys are made, and confirmed, on the
/// official page — and the wallet is an ordinary `getvela.app` one.
#[test]
fn creating_on_safari_runs_every_ceremony_on_the_official_page() {
    let mut sut = create_on(Some(SigningSurface::SafariExtension));
    let asked = first_key(&mut sut, Some("https://sign.getvela.app"));
    match asked.as_slice() {
        [ShellOperation::RegisterPasskey { page, method, .. }, ShellOperation::SignMemberProof {
            page: proof_page, ..
        }] => {
            assert_eq!(
                page.as_deref(),
                Some(OFFICIAL),
                "minted on the official page"
            );
            assert_eq!(*method, KeyMethod::Platform, "where the person said");
            assert_eq!(proof_page.as_deref(), Some(OFFICIAL), "confirmed there too");
        }
        other => panic!("expected the page's ceremonies, got {other:?}"),
    }
    let view = sut.view();
    assert_eq!(view.signing_domain, "getvela.app", "the apps' own keys");
    assert_eq!(
        view.signing_page, None,
        "no page was chosen — the surface decided"
    );

    // A second key: the same page.
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });
    let next = sut.dispatch(CreateEvent::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::RegisterPasskey { page: Some(page), .. }] if page == OFFICIAL),
        "{next:?}"
    );
}

/// Every other surface is R3 as it was: a `getvela.app` wallet's ceremonies in
/// the app, whether the shell said `standard` or nothing at all.
#[test]
fn creating_anywhere_else_is_unchanged() {
    for on in [None, Some(SigningSurface::Standard)] {
        let mut sut = create_on(on);
        let asked = first_key(&mut sut, None);
        assert!(
            matches!(
                asked.as_slice(),
                [
                    ShellOperation::RegisterPasskey { page: None, .. },
                    ShellOperation::SignMemberProof { page: None, .. }
                ]
            ),
            "{on:?}: {asked:?}"
        );
    }
}

/// The set's keys are made in one place: a surface said after the first key
/// changes nothing.
#[test]
fn a_surface_is_said_before_the_first_key() {
    let mut sut = create_on(None);
    first_key(&mut sut, None);
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });
    sut.dispatch(CreateEvent::SurfaceKnown {
        surface: SigningSurface::SafariExtension,
    });
    let next = sut.dispatch(CreateEvent::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::Platform,
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RegisterPasskey { page: None, .. }]
        ),
        "{next:?}"
    );
}

/// The wire: `{"type":"surface_known","surface":"safari_extension"}`, and a
/// shell that sends no surface is the standard one.
#[test]
fn the_surface_event_reads_from_the_wire() {
    let event: CreateEvent =
        serde_json::from_str(r#"{"type":"surface_known","surface":"safari_extension"}"#)
            .unwrap_or_else(|e| unreachable!("{e}"));
    assert!(matches!(
        event,
        CreateEvent::SurfaceKnown {
            surface: SigningSurface::SafariExtension
        }
    ));
    let event: LoginEvent =
        serde_json::from_str(r#"{"type":"surface_known"}"#).unwrap_or_else(|e| unreachable!("{e}"));
    assert!(matches!(
        event,
        LoginEvent::SurfaceKnown {
            surface: SigningSurface::Standard
        }
    ));
}

fn sign_in_on(on: Option<SigningSurface>, page: Option<&str>) -> Vec<ShellOperation> {
    let mut sut = Driver::<Login>::new();
    sut.dispatch(LoginEvent::Start);
    sut.resolve(ShellResult::IndexHealth { ok: true });
    if let Some(surface) = on {
        sut.dispatch(LoginEvent::SurfaceKnown { surface });
    }
    sut.dispatch(LoginEvent::SignIn {
        method: KeyMethod::Hybrid,
        page: page.map(str::to_owned),
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true })
}

/// Signing in on Safari: on the official page, or on the page chosen.
#[test]
fn signing_in_on_safari_runs_on_a_page() {
    assert_eq!(
        sign_in_on(Some(SigningSurface::SafariExtension), None),
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Hybrid,
            page: Some(OFFICIAL.to_owned()),
        }]
    );
    assert_eq!(
        sign_in_on(
            Some(SigningSurface::SafariExtension),
            Some("http://localhost:8140")
        ),
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Hybrid,
            page: Some("http://localhost:8140/".to_owned()),
        }]
    );
    for on in [None, Some(SigningSurface::Standard)] {
        assert_eq!(
            sign_in_on(on, None),
            vec![ShellOperation::AuthenticatePasskey {
                method: KeyMethod::Hybrid,
                page: None,
            }],
            "{on:?}"
        );
    }
}
