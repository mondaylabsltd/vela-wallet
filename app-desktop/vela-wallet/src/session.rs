//! The session machine, app-resident.
//!
//! One per process, outliving every screen — which on gpui means a `Global`
//! rather than an entity: the onboarding page hands a finished wallet to it and
//! then goes away, and the wallet page reads the active account from it without
//! knowing onboarding ever existed.
//!
//! `SessionView::allowed_route` is the route guard, and the split it encodes is
//! the point: **the core decides WHAT is allowed, the client decides WHEN to
//! navigate.** `main.rs` renders the route this reports and nothing else — it
//! never concludes "there is an account, so show the wallet", because during
//! the read there is no answer yet and `Loading` is how the core says so.
//!
//! ## Why this pump is synchronous
//!
//! Every session operation but one is a read or a write of one small local
//! JSON file. The web client does the same work against `localStorage` on its
//! main thread; moving it to a background task here would buy nothing
//! measurable and would introduce the one thing this module must not have — a
//! window in which the route is stale. The onboarding executor is the opposite
//! case (USB, TLS, a person's finger) and runs off-thread accordingly.
//!
//! The one is the landing watch's wait on a registry task (issue #409): a
//! network poll of up to two minutes, which no route depends on. The pump
//! hands it back instead of performing it, and [`run_off_thread`] performs it
//! on the background executor and answers the core when it is done.

use std::sync::atomic::{AtomicU64, Ordering};

use gpui::{App, Global};

use vela_core::app::session::{Session, SessionView};
use vela_core::app::shell::CompletionMode;

use crate::core_host::CoreHost;
use crate::executor;

pub struct SessionState {
    host: CoreHost<Session>,
    /// Which machine this is. [`reboot`] builds a fresh one whose effect ids
    /// start again at 1, so an answer performed off-thread for the OLD machine
    /// must never be handed to the new one under a colliding id.
    generation: u64,
    view: SessionView,
    /// A signed-in person asked for another account (spec 072): the route
    /// stays the wallet, and the window shows onboarding over it until the
    /// new account is established or they go back. A shell flag, not the
    /// core's: WHETHER a second account may be added is already the create
    /// and login machines' question — this is only where the window is.
    adding: Option<AddAccount>,
}

/// Which way into another account.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddAccount {
    /// "Create a new account" — straight into the create journey.
    Create,
    /// "Sign in to an existing account" — the sign-in methods.
    SignIn,
}

impl Global for SessionState {}

type SessionPending = crate::core_host::Pending<vela_core::app::session::SessionOperation>;

static GENERATION: AtomicU64 = AtomicU64::new(0);

impl SessionState {
    fn new() -> Self {
        let host = CoreHost::<Session>::new();
        let view = host.view();
        Self {
            host,
            view,
            generation: GENERATION.fetch_add(1, Ordering::Relaxed) + 1,
            adding: None,
        }
    }

    /// Drain the effect queue. Every local operation answers immediately, so
    /// this runs to quiescence; what would block — the landing watch's wait on
    /// a registry task — is returned for [`run_off_thread`] instead.
    #[must_use]
    fn pump(&mut self, mut pending: Vec<SessionPending>) -> Vec<SessionPending> {
        let mut blocking = Vec::new();
        while let Some(next) = pending.pop() {
            if executor::session_blocks(&next.operation) {
                blocking.push(next);
                continue;
            }
            let result = executor::perform_session(&next.operation);
            pending.extend(self.host.resolve(next.id, result));
        }
        self.view = self.host.view();
        blocking
    }
}

/// Perform the operations a pump handed back on the background executor, and
/// answer the machine that asked — if it is still the installed one.
fn run_off_thread(blocking: Vec<SessionPending>, generation: u64, cx: &mut App) {
    for next in blocking {
        cx.spawn(async move |cx| {
            let operation = next.operation;
            let result = cx
                .background_executor()
                .spawn(async move { executor::perform_session(&operation) })
                .await;
            cx.update(|cx| answer(generation, next.id, result, cx));
        })
        .detach();
    }
}

/// An off-thread answer, back on the main thread.
fn answer(
    generation: u64,
    id: u64,
    result: vela_core::app::session::SessionShellResult,
    cx: &mut App,
) {
    let current = cx
        .try_global::<SessionState>()
        .map(|state| state.generation);
    if current != Some(generation) {
        // The machine that asked was replaced (a reboot after an erase). Its
        // question went with it; the new machine runs its own watch.
        return;
    }
    let mut state = cx.remove_global::<SessionState>();
    let pending = state.host.resolve(id, result);
    let blocking = state.pump(pending);
    cx.set_global(state);
    run_off_thread(blocking, generation, cx);
}

/// Install the session and read storage. Called once, at startup.
pub fn boot(cx: &mut App) {
    let mut state = SessionState::new();
    let generation = state.generation;
    let pending = state.host.dispatch(vela_core::app::session::Event::Boot);
    let blocking = state.pump(pending);
    cx.set_global(state);
    run_off_thread(blocking, generation, cx);
}

/// The current view. Cheap to clone; screens read it per frame.
pub fn view(cx: &App) -> SessionView {
    cx.try_global::<SessionState>()
        .map_or_else(|| SessionState::new().view, |state| state.view.clone())
}

/// The onboarding hand-off. Both machines exit through `CompleteOnboarding`,
/// and this is what receives it.
///
/// `SwitchAccount` is still absent: the desktop wallet page has no account
/// switcher yet, and an event with no control is dead code. The operation
/// behind it is implemented in `executor::perform_session`, so adding the
/// switcher is a screen change and not a shell change.
pub fn account_established(mode: CompletionMode, cx: &mut App) {
    dispatch(
        vela_core::app::session::Event::AccountEstablished { mode },
        cx,
    );
    // Whatever way in it came from, the new account is the answer.
    set_adding(None, cx);
}

/// Open onboarding over a signed-in window, to add another account.
pub fn add_account(entry: AddAccount, cx: &mut App) {
    set_adding(Some(entry), cx);
}

/// Back to the wallet without one.
pub fn add_account_cancelled(cx: &mut App) {
    set_adding(None, cx);
}

/// Is another account being added, and which way.
pub fn adding_account(cx: &App) -> Option<AddAccount> {
    cx.try_global::<SessionState>()
        .and_then(|state| state.adding)
}

fn set_adding(adding: Option<AddAccount>, cx: &mut App) {
    if !cx.has_global::<SessionState>() {
        boot(cx);
    }
    // Written through `global_mut`, which notifies observers: the root is
    // one, and this flag is a navigation.
    cx.global_mut::<SessionState>().adding = adding;
}

/// Read storage again from nothing — after an erase (spec 072), when there is
/// no wallet left to be signed into and the next screen is the first run.
///
/// A fresh machine rather than an event: `Boot` is single-shot by the core's
/// own rule, and the machine that was booted remembers accounts that are no
/// longer on disk.
pub fn reboot(cx: &mut App) {
    boot(cx);
}

/// Open the sign-out confirmation. The core checks the pending-upload outbox
/// before the dialog appears, so the warning is decided rather than guessed.
/// The switcher picked a row.
///
/// `index` is a position in the ORIGINAL account list, not in whatever order a
/// screen drew — the core's invariant ⑦, and the reason `SessionAccountRow`
/// carries its own index rather than leaving the shell to count rows.
pub fn switch_account(index: usize, cx: &mut App) {
    dispatch(vela_core::app::session::Event::SwitchAccount { index }, cx);
}

/// Drop ONE wallet from this device and stay on the others (2026-09-23).
///
/// `index` is a position in the ORIGINAL list, as [`switch_account`] takes.
/// Removing the last one signs this device out, which the core decides.
pub fn remove_account(index: usize, cx: &mut App) {
    dispatch(vela_core::app::session::Event::RemoveAccount { index }, cx);
}

/// Spec 102 (D1): where the account at `address` reviews and signs on this
/// device. The core refuses — and writes nothing — when the venue cannot
/// reach the account's keys (R1); the screen never offers one that cannot.
pub fn choose_venue(address: &str, venue: vela_core::signing_venue::SigningVenue, cx: &mut App) {
    dispatch(
        vela_core::app::session::Event::SigningVenueChosen {
            address: address.to_owned(),
            venue,
        },
        cx,
    );
}

pub fn sign_out(cx: &mut App) {
    dispatch(vela_core::app::session::Event::SignOut, cx);
}

pub fn sign_out_confirmed(cx: &mut App) {
    dispatch(vela_core::app::session::Event::SignOutConfirmed, cx);
}

pub fn sign_out_dismissed(cx: &mut App) {
    dispatch(vela_core::app::session::Event::SignOutDismissed, cx);
}

fn dispatch(event: vela_core::app::session::Event, cx: &mut App) {
    if !cx.has_global::<SessionState>() {
        boot(cx);
    }
    let mut state = cx.remove_global::<SessionState>();
    let generation = state.generation;
    let pending = state.host.dispatch(event);
    let blocking = state.pump(pending);
    cx.set_global(state);
    run_off_thread(blocking, generation, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::session::{Event, SessionRoute};
    use vela_core::app::{Account, AccountKey};

    fn account() -> Account {
        Account {
            id: "cred0".to_owned(),
            name: "Everyday wallet".to_owned(),
            address: "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33".to_owned(),
            public_key_hex: "04aa".to_owned(),
            created_at_iso: "2026-08-25T00:00:00.000Z".to_owned(),
            keys: vec![AccountKey {
                credential_id: "cred0".to_owned(),
                public_key_hex: "04aa".to_owned(),
                name: "Everyday wallet".to_owned(),
                transports: "usb".to_owned(),
                signer_origin: None,
            }],
            sign_in_key: None,
            signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
            signing_venue: vela_core::signing_venue::SigningVenue::InVela,
        }
    }

    /// Two accounts, and switching between them.
    ///
    /// `SwitchAccount` has existed since 019 with nothing to trigger it —
    /// "an event with no control is dead code", as this file said about this
    /// very event. The control is the settings account row, and this is the
    /// property it depends on: the row's index is a position in the ORIGINAL
    /// list (invariant ⑦), so a display reorder cannot switch to the wrong
    /// wallet.
    #[test]
    fn switching_accounts_moves_the_active_address() {
        crate::executor::storage::tests::with_temp_state("session-switch", || {
            let first = account();
            let mut second = account();
            second.id = "cred1".to_owned();
            second.name = "Spending".to_owned();
            second.address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned();

            if crate::executor::storage::save_account(&first).is_err()
                || crate::executor::storage::save_account(&second).is_err()
            {
                unreachable!("could not seed");
            }

            let mut state = SessionState::new();
            let pending = state.host.dispatch(Event::Boot);
            let _ = state.pump(pending);
            assert_eq!(state.view.accounts.len(), 2, "both wallets are listed");

            let active = state.view.active_index;
            let other = state
                .view
                .accounts
                .iter()
                .map(|row| row.index)
                .find(|index| *index != active)
                .unwrap_or_else(|| unreachable!("two accounts, two indices"));
            let wanted = state.view.accounts[other].account.address.clone();

            let pending = state.host.dispatch(Event::SwitchAccount { index: other });
            let _ = state.pump(pending);
            assert_eq!(state.view.active_index, other);
            // The address is DERIVED from the active index (invariant ①), so
            // this is what every money surface will now read.
            assert_eq!(state.view.address, wanted);

            // Switching to the row already active changes nothing.
            let pending = state.host.dispatch(Event::SwitchAccount { index: other });
            let _ = state.pump(pending);
            assert_eq!(state.view.address, wanted);
        });
    }

    /// Sign in, then sign out, and land back on Welcome.
    ///
    /// This is the loop a person actually walks, and it was a ONE-WAY DOOR
    /// until the wallet page grew a sign-out row: `allowed_route` sends a
    /// signed-in desktop to the wallet and there was no control anywhere that
    /// could send it back. Wiring a route guard without wiring its exit is the
    /// specific mistake this test exists to catch — an `allowed_route` that
    /// never returns to `Onboarding` is a wallet nobody can leave.
    #[test]
    fn a_wallet_can_be_signed_out_of_and_the_route_goes_back() {
        crate::executor::storage::tests::with_temp_state("session-round-trip", || {
            let mut state = SessionState::new();
            let pending = state.host.dispatch(Event::Boot);
            let _ = state.pump(pending);
            assert_eq!(
                state.view.allowed_route,
                SessionRoute::Onboarding,
                "empty storage starts at Welcome"
            );

            let pending = state.host.dispatch(Event::AccountEstablished {
                mode: CompletionMode::AddAccount { account: account() },
            });
            let _ = state.pump(pending);
            assert_eq!(state.view.allowed_route, SessionRoute::Wallet);
            assert_eq!(state.view.address, account().address);

            // The confirmation is the core's, and it does not open until the
            // pending-upload question has an answer.
            let pending = state.host.dispatch(Event::SignOut);
            let _ = state.pump(pending);
            let dialog = state
                .view
                .sign_out
                .clone()
                .unwrap_or_else(|| unreachable!("the confirmation never opened"));
            assert!(
                !dialog.pending_upload_warning,
                "nothing is outstanding in this fixture"
            );
            assert_eq!(
                state.view.allowed_route,
                SessionRoute::Wallet,
                "opening the dialog must not navigate"
            );

            // Cancelling leaves the wallet exactly where it was.
            let pending = state.host.dispatch(Event::SignOutDismissed);
            let _ = state.pump(pending);
            assert!(state.view.sign_out.is_none());
            assert_eq!(state.view.allowed_route, SessionRoute::Wallet);

            let pending = state.host.dispatch(Event::SignOut);
            let _ = state.pump(pending);
            let pending = state.host.dispatch(Event::SignOutConfirmed);
            let _ = state.pump(pending);
            assert_eq!(
                state.view.allowed_route,
                SessionRoute::Onboarding,
                "there has to be a way back"
            );
            assert!(!state.view.has_wallet);

            // And it really left the disk, so a relaunch agrees.
            let mut relaunched = SessionState::new();
            let pending = relaunched.host.dispatch(Event::Boot);
            let _ = relaunched.pump(pending);
            assert_eq!(relaunched.view.allowed_route, SessionRoute::Onboarding);
        });
    }

    fn accepted_record(task_id: &str) -> vela_core::app::PendingUpload {
        vela_core::app::PendingUpload {
            id: "cred0".to_owned(),
            name: "Everyday wallet".to_owned(),
            public_key_hex: "04aa".to_owned(),
            attestation_object_hex: "a0".to_owned(),
            created_at_iso: "2026-10-04T00:00:00.000Z".to_owned(),
            authenticator_attachment: String::new(),
            transports: String::new(),
            members: Vec::new(),
            task_id: Some(task_id.to_owned()),
        }
    }

    fn outbox_len() -> usize {
        match crate::executor::storage::load_pending_uploads() {
            Ok(records) => records.len(),
            Err(_) => unreachable!("the outbox reads"),
        }
    }

    /// Issue #409: a one-key wallet entered at the registry's 202 leaves its
    /// record — with the task — for the session to confirm. The launch hands
    /// the wait on that task BACK rather than polling on the main thread; once
    /// it lands, the record goes, with no ceremony anywhere.
    #[test]
    fn a_landing_is_confirmed_off_the_main_thread_and_then_the_record_goes() {
        crate::executor::storage::tests::with_temp_state("session-landing-409", || {
            if crate::executor::storage::save_account(&account()).is_err()
                || crate::executor::storage::save_pending_upload(&accepted_record("t1")).is_err()
            {
                unreachable!("could not seed");
            }
            let mut state = SessionState::new();
            let pending = state.host.dispatch(Event::Boot);
            let blocking = state.pump(pending);
            assert_eq!(state.view.allowed_route, SessionRoute::Wallet);
            let [wait] = blocking.as_slice() else {
                unreachable!(
                    "exactly the landing wait is handed back: {}",
                    blocking.len()
                )
            };
            assert_eq!(
                wait.operation,
                vela_core::app::session::SessionOperation::AwaitRegistryLanding {
                    task_id: "t1".to_owned()
                }
            );
            assert_eq!(outbox_len(), 1, "nothing is removed before the answer");

            // What `run_off_thread` delivers once the registry says `done`.
            let pending = state.host.resolve(
                wait.id,
                vela_core::app::session::SessionShellResult::RegistryLanded,
            );
            assert!(state.pump(pending).is_empty());
            assert_eq!(outbox_len(), 0, "confirmed, so the record is gone");
        });
    }

    /// The landing failed: the record stays, and so does the sign-out warning.
    #[test]
    fn an_unconfirmed_landing_keeps_the_record_and_the_warning() {
        crate::executor::storage::tests::with_temp_state("session-landing-409-failed", || {
            if crate::executor::storage::save_account(&account()).is_err()
                || crate::executor::storage::save_pending_upload(&accepted_record("t1")).is_err()
            {
                unreachable!("could not seed");
            }
            let mut state = SessionState::new();
            let pending = state.host.dispatch(Event::Boot);
            let blocking = state.pump(pending);
            let [wait] = blocking.as_slice() else {
                unreachable!("the landing wait is handed back")
            };
            let pending = state.host.resolve(
                wait.id,
                vela_core::app::session::SessionShellResult::RegistryLandingUnconfirmed {
                    message: "Register failed: reverted".to_owned(),
                },
            );
            assert!(state.pump(pending).is_empty());
            assert_eq!(outbox_len(), 1);

            let pending = state.host.dispatch(Event::SignOut);
            let _ = state.pump(pending);
            let dialog = state
                .view
                .sign_out
                .clone()
                .unwrap_or_else(|| unreachable!("the confirmation never opened"));
            assert!(dialog.pending_upload_warning);
        });
    }

    /// An un-synced public key must make the dialog say so. A record in that
    /// state is a key the registry never confirmed, and signing out before it
    /// lands can leave the wallet unreachable from anywhere else.
    #[test]
    fn an_unconfirmed_public_key_warns_before_sign_out() {
        crate::executor::storage::tests::with_temp_state("session-pending", || {
            let mut state = SessionState::new();
            let pending = state.host.dispatch(Event::Boot);
            let _ = state.pump(pending);
            let pending = state.host.dispatch(Event::AccountEstablished {
                mode: CompletionMode::AddAccount { account: account() },
            });
            let _ = state.pump(pending);

            crate::executor::storage::tests::write_pending_upload("cred0");

            let pending = state.host.dispatch(Event::SignOut);
            let _ = state.pump(pending);
            let dialog = state
                .view
                .sign_out
                .clone()
                .unwrap_or_else(|| unreachable!("the confirmation never opened"));
            assert!(dialog.pending_upload_warning);
        });
    }
}
