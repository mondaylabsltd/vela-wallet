//! Issue #409 — the session's landing watch.
//!
//! A one-key wallet is entered as soon as the registry has ACCEPTED its publish
//! (owner's decision, 2026-10-04). The landing finishes after the create screen
//! — and the onboarding core with it — is gone, so the app-resident session
//! confirms it: it reads the pending-upload outbox and, for every record
//! waiting only on a landing, waits on that registry task. Landed ⇒ the record
//! goes. Anything else ⇒ the record stays, and with it the sign-out warning.
//!
//! One rule above the others: confirming a landing late must never put a
//! passkey prompt in front of anyone. `every_watch_operation_is_a_read_or_a_local_write`
//! pins that against the machine's whole vocabulary.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::session::{Event, Session, SessionOperation as Op, SessionShellResult as Res};
use vela_core::app::shell::CompletionMode;
use vela_core::app::{
    Account, OutboxRecord, PendingPublish, PendingUpload, PendingUploadMember,
    RegistryPublishMember,
};

type Sut = DomainDriver<Session>;

const ADDR_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const CRED: &str = "credential-1";
const TASK: &str = "task-409";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn member(credential_id: &str) -> PendingUploadMember {
    PendingUploadMember {
        credential_id: credential_id.to_owned(),
        name: "Ann".to_owned(),
        public_key_hex: support::expected_public_key_hex(),
        attestation_object_hex: "a0".to_owned(),
        authenticator_attachment: "platform".to_owned(),
        transports: "internal".to_owned(),
        signer_origin: None,
    }
}

/// A pending record as the create flow writes it: one key, and — once the
/// registry accepted the publish — the task it was queued under.
fn record(credential_id: &str, task_id: Option<&str>) -> PendingUpload {
    PendingUpload {
        id: credential_id.to_owned(),
        name: "Ann".to_owned(),
        public_key_hex: support::expected_public_key_hex(),
        attestation_object_hex: "a0".to_owned(),
        created_at_iso: support::NOW.to_owned(),
        authenticator_attachment: "platform".to_owned(),
        transports: "internal".to_owned(),
        members: vec![member(credential_id)],
        task_id: task_id.map(str::to_owned),
        publish: None,
    }
}

/// ob-2: the publish a create writes into its record — every member proven,
/// the group key alongside — so the session can send it with no prompt.
fn publish_for(credential_ids: &[&str]) -> PendingPublish {
    PendingPublish {
        metadata_hex: "a1b2".to_owned(),
        members: credential_ids
            .iter()
            .map(|credential_id| RegistryPublishMember {
                credential_id: (*credential_id).to_owned(),
                public_key_hex: support::expected_public_key_hex(),
                attestation_hex: String::new(),
                authenticator_attachment: "platform".to_owned(),
                transports: "internal".to_owned(),
                proof: Some(support::member_proof(credential_id)),
            })
            .collect(),
        group_seed_hex: "5eed".repeat(16),
        group_public_key_hex: "04feed".to_owned(),
        page: None,
    }
}

/// A two-key wallet whose publish never landed, as the create now writes it.
fn unlanded(credential_id: &str) -> PendingUpload {
    let mut unlanded = record(credential_id, None);
    unlanded.members.push(PendingUploadMember {
        public_key_hex: support::second_public_key_hex(),
        ..member("second-key")
    });
    unlanded.publish = Some(publish_for(&[credential_id, "second-key"]));
    unlanded
}

fn answer_publish(sut: &mut Sut, result: Res) -> Vec<Op> {
    sut.resolve_matching(|op| matches!(op, Op::PublishPendingUpload { .. }), result)
}

fn outbox(records: Vec<PendingUpload>) -> Res {
    Res::PendingUploadsLoaded {
        records: records.into_iter().map(|r| OutboxRecord(Some(r))).collect(),
    }
}

fn wallet() -> Account {
    support::account(CRED, "Ann", ADDR_A)
}

/// The create flow's hand-off: a new one-key wallet, entered on acceptance.
/// Returns the machine with the watch's outbox read outstanding (the index
/// write already acknowledged).
fn established() -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount { account: wallet() },
    });
    assert_eq!(
        ops,
        vec![Op::SaveActiveIndex { index: 0 }, Op::LoadPendingUploads],
        "an establishment starts the landing watch"
    );
    sut.resolve_matching(
        |op| matches!(op, Op::SaveActiveIndex { .. }),
        Res::ActiveIndexSaved,
    );
    sut
}

fn answer_outbox(sut: &mut Sut, records: Vec<PendingUpload>) -> Vec<Op> {
    sut.resolve_matching(|op| *op == Op::LoadPendingUploads, outbox(records))
}

fn answer_landing(sut: &mut Sut, result: Res) -> Vec<Op> {
    sut.resolve_matching(|op| matches!(op, Op::AwaitRegistryLanding { .. }), result)
}

/// What a session operation DOES, by kind — every variant named, so a new one
/// cannot be added without someone deciding here whether it may run unseen.
fn is_read_or_local_write(op: &Op) -> bool {
    match op {
        // Reads.
        Op::LoadAccounts
        | Op::LoadActiveIndex
        | Op::CheckPendingUploads
        | Op::LoadPendingUploads
        | Op::AwaitRegistryLanding { .. } => true,
        // Local writes.
        Op::SaveAccount { .. }
        | Op::SaveActiveIndex { .. }
        | Op::ClearSignedInWallet
        | Op::RemoveAccount { .. }
        | Op::ClearExtensionCache
        | Op::RemovePendingUpload { .. } => true,
        // ob-2: a replay of proofs the keys made at creation — no prompt.
        Op::PublishPendingUpload { publish } => publish.replayable(),
    }
}

// ---------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------

/// The common path, measured on a Xiaomi: the wallet is entered ~7 s before
/// the landing. The session waits on the task, and only once it is `done`
/// removes the record — with nothing but a read and a local write.
#[test]
fn a_landing_confirmed_later_removes_the_record_with_no_passkey() {
    let mut sut = established();
    let mut asked = vec![Op::LoadPendingUploads];

    let ops = answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: TASK.to_owned()
        }]
    );
    asked.extend(ops);

    let ops = answer_landing(&mut sut, Res::RegistryLanded);
    assert_eq!(
        ops,
        vec![Op::RemovePendingUpload {
            credential_id: CRED.to_owned()
        }],
        "confirmed: only now may the record go"
    );
    asked.extend(ops);

    let ops = sut.resolve(Res::PendingUploadRemoved);
    assert!(ops.is_empty(), "nothing left to confirm; got {ops:?}");
    assert!(sut.outstanding().is_empty());

    assert!(
        asked.iter().all(is_read_or_local_write),
        "no prompt, no signature: {asked:?}"
    );
    assert!(sut.view().has_wallet, "the watch never touches the session");
}

/// The landing failed (or timed out, or the task is gone): the record stays,
/// so the sign-out dialog still carries its warning — the person can still not
/// sign out unwarned of a key the registry never confirmed.
#[test]
fn a_failed_landing_keeps_the_record_and_the_sign_out_warning() {
    let mut sut = established();
    answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);
    let ops = answer_landing(
        &mut sut,
        Res::RegistryLandingUnconfirmed {
            message: "Register failed: execution reverted".to_owned(),
            refused: None,
        },
    );
    assert!(
        ops.is_empty(),
        "an unconfirmed landing removes nothing; got {ops:?}"
    );
    assert!(sut.outstanding().is_empty());

    // The outbox still holds the record, and the sign-out check reads it.
    let ops = sut.dispatch(Event::SignOut);
    assert_eq!(ops, vec![Op::CheckPendingUploads]);
    sut.resolve(Res::PendingUploads { has_pending: true });
    let dialog = sut.view().sign_out.expect("the dialog opened");
    assert!(dialog.pending_upload_warning);
}

/// The app closed inside those seconds — or the registry was slow past the
/// shell's budget. The next launch's restore asks again, and confirms it.
#[test]
fn the_next_launch_confirms_a_landing_the_last_run_could_not_see() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![wallet()],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    assert!(
        ops.contains(&Op::LoadPendingUploads),
        "a restore into Active starts the watch; got {ops:?}"
    );

    let ops = answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: TASK.to_owned()
        }]
    );
    let ops = answer_landing(&mut sut, Res::RegistryLanded);
    assert_eq!(
        ops,
        vec![Op::RemovePendingUpload {
            credential_id: CRED.to_owned()
        }]
    );
}

/// Only a record waiting on nothing but its landing is settled by a read: one
/// key, a task. A record with no task still owes the registry a publish — that
/// takes a passkey, so it is left exactly as it was. A multi-key record is
/// never touched (it never carries a task; this one is forged to prove the
/// rule holds even if it did).
#[test]
fn only_accepted_one_key_records_are_confirmed() {
    let mut sut = established();
    let mut multi = record("multi", Some("task-multi"));
    multi.members.push(member("multi-second"));
    let ops = answer_outbox(
        &mut sut,
        vec![
            record("never-accepted", None),
            multi,
            record(CRED, Some(TASK)),
        ],
    );
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: TASK.to_owned()
        }],
        "the one accepted one-key record, and nothing else"
    );
}

/// Records are confirmed one at a time, and an unconfirmed one does not stop
/// the next.
#[test]
fn every_waiting_record_is_confirmed_in_turn() {
    let mut sut = established();
    let ops = answer_outbox(
        &mut sut,
        vec![record("c1", Some("t1")), record("c2", Some("t2"))],
    );
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: "t1".to_owned()
        }]
    );
    let ops = answer_landing(
        &mut sut,
        Res::RegistryLandingUnconfirmed {
            message: "Register timed out after 120s".to_owned(),
            refused: None,
        },
    );
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: "t2".to_owned()
        }]
    );
    let ops = answer_landing(&mut sut, Res::RegistryLanded);
    assert_eq!(
        ops,
        vec![Op::RemovePendingUpload {
            credential_id: "c2".to_owned()
        }],
        "the record removed is the one whose task landed"
    );
    assert!(sut.resolve(Res::PendingUploadRemoved).is_empty());
}

/// A key that landed has landed whoever is signed in: a sign-out while the
/// watch waits must not orphan the confirmation (`attempt` is bumped by it;
/// the watch keeps its own generation).
#[test]
fn the_watch_survives_a_sign_out() {
    let mut sut = established();
    answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);

    sut.dispatch(Event::SignOut);
    sut.resolve_matching(
        |op| *op == Op::CheckPendingUploads,
        Res::PendingUploads { has_pending: true },
    );
    let ops = sut.dispatch(Event::SignOutConfirmed);
    assert!(
        !ops.contains(&Op::RemovePendingUpload {
            credential_id: CRED.to_owned()
        }),
        "signing out never clears the outbox"
    );
    assert!(!sut.view().has_wallet);

    let ops = answer_landing(&mut sut, Res::RegistryLanded);
    assert_eq!(
        ops,
        vec![Op::RemovePendingUpload {
            credential_id: CRED.to_owned()
        }]
    );
}

/// A newer watch reads the outbox again, so an older watch's late answer is
/// dropped rather than acted on twice.
#[test]
fn a_newer_watch_supersedes_an_older_one() {
    let mut sut = established();
    answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);

    // Another establishment (a second wallet added) starts a fresh watch.
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: support::account("credential-2", "Bo", ADDR_A),
        },
    });
    assert!(ops.contains(&Op::LoadPendingUploads));

    // The first watch's task lands now — and is ignored.
    let late = answer_landing(&mut sut, Res::RegistryLanded);
    assert!(
        late.is_empty(),
        "a superseded watch asks nothing; got {late:?}"
    );

    // The new watch finds the same record and confirms it itself.
    let ops = answer_outbox(&mut sut, vec![record(CRED, Some(TASK))]);
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: TASK.to_owned()
        }]
    );
}

/// An outbox that cannot be read is not knowledge: nothing is removed and
/// nothing is waited on.
#[test]
fn an_outbox_that_cannot_be_read_changes_nothing() {
    let mut sut = established();
    let ops = sut.resolve_matching(
        |op| *op == Op::LoadPendingUploads,
        Res::PendingUploadsUnavailable,
    );
    assert!(ops.is_empty(), "got {ops:?}");
    assert!(sut.outstanding().is_empty());
}

/// A device's outbox holds whatever every build ever wrote there — and tests
/// seed it with `{"unconfirmed":true}`. One record this build cannot read must
/// cost only itself: the shells hand the outbox over verbatim, and refusing the
/// whole answer would strand every record that could have been confirmed.
#[test]
fn an_unreadable_record_costs_only_itself() {
    let valid = serde_json::to_value(record(CRED, Some(TASK))).expect("serializes");
    let wire = serde_json::json!({
        "type": "pending_uploads_loaded",
        "records": [{ "unconfirmed": true }, "not even an object", valid],
    });
    let result: Res = serde_json::from_value(wire).expect("the answer is never refused");
    match &result {
        Res::PendingUploadsLoaded { records } => {
            assert_eq!(records.len(), 3);
            assert_eq!(records[0], OutboxRecord(None));
            assert_eq!(records[1], OutboxRecord(None));
            assert_eq!(
                records[2]
                    .0
                    .as_ref()
                    .and_then(PendingUpload::awaits_landing),
                Some(TASK)
            );
        }
        other => panic!("got {other:?}"),
    }

    let mut sut = established();
    let ops = sut.resolve_matching(|op| *op == Op::LoadPendingUploads, result);
    assert_eq!(
        ops,
        vec![Op::AwaitRegistryLanding {
            task_id: TASK.to_owned()
        }]
    );
}

/// The task id survives the round trip through a shell's storage, and a record
/// written by an older build — no task — reads as one that still owes a
/// publish.
#[test]
fn the_task_id_round_trips_and_older_records_read_without_one() {
    let stored = serde_json::to_string(&record(CRED, Some(TASK))).expect("serializes");
    let read: PendingUpload = serde_json::from_str(&stored).expect("reads back");
    assert_eq!(read.task_id.as_deref(), Some(TASK));

    let older = serde_json::json!({
        "id": CRED,
        "publicKeyHex": "04ab",
        "attestationObjectHex": "a0",
        "createdAt": support::NOW,
    });
    let read: PendingUpload = serde_json::from_value(older).expect("the retired spelling");
    assert_eq!(read.task_id, None);
    assert_eq!(read.awaits_landing(), None);
}

/// ob-2: a record needs no account list to be finished. A restore into Empty
/// — a wallet whose account could not be written, a device signed out before
/// its landing — still reads the outbox and sends what it owes.
#[test]
fn a_restore_into_empty_still_finishes_the_outbox() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded { accounts: vec![] });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    assert_eq!(ops, vec![Op::LoadPendingUploads]);
    let ops = answer_outbox(&mut sut, vec![unlanded(CRED)]);
    assert!(
        matches!(ops.as_slice(), [Op::PublishPendingUpload { .. }]),
        "got {ops:?}"
    );
    assert_eq!(sut.view().unsynced.len(), 1);
    assert_eq!(sut.view().unsynced[0].name, "Ann", "named as it was made");
}

/// ob-2: a wallet whose publish never landed carries it in its record — every
/// member's creation-time proof and the group key. The session sends it as it
/// stands (no prompt, whatever the key count), and only a landing removes it.
#[test]
fn a_record_carrying_its_publish_is_sent_and_removed_once_landed() {
    let mut sut = established();
    let record = unlanded(CRED);
    let ops = answer_outbox(&mut sut, vec![record.clone()]);
    assert_eq!(
        ops,
        vec![Op::PublishPendingUpload {
            publish: record.publish.clone().expect("carried")
        }],
        "the very publish the create would have sent"
    );
    assert!(ops.iter().all(is_read_or_local_write), "no prompt: {ops:?}");
    let unsynced = sut.view().unsynced;
    assert_eq!(unsynced.len(), 1);
    assert_eq!(unsynced[0].name, "Ann");
    assert_eq!(unsynced[0].address, ADDR_A, "the account it is, by its id");
    assert!(unsynced[0].retrying);

    let ops = answer_publish(&mut sut, Res::RegistryLanded);
    assert_eq!(
        ops,
        vec![Op::RemovePendingUpload {
            credential_id: CRED.to_owned()
        }]
    );
    assert!(
        sut.view().unsynced.is_empty(),
        "landed: nothing left to name"
    );
    assert!(sut.resolve(Res::PendingUploadRemoved).is_empty());
}

/// ob-2: a publish that did not land keeps its record — named, still retrying
/// — and the shell's "try again" (connection back, its timer) runs a fresh
/// round. A round already running is never doubled.
#[test]
fn an_unlanded_publish_stays_and_is_tried_again() {
    let mut sut = established();
    assert!(
        sut.dispatch(Event::SyncPending).is_empty(),
        "a round is running: the outbox read is still out"
    );
    answer_outbox(&mut sut, vec![unlanded(CRED)]);
    assert!(
        sut.dispatch(Event::SyncPending).is_empty(),
        "nor while it publishes"
    );
    let ops = answer_publish(
        &mut sut,
        Res::RegistryLandingUnconfirmed {
            message: "Register failed: 500".to_owned(),
            refused: None,
        },
    );
    assert!(ops.is_empty(), "nothing removed; got {ops:?}");
    assert_eq!(sut.view().unsynced.len(), 1, "still owed, still named");

    let ops = sut.dispatch(Event::SyncPending);
    assert_eq!(ops, vec![Op::LoadPendingUploads], "a fresh round");
    let ops = answer_outbox(&mut sut, vec![unlanded(CRED)]);
    assert!(matches!(ops.as_slice(), [Op::PublishPendingUpload { .. }]));
}

/// A record from before ob-2 — no task, no publish — cannot be sent without a
/// prompt, so the session leaves it alone; it is still named (it still owes
/// the registry), but as one the session is not retrying.
#[test]
fn a_record_with_no_replayable_publish_is_named_but_not_sent() {
    let mut sut = established();
    let mut unproven = unlanded("unproven");
    if let Some(publish) = unproven.publish.as_mut() {
        publish.members[1].proof = None;
    }
    let ops = answer_outbox(&mut sut, vec![record("older", None), unproven]);
    assert!(
        ops.is_empty(),
        "nothing a read or a replay can settle; got {ops:?}"
    );
    let unsynced = sut.view().unsynced;
    assert_eq!(unsynced.len(), 2);
    assert!(unsynced.iter().all(|row| !row.retrying));
    assert!(
        unsynced.iter().all(|row| row.address.starts_with("0x")),
        "a record not held here is named by the address its keys derive"
    );
}

/// The publish survives the round trip through a shell's storage.
#[test]
fn the_publish_round_trips_through_storage() {
    let stored = serde_json::to_string(&unlanded(CRED)).expect("serializes");
    let read: PendingUpload = serde_json::from_str(&stored).expect("reads back");
    assert_eq!(read.publish, unlanded(CRED).publish);
    assert!(read.replayable_publish().is_some());
}

/// Belt and braces for the rule in the module doc: whatever the watch is
/// answered, every operation it can ever issue is a read or a local write.
#[test]
fn every_watch_operation_is_a_read_or_a_local_write() {
    for landing in [
        Res::RegistryLanded,
        Res::RegistryLandingUnconfirmed {
            message: "gone".to_owned(),
            refused: None,
        },
    ] {
        let mut sut = established();
        let mut asked = answer_outbox(&mut sut, vec![record(CRED, Some(TASK)), unlanded("c2")]);
        asked.extend(answer_landing(&mut sut, landing.clone()));
        if sut
            .outstanding()
            .iter()
            .any(|op| matches!(op, Op::RemovePendingUpload { .. }))
        {
            asked.extend(sut.resolve(Res::PendingUploadRemoved));
        }
        asked.extend(answer_publish(&mut sut, landing));
        if sut
            .outstanding()
            .iter()
            .any(|op| matches!(op, Op::RemovePendingUpload { .. }))
        {
            asked.extend(sut.resolve(Res::PendingUploadRemoved));
        }
        assert!(asked.iter().all(is_read_or_local_write), "{asked:?}");
    }
}

/// r18-8 (§28, §32): a publish this test build refused to send to the live
/// index is never asked again by itself — the record stays (still named),
/// and the shell's "try again" runs no round.
#[test]
fn a_publish_the_test_build_refused_is_not_tried_again_by_itself() {
    let mut sut = established();
    answer_outbox(&mut sut, vec![unlanded(CRED)]);
    let ops = answer_publish(
        &mut sut,
        Res::RegistryLandingUnconfirmed {
            message: "refused".to_owned(),
            refused: Some(true),
        },
    );
    assert!(ops.is_empty(), "nothing removed; got {ops:?}");
    assert_eq!(sut.view().unsynced.len(), 1, "still owed, still named");
    assert!(
        sut.dispatch(Event::SyncPending).is_empty(),
        "no automatic retry of a refused write"
    );
}
