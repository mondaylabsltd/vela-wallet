//! Spec 075: the Trusted Signer's ceremonies besides signing — what the core
//! asks the page for, and which answers it takes (`contracts/clear-signer-channel.md`
//! §1.3–1.4). One test per acceptance and per refusal.

#![cfg(feature = "crux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde_json::{json, Value};
use vela_core::app::shell::{ProofPurpose, ShellOperation};
use vela_core::app::KeyMethod;
use vela_core::trusted_signer::ceremony::{request, verify, Answer, Ceremony};
use vela_core::trusted_signer::TrustedSignerError;

const PAGE: &str = "https://sign.getvela.app";
const CRED_HEX: &str = "a1b2c3d4";
const REGISTRY: &str = "https://p256-index-v2.getvela.app";

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn register_op() -> ShellOperation {
    ShellOperation::RegisterPasskey {
        name: "Ann".to_owned(),
        exclude_credential_ids: vec!["0a0b".to_owned()],
        method: KeyMethod::TrustedSigner,
    }
}

fn member_op() -> ShellOperation {
    ShellOperation::SignMemberProof {
        credential_id: CRED_HEX.to_owned(),
        public_key_hex: "04aa".to_owned(),
        attestation_hex: String::new(),
        transports: String::new(),
        method: KeyMethod::TrustedSigner,
        group_public_key_hex: "04bb".to_owned(),
        signer_origin: Some(PAGE.to_owned()),
    }
}

fn register() -> Ceremony {
    Ceremony::of(&register_op()).unwrap()
}

fn sign_in() -> Ceremony {
    Ceremony::AuthenticatePasskey {}
}

fn proof(purpose: ProofPurpose) -> Ceremony {
    Ceremony::SignProof {
        credential_id: CRED_HEX.to_owned(),
        purpose,
    }
}

fn member() -> Ceremony {
    Ceremony::of(&member_op()).unwrap()
}

/// An assertion answer over `challenge`, from `origin`, UV set unless `uv` is false.
fn assertion(challenge: &[u8], origin: &str, uv: bool) -> Value {
    let client = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"{origin}","crossOrigin":false}}"#,
        b64(challenge)
    );
    let mut auth_data = vec![0u8; 37];
    auth_data[32] = if uv { 0x05 } else { 0x01 };
    json!({
        "v": 1, "t": "result", "id": "r1",
        "assertion": {
            "credentialId": b64(&[0xa1, 0xb2, 0xc3, 0xd4]),
            "signatureDer": "3044",
            "authenticatorData": hex(&auth_data),
            "clientDataJSON": hex(client.as_bytes()),
            "userHandle": null,
            "authenticatorAttachment": "platform",
        },
        "origin": origin,
    })
}

fn registration(origin: &str) -> Value {
    let client = format!(
        r#"{{"type":"webauthn.create","challenge":"{}","origin":"{origin}"}}"#,
        b64(&[7u8; 32])
    );
    json!({
        "v": 1, "t": "result", "id": "r1",
        "registration": {
            "credentialId": b64(&[0xa1, 0xb2, 0xc3, 0xd4]),
            "attestationObject": support::attestation_object_hex(),
            "clientDataJSON": hex(client.as_bytes()),
            "authenticatorAttachment": "platform",
            "transports": "internal,hybrid",
        },
        "origin": origin,
    })
}

#[test]
fn a_ceremony_reads_from_the_operation_a_shell_already_holds() {
    for operation in [
        register_op(),
        member_op(),
        ShellOperation::AuthenticatePasskey {
            method: KeyMethod::TrustedSigner,
        },
    ] {
        let wire = serde_json::to_string(&operation).unwrap();
        assert_eq!(
            Ceremony::from_json(&wire),
            Ceremony::of(&operation),
            "{wire}"
        );
        assert!(Ceremony::of(&operation).is_some());
    }
    // Not a ceremony: nothing to ask.
    assert!(Ceremony::of(&ShellOperation::LoadAccounts).is_none());
    assert!(Ceremony::from_json(r#"{"type":"load_accounts"}"#).is_none());
}

#[test]
fn each_ceremony_asks_the_page_for_its_own_kind() {
    let create = request(&register(), "r1", "Ann", REGISTRY);
    assert_eq!(create["intent"]["method"], "vela_createPasskey");
    assert_eq!(
        create["intent"]["params"][0]["excludeCredentialIds"][0],
        b64(&[0x0a, 0x0b])
    );
    assert_eq!(create["context"]["walletName"], "Ann");
    assert_eq!(
        request(&sign_in(), "r2", "", "")["intent"]["method"],
        "vela_signIn"
    );
    let proof_request = request(&proof(ProofPurpose::RecoverSecond), "r3", "", "");
    assert_eq!(
        proof_request["intent"]["params"][0]["purpose"],
        "recover_second"
    );
    let member_request = request(&member(), "r4", "Ann", REGISTRY);
    assert_eq!(member_request["intent"]["method"], "vela_memberProof");
    assert_eq!(member_request["intent"]["params"][0]["registry"], REGISTRY);
}

#[test]
fn a_registration_from_the_page_becomes_the_machines_registration() {
    let Answer::Registration(registration) =
        verify(&register(), &registration(PAGE), PAGE, None).unwrap()
    else {
        panic!("a create answers with a registration");
    };
    assert_eq!(registration.credential_id, CRED_HEX);
    assert_eq!(registration.transports, "internal,hybrid");
    assert_eq!(registration.signer_origin.as_deref(), Some(PAGE));
}

#[test]
fn a_registration_from_another_origin_is_refused() {
    assert!(matches!(
        verify(
            &register(),
            &registration("https://evil.example"),
            PAGE,
            None
        ),
        Err(TrustedSignerError::Malformed(_))
    ));
}

#[test]
fn a_sign_in_takes_only_a_challenge_the_page_derived() {
    let ok = verify(
        &sign_in(),
        &assertion(b"vela-signin-1790000000000-0011aabb", PAGE, true),
        PAGE,
        None,
    );
    let Ok(Answer::Assertion(found)) = ok else {
        panic!("{ok:?}")
    };
    assert_eq!(found.signer_origin.as_deref(), Some(PAGE));
    assert_eq!(found.credential_id, CRED_HEX);
    // 32 bytes a requester chose — an operation hash in disguise — is not a sign-in.
    assert_eq!(
        verify(&sign_in(), &assertion(&[9u8; 32], PAGE, true), PAGE, None),
        Err(TrustedSignerError::WrongChallenge)
    );
    assert_eq!(
        verify(
            &sign_in(),
            &assertion(b"vela-recover-17", PAGE, true),
            PAGE,
            None
        ),
        Err(TrustedSignerError::WrongChallenge)
    );
}

#[test]
fn a_proof_takes_its_purposes_prefix_and_its_own_key() {
    let recover = assertion(b"vela-recover-1790000000000", PAGE, true);
    assert!(verify(&proof(ProofPurpose::RecoverSecond), &recover, PAGE, None).is_ok());
    assert_eq!(
        verify(&proof(ProofPurpose::Verify), &recover, PAGE, None),
        Err(TrustedSignerError::WrongChallenge)
    );
    let other_key = Ceremony::SignProof {
        credential_id: "ffff".to_owned(),
        purpose: ProofPurpose::Verify,
    };
    assert_eq!(
        verify(
            &other_key,
            &assertion(b"vela-verify-1790000000000", PAGE, true),
            PAGE,
            None
        ),
        Err(TrustedSignerError::ForeignKey)
    );
}

#[test]
fn a_member_proof_signs_exactly_the_registrys_challenge() {
    let challenge = [0x42u8; 32];
    let signed = assertion(&challenge, PAGE, true);
    assert!(verify(&member(), &signed, PAGE, Some(&challenge)).is_ok());
    assert_eq!(
        verify(
            &member(),
            &assertion(&[0x43u8; 32], PAGE, true),
            PAGE,
            Some(&challenge)
        ),
        Err(TrustedSignerError::WrongChallenge)
    );
    // Without the wallet's own fetch there is nothing to compare with.
    assert_eq!(
        verify(&member(), &signed, PAGE, None),
        Err(TrustedSignerError::WrongChallenge)
    );
}

#[test]
fn an_unverified_user_or_a_foreign_origin_is_refused() {
    assert_eq!(
        verify(
            &sign_in(),
            &assertion(b"vela-signin-1-00", PAGE, false),
            PAGE,
            None
        ),
        Err(TrustedSignerError::NotVerified)
    );
    assert!(matches!(
        verify(
            &sign_in(),
            &assertion(
                b"vela-signin-1-00",
                "https://sign.getvela.app.evil.example",
                true
            ),
            PAGE,
            None
        ),
        Err(TrustedSignerError::Malformed(_))
    ));
}

#[test]
fn a_declined_or_refused_page_says_so() {
    let declined = json!({"v":1,"t":"error","id":"r1","code":"user_rejected"});
    assert_eq!(
        verify(&sign_in(), &declined, PAGE, None),
        Err(TrustedSignerError::Declined)
    );
    let refused = json!({"v":1,"t":"error","id":"r1","code":"refused"});
    assert_eq!(
        verify(&register(), &refused, PAGE, None),
        Err(TrustedSignerError::Refused("refused".to_owned()))
    );
}
