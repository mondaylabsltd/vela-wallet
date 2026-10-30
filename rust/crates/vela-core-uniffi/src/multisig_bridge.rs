//! The multisig machines over UniFFI (F0.4): the same JSON surface the web
//! gets from `vela-core-wasm` (`MultisigAccountsCore`, …), so desktop,
//! Android and iOS drive the one copy of the multisig rules the core holds.
//!
//! ```text
//! dispatch(event_json) ─► { view, effects: [{ id, operation }], cancelled_effect_ids }
//! resolve_effect(id, result_json) ─► same shape
//! view() ─► the current view model
//! ```
//!
//! Every effect's `operation` is a `MultisigOperation` (HTTP, JSON-RPC on a
//! chain, a store key, a wait, the clock, random bytes, a passkey, a question
//! to the person, calls an account sends, a PeerConnect room, the injected
//! wallet, a notice to route): ONE executor per platform serves all of them.
//! The semantics are `onboarding_bridge.rs`' (monotonic effect ids, a stale
//! answer changes nothing, a `Mutex` per core held only for one dispatch);
//! this file only names the machines.

/// One machine as a uniffi object over [`crate::onboarding_bridge::Bridge`].
macro_rules! multisig_object {
    ($(#[$doc:meta])* $class:ident, $app:ty) => {
        $(#[$doc])*
        #[derive(uniffi::Object)]
        pub struct $class(std::sync::Mutex<crate::onboarding_bridge::Bridge<$app>>);

        #[uniffi::export]
        impl $class {
            #[uniffi::constructor]
            pub fn new() -> Self {
                Self(std::sync::Mutex::new(crate::onboarding_bridge::Bridge::new()))
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

multisig_object!(
    /// The multisigs on this device: create, add, import, records, recheck.
    MultisigAccountsCore,
    vela_core::app::multisig_accounts::MultisigAccounts
);

multisig_object!(
    /// A multisig's transactions in flight: read, verify, propose, approve,
    /// the signing gate, the outbox, execute and follow.
    MultisigProposalsCore,
    vela_core::app::multisig_proposals::MultisigProposals
);

multisig_object!(
    /// Who the members are on each network, what a change would be, and
    /// the waiting changes with their per-network progress.
    MultisigMembersCore,
    vela_core::app::multisig_members::MultisigMembers
);

multisig_object!(
    /// The connect page acting for an outside owner (§11.2, §11.3).
    MultisigOutsidePageCore,
    vela_core::app::multisig_outside::page::OutsidePage
);

multisig_object!(
    /// Vela's side of a live session with an outside owner's wallet.
    MultisigOutsideSessionCore,
    vela_core::app::multisig_outside::session::OutsideSession
);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// The JSON surface is the wasm one: a start asks the shell for its store.
    #[test]
    fn a_start_asks_for_the_store() {
        let core = MultisigAccountsCore::new();
        let out: serde_json::Value = serde_json::from_str(
            &core
                .dispatch(
                    r#"{"type":"start","service_url":"http://s","accounts":[],"networks":[100]}"#
                        .to_owned(),
                )
                .unwrap(),
        )
        .unwrap();
        let ops: Vec<&str> = out["effects"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["operation"]["type"].as_str().unwrap())
            .collect();
        assert_eq!(ops, vec!["load_store"]);
        assert_eq!(out["view"]["loaded"], false);
        let id = out["effects"][0]["id"].as_u64().unwrap();
        let out: serde_json::Value = serde_json::from_str(
            &core
                .resolve_effect(id, r#"{"type":"store_loaded","entries":{}}"#.to_owned())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(out["view"]["loaded"], true);
        // An answer that outlived its question changes nothing.
        assert!(core.resolve_effect(id, r#"{"type":"store_written"}"#.to_owned()).is_ok());
    }

    #[test]
    fn the_proposals_core_follows_nothing_until_told() {
        let core = MultisigProposalsCore::new();
        let view: serde_json::Value = serde_json::from_str(&core.view().unwrap()).unwrap();
        assert!(view["followed"].is_null());
        assert_eq!(view["pace_ms"], 12_000);
    }
}
