//! The uniffi half of the Crux bridge: the same JSON surface the web gets from
//! `vela-core-wasm`, exported to Swift and Kotlin instead of to JavaScript.
//!
//! ```text
//! dispatch(event_json) ─► { view, effects: [{ id, operation }], cancelled_effect_ids }
//! resolve_effect(id, result_json) ─► same shape
//! view() ─► the current view model
//! ```
//!
//! **The semantics are not re-derived here.** They are the ones
//! `rust/crates/vela-core-wasm/src/bridge.rs` has carried since spec 011, and a
//! divergence between the two would mean the four clients are running four
//! different machines:
//!
//! 1. Effect ids are monotonic, per core instance.
//! 2. An id this bridge does not know means **the answer outlived the
//!    question** — the shell resolved an operation that was already abandoned.
//!    Expected, not a fault: report the current view and change nothing.
//! 3. A `resolve` error means the command that owned the request was aborted
//!    before the answer arrived. Same story, one layer down.
//!
//! What differs from wasm is only what the language forces:
//!
//! - **A `Mutex` per core.** A `#[uniffi::export]`ed object is `Send + Sync` and
//!   its methods take `&self`, while `Core::process_event` needs `&mut`. wasm
//!   is single-threaded and needs neither. The lock is held for the duration of
//!   one dispatch and never across an effect — the shell performs effects
//!   outside it, which is what keeps a passkey ceremony from blocking the view.
//! - **`CoreError` instead of `JsValue`.** Same three failure sites, same
//!   messages.
//!
//! A poisoned lock is reported rather than recovered: it means a previous
//! dispatch panicked mid-mutation, and continuing over a half-updated model
//! would turn one bug into a wrong wallet.

use std::collections::HashMap;
use std::sync::Mutex;

use crux_core::capability::Operation;
use crux_core::{App, Core, Request};
use serde::de::DeserializeOwned;
use serde::Serialize;

use vela_core::app::SplitEffect;

use crate::CoreError;

#[derive(Serialize)]
struct DispatchResult<Op> {
    view: serde_json::Value,
    effects: Vec<ShellEffect<Op>>,
    /// Always empty today, and carried anyway: every machine keeps at most one
    /// in-flight operation per pipeline and drops stale answers by attempt, so
    /// the shell is never asked to abort. The shared effect loop on each client
    /// still reads the field, and a machine that one day does cancel must not
    /// require a wire change on four clients to say so.
    cancelled_effect_ids: Vec<u64>,
}

#[derive(Serialize)]
struct ShellEffect<Op> {
    id: u64,
    operation: Op,
}

/// The generic half. uniffi cannot export a generic object, so each exported
/// class is a thin [`bridge_object!`]-generated wrapper over this.
pub(crate) struct Bridge<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
{
    core: Core<A>,
    pending: HashMap<u64, Request<<A::Effect as SplitEffect>::Op>>,
    next_effect_id: u64,
}

impl<A> Bridge<A>
where
    A: App + Default,
    A::Model: Default,
    A::Event: DeserializeOwned,
    A::ViewModel: Serialize,
    A::Effect: SplitEffect,
    <A::Effect as SplitEffect>::Op: Clone + Serialize,
    <<A::Effect as SplitEffect>::Op as Operation>::Output: DeserializeOwned,
{
    pub(crate) fn new() -> Self {
        Self {
            core: Core::new(),
            pending: HashMap::new(),
            next_effect_id: 0,
        }
    }

    pub(crate) fn dispatch(&mut self, event_json: &str) -> Result<String, CoreError> {
        let event: A::Event = serde_json::from_str(event_json)
            .map_err(|error| CoreError::Internal(format!("invalid event from shell: {error}")))?;
        let effects = self.core.process_event(event);
        self.serialize(effects)
    }

    pub(crate) fn resolve_effect(
        &mut self,
        effect_id: u64,
        result_json: &str,
    ) -> Result<String, CoreError> {
        let result: <<A::Effect as SplitEffect>::Op as Operation>::Output =
            serde_json::from_str(result_json).map_err(|error| {
                CoreError::Internal(format!("invalid result from shell: {error}"))
            })?;

        // Rule 2.
        let Some(mut request) = self.pending.remove(&effect_id) else {
            return self.serialize(Vec::new());
        };

        match self.core.resolve(&mut request, result) {
            Ok(effects) => self.serialize(effects),
            // Rule 3.
            Err(_) => self.serialize(Vec::new()),
        }
    }

    pub(crate) fn view(&self) -> Result<String, CoreError> {
        serde_json::to_string(&self.core.view())
            .map_err(|error| CoreError::Internal(format!("could not serialize view: {error}")))
    }

    fn serialize(&mut self, effects: Vec<A::Effect>) -> Result<String, CoreError> {
        let mut shell_effects = Vec::new();
        for effect in effects {
            // `Effect::Render` falls out here as `None`. It must still be split
            // off, or it would be queued as an operation no executor can
            // perform; the shell re-renders from `view` on every result.
            if let Some(request) = effect.into_shell() {
                self.next_effect_id += 1;
                let id = self.next_effect_id;
                let operation = request.operation.clone();
                self.pending.insert(id, request);
                shell_effects.push(ShellEffect { id, operation });
            }
        }

        let view = serde_json::to_value(self.core.view())
            .map_err(|error| CoreError::Internal(format!("could not serialize view: {error}")))?;
        serde_json::to_string(&DispatchResult {
            view,
            effects: shell_effects,
            cancelled_effect_ids: Vec::new(),
        })
        .map_err(|error| CoreError::Internal(format!("could not serialize result: {error}")))
    }
}

/// Export one Crux machine as a uniffi object with the canonical
/// `dispatch` / `resolveEffect` / `view` surface.
macro_rules! bridge_object {
    ($(#[$doc:meta])* $class:ident, $app:ty) => {
        $(#[$doc])*
        #[derive(uniffi::Object)]
        pub struct $class(std::sync::Mutex<crate::onboarding_bridge::Bridge<$app>>);

        #[uniffi::export]
        impl $class {
            #[uniffi::constructor]
            pub fn new() -> Self {
                Self(std::sync::Mutex::new(
                    crate::onboarding_bridge::Bridge::new(),
                ))
            }

            pub fn dispatch(&self, event_json: String) -> Result<String, crate::CoreError> {
                crate::onboarding_bridge::locked(&self.0)?.dispatch(&event_json)
            }

            pub fn resolve_effect(
                &self,
                effect_id: u64,
                result_json: String,
            ) -> Result<String, crate::CoreError> {
                crate::onboarding_bridge::locked(&self.0)?.resolve_effect(effect_id, &result_json)
            }

            pub fn view(&self) -> Result<String, crate::CoreError> {
                crate::onboarding_bridge::locked(&self.0)?.view()
            }
        }

        impl Default for $class {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

/// Take the lock, refusing to work over state a panic left half-written.
pub(crate) fn locked<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>, CoreError> {
    mutex.lock().map_err(|_| {
        CoreError::Internal(
            "core lock poisoned: a previous dispatch panicked and the model is not trustworthy"
                .to_string(),
        )
    })
}

bridge_object!(
    /// Creating a wallet: register → prove signing → derive → sync → save.
    CreateWalletCore,
    vela_core::app::create_wallet::CreateWallet
);

bridge_object!(
    /// Signing in with an existing passkey, including on-device recovery.
    LoginCore,
    vela_core::app::login::Login
);

bridge_object!(
    /// Which account is current, and which route the shell is allowed to show.
    SessionCore,
    vela_core::app::session::Session
);

// The wallet-state machines, in the order the shells wire them. Each line is
// a machine the native clients can drive; the same set has been exported to
// the web since spec 016 (`vela-core-wasm/src/wallet_state.rs`), and the names
// are deliberately identical so a reader can follow one machine across four
// clients without a translation table.
//
// The cost is measured, not assumed (spec 040 research D6): a machine is
// ~357 KB stripped on arm64-v8a and the uniffi object around it is 5,920
// bytes, so the price of this list is the rules themselves. Add machines as
// their shell arrives — an exported machine nothing drives is dead weight in
// three ABIs.

bridge_object!(
    /// The address book: manual + history-derived merge, tombstones, groups.
    ContactsCore,
    vela_core::app::contacts::Contacts
);

bridge_object!(
    /// Network & endpoint configuration: add-network wizard, overrides,
    /// service endpoints, provider keys.
    NetworkAdminCore,
    vela_core::app::network_admin::NetworkAdmin
);

bridge_object!(
    /// The display currency: atomic code+rate pair, first-launch region seed,
    /// user-choice-wins.
    DisplayCurrencyCore,
    vela_core::app::display_currency::DisplayCurrency
);

// The read path (spec 041). `RpcPool` is the base every other one reads
// through — endpoint scoring, bans, cooldowns and the fastest-endpoint race
// are its decisions, and the shell contributes only fetch, clock and jitter.

bridge_object!(
    /// RPC/bundler endpoint pool decisions: scoring, cooldowns, bans.
    RpcPoolCore,
    vela_core::app::rpc_pool::RpcPool
);

bridge_object!(
    /// Balance aggregation & display policy (per active account).
    BalanceDashboardCore,
    vela_core::app::balance_dashboard::BalanceDashboard
);

bridge_object!(
    /// The activity feed: dedupe, batch folding, tombstones, celebrations.
    ActivityFeedCore,
    vela_core::app::activity_feed::ActivityFeed
);

bridge_object!(
    /// Manual custom-token management.
    ManageTokensCore,
    vela_core::app::manage_tokens::ManageTokens
);

bridge_object!(
    /// The token trust model: transfer allowlists, auto-add admission,
    /// asymmetric simulation trust.
    TokenTrustCore,
    vela_core::app::token_trust::TokenTrust
);

bridge_object!(
    /// Deposit detection on the Receive screen: phased polling, baseline
    /// diff, false-positive guards.
    ReceiveWatchCore,
    vela_core::app::receive_watch::ReceiveWatch
);

bridge_object!(
    /// Payment requests: the acknowledge gate, the EIP-681/pay-link builder,
    /// and the strict `/pay` validator.
    PaymentRequestCore,
    vela_core::app::payment_request::PaymentRequest
);
