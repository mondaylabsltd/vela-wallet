//! A dApp's request to a multisig, from its arrival to its answer — the rules
//! of the web's `core/dapp-request.ts` and the multisig part of
//! `DappRequestHost.svelte` (docs/multisig-vault-design.md §6), as the
//! proposals machine holds them ([`super::multisig_proposals::Event::DappRequest`]).
//!
//! The order is the whole point: the person sees what the site asks — the
//! core's reading of the transaction, at the number it would take — and only
//! "Propose and approve" signs anything. Every way out answers the site,
//! exactly once: declined before it was proposed is the person's `4001`;
//! left after it was proposed says so (`-32000`, carrying the safeTxHash),
//! and the proposal stays in the mailbox for the members.
//!
//! An `eth_sendTransaction` is answered with the transaction that executed
//! it — never with the safeTxHash, which is no transaction hash — and is
//! HELD until then, but never past the request's limit: [`ANSWER_MARGIN_MS`]
//! before it, the site is told it was proposed ([`left_proposed`]). A
//! `wallet_sendCalls` is ONE proposal of every call, its site answered the
//! batch's id (the safeTxHash) once it is proposed; `wallet_getCallsStatus`
//! follows it from the proposal's state
//! ([`super::dapp_rpc::multisig_calls_status`]).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// A JSON-RPC error a site is answered with.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigDappError"))]
pub struct DappError {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub data: Option<Value>,
}

/// What a site is answered: a result, or an error — exactly one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigDappAnswer"))]
pub struct DappAnswer {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<DappError>,
}

impl DappAnswer {
    pub fn ok(result: Value) -> Self {
        Self {
            result: Some(result),
            error: None,
        }
    }

    pub fn err(error: DappError) -> Self {
        Self {
            result: None,
            error: Some(error),
        }
    }
}

/// The person declined it before anything was proposed.
pub const DECLINED_CODE: i64 = 4001;
/// The JSON-RPC server-error class: the request was taken, its outcome is elsewhere.
pub const PROPOSED_CODE: i64 = -32000;
/// How long before the request's limit (counted from the claim before the
/// passkey) a proposed transaction is answered "proposed": room for the
/// answer to reach the page before the page gives up on it.
pub const ANSWER_MARGIN_MS: f64 = 20_000.0;

/// `DECLINED`: the person rejected it (4001).
pub fn declined() -> DappError {
    DappError {
        code: DECLINED_CODE,
        message: "User rejected the request".to_owned(),
        data: None,
    }
}

/// What a multisig answers a method it does not take (`multisigRefusal`);
/// `None` for a transaction or a batch of them, which become a proposal. A
/// message is refused: a multisig's signature is every approving member's,
/// which Vela does not collect for messages yet (§6).
pub fn multisig_refusal(method: &str) -> Option<DappError> {
    if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
        return None;
    }
    Some(DappError {
        code: 4200,
        message: "A multisig account signs no messages yet".to_owned(),
        data: None,
    })
}

/// A transaction proposed but not executed — the person left, or the
/// request's limit is near (`leftProposed`): not a rejection, not a
/// disconnect, not a hash. `data` carries the proposal, so nobody proposes
/// it twice.
pub fn left_proposed(safe_tx_hash: &str) -> DappError {
    DappError {
        code: PROPOSED_CODE,
        message: format!(
            "Proposed to the multisig's members (safeTxHash {safe_tx_hash}), waiting for their \
             approvals. This site is not told when it is executed."
        ),
        data: Some(json!({ "safeTxHash": safe_tx_hash, "status": "proposed" })),
    }
}

/// Its number was used by another transaction (a cancellation ran instead).
pub fn cancelled_by_members() -> DappError {
    DappError {
        code: DECLINED_CODE,
        message: "The members cancelled the transaction".to_owned(),
        data: None,
    }
}

/// A transaction needs a chain.
pub fn no_chain() -> DappError {
    DappError {
        code: -32602,
        message: "A transaction needs a chain".to_owned(),
        data: None,
    }
}

/// When a held `eth_sendTransaction` is told it was proposed: the claim
/// before the passkey plus the request's limit, less [`ANSWER_MARGIN_MS`];
/// `None`: the request has no limit.
pub fn tell_at(claimed_ms: f64, limit_ms: Option<f64>) -> Option<f64> {
    limit_ms.map(|limit| claimed_ms + limit - ANSWER_MARGIN_MS)
}

/// What closing the request's sheet owes the site now (`closing`).
pub fn closing(proposed: Option<&str>) -> DappAnswer {
    DappAnswer::err(proposed.map_or_else(declined, left_proposed))
}

/// Where a held request stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigHeldStage"))]
pub enum HeldStage {
    /// dapp-7: a method a multisig cannot do — said in the window first; the
    /// answer goes when the person has read it (or leaves).
    Refused,
    /// Its review (the draft) is up; nothing signed.
    #[default]
    Reviewing,
    /// Proposed: an `eth_sendTransaction` waits for its execution.
    Waiting,
    /// Answered (the answer is final).
    Answered,
}

/// A dApp's request held by the proposals machine, for the shell to draw and,
/// once `answer` is set, to answer the site with — exactly once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigHeldRequest"))]
pub struct HeldRequest {
    pub id: String,
    pub method: String,
    pub origin: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe: String,
    pub stage: HeldStage,
    /// What the core read the request as (one call, or a batch's every call).
    #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
    pub calls: Vec<crate::multisig::CallArg>,
    /// Its proposal, once proposed.
    pub safe_tx_hash: Option<String>,
    /// Its number, once proposed (decimal).
    pub nonce: Option<String>,
    /// When the site is told it was proposed (the limit less the margin).
    pub tell_at: Option<f64>,
    /// The site was told it was proposed and is waiting: the sheet stays,
    /// saying so; the proposal goes on in the wallet.
    pub told: bool,
    /// dapp-7: what a refused method is answered once the person read it.
    pub refusal: Option<DappError>,
    /// What the site is answered — final once set.
    pub answer: Option<DappAnswer>,
}

impl HeldRequest {
    /// Its answer is final: nothing about it changes again.
    pub fn settled(&self) -> bool {
        self.answer.is_some()
    }

    /// Answer the site with `answer`, once.
    pub fn settle(&mut self, answer: DappAnswer) {
        if self.answer.is_none() {
            self.answer = Some(answer);
            self.stage = HeldStage::Answered;
        }
    }
}
