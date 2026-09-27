//! Rules of the session machine, one test per rule.
//!
//! Inventory invariants ①–⑧ each have at least one test named after the rule.
//! The address-migration tests run the REAL `computeAddress` (the crate's safe
//! module) against the conformance fixture key, so the correction the machine
//! writes back is the same one the TS `vela-core` binding computes.
//!
//! Accounts that must keep a distinct fake address across a restore use an
//! empty `public_key_hex` ("legacy" records) — the migration skips them
//! exactly like the TS `!acct.publicKeyHex` falsy check does, which is itself
//! one of the ported rules.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::session::{
    Event, Session, SessionOperation as Op, SessionRoute, SessionShellResult as Res,
};
use vela_core::app::shell::CompletionMode;
use vela_core::app::{Account, AccountKey};

type Sut = DomainDriver<Session>;

const ADDR_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ADDR_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ADDR_C: &str = "0xcccccccccccccccccccccccccccccccccccccccc";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// The address the fixture public key actually computes to — the oracle the
/// migration must agree with.
fn correct_address() -> String {
    let key = vela_core::safe::parse_public_key(&support::expected_public_key_hex())
        .expect("fixture key parses");
    vela_core::safe::compute_safe_address(&key.x, &key.y)
        .expect("fixture address computes")
        .address
}

/// A legacy record without a public key: `!acct.publicKeyHex` ⇒ the migration
/// must leave its address alone, whatever it is.
fn legacy(id: &str, name: &str, address: &str) -> Account {
    Account {
        public_key_hex: String::new(),
        ..support::account(id, name, address)
    }
}

/// A record whose key does not even parse — `computeAddress` fails, the old
/// address must be kept.
fn bad_key(id: &str, name: &str, address: &str) -> Account {
    Account {
        public_key_hex: "zz-not-hex".to_owned(),
        ..support::account(id, name, address)
    }
}

/// A second founding key for multi-key fixtures. Shape-valid (parse only
/// checks 04‖x‖y layout); the multi derivation treats it as keys[1].
const SECOND_KEY_HEX: &str = "04\
8f9b2c3d4e5f60718293a4b5c6d7e8f90112233445566778899aabbccddeeff0\
7a8b9cadbecfd0e1f20314253647586970818293a4b5c6d7e8f9010203040506";

/// A two-key wallet whose scalar fields mirror keys[0], stored with `address`.
fn multi(id: &str, name: &str, address: &str) -> Account {
    Account {
        keys: vec![
            AccountKey {
                credential_id: id.to_owned(),
                public_key_hex: support::expected_public_key_hex(),
                name: name.to_owned(),
                transports: "internal".to_owned(),
                signer_origin: None,
            },
            AccountKey {
                credential_id: format!("{id}-second"),
                public_key_hex: SECOND_KEY_HEX.to_owned(),
                name: "Key 2".to_owned(),
                transports: "usb,nfc".to_owned(),
                signer_origin: None,
            },
        ],
        ..support::account(id, name, address)
    }
}

/// The address the two-key fixture set actually computes to.
fn correct_multi_address() -> String {
    let keys = [
        vela_core::safe::parse_public_key(&support::expected_public_key_hex())
            .expect("fixture key parses"),
        vela_core::safe::parse_public_key(SECOND_KEY_HEX).expect("second key parses"),
    ];
    vela_core::safe::compute_safe_address_multi(&keys)
        .expect("multi fixture address computes")
        .address
}

/// Boot a machine through a full restore and ack every write it issued, so
/// tests start from a settled session with an empty request queue.
fn restored(accounts: Vec<Account>, saved_index: usize) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Boot);
    assert_eq!(ops, vec![Op::LoadAccounts, Op::LoadActiveIndex]);
    assert!(
        sut.resolve(Res::AccountsLoaded { accounts }).is_empty(),
        "still gathering — the index has not answered"
    );
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: saved_index });
    for op in ops {
        let ack = match op {
            Op::SaveAccount { .. } => Res::AccountSaved,
            Op::SaveActiveIndex { .. } => Res::ActiveIndexSaved,
            other => panic!("unexpected restore op {other:?}"),
        };
        assert!(sut.resolve(ack).is_empty(), "write acks are inert");
    }
    sut
}

// ===========================================================================
// Boot & restore
// ===========================================================================

#[test]
fn boot_reads_storage_once() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Boot);
    assert_eq!(ops, vec![Op::LoadAccounts, Op::LoadActiveIndex]);
    // The restore effect ran once per process (`useEffect([])`); so does Boot.
    assert!(sut.dispatch(Event::Boot).is_empty());
}

/// Invariant ① — after the restore, `address` is exactly the saved-index
/// account's address.
#[test]
fn restore_activates_the_saved_account() {
    let sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        1,
    );
    let view = sut.view();
    assert!(!view.loading);
    assert!(view.has_wallet);
    assert_eq!(view.active_index, 1);
    assert_eq!(view.address, ADDR_B);
    assert_eq!(view.allowed_route, SessionRoute::Wallet);
    assert_eq!(view.accounts.len(), 2);
}

/// The two loads race in parallel (`Promise.all`) — either arrival order must
/// settle the same session.
#[test]
fn restore_result_order_does_not_matter() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    assert!(
        sut.resolve(Res::ActiveIndexLoaded { index: 1 }).is_empty(),
        "index alone decides nothing"
    );
    let ops = sut.resolve(Res::AccountsLoaded {
        accounts: vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 1 }]);
    assert_eq!(sut.view().address, ADDR_B);
}

/// Invariant ③ (empty half) — no accounts is `LOADED_EMPTY`: loading clears,
/// onboarding is the allowed route, and nothing is persisted.
#[test]
fn empty_storage_lands_empty() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    assert!(sut
        .resolve(Res::AccountsLoaded { accounts: vec![] })
        .is_empty());
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 3 });
    assert!(ops.is_empty(), "no index persist for an empty wallet");
    let view = sut.view();
    assert!(!view.loading);
    assert!(!view.has_wallet);
    assert_eq!(view.address, "");
    assert_eq!(view.allowed_route, SessionRoute::Onboarding);
}

/// Invariant ③ — a failed read lands Empty, never a forever-spinner; the
/// other load's late answer is inert.
#[test]
fn restore_failure_lands_empty_not_stuck() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    assert!(sut.resolve(Res::AccountsUnavailable).is_empty());
    let view = sut.view();
    assert!(!view.loading, "isLoading MUST clear on failure");
    assert_eq!(view.allowed_route, SessionRoute::Onboarding);
    // The index read answers late — inert.
    assert!(sut.resolve(Res::ActiveIndexLoaded { index: 1 }).is_empty());
    assert_eq!(sut.view(), view);
}

/// Invariant ④ — during the loading window the index is NEVER persisted (the
/// initial 0 would overwrite the user's saved value); the persist happens
/// exactly when the window closes.
#[test]
fn loading_window_never_persists_the_index() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    assert!(
        sut.resolve(Res::ActiveIndexLoaded { index: 2 }).is_empty(),
        "no SaveActiveIndex while accounts are still loading"
    );
    // A premature switch cannot persist anything either — the list is empty,
    // so it is the invariant-① whole no-op.
    assert!(sut.dispatch(Event::SwitchAccount { index: 0 }).is_empty());
    assert!(sut.view().loading);
    assert_eq!(sut.view().allowed_route, SessionRoute::Loading);
    let ops = sut.resolve(Res::AccountsLoaded {
        accounts: vec![
            legacy("c1", "Ann", ADDR_A),
            legacy("c2", "Bo", ADDR_B),
            legacy("c3", "Cy", ADDR_C),
        ],
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 2 }]);
    assert_eq!(sut.view().address, ADDR_C);
}

/// Invariant ③ — an out-of-range saved index clamps to 0, and the clamped
/// value is what gets persisted (durably repairing the stored one).
#[test]
fn saved_index_out_of_range_clamps_to_zero() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 7 });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    let view = sut.view();
    assert_eq!(view.active_index, 0);
    assert_eq!(view.address, ADDR_A, "never an empty address (invariant ①)");
}

// ===========================================================================
// Address migration (invariant ②)
// ===========================================================================

/// A stored address that disagrees with `computeAddress(publicKeyHex)` is
/// corrected BEFORE it is ever shown, and the corrected record is written
/// back.
#[test]
fn migration_rewrites_a_wrong_address() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    // `support::account` carries the real fixture public key; ADDR_A is not
    // what it computes to.
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![support::account("c1", "Ann", ADDR_A)],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    let corrected = correct_address();
    assert_eq!(
        ops,
        vec![
            Op::SaveAccount {
                account: Account {
                    address: corrected.clone(),
                    ..support::account("c1", "Ann", ADDR_A)
                },
            },
            Op::SaveActiveIndex { index: 0 },
        ]
    );
    // The view shows the corrected address immediately — not after the write.
    assert_eq!(sut.view().address, corrected);
    // The best-effort write acks are inert either way.
    assert!(sut.resolve(Res::AccountSaved).is_empty());
    assert!(sut.resolve(Res::ActiveIndexSaved).is_empty());
    assert_eq!(sut.view().address, corrected);
}

/// Invariant ② on a multi-key account derives from ALL founding keys: a
/// record already carrying its correct multi-key address is left untouched —
/// in particular it is NOT rewritten to what keys[0] alone would compute
/// (the exact corruption a single-key derivation here would cause).
#[test]
fn migration_leaves_a_correct_multikey_address_alone() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    let multi_addr = correct_multi_address();
    assert_ne!(
        multi_addr,
        correct_address(),
        "fixture must distinguish multi from keys[0]-only derivation"
    );
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![multi("c1", "Ann", &multi_addr)],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    assert_eq!(
        ops,
        vec![Op::SaveActiveIndex { index: 0 }],
        "no address write-back for a correct multi-key account"
    );
    assert_eq!(sut.view().address, multi_addr);
}

/// Invariant ② still repairs a multi-key account whose stored address is
/// wrong — to the multi-key address, never the keys[0] one.
#[test]
fn migration_rewrites_a_wrong_multikey_address_to_the_multi_derivation() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![multi("c1", "Ann", ADDR_A)],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    let corrected = correct_multi_address();
    match &ops[0] {
        Op::SaveAccount { account } => {
            assert_eq!(account.address, corrected);
            assert_eq!(account.keys.len(), 2, "the key set survives the repair");
        }
        other => panic!("expected the multi write-back, got {other:?}"),
    }
    assert_eq!(sut.view().address, corrected);
}

/// One account's migration failing keeps ITS old address and never blocks the
/// others; a keyless legacy record is skipped entirely.
#[test]
fn migration_failure_keeps_the_old_address_without_blocking_others() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![
            bad_key("c1", "Bad", ADDR_A),
            support::account("c2", "Ann", ADDR_B),
            legacy("c3", "Old", ADDR_C),
        ],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    // Exactly one write-back: only the account whose key computed a different
    // address.
    assert_eq!(ops.len(), 2, "one SaveAccount + the index persist");
    match &ops[0] {
        Op::SaveAccount { account } => {
            assert_eq!(account.id, "c2");
            assert_eq!(account.address, correct_address());
        }
        other => panic!("expected the c2 write-back, got {other:?}"),
    }
    let view = sut.view();
    assert_eq!(view.accounts.len(), 3, "nothing was dropped");
    assert_eq!(
        view.accounts[0].account.address, ADDR_A,
        "failed compute keeps the old address"
    );
    assert_eq!(view.accounts[1].account.address, correct_address());
    assert_eq!(
        view.accounts[2].account.address, ADDR_C,
        "keyless record untouched"
    );
    assert_eq!(
        view.allowed_route,
        SessionRoute::Wallet,
        "restore completed"
    );
}

/// A record whose address already matches is not rewritten — no write, no
/// churn.
#[test]
fn matching_addresses_are_not_rewritten() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![support::account("c1", "Ann", &correct_address())],
    });
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
}

// ===========================================================================
// Switching (invariants ① and ⑦)
// ===========================================================================

/// Invariant ① — an out-of-range switch is a WHOLE no-op: the address is not
/// blanked, the index does not move, nothing is persisted.
#[test]
fn switch_out_of_range_is_a_whole_noop() {
    let mut sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        0,
    );
    let before = sut.view();
    assert!(sut.dispatch(Event::SwitchAccount { index: 5 }).is_empty());
    assert_eq!(sut.view(), before);
    assert_eq!(sut.view().address, ADDR_A, "address survives untouched");
}

/// A valid switch moves the derived address with the index and persists the
/// raw index.
#[test]
fn switch_updates_address_and_persists_the_raw_index() {
    let mut sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        0,
    );
    let ops = sut.dispatch(Event::SwitchAccount { index: 1 });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 1 }]);
    let view = sut.view();
    assert_eq!(view.active_index, 1);
    assert_eq!(view.address, ADDR_B);
}

/// Ported React quirk: switching to the already-active index re-renders but
/// never persists (the effect's deps are unchanged).
#[test]
fn switch_to_the_current_index_never_persists() {
    let mut sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        1,
    );
    assert!(sut.dispatch(Event::SwitchAccount { index: 1 }).is_empty());
    assert_eq!(sut.view().address, ADDR_B);
}

/// Invariant ⑦ — the view rows carry each account's ORIGINAL index. A
/// balance-sorted switcher (which may show "Bo" first) must dispatch the
/// row's `index`, and doing so activates that row's account — never whoever
/// happens to sit at the same display position.
#[test]
fn switcher_rows_carry_the_original_index() {
    let mut sut = restored(
        vec![legacy("c1", "Zoe", ADDR_A), legacy("c2", "Amy", ADDR_B)],
        0,
    );
    let view = sut.view();
    assert_eq!(view.accounts[0].index, 0);
    assert_eq!(view.accounts[0].account.name, "Zoe");
    assert_eq!(view.accounts[1].index, 1);
    assert_eq!(view.accounts[1].account.name, "Amy");
    // However the shell reorders for display, the ride-along index is what it
    // dispatches — and it lands on exactly that account.
    let amy = view.accounts[1].clone();
    sut.dispatch(Event::SwitchAccount { index: amy.index });
    assert_eq!(sut.view().address, amy.account.address);
}

// ===========================================================================
// The onboarding hand-off (invariant ⑥, the unified dual entry)
// ===========================================================================

/// Invariant ⑥ — ADD_ACCOUNT: the established account is appended and MUST
/// become active.
#[test]
fn established_account_becomes_active() {
    let mut sut = restored(vec![], 0); // booted empty → onboarding
    let first = legacy("c1", "First", ADDR_A);
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: first.clone(),
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    let view = sut.view();
    assert!(view.has_wallet);
    assert_eq!(view.address, ADDR_A);
    assert_eq!(view.allowed_route, SessionRoute::Wallet);

    // A second account (added later from settings) activates too.
    let second = legacy("c2", "Second", ADDR_B);
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount { account: second },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 1 }]);
    assert_eq!(sut.view().active_index, 1);
    assert_eq!(sut.view().address, ADDR_B);
}

/// SET_WALLET: a locally-known sign-in restores the whole list with the right
/// account selected — the other half of the dual entry.
#[test]
fn established_set_wallet_restores_the_list() {
    let mut sut = restored(vec![], 0);
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::SetWallet {
            accounts: vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
            active_index: 1,
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 1 }]);
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2);
    assert_eq!(view.address, ADDR_B);
    assert_eq!(view.allowed_route, SessionRoute::Wallet);
}

/// An out-of-range hand-off index clamps to 0 (the reducer would render
/// `address: ''` beside `hasWallet: true`, which invariant ① forbids; every
/// live dispatch site pre-clamps, so this is the fail-closed port of an
/// unreachable branch).
#[test]
fn established_set_wallet_clamps_an_out_of_range_index() {
    let mut sut = restored(vec![], 0);
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::SetWallet {
            accounts: vec![legacy("c1", "Ann", ADDR_A)],
            active_index: 9,
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    let view = sut.view();
    assert_eq!(view.active_index, 0);
    assert_eq!(view.address, ADDR_A, "never an empty address");
}

/// The resident-machine staleness rule: a restore that resolves AFTER
/// onboarding already established the session is dropped whole — the stored
/// list (written before the hand-off) must not clobber the live one.
#[test]
fn late_restore_results_never_clobber_an_established_session() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Boot); // loads in flight…
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: legacy("c9", "New", ADDR_C),
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    // …and only now do the boot reads answer: both stale, both dropped.
    assert!(sut
        .resolve(Res::AccountsLoaded {
            accounts: vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        })
        .is_empty());
    assert!(sut.resolve(Res::ActiveIndexLoaded { index: 1 }).is_empty());
    let view = sut.view();
    assert_eq!(view.accounts.len(), 1, "the established session stands");
    assert_eq!(view.address, ADDR_C);
}

// ===========================================================================
// Sign-out (invariant ⑤) and the ported logout semantics
// ===========================================================================

fn active_pair() -> Sut {
    restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        0,
    )
}

/// One wallet leaves, the others stay — the half a person could not reach.
///
/// Signing out took every row, so a device holding six could sign out of all
/// six or none (owner, 2026-09-23: 「有时候不想退出所有，只想退出单个」).
/// Removing a row is not deleting a wallet: the passkey still opens it, and
/// the row comes back on the next sign-in with everything keyed to its
/// address — the same reason the wide half is safe.
#[test]
fn one_account_can_leave_without_taking_the_others() {
    // Two wallets, the SECOND active: removing the first must not leave the
    // index pointing past its row.
    let mut sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        1,
    );
    let ops = sut.dispatch(Event::RemoveAccount { index: 0 });
    assert_eq!(
        ops,
        vec![
            Op::RemoveAccount {
                address: ADDR_A.to_owned()
            },
            Op::SaveActiveIndex { index: 0 },
        ],
        "the row goes by ADDRESS, and the active one follows it down"
    );
    let view = sut.view();
    assert!(view.has_wallet, "the other wallet is still signed in");
    assert_eq!(view.accounts.len(), 1);
    assert_eq!(view.address, ADDR_B, "and it is the one that was active");
    assert_eq!(view.allowed_route, SessionRoute::Wallet);
}

/// Removing the ACTIVE row puts another wallet in front of the person — and
/// drops the extension's snapshot, which mirrors the active account and would
/// otherwise keep answering as a wallet this device just put away.
#[test]
fn removing_the_active_wallet_lands_on_another_and_clears_the_mirror() {
    let mut sut = active_pair();
    let ops = sut.dispatch(Event::RemoveAccount { index: 0 });
    assert_eq!(
        ops,
        vec![
            Op::RemoveAccount {
                address: ADDR_A.to_owned()
            },
            Op::SaveActiveIndex { index: 0 },
            Op::ClearExtensionCache,
        ]
    );
    let view = sut.view();
    assert_eq!(view.address, ADDR_B);
    assert_eq!(view.active_index, 0);
}

/// The LAST row leaving is being signed out, reached the same way — there is
/// one definition of "this device is not signed in", and a wallet-less
/// `Active` phase is what invariant ① forbids.
#[test]
fn removing_the_last_wallet_is_signing_out() {
    let mut sut = restored(vec![legacy("c1", "Ann", ADDR_A)], 0);
    let ops = sut.dispatch(Event::RemoveAccount { index: 0 });
    assert_eq!(ops, vec![Op::ClearSignedInWallet, Op::ClearExtensionCache]);
    let view = sut.view();
    assert!(!view.has_wallet);
    assert_eq!(view.address, "");
    assert_eq!(view.allowed_route, SessionRoute::Onboarding);
}

/// A row the list does not have is the whole action's no-op — as a switch is.
/// A balance-sorted display that reordered under a slow write must not be able
/// to remove a stranger.
#[test]
fn removing_a_row_that_is_not_there_does_nothing() {
    let mut sut = active_pair();
    assert!(sut.dispatch(Event::RemoveAccount { index: 7 }).is_empty());
    assert_eq!(sut.view().accounts.len(), 2);
}

/// The sign-out dialog knows how many wallets it is about to take, so the copy
/// can say so instead of implying one.
#[test]
fn the_sign_out_dialog_counts_the_wallets_it_takes() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    sut.resolve(Res::PendingUploads { has_pending: false });
    assert_eq!(
        sut.view().sign_out.map(|s| s.account_count),
        Some(2),
        "six wallets and one wallet cannot read the same"
    );
}

/// Invariant ⑤ — signing out with un-synced pending uploads warns; and LOGOUT
/// ends the sign-in on disk too (open question 2, decided in spec 017): the
/// confirm emits both clears alongside the in-memory wipe.
#[test]
fn sign_out_warns_when_uploads_are_pending() {
    let mut sut = active_pair();
    let ops = sut.dispatch(Event::SignOut);
    assert_eq!(ops, vec![Op::CheckPendingUploads]);
    assert!(
        sut.view().sign_out.is_none(),
        "the dialog waits for the check"
    );
    assert!(sut
        .resolve(Res::PendingUploads { has_pending: true })
        .is_empty());
    let view = sut.view();
    assert_eq!(
        view.sign_out.map(|s| s.pending_upload_warning),
        Some(true),
        "the warning rides the dialog"
    );

    let ops = sut.dispatch(Event::SignOutConfirmed);
    assert_eq!(
        ops,
        vec![Op::ClearSignedInWallet, Op::ClearExtensionCache],
        "the sign-in has to leave the disk too — otherwise the next launch \
         restores the session the user just ended"
    );
    let view = sut.view();
    assert!(!view.has_wallet);
    assert!(view.accounts.is_empty());
    assert_eq!(view.address, "");
    assert!(!view.loading);
    assert_eq!(view.allowed_route, SessionRoute::Onboarding);
    assert!(view.sign_out.is_none());
}

#[test]
fn sign_out_without_pending_uploads_carries_no_warning() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    sut.resolve(Res::PendingUploads { has_pending: false });
    assert_eq!(
        sut.view().sign_out.map(|s| s.pending_upload_warning),
        Some(false)
    );
}

/// Invariant ⑤ structurally — there is no confirm path that skips the check:
/// without the dialog open, `SignOutConfirmed` is inert, including while the
/// check is still in flight.
#[test]
fn confirm_without_the_dialog_is_inert() {
    let mut sut = active_pair();
    assert!(sut.dispatch(Event::SignOutConfirmed).is_empty());
    assert!(sut.view().has_wallet, "no dialog → no logout");

    sut.dispatch(Event::SignOut); // check in flight, dialog not open yet
    assert!(sut.dispatch(Event::SignOutConfirmed).is_empty());
    assert!(sut.view().has_wallet);
}

/// Ported verbatim: `hasPendingUploads()` throwing kills `handleOpenSignOut`
/// before `setShowSignOut(true)` — the dialog silently never opens. Fail-closed
/// for ⑤, and the user can simply tap again.
#[test]
fn failed_upload_check_leaves_the_dialog_closed() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    assert!(sut.resolve(Res::PendingUploadsUnavailable).is_empty());
    assert!(sut.view().sign_out.is_none());
    assert!(sut.view().has_wallet, "still signed in");
    // The next tap re-checks.
    assert_eq!(sut.dispatch(Event::SignOut), vec![Op::CheckPendingUploads]);
}

/// Dismissing closes the dialog without logging out; reopening re-runs the
/// pending-upload check (its answer may have changed).
#[test]
fn dismiss_closes_the_dialog_and_reopen_rechecks() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    sut.resolve(Res::PendingUploads { has_pending: true });
    assert!(sut.dispatch(Event::SignOutDismissed).is_empty());
    let view = sut.view();
    assert!(view.sign_out.is_none());
    assert!(view.has_wallet, "dismiss is not a logout");
    assert_eq!(view.allowed_route, SessionRoute::Wallet);

    assert_eq!(sut.dispatch(Event::SignOut), vec![Op::CheckPendingUploads]);
}

/// The check is single-flight and the dialog does not stack.
#[test]
fn sign_out_is_single_flight() {
    let mut sut = active_pair();
    assert_eq!(sut.dispatch(Event::SignOut), vec![Op::CheckPendingUploads]);
    assert!(sut.dispatch(Event::SignOut).is_empty(), "check in flight");
    sut.resolve(Res::PendingUploads { has_pending: false });
    assert!(
        sut.dispatch(Event::SignOut).is_empty(),
        "dialog already open"
    );
}

/// Sign-out is a settings action — it needs a signed-in session.
#[test]
fn sign_out_requires_an_active_session() {
    let mut sut = restored(vec![], 0); // Empty
    assert!(sut.dispatch(Event::SignOut).is_empty());
    assert!(sut.dispatch(Event::SignOutDismissed).is_empty());
}

/// The clear acknowledgements are inert: they arrive after the session is
/// already signed out and must not resurrect anything.
#[test]
fn the_logout_clear_acks_change_nothing() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    sut.resolve(Res::PendingUploads { has_pending: false });
    sut.dispatch(Event::SignOutConfirmed);
    assert!(sut.resolve(Res::SignedInWalletCleared).is_empty());
    assert!(sut.resolve(Res::ExtensionCacheCleared).is_empty());
    let view = sut.view();
    assert!(!view.has_wallet);
    assert!(!view.loading);
    assert_eq!(view.allowed_route, SessionRoute::Onboarding);
}

/// After a logout the same process can onboard again — the hand-off re-enters
/// an Active session, and a clear ack that lands late is dropped by attempt.
#[test]
fn sign_in_again_after_sign_out() {
    let mut sut = active_pair();
    sut.dispatch(Event::SignOut);
    sut.resolve(Res::PendingUploads { has_pending: false });
    sut.dispatch(Event::SignOutConfirmed);
    assert_eq!(sut.view().allowed_route, SessionRoute::Onboarding);

    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: legacy("c9", "Back", ADDR_C),
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    let view = sut.view();
    assert_eq!(view.allowed_route, SessionRoute::Wallet);
    assert_eq!(view.address, ADDR_C);
}

// ===========================================================================
// Route guard (invariant ⑧) & mispaired results
// ===========================================================================

/// Invariant ⑧ — the allowed route follows the session phase: no judgment
/// while loading, wallet only with a wallet, onboarding otherwise.
#[test]
fn route_gate_follows_the_session_phase() {
    let mut sut = Sut::new();
    // Pristine = still loading (INITIAL_STATE.isLoading) — no redirect yet.
    assert_eq!(sut.view().allowed_route, SessionRoute::Loading);
    sut.dispatch(Event::Boot);
    assert_eq!(sut.view().allowed_route, SessionRoute::Loading);
    sut.resolve(Res::AccountsLoaded {
        accounts: vec![legacy("c1", "Ann", ADDR_A)],
    });
    assert_eq!(
        sut.view().allowed_route,
        SessionRoute::Loading,
        "half a restore is still loading"
    );
    sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    assert_eq!(sut.view().allowed_route, SessionRoute::Wallet);
}

/// A result the machine never asked for (or no longer expects) changes
/// nothing — here a pending-upload answer arriving with no check in flight
/// must not open the dialog.
#[test]
fn mispaired_results_are_inert() {
    let mut sut = active_pair();
    sut.dispatch(Event::SwitchAccount { index: 1 }); // SaveActiveIndex outstanding
    let before = sut.view();
    assert!(sut
        .resolve(Res::PendingUploads { has_pending: true })
        .is_empty());
    assert_eq!(sut.view(), before, "no dialog from an unrequested answer");
}

// ---------------------------------------------------------------------------
// Spec 048: the retired client's records restore the session too
// ---------------------------------------------------------------------------

#[test]
fn an_expo_era_record_restores_the_session() {
    let stored = Account {
        id: "cred-a".to_owned(),
        name: "Ann".to_owned(),
        address: ADDR_A.to_owned(),
        public_key_hex: "04ab".to_owned(),
        created_at_iso: "2026-08-25T10:00:00.000Z".to_owned(),
        keys: vec![AccountKey {
            credential_id: "cred-a".to_owned(),
            public_key_hex: "04ab".to_owned(),
            name: "Ann".to_owned(),
            transports: String::new(),
            signer_origin: None,
        }],
        signed_in_with: None,
    };
    let json = serde_json::to_string(&Res::AccountsLoaded {
        accounts: vec![stored.clone()],
    })
    .unwrap()
    .replace("\"public_key_hex\"", "\"publicKeyHex\"")
    .replace("\"created_at_iso\"", "\"createdAt\"")
    .replace("\"credential_id\"", "\"credentialId\"")
    .replace(",\"transports\":\"\"", "");
    let loaded: Res =
        serde_json::from_str(&json).expect("the core reads the retired client's spelling");
    assert_eq!(
        loaded,
        Res::AccountsLoaded {
            accounts: vec![stored]
        }
    );

    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Boot);
    assert_eq!(ops, vec![Op::LoadAccounts, Op::LoadActiveIndex]);
    assert!(sut.resolve(loaded).is_empty());
    let ops = sut.resolve(Res::ActiveIndexLoaded { index: 0 });
    for op in ops {
        let ack = match op {
            Op::SaveAccount { .. } => Res::AccountSaved,
            Op::SaveActiveIndex { .. } => Res::ActiveIndexSaved,
            other => panic!("unexpected restore op {other:?}"),
        };
        sut.resolve(ack);
    }
    let view = sut.view();
    assert_eq!(view.address, ADDR_A);
    assert_eq!(view.allowed_route, SessionRoute::Wallet);
}

// ===========================================================================
// One wallet, one row (invariant ⑨)
// ===========================================================================

/// A wallet this device already holds is ACTIVATED, not appended again.
///
/// A multi-key wallet signed into with a second passkey recovers to the same
/// ADDRESS under a different credential id, so the credential match in
/// `login.rs` misses it. It used to arrive here as a plain `AddAccount` and
/// land in the list twice: one wallet, two rows, both opening the same Safe.
#[test]
fn an_address_already_held_is_activated_not_appended() {
    let mut sut = restored(
        vec![legacy("c1", "Ann", ADDR_A), legacy("c2", "Bo", ADDR_B)],
        1,
    );

    // The same wallet as `c1`, recovered under another credential.
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: legacy("c1-second-key", "Ann", ADDR_A),
        },
    });

    // Activated in place — and it IS the active one, as invariant ⑥ requires.
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 0 }]);
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2, "no second row for one wallet");
    assert_eq!(view.active_index, 0);
    assert_eq!(view.address, ADDR_A);
    // The record that was already here is the one kept, name and all.
    assert_eq!(view.accounts[0].account.id, "c1");
}

/// Case is not identity: the same Safe written in two checksummings is one
/// wallet.
#[test]
fn an_address_held_in_another_casing_is_the_same_wallet() {
    let mut sut = restored(vec![legacy("c1", "Ann", ADDR_A)], 0);
    let shouted = ADDR_A.to_uppercase().replace("0X", "0x");
    sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: legacy("c1-second-key", "Ann", &shouted),
        },
    });
    assert_eq!(sut.view().accounts.len(), 1);
}

/// A genuinely different wallet still appends and still becomes active.
#[test]
fn a_new_address_still_appends() {
    let mut sut = restored(vec![legacy("c1", "Ann", ADDR_A)], 0);
    let ops = sut.dispatch(Event::AccountEstablished {
        mode: CompletionMode::AddAccount {
            account: legacy("c2", "Bo", ADDR_B),
        },
    });
    assert_eq!(ops, vec![Op::SaveActiveIndex { index: 1 }]);
    assert_eq!(sut.view().accounts.len(), 2);
    assert_eq!(sut.view().address, ADDR_B);
}

/// A device that ALREADY stores the pair is healed on the next boot, and the
/// saved index follows its wallet rather than its old slot.
#[test]
fn a_stored_duplicate_collapses_on_restore() {
    // Saved index 2 points at the SECOND copy of Ann's wallet.
    let sut = restored(
        vec![
            legacy("c1", "Ann", ADDR_A),
            legacy("c2", "Bo", ADDR_B),
            legacy("c1-second-key", "Ann", ADDR_A),
        ],
        2,
    );
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2, "one wallet, one row");
    assert_eq!(view.accounts[0].account.id, "c1");
    assert_eq!(view.accounts[1].account.id, "c2");
    assert_eq!(view.address, ADDR_A, "still Ann's wallet, not Bo's");
    assert_eq!(view.active_index, 0);
}

/// Dropping a row ABOVE the active one must not select a different account.
#[test]
fn collapsing_a_row_above_the_active_one_keeps_the_same_wallet() {
    let sut = restored(
        vec![
            legacy("c1", "Ann", ADDR_A),
            legacy("c1-second-key", "Ann", ADDR_A),
            legacy("c2", "Bo", ADDR_B),
        ],
        2,
    );
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2);
    assert_eq!(view.address, ADDR_B, "the saved index followed Bo");
    assert_eq!(view.active_index, 1);
}

/// Invariant ③ is not weakened by the collapse: an out-of-range saved index
/// still lands on 0 rather than on whatever row happens to be last.
#[test]
fn an_out_of_range_index_still_lands_on_zero_after_collapsing() {
    let sut = restored(
        vec![
            legacy("c1", "Ann", ADDR_A),
            legacy("c1-second-key", "Ann", ADDR_A),
            legacy("c2", "Bo", ADDR_B),
        ],
        99,
    );
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2);
    assert_eq!(view.active_index, 0);
    assert_eq!(view.address, ADDR_A);
}
