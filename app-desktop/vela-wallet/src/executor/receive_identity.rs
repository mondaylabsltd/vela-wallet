//! §37 (review 1-7): an account the key index alone vouched for.
//!
//! A multi-key group whose members only the index named is saved
//! `unconfirmed` by the core (`login::reconstruct_account`): the person is
//! in and can sign, but the wallet shows no address to receive at — a lying
//! index can hand back the person's real credential beside an attacker's
//! keys, and that address is one the attacker co-owns. Whether an account
//! may be received at, and what the Receive screen says when it may not, is
//! the core's `receive_standing`; this file only asks the chain again
//! (`registry::confirm_identity`, the core's chain-only `confirm_step`) and
//! saves what it found.
//!
//! One check at a time, process-wide: Receive's open, its Check again and
//! the launch's background re-check all come here.

use std::sync::atomic::{AtomicBool, Ordering};

use vela_core::app::{Account, ReceiveStanding};
use vela_core::registry_resolve::{Identity, VerifiedBy};

use super::storage;

/// A confirm walk is running.
static CHECKING: AtomicBool = AtomicBool::new(false);

/// Is the chain being asked right now? Receive says "Checking on-chain…"
/// and offers no Check again while it is.
#[must_use]
pub fn checking() -> bool {
    CHECKING.load(Ordering::Acquire)
}

/// The stored account at `address`, if this device holds one.
fn stored(address: &str) -> Option<Account> {
    storage::load_accounts()
        .ok()?
        .into_iter()
        .find(|account| account.address.eq_ignore_ascii_case(address))
}

/// The core's word on receiving at the account at `address`. `None` when no
/// stored account is at it (the mocks; a multisig is not a Vela account).
#[must_use]
pub fn standing_of(address: &str) -> Option<ReceiveStanding> {
    stored(address).map(|account| vela_core::app::receive_standing(&account))
}

/// The key the confirm walk asks the chain about: the account's first
/// founding key — or the scalar one of a record from before `keys`.
fn walk_key(account: &Account) -> Option<String> {
    account
        .keys
        .first()
        .map(|key| key.public_key_hex.clone())
        .or_else(|| Some(account.public_key_hex.clone()))
        .filter(|key| !key.trim().is_empty())
}

/// Claim the one check: `false` when one is already running.
#[must_use]
pub fn try_begin() -> bool {
    CHECKING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
}

/// Does the account at `address` want its identity checked before Receive
/// shows anything (`ReceiveStanding.check_first`)?
#[must_use]
pub fn wants_check(address: &str) -> bool {
    standing_of(address).is_some_and(|standing| standing.check_first)
}

/// Run a check [`try_begin`] claimed, for the account at `address`, and
/// release it. Blocking — the chain is asked — so a background thread's.
/// Answers the identity now stored, `None` when there was nothing to check.
pub fn run_begun(address: &str) -> Option<Identity> {
    let found = check_with(address, super::registry::confirm_identity);
    CHECKING.store(false, Ordering::Release);
    found
}

/// [`run_begun`]'s work, the walk being `confirm`.
fn check_with(
    address: &str,
    confirm: impl FnOnce(&str, &str) -> (Identity, VerifiedBy),
) -> Option<Identity> {
    let account = stored(address)?;
    if !vela_core::app::receive_standing(&account).check_first {
        return Some(account.identity);
    }
    let key = walk_key(&account)?;
    let (found, verified_by) = confirm(&key, &account.address);
    crate::diag::vlog!(
        "identity",
        "confirm walk: {} (verified by {:?})",
        serde_json::to_value(found)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default(),
        verified_by
    );
    apply(&account.id, found)
}

/// Save what the chain found on the stored account `id` — re-read now, so
/// a rename made while the chain was asked is not undone; only `identity`
/// changes (`vela_core::app::account_with_identity_json`'s rule). Nothing is
/// written when it says what is stored already. Answers the identity stored.
fn apply(id: &str, found: Identity) -> Option<Identity> {
    let mut account = storage::load_accounts()
        .ok()?
        .into_iter()
        .find(|account| account.id == id)?;
    if account.identity == found {
        return Some(found);
    }
    account.identity = found;
    match storage::save_account(&account) {
        Ok(()) => Some(found),
        Err(error) => {
            crate::diag::vlog!("identity", "could not save the identity: {}", error);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    /// A two-key wallet, as a sign-in from the index rebuilds one.
    fn account(identity: Identity) -> Account {
        Account {
            id: "cred0".to_owned(),
            name: "Everyday wallet".to_owned(),
            address: ADDRESS.to_owned(),
            public_key_hex: "0400".to_owned(),
            created_at_iso: "2026-11-08T00:00:00.000Z".to_owned(),
            keys: (0..2)
                .map(|index| vela_core::app::AccountKey {
                    credential_id: format!("cred{index}"),
                    public_key_hex: format!("04{index:02}"),
                    name: format!("Key {}", index + 1),
                    transports: "internal".to_owned(),
                    signer_origin: None,
                })
                .collect(),
            sign_in_key: None,
            signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
            signing_venue: vela_core::signing_venue::SigningVenue::InVela,
            identity,
        }
    }

    /// The core's standing is what Receive draws: an unconfirmed account
    /// shows no address and asks the chain first; a refuted one shows no
    /// address and never asks; a confirmed one is the normal Receive.
    #[test]
    fn the_standing_is_the_cores_for_each_identity() {
        storage::tests::with_temp_state("receive-standing", || {
            assert!(standing_of(ADDRESS).is_none(), "no account, no standing");
            for (identity, receivable, check_first, title) in [
                (Identity::Confirmed, true, false, None),
                (
                    Identity::Unconfirmed,
                    false,
                    true,
                    Some("receive.unconfirmed.title"),
                ),
                (
                    Identity::Refuted,
                    false,
                    false,
                    Some("receive.refuted.title"),
                ),
            ] {
                assert!(storage::save_account(&account(identity)).is_ok());
                let standing = standing_of(&ADDRESS.to_lowercase())
                    .unwrap_or_else(|| unreachable!("the stored account"));
                assert_eq!(standing.receivable, receivable, "{identity:?}");
                assert_eq!(standing.check_first, check_first, "{identity:?}");
                assert_eq!(standing.title_key.as_deref(), title, "{identity:?}");
                assert_eq!(wants_check(ADDRESS), check_first, "{identity:?}");
            }
        });
    }

    /// The identity round-trips through the desktop's store: a re-save that
    /// dropped it would silently confirm the account.
    #[test]
    fn the_identity_survives_a_save_and_a_read() {
        storage::tests::with_temp_state("receive-identity-store", || {
            assert!(storage::save_account(&account(Identity::Unconfirmed)).is_ok());
            let read = storage::load_accounts().unwrap_or_default();
            assert_eq!(
                read.first().map(|a| a.identity),
                Some(Identity::Unconfirmed)
            );
            // A rename (any other save of the record) keeps it.
            let mut renamed = read[0].clone();
            "Renamed".clone_into(&mut renamed.name);
            assert!(storage::save_account(&renamed).is_ok());
            assert_eq!(
                storage::load_accounts()
                    .unwrap_or_default()
                    .first()
                    .map(|a| a.identity),
                Some(Identity::Unconfirmed)
            );
        });
    }

    /// Confirmed on chain → saved, and Receive is the normal one; refuted →
    /// saved, never received at; a chain that said nothing changes nothing.
    /// The walk is asked about the account's first key and its address.
    #[test]
    fn a_check_saves_only_what_the_chain_changed() {
        storage::tests::with_temp_state("receive-identity-check", || {
            let unconfirmed = account(Identity::Unconfirmed);
            let first_key = unconfirmed.keys[0].public_key_hex.clone();
            assert!(storage::save_account(&unconfirmed).is_ok());

            // Silent chains: still unconfirmed, nothing written.
            let before = storage::generation();
            let asked = std::cell::RefCell::new(None);
            let found = check_with(ADDRESS, |key, address| {
                *asked.borrow_mut() = Some((key.to_owned(), address.to_owned()));
                (Identity::Unconfirmed, VerifiedBy::None)
            });
            assert_eq!(found, Some(Identity::Unconfirmed));
            assert_eq!(
                storage::generation(),
                before,
                "nothing changed, nothing written"
            );
            assert_eq!(
                asked.take(),
                Some((first_key, ADDRESS.to_owned())),
                "the walk asks about the first key and the account's address"
            );

            // Refuted: saved, and the account still refuses deposits.
            let found = check_with(ADDRESS, |_, _| (Identity::Refuted, VerifiedBy::Gnosis));
            assert_eq!(found, Some(Identity::Refuted));
            assert!(standing_of(ADDRESS).is_some_and(|s| !s.receivable && !s.check_first));
            // …and is never asked again.
            let found = check_with(ADDRESS, |_, _| {
                unreachable!("a refuted account is not re-asked")
            });
            assert_eq!(found, Some(Identity::Refuted));

            // Confirmed: the normal Receive.
            assert!(storage::save_account(&account(Identity::Unconfirmed)).is_ok());
            let found = check_with(ADDRESS, |_, _| (Identity::Confirmed, VerifiedBy::Ethereum));
            assert_eq!(found, Some(Identity::Confirmed));
            assert!(standing_of(ADDRESS).is_some_and(|s| s.receivable));
            // A confirmed account is not asked.
            let found = check_with(ADDRESS, |_, _| {
                unreachable!("a confirmed account is not re-asked")
            });
            assert_eq!(found, Some(Identity::Confirmed));
        });
    }

    /// One check at a time: a second claim while one runs is refused.
    #[test]
    fn only_one_check_runs_at_once() {
        storage::tests::with_temp_state("receive-identity-once", || {
            assert!(try_begin());
            assert!(checking());
            assert!(!try_begin(), "a second check while one runs");
            // Nothing stored: the claimed run releases and answers nothing.
            assert_eq!(run_begun(ADDRESS), None);
            assert!(!checking());
        });
    }
}
