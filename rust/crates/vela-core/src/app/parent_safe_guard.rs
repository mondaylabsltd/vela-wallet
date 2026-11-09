//! The parent-Safe guard (§31) — a site may never make this account act for a
//! Safe it owns.
//!
//! The self-call guard ([`super::self_call_guard`]) refuses a request that
//! rewrites who controls *this* account. A Vela account can also be a member
//! — an owner — of other Safes: every Vela multisig it belongs to (its
//! "parent" Safes). Those take this account's authority in three shapes, and
//! none of them points at the account itself:
//!
//! - **a call** — `parent.execTransaction(…, sig {r = account, v = 1})` (the
//!   "approved by `msg.sender`" signature: for a 1-of-N parent, an immediate
//!   takeover), `parent.approveHash(h)` (one counting approval), or any owner,
//!   threshold, module, guard, fallback-handler or singleton-changing call the
//!   self-call list names, sent straight or hidden in a `multiSend` /
//!   `execTransaction` payload;
//! - **a signature** — EIP-712 typed data with `primaryType` `SafeTx` or
//!   `SafeMessage` under the parent's domain (`verifyingContract` = the
//!   parent). The wallet wraps it in this account's own `SafeMessage`, which
//!   is exactly a nested EIP-1271 approval the parent counts as this owner's.
//!
//! The rule, one for every surface (disclosure 2026-09-29, APPENDIX-PARENT):
//! a request from a site — any request that reaches the dApp signing machine,
//! first-party ones included, or a multisig's `DappRequest` — is refused, with
//! no "anyway", when one of those shapes targets **a Safe this account owns**:
//!
//! 1. any multisig on this device's list (whoever its members are — no site
//!    has a reason to drive one), or
//! 2. any other address whose on-chain `isOwner(account)` answers `true`.
//!    That read is made before the request can be signed; when it cannot be
//!    made the request is refused "couldn't check", never allowed.
//!
//! Vela's own multisig flows (approve, execute, propose, the owner proof)
//! never reach this rule: they run in the multisig machines, whose
//! operations (`passkey` with its `approve` / `record` purpose, `send_calls`
//! with its `execute` purpose) are composed by the core and sign through the
//! multisig sheets and the page's own `vela_multisig*` methods — not through
//! the dApp request path. The distinction is the path a request enters by,
//! never a field a site can write.
//!
//! This module is pure: it says which targets a request touches, and what the
//! facts make of them. The machines ask for the reads and keep the answers.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::self_call_guard::{
    blocked_function, decode_exec_transaction, decode_multi_send, hex_bytes, same_address,
    selector_of, SelfCallFunction, EXEC_TRANSACTION, MULTI_SEND,
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
    /// The Safe is one of this device's multisigs.
    Member,
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

/// R22 (SEC-PARENT-3): what a corroborated read says — the first node's
/// answer, and a second, independent node's. "Owner" and "unreadable" need no
/// second word (a lie can only refuse). "Not an owner" and "not a Safe" clear
/// a request, so they stand only when the second node read the same — or when
/// the person's own node answered alone, by their choice. One node's "no",
/// or two that differ, is unreadable: refused, "couldn't check".
#[must_use]
pub fn classify_corroborated(
    answer: &SafeOwnerAnswer,
    witness: Option<&SafeOwnerAnswer>,
    own_node: bool,
) -> OwnerReading {
    let first = classify_owner_answer(answer);
    if matches!(first, OwnerReading::Owner | OwnerReading::Unreadable) || own_node {
        return first;
    }
    match witness.map(classify_owner_answer) {
        Some(second) if second == first => first,
        _ => OwnerReading::Unreadable,
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

fn lower(address: &str) -> String {
    format!(
        "0x{}",
        address.trim().trim_start_matches("0x").to_ascii_lowercase()
    )
}

fn is_address(text: &str) -> bool {
    let body = text.trim().strip_prefix("0x").unwrap_or("");
    body.len() == 40 && body.bytes().all(|b| b.is_ascii_hexdigit())
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
        // The account's own configuration is the self-call guard's.
        if is_address(to) && !same_address(to, self.signer) {
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

    /// A typed-data document: a `SafeTx` / `SafeMessage` under another
    /// contract's domain.
    fn typed(&mut self, doc: &Value) {
        let function = match doc.get("primaryType").and_then(Value::as_str) {
            Some("SafeTx") => SelfCallFunction::SafeTxTypedData,
            Some("SafeMessage") => SelfCallFunction::SafeMessageTypedData,
            _ => return,
        };
        let Some(contract) = doc
            .get("domain")
            .and_then(|d| d.get("verifyingContract"))
            .and_then(Value::as_str)
            .filter(|c| is_address(c))
        else {
            return;
        };
        if same_address(contract, self.signer) {
            return;
        }
        let chain = doc
            .get("domain")
            .and_then(|d| d.get("chainId"))
            .and_then(chain_of)
            .unwrap_or(self.chain_id);
        let saved = self.chain_id;
        self.chain_id = chain;
        self.push(contract, function.as_str(), String::new(), None, false);
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

/// What the facts make of `targets`: refused for the first one that is a
/// multisig on this device (`known`), or that a read says this account owns
/// or could not read; otherwise the reads still missing; otherwise clear.
pub fn verdict(targets: &[ParentTarget], known: &[String], facts: &[OwnerFact]) -> ParentVerdict {
    if let Some(target) = targets
        .iter()
        .find(|t| known.iter().any(|k| same_address(k, &t.safe)))
    {
        return ParentVerdict::Refuse {
            target: target.clone(),
            reason: ParentReason::Member,
        };
    }
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
pub fn enforce(targets: &[ParentTarget], known: &[String], facts: &[OwnerFact]) -> ParentVerdict {
    match verdict(targets, known, facts) {
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
    pub known: Vec<String>,
    #[serde(default)]
    pub answers: Vec<AnsweredRead>,
    /// The submit chokepoint's form ([`enforce`]): a read still missing refuses.
    #[serde(default)]
    pub at_submit: bool,
}

/// [`parent_targets`] then [`verdict`] (or [`enforce`]), from one JSON input
/// to one JSON verdict — what wasm, uniffi and the desktop replay.
pub fn check(input: &CheckInput) -> ParentVerdict {
    let targets = parent_targets(&input.method, &input.params, &input.signer, input.chain_id);
    let facts: Vec<OwnerFact> = input
        .answers
        .iter()
        .map(|a| OwnerFact {
            chain_id: a.chain_id,
            safe: lower(&a.safe),
            reading: classify_owner_answer(&a.answer),
        })
        .collect();
    if input.at_submit {
        enforce(&targets, &input.known, &facts)
    } else {
        verdict(&targets, &input.known, &facts)
    }
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
