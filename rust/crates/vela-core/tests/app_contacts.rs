//! Rules of the address book, one test per rule.
//!
//! Ports the jest vectors from `src/__tests__/services/contacts.test.ts` and
//! `contact-io.test.ts` (the import half — file parsing stays in the shell),
//! plus the trust/risk semantics from `RecipientTrust.tsx` and
//! `recipient-risk.ts`. The TS `_writeChain` write lock and the implicit
//! `clearContactsCache()` have no equivalents here: the core is
//! single-threaded and the account switch is an explicit event.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::contacts::{
    contact_display_name, is_address, matches_query, sort_contacts, Contact, ContactGroup,
    ContactGroupInput, ContactHistoryTx, ContactIdentity, ContactImportEntry, ContactImportGroup,
    ContactImportReport, ContactKind, ContactOperation as Op, ContactSaveInput,
    ContactShellResult as Res, ContactSource, ContactTombstone, ContactTxKind, Contacts,
    ContactsView,
};

type Sut = DomainDriver<Contacts>;

const A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const ME: &str = "0x1111111111111111111111111111111111111111";
const ZERO: &str = "0x0000000000000000000000000000000000000000";

#[test]
fn fixture_addresses_are_well_formed() {
    for addr in [A, B, C, ME, ZERO] {
        assert!(is_address(addr), "bad fixture: {addr}");
    }
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn tx(kind: Option<ContactTxKind>, to: &str, ts: f64) -> ContactHistoryTx {
    ContactHistoryTx {
        kind,
        to: Some(to.to_owned()),
        to_name: None,
        timestamp_ms: Some(ts),
    }
}

fn send(to: &str, ts: f64) -> ContactHistoryTx {
    tx(Some(ContactTxKind::Send), to, ts)
}

fn named_send(to: &str, ts: f64, name: &str) -> ContactHistoryTx {
    ContactHistoryTx {
        to_name: Some(name.to_owned()),
        ..send(to, ts)
    }
}

fn manual(address: &str, name: Option<&str>, favorite: bool, last_used: f64) -> Contact {
    Contact {
        address: address.to_owned(),
        name: name.map(str::to_owned),
        resolved_name: None,
        resolved_source: None,
        kind: ContactKind::Unknown,
        favorite,
        note: None,
        tx_count: 0,
        last_used_ms: last_used,
        first_seen_ms: last_used,
        source: ContactSource::Manual,
    }
}

fn group(id: &str, name: &str, members: &[&str]) -> ContactGroup {
    ContactGroup {
        id: id.to_owned(),
        name: name.to_owned(),
        color: None,
        members: members.iter().map(|m| (*m).to_owned()).collect(),
    }
}

fn save_input(address: &str, name: Option<&str>) -> ContactSaveInput {
    ContactSaveInput {
        address: address.to_owned(),
        name: name.map(str::to_owned),
        note: None,
        favorite: None,
        kind: None,
        resolved_name: None,
        resolved_source: None,
    }
}

fn gin(id: Option<&str>, name: &str, members: Option<&[&str]>) -> ContactGroupInput {
    ContactGroupInput {
        id: id.map(str::to_owned),
        name: name.to_owned(),
        color: None,
        members: members.map(|m| m.iter().map(|a| (*a).to_owned()).collect()),
    }
}

fn entry(address: &str, name: Option<&str>) -> ContactImportEntry {
    ContactImportEntry {
        address: address.to_owned(),
        name: name.map(str::to_owned),
        note: None,
        favorite: None,
    }
}

fn identity(name: &str, source: &str) -> ContactIdentity {
    ContactIdentity {
        name: name.to_owned(),
        source: source.to_owned(),
    }
}

use vela_core::app::contacts::Event;

/// Boot a session for `ME` and land the given store + history.
fn booted(
    saved: Vec<Contact>,
    tombstones: Vec<ContactTombstone>,
    groups: Vec<ContactGroup>,
    history: Vec<ContactHistoryTx>,
) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountSwitched {
        my_address: Some(ME.to_owned()),
    });
    assert_eq!(ops, vec![Op::ReadStore, Op::LoadSendHistory]);
    let ops = sut.resolve(Res::StoreLoaded {
        contacts: saved,
        tombstones,
        groups,
    });
    assert!(ops.is_empty());
    let ops = sut.resolve(Res::HistoryLoaded { txs: history });
    assert!(ops.is_empty());
    sut
}

fn booted_empty() -> Sut {
    booted(vec![], vec![], vec![], vec![])
}

/// Acknowledge `count` best-effort writes (they never change state).
fn ack_writes(sut: &mut Sut, count: usize) {
    for _ in 0..count {
        let ops = sut.resolve(Res::Written);
        assert!(ops.is_empty(), "a write ack must not trigger new work");
    }
}

fn addresses(view: &ContactsView) -> Vec<String> {
    view.contacts.iter().map(|c| c.address.clone()).collect()
}

fn find<'v>(view: &'v ContactsView, addr: &str) -> &'v Contact {
    view.contacts
        .iter()
        .find(|c| c.address == addr)
        .unwrap_or_else(|| panic!("contact {addr} not in view"))
}

// ---------------------------------------------------------------------------
// Boot & session boundaries
// ---------------------------------------------------------------------------

/// Session start reads all three stores and the send history, and nothing
/// renders as loaded until the store answers.
#[test]
fn boot_reads_stores_and_history() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountSwitched {
        my_address: Some(ME.to_owned()),
    });
    assert_eq!(ops, vec![Op::ReadStore, Op::LoadSendHistory]);
    assert!(!sut.view().loaded);

    sut.resolve(Res::StoreLoaded {
        contacts: vec![manual(A, Some("Alice"), false, 1_000.0)],
        tombstones: vec![],
        groups: vec![],
    });
    assert!(sut.view().loaded);
    assert_eq!(addresses(&sut.view()), vec![A.to_owned()]);
}

/// The TS equivalent was every mutator `await`ing its lazy load; here a
/// mutation before the store answered is dropped, never applied to an
/// unloaded book.
#[test]
fn mutations_before_load_are_dropped() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::Save {
            input: save_input(A, Some("Alice")),
            now_ms: 1_000.0,
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::GroupSave {
            input: gin(None, "Team", None),
        })
        .is_empty());

    // Mid-boot (store still in flight) is equally unloaded.
    sut.dispatch(Event::AccountSwitched {
        my_address: Some(ME.to_owned()),
    });
    assert!(sut
        .dispatch(Event::Delete {
            address: A.to_owned(),
            now_ms: 1_000.0,
        })
        .is_empty());
    assert!(!sut.view().loaded);
    assert!(sut.view().contacts.is_empty());
}

/// Integration note: a previous account's read landing after a switch is
/// dropped — that IS the "books never cross accounts" rule.
#[test]
fn stale_results_from_a_previous_account_are_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        my_address: Some(ME.to_owned()),
    });
    sut.dispatch(Event::AccountSwitched {
        my_address: Some(B.to_owned()),
    });

    // The FIRST session's store + history answer late — both stale.
    let ops = sut.resolve(Res::StoreLoaded {
        contacts: vec![manual(A, Some("Old Book Alice"), false, 1_000.0)],
        tombstones: vec![],
        groups: vec![],
    });
    assert!(ops.is_empty());
    assert!(
        !sut.view().loaded,
        "a stale store read must not load the book"
    );
    sut.resolve(Res::HistoryLoaded {
        txs: vec![send(A, 100.0)],
    });
    assert!(sut.view().contacts.is_empty());

    // The CURRENT session's answers land normally.
    sut.resolve(Res::StoreLoaded {
        contacts: vec![],
        tombstones: vec![],
        groups: vec![],
    });
    assert!(sut.view().loaded);
    sut.resolve(Res::HistoryLoaded { txs: vec![] });
    assert!(sut.view().contacts.is_empty());
}

/// The event-driven replacement for `clearContactsCache()`: switching wipes
/// the ledger AND the identity/classification caches, so nothing from the
/// previous account can feed the new book.
#[test]
fn account_switch_clears_ledger_and_caches() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    assert_eq!(ops.len(), 2);
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some("0x".to_owned()),
    });
    sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: Some(identity("alice.eth", "ENS")),
    });
    // Cached: a repeat inspect asks for nothing.
    assert!(sut
        .dispatch(Event::InspectRecipient {
            chain_id: 1,
            address: A.to_owned(),
        })
        .is_empty());

    let ops = sut.dispatch(Event::AccountSwitched {
        my_address: Some(B.to_owned()),
    });
    assert_eq!(ops, vec![Op::ReadStore, Op::LoadSendHistory]);
    assert!(
        sut.view().recipient.is_none(),
        "the inspected recipient does not survive a switch"
    );
    sut.resolve(Res::StoreLoaded {
        contacts: vec![],
        tombstones: vec![],
        groups: vec![],
    });
    sut.resolve(Res::HistoryLoaded { txs: vec![] });

    // Caches are gone: the same recipient is looked up afresh.
    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::ClassifyRecipient {
                chain_id: 1,
                address: A.to_owned(),
            },
            Op::ResolveIdentity {
                address: A.to_owned(),
            },
        ]
    );
}

/// contacts.ts:286-290 — `loadTransactions` throwing yields no suggestions.
#[test]
fn history_failure_yields_no_suggestions() {
    let mut sut = booted(vec![], vec![], vec![], vec![send(A, 100.0)]);
    assert_eq!(addresses(&sut.view()), vec![A.to_owned()]);

    let ops = sut.dispatch(Event::HistoryChanged);
    assert_eq!(ops, vec![Op::LoadSendHistory]);
    sut.resolve(Res::HistoryFailed);
    assert!(sut.view().contacts.is_empty());
}

// ---------------------------------------------------------------------------
// Derivation from send history — invariant ①
// ---------------------------------------------------------------------------

/// Invariant ① — only `type: 'send'` rows suggest. dApp contract calls,
/// receives and legacy untyped rows never pollute the trust signal.
#[test]
fn only_send_txs_become_suggestions_never_dapp_calls() {
    let sut = booted(
        vec![],
        vec![],
        vec![],
        vec![
            send(A, 100.0),
            tx(Some(ContactTxKind::DappTx), B, 200.0),
            tx(Some(ContactTxKind::Receive), ME, 300.0),
            tx(None, C, 400.0), // legacy untyped — never a suggestion
        ],
    );
    let view = sut.view();
    assert_eq!(addresses(&view), vec![A.to_owned()]);
    assert_eq!(view.contacts[0].source, ContactSource::Auto);
    assert_eq!(view.contacts[0].tx_count, 1);
}

#[test]
fn derivation_dedupes_counts_and_tracks_recency() {
    let sut = booted(
        vec![],
        vec![],
        vec![],
        vec![send(A, 100.0), send(A, 300.0), send(A, 200.0)],
    );
    let view = sut.view();
    assert_eq!(view.contacts.len(), 1);
    let a = &view.contacts[0];
    assert_eq!(a.tx_count, 3);
    assert_eq!(a.last_used_ms, 300.0);
    assert_eq!(a.first_seen_ms, 100.0);
}

#[test]
fn derivation_skips_self_and_malformed_recipients() {
    let sut = booted(
        vec![],
        vec![],
        vec![],
        vec![
            send(ME, 100.0),
            send("0x123", 150.0),
            send(A, 200.0),
            ContactHistoryTx {
                kind: Some(ContactTxKind::Send),
                to: None,
                to_name: None,
                timestamp_ms: Some(250.0),
            },
        ],
    );
    assert_eq!(addresses(&sut.view()), vec![A.to_owned()]);
}

#[test]
fn derivation_carries_to_name_as_resolved_name() {
    let sut = booted(
        vec![],
        vec![],
        vec![],
        vec![named_send(A, 100.0, "vitalik.eth")],
    );
    assert_eq!(
        sut.view().contacts[0].resolved_name.as_deref(),
        Some("vitalik.eth")
    );
}

// ---------------------------------------------------------------------------
// Merge of saved ⊕ history
// ---------------------------------------------------------------------------

/// Saved entries win on identity/name; recency and count refresh from history
/// so a saved contact still sorts by recent use.
#[test]
fn saved_wins_name_recency_refreshed_from_history() {
    let sut = booted(
        vec![manual(A, Some("Alice"), false, 1_000.0)],
        vec![],
        vec![],
        vec![send(A, 2_000.0), send(A, 1_500.0)],
    );
    let view = sut.view();
    assert_eq!(view.contacts.len(), 1);
    let a = &view.contacts[0];
    assert_eq!(a.name.as_deref(), Some("Alice"));
    assert_eq!(a.source, ContactSource::Manual);
    assert_eq!(a.tx_count, 2);
    assert_eq!(a.last_used_ms, 2_000.0);
}

#[test]
fn saved_only_contact_still_appears() {
    let sut = booted(
        vec![manual(B, Some("Bob"), false, 1_000.0)],
        vec![],
        vec![],
        vec![send(A, 100.0)],
    );
    let mut got = addresses(&sut.view());
    got.sort();
    assert_eq!(got, vec![A.to_owned(), B.to_owned()]);
}

// ---------------------------------------------------------------------------
// Delete tombstones — invariant ②
// ---------------------------------------------------------------------------

/// Invariant ② — regression from TS: a deleted contact with send history used
/// to re-appear as an `auto` suggestion. The tombstone buries it for good.
#[test]
fn deleted_recipient_never_resurrects_without_new_interaction() {
    let mut sut = booted(
        vec![manual(A, Some("Alice"), true, 900.0)],
        vec![],
        vec![],
        vec![send(A, 500.0)],
    );
    assert_eq!(addresses(&sut.view()), vec![A.to_owned()]);

    let ops = sut.dispatch(Event::Delete {
        address: A.to_owned(),
        now_ms: 1_000.0,
    });
    assert_eq!(ops.len(), 2, "contacts + tombstones both persist");
    assert!(matches!(&ops[0], Op::WriteContacts { contacts } if contacts.is_empty()));
    assert!(matches!(&ops[1], Op::WriteDismissed { tombstones }
            if tombstones.len() == 1 && tombstones[0].address == A && tombstones[0].dismissed_at_ms == 1_000.0));
    ack_writes(&mut sut, 2);
    assert!(
        sut.view().contacts.is_empty(),
        "the send at 500 <= dismissal at 1000 stays buried"
    );
}

/// A send AFTER the deletion lifts the tombstone: `lastUsed > dismissedAt`.
#[test]
fn a_send_after_deletion_resurfaces_the_recipient() {
    let mut sut = booted(vec![], vec![], vec![], vec![send(A, 500.0)]);
    let ops = sut.dispatch(Event::Delete {
        address: A.to_owned(),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    assert!(sut.view().contacts.is_empty());

    sut.dispatch(Event::HistoryChanged);
    sut.resolve(Res::HistoryLoaded {
        txs: vec![send(A, 500.0), send(A, 2_000.0)],
    });
    let view = sut.view();
    assert_eq!(addresses(&view), vec![A.to_owned()]);
    assert_eq!(view.contacts[0].tx_count, 2);
}

/// Invariant ② second half — re-saving clears the tombstone (and persists the
/// cleared tombstone set).
#[test]
fn resaving_a_deleted_address_clears_the_tombstone() {
    let mut sut = booted(vec![], vec![], vec![], vec![send(A, 500.0)]);
    let ops = sut.dispatch(Event::Delete {
        address: A.to_owned(),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    assert!(sut.view().contacts.is_empty());

    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 2_000.0,
    });
    assert_eq!(ops.len(), 2);
    assert!(matches!(&ops[0], Op::WriteContacts { .. }));
    assert_eq!(
        ops[1],
        Op::WriteDismissed { tombstones: vec![] },
        "the tombstone is gone from the persisted set"
    );
    ack_writes(&mut sut, 2);

    let view = sut.view();
    let a = find(&view, A);
    assert_eq!(a.name.as_deref(), Some("Alice"));
    assert_eq!(a.source, ContactSource::Manual);
}

// ---------------------------------------------------------------------------
// Saved CRUD
// ---------------------------------------------------------------------------

/// contacts.ts:183-209 — keyed on the lowercased address; a second save
/// merges instead of duplicating.
#[test]
fn save_is_idempotent_on_lowercased_address() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(&A.to_uppercase().replace("0X", "0x"), Some("Alice")),
        now_ms: 1_000.0,
    });
    assert_eq!(
        ops.len(),
        1,
        "no tombstone to clear, only the contacts write"
    );
    ack_writes(&mut sut, 1);
    let view = sut.view();
    assert_eq!(addresses(&view), vec![A.to_owned()]);
    assert_eq!(view.contacts[0].source, ContactSource::Manual);

    let ops = sut.dispatch(Event::Save {
        input: ContactSaveInput {
            note: Some("hi".to_owned()),
            ..save_input(A, Some("Alice 2"))
        },
        now_ms: 5_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    assert_eq!(view.contacts.len(), 1, "still one — keyed by address");
    let a = &view.contacts[0];
    assert_eq!(a.name.as_deref(), Some("Alice 2"));
    assert_eq!(a.note.as_deref(), Some("hi"));
    assert_eq!(a.first_seen_ms, 1_000.0, "merge keeps the original stamps");
}

/// contacts.ts:238-245 — a saved contact flips in place; an unsaved
/// suggestion is promoted to a starred saved contact.
#[test]
fn toggle_favorite_flips_saved_and_promotes_suggestion() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::ToggleFavorite {
        address: A.to_owned(),
        now_ms: 2_000.0,
    });
    assert_eq!(ops.len(), 1);
    ack_writes(&mut sut, 1);
    assert!(find(&sut.view(), A).favorite);

    // B was never saved → promoted via the full save path.
    let ops = sut.dispatch(Event::ToggleFavorite {
        address: B.to_owned(),
        now_ms: 3_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    let b = find(&view, B);
    assert!(b.favorite);
    assert_eq!(b.source, ContactSource::Manual);
}

/// The single-threaded core replaces `_writeChain`: every write carries the
/// whole ledger, so back-to-back mutations can never drop each other.
#[test]
fn sequential_saves_never_lose_each_others_writes() {
    let mut sut = booted_empty();
    sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    // The second save fires BEFORE the first write is acknowledged.
    let ops = sut.dispatch(Event::Save {
        input: save_input(B, Some("Bob")),
        now_ms: 2_000.0,
    });
    let Op::WriteContacts { contacts } = &ops[0] else {
        panic!("expected a contacts write");
    };
    let mut got: Vec<&str> = contacts.iter().map(|c| c.address.as_str()).collect();
    got.sort();
    assert_eq!(got, vec![A, B], "the second write carries both contacts");
}

// ---------------------------------------------------------------------------
// Groups — invariants ③ and ⑥
// ---------------------------------------------------------------------------

/// Invariant ③ — members are lowercased, invalid dropped, first-wins deduped.
#[test]
fn group_members_are_normalized_lowercased_deduped_valid_only() {
    let mut sut = booted_empty();
    let upper = A.to_uppercase().replace("0X", "0x");
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Payroll", Some(&[&upper, A, B, "0xnope"])),
    });
    assert_eq!(ops.len(), 1);
    assert!(matches!(&ops[0], Op::WriteGroups { .. }));
    ack_writes(&mut sut, 1);

    let view = sut.view();
    assert_eq!(view.groups.len(), 1);
    assert_eq!(view.groups[0].id, "grp_1");
    assert_eq!(view.groups[0].name, "Payroll");
    let members: Vec<&str> = view.groups[0]
        .members
        .iter()
        .map(|c| c.address.as_str())
        .collect();
    assert_eq!(
        members,
        vec![A, B],
        "upper-cased dup dropped, invalid dropped"
    );
}

/// Invariant ⑥ — ids derive from the max persisted suffix, never a clock or a
/// process counter, so a cold reload can't mint a colliding id.
#[test]
fn group_ids_are_deterministic_and_never_collide_across_cold_reload() {
    // The store already holds grp_9 plus ids a counter would misread.
    let mut sut = booted(
        vec![],
        vec![],
        vec![
            group("grp_9", "Nine", &[]),
            group("custom", "X", &[]), // parseInt('custom') → NaN, ignored
            group("grp_3x", "Y", &[]), // parseInt('3x') → 3
        ],
        vec![],
    );
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Ten", None),
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    assert_eq!(view.groups.len(), 4);
    assert_eq!(view.groups[3].id, "grp_10");
}

/// contacts.ts:337-360 — updating keeps the id; a blank rename keeps the old
/// name; omitting members leaves the membership untouched.
#[test]
fn group_update_in_place_keeps_id_blank_rename_keeps_name() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Team", Some(&[A])),
    });
    ack_writes(&mut sut, ops.len());

    // Blank rename + no members: nothing changes but the write still lands.
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(Some("grp_1"), "   ", None),
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    assert_eq!(view.groups[0].name, "Team");
    assert_eq!(view.groups[0].members.len(), 1);

    let ops = sut.dispatch(Event::GroupSave {
        input: gin(Some("grp_1"), "Team Renamed", Some(&[B, C])),
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    assert_eq!(view.groups.len(), 1, "updated in place, not duplicated");
    assert_eq!(view.groups[0].id, "grp_1");
    assert_eq!(view.groups[0].name, "Team Renamed");
    let members: Vec<&str> = view.groups[0]
        .members
        .iter()
        .map(|c| c.address.as_str())
        .collect();
    assert_eq!(members, vec![B, C]);
}

#[test]
fn set_group_members_replaces_whole_list() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Team", Some(&[A])),
    });
    ack_writes(&mut sut, ops.len());
    let ops = sut.dispatch(Event::SetGroupMembers {
        id: "grp_1".to_owned(),
        members: vec![C.to_owned(), B.to_owned(), C.to_owned()],
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    let members: Vec<&str> = view.groups[0]
        .members
        .iter()
        .map(|c| c.address.as_str())
        .collect();
    assert_eq!(members, vec![C, B], "replaced, normalized, first-wins");
}

/// contacts.ts:362-366 — deleting a group never touches the contacts.
#[test]
fn group_delete_never_touches_contacts() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Team", Some(&[A])),
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::GroupDelete {
        id: "grp_1".to_owned(),
    });
    assert_eq!(ops.len(), 1);
    assert!(matches!(&ops[0], Op::WriteGroups { groups } if groups.is_empty()));
    ack_writes(&mut sut, 1);
    let view = sut.view();
    assert!(view.groups.is_empty());
    assert_eq!(find(&view, A).name.as_deref(), Some("Alice"));
}

/// Invariant ③ — deleting a contact cascades it out of every group; groups
/// are only written when one actually held the address.
#[test]
fn deleting_a_contact_cascades_out_of_every_group() {
    let mut sut = booted_empty();
    for (addr, name) in [(A, "Alice"), (B, "Bob"), (C, "Cara")] {
        let ops = sut.dispatch(Event::Save {
            input: save_input(addr, Some(name)),
            now_ms: 1_000.0,
        });
        ack_writes(&mut sut, ops.len());
    }
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Payroll", Some(&[A, B])),
    });
    ack_writes(&mut sut, ops.len());
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Friends", Some(&[A])),
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::Delete {
        address: A.to_owned(),
        now_ms: 2_000.0,
    });
    assert_eq!(ops.len(), 3, "contacts + tombstones + groups all persist");
    assert!(matches!(&ops[2], Op::WriteGroups { .. }));
    ack_writes(&mut sut, 3);

    let view = sut.view();
    let payroll = &view.groups[0];
    let friends = &view.groups[1];
    assert_eq!(
        payroll
            .members
            .iter()
            .map(|c| c.address.as_str())
            .collect::<Vec<_>>(),
        vec![B]
    );
    assert!(friends.members.is_empty(), "empty, never dangling");

    // C belongs to no group: deleting it must NOT write groups.
    let ops = sut.dispatch(Event::Delete {
        address: C.to_owned(),
        now_ms: 3_000.0,
    });
    assert_eq!(ops.len(), 2, "no group held C, so no group write");
}

/// contacts.ts:396-410 — a member with a saved contact carries its name; a
/// bare-address member is synthesised, never silently dropped from a payout.
#[test]
fn group_view_resolves_saved_and_synthesises_unsaved_members() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Team", Some(&[A, B])),
    });
    ack_writes(&mut sut, ops.len());

    let view = sut.view();
    let members = &view.groups[0].members;
    assert_eq!(members.len(), 2, "order kept, nothing dropped");
    assert_eq!(members[0].name.as_deref(), Some("Alice"));
    assert_eq!(members[0].source, ContactSource::Manual);
    assert_eq!(members[1].address, B);
    assert_eq!(members[1].source, ContactSource::Auto);
}

// ---------------------------------------------------------------------------
// Import — invariant ⑤ (existing-wins)
// ---------------------------------------------------------------------------

/// contact-io.ts:203-223 — import only ADDS; a row whose address already
/// exists is skipped untouched, an invalid address is counted, never stored.
#[test]
fn import_is_existing_wins_never_overwrites_local() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: ContactSaveInput {
            favorite: Some(true),
            ..save_input(A, Some("Local Alice"))
        },
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::ImportParsed {
        contacts: vec![
            entry(A, Some("Imported Alice")), // exists → skipped, local wins
            entry(B, Some("Bob")),            // new → added
            entry("0xnope", Some("Bad")),     // invalid → dropped
        ],
        groups: vec![],
        now_ms: 2_000.0,
    });
    assert_eq!(
        ops.len(),
        1,
        "one contacts write, no group/tombstone writes"
    );
    ack_writes(&mut sut, 1);

    let view = sut.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 1,
            skipped: 1,
            invalid: 1,
            groups_created: 0,
        })
    );
    let a = find(&view, A);
    assert_eq!(a.name.as_deref(), Some("Local Alice"), "untouched");
    assert!(a.favorite);
    assert_eq!(find(&view, B).name.as_deref(), Some("Bob"));
    assert!(view.contacts.iter().all(|c| c.address != "0xnope"));
}

/// Invariant ⑤ — a duplicate address within the same file is added once
/// (first row wins).
#[test]
fn import_duplicate_within_file_added_once() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::ImportParsed {
        contacts: vec![entry(A, Some("One")), entry(A, Some("Two"))],
        groups: vec![],
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let view = sut.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 1,
            skipped: 1,
            invalid: 0,
            groups_created: 0,
        })
    );
    assert_eq!(view.contacts.len(), 1);
    assert_eq!(view.contacts[0].name.as_deref(), Some("One"));
}

/// contact-io.ts:225-241 — groups are additive: created if missing, attaching
/// ONLY the newly-added members; a pre-existing contact's memberships are
/// never altered by an import.
#[test]
fn import_creates_missing_groups_attaching_only_new_members() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Local Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::ImportParsed {
        contacts: vec![entry(A, None), entry(B, None), entry(C, None)],
        groups: vec![ContactImportGroup {
            name: "Payroll".to_owned(),
            color: None,
            members: vec![A.to_owned(), B.to_owned(), C.to_owned()],
        }],
        now_ms: 2_000.0,
    });
    assert_eq!(ops.len(), 2);
    assert!(matches!(&ops[0], Op::WriteContacts { .. }));
    assert!(matches!(&ops[1], Op::WriteGroups { .. }));
    ack_writes(&mut sut, 2);

    let view = sut.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 2,
            skipped: 1,
            invalid: 0,
            groups_created: 1,
        })
    );
    let payroll = &view.groups[0];
    assert_eq!(payroll.name, "Payroll");
    let members: Vec<&str> = payroll.members.iter().map(|c| c.address.as_str()).collect();
    assert_eq!(members, vec![B, C], "pre-existing A is NOT attached");
}

/// contact-io.ts:231-235 — a same-named group (case-insensitive) unions the
/// new members in, keeping the current ones.
#[test]
fn import_unions_new_members_into_existing_same_named_group() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());
    let ops = sut.dispatch(Event::GroupSave {
        input: gin(None, "Payroll", Some(&[A])),
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::ImportParsed {
        contacts: vec![entry(B, None)],
        groups: vec![ContactImportGroup {
            name: "payroll".to_owned(), // lower-case on purpose
            color: None,
            members: vec![B.to_owned()],
        }],
        now_ms: 2_000.0,
    });
    ack_writes(&mut sut, ops.len());

    let view = sut.view();
    assert_eq!(view.groups.len(), 1, "unioned into grp_1, not duplicated");
    assert_eq!(view.groups[0].id, "grp_1");
    let members: Vec<&str> = view.groups[0]
        .members
        .iter()
        .map(|c| c.address.as_str())
        .collect();
    assert_eq!(members, vec![A, B], "A kept, B unioned in");
    assert_eq!(
        view.last_import.map(|r| r.groups_created),
        Some(0),
        "no group was created"
    );
}

/// An import that adds nothing writes nothing — but the report still renders.
#[test]
fn import_with_nothing_new_writes_nothing() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::Save {
        input: save_input(A, Some("Alice")),
        now_ms: 1_000.0,
    });
    ack_writes(&mut sut, ops.len());

    let ops = sut.dispatch(Event::ImportParsed {
        contacts: vec![entry(A, Some("Shadow Alice"))],
        groups: vec![ContactImportGroup {
            name: "Payroll".to_owned(),
            color: None,
            members: vec![A.to_owned()],
        }],
        now_ms: 2_000.0,
    });
    assert!(ops.is_empty(), "no store changed, so nothing is written");
    let view = sut.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 0,
            skipped: 1,
            invalid: 0,
            groups_created: 0,
        })
    );
    assert!(view.groups.is_empty(), "existing memberships untouched");
    assert_eq!(find(&view, A).name.as_deref(), Some("Alice"));
}

// ---------------------------------------------------------------------------
// Recipient trust & risk — invariant ⑦, RecipientTrust.tsx semantics
// ---------------------------------------------------------------------------

/// RecipientTrust.tsx:5-8 — saved **and** starred is the ONLY state that
/// earns the green check; a poisoned look-alike is never a starred contact.
#[test]
fn green_check_requires_saved_and_starred() {
    let mut sut = booted(
        vec![manual(A, Some("Alice"), false, 1_000.0)],
        vec![],
        vec![],
        vec![],
    );
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert!(recipient.saved);
    assert!(!recipient.verified, "saved but unstarred: no green check");

    let ops = sut.dispatch(Event::ToggleFavorite {
        address: A.to_owned(),
        now_ms: 2_000.0,
    });
    assert_eq!(ops.len(), 1);
    let recipient = sut.view().recipient.expect("recipient projected");
    assert!(recipient.verified, "saved ∧ starred earns the check");

    // An unsaved recipient can never be verified, whatever else resolves.
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: B.to_owned(),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert!(!recipient.saved);
    assert!(!recipient.verified);
}

/// Contact name → contact resolved name → live identity → None.
#[test]
fn display_name_prefers_contact_name_over_identity() {
    let mut sut = booted(
        vec![manual(A, Some("Alice"), false, 1_000.0)],
        vec![],
        vec![],
        vec![],
    );
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some("0x".to_owned()),
    });
    let ops = sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: Some(identity("alice.eth", "ENS")),
    });
    assert!(ops.is_empty(), "a named contact is never written back over");

    let recipient = sut.view().recipient.expect("recipient projected");
    assert_eq!(recipient.display_name.as_deref(), Some("Alice"));
    assert_eq!(
        recipient.identity.map(|i| i.name),
        Some("alice.eth".to_owned()),
        "the live identity still rides along for the source tag"
    );
}

/// Invariant ⑦ — an EIP-7702 delegated EOA (`0xef0100 ++ addr`, 23 bytes) is
/// a person's wallet and must NOT be badged "Contract"; the address-book
/// `kind` still reads it as a smart account (contacts.ts:438, ported
/// verbatim).
#[test]
fn eip7702_delegated_eoa_is_never_badged_contract() {
    let mut sut = booted_empty();
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    let delegation = format!("0xef0100{}", "ab".repeat(20));
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some(delegation),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert_eq!(
        recipient.is_contract,
        Some(false),
        "a wallet, not a contract"
    );
    assert_eq!(
        recipient.kind,
        ContactKind::Account,
        "the book's kind projection keeps the TS behaviour verbatim"
    );
}

/// recipient-risk.ts:50 / contacts.ts:438 — real bytecode is a contract and a
/// smart account; empty code is an EOA.
#[test]
fn contract_code_classifies_account_and_contract() {
    let mut sut = booted_empty();
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some("0x60016002".to_owned()),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert_eq!(recipient.is_contract, Some(true));
    assert_eq!(recipient.kind, ContactKind::Account);

    sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: None,
    });
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: B.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: B.to_owned(),
        code: Some("0x".to_owned()),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert_eq!(recipient.is_contract, Some(false));
    assert_eq!(recipient.kind, ContactKind::Eoa);
}

/// Invariant ⑦ — RPC unreachable is unknown, NOT a verdict: never a false
/// alarm, never cached, so the next inspect retries.
#[test]
fn unreachable_classification_is_unknown_and_never_cached() {
    let mut sut = booted_empty();
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: None,
    });
    sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: None,
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert_eq!(recipient.is_contract, None, "unknown, not false");

    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::ClassifyRecipient {
                chain_id: 1,
                address: A.to_owned(),
            },
            Op::ResolveIdentity {
                address: A.to_owned(),
            },
        ],
        "both failed lookups are re-asked"
    );
}

/// recipient-identity.ts:232-267 / invariant ⑦ — only positive resolutions
/// are cached; a `None` answer is re-asked next time.
#[test]
fn only_positive_identity_resolutions_are_cached() {
    let mut sut = booted_empty();
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some("0x".to_owned()),
    });
    sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: None,
    });

    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::ResolveIdentity {
            address: A.to_owned(),
        }],
        "the verdict is cached, the negative identity is not"
    );
    sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: Some(identity("alice.eth", "ENS")),
    });

    assert!(
        sut.dispatch(Event::InspectRecipient {
            chain_id: 1,
            address: A.to_owned(),
        })
        .is_empty(),
        "a positive resolution IS cached"
    );
}

/// RecipientTrust.tsx:78-84 — a saved-but-unnamed contact adopts the resolved
/// identity and the adoption is persisted, so the picker shows the real name.
#[test]
fn identity_write_back_names_a_saved_unnamed_contact() {
    let mut sut = booted(
        vec![manual(A, None, false, 1_000.0)],
        vec![],
        vec![],
        vec![],
    );
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    sut.resolve(Res::RecipientClassified {
        chain_id: 1,
        address: A.to_owned(),
        code: Some("0x".to_owned()),
    });
    let ops = sut.resolve(Res::IdentityResolved {
        address: A.to_owned(),
        identity: Some(identity("alice.eth", "ENS")),
    });
    assert_eq!(ops.len(), 1, "the adopted name is persisted");
    let Op::WriteContacts { contacts } = &ops[0] else {
        panic!("expected a contacts write");
    };
    assert_eq!(contacts[0].resolved_name.as_deref(), Some("alice.eth"));
    assert_eq!(contacts[0].resolved_source.as_deref(), Some("ENS"));

    let view = sut.view();
    assert_eq!(find(&view, A).resolved_name.as_deref(), Some("alice.eth"));
    let recipient = view.recipient.expect("recipient projected");
    assert_eq!(recipient.display_name.as_deref(), Some("alice.eth"));
}

/// recipient-risk.ts:59-69 — prior interaction counts sends, dApp txs AND
/// legacy untyped rows (verbatim: broader than what may suggest).
#[test]
fn first_interaction_counts_sends_dapp_and_legacy_rows() {
    let mut sut = booted(
        vec![],
        vec![],
        vec![],
        vec![
            send(&A.to_uppercase().replace("0X", "0x"), 100.0), // case-blind match
            tx(Some(ContactTxKind::DappTx), B, 200.0),
            tx(None, C, 300.0), // legacy untyped
            tx(Some(ContactTxKind::Receive), ME, 400.0),
        ],
    );
    for addr in [A, B, C] {
        sut.dispatch(Event::InspectRecipient {
            chain_id: 1,
            address: addr.to_owned(),
        });
        let recipient = sut.view().recipient.expect("recipient projected");
        assert!(
            !recipient.first_interaction,
            "{addr} has prior outgoing history"
        );
    }
    sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: ME.to_owned(),
    });
    let recipient = sut.view().recipient.expect("recipient projected");
    assert!(
        recipient.first_interaction,
        "a receive is not an outgoing interaction — the poisoning tell fires"
    );
}

/// recipient-risk.ts:76-78 — a malformed address gets no lookups and the
/// `{is_contract: null, first_interaction: false}` shape.
#[test]
fn invalid_recipient_gets_no_lookups_and_null_verdict() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: "0xnope".to_owned(),
    });
    assert!(ops.is_empty(), "no RPC for garbage");
    let recipient = sut.view().recipient.expect("recipient projected");
    assert!(!recipient.saved);
    assert_eq!(recipient.is_contract, None);
    assert!(!recipient.first_interaction);
}

/// recipient-identity.ts:233-236 — the zero address is a mint/burn
/// counterparty: classification still runs, identity is never asked.
#[test]
fn zero_address_skips_identity_lookup() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: ZERO.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::ClassifyRecipient {
            chain_id: 1,
            address: ZERO.to_owned(),
        }]
    );
}

/// The core-side replacement for the TS hook's module-level inflight merge:
/// a second inspect while lookups are in flight issues nothing.
#[test]
fn inflight_lookups_are_not_duplicated() {
    let mut sut = booted_empty();
    let ops = sut.dispatch(Event::InspectRecipient {
        chain_id: 1,
        address: A.to_owned(),
    });
    assert_eq!(ops.len(), 2);
    assert!(
        sut.dispatch(Event::InspectRecipient {
            chain_id: 1,
            address: A.to_owned(),
        })
        .is_empty(),
        "both lookups are already in flight"
    );
}

// ---------------------------------------------------------------------------
// Sort & search (pure projections)
// ---------------------------------------------------------------------------

/// contacts.ts:452-458 — favourites first (despite older lastUsed), then
/// most-recently-used.
#[test]
fn sort_favorites_first_then_most_recent() {
    let sut = booted(
        vec![
            manual(A, None, false, 100.0),
            manual(B, None, false, 200.0),
            manual(C, None, true, 50.0),
        ],
        vec![],
        vec![],
        vec![],
    );
    assert_eq!(
        addresses(&sut.view()),
        vec![C.to_owned(), B.to_owned(), A.to_owned()]
    );
}

#[test]
fn sort_ties_break_on_display_name_then_address() {
    let sorted = sort_contacts(vec![
        manual(B, Some("bob"), false, 100.0),
        manual(A, Some("Alice"), false, 100.0),
        manual(C, None, false, 100.0), // no name → empty display name first
    ]);
    let names: Vec<String> = sorted.iter().map(contact_display_name).collect();
    assert_eq!(
        names,
        vec![String::new(), "Alice".to_owned(), "bob".to_owned()]
    );
}

/// contacts.ts:461-469.
#[test]
fn matches_query_on_address_name_resolved_name() {
    let mut c = manual(A, Some("Alice"), false, 0.0);
    c.resolved_name = Some("alice.eth".to_owned());
    assert!(matches_query(&c, ""));
    assert!(matches_query(&c, "ali"));
    assert!(matches_query(&c, "ALI"), "query is lowercased");
    assert!(matches_query(&c, ".eth"));
    assert!(matches_query(&c, "aaaa"), "address substring");
    assert!(!matches_query(&c, "zzz"));
}

// ---------------------------------------------------------------------------
// The book as a file (spec 028 US5, FR-408) — export, import, into a group
// ---------------------------------------------------------------------------

use vela_core::app::contacts::{
    ContactExportFile, ContactExportScope, ContactFileFormat, ContactImportFailure,
};
use vela_core::app::contacts_io;

const STAMP: &str = "2026-09-05T03:00:00.000Z";

fn export(
    sut: &mut Sut,
    scope: ContactExportScope,
    format: ContactFileFormat,
) -> ContactExportFile {
    let ops = sut.dispatch(Event::ExportRequested {
        scope,
        format,
        exported_at_iso: STAMP.to_owned(),
    });
    assert!(
        ops.is_empty(),
        "an export asks the shell for nothing — the file is in the view"
    );
    sut.view().export.expect("the view carries the file")
}

fn import_file(
    sut: &mut Sut,
    content: &str,
    filename: Option<&str>,
    into_group: Option<&str>,
) -> Vec<Op> {
    sut.dispatch(Event::ImportFile {
        content: content.to_owned(),
        filename: filename.map(str::to_owned),
        into_group: into_group.map(str::to_owned),
        now_ms: 5_000.0,
    })
}

fn member_addresses(view: &ContactsView, index: usize) -> Vec<&str> {
    view.groups[index]
        .members
        .iter()
        .map(|m| m.address.as_str())
        .collect()
}

/// A backup written on one device restores a fresh book on another, whole —
/// names, stars and groups — and re-importing it where it came from changes
/// nothing (SC-408).
#[test]
fn export_then_import_restores_a_fresh_book_and_reimport_changes_nothing() {
    let mut first = booted(
        vec![
            manual(A, Some("Alice"), true, 1_000.0),
            manual(B, Some("Bob"), false, 500.0),
        ],
        vec![],
        vec![group("grp_1", "Payroll", &[A, B])],
        vec![],
    );
    let file = export(&mut first, ContactExportScope::All, ContactFileFormat::Json);
    assert_eq!(file.filename, "vela-contacts-2026-09-05.json");
    assert_eq!(file.mime, "application/json");
    assert_eq!(file.contacts, 2);
    assert!(file.content.contains("\"version\": 1"), "{}", file.content);
    assert!(file
        .content
        .contains(&format!("\"exportedAt\": \"{STAMP}\"")));
    // Taken = gone: the view does not keep offering the same download.
    first.dispatch(Event::ExportTaken);
    assert!(first.view().export.is_none());

    let mut second = booted_empty();
    let ops = import_file(&mut second, &file.content, Some(&file.filename), None);
    assert!(
        matches!(
            ops.as_slice(),
            [Op::WriteContacts { .. }, Op::WriteGroups { .. }]
        ),
        "{ops:?}"
    );
    ack_writes(&mut second, 2);
    let view = second.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 2,
            skipped: 0,
            invalid: 0,
            groups_created: 1
        })
    );
    assert_eq!(view.import_failure, None);
    assert_eq!(find(&view, A).name.as_deref(), Some("Alice"));
    assert!(find(&view, A).favorite);
    assert_eq!(find(&view, B).name.as_deref(), Some("Bob"));
    assert_eq!(view.groups[0].name, "Payroll");
    assert_eq!(member_addresses(&view, 0), vec![A, B]);

    // The same file back where it came from: every row already exists, no
    // group gains a member, nothing is written.
    let ops = import_file(&mut first, &file.content, Some(&file.filename), None);
    assert!(ops.is_empty(), "{ops:?}");
    let view = first.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 0,
            skipped: 2,
            invalid: 0,
            groups_created: 0
        })
    );
    assert_eq!(find(&view, A).name.as_deref(), Some("Alice"));
    assert_eq!(member_addresses(&view, 0), vec![A, B]);
}

/// "Import into this group": every valid row is seated in the group — the
/// ones just added AND the ones already saved — while existing-wins still
/// protects every saved name.
#[test]
fn import_into_group_seats_every_valid_row_but_never_renames_an_existing_contact() {
    let mut sut = booted(
        vec![manual(A, Some("Local Alice"), false, 1.0)],
        vec![],
        vec![group("grp_1", "Payroll", &[])],
        vec![],
    );
    let csv = format!("address,name\n{A},Imported Alice\n{B},Bob\nnot-an-address,Nobody\n");
    let ops = import_file(&mut sut, &csv, Some("team.csv"), Some("grp_1"));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::WriteContacts { .. }, Op::WriteGroups { .. }]
        ),
        "{ops:?}"
    );
    ack_writes(&mut sut, 2);
    let view = sut.view();
    assert_eq!(
        view.last_import,
        Some(ContactImportReport {
            added: 1,
            skipped: 1,
            invalid: 1,
            groups_created: 0
        })
    );
    assert_eq!(
        find(&view, A).name.as_deref(),
        Some("Local Alice"),
        "existing-wins holds inside a group import"
    );
    assert_eq!(find(&view, B).name.as_deref(), Some("Bob"));
    assert_eq!(member_addresses(&view, 0), vec![A, B]);
}

/// A file the core cannot read is REFUSED, with a reason, before anything is
/// written (D50) — and the refusal clears when acknowledged.
#[test]
fn a_bad_file_is_refused_before_anything_is_written() {
    let mut sut = booted(
        vec![manual(A, Some("Alice"), false, 1.0)],
        vec![],
        vec![group("grp_1", "Payroll", &[A])],
        vec![],
    );
    let stale_group = format!("address\n{B}\n");
    let cases: [(&str, Option<&str>, Option<&str>, ContactImportFailure); 4] = [
        (
            "name,email\nAlice,a@example.com\nBob,b@example.com\n",
            Some("theirs.csv"),
            None,
            ContactImportFailure::NoAddressColumn,
        ),
        (
            "{ not json",
            Some("x.json"),
            None,
            ContactImportFailure::MalformedJson,
        ),
        ("", Some("empty.csv"), None, ContactImportFailure::Empty),
        (
            stale_group.as_str(),
            Some("ok.csv"),
            Some("grp_99"),
            ContactImportFailure::UnknownGroup,
        ),
    ];
    for (content, filename, into_group, expected) in cases {
        let ops = import_file(&mut sut, content, filename, into_group);
        assert!(ops.is_empty(), "{expected:?}: a refusal writes nothing");
        let view = sut.view();
        assert_eq!(view.import_failure, Some(expected));
        assert_eq!(view.last_import, None);
        assert_eq!(addresses(&view), vec![A.to_owned()]);
        assert_eq!(member_addresses(&view, 0), vec![A]);
    }
    sut.dispatch(Event::ImportAcknowledged);
    assert_eq!(sut.view().import_failure, None);
}

/// Exporting one group carries its members only, names only that group, and
/// an unsaved member still travels as its address (invariant ③).
#[test]
fn exporting_a_group_covers_its_members_only() {
    let mut sut = booted(
        vec![
            manual(A, Some("Alice"), false, 1.0),
            manual(B, Some("Bob"), false, 1.0),
        ],
        vec![],
        vec![
            group("grp_1", "Payroll", &[A, C]),
            group("grp_2", "Friends", &[A, B]),
        ],
        vec![],
    );
    let file = export(
        &mut sut,
        ContactExportScope::Group {
            id: "grp_1".to_owned(),
        },
        ContactFileFormat::Csv,
    );
    assert_eq!(file.filename, "vela-contacts-payroll-2026-09-05.csv");
    assert_eq!(file.mime, "text/csv");
    assert_eq!(file.contacts, 2);
    let lines: Vec<&str> = file.content.lines().collect();
    assert_eq!(lines[0], "address,name,note,favorite,groups");
    assert_eq!(lines[1], format!("{A},Alice,,,Payroll"));
    assert_eq!(lines[2], format!("{C},,,,Payroll"));
    assert_eq!(lines.len(), 3, "Bob is not in Payroll");

    // A group that no longer exists exports nothing.
    sut.dispatch(Event::ExportRequested {
        scope: ContactExportScope::Group {
            id: "grp_9".to_owned(),
        },
        format: ContactFileFormat::Csv,
        exported_at_iso: STAMP.to_owned(),
    });
    assert!(sut.view().export.is_none());
}

#[test]
fn export_filename_dates_and_slugs() {
    use ContactFileFormat::{Csv, Json};
    assert_eq!(
        contacts_io::export_filename(None, Json, "2026-09-05T03:00:00Z"),
        "vela-contacts-2026-09-05.json"
    );
    assert_eq!(
        contacts_io::export_filename(Some("Pay Roll / 2026"), Csv, "2026-09-05"),
        "vela-contacts-pay-roll-2026-2026-09-05.csv"
    );
    // Any script's letters survive; a stamp that is not a date adds nothing.
    assert_eq!(
        contacts_io::export_filename(Some("家人"), Json, "not a date"),
        "vela-contacts-家人.json"
    );
    assert_eq!(
        contacts_io::export_filename(Some("***"), Csv, ""),
        "vela-contacts-group.csv"
    );
}

/// The three membership edits: union (normalised), remove, and one contact's
/// whole answer at once — each writing only when something changed.
#[test]
fn add_remove_and_reseat_group_members() {
    let mut sut = booted(
        vec![manual(A, Some("Alice"), false, 1.0)],
        vec![],
        vec![
            group("grp_1", "Payroll", &[A]),
            group("grp_2", "Friends", &[]),
        ],
        vec![],
    );
    // Union, normalised: a mixed-case B lowercases, a duplicate and a
    // malformed entry vanish, A is already seated.
    let mixed = format!("0x{}", B[2..].to_uppercase());
    let ops = sut.dispatch(Event::AddGroupMembers {
        id: "grp_1".to_owned(),
        members: vec![mixed, B.to_owned(), A.to_owned(), "nope".to_owned()],
    });
    assert!(
        matches!(ops.as_slice(), [Op::WriteGroups { groups }]
            if groups[0].members == vec![A.to_owned(), B.to_owned()]),
        "{ops:?}"
    );
    ack_writes(&mut sut, 1);
    // Seating what is already seated writes nothing.
    assert!(sut
        .dispatch(Event::AddGroupMembers {
            id: "grp_1".to_owned(),
            members: vec![A.to_owned()],
        })
        .is_empty());
    // An unknown group writes nothing.
    assert!(sut
        .dispatch(Event::AddGroupMembers {
            id: "grp_9".to_owned(),
            members: vec![B.to_owned()],
        })
        .is_empty());

    let ops = sut.dispatch(Event::RemoveGroupMember {
        id: "grp_1".to_owned(),
        address: B.to_uppercase(),
    });
    assert!(
        matches!(ops.as_slice(), [Op::WriteGroups { groups }] if groups[0].members == vec![A.to_owned()]),
        "{ops:?}"
    );
    ack_writes(&mut sut, 1);

    // Reseat: A leaves Payroll and joins Friends in ONE write.
    let ops = sut.dispatch(Event::SetContactGroups {
        address: A.to_owned(),
        group_ids: vec!["grp_2".to_owned()],
    });
    assert!(
        matches!(ops.as_slice(), [Op::WriteGroups { groups }]
            if groups[0].members.is_empty() && groups[1].members == vec![A.to_owned()]),
        "{ops:?}"
    );
    ack_writes(&mut sut, 1);
    // The same answer again changes nothing; a malformed address is ignored.
    assert!(sut
        .dispatch(Event::SetContactGroups {
            address: A.to_owned(),
            group_ids: vec!["grp_2".to_owned()],
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::SetContactGroups {
            address: "nope".to_owned(),
            group_ids: vec!["grp_1".to_owned()],
        })
        .is_empty());
    let view = sut.view();
    assert!(view.groups[0].members.is_empty());
    assert_eq!(member_addresses(&view, 1), vec![A]);
}

// ---------------------------------------------------------------------------
// The file format itself (contacts_io.rs) — the heuristics every shell shares
// ---------------------------------------------------------------------------

/// A foreign header that does not say "address" — the data says where it is.
/// Falling back to column 0 is what made an import report "0 added, 0 already
/// existed": nothing imported, nothing explained.
#[test]
fn a_foreign_header_is_read_from_the_data_not_from_column_zero() {
    let csv = format!("label,wallet\nAlice,{A}\nBob,{B}\n");
    let parsed = contacts_io::parse(&csv, Some("theirs.csv")).expect("it has addresses");
    assert_eq!(parsed.contacts.len(), 2);
    assert_eq!(parsed.contacts[0].address, A);
    // The label beside it is the one unambiguous extra.
    assert_eq!(parsed.contacts[0].name.as_deref(), Some("Alice"));
}

/// A headerless file in our own order, and one whose address is elsewhere.
#[test]
fn a_headerless_file_is_positional_only_when_the_address_leads() {
    let ours = format!("{A},Alice,a note,true,Family\n");
    let parsed = contacts_io::parse(&ours, None).expect("valid");
    assert_eq!(parsed.contacts[0].name.as_deref(), Some("Alice"));
    assert_eq!(parsed.contacts[0].note.as_deref(), Some("a note"));
    assert_eq!(parsed.contacts[0].favorite, Some(true));
    assert_eq!(parsed.groups[0].name, "Family");
    assert_eq!(parsed.groups[0].members, vec![A.to_owned()]);

    // Address in column 1: the file has told us nothing about columns 2+, so
    // only the label beside it is taken.
    let theirs = format!("Alice,{A},something,else\n");
    let parsed = contacts_io::parse(&theirs, None).expect("valid");
    assert_eq!(parsed.contacts[0].address, A);
    assert_eq!(parsed.contacts[0].name.as_deref(), Some("Alice"));
    assert_eq!(parsed.contacts[0].note, None, "column 2 means nothing here");
}

/// Quoting round-trips, and a malformed row is CARRIED so `apply_import` can
/// count it as invalid rather than having it swallowed before the count.
#[test]
fn csv_quoting_round_trips_and_a_bad_row_reaches_the_policy() {
    let mut alice = manual(A, Some("Alice, the one"), true, 1.0);
    alice.note = Some("said \"hi\"".to_owned());
    let text = contacts_io::to_csv(&[alice], &[group("grp_1", "Family", &[A])]);
    assert!(text.contains("\"Alice, the one\""), "{text}");
    assert!(text.contains("\"said \"\"hi\"\"\""), "{text}");
    let parsed = contacts_io::parse(&text, Some("book.csv")).expect("our own file");
    assert_eq!(parsed.contacts[0].name.as_deref(), Some("Alice, the one"));
    assert_eq!(parsed.contacts[0].note.as_deref(), Some("said \"hi\""));
    assert_eq!(parsed.contacts[0].favorite, Some(true));
    assert_eq!(parsed.groups[0].members, vec![A.to_owned()]);

    let csv = format!("address,name\n{A},Alice\nnot-an-address,Nobody\n");
    let parsed = contacts_io::parse(&csv, Some("book.csv")).expect("one is valid");
    assert_eq!(
        parsed.contacts.len(),
        2,
        "the bad row is the policy's to judge"
    );
    assert_eq!(parsed.contacts[1].address, "not-an-address");
}

/// An absent field is OMITTED (never `"name": ""`); a spreadsheet's `"true"`
/// string is a star; an object with neither key is an empty backup, not a
/// malformed one.
#[test]
fn json_omits_absent_fields_and_reads_string_true() {
    let text = contacts_io::to_json(&[manual(A, None, false, 1.0)], &[], STAMP);
    assert!(!text.contains("\"name\""), "{text}");
    assert!(!text.contains("\"favorite\""), "{text}");
    let theirs =
        format!("{{\"contacts\":[{{\"address\":\"{B}\",\"favorite\":\"true\",\"name\":\"\"}}]}}");
    let parsed = contacts_io::parse(&theirs, None).expect("an object");
    assert_eq!(parsed.contacts[0].favorite, Some(true));
    assert_eq!(parsed.contacts[0].name, None, "an empty name is no name");
    assert_eq!(
        contacts_io::parse("{}", Some("x.json")),
        Ok(contacts_io::ParsedContactsFile::default())
    );
    // A BOM in front of `{` is still JSON.
    assert!(contacts_io::parse("\u{feff}{}", None).is_ok());
}

// ---------------------------------------------------------------------------
// The A–Z directory (spec 028 US5 addendum) — the drawing's rule, in the core
// ---------------------------------------------------------------------------

use vela_core::app::contacts::{initial_of, section_contacts};
use vela_core::app::contacts_initials::section_letter;

/// Every name in the drawn roster (app-web `contacts/fixtures.ts`, the 018
/// canon) files where the drawing files it. The web filed every Chinese name
/// under `#` for two specs; this is the assertion that would have caught it.
#[test]
fn the_drawn_roster_files_where_the_drawing_files_it() {
    let roster = [
        ("Alice", 'A'),
        ("阿豪", 'A'),
        ("Bartholomew Vanderbilt-Konstantinopoulos.eth", 'B'),
        ("Bob · 泵泵", 'B'),
        ("Charlie", 'C'),
        ("DAO 金库", 'D'),
        ("hold on", 'H'),
        ("妈妈", 'M'),
        ("表弟", 'B'),
    ];
    for (name, letter) in roster {
        assert_eq!(section_letter(name), letter, "{name}");
    }
}

/// The edges of the rule: diacritics fold, a nameless address is `#`, so is
/// a symbol, a digit, whitespace-only and the empty string; polyphones take
/// their common reading.
#[test]
fn initials_fold_diacritics_and_file_the_nameless_under_hash() {
    assert_eq!(section_letter("éclair"), 'E');
    assert_eq!(section_letter("Ñandú"), 'N');
    assert_eq!(section_letter("  zoë"), 'Z');
    assert_eq!(section_letter("0x1234…abcd"), '#');
    assert_eq!(section_letter("Ø"), '#');
    assert_eq!(section_letter("42"), '#');
    assert_eq!(section_letter("   "), '#');
    assert_eq!(section_letter(""), '#');
    assert_eq!(initial_of('曾'), 'C');
    assert_eq!(initial_of('重'), 'Z');
    assert_eq!(initial_of('金'), 'J');
    // The scripts the app ships locales for (ja, ko) and two more ICU knows —
    // the iOS shell's transliteration filed these; the core must not do worse.
    assert_eq!(section_letter("さくら"), 'S');
    assert_eq!(section_letter("スズキ"), 'S');
    assert_eq!(initial_of('ヴ'), 'V');
    assert_eq!(section_letter("김민준"), 'G');
    assert_eq!(
        section_letter("안녕"),
        'A',
        "a silent ㅇ files under its vowel"
    );
    assert_eq!(section_letter("한"), 'H');
    assert_eq!(section_letter("Ελένη"), 'E');
    assert_eq!(section_letter("Дмитрий"), 'D');
    // Outside every table: Arabic, Devanagari, Thai file under `#` today.
    assert_eq!(initial_of('ع'), '#');
    assert_eq!(initial_of('क'), '#');
}

/// Sections run A–Z then `#`, and inside a letter the book's order holds —
/// favourites first, then most-recent — so the person you pay most stays on
/// top of their letter.
#[test]
fn sections_run_a_to_z_then_hash_and_keep_the_books_order_within_a_letter() {
    let mut anton = manual(A, Some("Anton"), false, 900.0);
    anton.address = A.to_owned();
    let alice = manual(B, Some("Alice"), true, 100.0); // starred → first overall
    let mama = manual(C, Some("妈妈"), false, 500.0);
    let nobody = manual(ME, None, false, 50.0); // unnamed → `#`
    let sut = booted(vec![anton, alice, mama, nobody], vec![], vec![], vec![]);
    let view = sut.view();
    let letters: Vec<&str> = view.sections.iter().map(|s| s.letter.as_str()).collect();
    assert_eq!(letters, vec!["A", "M", "#"]);
    // Alice (starred) before Anton (more recent, unstarred) — the book's order.
    assert_eq!(view.sections[0].addresses, vec![B.to_owned(), A.to_owned()]);
    assert_eq!(view.sections[1].addresses, vec![C.to_owned()]);
    assert_eq!(view.sections[2].addresses, vec![ME.to_owned()]);
    // The pure function agrees with the view.
    assert_eq!(section_contacts(&view.contacts), view.sections);
}
