//! Rules of signing in and recovering, one test per rule.
//!
//! The sign-in flow has no end-to-end coverage today, so these are the only
//! automated tests that exercise its branches at all.

#![cfg(feature = "crux")]

mod support;

use support::{Driver, NOW};
use vela_core::app::login::{Event, Login};
use vela_core::app::shell::{CompletionMode, ProofPurpose, ShellOperation, ShellResult};
use vela_core::app::{Assertion, FailureKind, KeyMethod, PromptKind, SignInKey};
use vela_core::registry_resolve::VerifiedBy;

const CRED: &str = "credential-1";

type Sut = Driver<Login>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Mounted, with the health probe answered so it is out of the way.
fn mounted() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.resolve(ShellResult::IndexHealth { ok: true });
    sut
}

/// …→ signed in with a genuine, Safe-compatible assertion, accounts loaded next.
fn authenticated() -> Sut {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    sut
}

/// A wallet this device holds opens after its record is re-saved to name the
/// key that just signed in (founder, 2026-09-26). Walks that local write — no
/// server is involved — and returns what follows it.
fn past_the_resave(sut: &mut Sut, next: Vec<ShellOperation>) -> Vec<ShellOperation> {
    match next.as_slice() {
        [ShellOperation::SaveAccount { .. }] => sut.resolve(ShellResult::AccountSaved),
        _ => next,
    }
}

// ---------------------------------------------------------------------------
// Reachability probe (FR-023)
// ---------------------------------------------------------------------------

/// Spec 038: a probe that never left the machine is the person's network,
/// not our service. The verdict carries that bit so the screen can say
/// "check your network" and withhold the endpoint field.
#[test]
fn three_local_failures_say_this_machine_could_not_get_out() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    for _ in 0..3 {
        if let [ShellOperation::Wait { .. }] =
            sut.resolve(ShellResult::IndexTransportFailed).as_slice()
        {
            sut.resolve(ShellResult::Waited);
        }
    }
    let view = sut.view();
    assert!(view.endpoint_unreachable, "unreachable either way");
    assert!(view.transport_failed, "and this time it is the machine");

    // A remote failure on the LAST probe decides the sentence: the index was
    // reached for by a route that existed, so no local claim.
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.resolve(ShellResult::IndexTransportFailed);
    sut.resolve(ShellResult::Waited);
    sut.resolve(ShellResult::IndexTransportFailed);
    sut.resolve(ShellResult::Waited);
    sut.resolve(ShellResult::IndexHealth { ok: false });
    let view = sut.view();
    assert!(view.endpoint_unreachable);
    assert!(!view.transport_failed);

    // Reachable clears both, as a re-probe after a fixed network would.
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.resolve(ShellResult::IndexTransportFailed);
    sut.resolve(ShellResult::Waited);
    sut.resolve(ShellResult::IndexHealth { ok: true });
    assert!(!sut.view().transport_failed);
}

/// Three failed probes, spaced, before the endpoint settings are surfaced.
#[test]
fn three_failed_probes_declare_the_index_unreachable() {
    let mut sut = Sut::new();
    let first = sut.dispatch(Event::Start);
    assert_eq!(first, vec![ShellOperation::ProbeIndexHealth]);

    let mut waits = 0;
    for _ in 0..3 {
        match sut
            .resolve(ShellResult::IndexHealth { ok: false })
            .as_slice()
        {
            [ShellOperation::Wait { ms: 2000 }] => {
                waits += 1;
                sut.resolve(ShellResult::Waited);
            }
            [] => break,
            other => panic!("unexpected operation while probing: {other:?}"),
        }
    }

    assert_eq!(waits, 2, "two gaps between three probes");
    assert!(sut.view().endpoint_unreachable);
}

/// A server that answers on the second try is not unreachable, and the user is
/// never told anything.
#[test]
fn a_probe_that_succeeds_leaves_the_endpoint_alone() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.resolve(ShellResult::IndexHealth { ok: false });
    sut.resolve(ShellResult::Waited);
    let next = sut.resolve(ShellResult::IndexHealth { ok: true });

    assert!(next.is_empty(), "probing stops");
    assert!(!sut.view().endpoint_unreachable);
}

// ---------------------------------------------------------------------------
// Resolution order (FR-016, FR-017)
// ---------------------------------------------------------------------------

/// A credential this device already knows opens the wallet with no server call
/// at all — the index is a cache, not a gate.
#[test]
fn a_locally_known_credential_opens_the_wallet_without_the_index() {
    let mut sut = authenticated();
    let accounts = vec![
        support::account(
            "other",
            "Other",
            "0x1111111111111111111111111111111111111111",
        ),
        support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222"),
    ];

    let next = sut.resolve(ShellResult::AccountsLoaded { accounts });
    let next = past_the_resave(&mut sut, next);

    match next.as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode:
                CompletionMode::SetWallet {
                    accounts,
                    active_index,
                },
        }] => {
            assert_eq!(*active_index, 1, "the matching account is the active one");
            assert_eq!(accounts.len(), 2, "the whole list is restored");
        }
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

/// No local account: one signature yields two candidate keys, and the registry
/// is asked which one it knows — no second signature yet.
#[test]
fn no_local_account_queries_the_candidate_keys() {
    let mut sut = authenticated();

    let next = sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });

    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RegistryQueryByPublicKey { .. }]
        ),
        "a candidate key is checked against the registry first; got {next:?}"
    );
}

/// A candidate the registry already knows IS the real key: enter with a single
/// signature, no recovery prompt, no publish.
#[test]
fn a_registered_candidate_enters_with_one_signature() {
    let mut sut = authenticated();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });

    // The first candidate queried is the one the registry holds — a legacy
    // entry with no group, so the historical single-key resolution applies.
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::SaveAccount { .. }]),
        "a known candidate is saved directly — one signature; got {next:?}"
    );
    let next = sut.resolve(ShellResult::AccountSaved);
    assert!(
        next.iter()
            .any(|op| matches!(op, ShellOperation::CompleteOnboarding { .. })),
        "the wallet opens"
    );
}

/// Walk the one-signature candidate checks with "not registered" until the
/// two-signature recovery offer appears.
fn walk_to_recover_offer(sut: &mut Sut) {
    loop {
        match sut
            .resolve(ShellResult::RegistryKeyStatus {
                registered: false,
                unit_ids: vec![],
                verified_by: VerifiedBy::Gnosis,
            })
            .as_slice()
        {
            [ShellOperation::RegistryQueryByPublicKey { .. }] => continue,
            [ShellOperation::Prompt {
                kind: PromptKind::RecoverOffer,
                ..
            }] => break,
            other => panic!("unexpected operation while matching candidates: {other:?}"),
        }
    }
}

/// FR-016 — compatibility is checked before anything is resolved or written.
#[test]
fn an_incompatible_provider_stops_before_any_resolution() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });

    let next = sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::incompatible_assertion(CRED),
        now_iso: NOW.to_owned(),
    });

    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: PromptKind::IncompatibleLogin,
                ..
            }]
        ),
        "no accounts are loaded, no index is queried, nothing is saved"
    );
    assert!(!sut.view().busy);
}

// ---------------------------------------------------------------------------
// Recovery (FR-018, FR-019)
// ---------------------------------------------------------------------------

/// When neither candidate is known, declining the recovery offer leaves no
/// trace.
#[test]
fn declining_recovery_persists_nothing() {
    let mut sut = authenticated();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    walk_to_recover_offer(&mut sut);

    let next = sut.resolve(ShellResult::PromptAnswered { accepted: false });
    assert!(next.is_empty(), "declining asks for nothing");
    assert!(!sut.view().busy);
}

/// …→ both candidates unknown → recovery accepted → the second signature
/// requested.
fn awaiting_second_signature(first: Assertion) -> Sut {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: first,
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    walk_to_recover_offer(&mut sut);
    let next = sut.resolve(ShellResult::PromptAnswered { accepted: true });
    assert_eq!(
        next,
        vec![ShellOperation::SignProof {
            credential_id: CRED.to_owned(),
            // Inferred from the assertion's attachment: the fixture reports
            // "platform", so the shell is pointed at the device's own vault
            // rather than left to guess (and Android guesses "security key").
            transports: "internal".to_owned(),
            // The route carries the sign-in method through, so the second
            // signature reaches the same authenticator the first did — here the
            // platform vault this walk signed in with.
            method: KeyMethod::Platform,
            purpose: ProofPurpose::RecoverSecond,
            page: None,
        }],
        "accepting asks for the disambiguating second signature"
    );
    sut
}

/// Accepting rebuilds the key on-device, then — option B — publishes the group
/// to the registry BEFORE entering, and only then opens the wallet.
#[test]
fn accepted_recovery_publishes_then_enters() {
    let (first, second) = support::assertion_pair(CRED);
    let mut sut = awaiting_second_signature(first);

    // The second signature pins down the real key. Both candidates were
    // already checked and unknown, so it publishes straight away — no query.
    let next = sut.resolve(ShellResult::ProofSigned {
        assertion: second,
        now_iso: NOW.to_owned(),
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::RegistryPublish { .. }]),
        "an unpublished recovered key is registered before entry; got {next:?}"
    );

    // Published → save the account and enter.
    let next = sut.resolve(ShellResult::RegistryPublished);
    assert!(matches!(
        next.as_slice(),
        [ShellOperation::SaveAccount { .. }]
    ));
    let next = sut.resolve(ShellResult::AccountSaved);
    assert!(
        next.iter()
            .any(|op| matches!(op, ShellOperation::CompleteOnboarding { .. })),
        "the wallet opens"
    );
}

/// A publish that fails must not trap the user out of a wallet they have
/// already recovered: it still saves and enters.
#[test]
fn a_failed_publish_still_enters() {
    let (first, second) = support::assertion_pair(CRED);
    let mut sut = awaiting_second_signature(first);
    sut.resolve(ShellResult::ProofSigned {
        assertion: second,
        now_iso: NOW.to_owned(),
    });

    let next = sut.resolve_matching(
        |op| matches!(op, ShellOperation::RegistryPublish { .. }),
        ShellResult::IndexFailed {
            message: "still down".to_owned(),
            network: true,
        },
    );
    assert!(
        matches!(next.as_slice(), [ShellOperation::SaveAccount { .. }]),
        "a failed publish still saves and enters; got {next:?}"
    );
}

/// Two signatures that do not pin down exactly one key must fail closed —
/// never guess an address, never query or publish.
#[test]
fn an_unrecoverable_signature_pair_persists_nothing() {
    let mut sut = awaiting_second_signature(support::assertion(CRED));

    // The same signature twice: the candidate sets are ambiguous by design.
    let next = sut.resolve(ShellResult::ProofSigned {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });

    assert!(matches!(
        next.as_slice(),
        [ShellOperation::Prompt {
            kind: PromptKind::RecoverFailed,
            ..
        }]
    ));
}

// ---------------------------------------------------------------------------
// Failure classification (FR-021, FR-022)
// ---------------------------------------------------------------------------

/// A dismissed OS sheet is not an error and must not raise an alert.
#[test]
fn a_cancelled_ceremony_is_silent() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });

    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });

    assert!(next.is_empty(), "no prompt, no error state");
    assert!(!sut.view().busy);
}

/// Issue #459: dismissing the "Scan a code" sheet is how a person changes
/// their mind about signing in with a phone. The shell answers the phone
/// ceremony with `cancelled`, and that is as silent as a dismissed OS sheet —
/// even though every other failure on the phone route is the link's
/// (`phone_link`, issue #446) — and it frees Welcome at once.
#[test]
fn a_cancelled_phone_ceremony_is_silent_too() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Hybrid,
        page: None,
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Hybrid,
            page: None,
        }],
        "the phone route shows a code"
    );
    assert!(sut.view().busy, "busy while the code is up");

    // Whatever words the shell attaches, a cancel is a cancel.
    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: Some("The code was dismissed".to_owned()),
    });
    assert!(
        next.is_empty(),
        "no phone_link prompt, no error state: {next:?}"
    );
    assert!(!sut.view().busy, "Welcome's buttons are live again");

    // The cancel belonged to that ceremony: the next phone sign-in starts
    // afresh and can still succeed.
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Hybrid,
        page: None,
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Hybrid,
            page: None,
        }]
    );
    let next = sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::LoadAccounts]),
        "{next:?}"
    );
}

/// The recovery's second signature rides the same phone route (issue #459):
/// dismissing its code leaves the person where they were, with no
/// "recovery failed" sheet.
#[test]
fn a_cancelled_phone_recovery_signature_is_silent() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Hybrid,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    walk_to_recover_offer(&mut sut);
    let next = sut.resolve(ShellResult::PromptAnswered { accepted: true });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::SignProof {
                method: KeyMethod::Hybrid,
                purpose: ProofPurpose::RecoverSecond,
                ..
            }]
        ),
        "the second signature asks the phone again: {next:?}"
    );

    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });
    assert!(next.is_empty(), "no RecoverFailed, no phone_link: {next:?}");
    assert!(!sut.view().busy);
}

/// Issue #446: an iPad's sign-in with a phone failed because the tunnel was
/// cancelled under it, and the sheet said to set up Face ID. A failure of the
/// link to the other device — as `cable::conn` words it — is flagged so the
/// sheet says to scan again; an authenticator's own failure is not.
#[test]
fn a_failed_link_to_the_phone_is_not_blamed_on_biometrics() {
    let prompt_after = |method: KeyMethod, message: &str| {
        let mut sut = mounted();
        sut.dispatch(Event::SignIn { method, page: None });
        sut.resolve(ShellResult::PasskeySupport { supported: true });
        let next = sut.resolve(ShellResult::PasskeyFailed {
            kind: FailureKind::Other,
            message: Some(message.to_owned()),
        });
        match next.as_slice() {
            [ShellOperation::Prompt { kind, .. }] => kind.clone(),
            other => panic!("expected one prompt, got {other:?}"),
        }
    };
    let phone_link = |kind: &PromptKind| {
        matches!(
            kind,
            PromptKind::SignInFailed {
                phone_link: true,
                ..
            }
        )
    };
    let tunnel = prompt_after(
        KeyMethod::Hybrid,
        "caBLE transport: Error Domain=NSURLErrorDomain Code=-999 \"cancelled\"",
    );
    assert!(
        phone_link(&tunnel),
        "the tunnel's failure is the link's: {tunnel:?}"
    );
    let handshake = prompt_after(KeyMethod::Hybrid, "caBLE Noise failure: BadMac");
    assert!(phone_link(&handshake), "{handshake:?}");
    // The platform's own words for a phone that never answered (Android's
    // HybridCeremony timeout): still the link's — the fingerprint is on the
    // OTHER device, so "set up Face ID here" is wrong (issue #446).
    let unanswered = prompt_after(
        KeyMethod::Hybrid,
        "No phone answered the code. Scan it with the other device and try again.",
    );
    assert!(phone_link(&unanswered), "{unanswered:?}");
    // A passkey on THIS device keeps the old sheet.
    let local = prompt_after(KeyMethod::Platform, "The operation couldn't be completed.");
    assert!(
        matches!(
            local,
            PromptKind::SignInFailed {
                phone_link: false,
                ..
            }
        ),
        "an authenticator's failure keeps the old sheet: {local:?}"
    );
}

/// The flag is additive: a prompt from a core that predates it still reads.
#[test]
fn a_sign_in_failure_without_the_flag_still_decodes() {
    let kind: PromptKind =
        serde_json::from_str(r#"{"type":"sign_in_failed","detail":"x"}"#).unwrap();
    assert!(matches!(
        kind,
        PromptKind::SignInFailed {
            phone_link: false,
            ..
        }
    ));
    assert!(vela_core::cable::conn::is_link_failure(
        "caBLE transport: closed"
    ));
    assert!(!vela_core::cable::conn::is_link_failure("transport: caBLE"));
}

// Resolution no longer performs a credential-id index query, so the old
// "transport failure surfaces settings" and "server error is reported"
// branches at resolution time no longer exist. Endpoint reachability is now
// surfaced solely by the health probe (see the reachability tests above); a
// publish/query failure after recovery degrades to entry (a_failed_publish…).

// ---------------------------------------------------------------------------
// Races (FR-033)
// ---------------------------------------------------------------------------

/// FR-025 — a result belonging to a superseded attempt cannot move the machine.
///
/// The realistic shape: an alert from a failed attempt is still on screen when
/// the user starts a fresh sign-in. Its dismissal arrives late, and must not be
/// mistaken for an answer the *new* attempt is waiting for.
#[test]
fn late_result_after_supersede_cannot_overwrite() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    let stale_prompt = sut.resolve(ShellResult::PasskeySupport { supported: false });
    assert!(matches!(
        stale_prompt.as_slice(),
        [ShellOperation::Prompt { .. }]
    ));

    // A new attempt starts while the alert is still up.
    let fresh = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    assert_eq!(fresh, vec![ShellOperation::CheckPasskeySupport]);

    // The old alert is dismissed now.
    let next = sut.resolve_matching(
        |op| matches!(op, ShellOperation::Prompt { .. }),
        ShellResult::PromptAnswered { accepted: true },
    );

    assert!(next.is_empty(), "the stale answer is dropped");
    assert!(sut.view().busy, "the new attempt is still in flight");

    // …and the new attempt proceeds normally.
    let next = sut.resolve_matching(
        |op| matches!(op, ShellOperation::CheckPasskeySupport),
        ShellResult::PasskeySupport { supported: true },
    );
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Platform,
            page: None,
        }]
    );
}

/// The sign-in method choice reaches the ceremony. Signing in with a security
/// key on a device that ALSO has a platform passkey must run the app-owned
/// route, not let the system pick the platform credential silently — the only
/// way to reach a wallet that lives on a hardware key there.
#[test]
fn the_sign_in_method_choice_reaches_the_ceremony() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::SecurityKey,
        page: None,
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::SecurityKey,
            page: None,
        }],
        "the who-are-you ceremony must run on the chosen route"
    );
}

/// FR-024 — one ceremony at a time on the welcome screen too.
#[test]
fn sign_in_while_busy_is_a_no_op() {
    let mut sut = mounted();
    let first = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    let second = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });

    assert_eq!(first.len(), 1);
    assert!(second.is_empty());
}

// ---------------------------------------------------------------------------
// Multi-key group reconstruction
// ---------------------------------------------------------------------------

use vela_core::app::RegistryUnitMember;
use vela_core::registry_metadata::{RegistryMetadata, REGISTRY_METADATA_VERSION};

const CRED2: &str = "credential-2";

/// The founding members of the two-key fixture wallet, founding order.
fn unit_members() -> Vec<RegistryUnitMember> {
    vec![
        RegistryUnitMember {
            credential_id: CRED.to_owned(),
            public_key_hex: support::expected_public_key_hex(),
            authenticator_attachment: "platform".to_owned(),
            transports: "hybrid,internal".to_owned(),
        },
        RegistryUnitMember {
            credential_id: CRED2.to_owned(),
            public_key_hex: support::second_public_key_hex(),
            authenticator_attachment: String::new(),
            transports: String::new(),
        },
    ]
}

fn multi_address() -> String {
    let keys = [
        vela_core::safe::parse_public_key(&support::expected_public_key_hex()).unwrap(),
        vela_core::safe::parse_public_key(&support::second_public_key_hex()).unwrap(),
    ];
    vela_core::safe::compute_safe_address_multi(&keys)
        .unwrap()
        .address
}

fn unit_metadata_hex() -> String {
    RegistryMetadata {
        version: REGISTRY_METADATA_VERSION,
        address: multi_address(),
        wallet_version: "safe-1.4.1".to_owned(),
        key_names: vec!["Ann".to_owned(), "Backup".to_owned()],
        created_at_iso: NOW.to_owned(),
    }
    .encode_hex()
    .unwrap()
}

/// …→ the registered candidate belongs to a group: the unit fetch is next.
fn fetching_unit() -> Sut {
    let mut sut = authenticated();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![7, 3], // the lowest id is the founding group
        verified_by: VerifiedBy::None,
    });
    match next.as_slice() {
        [ShellOperation::RegistryQueryUnit { unit_id }] => {
            assert_eq!(*unit_id, 3, "the lowest unit id is the founding group");
        }
        other => panic!("expected the unit fetch, got {other:?}"),
    }
    sut
}

/// A grouped key is reconstructed from the FULL founding set: all members,
/// the recorded multi-key address, names from the metadata blob.
#[test]
fn a_grouped_candidate_reconstructs_the_full_key_set() {
    let mut sut = fetching_unit();
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: unit_members(),
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.address, multi_address());
            assert_eq!(account.keys.len(), 2);
            assert_eq!(account.id, CRED, "identity is the pinned first key");
            assert_eq!(account.keys[1].credential_id, CRED2);
            assert_eq!(account.keys[1].name, "Backup");
            assert_eq!(account.public_key_hex, support::expected_public_key_hex());
        }
        other => panic!("expected the reconstructed save, got {other:?}"),
    }
}

/// A server that reorders members cannot move the wallet: the pin is re-found
/// by derivation against the recorded address.
#[test]
fn reconstruction_survives_shuffled_member_order() {
    let mut sut = fetching_unit();
    let mut shuffled = unit_members();
    shuffled.reverse();
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: shuffled,
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.address, multi_address());
            assert_eq!(
                account.keys[0].credential_id, CRED,
                "the pin is recovered by derivation, not trusted from fetch order"
            );
        }
        other => panic!("expected the reconstructed save, got {other:?}"),
    }
}

/// A unit whose members cannot recompute the recorded address is never
/// persisted — nothing enters on a guess.
#[test]
fn a_group_that_does_not_derive_its_address_is_refused() {
    let mut sut = fetching_unit();
    let metadata_hex = RegistryMetadata {
        version: REGISTRY_METADATA_VERSION,
        address: "0x000000000000000000000000000000000000dEaD".to_owned(),
        wallet_version: "safe-1.4.1".to_owned(),
        key_names: vec!["Ann".to_owned(), "Backup".to_owned()],
        created_at_iso: NOW.to_owned(),
    }
    .encode_hex()
    .unwrap();
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex,
        members: unit_members(),
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: PromptKind::SignInFailed { .. },
                ..
            }]
        ),
        "a non-deriving group is a sign-in failure, never a save; got {next:?}"
    );
    assert!(!sut.view().busy);
}

/// The unit exists but could not be read: a multi-key address cannot be
/// derived from one key, so nothing is guessed — the registry could not be
/// reached, and the person may ask again from the same signature.
#[test]
fn a_failed_unit_fetch_says_the_registry_is_unreachable_instead_of_guessing() {
    let mut sut = fetching_unit();
    let next = sut.resolve(ShellResult::IndexFailed {
        message: "offline".to_owned(),
        network: true,
    });
    assert_eq!(
        next,
        vec![ShellOperation::Prompt {
            kind: PromptKind::RegistryUnreachable { local: true },
            confirmable: true,
        }],
        "no single-key fallback exists for a known group"
    );
    // "Try again" starts over from the signature already made.
    let next = sut.resolve(ShellResult::PromptAnswered { accepted: true });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RegistryQueryByPublicKey { .. }]
        ),
        "a retry re-asks the registry, with no new ceremony; got {next:?}"
    );
}

/// A sibling credential of a locally stored multi-key wallet opens it
/// directly — any founding key matches, not just the first.
#[test]
fn a_sibling_credential_matches_the_local_multikey_account() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    // Authenticated with the SECOND founding key's credential.
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED2),
        now_iso: NOW.to_owned(),
    });

    let account = vela_core::app::Account {
        keys: vec![
            vela_core::app::AccountKey {
                credential_id: CRED.to_owned(),
                public_key_hex: support::expected_public_key_hex(),
                name: "Ann".to_owned(),
                transports: "internal".to_owned(),
                signer_origin: None,
            },
            vela_core::app::AccountKey {
                credential_id: CRED2.to_owned(),
                public_key_hex: support::second_public_key_hex(),
                name: "Backup".to_owned(),
                transports: "usb,nfc".to_owned(),
                signer_origin: None,
            },
        ],
        ..support::account(CRED, "Ann", &multi_address())
    };
    let next = sut.resolve(ShellResult::AccountsLoaded {
        accounts: vec![account],
    });
    let next = past_the_resave(&mut sut, next);
    match next.as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode: CompletionMode::SetWallet { active_index, .. },
        }] => {
            assert_eq!(*active_index, 0, "the sibling key opens the wallet locally");
        }
        other => panic!("expected immediate completion, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Legacy name recovery (v1-era wallets whose handle yields no name)
// ---------------------------------------------------------------------------

/// …→ signed in with a handle-LESS assertion (a v1-era passkey whose
/// userHandle yields no name): the resolved name falls to the fallback.
fn authenticated_nameless() -> Sut {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: Assertion {
            user_id_hex: None,
            ..support::assertion(CRED)
        },
        now_iso: NOW.to_owned(),
    });
    sut
}

/// The conformance assertion carries no decodable user handle, so the
/// resolved name falls to the fallback — exactly the v1-era shape.
#[test]
fn a_fallback_name_asks_the_legacy_index_before_entering() {
    let mut sut = authenticated_nameless();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    // Legacy v2 entry, no group: the historical single-key resolution.
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    match next.as_slice() {
        [ShellOperation::LookupLegacyName { credential_id }] => {
            assert_eq!(credential_id, CRED);
        }
        other => panic!("a fallback name is worth one legacy lookup; got {other:?}"),
    }
    let next = sut.resolve(ShellResult::LegacyName {
        name: Some("大表哥".to_owned()),
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.name, "大表哥");
            assert_eq!(
                account.keys[0].name, "大表哥",
                "keys[0] carries the wallet name"
            );
        }
        other => panic!("expected the save with the recovered name, got {other:?}"),
    }
}

/// No legacy record (or the index is offline): the fallback stands and the
/// login is never blocked.
#[test]
fn a_missing_legacy_name_keeps_the_fallback_and_enters() {
    let mut sut = authenticated_nameless();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    let next = sut.resolve(ShellResult::LegacyName { name: None });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.name, "Wallet");
        }
        other => panic!("expected the save, got {other:?}"),
    }
}

/// The recovery PUBLISH freezes the name into the group metadata — the one
/// write that made this bug permanent — so it too resolves first.
#[test]
fn recovery_resolves_the_name_before_freezing_it_into_the_publish() {
    let mut sut = authenticated_nameless();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    walk_to_recover_offer(&mut sut);
    sut.resolve(ShellResult::PromptAnswered { accepted: true });
    let (_, second) = support::assertion_pair(CRED);
    let next = sut.resolve(ShellResult::ProofSigned {
        assertion: second,
        now_iso: NOW.to_owned(),
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::LookupLegacyName { .. }]),
        "the publish would freeze the fallback; got {next:?}"
    );
    let next = sut.resolve(ShellResult::LegacyName {
        name: Some("大表哥".to_owned()),
    });
    match next.as_slice() {
        [ShellOperation::RegistryPublish {
            members,
            metadata_hex,
            ..
        }] => {
            assert_eq!(members.len(), 1);
            let metadata =
                vela_core::registry_metadata::RegistryMetadata::decode_hex(metadata_hex).unwrap();
            assert_eq!(metadata.key_names, vec!["大表哥".to_owned()]);
        }
        other => panic!("expected the publish, got {other:?}"),
    }
}

/// An unprintable or oversized server name is refused — the same bar the
/// handle is held to; the fallback stands instead.
#[test]
fn a_malformed_legacy_name_is_refused() {
    let mut sut = authenticated_nameless();
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    let next = sut.resolve(ShellResult::LegacyName {
        name: Some("bad\u{7}name".to_owned()),
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.name, "Wallet");
        }
        other => panic!("expected the save, got {other:?}"),
    }
}

/// An UPPERCASE uuid tail (iOS `UUID().uuidString`) no longer discards the
/// handle's name — uuid shape is case-insensitive now.
#[test]
fn an_uppercase_uuid_handle_still_yields_its_name() {
    use vela_core::app::Assertion;
    let assertion = Assertion {
        credential_id: CRED.to_owned(),
        signature_der_hex: String::new(),
        authenticator_data_hex: String::new(),
        client_data_json_hex: String::new(),
        user_id_hex: Some(
            "大表哥\u{0}0F8FAD5B-D9CB-469F-A165-70867728950E"
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect(),
        ),
        authenticator_attachment: String::new(),
        signer_origin: None,
    };
    // user_name() is pub(crate); observe through the machine instead: a
    // local account match is not needed — the name only matters on save, so
    // assert via the matched-candidate path with a crafted assertion.
    // (Direct: the mod-level unit test covers the decode; this pins the
    // login-visible behavior.)
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: Assertion {
            // A REAL, Safe-compatible assertion body with the crafted handle.
            user_id_hex: assertion.user_id_hex.clone(),
            ..support::assertion(CRED)
        },
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(
                account.name, "大表哥",
                "no legacy lookup needed — the handle decodes"
            );
        }
        other => panic!("expected a direct save, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Spec 048: the retired client's records (camelCase, at the same web origin)
// ---------------------------------------------------------------------------

/// The same list `support::account` builds, re-spelt the way the retired
/// client wrote it: `publicKeyHex`, `createdAt`, `keys[].credentialId`.
/// `drop_keys` = a wallet from before the multi-key change (no `keys` at all).
fn expo_spelling(accounts: Vec<vela_core::app::Account>, drop_keys: bool) -> ShellResult {
    let mut value = serde_json::to_value(ShellResult::AccountsLoaded { accounts }).unwrap();
    let list = value["accounts"].as_array_mut().unwrap();
    for account in list.iter_mut() {
        let object = account.as_object_mut().unwrap();
        let pk = object.remove("public_key_hex").unwrap();
        object.insert("publicKeyHex".into(), pk);
        let created = object.remove("created_at_iso").unwrap();
        object.insert("createdAt".into(), created);
        let keys = object.remove("keys").unwrap();
        if !drop_keys {
            let keys: Vec<serde_json::Value> = keys
                .as_array()
                .unwrap()
                .iter()
                .map(|key| {
                    let k = key.as_object().unwrap();
                    serde_json::json!({
                        "credentialId": k["credential_id"],
                        "publicKeyHex": k["public_key_hex"],
                        "name": k["name"],
                    })
                })
                .collect();
            object.insert("keys".into(), serde_json::Value::Array(keys));
        }
    }
    let json = serde_json::to_string(&value).unwrap();
    assert!(
        json.contains("publicKeyHex") && !json.contains("public_key_hex"),
        "{json}"
    );
    serde_json::from_str(&json).expect("the core reads the retired client's spelling")
}

#[test]
fn an_expo_era_record_still_opens_the_wallet() {
    let mut sut = authenticated();
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    let next = sut.resolve(expo_spelling(vec![stored.clone()], false));
    let next = past_the_resave(&mut sut, next);
    match next.as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode:
                CompletionMode::SetWallet {
                    accounts,
                    active_index,
                },
        }] => {
            assert_eq!(*active_index, 0);
            assert_eq!(accounts[0].public_key_hex, stored.public_key_hex);
            assert_eq!(accounts[0].created_at_iso, stored.created_at_iso);
            assert_eq!(accounts[0].keys.len(), stored.keys.len());
            if let Some(key) = accounts[0].keys.first() {
                assert_eq!(key.credential_id, CRED);
                assert_eq!(
                    key.transports, "",
                    "the old client never recorded where the key lives"
                );
            }
        }
        other => panic!("expected the wallet to open, got {other:?}"),
    }
}

#[test]
fn an_expo_era_record_without_keys_still_opens_the_wallet() {
    let mut sut = authenticated();
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    let next = sut.resolve(expo_spelling(vec![stored.clone()], true));
    let next = past_the_resave(&mut sut, next);
    match next.as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode:
                CompletionMode::SetWallet {
                    accounts,
                    active_index,
                },
        }] => {
            assert_eq!(*active_index, 0);
            assert!(
                accounts[0].keys.is_empty(),
                "no keys list: the scalar fields are the key"
            );
            assert_eq!(accounts[0].public_key_hex, stored.public_key_hex);
        }
        other => panic!("expected the wallet to open, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// One wallet, one record
// ---------------------------------------------------------------------------

/// Signing in with a SECOND passkey of a wallet this device already holds is
/// a sign-in, not an addition.
///
/// The credential match above cannot see it: the stored record was written
/// before this key joined, so it lists neither the credential nor its key —
/// the recovery path runs and rebuilds the wallet from the registry. What
/// comes back has the same ADDRESS, because the address derives from the
/// whole founding set, and `SaveAccount` upserts by `id`. So the rebuild used
/// to be stored under the new credential's id: one wallet, two records, two
/// rows in every switcher, and both opening the same Safe (founder-reported,
/// 2026-09-16).
#[test]
fn a_wallet_already_held_is_entered_not_saved_again() {
    let mut sut = authenticated();
    let held = support::account("some-earlier-credential", "Ann", &multi_address());
    sut.resolve(ShellResult::AccountsLoaded {
        accounts: vec![held.clone()],
    });
    sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![3],
        verified_by: VerifiedBy::None,
    });

    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: unit_members(),
    });

    match next.as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode:
                CompletionMode::SetWallet {
                    accounts,
                    active_index,
                },
        }] => {
            assert_eq!(accounts.len(), 1, "nothing was added");
            assert_eq!(
                accounts[0].id, held.id,
                "the record already here is the one kept"
            );
            assert_eq!(*active_index, 0, "and it is the one entered");
        }
        other => panic!("expected a sign-in, not a save; got {other:?}"),
    }
}

/// The address is the identity, not its casing.
#[test]
fn a_wallet_held_in_another_casing_is_still_the_same_wallet() {
    let mut sut = authenticated();
    let shouted = multi_address().to_uppercase().replace("0X", "0x");
    sut.resolve(ShellResult::AccountsLoaded {
        accounts: vec![support::account("some-earlier-credential", "Ann", &shouted)],
    });
    sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![3],
        verified_by: VerifiedBy::None,
    });
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: unit_members(),
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::CompleteOnboarding {
                mode: CompletionMode::SetWallet { .. }
            }]
        ),
        "expected a sign-in; got {next:?}"
    );
}

/// A wallet this device does NOT hold is still saved and still added — the
/// recovery path is untouched for everything that is genuinely new.
#[test]
fn an_unheld_wallet_is_still_saved_and_added() {
    let mut sut = authenticated();
    sut.resolve(ShellResult::AccountsLoaded {
        accounts: vec![support::account(
            "unrelated",
            "Bo",
            "0x1111111111111111111111111111111111111111",
        )],
    });
    sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![3],
        verified_by: VerifiedBy::None,
    });
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: unit_members(),
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.address, multi_address());
        }
        other => panic!("expected the reconstructed save, got {other:?}"),
    }
    let next = sut.resolve(ShellResult::AccountSaved);
    assert!(
        next.iter().any(|op| matches!(
            op,
            ShellOperation::CompleteOnboarding {
                mode: CompletionMode::AddAccount { .. }
            }
        )),
        "a genuinely new wallet is still ADDED; got {next:?}"
    );
}

// ---------------------------------------------------------------------------
// The key this device signs with (founder, 2026-09-26)
// ---------------------------------------------------------------------------

/// Signed in with `method`, holding `stored`, and past the account load.
fn signed_in_holding(
    credential: &str,
    method: KeyMethod,
    stored: Vec<vela_core::app::Account>,
) -> (Sut, Vec<ShellOperation>) {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn { method, page: None });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(credential),
        now_iso: NOW.to_owned(),
    });
    let next = sut.resolve(ShellResult::AccountsLoaded { accounts: stored });
    (sut, next)
}

/// The key as the sign-in records it. The fixture assertions report a
/// `platform` attachment, so the key was found on this device.
fn named(credential: &str, method: KeyMethod) -> Option<SignInKey> {
    Some(SignInKey {
        credential_id: credential.to_owned(),
        method,
        transports: "internal".to_owned(),
    })
}

/// The sign-in says which key and which route; the record keeps both, and the
/// wallet opens holding them — no per-signature choice is left to make.
#[test]
fn the_sign_in_names_the_key_this_device_signs_with() {
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    let (mut sut, next) = signed_in_holding(CRED, KeyMethod::Hybrid, vec![stored.clone()]);
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.id, stored.id, "the held record, updated in place");
            assert_eq!(account.sign_in_key, named(CRED, KeyMethod::Hybrid));
        }
        other => panic!("expected the record re-saved first, got {other:?}"),
    }
    match sut.resolve(ShellResult::AccountSaved).as_slice() {
        [ShellOperation::CompleteOnboarding {
            mode: CompletionMode::SetWallet { accounts, .. },
        }] => assert_eq!(accounts[0].sign_in_key, named(CRED, KeyMethod::Hybrid)),
        other => panic!("expected the wallet to open, got {other:?}"),
    }
}

/// The same key over the same route again: nothing to write.
#[test]
fn signing_in_the_same_way_again_writes_nothing() {
    let stored = vela_core::app::Account {
        sign_in_key: named(CRED, KeyMethod::Platform),
        ..support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222")
    };
    let (_, next) = signed_in_holding(CRED, KeyMethod::Platform, vec![stored]);
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::CompleteOnboarding {
                mode: CompletionMode::SetWallet { .. }
            }]
        ),
        "expected an immediate sign-in; got {next:?}"
    );
}

/// There is no switching at signing time: signing in with ANOTHER key of the
/// wallet is how a person changes the one it signs with.
#[test]
fn signing_in_with_another_key_changes_the_one_it_signs_with() {
    let stored = vela_core::app::Account {
        keys: vec![
            vela_core::app::AccountKey {
                credential_id: CRED.to_owned(),
                public_key_hex: support::expected_public_key_hex(),
                name: "Ann".to_owned(),
                transports: "internal".to_owned(),
                signer_origin: None,
            },
            vela_core::app::AccountKey {
                credential_id: CRED2.to_owned(),
                public_key_hex: support::second_public_key_hex(),
                name: "Backup".to_owned(),
                transports: "usb,nfc".to_owned(),
                signer_origin: None,
            },
        ],
        sign_in_key: named(CRED, KeyMethod::Platform),
        ..support::account(CRED, "Ann", &multi_address())
    };
    let (_, next) = signed_in_holding(CRED2, KeyMethod::SecurityKey, vec![stored]);
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.sign_in_key, named(CRED2, KeyMethod::SecurityKey));
            let route = account.key_route().unwrap_or_else(|| unreachable!());
            assert_eq!(route.credential_id, CRED2, "the second key signs now");
        }
        other => panic!("expected the record re-saved, got {other:?}"),
    }
}

/// The sign-in succeeded; a failed write of the record must not undo it.
#[test]
fn a_failed_resave_still_opens_the_wallet() {
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    let (mut sut, _) = signed_in_holding(CRED, KeyMethod::Platform, vec![stored]);
    let next = sut.resolve(ShellResult::StorageFailed {
        message: "disk full".to_owned(),
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::CompleteOnboarding {
                mode: CompletionMode::SetWallet { .. }
            }]
        ),
        "expected the wallet to open anyway; got {next:?}"
    );
}

/// A wallet new to this device is saved naming its sign-in key — the place
/// the person chose, never the page (spec 102).
#[test]
fn a_recovered_wallet_is_saved_naming_its_sign_in_key() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::SecurityKey,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(
                account.sign_in_key,
                Some(SignInKey {
                    credential_id: CRED.to_owned(),
                    method: KeyMethod::SecurityKey,
                    transports: "internal".to_owned(),
                })
            );
            assert_eq!(account.signing_domain, "getvela.app");
            assert_eq!(
                account.signing_venue,
                vela_core::signing_venue::SigningVenue::InVela
            );
        }
        other => panic!("expected the recovered save, got {other:?}"),
    }
}

/// An address no browser would run a passkey ceremony on is not a page to
/// sign in on: nothing starts.
#[test]
fn an_address_no_browser_signs_on_starts_nothing() {
    let mut sut = mounted();
    let next = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: Some("http://192.168.1.4:8140/".to_owned()),
    });
    assert!(next.is_empty(), "{next:?}");
}

/// Spec 102 R3: "Sign in on my own signing page" on a custom domain runs the
/// ceremony on that page — its keys answer nowhere else — and the account is
/// saved on that domain, locked to that page (R2), every key naming the page
/// for older builds.
#[test]
fn a_custom_page_sign_in_is_saved_on_its_domain() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Hybrid,
        page: Some("http://localhost:8140".to_owned()),
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Hybrid,
            page: Some("http://localhost:8140/".to_owned()),
        }]
    );
    let mut assertion = support::assertion(CRED);
    assertion.signer_origin = Some("http://localhost:8140".to_owned());
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion,
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    let next = sut.resolve(ShellResult::RegistryKeyStatus {
        registered: true,
        unit_ids: vec![],
        verified_by: VerifiedBy::Gnosis,
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.signing_domain, "localhost");
            assert_eq!(
                account.signing_venue,
                vela_core::signing_venue::SigningVenue::Page {
                    url: "http://localhost:8140/".to_owned()
                }
            );
            assert_eq!(
                account.sign_in_key.as_ref().map(|key| key.method),
                Some(KeyMethod::Hybrid),
                "where the key lives, never the page"
            );
            assert!(account
                .keys
                .iter()
                .all(|key| key.signer_origin.as_deref() == Some("http://localhost:8140")));
            assert!(account.signing_plan().blocked.is_none());
        }
        other => panic!("expected the save, got {other:?}"),
    }
}

/// Spec 102 R3: a `getvela.app` page signs in in the app — there is nothing to
/// preview — and becomes the account's venue.
#[test]
fn signing_in_on_the_official_page_runs_in_the_app_and_keeps_the_page() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: Some("https://sign.getvela.app".to_owned()),
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::Platform,
            page: None,
        }]
    );
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    match sut
        .resolve(ShellResult::AccountsLoaded {
            accounts: vec![stored],
        })
        .as_slice()
    {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(
                account.signing_venue,
                vela_core::signing_venue::SigningVenue::official()
            );
            assert_eq!(account.signing_domain, "getvela.app");
        }
        other => panic!("expected the record re-saved, got {other:?}"),
    }
}

/// "This device" picked, a phone or a key on the desk answered: the record
/// keeps the choice and where the key was actually found.
#[test]
fn the_sign_in_records_where_the_key_answered_from() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
        page: None,
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    let mut assertion = support::assertion(CRED);
    assertion.authenticator_attachment = "cross-platform".to_owned();
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion,
        now_iso: NOW.to_owned(),
    });
    let stored = support::account(CRED, "Ann", "0x2222222222222222222222222222222222222222");
    match sut
        .resolve(ShellResult::AccountsLoaded {
            accounts: vec![stored],
        })
        .as_slice()
    {
        [ShellOperation::SaveAccount { account }] => {
            let key = account
                .sign_in_key
                .clone()
                .unwrap_or_else(|| unreachable!());
            assert_eq!(key.method, KeyMethod::Platform);
            assert_eq!(key.transports, "usb,nfc,ble,hybrid");
            let route = account.key_route().unwrap_or_else(|| unreachable!());
            assert_eq!(route.transports, "internal,usb,nfc,ble,hybrid");
        }
        other => panic!("expected the record re-saved, got {other:?}"),
    }
}

/// Issue #450 — a security key this device could not use is said as the
/// key's problem. "Biometric authentication is not available on this device"
/// sent an iPad owner holding a YubiKey off to look for Face ID.
#[test]
fn a_security_key_that_cannot_run_is_not_blamed_on_biometrics() {
    let prompt_after = |method: KeyMethod| {
        let mut sut = mounted();
        sut.dispatch(Event::SignIn { method, page: None });
        sut.resolve(ShellResult::PasskeySupport { supported: true });
        let next = sut.resolve(ShellResult::PasskeyFailed {
            kind: FailureKind::NotSupported,
            message: Some("Smart-card access is unavailable".to_owned()),
        });
        match next.as_slice() {
            [ShellOperation::Prompt { kind, .. }] => kind.clone(),
            other => panic!("expected one prompt, got {other:?}"),
        }
    };
    assert_eq!(
        prompt_after(KeyMethod::SecurityKey),
        PromptKind::NotSupportedLogin { security_key: true }
    );
    assert_eq!(
        prompt_after(KeyMethod::Platform),
        PromptKind::NotSupportedLogin {
            security_key: false
        },
        "a passkey on this device keeps the biometrics sheet"
    );
}

/// The flag is additive: a not-supported prompt from a core that predates it
/// still reads, as the device's.
#[test]
fn a_not_supported_prompt_without_the_flag_still_decodes() {
    let kind: PromptKind = serde_json::from_str(r#"{"type":"not_supported_login"}"#).unwrap();
    assert_eq!(
        kind,
        PromptKind::NotSupportedLogin {
            security_key: false
        }
    );
}

// ---------------------------------------------------------------------------
// The rebuild is offered only on Gnosis's verdict (two-signature rebuild of a
// multi-key wallet's key, 2026-10-09)
// ---------------------------------------------------------------------------

/// What one candidate's lookup answered, for the walks below.
#[derive(Clone, Copy, Debug)]
enum Answer {
    /// The lookup failed (`local`: it never left this device).
    Failed { local: bool },
    /// "No record", vouched for by `by`.
    NoRecord(VerifiedBy),
    /// "Registered, no groups", vouched for by `by`.
    NoGroups(VerifiedBy),
    /// A member of the two-key fixture wallet's group, listed by the index.
    Grouped,
}

fn result_of(answer: Answer) -> ShellResult {
    match answer {
        Answer::Failed { local } => ShellResult::IndexFailed {
            message: "registry unreachable".to_owned(),
            network: local,
        },
        Answer::NoRecord(verified_by) => ShellResult::RegistryKeyStatus {
            registered: false,
            unit_ids: vec![],
            verified_by,
        },
        Answer::NoGroups(verified_by) => ShellResult::RegistryKeyStatus {
            registered: true,
            unit_ids: vec![],
            verified_by,
        },
        Answer::Grouped => ShellResult::RegistryKeyStatus {
            registered: true,
            unit_ids: vec![3],
            verified_by: VerifiedBy::None,
        },
    }
}

/// Signed in on a device holding nothing; the first candidate is being asked.
fn matching() -> Sut {
    let mut sut = authenticated();
    let next = sut.resolve(ShellResult::AccountsLoaded { accounts: vec![] });
    assert!(matches!(
        next.as_slice(),
        [ShellOperation::RegistryQueryByPublicKey { .. }]
    ));
    sut
}

/// Answer both candidates in order; returns what followed the second.
fn answer_both(sut: &mut Sut, first: Answer, second: Answer) -> Vec<ShellOperation> {
    let next = sut.resolve(result_of(first));
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RegistryQueryByPublicKey { .. }]
        ),
        "after {first:?} the other candidate is asked; got {next:?}"
    );
    sut.resolve(result_of(second))
}

fn unreachable_prompt(local: bool) -> Vec<ShellOperation> {
    vec![ShellOperation::Prompt {
        kind: PromptKind::RegistryUnreachable { local },
        confirmable: true,
    }]
}

/// Nothing that rebuilds, publishes or saves.
fn assert_nothing_rebuilt(ops: &[ShellOperation]) {
    for op in ops {
        assert!(
            !matches!(
                op,
                ShellOperation::Prompt {
                    kind: PromptKind::RecoverOffer,
                    ..
                } | ShellOperation::SignProof { .. }
                    | ShellOperation::RegistryPublish { .. }
                    | ShellOperation::SaveAccount { .. }
                    | ShellOperation::CompleteOnboarding { .. }
            ),
            "nothing may be rebuilt, published or saved on an answer nobody vouched for; got {op:?}"
        );
    }
}

/// The reported case: the index and both chains unreachable. Both candidates
/// are asked, and the person is told the registry cannot be reached — never
/// offered the rebuild, which would make a one-key address.
#[test]
fn an_unreachable_registry_never_offers_the_rebuild() {
    let mut sut = matching();
    let next = answer_both(
        &mut sut,
        Answer::Failed { local: false },
        Answer::Failed { local: false },
    );
    assert_eq!(next, unreachable_prompt(false));
    assert_nothing_rebuilt(&next);
    assert!(sut.view().busy, "the flow waits on the person's answer");
}

/// Every lookup that failed never left this device: the sheet may say "check
/// your connection".
#[test]
fn lookups_that_never_left_the_device_say_so() {
    let mut sut = matching();
    let next = answer_both(
        &mut sut,
        Answer::Failed { local: true },
        Answer::Failed { local: true },
    );
    assert_eq!(next, unreachable_prompt(true));

    // One of them reached somebody: no claim about this device.
    let mut sut = matching();
    let next = answer_both(
        &mut sut,
        Answer::Failed { local: true },
        Answer::NoRecord(VerifiedBy::None),
    );
    assert_eq!(next, unreachable_prompt(false));
}

/// "No record" from the index alone (Gnosis silent), or from Ethereum (which
/// holds only what somebody copied there), is not a verdict.
#[test]
fn an_unverified_no_is_not_a_verdict() {
    for by in [VerifiedBy::None, VerifiedBy::Ethereum] {
        let mut sut = matching();
        let next = answer_both(&mut sut, Answer::NoRecord(by), Answer::NoRecord(by));
        assert_eq!(next, unreachable_prompt(false), "{by:?}");
        assert_nothing_rebuilt(&next);
    }
}

/// Gnosis saying neither key has a record IS the verdict — the one-key wallet
/// whose record never landed — and the rebuild is still offered.
#[test]
fn a_gnosis_verdict_still_offers_the_rebuild() {
    let mut sut = matching();
    let next = answer_both(
        &mut sut,
        Answer::NoRecord(VerifiedBy::Gnosis),
        Answer::NoRecord(VerifiedBy::Gnosis),
    );
    assert_eq!(
        next,
        vec![ShellOperation::Prompt {
            kind: PromptKind::RecoverOffer,
            confirmable: true,
        }]
    );
}

/// One candidate nobody could vouch for spoils the round, in either order:
/// it may be the real key, and it may belong to a group.
#[test]
fn one_unknown_candidate_poisons_the_verdict() {
    for (first, second) in [
        (
            Answer::NoRecord(VerifiedBy::Gnosis),
            Answer::Failed { local: false },
        ),
        (
            Answer::Failed { local: false },
            Answer::NoRecord(VerifiedBy::Gnosis),
        ),
        (
            Answer::NoRecord(VerifiedBy::Gnosis),
            Answer::NoRecord(VerifiedBy::None),
        ),
        (
            Answer::NoRecord(VerifiedBy::None),
            Answer::NoRecord(VerifiedBy::Gnosis),
        ),
    ] {
        let mut sut = matching();
        let next = answer_both(&mut sut, first, second);
        assert_eq!(next, unreachable_prompt(false), "{first:?} then {second:?}");
    }
}

/// A failed lookup on the first candidate does not end the round: the other
/// may resolve to the wallet's group, and then the multi-key wallet opens.
#[test]
fn an_index_failure_still_tries_the_other_candidate() {
    let mut sut = matching();
    let next = answer_both(&mut sut, Answer::Failed { local: true }, Answer::Grouped);
    assert_eq!(next, vec![ShellOperation::RegistryQueryUnit { unit_id: 3 }]);
    let next = sut.resolve(ShellResult::RegistryUnit {
        metadata_hex: unit_metadata_hex(),
        members: unit_members(),
    });
    match next.as_slice() {
        [ShellOperation::SaveAccount { account }] => {
            assert_eq!(account.address, multi_address());
        }
        other => panic!("expected the multi-key wallet's save, got {other:?}"),
    }
}

/// "Try again" re-asks the registry from the signature already made: no new
/// passkey prompt. A verdict this time behaves as it always did.
#[test]
fn retry_requeries_without_a_new_signature() {
    let mut sut = matching();
    answer_both(
        &mut sut,
        Answer::Failed { local: false },
        Answer::Failed { local: false },
    );
    let next = sut.resolve(ShellResult::PromptAnswered { accepted: true });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RegistryQueryByPublicKey { .. }]
        ),
        "the retry asks the registry again; got {next:?}"
    );
    for op in &next {
        assert!(
            !matches!(
                op,
                ShellOperation::AuthenticatePasskey { .. } | ShellOperation::SignProof { .. }
            ),
            "a retry never asks for a new signature"
        );
    }
    // The second round reaches Gnosis: the verdict decides, as before.
    let next = sut.resolve(result_of(Answer::NoRecord(VerifiedBy::Gnosis)));
    assert!(matches!(
        next.as_slice(),
        [ShellOperation::RegistryQueryByPublicKey { .. }]
    ));
    let next = sut.resolve(result_of(Answer::NoRecord(VerifiedBy::Gnosis)));
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: PromptKind::RecoverOffer,
                ..
            }]
        ),
        "a failure in the first round does not poison the second; got {next:?}"
    );
}

/// "Cancel" ends the attempt with nothing saved.
#[test]
fn declining_the_retry_persists_nothing() {
    let mut sut = matching();
    answer_both(
        &mut sut,
        Answer::Failed { local: false },
        Answer::Failed { local: false },
    );
    let next = sut.resolve(ShellResult::PromptAnswered { accepted: false });
    assert_nothing_rebuilt(&next);
    assert!(!sut.view().busy, "back to the welcome screen");
    assert!(sut.outstanding().is_empty());
}

/// The sibling hole: an index answering "registered, no groups" while no chain
/// can confirm it must not open a one-key wallet after ONE signature. Vela
/// never writes an entry without a group, so only Gnosis may say it.
#[test]
fn an_unverified_entry_without_groups_does_not_enter_single_key() {
    for by in [VerifiedBy::None, VerifiedBy::Ethereum] {
        let mut sut = matching();
        let next = answer_both(&mut sut, Answer::NoGroups(by), Answer::NoRecord(by));
        assert_eq!(next, unreachable_prompt(false), "{by:?}");
        assert_nothing_rebuilt(&next);
    }
    // On Gnosis's word it is the historical single-key wallet, as before.
    let mut sut = matching();
    let next = sut.resolve(result_of(Answer::NoGroups(VerifiedBy::Gnosis)));
    assert!(
        matches!(next.as_slice(), [ShellOperation::SaveAccount { .. }]),
        "got {next:?}"
    );
}

/// The harm the fix exists for, as a property: a member of a two-key wallet
/// signing in on a new device while the registry is down — in any mix of
/// failures and unvouched answers, retried as often as the person likes —
/// never saves any address but the wallet's own.
#[test]
fn a_multikey_member_offline_never_saves_another_address() {
    let degraded = [
        Answer::Failed { local: true },
        Answer::Failed { local: false },
        Answer::NoRecord(VerifiedBy::None),
        Answer::NoRecord(VerifiedBy::Ethereum),
        Answer::NoGroups(VerifiedBy::None),
        Answer::NoGroups(VerifiedBy::Ethereum),
    ];
    for first in degraded {
        for second in degraded {
            let mut sut = matching();
            let mut next = answer_both(&mut sut, first, second);
            // Retry twice more under the same outage…
            for _ in 0..2 {
                assert_eq!(
                    next.len(),
                    1,
                    "{first:?}/{second:?}: one prompt, got {next:?}"
                );
                assert_nothing_rebuilt(&next);
                let asked = sut.resolve(ShellResult::PromptAnswered { accepted: true });
                assert!(matches!(
                    asked.as_slice(),
                    [ShellOperation::RegistryQueryByPublicKey { .. }]
                ));
                next = answer_both(&mut sut, first, second);
            }
            // …then the registry comes back and lists the group.
            sut.resolve(ShellResult::PromptAnswered { accepted: true });
            let next = sut.resolve(result_of(Answer::Grouped));
            assert_eq!(next, vec![ShellOperation::RegistryQueryUnit { unit_id: 3 }]);
            let next = sut.resolve(ShellResult::RegistryUnit {
                metadata_hex: unit_metadata_hex(),
                members: unit_members(),
            });
            match next.as_slice() {
                [ShellOperation::SaveAccount { account }] => {
                    assert_eq!(account.address, multi_address(), "{first:?}/{second:?}");
                }
                other => panic!("{first:?}/{second:?}: expected the save, got {other:?}"),
            }
        }
    }
}

/// A shell that predates `verified_by` still decodes — and fails closed: its
/// "no record" is nobody's verdict.
#[test]
fn a_key_status_without_the_verifier_reads_as_unverified() {
    let decoded: ShellResult =
        serde_json::from_str(r#"{"type":"registry_key_status","registered":false,"unit_ids":[]}"#)
            .unwrap();
    assert_eq!(
        decoded,
        ShellResult::RegistryKeyStatus {
            registered: false,
            unit_ids: vec![],
            verified_by: VerifiedBy::None,
        }
    );
    let decoded: ShellResult = serde_json::from_str(
        r#"{"type":"registry_key_status","registered":false,"unit_ids":[],"verified_by":"gnosis"}"#,
    )
    .unwrap();
    assert!(matches!(
        decoded,
        ShellResult::RegistryKeyStatus {
            verified_by: VerifiedBy::Gnosis,
            ..
        }
    ));
    // And the new prompt's wire shape, `local` defaulting for an old reader.
    let prompt: PromptKind = serde_json::from_str(r#"{"type":"registry_unreachable"}"#).unwrap();
    assert_eq!(prompt, PromptKind::RegistryUnreachable { local: false });
}
