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

/// The multisig links (`vela_core::multisig_link`): `method` plus its
/// arguments as JSON, the answer as JSON — the wasm `multisigLinkCall`.
///
/// The same door answers the multisig's pure rules a screen asks one at a
/// time (`vela_core::multisig_rules`), by `<area>.<method>`: `relay.*`
/// (the PeerConnect token schedule), `names.*` (a member's label and kind),
/// `networks.*` (the order networks are offered in), `calls.*` (which calls a
/// review reads). docs/multisig-machines.md lists them.
#[uniffi::export]
pub fn multisig_link_call(method: String, args_json: String) -> Result<String, crate::CoreError> {
    Ok(vela_core::multisig_link::call_json(&method, &args_json)?)
}

/// The multisig's kernel (`vela_core::multisig::call_json`): `method` plus
/// its arguments as JSON, the answer as JSON — the wasm `multisigCall`
/// (`validName`, `validOwner`, `genesis`, `safeTxHash`, …), so a native
/// create sheet asks the same rules the web's does.
#[uniffi::export]
pub fn multisig_call(method: String, args_json: String) -> Result<String, crate::CoreError> {
    Ok(vela_core::multisig::call_json(&method, &args_json)?)
}

/// What an entered address reads as — `"valid"`, `"malformed"` or
/// `"checksum"` (mixed case that is not its EIP-55 checksum): the one rule
/// every field that takes an address refuses by (send-2), the wasm
/// `addressEntry`. A multisig's member field reads it on every platform.
#[uniffi::export]
#[must_use]
pub fn address_entry(address: String) -> String {
    use vela_core::primitives::AddressEntry;
    match vela_core::primitives::address_entry(&address) {
        AddressEntry::Valid => "valid",
        AddressEntry::Malformed => "malformed",
        AddressEntry::BadChecksum => "checksum",
    }
    .to_owned()
}

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

            /// Opt in to lighter answers, as every other machine's object
            /// can (`onboarding_bridge.rs`): from the next answer on, each
            /// carries `view_version`, and `view` only when it differs from
            /// the last view this object sent (no `view`: keep the one you
            /// hold). `include_volatile`: also send the machine's volatile
            /// fields apart, in `volatile`, when they move.
            pub fn skip_unchanged_views(
                &self,
                include_volatile: bool,
            ) -> Result<(), crate::CoreError> {
                crate::onboarding_bridge::locked(&self.0)?.skip_unchanged_views(include_volatile);
                Ok(())
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
        assert!(core
            .resolve_effect(id, r#"{"type":"store_written"}"#.to_owned())
            .is_ok());
    }

    #[test]
    fn the_proposals_core_follows_nothing_until_told() {
        let core = MultisigProposalsCore::new();
        let view: serde_json::Value = serde_json::from_str(&core.view().unwrap()).unwrap();
        assert!(view["followed"].is_null());
        assert_eq!(view["pace_ms"], 12_000);
        // W9: the machine's own words for the multisig it follows.
        assert_eq!(view["members_anywhere"], serde_json::json!([]));
    }

    /// W9: the natives ask the multisig's pure rules through the link door,
    /// as the web does (`multisigLinkCall`).
    #[test]
    fn the_rules_answer_through_the_link_door() {
        let none = multisig_link_call("calls.readable".to_owned(), "{}".to_owned()).unwrap();
        assert_eq!(none, "null");
        assert!(multisig_link_call("nobody.knows".to_owned(), "{}".to_owned()).is_err());
    }

    /// A shell that opted in hears `view_version` on every answer and the
    /// view only when it moved; one that did not keeps the whole view.
    #[test]
    fn a_shell_may_skip_unchanged_views() {
        let online = r#"{"type":"online","online":true}"#;
        let classic = MultisigProposalsCore::new();
        for _ in 0..2 {
            let out: serde_json::Value =
                serde_json::from_str(&classic.dispatch(online.to_owned()).unwrap()).unwrap();
            assert!(out["view"].is_object());
            assert!(out.get("view_version").is_none());
        }

        let light = MultisigProposalsCore::new();
        light.skip_unchanged_views(false).unwrap();
        let first: serde_json::Value =
            serde_json::from_str(&light.dispatch(online.to_owned()).unwrap()).unwrap();
        assert!(first["view_version"].is_u64());
        assert!(
            first["view"].is_object(),
            "the first answer carries the view"
        );
        let again: serde_json::Value =
            serde_json::from_str(&light.dispatch(online.to_owned()).unwrap()).unwrap();
        assert_eq!(again["view_version"], first["view_version"]);
        assert!(
            again.get("view").is_none(),
            "an unchanged view is not sent again"
        );
        // The view is still there to ask for.
        assert!(light.view().unwrap().starts_with('{'));
    }

    /// The kernel door answers as the wasm's `multisigCall` does.
    #[test]
    fn the_kernel_answers_through_its_door() {
        assert_eq!(
            multisig_call("validName".to_owned(), r#"{"name":"Treasury"}"#.to_owned()).unwrap(),
            "true"
        );
        assert_eq!(
            multisig_call("validName".to_owned(), r#"{"name":" padded"}"#.to_owned()).unwrap(),
            "false"
        );
        assert_eq!(
            multisig_call(
                "validOwner".to_owned(),
                r#"{"address":"0x0000000000000000000000000000000000000001"}"#.to_owned()
            )
            .unwrap(),
            "false"
        );
        assert!(multisig_call("nobody.knows".to_owned(), "{}".to_owned()).is_err());
    }

    /// The address rule, as the web's `addressEntry` says it.
    #[test]
    fn an_entered_address_reads_as_the_web_reads_it() {
        assert_eq!(
            address_entry("0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69".to_owned()),
            "valid"
        );
        assert_eq!(
            address_entry("0x6813eb9362372eef6200f3b1dbc3f819671cba69".to_owned()),
            "valid"
        );
        assert_eq!(
            address_entry("0x6813Eb9362372EEF6200f3b1dbC3f819671cBA6A".to_owned()),
            "checksum"
        );
        assert_eq!(address_entry("0x1234".to_owned()), "malformed");
    }
}
