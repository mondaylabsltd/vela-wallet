//! Rules of signing in and recovering, one test per rule.
//!
//! The sign-in flow has no end-to-end coverage today, so these are the only
//! automated tests that exercise its branches at all.

#![cfg(feature = "crux")]

mod support;

use support::{Driver, NOW};
use vela_core::app::login::{Event, Login};
use vela_core::app::shell::{CompletionMode, ProofPurpose, ShellOperation, ShellResult};
use vela_core::app::{Assertion, FailureKind, KeyMethod, PromptKind};

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
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(ShellResult::PasskeyAuthenticated {
        assertion: support::assertion(CRED),
        now_iso: NOW.to_owned(),
    });
    sut
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
        if let [ShellOperation::Wait { .. }] = sut
            .resolve(ShellResult::IndexTransportFailed)
            .as_slice()
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
    });
    sut.resolve(ShellResult::PasskeySupport { supported: true });

    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });

    assert!(next.is_empty(), "no prompt, no error state");
    assert!(!sut.view().busy);
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
    });
    let stale_prompt = sut.resolve(ShellResult::PasskeySupport { supported: false });
    assert!(matches!(
        stale_prompt.as_slice(),
        [ShellOperation::Prompt { .. }]
    ));

    // A new attempt starts while the alert is still up.
    let fresh = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
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
            method: KeyMethod::Platform
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
    });
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert_eq!(
        next,
        vec![ShellOperation::AuthenticatePasskey {
            method: KeyMethod::SecurityKey
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
    });
    let second = sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
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
/// derived from one key, so the sign-in fails rather than guessing.
#[test]
fn a_failed_unit_fetch_fails_the_sign_in_instead_of_guessing() {
    let mut sut = fetching_unit();
    let next = sut.resolve(ShellResult::IndexFailed {
        message: "offline".to_owned(),
        network: true,
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: PromptKind::SignInFailed { .. },
                ..
            }]
        ),
        "no single-key fallback exists for a known group; got {next:?}"
    );
}

/// A sibling credential of a locally stored multi-key wallet opens it
/// directly — any founding key matches, not just the first.
#[test]
fn a_sibling_credential_matches_the_local_multikey_account() {
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
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
            },
            vela_core::app::AccountKey {
                credential_id: CRED2.to_owned(),
                public_key_hex: support::second_public_key_hex(),
                name: "Backup".to_owned(),
                transports: "usb,nfc".to_owned(),
            },
        ],
        ..support::account(CRED, "Ann", &multi_address())
    };
    let next = sut.resolve(ShellResult::AccountsLoaded {
        accounts: vec![account],
    });
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
    };
    // user_name() is pub(crate); observe through the machine instead: a
    // local account match is not needed — the name only matters on save, so
    // assert via the matched-candidate path with a crafted assertion.
    // (Direct: the mod-level unit test covers the decode; this pins the
    // login-visible behavior.)
    let mut sut = mounted();
    sut.dispatch(Event::SignIn {
        method: KeyMethod::Platform,
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
