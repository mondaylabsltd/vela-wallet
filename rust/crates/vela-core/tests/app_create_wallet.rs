//! Rules of wallet creation, one test per rule.
//!
//! Every one of these was previously only reachable by clicking through a
//! browser with a virtual authenticator. Each test names the rule it pins.

#![cfg(feature = "crux")]

mod support;

use support::{Driver, NOW};
use vela_core::app::create_wallet::{CreateStage, CreateWallet, Event, SubmitLabel, ACK_COUNT};
use vela_core::app::shell::{ShellOperation, ShellResult};
use vela_core::app::{FailureKind, KeyMethod, StatusKey};

const CRED: &str = "credential-1";

type Sut = Driver<CreateWallet>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn filled(name: &str) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(Event::NameChanged {
        name: name.to_owned(),
    });
    for index in 0..ACK_COUNT {
        sut.dispatch(Event::AckToggled { index });
    }
    sut
}

const GROUP_SEED: &str = "5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed5eed";
const GROUP_KEY: &str = "04feedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed\
feedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeedfeed01";

fn group_key_generated() -> ShellResult {
    ShellResult::GroupKeyGenerated {
        seed_hex: GROUP_SEED.to_owned(),
        group_public_key_hex: GROUP_KEY.to_owned(),
    }
}

/// Form → group key minted → the (empty) key list → key 1 registered AND its
/// membership confirmed (interleaved: one create + one get per key) → the key
/// list. The set is frozen by `FinishKeys`, after which the publish needs NO
/// prompts.
fn registered(name: &str) -> Sut {
    let mut sut = filled(name);
    sut.dispatch(Event::Submit);
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert!(
        matches!(next.as_slice(), [ShellOperation::GenerateGroupKey]),
        "the group key anchors every member proof, so it is minted first; got {next:?}"
    );
    let next = sut.resolve(group_key_generated());
    assert!(
        next.is_empty(),
        "the group key lands on the key list; no ceremony starts before the \
         person picks a method for the first key; got {next:?}"
    );
    assert_eq!(sut.view().stage, CreateStage::AddKeys);
    assert!(sut.view().keys.is_empty());
    let next = sut.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::Platform,
    });
    match next.as_slice() {
        [ShellOperation::RegisterPasskey { name: display, .. }] => {
            assert_eq!(
                display, name,
                "key 1's provider display name IS the wallet name (N=1 stays \
                 byte-identical to the single-key flow)"
            );
        }
        other => panic!("expected the first registration, got {other:?}"),
    }
    let next = sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::registration(CRED),
        now_iso: NOW.to_owned(),
    });
    match next.as_slice() {
        [ShellOperation::SignMemberProof {
            credential_id,
            group_public_key_hex,
            transports,
            ..
        }] => {
            assert_eq!(credential_id, CRED);
            assert_eq!(group_public_key_hex, GROUP_KEY);
            // WHERE the key lives rides along with WHICH key it is.
            //
            // A `get()` whose allowCredentials entry carries no transports
            // leaves the platform to guess, and Android's Credential Manager
            // guesses "removable security key" — it drew "Connect your security
            // key" for a passkey living in Apple Passwords on another phone,
            // which is a dead end the person cannot answer (device-found
            // 2026-08-26). The fixture's authenticator reports `hybrid`, and
            // that is what must reach the shell.
            assert_eq!(
                transports, "hybrid,internal",
                "the confirmation must say where to look for the key"
            );
        }
        other => panic!("registration flows straight into its membership get; got {other:?}"),
    }
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });
    sut
}

/// …→ set frozen: the pending-record write is in flight.
fn finished(name: &str) -> Sut {
    let mut sut = registered(name);
    sut.dispatch(Event::FinishKeys);
    sut
}

/// …→ pending record written → the registry publish (one get per key) in
/// flight.
fn uploading(name: &str) -> Sut {
    let mut sut = finished(name);
    sut.resolve(ShellResult::PendingUploadSaved);
    sut
}

fn is_save_account(operation: &ShellOperation) -> bool {
    matches!(operation, ShellOperation::SaveAccount { .. })
}

// ---------------------------------------------------------------------------
// Form rules
// ---------------------------------------------------------------------------

/// FR-015 — the acknowledgment gate is a business rule, not a UI decoration.
#[test]
fn submit_requires_every_acknowledgment() {
    let mut sut = Sut::new();
    sut.dispatch(Event::NameChanged {
        name: "Ann".to_owned(),
    });
    for index in 0..ACK_COUNT - 1 {
        sut.dispatch(Event::AckToggled { index });
    }
    assert!(
        !sut.view().can_submit,
        "all but one box ticked must not pass"
    );

    sut.dispatch(Event::AckToggled {
        index: ACK_COUNT - 1,
    });
    assert!(sut.view().can_submit);
}

/// The form is the flow's FIRST screen, so the core has nowhere to send a
/// person who presses back there — and every host reads `can_go_back` to
/// decide between "the core steps back" and "I leave the flow". Reporting
/// `true` here made the back affordance dead on the name screen on all four
/// shells (device-found 2026-08-25).
#[test]
fn the_form_has_no_step_back_so_the_host_owns_leaving() {
    let mut sut = filled("Ann");
    assert!(
        !sut.view().can_go_back,
        "the name screen's back leaves the flow; it is not a core step"
    );

    let view_before = sut.view();
    sut.dispatch(Event::GoBack);
    let view_after = sut.view();
    assert_eq!(view_after.stage, view_before.stage);
    assert_eq!(view_after.name, view_before.name);
    assert_eq!(view_after.acks, view_before.acks);
}

/// The key list DOES have a step back: the form it came from. The drafts are
/// kept — the form re-entered with drafts is the "finish verification" state,
/// and its submit returns to this same list without re-registering anything.
#[test]
fn back_from_the_key_list_returns_to_the_form_and_keeps_the_drafts() {
    let mut sut = registered("Ann");
    assert_eq!(sut.view().stage, CreateStage::AddKeys);
    assert!(sut.view().can_go_back);

    sut.dispatch(Event::GoBack);
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::Form);
    assert_eq!(view.keys.len(), 1, "going back keeps the minted key");
    assert_eq!(view.submit_label, SubmitLabel::FinishVerify);
    assert!(!view.name_editable, "a draft pins the name");
    assert!(!view.can_go_back, "and the form still has nowhere back to");

    sut.dispatch(Event::Submit);
    assert_eq!(
        sut.view().stage,
        CreateStage::AddKeys,
        "submit returns to the list it came from, with no new ceremony"
    );
}

/// FR-015 — a name that cannot fit the WebAuthn user handle is rejected before
/// any ceremony starts, not deep inside one with a cryptic platform error.
#[test]
fn overlong_name_is_rejected_before_any_effect_is_requested() {
    let mut sut = filled("十个汉字就超过了预算啦"); // 33 UTF-8 bytes > 27

    assert!(sut.view().name_too_long);
    assert!(!sut.view().can_submit);
    assert!(
        sut.dispatch(Event::Submit).is_empty(),
        "no operation may be requested for a name that cannot be registered"
    );
}

// ---------------------------------------------------------------------------
// Registration and the proof of signing
// ---------------------------------------------------------------------------

/// FR-006 — nothing is persisted before the passkey proves it can sign. A
/// cancelled FIRST registration returns to the (empty) key list, where the
/// method choice lives — trying again with a different authenticator must not
/// require re-entering the name.
#[test]
fn cancelling_registration_persists_nothing() {
    let mut sut = filled("Ann");
    sut.dispatch(Event::Submit);
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(group_key_generated());
    sut.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::Platform,
    });
    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });

    assert!(
        next.is_empty(),
        "a cancelled registration asks the shell for nothing"
    );
    let view = sut.view();
    assert_eq!(view.status, Some(StatusKey::SetupCancelled));
    assert_eq!(view.stage, CreateStage::AddKeys);
    assert!(view.keys.is_empty(), "nothing was drafted");
    assert!(!view.busy);
}

// The old separate "verification" signature is gone: the register member
// proof (a single get) is itself proof the passkey can sign, so a cancelled
// publish resumes via RetryUpload — see
// retry_upload_resumes_at_the_publish_never_at_registration.

/// FR-006, issue #1 — a device-local credential would sign here and be invisible
/// everywhere else, so the flow stops with nothing written.
#[test]
fn non_discoverable_credential_aborts_without_persisting() {
    let mut sut = filled("Ann");
    sut.dispatch(Event::Submit);
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(group_key_generated());
    sut.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::Platform,
    });
    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::NotDiscoverable,
        message: None,
    });

    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: vela_core::app::PromptKind::NotDiscoverable,
                confirmable: false
            }]
        ),
        "the user is told to use a different provider, and nothing else happens"
    );
    assert_eq!(
        sut.view().submit_label,
        SubmitLabel::Create,
        "no draft kept"
    );
    assert!(sut.view().keys.is_empty(), "no draft kept");
}

// An incompatible provider is no longer caught by a separate verification
// signature (create mints an ES256 key by construction); an unusable key
// would fail the register member proof and land on the retry screen instead.

/// FR-010 — the pending record exists before the publish, so an interrupted
/// creation is retried.
#[test]
fn pending_record_is_written_before_the_first_upload() {
    let mut sut = registered("Ann");
    // Registration + confirmation land on the key list; nothing is derived
    // or persisted yet.
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::AddKeys);
    assert_eq!(view.keys.len(), 1);
    assert_eq!(view.keys[0].name, "Ann");
    assert!(view.keys[0].confirmed, "the interleaved get already ran");

    // Freezing the set derives and writes the pending record first.
    let next = sut.dispatch(Event::FinishKeys);
    match next.as_slice() {
        [ShellOperation::SavePendingUpload { record }] => {
            assert_eq!(record.id, CRED);
            assert_eq!(record.public_key_hex, support::expected_public_key_hex());
            assert_eq!(record.created_at_iso, NOW);
            assert_eq!(record.members.len(), 1, "one member per founding key");
            assert_eq!(record.members[0].credential_id, CRED);
        }
        other => panic!("expected the pending record to be written first, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Registry publish (option B: publish before entering)
// ---------------------------------------------------------------------------

/// The pending record written, the possession-proven publish is the next step.
#[test]
fn the_pending_record_is_followed_by_the_registry_publish() {
    let mut sut = finished("Ann");
    let next = sut.resolve(ShellResult::PendingUploadSaved);

    assert!(
        matches!(next.as_slice(), [ShellOperation::RegistryPublish { .. }]),
        "publishing runs before entry; got {next:?}"
    );
}

/// N=1 stays the historical single-key publish: one member, `key_names` is
/// exactly the wallet name.
#[test]
fn a_single_key_wallet_publishes_the_historical_payload() {
    let mut sut = finished("Ann");
    let next = sut.resolve(ShellResult::PendingUploadSaved);

    match next.as_slice() {
        [ShellOperation::RegistryPublish { members, .. }] => {
            assert_eq!(members.len(), 1);
            assert_eq!(members[0].credential_id, CRED);
            assert_eq!(
                members[0].public_key_hex,
                support::expected_public_key_hex()
            );
        }
        other => panic!("expected the publish, got {other:?}"),
    }
}

/// A published group clears the pending record, saves, and only then reveals
/// the address — the fund-safety ordering, now gated on the publish.
#[test]
fn a_published_group_removes_the_pending_saves_and_reveals_the_address() {
    let mut sut = uploading("Ann");
    assert!(sut.view().address.is_none());

    let next = sut.resolve(ShellResult::RegistryPublished);
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::RemovePendingUpload { .. }]
        ),
        "a published group clears its pending record; got {next:?}"
    );
    assert!(sut.view().address.is_none());

    let next = sut.resolve(ShellResult::PendingUploadRemoved);
    assert!(next.iter().any(is_save_account));
    assert!(
        sut.view().address.is_none(),
        "not even a requested save may reveal the address"
    );

    sut.resolve(ShellResult::AccountSaved);
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::Created);
    assert!(view.address.is_some());
}

/// A publish failure has no silent retry (it needs a signature), so it surfaces
/// the retry screen with the reason, keeping the passkey and its draft.
#[test]
fn a_failed_publish_shows_the_retry_screen() {
    let mut sut = uploading("Ann");

    let next = sut.resolve(ShellResult::IndexFailed {
        message: "offline".to_owned(),
        network: true,
    });
    assert!(next.is_empty() || next.iter().all(|op| !is_save_account(op)));

    let view = sut.view();
    assert_eq!(view.stage, CreateStage::SyncFailed);
    assert_eq!(view.sync_error_detail.as_deref(), Some("offline"));
    assert!(
        !view.can_go_back,
        "the back arrow is hidden while sync-failed"
    );
}

/// FR-013 — retry resumes at the publish. Re-registering would mint a second
/// passkey for a wallet that already has one.
#[test]
fn retry_upload_resumes_at_the_publish_never_at_registration() {
    let mut sut = uploading("Ann");
    sut.resolve(ShellResult::IndexFailed {
        message: "offline".to_owned(),
        network: true,
    });
    assert_eq!(sut.view().stage, CreateStage::SyncFailed);

    let requested = sut.dispatch(Event::RetryUpload);
    assert!(matches!(
        requested.as_slice(),
        [ShellOperation::RegistryPublish { .. }]
    ));
}

// ---------------------------------------------------------------------------
// Handover
// ---------------------------------------------------------------------------

/// FR-014 — entering the wallet is a state transition, not another ceremony.
#[test]
fn entering_the_wallet_requires_no_further_ceremony() {
    let mut sut = uploading("Ann");
    sut.resolve(ShellResult::RegistryPublished);
    sut.resolve(ShellResult::PendingUploadRemoved);
    sut.resolve(ShellResult::AccountSaved);

    let requested = sut.dispatch(Event::EnterWallet);
    match requested.as_slice() {
        [ShellOperation::CompleteOnboarding { mode }] => {
            assert!(matches!(
                mode,
                vela_core::app::shell::CompletionMode::AddAccount { .. }
            ));
        }
        other => panic!("expected completion, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Races (FR-033)
// ---------------------------------------------------------------------------

/// A result belonging to an abandoned draft must never resurrect it. This is
/// exactly the class of bug that was unreachable in a browser test.
#[test]
fn late_upload_result_after_start_over_is_ignored() {
    let mut sut = uploading("Ann");
    sut.dispatch(Event::StartOver);

    let view = sut.view();
    assert_eq!(view.submit_label, SubmitLabel::Create);
    assert!(view.name_editable);

    // The publish that was in flight when the user gave up now comes back.
    let next = sut.resolve(ShellResult::RegistryPublished);

    assert!(
        next.is_empty(),
        "the abandoned run may not request anything"
    );
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::Form);
    assert_eq!(view.submit_label, SubmitLabel::Create);
    assert!(view.address.is_none());
}

/// FR-024 — one ceremony at a time. A double tap must not mint two passkeys.
#[test]
fn submit_while_busy_is_a_no_op() {
    let mut sut = filled("Ann");
    let first = sut.dispatch(Event::Submit);
    let second = sut.dispatch(Event::Submit);

    assert_eq!(first.len(), 1);
    assert!(second.is_empty(), "the second tap asks for nothing");
    assert!(sut.view().busy);
}

/// A draft outlives a successful creation, and `Created` is not a "busy" stage —
/// so submit must be refused by *stage*, not merely by busyness. Otherwise a
/// stray submit after the wallet exists would start a second ceremony for it.
///
/// (Found by the extraction: invisible while the rule lived inside a component
/// that simply never rendered the button in that state.)
#[test]
fn submit_after_the_wallet_exists_is_refused() {
    let mut sut = uploading("Ann");
    sut.resolve(ShellResult::RegistryPublished);
    sut.resolve(ShellResult::PendingUploadRemoved);
    sut.resolve(ShellResult::AccountSaved);
    assert_eq!(sut.view().stage, CreateStage::Created);

    assert!(
        sut.dispatch(Event::Submit).is_empty(),
        "the wallet is already made; no further ceremony may start"
    );
    assert_eq!(sut.view().stage, CreateStage::Created);
}

/// The same guard on the retry button.
#[test]
fn retry_upload_is_ignored_unless_the_flow_is_sync_failed() {
    let mut sut = uploading("Ann");
    assert!(sut.dispatch(Event::RetryUpload).is_empty());
}

// ---------------------------------------------------------------------------
// Multi-key founding set
// ---------------------------------------------------------------------------

const CRED2: &str = "credential-2";

/// …→ two founding keys drafted AND confirmed, back at the key list.
fn two_keys(name: &str) -> Sut {
    let mut sut = registered(name);
    let next = sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    match next.as_slice() {
        [ShellOperation::RegisterPasskey {
            exclude_credential_ids,
            ..
        }] => {
            assert_eq!(
                exclude_credential_ids.as_slice(),
                [CRED.to_owned()],
                "the provider must refuse to reuse founding credentials"
            );
        }
        other => panic!("expected a second registration, got {other:?}"),
    }
    let next = sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::SignMemberProof { .. }]),
        "each key confirms while its authenticator is in hand; got {next:?}"
    );
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });
    sut
}

/// The add method is the person's choice, and it has to survive the round trip
/// through the shell — the ceremony is selected from it. A method that arrives
/// at the shell as `Platform` when the person asked for a security key runs the
/// wrong ceremony.
///
/// Since issue #207 it no longer LABELS the row: `kind` does, from what the
/// authenticator reported. The two fields answer different questions and are
/// checked here side by side so neither drifts into the other's job.
#[test]
fn the_chosen_add_method_reaches_the_shell_and_the_key_row() {
    let mut sut = registered("Ann");

    let next = sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    match next.as_slice() {
        [ShellOperation::RegisterPasskey { method, .. }] => {
            assert_eq!(
                *method,
                KeyMethod::SecurityKey,
                "the ceremony must be selected from what the person chose"
            );
        }
        other => panic!("expected a registration, got {other:?}"),
    }

    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });

    let keys = sut.view().keys;
    assert_eq!(keys.len(), 2);
    assert_eq!(
        keys[0].method,
        KeyMethod::Platform,
        "the helper minted key 1 through the platform method, and the row says so"
    );
    assert_eq!(
        keys[1].method,
        KeyMethod::SecurityKey,
        "the choice must survive the round trip — it is what routes the ceremony"
    );
    // …and `kind` is the other half of issue #207's split: the fixture
    // authenticator reports `platform`/`hybrid,internal`, so that is what the
    // ROW says, whatever was tapped. A row that drew the tap could show a
    // hardware fob for a passkey that lives on this laptop.
    assert_eq!(keys[1].kind, KeyMethod::Platform, "the row says what it IS");
}

/// Spec 075: a key minted on the Clear Signer page LIVES behind that page, and
/// the row has to say so.
///
/// The page runs the ceremony in a browser, so the authenticator's report is
/// `platform` — the device pass of 2026-09-22 found the row drawing a key made
/// on a page as the built-in passkey of the phone, which is the wrong side of
/// the page and the one place this wallet cannot reach it.
#[test]
fn a_key_minted_on_the_page_is_drawn_as_living_there() {
    let mut sut = registered("Ann");

    sut.dispatch(Event::AddKey {
        name: "Page".to_owned(),
        method: KeyMethod::ClearSigner,
    });
    let mut registration = support::second_registration(CRED2);
    registration.signer_origin = Some("https://sign.getvela.app".to_owned());
    sut.resolve(ShellResult::PasskeyRegistered {
        registration,
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });

    let keys = sut.view().keys;
    assert_eq!(
        keys[1].method,
        KeyMethod::ClearSigner,
        "the choice routes the ceremony"
    );
    assert_eq!(
        keys[1].kind,
        KeyMethod::ClearSigner,
        "and the row says where the key lives — the page, not this device"
    );
}

/// Spec 075, ruling 2026-09-23: a wallet's keys must share one relying party.
///
/// The registry stores ONE `rpId` per unit and every member's proof carries
/// `sha256(rpId)` from its own authenticator, so a set spread over two sites
/// can never be proved. The first key decides, and the rest of the choices
/// narrow to match — refused here rather than discovered at the publish, when
/// the person would already be holding a passkey they cannot use.
#[test]
fn the_first_key_decides_where_the_rest_may_come_from() {
    // A key on somebody's own deployment: only that page may mint the rest.
    let mut sut = registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Page".to_owned(),
        method: KeyMethod::ClearSigner,
    });
    let mut registration = support::registration("credential-2");
    registration.signer_origin = Some("http://localhost:8140".to_owned());
    sut.resolve(ShellResult::PasskeyRegistered {
        registration,
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });

    // …but this set's FIRST key was minted by the helper on the platform, so
    // the set belongs to getvela.app and every route is still open.
    let view = sut.view();
    assert_eq!(view.key_relying_party.as_deref(), Some("getvela.app"));
    assert_eq!(view.add_methods.len(), 4, "nothing is ruled out yet");

    // A set whose FIRST key lives behind somebody's own page is different:
    // build one from the form up, with that origin on key 1.
    let mut own = filled("Ann");
    own.dispatch(Event::Submit);
    own.resolve(ShellResult::PasskeySupport { supported: true });
    own.resolve(group_key_generated());
    own.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::ClearSigner,
    });
    let mut first = support::registration(CRED);
    first.signer_origin = Some("http://localhost:8140".to_owned());
    own.resolve(ShellResult::PasskeyRegistered {
        registration: first,
        now_iso: NOW.to_owned(),
    });
    own.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });

    let view = own.view();
    assert_eq!(view.key_relying_party.as_deref(), Some("localhost"));
    assert_eq!(
        view.key_signer_origin.as_deref(),
        Some("http://localhost:8140")
    );
    assert_eq!(
        view.add_methods,
        vec![KeyMethod::ClearSigner],
        "only the page that minted the first key can mint another of its domain"
    );

    // And the machine refuses the others even if a shell asks anyway: minting
    // one would leave a passkey that can never join this wallet.
    let before = own.view().keys.len();
    let asked = own.dispatch(Event::AddKey {
        name: "Second".to_owned(),
        method: KeyMethod::Platform,
    });
    assert!(asked.is_empty(), "no ceremony is started: {asked:?}");
    assert_eq!(own.view().keys.len(), before, "and no draft appears");
}

/// The creation-time confirmation must run on the route that MINTED the key.
///
/// `SignMemberProof` is a `get()` against the credential the previous step just
/// created, so it has to reach the same authenticator. A key minted on a phone
/// over caBLE lives on that phone and nowhere else: confirming it against the
/// USB port cannot succeed, and a shell that is its own CTAP client has only
/// this field to route from — `transports` is for platforms that route for
/// themselves. The desktop hard-coded a security key here, which made a
/// scan-created wallet impossible to confirm.
#[test]
fn the_confirmation_runs_on_the_route_that_minted_the_key() {
    let mut sut = registered("Ann");

    let next = sut.dispatch(Event::AddKey {
        name: "Phone".to_owned(),
        method: KeyMethod::Hybrid,
    });
    assert!(
        matches!(next.as_slice(), [ShellOperation::RegisterPasskey { .. }]),
        "expected a registration, got {next:?}"
    );

    let next = sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    match next.as_slice() {
        [ShellOperation::SignMemberProof { method, .. }] => {
            assert_eq!(
                *method,
                KeyMethod::Hybrid,
                "the confirmation must return to the authenticator that minted the key"
            );
        }
        other => panic!("expected the membership confirmation, got {other:?}"),
    }

    // The row's own retry carries it too — a cancelled confirmation must not
    // come back on a different route than the one that can answer it.
    sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });
    let next = sut.dispatch(Event::ConfirmKey { index: 1 });
    match next.as_slice() {
        [ShellOperation::SignMemberProof { method, .. }] => {
            assert_eq!(
                *method,
                KeyMethod::Hybrid,
                "the retry must run on the same route as the first attempt"
            );
        }
        other => panic!("expected the per-key confirmation retry, got {other:?}"),
    }
}

/// The FIRST key's method is the person's choice too. This is the Xiaomi
/// lock-out fix (device-found 2026-08-26): the core used to mint key 1 with
/// `KeyMethod::default()` before the key screen existed, so on an OEM whose
/// system sheet cannot reach a security key, a hardware-key owner could not
/// create a wallet at all.
#[test]
fn the_first_key_carries_the_persons_method_choice() {
    let mut sut = filled("Ann");
    sut.dispatch(Event::Submit);
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(group_key_generated());

    let next = sut.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::SecurityKey,
    });
    match next.as_slice() {
        [ShellOperation::RegisterPasskey { method, name, .. }] => {
            assert_eq!(*method, KeyMethod::SecurityKey);
            assert_eq!(name, "Ann", "key 1's display name is the wallet name");
        }
        other => panic!("expected the first registration, got {other:?}"),
    }

    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::registration(CRED),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });
    let keys = sut.view().keys;
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].method, KeyMethod::SecurityKey);
    assert_eq!(keys[0].name, "Ann", "row 0's label is the wallet name");
}

/// The multi-key oracle: what the FULL founding set derives to.
fn expected_multi_address() -> String {
    let keys = [
        vela_core::safe::parse_public_key(&support::expected_public_key_hex()).unwrap(),
        vela_core::safe::parse_public_key(&support::second_public_key_hex()).unwrap(),
    ];
    vela_core::safe::compute_safe_address_multi(&keys)
        .unwrap()
        .address
}

/// A two-key wallet publishes both members in founding order and derives the
/// address from the FULL set — never from keys[0] alone.
#[test]
fn a_two_key_wallet_publishes_both_members_and_the_multi_address() {
    let mut sut = two_keys("Ann");
    assert_eq!(sut.view().keys.len(), 2);
    assert_eq!(sut.view().keys[1].name, "Backup");

    let next = sut.dispatch(Event::FinishKeys);
    match next.as_slice() {
        [ShellOperation::SavePendingUpload { record }] => {
            assert_eq!(
                record.id, CRED,
                "the pending record keys off the pinned first key"
            );
            assert_eq!(record.members.len(), 2);
            assert_eq!(record.members[0].credential_id, CRED);
            assert_eq!(record.members[1].credential_id, CRED2);
            assert_eq!(record.members[1].name, "Backup");
        }
        other => panic!("expected the pending record, got {other:?}"),
    }

    let next = sut.resolve(ShellResult::PendingUploadSaved);
    match next.as_slice() {
        [ShellOperation::RegistryPublish { members, .. }] => {
            assert_eq!(members.len(), 2, "one possession proof per founding key");
            assert_eq!(members[0].credential_id, CRED);
            assert_eq!(members[1].credential_id, CRED2);
        }
        other => panic!("expected the publish, got {other:?}"),
    }

    sut.resolve(ShellResult::RegistryPublished);
    let requested = sut.resolve(ShellResult::PendingUploadRemoved);
    match requested
        .iter()
        .find(|op| matches!(op, ShellOperation::SaveAccount { .. }))
    {
        Some(ShellOperation::SaveAccount { account }) => {
            assert_eq!(account.address, expected_multi_address());
            assert_eq!(account.keys.len(), 2);
            assert_eq!(account.id, CRED);
            assert_eq!(account.public_key_hex, support::expected_public_key_hex());
        }
        other => panic!("expected the account save, got {other:?}"),
    }
}

/// Cancelling an ADDED key's ceremony keeps the existing drafts — the minted
/// passkeys are real; only StartOver abandons them.
#[test]
fn cancelling_an_added_key_keeps_the_existing_drafts() {
    let mut sut = registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });
    assert!(next.is_empty());

    let view = sut.view();
    assert_eq!(view.stage, CreateStage::AddKeys);
    assert_eq!(view.keys.len(), 1, "key 1 survives the cancelled add");
    assert_eq!(view.status, Some(StatusKey::SetupCancelled));
}

/// A drafted extra key can be removed; the pinned first key cannot.
#[test]
fn remove_key_drops_extras_but_never_the_first() {
    let mut sut = two_keys("Ann");
    assert!(sut.dispatch(Event::RemoveKey { index: 0 }).is_empty());
    assert_eq!(sut.view().keys.len(), 2, "index 0 is not removable");

    sut.dispatch(Event::RemoveKey { index: 1 });
    assert_eq!(sut.view().keys.len(), 1);

    // The set still freezes fine as a single key afterwards.
    let next = sut.dispatch(Event::FinishKeys);
    assert!(matches!(
        next.as_slice(),
        [ShellOperation::SavePendingUpload { .. }]
    ));
}

/// A publish failure on a multi-key set keeps every draft and retries the
/// FULL publish — all member proofs run again.
#[test]
fn a_failed_multi_publish_retries_every_member() {
    let mut sut = two_keys("Ann");
    sut.dispatch(Event::FinishKeys);
    sut.resolve(ShellResult::PendingUploadSaved);
    sut.resolve(ShellResult::IndexFailed {
        message: "offline".to_owned(),
        network: true,
    });
    assert_eq!(sut.view().stage, CreateStage::SyncFailed);

    let requested = sut.dispatch(Event::RetryUpload);
    match requested.as_slice() {
        [ShellOperation::RegistryPublish { members, .. }] => {
            assert_eq!(members.len(), 2);
        }
        other => panic!("expected the full republish, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Interleaved confirmation (create → sign, per key)
// ---------------------------------------------------------------------------

/// A cancelled membership get leaves the key DRAFTED but unconfirmed: its row
/// offers a retry, and the set cannot be frozen until every key confirmed.
#[test]
fn a_cancelled_confirmation_gates_finish_and_retries_per_row() {
    let mut sut = registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    // The membership get for key 2 is cancelled.
    let next = sut.resolve(ShellResult::PasskeyFailed {
        kind: FailureKind::Cancelled,
        message: None,
    });
    assert!(next.is_empty());
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::AddKeys);
    assert_eq!(view.keys.len(), 2, "the minted passkey stays drafted");
    assert!(view.keys[0].confirmed);
    assert!(!view.keys[1].confirmed);
    assert!(!view.can_finish, "an unconfirmed key must gate the freeze");
    assert!(
        sut.dispatch(Event::FinishKeys).is_empty(),
        "freezing with an unconfirmed key is refused"
    );

    // The row's own retry re-runs exactly that key's confirmation.
    let next = sut.dispatch(Event::ConfirmKey { index: 1 });
    match next.as_slice() {
        [ShellOperation::SignMemberProof { credential_id, .. }] => {
            assert_eq!(credential_id, CRED2);
        }
        other => panic!("expected the per-key confirmation retry, got {other:?}"),
    }
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2-retry"),
    });
    let view = sut.view();
    assert!(view.keys[1].confirmed);
    assert!(view.can_finish);
}

/// Confirming an already-confirmed key is a no-op — no wasted prompt.
#[test]
fn confirming_a_confirmed_key_asks_for_nothing() {
    let mut sut = registered("Ann");
    assert!(sut.dispatch(Event::ConfirmKey { index: 0 }).is_empty());
}

/// A duplicate authenticator (same credential material behind a new id) is
/// refused THE MOMENT it registers — not at FinishKeys.
#[test]
fn a_duplicate_founding_key_is_refused_at_registration() {
    let mut sut = registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    // The provider returns the SAME public key under a different credential.
    let next = sut.resolve(ShellResult::PasskeyRegistered {
        registration: vela_core::app::Registration {
            credential_id: CRED2.to_owned(),
            ..support::registration(CRED)
        },
        now_iso: NOW.to_owned(),
    });
    assert!(
        matches!(
            next.as_slice(),
            [ShellOperation::Prompt {
                kind: vela_core::app::PromptKind::CreateFailed { .. },
                ..
            }]
        ),
        "a duplicate key is refused immediately; got {next:?}"
    );
    let view = sut.view();
    assert_eq!(view.keys.len(), 1, "the duplicate was never drafted");
    assert_eq!(view.stage, CreateStage::AddKeys);
}

/// StartOver abandons the group key too: the next run gets a fresh one, so a
/// stale seed can never anchor a new wallet's proofs.
#[test]
fn start_over_mints_a_fresh_group_key() {
    let mut sut = registered("Ann");
    sut.dispatch(Event::StartOver);
    // Name and acks survive StartOver; only the drafts and group key reset.
    sut.dispatch(Event::Submit);
    let next = sut.resolve(ShellResult::PasskeySupport { supported: true });
    assert!(
        matches!(next.as_slice(), [ShellOperation::GenerateGroupKey]),
        "a fresh run mints a fresh group key; got {next:?}"
    );
}

// ---------------------------------------------------------------------------
// Second-key gate (a device-bound sole key must not become a wallet alone)
// ---------------------------------------------------------------------------

/// Register key 1 as a DEVICE-BOUND credential (no BE/BS) and confirm it.
fn device_bound_registered(name: &str) -> Sut {
    let mut sut = filled(name);
    sut.dispatch(Event::Submit);
    sut.resolve(ShellResult::PasskeySupport { supported: true });
    sut.resolve(group_key_generated());
    sut.dispatch(Event::AddKey {
        name: String::new(),
        method: KeyMethod::Platform,
    });
    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::device_bound_registration(CRED),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k1"),
    });
    sut
}

/// A sole device-bound key is one lost device from an unrecoverable wallet:
/// the freeze is refused and the view says a second key is needed.
#[test]
fn a_sole_device_bound_key_cannot_finish_alone() {
    let mut sut = device_bound_registered("Ann");
    let view = sut.view();
    assert_eq!(view.stage, CreateStage::AddKeys);
    assert!(view.keys[0].confirmed);
    assert!(!view.keys[0].synced, "flags 0x45 carries no BS bit");
    assert!(view.needs_second_key);
    assert!(!view.can_finish);
    assert!(
        sut.dispatch(Event::FinishKeys).is_empty(),
        "freezing a sole unsynced key is refused"
    );
}

/// Adding ANY second key — even another device-bound one — breaks the single
/// point of failure and unlocks the freeze.
#[test]
fn a_second_key_satisfies_the_device_bound_gate() {
    let mut sut = device_bound_registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });
    let view = sut.view();
    assert!(!view.needs_second_key);
    assert!(view.can_finish);
    assert!(matches!(
        sut.dispatch(Event::FinishKeys).as_slice(),
        [ShellOperation::SavePendingUpload { .. }]
    ));
}

/// …and removing back down to the sole device-bound key re-arms the gate.
#[test]
fn removing_back_to_a_sole_device_bound_key_rearms_the_gate() {
    let mut sut = device_bound_registered("Ann");
    sut.dispatch(Event::AddKey {
        name: "Backup".to_owned(),
        method: KeyMethod::SecurityKey,
    });
    sut.resolve(ShellResult::PasskeyRegistered {
        registration: support::second_registration(CRED2),
        now_iso: NOW.to_owned(),
    });
    sut.resolve(ShellResult::MemberProofSigned {
        proof: support::member_proof("k2"),
    });
    sut.dispatch(Event::RemoveKey { index: 1 });
    let view = sut.view();
    assert!(view.needs_second_key);
    assert!(!view.can_finish);
}

/// A SYNCED sole key is the common happy path and is never gated; the row
/// also carries the sync signal for the UI badge.
#[test]
fn a_synced_sole_key_finishes_alone() {
    let sut = registered("Ann");
    let view = sut.view();
    assert!(
        view.keys[0].synced,
        "the default fixture is a synced passkey"
    );
    assert!(!view.needs_second_key);
    assert!(view.can_finish);
}
