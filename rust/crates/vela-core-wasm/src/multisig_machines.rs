//! wasm classes for the multisig machines (F0.4): the shared
//! [`crate::bridge::Bridge`] over each — JSON in, JSON out, the one
//! `MultisigOperation` vocabulary out to the shell. The web keeps its
//! TypeScript orchestration for now; these exist so the web can move onto
//! the same machines the native shells drive (`vela-core-uniffi`'s
//! `multisig_bridge.rs`), one copy of the money rules.

use crate::bridge::bridge_class;

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
