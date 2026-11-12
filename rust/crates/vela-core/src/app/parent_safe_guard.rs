//! The parent-Safe guard — a site may never make this account act for a
//! Safe it owns (VELA-2026-002, the 0.9.9 hotfix).
//!
//! The self-call guard ([`super::self_call_guard`]) refuses a request that
//! rewrites who controls *this* account. A Vela account can also be an owner
//! of other Safes — a Safe created outside Vela that added this account's
//! address as one of its owners (its "parent" Safe). Those take this
//! account's authority in three shapes, and none of them points at the
//! account itself:
//!
//! - **a call** — `parent.execTransaction(…, sig {r = account, v = 1})` (the
//!   "approved by `msg.sender`" signature: for a 1-of-N parent, an immediate
//!   takeover), `parent.approveHash(h)` (one counting approval), or any owner,
//!   threshold, module, guard, fallback-handler or singleton-changing call the
//!   self-call list names, sent straight or hidden in a `multiSend` /
//!   `execTransaction` payload;
//! - **a signature** — EIP-712 typed data with `primaryType` `SafeTx` or
//!   `SafeMessage` under the parent's domain (`verifyingContract` = the
//!   parent), or a `SafeOp` whose `safe` is the parent (under the 4337
//!   module's domain) or under the parent's own domain. The wallet wraps it in
//!   this account's own `SafeMessage`, which is exactly a nested EIP-1271
//!   approval the parent counts as this owner's (0.9.10: a Safe v1.5.0 checks
//!   an owner contract with `isValidSignature(bytes32)`, which a Vela account
//!   answers with that very wrap — so all three are refused, whatever the
//!   parent's version).
//!
//! 0.9.10: every address is read the way the encoders sign it — bare hex,
//! `0X`, under-width ([`super::self_call_guard::normalise_address`]). Arrival
//! already refuses those forms from a site (`tx_request::is_canonical_address`,
//! `typed_data_request::canonical`); this guard no longer depends on it.
//!
//! The rule (disclosure 2026-09-29, APPENDIX-PARENT): a request from a site —
//! any request that reaches the dApp signing machine — is refused, with no
//! "anyway", when one of those shapes targets a contract whose on-chain
//! `isOwner(account)` answers `true`. That read is made before the request
//! can be signed; when it cannot be made the request is refused "couldn't
//! check", never allowed. A contract that is not a Safe (no code, or no
//! `isOwner`), or a Safe this account does not own, is left as before.
//!
//! This module is pure: it says which targets a request touches, and what the
//! facts make of them. The machine asks for the reads and keeps the answers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::self_call_guard::{
    blocked_function, decode_exec_transaction, decode_multi_send, hex_bytes, normalise_address,
    same_address, selector_of, SelfCallFunction, EXEC_TRANSACTION, MULTI_SEND,
};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// `approveHash(bytes32)`.
pub const APPROVE_HASH: &str = "0xd4d9bdcd";
/// `isOwner(address)` — the read that tells a Safe this account owns.
pub const IS_OWNER: &str = "0x2f54bf6e";

/// How deep to follow nested `multiSend` / `execTransaction` payloads — the
/// self-call guard's own cap.
const MAX_DEPTH: u8 = 4;

/// One place a request would act for a Safe other than the signing account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ParentTarget {
    /// The Safe, lowercase `0x…`.
    pub safe: String,
    /// The chain the call runs on, or the typed data's domain names.
    pub chain_id: u32,
    /// Stable wire name of what it would do (`execTransaction`,
    /// `approveHash`, `swapOwner`, `SafeTx`, `SafeMessage`, …).
    pub function: String,
    /// `0x…` four bytes; empty for typed data.
    pub selector: String,
    /// 1-based position in a `wallet_sendCalls` batch.
    pub leg_index: Option<u32>,
    /// Found inside a `multiSend` or `execTransaction` payload.
    pub nested: bool,
}

/// Why a request was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ParentReason {
    /// The chain says this account is one of its owners.
    Owner,
    /// Whether this account owns it could not be read — refused all the same.
    Unchecked,
}

/// What the shell's `eth_call` of `isOwner(account)` on the Safe came back as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SafeOwnerAnswer {
    /// The node answered `result` (hex).
    Returned { data: String },
    /// The node answered a JSON-RPC error.
    RpcError {
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        code: Option<i64>,
        #[serde(default)]
        message: String,
    },
    /// No node answered (offline, timed out, every endpoint failed).
    Unreachable {
        #[serde(default)]
        detail: String,
    },
}

/// What one read says about one Safe.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum OwnerReading {
    /// `isOwner(account)` is `true`.
    Owner,
    /// `isOwner(account)` is `false`.
    NotOwner,
    /// No Safe there: no code (an empty answer) or the call reverted — the
    /// address has no `isOwner`, which every Safe has.
    NotASafe,
    /// Anything else: no answer, an error that is not a revert, an answer
    /// that is not one ABI `bool`.
    Unreadable,
}

/// What `answer` says (fail-closed: only a plain revert, an empty answer or
/// one canonical `bool` is read as a fact).
pub fn classify_owner_answer(answer: &SafeOwnerAnswer) -> OwnerReading {
    match answer {
        SafeOwnerAnswer::Returned { data } => {
            let body = data.trim().trim_start_matches("0x");
            if body.is_empty() {
                return OwnerReading::NotASafe;
            }
            if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
                return OwnerReading::Unreadable;
            }
            let bytes = hex_bytes(body);
            if bytes[..31].iter().any(|b| *b != 0) {
                return OwnerReading::Unreadable;
            }
            match bytes[31] {
                0 => OwnerReading::NotOwner,
                1 => OwnerReading::Owner,
                _ => OwnerReading::Unreadable,
            }
        }
        SafeOwnerAnswer::RpcError { code, message } => {
            if *code == Some(3) || message.to_ascii_lowercase().contains("revert") {
                OwnerReading::NotASafe
            } else {
                OwnerReading::Unreadable
            }
        }
        SafeOwnerAnswer::Unreachable { .. } => OwnerReading::Unreadable,
    }
}

/// The calldata of `isOwner(account)`.
pub fn is_owner_calldata(account: &str) -> String {
    let body = account.trim().trim_start_matches("0x").to_ascii_lowercase();
    format!("{IS_OWNER}{body:0>64}")
}

/// One read the machine holds: `safe` on `chain_id`, read as `reading`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct OwnerFact {
    pub chain_id: u32,
    pub safe: String,
    pub reading: OwnerReading,
}

/// One read still to make.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct OwnerRead {
    pub chain_id: u32,
    pub safe: String,
}

/// The verdict on a request's parent targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ParentVerdict {
    /// Nothing in it acts for a Safe this account owns.
    Clear,
    /// Refused: `target` is a Safe this account owns (or could not be read).
    Refuse {
        target: ParentTarget,
        reason: ParentReason,
    },
    /// These Safes must be read (`isOwner`) before anything else is decided.
    Read { reads: Vec<OwnerRead> },
}

/// What the vectors' door ([`check`]) answers: the guard's verdict, or —
/// with `at_arrival` — arrival's refusal before any guard ran.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CheckVerdict {
    Guard(ParentVerdict),
    Arrival(ArrivalRefusal),
}

/// 0.9.10: refused at arrival (`-32602`, `message` the dApp's answer) — an
/// address not written as `0x` and 40 hex characters (a tx `to`, a batch
/// leg's `to`, a typed-data `verifyingContract`), or anything else arrival
/// refuses. The signing machine answers it there, before any sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum ArrivalRefusal {
    Invalid { message: String },
}

fn lower(address: &str) -> String {
    normalise_address(address).unwrap_or_else(|| {
        format!(
            "0x{}",
            address.trim().trim_start_matches("0x").to_ascii_lowercase()
        )
    })
}

struct Walk<'a> {
    signer: &'a str,
    chain_id: u32,
    out: Vec<ParentTarget>,
}

impl Walk<'_> {
    fn push(
        &mut self,
        safe: &str,
        function: &str,
        selector: String,
        leg: Option<u32>,
        nested: bool,
    ) {
        let target = ParentTarget {
            safe: lower(safe),
            chain_id: self.chain_id,
            function: function.to_owned(),
            selector,
            leg_index: leg,
            nested,
        };
        if !self.out.contains(&target) {
            self.out.push(target);
        }
    }

    /// `to` called with `data` (by the account, or by a payload it carries).
    fn call(&mut self, to: &str, data: &str, leg: Option<u32>, nested: bool, depth: u8) {
        let Some(selector) = selector_of(data) else {
            return;
        };
        // The account's own configuration is the self-call guard's. 0.9.10:
        // `to` is read the way the encoder signs it (bare hex too) — a form
        // only this guard did not read was a call it never saw.
        let to = normalise_address(to);
        if let Some(to) = to.as_deref().filter(|to| !same_address(to, self.signer)) {
            let function = if selector.eq_ignore_ascii_case(APPROVE_HASH) {
                Some(SelfCallFunction::ApproveHash)
            } else {
                blocked_function(&selector)
            };
            if let Some(function) = function {
                self.push(to, function.as_str(), selector.clone(), leg, nested);
            }
        }
        if depth >= MAX_DEPTH {
            return;
        }
        if selector.eq_ignore_ascii_case(MULTI_SEND) {
            for inner in decode_multi_send(data) {
                self.call(&inner.to, &inner.data, leg, true, depth + 1);
            }
        } else if selector.eq_ignore_ascii_case(EXEC_TRANSACTION) {
            if let Some(inner) = decode_exec_transaction(data) {
                self.call(&inner.to, &inner.data, leg, true, depth + 1);
            }
        }
    }

    /// A typed-data document: a `SafeTx` / `SafeMessage` / `SafeOp` under
    /// another contract's domain, or a `SafeOp` for another Safe under the
    /// 4337 module's.
    ///
    /// 0.9.10: a Safe v1.5.0 checks an owner contract's signature with
    /// `isValidSignature(bytes32 dataHash, …)` (v1.4.1 passes the hash's
    /// preimage, the legacy `bytes` form). An owner that is a Vela account
    /// answers that form with its `SafeMessage(keccak256(dataHash))` — exactly
    /// the wrap a site's typed data gets. So under a v1.5.0 parent ANY
    /// document whose digest is the parent's own hash is this owner's
    /// approval: its `SafeTx` (an execution), its `SafeMessage` (a message
    /// the parent signs), and its `SafeOp` (a user operation the 4337 module
    /// has the parent check). All three are refused for every Safe this
    /// account owns, whatever its version.
    fn typed(&mut self, doc: &Value) {
        let function = match doc.get("primaryType").and_then(Value::as_str) {
            Some("SafeTx") => SelfCallFunction::SafeTxTypedData,
            Some("SafeMessage") => SelfCallFunction::SafeMessageTypedData,
            Some("SafeOp") => SelfCallFunction::SafeOpTypedData,
            _ => return,
        };
        let chain = doc
            .get("domain")
            .and_then(|d| d.get("chainId"))
            .and_then(chain_of)
            .unwrap_or(self.chain_id);
        let saved = self.chain_id;
        self.chain_id = chain;
        // 0.9.10: the contract as the hasher reads it — bare and under-width
        // too.
        let contract = doc
            .get("domain")
            .and_then(|d| d.get("verifyingContract"))
            .and_then(Value::as_str)
            .and_then(normalise_address);
        let mut contract = contract.filter(|c| !same_address(c, self.signer));
        if function == SelfCallFunction::SafeOpTypedData {
            // The Safe a `SafeOp` acts for is its `safe`. Its domain is the
            // 4337 module's, which is no Safe and is not read as one.
            let safe = doc
                .get("message")
                .and_then(|m| m.get("safe"))
                .and_then(Value::as_str)
                .and_then(normalise_address);
            if let Some(safe) = safe.filter(|s| !same_address(s, self.signer)) {
                self.push(&safe, function.as_str(), String::new(), None, false);
            }
            contract = contract.filter(|c| !same_address(c, crate::safe::SAFE_4337_MODULE));
        }
        if let Some(contract) = contract {
            self.push(&contract, function.as_str(), String::new(), None, false);
        }
        self.chain_id = saved;
    }
}

fn chain_of(value: &Value) -> Option<u32> {
    match value {
        Value::Number(n) => n.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(s) => {
            let t = s.trim();
            match t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => t.parse().ok(),
            }
        }
        _ => None,
    }
    .filter(|n| *n > 0)
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// Every place `method(params)`, signed by `signer` on `chain_id`, would act
/// for a Safe other than `signer` — in the order they appear, each once.
pub fn parent_targets(
    method: &str,
    params: &Value,
    signer: &str,
    chain_id: u32,
) -> Vec<ParentTarget> {
    let mut walk = Walk {
        signer,
        chain_id,
        out: Vec::new(),
    };
    if method.contains("signTypedData") {
        // Every element, as the SafeTx rule reads them: a request is refused
        // for a document anywhere in it, not only the one a reader picked.
        for entry in params.as_array().into_iter().flatten() {
            let doc = match entry {
                Value::String(raw) => serde_json::from_str::<Value>(raw).ok(),
                Value::Object(_) => Some(entry.clone()),
                _ => None,
            };
            if let Some(doc) = doc {
                walk.typed(&doc);
            }
        }
        return walk.out;
    }
    let text = |call: &Value, key: &str| str_field(call, key).unwrap_or_default().to_owned();
    match method {
        "eth_sendTransaction" => {
            if let Some(tx) = params.get(0) {
                let data = str_field(tx, "data")
                    .or_else(|| str_field(tx, "input"))
                    .unwrap_or_default()
                    .to_owned();
                walk.call(&text(tx, "to"), &data, None, false, 0);
            }
        }
        "wallet_sendCalls" => {
            let calls = params
                .get(0)
                .and_then(|p| p.get("calls"))
                .and_then(Value::as_array);
            for (index, call) in calls.into_iter().flatten().enumerate() {
                let data = str_field(call, "data")
                    .or_else(|| str_field(call, "input"))
                    .unwrap_or_default()
                    .to_owned();
                let leg = u32::try_from(index + 1).ok();
                walk.call(&text(call, "to"), &data, leg, false, 0);
            }
        }
        _ => {}
    }
    walk.out
}

/// What the facts make of `targets`: refused for the first one a read says
/// this account owns or could not read; otherwise the reads still missing;
/// otherwise clear.
pub fn verdict(targets: &[ParentTarget], facts: &[OwnerFact]) -> ParentVerdict {
    let fact = |t: &ParentTarget| {
        facts
            .iter()
            .find(|f| f.chain_id == t.chain_id && same_address(&f.safe, &t.safe))
            .map(|f| f.reading)
    };
    for target in targets {
        match fact(target) {
            Some(OwnerReading::Owner) => {
                return ParentVerdict::Refuse {
                    target: target.clone(),
                    reason: ParentReason::Owner,
                }
            }
            Some(OwnerReading::Unreadable) => {
                return ParentVerdict::Refuse {
                    target: target.clone(),
                    reason: ParentReason::Unchecked,
                }
            }
            _ => {}
        }
    }
    let mut reads: Vec<OwnerRead> = Vec::new();
    for target in targets.iter().filter(|t| fact(t).is_none()) {
        let read = OwnerRead {
            chain_id: target.chain_id,
            safe: target.safe.clone(),
        };
        if !reads.contains(&read) {
            reads.push(read);
        }
    }
    if reads.is_empty() {
        ParentVerdict::Clear
    } else {
        ParentVerdict::Read { reads }
    }
}

/// The submit chokepoint's form: a read still missing is "couldn't check" —
/// a request is never signed past a Safe nobody read.
pub fn enforce(targets: &[ParentTarget], facts: &[OwnerFact]) -> ParentVerdict {
    match verdict(targets, facts) {
        ParentVerdict::Read { reads } => {
            let target = targets
                .iter()
                .find(|t| {
                    reads
                        .iter()
                        .any(|r| r.chain_id == t.chain_id && same_address(&r.safe, &t.safe))
                })
                .cloned()
                .unwrap_or_else(|| targets[0].clone());
            ParentVerdict::Refuse {
                target,
                reason: ParentReason::Unchecked,
            }
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// The one door every platform's vectors go through
// ---------------------------------------------------------------------------

/// A read's answer as the vectors (and a shell) name it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnsweredRead {
    pub chain_id: u32,
    pub safe: String,
    pub answer: SafeOwnerAnswer,
}

/// The rule's whole input (`tests/vectors/rules/parent-safe-guard.json`).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct CheckInput {
    pub method: String,
    pub params: Value,
    pub signer: String,
    pub chain_id: u32,
    #[serde(default)]
    pub answers: Vec<AnsweredRead>,
    /// The submit chokepoint's form ([`enforce`]): a read still missing refuses.
    #[serde(default)]
    pub at_submit: bool,
    /// 0.9.10: arrival's own reading first — what the signing machine does
    /// with a site's request before any guard: a transaction's `to`s and
    /// values (`tx_request::canonical_params_json`), a typed-data request's
    /// one document and its `verifyingContract`
    /// (`typed_data_request::signable`). Refused there:
    /// [`ArrivalRefusal::Invalid`]; otherwise the guard reads what arrival
    /// carried on.
    #[serde(default)]
    pub at_arrival: bool,
}

/// Arrival's reading of a site's request (`sign_request`): the params every
/// later reader holds, or the `-32602` message it is refused with.
///
/// # Errors
/// The message the dApp is answered with.
pub fn arrival_params(method: &str, params: &Value) -> Result<Value, String> {
    if crate::typed_data_request::looks_like_typed_data(method) {
        return crate::typed_data_request::signable(method, params)
            .map(|read| read.params())
            .map_err(|error| error.message());
    }
    match crate::tx_request::canonical_params_json(method, &params.to_string()) {
        Err(error) => Err(error.message()),
        Ok(Some(canonical)) => serde_json::from_str(&canonical).map_err(|e| e.to_string()),
        Ok(None) => Ok(params.clone()),
    }
}

/// [`parent_targets`] then [`verdict`] (or [`enforce`]), from one JSON input
/// to one JSON verdict — what wasm, uniffi and the desktop replay.
pub fn check(input: &CheckInput) -> CheckVerdict {
    let params = if input.at_arrival {
        match arrival_params(&input.method, &input.params) {
            Ok(params) => params,
            Err(message) => return CheckVerdict::Arrival(ArrivalRefusal::Invalid { message }),
        }
    } else {
        input.params.clone()
    };
    let targets = parent_targets(&input.method, &params, &input.signer, input.chain_id);
    let facts: Vec<OwnerFact> = input
        .answers
        .iter()
        .map(|a| OwnerFact {
            chain_id: a.chain_id,
            safe: lower(&a.safe),
            reading: classify_owner_answer(&a.answer),
        })
        .collect();
    CheckVerdict::Guard(if input.at_submit {
        enforce(&targets, &facts)
    } else {
        verdict(&targets, &facts)
    })
}

/// [`check`] over JSON: `Err` names a malformed input.
pub fn check_json(input: &str) -> Result<String, String> {
    let input: CheckInput = serde_json::from_str(input).map_err(|e| e.to_string())?;
    serde_json::to_string(&check(&input)).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::primitives::{keccak256, to_hex};

    fn selector(signature: &str) -> String {
        to_hex(&keccak256(signature.as_bytes())[..4], true)
    }

    #[test]
    fn the_selectors_are_the_safe_functions() {
        assert_eq!(selector("approveHash(bytes32)"), APPROVE_HASH);
        assert_eq!(selector("isOwner(address)"), IS_OWNER);
    }

    #[test]
    fn an_owner_answer_is_read_fail_closed() {
        let ret = |d: &str| SafeOwnerAnswer::Returned { data: d.to_owned() };
        let word = |b: u8| format!("0x{:0>64}", format!("{b:x}"));
        assert_eq!(classify_owner_answer(&ret(&word(1))), OwnerReading::Owner);
        assert_eq!(
            classify_owner_answer(&ret(&word(0))),
            OwnerReading::NotOwner
        );
        assert_eq!(classify_owner_answer(&ret("0x")), OwnerReading::NotASafe);
        assert_eq!(
            classify_owner_answer(&ret(&word(2))),
            OwnerReading::Unreadable
        );
        assert_eq!(
            classify_owner_answer(&ret("0x01")),
            OwnerReading::Unreadable
        );
        assert_eq!(
            classify_owner_answer(&SafeOwnerAnswer::RpcError {
                code: Some(3),
                message: String::new()
            }),
            OwnerReading::NotASafe
        );
        assert_eq!(
            classify_owner_answer(&SafeOwnerAnswer::RpcError {
                code: Some(-32000),
                message: "execution reverted".into()
            }),
            OwnerReading::NotASafe
        );
        assert_eq!(
            classify_owner_answer(&SafeOwnerAnswer::RpcError {
                code: Some(-32005),
                message: "rate limited".into()
            }),
            OwnerReading::Unreadable
        );
        assert_eq!(
            classify_owner_answer(&SafeOwnerAnswer::Unreachable {
                detail: "offline".into()
            }),
            OwnerReading::Unreadable
        );
    }

    #[test]
    fn the_read_is_is_owner_of_the_account() {
        assert_eq!(
            is_owner_calldata("0xAAAAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            format!("{IS_OWNER}{:0>64}", "a".repeat(40))
        );
    }
}
