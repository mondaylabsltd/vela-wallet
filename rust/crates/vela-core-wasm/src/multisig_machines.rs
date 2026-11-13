//! wasm classes for the multisig machines (F0.4): the shared
//! [`crate::bridge::Bridge`] over each — JSON in, JSON out, the one
//! `MultisigOperation` vocabulary out to the shell. The web keeps its
//! TypeScript orchestration for now; these exist so the web can move onto
//! the same machines the native shells drive (`vela-core-uniffi`'s
//! `multisig_bridge.rs`), one copy of the money rules.

use crate::bridge::bridge_class;
use wasm_bindgen::prelude::wasm_bindgen;

/// **Multisig links** (`vela_core::multisig_link`): the proposal link
/// (`#ms=`), the invite (`#msa=`, networks, member names and kinds, the
/// checksum refusal), the connect page's link (`ms=`, `for=`, damaged),
/// row ids and units — `method` plus its arguments as JSON, as `multisigCall`.
#[wasm_bindgen(js_name = multisigLinkCall)]
pub fn multisig_link_call(method: &str, args_json: &str) -> Result<String, wasm_bindgen::JsValue> {
    vela_core::multisig_link::call_json(method, args_json).map_err(crate::err)
}

bridge_class!(
    /// The multisigs on this device: create, add, import, records, recheck.
    MultisigAccountsCore,
    vela_core::app::multisig_accounts::MultisigAccounts
);

bridge_class!(
    /// A multisig's transactions in flight: read, verify, propose, approve,
    /// the signing gate, the outbox, execute and follow.
    MultisigProposalsCore,
    vela_core::app::multisig_proposals::MultisigProposals
);

bridge_class!(
    /// Who the members are on each network, what a change would be, and
    /// the waiting changes with their per-network progress.
    MultisigMembersCore,
    vela_core::app::multisig_members::MultisigMembers
);

bridge_class!(
    /// The connect page acting for an outside owner (§11.2, §11.3): the
    /// approve-later link, verification, the gate, sign / approveHash /
    /// execute / reject through the injected wallet, its outbox.
    MultisigOutsidePageCore,
    vela_core::app::multisig_outside::page::OutsidePage
);

bridge_class!(
    /// Vela's side of a live session with an outside owner's wallet.
    MultisigOutsideSessionCore,
    vela_core::app::multisig_outside::session::OutsideSession
);
