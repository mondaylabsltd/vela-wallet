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
//! Every session operation is a read or a write of one small local JSON file.
//! The web client does the same work against `localStorage` on its main thread;
//! moving it to a background task here would buy nothing measurable and would
//! introduce the one thing this module must not have — a window in which the
//! route is stale. The onboarding executor is the opposite case (USB, TLS, a
//! person's finger) and runs off-thread accordingly.

use gpui::{App, Global};

use vela_core::app::session::{Session, SessionView};
use vela_core::app::shell::CompletionMode;

use crate::core_host::CoreHost;
use crate::executor;

pub struct SessionState {
    host: CoreHost<Session>,
    view: SessionView,
}

impl Global for SessionState {}

impl SessionState {
    fn new() -> Self {
        let host = CoreHost::<Session>::new();
        let view = host.view();
        Self { host, view }
    }

    /// Drain the effect queue. Every operation answers immediately, so this
    /// runs to quiescence rather than leaving anything outstanding.
    fn pump(
        &mut self,
        mut pending: Vec<crate::core_host::Pending<vela_core::app::session::SessionOperation>>,
    ) {
        while let Some(next) = pending.pop() {
            let result = executor::perform_session(&next.operation);
            pending.extend(self.host.resolve(next.id, result));
        }
        self.view = self.host.view();
    }
}

/// Install the session and read storage. Called once, at startup.
pub fn boot(cx: &mut App) {
    let mut state = SessionState::new();
    let pending = state.host.dispatch(vela_core::app::session::Event::Boot);
    state.pump(pending);
    cx.set_global(state);
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
    let pending = state.host.dispatch(event);
    state.pump(pending);
    cx.set_global(state);
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
            }],
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
            state.pump(pending);
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
            state.pump(pending);
            assert_eq!(state.view.active_index, other);
            // The address is DERIVED from the active index (invariant ①), so
            // this is what every money surface will now read.
            assert_eq!(state.view.address, wanted);

            // Switching to the row already active changes nothing.
            let pending = state.host.dispatch(Event::SwitchAccount { index: other });
            state.pump(pending);
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
            state.pump(pending);
            assert_eq!(
                state.view.allowed_route,
                SessionRoute::Onboarding,
                "empty storage starts at Welcome"
            );

            let pending = state.host.dispatch(Event::AccountEstablished {
                mode: CompletionMode::AddAccount { account: account() },
            });
            state.pump(pending);
            assert_eq!(state.view.allowed_route, SessionRoute::Wallet);
            assert_eq!(state.view.address, account().address);

            // The confirmation is the core's, and it does not open until the
            // pending-upload question has an answer.
            let pending = state.host.dispatch(Event::SignOut);
            state.pump(pending);
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
            state.pump(pending);
            assert!(state.view.sign_out.is_none());
            assert_eq!(state.view.allowed_route, SessionRoute::Wallet);

            let pending = state.host.dispatch(Event::SignOut);
            state.pump(pending);
            let pending = state.host.dispatch(Event::SignOutConfirmed);
            state.pump(pending);
            assert_eq!(
                state.view.allowed_route,
                SessionRoute::Onboarding,
                "there has to be a way back"
            );
            assert!(!state.view.has_wallet);

            // And it really left the disk, so a relaunch agrees.
            let mut relaunched = SessionState::new();
            let pending = relaunched.host.dispatch(Event::Boot);
            relaunched.pump(pending);
            assert_eq!(relaunched.view.allowed_route, SessionRoute::Onboarding);
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
            state.pump(pending);
            let pending = state.host.dispatch(Event::AccountEstablished {
                mode: CompletionMode::AddAccount { account: account() },
            });
            state.pump(pending);

            crate::executor::storage::tests::write_pending_upload("cred0");

            let pending = state.host.dispatch(Event::SignOut);
            state.pump(pending);
            let dialog = state
                .view
                .sign_out
                .clone()
                .unwrap_or_else(|| unreachable!("the confirmation never opened"));
            assert!(dialog.pending_upload_warning);
        });
    }
}
