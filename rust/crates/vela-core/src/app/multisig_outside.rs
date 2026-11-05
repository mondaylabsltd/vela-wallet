//! Machines — an owner whose wallet is not Vela (MetaMask, Rabby, a wallet
//! app) acts on a multisig transaction (docs/multisig-vault-design.md §11.2,
//! §11.3; §3.5 and §3.6 before every signature).
//!
//! A port of the web's `external/` (`outside.svelte.ts`, `session.svelte.ts`,
//! `bridge.ts`, `relay.ts`, `typed.ts`, `onchain.ts`, `errors.ts`,
//! `signing.svelte.ts`) with `core/signers.svelte.ts` and the answer rule of
//! `core/page-approval.svelte.ts`, rule for rule. **Two machines in one
//! file**, one per side of the WalletPair channel, sharing every rule above
//! them:
//!
//! - [`page::OutsidePage`] — Vela's `/connect` page, in the browser that holds
//!   the owner's wallet (WalletPair's *Wallet* side). It acts on a
//!   transaction from a link alone (`#for=<members>&ms=<chain>.<safe>.<hash>`,
//!   parsed by the shell with `multisig_link` and handed in as a
//!   [`TxLink`]): the mailbox's copy is believed only once the core re-hashes
//!   it to the link's hash; every approval in it is re-verified and counts
//!   only for an owner on the chain NOW, plus the chain's own
//!   `approvedHashes`; who may act is the chain's word (§3.5, re-read before
//!   every signature with the state-diff review of §3.6). It approves
//!   (`eth_signTypedData_v4`, Safe's `eth_sign` form, or `approveHash` on
//!   chain), executes (`execTransaction`, fee estimated first) and rejects (a
//!   same-number cancellation) through the injected wallet
//!   ([`MultisigOperation::WalletRequest`]). A signed approval the mailbox
//!   does not take is kept on the device and sent again — never signed twice.
//!   The same machine is the page's side of live sessions: rooms joined from
//!   Vela's invite, each request read and held for the owner's tap.
//! - [`session::OutsideSession`] — Vela's side of a live session (WalletPair's
//!   *DApp* side): a room opened (or a pairing reopened), the invite for the
//!   QR code, `walletpair_connect`, the owner among the wallet's accounts,
//!   the network switched, the SafeTx signed — and the answer verified here
//!   before anything is done with it. Cancelling withdraws the request
//!   (`walletpair_cancel`); the pairing is kept for the next signature.
//!
//! The PeerConnect transport (Noise, the relay, its tokens) stays in the
//! shell ([`MultisigOperation::PeerOpen`] and friends); the core decides what
//! to send and how to read what arrives. The relay token's schedule
//! ([`relay`]) is ported as plain rules a shell's transport follows.
//!
//! Nothing a page or a wallet says is believed: every signature is checked
//! here ([`multisig::verify_ecdsa_approval`] must recover the owner for this
//! safeTxHash; [`multisig::ecdsa_form`] tells the `eth_sign` form apart)
//! before it is handed to the mailbox or to the caller.

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::U256;
use crux_core::{render::render, Command};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_gather;
use super::multisig_review_state::{self as review_state, ReviewState};
use super::multisig_shell::{
    self as shell, lock, same_address, ChainMembers, Io, MultisigEffect, MultisigOperation,
    MultisigShellResult, Proposal, Service, ServiceFail, Shared,
};
use crate::multisig::{self, Approval, EcdsaApproval, EcdsaForm, Genesis, SafeTx};

/// How long a signed approval is sent again on its own before it is said
/// as not sent (kept, with Send again).
pub const POST_PATIENCE_MS: f64 = 120_000.0;
/// How long a sent transaction is waited for on chain.
pub const RECEIPT_TIMEOUT_MS: f64 = 180_000.0;
const RECEIPT_EVERY_MS: u64 = 2_000;
/// An on-chain approval read as "no" is asked again at most this often.
pub const RECHECK_MS: f64 = 30_000.0;
/// Every read of the review is bounded (`REVIEW_READ_MS`, `bounded.ts`).
pub const REVIEW_READ_MS: u32 = 8_000;
/// What the page keeps signed and not yet taken (the web's own key).
pub const KEPT_KEY: &str = "vela.connect.unsent.v1";
/// The page's live sessions (the web's own key).
pub const SESSIONS_KEY: &str = "vela.connect.sessions.v1";
/// Vela's pairings with outside wallets (the web's own key).
pub const PAIRINGS_KEY: &str = "vela.multisig.externalWallets.v1";

const APPROVED_HASHES: &str = "0x7d832974";
const APPROVE_HASH: &str = "0xd4d9bdcd";

// ---------------------------------------------------------------------------
// A wallet's refusal, in words a person acts on (`errors.ts`)
// ---------------------------------------------------------------------------

/// What a wallet's error means (r4C-11) — the same reading on both sides.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideWalletProblem"))]
pub enum WalletProblem {
    /// 4001: the person said no in the wallet.
    Declined,
    /// 4100: the account is not connected to this page in the wallet.
    Unauthorized,
    /// 4200 / -32601: the wallet cannot do this (typed data, say).
    Unsupported,
    /// 4901: the wallet is on another network.
    OtherChain,
    /// 4900: the wallet (or the page) went away.
    Disconnected,
    /// Anything else: its own words, short.
    Unknown { reason: String },
}

/// The longest reason said.
pub const MAX_REASON: usize = 120;

/// A wallet's message cut to one readable line: JSON and escapes undone,
/// "Error:" prefixes dropped, at most [`MAX_REASON`] characters.
pub fn short_reason(message: &str) -> String {
    let mut text = message.to_owned();
    if let Ok(parsed) = serde_json::from_str::<Value>(message) {
        match &parsed {
            Value::String(s) => text.clone_from(s),
            Value::Object(o) => {
                if let Some(m) = o.get("message") {
                    text = m.as_str().map_or_else(|| m.to_string(), str::to_owned);
                }
            }
            _ => {}
        }
    }
    let text = text.replace("\\n", " ").replace("\\\"", "\"");
    let mut rest = text.as_str();
    loop {
        let trimmed = rest.trim_start();
        let lower = trimmed.to_ascii_lowercase();
        let Some(prefix) = ["error:", "walleterror:", "callerror:"]
            .iter()
            .find(|p| lower.starts_with(**p))
        else {
            break;
        };
        rest = trimmed[prefix.len()..].trim_start();
    }
    let first = rest.split('\n').next().unwrap_or("");
    let first = first.strip_suffix('\r').unwrap_or(first);
    let line = first.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > MAX_REASON {
        let mut cut: String = line.chars().take(MAX_REASON - 1).collect();
        cut.push('…');
        return cut;
    }
    line
}

fn word_follows(text: &str, word: &str) -> bool {
    text.starts_with(word)
        && text[word.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

/// How a wallet's error reads; `code` is the EIP-1193 one when there is one.
pub fn wallet_problem(code: Option<i64>, message: &str) -> WalletProblem {
    let lower = message.to_lowercase();
    if code == Some(4001)
        || ["user rejected", "user denied", "user cancel"]
            .iter()
            .any(|p| lower.contains(p))
    {
        return WalletProblem::Declined;
    }
    let trimmed = lower.trim();
    if code == Some(4100)
        || word_follows(trimmed, "unauthorized")
        || word_follows(trimmed, "unauthorised")
    {
        return WalletProblem::Unauthorized;
    }
    if matches!(code, Some(4200 | -32601))
        || lower.contains("not supported")
        || lower.contains("not implemented")
        || lower.contains("unsupported")
    {
        return WalletProblem::Unsupported;
    }
    match code {
        Some(4901) => WalletProblem::OtherChain,
        Some(4900) => WalletProblem::Disconnected,
        _ => {
            let reason = short_reason(message);
            WalletProblem::Unknown {
                reason: if reason.is_empty() {
                    code.map(|c| c.to_string()).unwrap_or_default()
                } else {
                    reason
                },
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The SafeTx a typed-data request asks for, and Safe's eth_sign form (`typed.ts`)
// ---------------------------------------------------------------------------

/// A SafeTx an `eth_signTypedData_v4` request asks for, as the core read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSafeTxRequest"))]
pub struct SafeTxRequest {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub tx: SafeTx,
    /// r12: the multisig's genesis as Vela sent it beside the request (the
    /// request object's `genesis` member) — kept only when the core derives
    /// `safe` from it ([`genesis_for`]): a fact the page checks, never a
    /// claim. With it, a multisig deployed nowhere yet and recorded with no
    /// service (one imported from a Safe{Wallet} export) is reviewed — its
    /// members are the genesis's — instead of read as "Couldn't read this
    /// transaction" with "Sign with …" shut for ever.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(type = "unknown", optional))]
    pub genesis: Option<Genesis>,
}

/// r12: `genesis` (a request's `genesis` member) when the core derives
/// `safe` from it — never otherwise.
pub fn genesis_for(safe: &str, genesis: Option<&Value>) -> Option<Genesis> {
    let g: Genesis = serde_json::from_value(genesis?.clone()).ok()?;
    multisig::genesis_address(&g)
        .ok()
        .filter(|a| same_address(a, safe))
        .map(|_| g)
}

const SAFE_TX_FIELDS: [&str; 10] = [
    "to",
    "value",
    "data",
    "operation",
    "safeTxGas",
    "baseGas",
    "gasPrice",
    "gasToken",
    "refundReceiver",
    "nonce",
];

/// `String(value)` as JavaScript gives it.
fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_owned(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}

/// `BigInt(String(value))`: a decimal, a `0x` quantity, a number.
fn big(v: Option<&Value>) -> Option<U256> {
    match v? {
        Value::Number(n) => n.as_u64().map(U256::from),
        Value::String(s) => {
            let s = s.trim();
            if s.is_empty() {
                return Some(U256::ZERO);
            }
            if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                if hex.is_empty() {
                    return None;
                }
                return U256::from_str_radix(hex, 16).ok();
            }
            if !s.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            U256::from_str_radix(s, 10).ok()
        }
        _ => None,
    }
}

fn hex_quantity(v: Option<&Value>) -> Option<String> {
    big(v).map(|n| format!("0x{n:x}"))
}

fn is_address(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The SafeTx an `eth_signTypedData_v4` request's typed data (a JSON string
/// or an object) asks for — only when it is exactly the core's own typed data
/// for that SafeTx (same domain, same types, same fields); `None` otherwise.
pub fn read_safe_tx_request(typed: &Value) -> Option<SafeTxRequest> {
    let parsed = match typed {
        Value::String(s) => serde_json::from_str::<Value>(s).ok()?,
        other => other.clone(),
    };
    if parsed.get("primaryType")?.as_str()? != "SafeTx" {
        return None;
    }
    let domain = parsed.get("domain")?.as_object()?;
    let m = parsed.get("message")?.as_object()?;
    let chain_id = u64::try_from(big(domain.get("chainId"))?).ok()?;
    if chain_id > (1u64 << 53) - 1 {
        return None;
    }
    let safe = domain.get("verifyingContract")?.as_str()?.to_owned();
    if !is_address(&safe) {
        return None;
    }
    let data = match m.get("data") {
        None | Some(Value::Null | Value::Bool(false)) => "0x".to_owned(),
        Some(Value::String(s)) if s.is_empty() => "0x".to_owned(),
        Some(v) => js_string(Some(v)),
    };
    let operation = match m.get("operation")? {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse::<u64>().ok(),
        _ => None,
    }
    .and_then(|o| u8::try_from(o).ok())?;
    let tx = SafeTx {
        to: js_string(m.get("to")),
        value: hex_quantity(m.get("value"))?,
        data,
        operation,
        safe_tx_gas: hex_quantity(m.get("safeTxGas"))?,
        base_gas: hex_quantity(m.get("baseGas"))?,
        gas_price: hex_quantity(m.get("gasPrice"))?,
        gas_token: js_string(m.get("gasToken")),
        refund_receiver: js_string(m.get("refundReceiver")),
        nonce: hex_quantity(m.get("nonce"))?,
    };
    let own = multisig::safe_tx_typed_data(chain_id, &safe, &tx).ok()?;
    if own.get("types") != parsed.get("types") {
        return None;
    }
    let own_domain = own.get("domain")?;
    if !js_string(own_domain.get("verifyingContract")).eq_ignore_ascii_case(&safe)
        || own_domain.get("chainId").and_then(Value::as_u64) != Some(chain_id)
        || domain.len() != 2
    {
        return None;
    }
    let mine_message = own.get("message")?;
    for field in SAFE_TX_FIELDS {
        let mine = mine_message.get(field);
        let theirs = m.get(field);
        let numeric = mine
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()));
        let same = if numeric {
            big(theirs).is_some() && big(theirs) == big(mine)
        } else {
            js_string(mine).eq_ignore_ascii_case(&js_string(theirs))
        };
        if !same {
            return None;
        }
    }
    Some(SafeTxRequest {
        chain_id,
        safe,
        tx,
        genesis: None,
    })
}

/// A `personal_sign` answer over a safeTxHash as Safe reads its `eth_sign`
/// form: `v` made 27/28, then `+ 4` (31/32). The core checks it after.
pub fn eth_sign_form(signature: &str) -> Result<String, String> {
    let clean = signature.strip_prefix("0x").unwrap_or(signature);
    if clean.len() != 130 || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("not a 65-byte signature".to_owned());
    }
    let mut v = u32::from_str_radix(&clean[128..], 16).map_err(|e| e.to_string())?;
    if v < 27 {
        v += 27;
    }
    if v != 27 && v != 28 {
        return Err(format!("v {v} is not a personal_sign answer's"));
    }
    Ok(format!("0x{}{:02x}", &clean[..128], v + 4))
}

/// A wallet's answer as the Safe will read it, or `None` when it is not an
/// owner's in `owners` for this hash (`signing.svelte.ts` `checked`).
pub fn checked(
    safe_tx_hash: &str,
    owners: &[String],
    owner: &str,
    signature: &str,
) -> Option<EcdsaApproval> {
    let approval = multisig::verify_ecdsa_approval(
        safe_tx_hash,
        &EcdsaApproval {
            owner: owner.to_owned(),
            signature: signature.to_owned(),
        },
    )
    .ok()?;
    owners
        .iter()
        .any(|o| same_address(o, &approval.owner))
        .then_some(approval)
}

// ---------------------------------------------------------------------------
// Approvals on chain (`onchain.ts`)
// ---------------------------------------------------------------------------

fn word(hex: &str) -> String {
    format!(
        "{:0>64}",
        hex.strip_prefix("0x").unwrap_or(hex).to_ascii_lowercase()
    )
}

/// `approvedHashes(owner, hash)` calldata.
pub fn approved_hashes_call(owner: &str, hash: &str) -> String {
    format!("{APPROVED_HASHES}{}{}", word(owner), word(hash))
}

/// `approveHash(hash)` calldata: an owner's approval on chain, sent from its own wallet.
pub fn approve_hash_call(hash: &str) -> String {
    format!("{APPROVE_HASH}{}", word(hash))
}

/// What this session read of `approvedHashes`: a yes for good, a no for a while.
pub type OnChainSeen = BTreeMap<String, (bool, f64)>;

/// Which of `owners` approved `hash` on `safe` on chain. An owner the chain
/// does not answer for is left out (not approved, said nowhere). A yes is
/// kept (the Safe never clears it); a no is asked again after [`RECHECK_MS`].
pub async fn on_chain_approvals<Ev: Send + 'static>(
    io: &Io<Ev>,
    seen: &Shared<OnChainSeen>,
    chain_id: u64,
    safe: &str,
    hash: &str,
    owners: &[String],
    now: f64,
) -> Vec<String> {
    let reads = owners.iter().map(|owner| async move {
        let key = format!("{chain_id}:{safe}:{hash}:{owner}").to_ascii_lowercase();
        if let Some((yes, at)) = lock(seen).get(&key).copied() {
            if yes || now - at < RECHECK_MS {
                return yes.then(|| owner.clone());
            }
        }
        let answer = io
            .eth_call(chain_id, safe, &approved_hashes_call(owner, hash))
            .await
            .ok()?;
        let yes = !(answer.starts_with("0x") && answer[2..].bytes().all(|b| b == b'0'));
        lock(seen).insert(key, (yes, now));
        yes.then(|| owner.clone())
    });
    futures::future::join_all(reads)
        .await
        .into_iter()
        .flatten()
        .collect()
}

// ---------------------------------------------------------------------------
// The mailbox's refusal (`outside.svelte.ts`)
// ---------------------------------------------------------------------------

/// A mailbox refusal as the page says it (r5reg-12) — never its raw text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideRefusalKind"))]
pub enum RefusalKind {
    /// It could not read what it checks the setup against: try again.
    SetupUnchecked,
    Setup,
    Signature,
    Full,
    Transaction,
    Other,
}

/// The kind of a mailbox's refusal.
pub fn refusal_kind(refusal: &Value) -> RefusalKind {
    match refusal.get("kind").and_then(Value::as_str) {
        Some("genesis") => {
            let detail = js_string(refusal.get("detail")).to_lowercase();
            if detail.contains("could not be read") {
                RefusalKind::SetupUnchecked
            } else {
                RefusalKind::Setup
            }
        }
        Some("approval") => RefusalKind::Signature,
        Some("full" | "tooManyProposals") => RefusalKind::Full,
        Some("transaction") => RefusalKind::Transaction,
        _ => RefusalKind::Other,
    }
}

/// The mailbox's refusal in one line: its own words where it has them
/// (`detail`, `reason`, `message`), else its kind.
pub fn short_refusal(refusal: &Value) -> String {
    fn words(v: &Value) -> Option<String> {
        match v {
            Value::String(s) => Some(s.clone()),
            Value::Object(r) => {
                for key in ["detail", "reason", "message"] {
                    if let Some(s) = r.get(key).and_then(Value::as_str) {
                        return Some(s.to_owned());
                    }
                }
                if let Some(inner) = r.get("error").and_then(words) {
                    return Some(inner);
                }
                r.get("kind").and_then(Value::as_str).map(str::to_owned)
            }
            _ => None,
        }
    }
    words(refusal)
        .unwrap_or_else(|| refusal.to_string())
        .chars()
        .take(120)
        .collect()
}

/// Whether the mailbox refused a plain account's approval for its form — the
/// `eth_sign` form, which a mailbox taking only EIP-712 calls "not an EIP-712
/// signature".
pub fn refused_form(refusal: &Value) -> bool {
    let text = refusal.to_string().to_ascii_lowercase();
    text.contains("eip712") || text.contains("eip-712")
}

// ---------------------------------------------------------------------------
// The transaction, as this page reads it (`readProposal`, `matchProposal`)
// ---------------------------------------------------------------------------

/// A transaction a link names, parsed by the shell with `multisig_link` (the
/// connect link's `ms=` and `for=`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTxLink"))]
pub struct TxLink {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe: String,
    pub safe_tx_hash: String,
    /// The members Vela asked for (`for=`), lowercase; empty: any.
    #[serde(default)]
    pub for_members: Vec<String>,
}

/// What the mailbox holds under a link's hash.
#[derive(Clone, Debug, PartialEq)]
pub enum Matched {
    Found(Box<Proposal>),
    /// Not proposed yet (or Vela's side still has it).
    Missing,
    /// What it holds under that hash is another transaction: refused.
    Mismatch,
}

/// The proposal the mailbox holds under the link's hash, re-hashed by the core.
pub fn match_proposal(link: &TxLink, list: &[Proposal]) -> Matched {
    let Some(found) = list
        .iter()
        .find(|p| p.safe_tx_hash.eq_ignore_ascii_case(&link.safe_tx_hash))
    else {
        return Matched::Missing;
    };
    match multisig::safe_tx_hash(link.chain_id, &link.safe, &found.safe_tx) {
        Ok(hash) if hash.eq_ignore_ascii_case(&link.safe_tx_hash) => {
            Matched::Found(Box::new(found.clone()))
        }
        _ => Matched::Mismatch,
    }
}

/// One approval as this page counts it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideApprovalRow"))]
pub struct ApprovalRow {
    pub owner: String,
    /// Verified, and its owner is a member on the chain now.
    pub counts: bool,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub approval: Approval,
    pub created_ms: f64,
}

/// Where a transaction stands against the chain's nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideItemStatus"))]
pub enum ItemStatus {
    /// Its number is used.
    Done,
    /// Behind another number.
    Queued,
    Ready,
    Pending,
}

/// The transaction as this page reads it: approvals verified, counted by the
/// chain's owners (`ProposalItem`, what the outside page needs of it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideItem"))]
pub struct OutsideItem {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub review: multisig::Review,
    /// Decimal.
    pub nonce: String,
    pub proposer: String,
    pub approvals: Vec<ApprovalRow>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub approved: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    pub status: ItemStatus,
    /// A rename runs only where the multisig is deployed.
    pub rename_undeployed: bool,
    /// r11 S4: what a queued one waits behind — the proposals machine's own
    /// rule (`multisig_proposals::waits_for`), so the page says the same
    /// numbers as the wallet. Empty unless queued.
    #[serde(default)]
    pub waits_for: Vec<String>,
}

/// `p` for `safe` read against `members` and the chain's own approvals
/// (`readProposal`, the wallet's own rule): `None` when its hash is not its
/// transaction's.
pub fn read_item(
    safe: &str,
    members: &ChainMembers,
    p: &Proposal,
    on_chain: &[String],
) -> Option<OutsideItem> {
    let hash = multisig::safe_tx_hash(p.chain_id, safe, &p.safe_tx).ok()?;
    if !hash.eq_ignore_ascii_case(&p.safe_tx_hash) {
        return None;
    }
    let review = multisig::review(p.chain_id, safe, &p.safe_tx).ok()?;
    let nonce = shell::quantity(&p.safe_tx.nonce)?;
    let member = |a: &str| members.owners.iter().any(|o| same_address(o, a));
    let mut rows: Vec<ApprovalRow> = Vec::new();
    for stored in &p.approvals {
        let a = &stored.approval;
        if rows.iter().any(|r| same_address(&r.owner, a.owner())) {
            continue;
        }
        // An on-chain approval is the chain's to say, read here — never the mailbox's.
        let valid = match a {
            Approval::OnChain(_) => false,
            Approval::Ecdsa(e) => multisig::verify_ecdsa_approval(&hash, e)
                .is_ok_and(|c| same_address(&c.owner, &e.owner)),
            Approval::Passkey(k) => multisig::verify_approval(p.chain_id, &hash, k).is_ok(),
        };
        rows.push(ApprovalRow {
            owner: a.owner().to_owned(),
            counts: valid && member(a.owner()),
            approval: a.clone(),
            created_ms: stored.created_ms,
        });
    }
    for owner in on_chain {
        let at = rows.iter().position(|r| same_address(&r.owner, owner));
        if at.is_some_and(|i| rows[i].counts) {
            continue;
        }
        let row = ApprovalRow {
            owner: owner.clone(),
            counts: member(owner),
            approval: Approval::OnChain(multisig::OnChainApproval {
                owner: owner.clone(),
                on_chain: true,
            }),
            created_ms: p.created_ms,
        };
        match at {
            Some(i) => rows[i] = row,
            None => rows.push(row),
        }
    }
    let approved = rows.iter().filter(|r| r.counts).count() as u64;
    let status = if nonce < members.nonce {
        ItemStatus::Done
    } else if nonce > members.nonce {
        ItemStatus::Queued
    } else if approved >= members.threshold {
        ItemStatus::Ready
    } else {
        ItemStatus::Pending
    };
    let rename_undeployed =
        matches!(review.action, Some(multisig::Action::Rename { .. })) && !members.deployed;
    Some(OutsideItem {
        chain_id: p.chain_id,
        safe_tx_hash: hash,
        review,
        nonce: nonce.to_string(),
        proposer: p.proposer.clone(),
        approvals: rows,
        approved,
        threshold: members.threshold,
        waits_for: if status == ItemStatus::Queued {
            super::multisig_proposals::waits_for(members.nonce, nonce)
        } else {
            Vec::new()
        },
        status,
        rename_undeployed,
    })
}

// ---------------------------------------------------------------------------
// Who may be asked to sign through an outside wallet (`signers.svelte.ts`)
// ---------------------------------------------------------------------------

// The one rule of what a member is and how it signs lives in
// `multisig_signers` (every machine uses it); these are its names here.
pub use super::multisig_signers::{
    plain_accounts, signer_kind, ApprovedWith, SignerFacts, SignerKind,
};

/// What the proposals this device reads say of how `address` approved, if it
/// did (r4a-9): only a counted approval is evidence.
pub fn approved_with(items: &[Vec<ApprovalRow>], address: &str) -> Option<ApprovedWith> {
    super::multisig_signers::approved_with_in(
        items
            .iter()
            .flatten()
            .map(|a| (a.owner.as_str(), a.counts, &a.approval)),
        address,
    )
}

// ---------------------------------------------------------------------------
// A Vela member's approval on its trusted page (`page-approval.svelte.ts`)
// ---------------------------------------------------------------------------

/// The approval a member's trusted page answered
/// ([`MultisigShellResult::PageApproved`]), accepted only as the chain will:
/// a passkey approval by `member` that verifies for `safe_tx_hash`
/// ([`multisig::verify_approval`]). Nothing the page says beside it is read.
pub fn accept_page_approval(
    approval: &Value,
    chain_id: u64,
    member: &str,
    safe_tx_hash: &str,
) -> Result<multisig::PasskeyApproval, String> {
    let a: multisig::PasskeyApproval =
        serde_json::from_value(approval.clone()).map_err(|e| e.to_string())?;
    if !same_address(&a.owner, member) {
        return Err("not the member's approval".to_owned());
    }
    multisig::verify_approval(chain_id, safe_tx_hash, &a).map_err(|e| e.to_string())?;
    Ok(a)
}

/// What the pages answered this session, by member, chain and safeTxHash:
/// the same bytes approved twice are one approval — the person is not asked
/// again (`answered`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageAnswers(BTreeMap<String, multisig::PasskeyApproval>);

impl PageAnswers {
    fn key(member: &str, chain_id: u64, safe_tx_hash: &str) -> String {
        format!(
            "{}:{chain_id}:{}",
            member.to_ascii_lowercase(),
            safe_tx_hash.to_ascii_lowercase()
        )
    }

    /// The approval answered before, if any: nothing is asked.
    pub fn get(
        &self,
        member: &str,
        chain_id: u64,
        safe_tx_hash: &str,
    ) -> Option<&multisig::PasskeyApproval> {
        self.0.get(&Self::key(member, chain_id, safe_tx_hash))
    }

    /// Keep an accepted approval.
    pub fn keep(&mut self, chain_id: u64, safe_tx_hash: &str, approval: multisig::PasskeyApproval) {
        self.0
            .insert(Self::key(&approval.owner, chain_id, safe_tx_hash), approval);
    }
}

// ---------------------------------------------------------------------------
// The relay token's rules (`relay.ts`), for a shell's transport
// ---------------------------------------------------------------------------

/// The relay token's schedule: which relay asks for one, whether a kept one
/// is still good, what an answer means, and how long to wait after a refusal
/// — the rules a shell's PeerConnect transport follows (r4C-1, r4C-4).
pub mod relay {
    use serde::{Deserialize, Serialize};

    #[cfg(feature = "bindings")]
    use ts_rs::TS;

    pub const DEFAULT_RELAY: &str = "wss://relay.peerconnect.dev";
    pub const TOKEN_ENDPOINT: &str = "https://getvela.app/api/peerconnect-token";
    pub const TRIAL_ENDPOINT: &str = "https://peerconnect.dev/api/trial";
    /// A token this close to expiring (seconds) is replaced before it is used.
    pub const MARGIN_S: f64 = 600.0;
    /// Waits between tokens, refused or not given: each longer, at most 30 s.
    pub const BACKOFF_MS: [u64; 5] = [2_000, 4_000, 8_000, 16_000, 30_000];
    /// Tokens refused in a row before it stops: the kept one, a fresh one, the trial's.
    pub const REFUSALS: u32 = 3;
    /// With a token and still offline this long: the relay is said not to answer.
    pub const UNREACHABLE_AFTER_MS: f64 = 10_000.0;

    /// Whether `relay` is the public relay, the one that asks for tokens.
    pub fn needs_token(relay: &str) -> bool {
        relay.trim_end_matches('/') == DEFAULT_RELAY
    }

    /// Who issued a token.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "RelayTokenSource"))]
    pub enum TokenSource {
        Site,
        Trial,
    }

    /// A token kept on the device (`vela.peerconnectToken.v1`).
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "RelayKeptToken"))]
    pub struct KeptToken {
        pub token: String,
        /// Unix seconds.
        pub expires_at: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source: Option<TokenSource>,
    }

    /// The kept token, when it has time left (and is the trial's, when the
    /// trial's is asked for).
    pub fn usable(
        kept: Option<&KeptToken>,
        now_s: f64,
        trial: bool,
    ) -> Option<(String, TokenSource)> {
        let k = kept?;
        let source = k.source.unwrap_or(TokenSource::Site);
        (k.expires_at - MARGIN_S > now_s && (!trial || source == TokenSource::Trial))
            .then(|| (k.token.clone(), source))
    }

    /// What one token endpoint answered: a token, or what to say.
    pub fn token_answer(status: Option<u16>, body: &str) -> Result<KeptToken, String> {
        let Some(status) = status else {
            return Err("no answer".to_owned());
        };
        if !(200..300).contains(&status) {
            return Err(format!("HTTP {status}"));
        }
        let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "no answer")?;
        match (
            v.get("token").and_then(serde_json::Value::as_str),
            v.get("expires_at").and_then(serde_json::Value::as_f64),
        ) {
            (Some(token), Some(expires_at)) => Ok(KeptToken {
                token: token.to_owned(),
                expires_at,
                source: None,
            }),
            _ => Err("no token in the answer".to_owned()),
        }
    }

    fn host(url: &str) -> &str {
        let rest = url.split_once("://").map_or(url, |(_, r)| r);
        rest.split(['/', '?', '#']).next().unwrap_or(rest)
    }

    /// No token: what each place answered
    /// ("getvela.app: HTTP 429 · peerconnect.dev: HTTP 429").
    pub fn no_token_detail(endpoint: &str, site: Option<&str>, trial: &str) -> String {
        let mut said = Vec::new();
        if let Some(site) = site {
            said.push(format!("{}: {site}", host(endpoint)));
        }
        said.push(format!("{}: {trial}", host(TRIAL_ENDPOINT)));
        said.join(" · ")
    }

    /// Whether the kept token goes when the relay refused `refused` (any, when `None`).
    pub fn forgets(kept: Option<&KeptToken>, refused: Option<&str>) -> bool {
        refused.is_none_or(|r| kept.is_some_and(|k| k.token == r))
    }

    /// Why a device's own side cannot get onto the relay.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "RelayTrouble"))]
    pub enum RelayTrouble {
        /// The relay refused the token (`4403`).
        Refused,
        /// Neither Vela's site nor the trial gave one.
        NoToken,
        /// A token, and still no connection.
        Unreachable,
    }

    /// Where this side's connection to the relay stands, for the screen to say.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "RelayStatus"))]
    pub enum RelayStatus {
        #[default]
        Connecting,
        Up,
        /// Tried again at `at` (ms; `None`: on its own, soon).
        Retrying {
            why: RelayTrouble,
            detail: String,
            at: Option<f64>,
        },
        /// Stopped: said until Retry.
        Failed {
            why: RelayTrouble,
            detail: String,
        },
    }

    impl RelayStatus {
        fn rank(&self) -> u8 {
            match self {
                Self::Up => 0,
                Self::Connecting => 1,
                Self::Retrying { .. } => 2,
                Self::Failed { .. } => 3,
            }
        }
    }

    /// A page's own way onto the relay: the worst of its rooms'.
    pub fn worst<'a>(statuses: impl IntoIterator<Item = &'a RelayStatus>) -> RelayStatus {
        let mut out = RelayStatus::Up;
        for s in statuses {
            if s.rank() > out.rank() {
                out = s.clone();
            }
        }
        out
    }

    /// What a connection does before it asks for its next token.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    pub enum Before {
        /// Ask now (`trial`: the trial's, the last chance before stopping).
        Go { trial: bool },
        /// Wait this long first, said as retrying.
        Wait { ms: u64 },
        /// Stop: said as failed, until Retry.
        Stop,
    }

    /// One connection's refusals by the relay: never another token at once
    /// (2 s, then 4 s), a fresh one from the site, then the trial's once,
    /// then stopped.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase", default)]
    pub struct Refusals {
        refused: u32,
        trial_tried: bool,
        pending: bool,
        source: Option<TokenSource>,
    }

    impl Refusals {
        /// The relay refused the token in use (`unauthorized`).
        pub fn unauthorized(&mut self) {
            self.pending = true;
        }

        /// Before the next token is asked for.
        pub fn before_token(&mut self) -> Before {
            if self.pending {
                self.pending = false;
                self.refused += 1;
                if self.source == Some(TokenSource::Trial) {
                    self.trial_tried = true;
                }
                if self.refused >= REFUSALS || (self.refused + 1 >= REFUSALS && self.trial_tried) {
                    return Before::Stop;
                }
                let at = usize::try_from(self.refused - 1).unwrap_or(0);
                return Before::Wait {
                    ms: BACKOFF_MS[at.min(BACKOFF_MS.len() - 1)],
                };
            }
            Before::Go {
                trial: self.refused + 1 == REFUSALS && !self.trial_tried,
            }
        }

        /// The person tapped Retry after a stop: counted afresh.
        pub fn retried(&mut self) {
            self.refused = 0;
            self.trial_tried = false;
        }

        /// A token was issued by `source`.
        pub fn issued(&mut self, source: TokenSource) {
            self.source = Some(source);
        }

        /// On the relay: the refusals are forgotten.
        pub fn accepted(&mut self) {
            self.refused = 0;
            self.trial_tried = false;
        }
    }

    /// Getting no token at all: wait each step of [`BACKOFF_MS`], then stop.
    pub fn no_token_step(tries: usize) -> Before {
        BACKOFF_MS
            .get(tries)
            .map_or(Before::Stop, |ms| Before::Wait { ms: *ms })
    }

    /// Where a connection stands once its state moved ([`on_state`]).
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    pub struct OnState {
        /// The status to say.
        pub status: RelayStatus,
        /// core-ms-2: it is on the relay (`waiting`/`connected`): its
        /// refusals are forgotten ([`Refusals::accepted`]) — a connection
        /// with a token tells its count so.
        pub accepted: bool,
    }

    /// The status once the connection is `state` (`waiting`/`connected`:
    /// up, and accepted), offline for `offline_ms` with a token: unreachable
    /// after [`UNREACHABLE_AFTER_MS`].
    pub fn on_state(status: &RelayStatus, state: &str, offline_ms: f64, relay: &str) -> OnState {
        OnState {
            status: status_on(status, state, offline_ms, relay),
            accepted: matches!(state, "waiting" | "connected"),
        }
    }

    fn status_on(status: &RelayStatus, state: &str, offline_ms: f64, relay: &str) -> RelayStatus {
        match state {
            "waiting" | "connected" => RelayStatus::Up,
            "closed" => status.clone(),
            _ if offline_ms >= UNREACHABLE_AFTER_MS
                && matches!(status, RelayStatus::Connecting | RelayStatus::Up) =>
            {
                RelayStatus::Retrying {
                    why: RelayTrouble::Unreachable,
                    detail: host(relay).to_owned(),
                    at: None,
                }
            }
            _ if *status == RelayStatus::Up => RelayStatus::Connecting,
            _ => status.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// WalletPair's messages (spec core §2.3, §3, §4)
// ---------------------------------------------------------------------------

/// The EIP-1193 error codes WalletPair answers with.
pub mod code {
    pub const USER_REJECTED: i64 = 4001;
    pub const UNAUTHORIZED: i64 = 4100;
    pub const UNSUPPORTED: i64 = 4200;
    pub const DISCONNECTED: i64 = 4900;
    pub const CHAIN_MISMATCH: i64 = 4901;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL: i64 = -32603;
}

/// An id or a method: printable ASCII, 1 to 128 characters.
fn printable(v: &Value) -> Option<&str> {
    let s = v.as_str()?;
    ((1..=128).contains(&s.len()) && s.bytes().all(|b| (0x20..=0x7e).contains(&b))).then_some(s)
}

fn is_evm_chain(s: &str) -> bool {
    s.strip_prefix("eip155:").is_some_and(|n| {
        (1..=32).contains(&n.len()) && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())
    })
}

fn is_evm_account(s: &str) -> bool {
    s.len() == 42
        && s.starts_with("0x")
        && s[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// `eip155:137` → 137.
pub fn chain_id_of(chain: &str) -> Option<u64> {
    chain.strip_prefix("eip155:")?.parse().ok()
}

/// A display name: 1 to 128 characters, nothing that can disguise it (core §4.1).
fn is_name(v: &Value) -> bool {
    let Some(s) = v.as_str() else { return false };
    let n = s.chars().count();
    (1..=128).contains(&n)
        && s.trim() == s
        && !s.chars().any(|c| {
            c.is_control()
                || matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
        })
}

/// One WalletPair request.
pub fn pair_request(id: &str, method: &str, params: Value, origin: Option<&str>) -> Value {
    let mut r = json!({ "id": id, "method": method, "params": params });
    if let Some(o) = origin {
        r["origin"] = json!(o);
    }
    r
}

/// A response with a result.
pub fn pair_result(id: &str, result: Value) -> Value {
    json!({ "id": id, "result": result })
}

/// A response with an error.
pub fn pair_error(id: &str, code: i64, message: &str) -> Value {
    json!({ "id": id, "error": { "code": code, "message": message } })
}

/// The `state` event.
pub fn pair_state_event(state: Value) -> Value {
    json!({ "event": "state", "data": state })
}

/// The `eip155` part of a session state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsidePairState"))]
pub struct PairState {
    pub chain: String,
    pub accounts: Vec<String>,
}

/// A session state (core §4.2): `None` when invalid; `Some(None)` when it
/// approves nothing this side knows.
pub fn parse_state(v: &Value) -> Option<Option<PairState>> {
    let o = v.as_object()?;
    let mut out = None;
    for (namespace, entry) in o {
        if !(3..=8).contains(&namespace.len())
            || !namespace
                .bytes()
                .all(|b| b == b'-' || b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return None;
        }
        if namespace != "eip155" {
            continue;
        }
        let chain = entry.get("chain")?.as_str()?;
        if !is_evm_chain(chain) {
            return None;
        }
        let accounts = entry.get("accounts")?.as_array()?;
        if accounts.is_empty() || accounts.len() > 64 {
            return None;
        }
        let mut list: Vec<String> = Vec::new();
        for a in accounts {
            let a = a.as_str()?;
            if !is_evm_account(a) || list.iter().any(|x| x == a) {
                return None;
            }
            list.push(a.to_owned());
        }
        out = Some(PairState {
            chain: chain.to_owned(),
            accounts: list,
        });
    }
    Some(out)
}

/// What a DApp side reads from the Wallet (core §3.4).
#[derive(Clone, Debug, PartialEq)]
pub enum FromWallet {
    Ignore,
    Response {
        id: String,
        answer: Result<Value, (i64, String)>,
    },
    Event {
        event: String,
        data: Value,
    },
}

/// How a DApp side classifies a message from the Wallet.
pub fn parse_from_wallet(v: &Value) -> FromWallet {
    let Some(o) = v.as_object() else {
        return FromWallet::Ignore;
    };
    if let Some(event) = o.get("event").and_then(Value::as_str) {
        return match o.get("data") {
            Some(data) => FromWallet::Event {
                event: event.to_owned(),
                data: data.clone(),
            },
            None => FromWallet::Ignore,
        };
    }
    let Some(id) = o.get("id").and_then(printable) else {
        return FromWallet::Ignore;
    };
    match (o.get("result"), o.get("error")) {
        (Some(result), None) => FromWallet::Response {
            id: id.to_owned(),
            answer: Ok(result.clone()),
        },
        (None, Some(error)) => {
            let (Some(code), Some(message)) = (
                error.get("code").and_then(Value::as_i64),
                error.get("message").and_then(Value::as_str),
            ) else {
                return FromWallet::Ignore;
            };
            FromWallet::Response {
                id: id.to_owned(),
                answer: Err((code, message.to_owned())),
            }
        }
        _ => FromWallet::Ignore,
    }
}

/// The account a request signs or sends as, lowercase (`signingAccount`).
pub fn signing_account(method: &str, params: &Value) -> Option<String> {
    let list = params.as_array();
    let at = |i: usize| list.and_then(|l| l.get(i));
    let account = match method {
        "eth_sendTransaction" => at(0).and_then(|t| t.get("from")),
        "eth_signTypedData_v4" | "eth_sign" => at(0),
        "personal_sign" => at(1),
        _ => None,
    }?;
    account.as_str().map(str::to_ascii_lowercase)
}

/// Whether `account` is one of `members` — any account, when none is named.
pub fn expected(members: &[String], account: &str) -> bool {
    members.is_empty() || members.iter().any(|m| m.eq_ignore_ascii_case(account))
}

// ---------------------------------------------------------------------------
// Shared async helpers
// ---------------------------------------------------------------------------

/// One EIP-1193 request on the injected wallet.
async fn wallet_ask<Ev: Send + 'static>(
    io: &Io<Ev>,
    method: &str,
    params: Value,
) -> Result<Value, (Option<i64>, String)> {
    match io
        .ask(MultisigOperation::WalletRequest {
            method: method.to_owned(),
            params,
        })
        .await
    {
        MultisigShellResult::WalletAnswered { result } => Ok(result),
        MultisigShellResult::WalletRefused { code, message } => Err((code, message)),
        other => Err((
            Some(code::DISCONNECTED),
            format!("unexpected answer {other:?}"),
        )),
    }
}

/// The wallet's active network.
async fn wallet_chain<Ev: Send + 'static>(io: &Io<Ev>) -> Result<u64, (Option<i64>, String)> {
    let r = wallet_ask(io, "eth_chainId", json!([])).await?;
    r.as_str()
        .and_then(shell::quantity)
        .or_else(|| r.as_u64().map(u128::from))
        .and_then(|c| u64::try_from(c).ok())
        .ok_or_else(|| (None, "the wallet named no network".to_owned()))
}

/// The one review of `tx` (§3.6), as a Vela member gets it — the very reads
/// the proposals machine makes ([`multisig_gather::gather_review`]): the
/// core's reading, the three hashes, the state-diff guard (trace and
/// probe), the signing sheet's simulation and each recipient's facts. What
/// the page draws and the sign rule its gate uses. `None`: the core cannot
/// read the transaction at all.
pub async fn review_now<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
    members: &ChainMembers,
    genesis: Option<&Genesis>,
) -> Option<crate::multisig_review::MultisigReview> {
    let subject = multisig_gather::ReviewSubject {
        chain_id,
        safe: safe.to_owned(),
        tx: tx.clone(),
        members: members.clone(),
        genesis: genesis.cloned(),
        // The page knows no contacts: the members are who it knows.
        known: Vec::new(),
        member_names: BTreeMap::new(),
        origin: None,
        tokens: BTreeMap::new(),
    };
    multisig_gather::review_of(&multisig_gather::gather_review(io, &subject).await)
}

/// §12: [`review_now`], bounded by `REVIEW_TIMEOUT_MS` — the review and
/// where it stands: `ready`, or `failed` (timed out, not readable). A
/// review never waits for ever, and none is "allowed" by default.
pub async fn review_read<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
    members: &ChainMembers,
    genesis: Option<&Genesis>,
) -> (Option<crate::multisig_review::MultisigReview>, ReviewState) {
    match review_state::bounded(io, review_now(io, chain_id, safe, tx, members, genesis)).await {
        Ok(Some(review)) => (Some(review), ReviewState::Ready),
        Ok(None) => (None, ReviewState::failed(review_state::REASON_UNREADABLE)),
        Err(failed) => (None, failed),
    }
}

/// The registry's genesis for `safe`, only if the core derives that address
/// from it (`verifiedGenesis`).
pub async fn verified_genesis<Ev: Send + 'static>(
    io: &Io<Ev>,
    service: &Service,
    safe: &str,
) -> Option<Genesis> {
    let answer = service.fetch_genesis(io, safe, false).await.ok()?;
    let g = answer.genesis.filter(|_| answer.found)?;
    multisig::genesis_address(&g)
        .ok()
        .filter(|a| same_address(a, safe))
        .map(|_| g)
}

// ---------------------------------------------------------------------------
// What a transaction does, in facts a page words (r4C-7, r4C-12)
// ---------------------------------------------------------------------------

/// What a page names a transaction by — its title, its rows — in facts it
/// only words: the token's symbol and decimals read once here, the number
/// decimal, the recipient. Nothing about a transaction is decided by a
/// shell re-reading it.
pub mod reading {
    use std::collections::BTreeMap;

    use alloy_primitives::U256;
    use serde::{Deserialize, Serialize};
    use serde_json::json;

    #[cfg(feature = "bindings")]
    use ts_rs::TS;

    use super::shell::{self, Io};
    use crate::app::multisig_review_state::ReviewState;
    use crate::app::token_registry::registry_token;
    use crate::multisig::{self, Action, SafeTx};

    const SYMBOL: &str = "0x95d89b41";
    const DECIMALS: &str = "0x313ce567";
    const EXEC_TRANSACTION: &str = "0x6a761202";

    /// A token as an amount is said in it.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTokenFacts"))]
    pub struct TokenFacts {
        /// The token contract, lowercase; `None`: the network's own coin.
        pub token: Option<String>,
        /// The registry's symbol, or the contract's own answer; `None` for
        /// the network's coin (the shell's chain table names it) or when
        /// the contract did not answer.
        pub symbol: Option<String>,
        /// `None`: the contract did not say (the amount is said raw).
        pub decimals: Option<u32>,
        /// The wallet's registry names it; else it is the contract's own word.
        pub known: bool,
    }

    impl TokenFacts {
        fn native() -> Self {
            Self {
                token: None,
                symbol: None,
                decimals: Some(18),
                known: true,
            }
        }

        /// `token` on `chain_id`: the registry's, else what was read of it.
        pub fn of(chain_id: u64, token: Option<&str>, read: &BTreeMap<String, Self>) -> Self {
            let Some(token) = token else {
                return Self::native();
            };
            let key = token.to_ascii_lowercase();
            if let Some(r) = u32::try_from(chain_id)
                .ok()
                .and_then(|c| registry_token(c, &key))
            {
                return Self {
                    token: Some(key),
                    symbol: Some(r.symbol.to_owned()),
                    decimals: Some(r.decimals),
                    known: true,
                };
            }
            read.get(&key).cloned().unwrap_or(Self {
                token: Some(key),
                symbol: None,
                decimals: None,
                known: false,
            })
        }
    }

    /// One transfer a transaction makes.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTransferFacts"))]
    pub struct TransferFacts {
        pub to: String,
        /// Base units, decimal.
        pub amount: String,
        pub token: TokenFacts,
    }

    /// What a transaction does, as its one line says it ("Send 500 USDC",
    /// "Add 0x12…34", "Cancel #7"): the core's reading, in facts.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTxTitle"))]
    pub enum TxTitle {
        Send {
            to: String,
            /// Base units, decimal.
            amount: String,
            token: TokenFacts,
        },
        /// Several calls; `transfers`: each leg that sends money, when every
        /// leg does (a batch with calls is read call by call).
        Batch {
            total: u32,
            transfers_only: bool,
            transfers: Vec<TransferFacts>,
        },
        Cancel,
        AddOwner {
            owner: String,
        },
        RemoveOwner {
            owner: String,
        },
        SwapOwner {
            old_owner: String,
            new_owner: String,
        },
        ChangeThreshold {
            threshold: String,
        },
        Rename {
            name: String,
        },
        /// A call to another contract; `value`: the coin it sends, base units.
        Call {
            to: String,
            value: String,
        },
        /// Not one Vela signs (`refusal`: the core's reason).
        Unsupported {
            refusal: Option<String>,
        },
    }

    impl TxTitle {
        /// Who it pays or calls, when that is one address.
        pub fn recipient(&self) -> Option<&str> {
            match self {
                Self::Send { to, .. } | Self::Call { to, .. } => Some(to),
                _ => None,
            }
        }
    }

    /// `review`'s action as its title, its tokens' facts from `read`.
    pub fn title_of(
        chain_id: u64,
        review: &multisig::Review,
        read: &BTreeMap<String, TokenFacts>,
    ) -> TxTitle {
        let Some(action) = &review.action else {
            return TxTitle::Unsupported {
                refusal: review.refusal.clone(),
            };
        };
        let transfer = |token: &Option<String>, to: &str, amount: &str| TransferFacts {
            to: to.to_owned(),
            amount: amount.to_owned(),
            token: TokenFacts::of(chain_id, token.as_deref(), read),
        };
        match action {
            Action::Transfer { token, to, amount } => {
                let t = transfer(token, to, amount);
                TxTitle::Send {
                    to: t.to,
                    amount: t.amount,
                    token: t.token,
                }
            }
            Action::Batch { calls } => {
                let transfers: Vec<TransferFacts> = calls
                    .iter()
                    .filter_map(|c| match &c.action {
                        Action::Transfer { token, to, amount } => Some(transfer(token, to, amount)),
                        _ => None,
                    })
                    .collect();
                let transfers_only = transfers.len() == calls.len();
                TxTitle::Batch {
                    total: u32::try_from(calls.len()).unwrap_or(u32::MAX),
                    transfers_only,
                    transfers: if transfers_only {
                        transfers
                    } else {
                        Vec::new()
                    },
                }
            }
            Action::Cancel => TxTitle::Cancel,
            Action::AddOwner { owner, .. } => TxTitle::AddOwner {
                owner: owner.clone(),
            },
            Action::RemoveOwner { owner, .. } => TxTitle::RemoveOwner {
                owner: owner.clone(),
            },
            Action::SwapOwner {
                old_owner,
                new_owner,
            } => TxTitle::SwapOwner {
                old_owner: old_owner.clone(),
                new_owner: new_owner.clone(),
            },
            Action::ChangeThreshold { threshold } => TxTitle::ChangeThreshold {
                threshold: threshold.clone(),
            },
            Action::Rename { name } => TxTitle::Rename { name: name.clone() },
            Action::Call { to, value, .. } => TxTitle::Call {
                to: to.clone(),
                value: value.clone(),
            },
        }
    }

    /// The name a multisig's record carries, when the core displays it.
    pub fn shown_name(genesis: Option<&multisig::Genesis>) -> Option<String> {
        genesis
            .and_then(|g| g.name.clone())
            .filter(|n| multisig::valid_name(n))
    }

    /// A number as written in a SafeTx (hex or decimal), decimal.
    pub fn decimal(nonce: &str) -> String {
        shell::quantity(nonce).map_or_else(|| nonce.to_owned(), |n| n.to_string())
    }

    /// The ERC-20 tokens `action` moves that the registry does not name.
    fn unknown_tokens(chain_id: u64, action: Option<&Action>) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut add = |a: &Action| {
            if let Action::Transfer {
                token: Some(token), ..
            } = a
            {
                let key = token.to_ascii_lowercase();
                let known = u32::try_from(chain_id)
                    .ok()
                    .and_then(|c| registry_token(c, &key))
                    .is_some();
                if !known && !out.contains(&key) {
                    out.push(key);
                }
            }
        };
        match action {
            Some(Action::Batch { calls }) => calls.iter().for_each(|c| add(&c.action)),
            Some(a) => add(a),
            None => {}
        }
        out
    }

    /// Hex bytes as UTF-8, up to the first NUL; nothing when empty or not UTF-8.
    fn utf8(hex: &str) -> Option<String> {
        let mut out = Vec::new();
        for i in 0..hex.len() / 2 {
            let b = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
            if b == 0 {
                break;
            }
            out.push(b);
        }
        let text = String::from_utf8(out).ok()?;
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    }

    /// An ABI `string` answer (or a legacy `bytes32` one), UTF-8 or nothing.
    pub fn abi_string(hex: &str) -> Option<String> {
        let data = hex.strip_prefix("0x").unwrap_or(hex);
        if data.len() < 64 {
            return None;
        }
        if data.len() < 128 {
            return utf8(data.get(..64)?);
        }
        let length = usize::from_str_radix(data.get(64..128)?.trim_start_matches('0'), 16).ok();
        let end = length.and_then(|l| l.checked_mul(2)?.checked_add(128).map(|e| (l, e)));
        match end {
            Some((l, end)) if l > 0 && l <= 4096 && end <= data.len() => utf8(data.get(128..end)?),
            _ => utf8(data.get(..64)?),
        }
    }

    /// The symbol and decimals of the tokens `action` moves that the
    /// registry does not name, asked of their contracts on `chain_id`.
    pub async fn read_tokens<Ev: Send + 'static>(
        io: &Io<Ev>,
        chain_id: u64,
        action: Option<&Action>,
    ) -> BTreeMap<String, TokenFacts> {
        read_these(io, chain_id, unknown_tokens(chain_id, action), None).await
    }

    /// The symbol and decimals of `tokens` (lowercase), asked of their
    /// contracts on `chain_id`, each read waited for at most `bound_ms`. A
    /// token whose contract did not answer is left out: it is unread, never
    /// read as 18 decimals (r8-2).
    pub async fn read_these<Ev: Send + 'static>(
        io: &Io<Ev>,
        chain_id: u64,
        tokens: Vec<String>,
        bound_ms: Option<u32>,
    ) -> BTreeMap<String, TokenFacts> {
        let reads = futures::future::join_all(tokens.into_iter().map(|token| async move {
            let call = |data: &str| json!([{ "to": token, "data": data }, "latest"]);
            let (symbol, decimals) = futures::join!(
                io.rpc_within(chain_id, "eth_call", call(SYMBOL), bound_ms),
                io.rpc_within(chain_id, "eth_call", call(DECIMALS), bound_ms)
            );
            let symbol = symbol.ok().and_then(|v| v.as_str().and_then(abi_string));
            // r9 (r8-2's class): `decimals()` answers one 32-byte word. An
            // address with no code answers "0x" — no answer, never 0
            // decimals (which read 1,000,000 base units as "1,000,000").
            let decimals = decimals
                .ok()
                .and_then(|v| {
                    v.as_str()
                        .filter(|s| s.strip_prefix("0x").is_some_and(|h| h.len() == 64))
                        .and_then(shell::quantity)
                })
                .and_then(|d| u32::try_from(d).ok())
                .filter(|d| *d <= 36);
            (token, symbol, decimals)
        }))
        .await;
        reads
            .into_iter()
            .filter(|(_, symbol, decimals)| symbol.is_some() || decimals.is_some())
            .map(|(token, symbol, decimals)| {
                (
                    token.clone(),
                    TokenFacts {
                        token: Some(token),
                        symbol,
                        decimals,
                        known: false,
                    },
                )
            })
            .collect()
    }

    // -- the rows of what was asked and done (r4C-12) -----------------------------

    /// What a row was: "Approve #7 · Send 0.02 USDC to 0x…", "Reject #8",
    /// "Execute #7" — or, when it is not a transaction the core read, what
    /// was asked.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideRowLabel"))]
    pub enum RowLabel {
        /// Approve its number (decimal), doing `title`.
        Approve {
            #[cfg_attr(feature = "bindings", ts(type = "number"))]
            chain_id: u64,
            nonce: String,
            title: TxTitle,
        },
        /// The same, said with whom it pays or calls.
        ApproveTo {
            #[cfg_attr(feature = "bindings", ts(type = "number"))]
            chain_id: u64,
            nonce: String,
            title: TxTitle,
            to: String,
        },
        /// The same-number cancellation.
        Reject {
            nonce: String,
        },
        /// `execTransaction` of a SafeTx this page has seen.
        Execute {
            nonce: String,
        },
        /// Typed data that is not a SafeTx the core reads.
        SignTx,
        /// A transaction the page has not matched to a SafeTx.
        SendTx,
        SwitchChain,
        Other {
            method: String,
        },
    }

    /// Approving `review`'s transaction, said as its row is.
    pub fn approve_label(
        chain_id: u64,
        review: &multisig::Review,
        read: &BTreeMap<String, TokenFacts>,
    ) -> RowLabel {
        let nonce = decimal(&review.nonce);
        if matches!(review.action, Some(Action::Cancel)) {
            return RowLabel::Reject { nonce };
        }
        let title = title_of(chain_id, review, read);
        match title.recipient().map(str::to_owned) {
            Some(to) => RowLabel::ApproveTo {
                chain_id,
                nonce,
                title,
                to,
            },
            None => RowLabel::Approve {
                chain_id,
                nonce,
                title,
            },
        }
    }

    fn quantity(s: &str) -> Option<U256> {
        let h = s.strip_prefix("0x").unwrap_or(s);
        if h.is_empty() {
            return Some(U256::ZERO);
        }
        U256::from_str_radix(h, 16).ok()
    }

    /// The `execTransaction` call `data` sent to `to`, matched to a SafeTx
    /// of `known` (multisig, SafeTx) field by field, its bytes too —
    /// `execTransaction` carries no number: its number, decimal.
    pub fn executed_nonce(to: &str, data: &str, known: &[(String, SafeTx)]) -> Option<String> {
        let data = data.to_ascii_lowercase();
        let body = data.strip_prefix(EXEC_TRANSACTION)?;
        let word = |i: usize| -> Option<U256> {
            U256::from_str_radix(body.get(i * 64..(i + 1) * 64)?, 16).ok()
        };
        // The call's own bytes, at the offset word 2 names.
        let calldata = word(2).and_then(|o| usize::try_from(o).ok()).and_then(|o| {
            let at = o.checked_mul(2)?;
            let len =
                usize::try_from(U256::from_str_radix(body.get(at..at + 64)?, 16).ok()?).ok()?;
            body.get(at + 64..at + 64 + len.checked_mul(2)?)
                .map(str::to_owned)
        });
        known.iter().find_map(|(safe, t)| {
            let bytes = t.data.trim_start_matches("0x").to_ascii_lowercase();
            let same = shell::same_address(safe, to)
                && quantity(&t.to) == word(0)
                && quantity(&t.value) == word(1)
                && calldata.as_deref() == Some(bytes.as_str())
                && Some(U256::from(t.operation)) == word(3)
                && quantity(&t.safe_tx_gas) == word(4)
                && quantity(&t.base_gas) == word(5)
                && quantity(&t.gas_price) == word(6)
                && quantity(&t.gas_token) == word(7)
                && quantity(&t.refund_receiver) == word(8);
            same.then(|| decimal(&t.nonce))
        })
    }

    /// How a row ended, as its outcome column says it.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideRowOutcome"))]
    pub enum RowOutcome {
        /// Shown here, not handed to the wallet yet.
        Waiting,
        InWallet,
        /// Signed (typed data).
        Done,
        /// Signed in the `eth_sign` form of Safe.
        SignedHash,
        Sent,
        Switched,
        Withdrawn,
        /// Declined here.
        Declined,
        /// Declined in the wallet.
        WalletDeclined,
        /// Failed: the problem the wallet said, when it said one.
        Failed {
            problem: Option<super::WalletProblem>,
        },
        /// Signed here and kept: the mailbox has not taken it.
        NotSent,
    }

    impl RowOutcome {
        /// Said as trouble.
        pub fn bad(&self) -> bool {
            matches!(self, Self::Failed { .. } | Self::NotSent)
        }
    }

    /// One row of the list on the page: a request from Vela, or what was done here.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideActivityRow"))]
    pub struct ActivityRow {
        pub id: String,
        pub at: f64,
        pub label: RowLabel,
        pub outcome: RowOutcome,
        pub bad: bool,
    }

    /// The review a page draws for a transaction (the one its link names, or
    /// the one Vela asks for) in the one reading of the core.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTxShown"))]
    pub struct TxShown {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        pub chain_id: u64,
        pub safe: String,
        /// Decimal.
        pub nonce: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub hashes: multisig::SafeTxHashes,
        /// The name its record carries, when the core derives the address
        /// from that record and displays the name.
        pub name: Option<String>,
        pub title: TxTitle,
        /// The one review (§3.6); `None`: it could not be gathered now.
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub review: Option<serde_json::Value>,
        /// §12: where `review` stands — the request is handed to the wallet
        /// only when `ready` and its sign rule allows it.
        #[serde(default)]
        pub review_state: ReviewState,
        /// The next number of the chain, decimal, when read.
        pub chain_nonce: Option<String>,
        /// r11 S4: every number it waits behind, once the chain's is read
        /// (`multisig_proposals::waits_for` — the wallet's own rule): say
        /// `proposal.queuedNote` for one, `proposal.queuedNoteMany` for more.
        #[serde(default)]
        pub waits_for: Vec<String>,
    }
}

// ===========================================================================
// The connect page
// ===========================================================================

/// The connect page acting for an outside owner (WalletPair's Wallet side).
pub mod page {
    use super::*;

    // -- events ------------------------------------------------------------------

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsidePageEvent"))]
    pub enum Event {
        /// The page came up. `wallet_name`: the injected wallet's own name
        /// (EIP-6963) — what Vela calls it, never this page's.
        Start {
            service_url: String,
            #[serde(default)]
            wallet_name: String,
        },
        /// The wallet's accounts or network changed (its own events).
        Wallet {
            accounts: Vec<String>,
            #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
            chain_id: Option<u64>,
        },
        /// Connect the injected wallet (`eth_requestAccounts`).
        ConnectWallet,
        /// The transaction a link names (parsed by the shell with `multisig_link`).
        Link { link: TxLink },
        /// Read it again (the page's 12 s refresh, a tap).
        Refresh,
        /// Approve with the wallet: typed data, or Safe's `eth_sign` form.
        Approve {
            account: String,
            #[serde(default)]
            #[cfg_attr(feature = "bindings", ts(type = "'typedData' | 'ethSign' | null"))]
            form: Option<EcdsaForm>,
        },
        /// Approve on chain: `approveHash(safeTxHash)` sent by the wallet.
        ApproveOnChain { account: String },
        /// What executing costs `account`'s wallet, said before it is sent.
        EstimateFee { account: String },
        /// What `account` holds of the network's coin (which pays the fee).
        Balance { account: String },
        /// Execute from the wallet; its fee must have been estimated first.
        Execute { account: String },
        /// Propose the same-number cancellation, approved by `account`.
        Reject { account: String },
        /// Send again what the mailbox has not taken (nothing re-signed).
        SendAgain,
        /// Stop waiting on the wallet (its own prompt stays until dismissed there).
        Cancel,
        /// §12: the person's "Try again" on a review that failed — the
        /// link's transaction's and any request's: `loading` at once, read
        /// again (bounded), then `ready` or `failed`.
        ReviewAgain,
        /// Reconnect every saved live session.
        ResumeAll,
        /// Join the room of a scanned invite, for `members` (the link's).
        Join {
            invite: String,
            #[serde(default)]
            members: Vec<String>,
        },
        /// The person's answer to a session's connection prompt.
        AnswerConnect { session: String, approve: bool },
        /// The person's tap on a request shown: hand it to the wallet, or decline.
        AnswerRequest {
            session: String,
            id: String,
            go: bool,
        },
        /// End a session for both sides.
        End { session: String },
        /// A room's connection state (`offline`, `waiting`, `connected`).
        PeerState { room: String, state: String },
        /// A room paired: its record to keep.
        Paired { room: String, pairing: String },
        /// A room's way onto the relay, from the shell's transport (its
        /// token refused, none given, retrying): said as the page's own.
        Relay {
            #[serde(default)]
            room: String,
            status: relay::RelayStatus,
        },
        #[serde(skip)]
        Opened { session: String, room: String },
        #[serde(skip)]
        Arrived { room: String, message: Value },
        #[serde(skip)]
        Closed { room: String, reason: String },
        #[serde(skip)]
        Changed,
    }

    // -- what the page draws -----------------------------------------------------

    /// Where reading the link's transaction stands.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideLoad"))]
    pub enum OutsideLoad {
        #[default]
        Idle,
        Loading,
        Ready,
        /// The mailbox does not hold it.
        Missing,
        /// What the mailbox holds under that hash is not that transaction.
        Mismatch,
        /// The mailbox (`mailbox`) or the chain (`chain`) did not answer.
        Unavailable {
            what: String,
            detail: String,
        },
    }

    /// What the person does from here.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideAction"))]
    pub enum OutsideAction {
        Approve,
        ApproveOnChain,
        Execute,
        Reject,
    }

    /// How an approval was given.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideApprovalForm"))]
    pub enum ApprovalForm {
        TypedData,
        EthSign,
        OnChain,
    }

    /// Why an action ended without its outcome, when it is not the wallet's.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideFailReason"))]
    pub enum FailReason {
        /// The wallet's signature is not this owner's for this transaction.
        BadSignature,
        /// The mailbox does not take the `eth_sign` form (approve on chain).
        NoEthSign,
        /// Not taken within [`POST_PATIENCE_MS`]: kept, Send again.
        NotSent,
        /// The mailbox refused it (its words, short).
        Refused { detail: String },
        /// Landed and failed (the fee was paid).
        Reverted { tx_hash: String },
        /// Not on chain within [`RECEIPT_TIMEOUT_MS`].
        NotLanded { tx_hash: String },
        /// The wallet answered no transaction.
        NoTransaction,
        /// The genesis deploying it is refused on the code read just now.
        DeployRefused,
        /// The chain did not answer for that code.
        DeployUnverified,
        /// Who can act for it could not be read just now: nothing signed yet.
        Unchecked,
        /// Its genesis is refused on this network: read-only.
        ReadOnly,
        /// The core does not sign this transaction (no override).
        Blocked,
        /// The account is not a member on the chain now.
        NotMember,
    }

    /// Where an action stands.
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideStep"))]
    pub enum OutsideStep {
        #[default]
        Idle,
        /// Reading the chain again before the signature (§3.6).
        Checking {
            what: OutsideAction,
        },
        /// Moving the wallet to the transaction's network.
        Switching {
            since: f64,
        },
        /// The wallet is asking the owner (`index` of `total`).
        Wallet {
            what: OutsideAction,
            since: f64,
            index: u32,
            total: u32,
        },
        /// The wallet cannot sign typed data: the `eth_sign` form, or on chain.
        Unsupported,
        /// Handing it to the mailbox (`retry_at`: sent again then).
        Sending {
            retry_at: Option<f64>,
        },
        /// Sent, waiting for the chain.
        Mining {
            what: OutsideAction,
            since: f64,
            tx_hash: String,
        },
        Approved {
            form: ApprovalForm,
        },
        Executed {
            tx_hash: String,
        },
        Rejected,
        Failed {
            what: OutsideAction,
            problem: Option<WalletProblem>,
            reason: Option<FailReason>,
            refused: Option<RefusalKind>,
            /// Signed and kept: Send again re-sends it, nothing re-signed.
            unsent: bool,
        },
    }

    impl OutsideStep {
        /// r4C-3: the wallet silent this long is said to be waiting, maybe in
        /// another window — with Cancel and Ask again, never "confirm in your
        /// wallet" forever.
        pub fn stalled(&self, now: f64) -> bool {
            match self {
                Self::Switching { since } | Self::Wallet { since, .. } => {
                    now - since > session::STILL_WAITING_MS
                }
                _ => false,
            }
        }

        fn busy(&self) -> bool {
            matches!(
                self,
                Self::Checking { .. }
                    | Self::Switching { .. }
                    | Self::Wallet { .. }
                    | Self::Sending { .. }
                    | Self::Mining { .. }
            )
        }
    }

    /// §3.5 on this page: the core's verdict for the network now.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideGate"))]
    pub enum PageGate {
        Open,
        Unchecked,
        Refused {
            #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
            findings: Vec<Value>,
        },
        Blocked,
        Risk {
            #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
            findings: Vec<Value>,
        },
    }

    impl From<crate::multisig_review::Gate> for PageGate {
        fn from(g: crate::multisig_review::Gate) -> Self {
            use crate::multisig_review::Gate;
            match g {
                Gate::Open => Self::Open,
                Gate::Unchecked => Self::Unchecked,
                Gate::Refused { findings } => Self::Refused { findings },
                Gate::Blocked => Self::Blocked,
                Gate::Risk { findings } => Self::Risk { findings },
            }
        }
    }

    /// The link's transaction, read.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideTxView"))]
    pub struct OutsideTxView {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        pub chain_id: u64,
        pub safe: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub tx: SafeTx,
        pub item: OutsideItem,
        pub members: ChainMembers,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub hashes: multisig::SafeTxHashes,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub genesis: Option<Genesis>,
        /// §3.5: as the chain says now; `None`: not checkable yet.
        #[cfg_attr(feature = "bindings", ts(type = "unknown[] | null"))]
        pub findings: Option<Vec<Value>>,
        pub gate: PageGate,
        /// The one review (§3.6), as `multisig_review` gives it.
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub review: Option<Value>,
        /// §12: where `review` stands. Nothing is approved, executed or
        /// rejected here unless `ready` (`actions` already says so).
        #[serde(default)]
        pub review_state: ReviewState,
        /// What it does, in the facts its title is said with.
        pub title: reading::TxTitle,
        /// The name its record carries, when the core displays it.
        pub name: Option<String>,
        /// r8-24: its number is used — what happened: executed (by which
        /// transaction) or cancelled (another transaction at its number
        /// ran). `None`: still open, or the chain could not say yet ("its
        /// number is used: it was executed or cancelled").
        #[serde(default)]
        pub done: Option<OutsideDone>,
    }

    /// r8-24: what became of a transaction whose number is used.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(
        tag = "kind",
        rename_all = "camelCase",
        rename_all_fields = "camelCase"
    )]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideDone"))]
    pub enum OutsideDone {
        /// It ran, in `tx_hash` (its `ExecutionSuccess`).
        Executed { tx_hash: String },
        /// Another transaction at its number ran: it never will.
        Cancelled,
    }

    impl OutsideTxView {
        fn is_owner(&self, account: &str) -> bool {
            !account.is_empty() && self.members.owners.iter().any(|o| same_address(o, account))
        }

        fn has_approved(&self, account: &str) -> bool {
            !account.is_empty()
                && self
                    .item
                    .approvals
                    .iter()
                    .any(|a| a.counts && same_address(&a.owner, account))
        }

        /// Open, readable by the core, at the next number or later — and
        /// acted on here (§3.5).
        pub fn open(&self) -> bool {
            self.item.status != ItemStatus::Done
                && self.review_state.is_ready()
                && self.item.review.action.is_some()
                && matches!(self.gate, PageGate::Open | PageGate::Risk { .. })
        }

        /// The approvals meet the threshold: executing is what is left.
        pub fn enough(&self) -> bool {
            self.item.approved >= self.item.threshold
        }

        pub fn can_approve(&self, account: &str) -> bool {
            self.open() && self.is_owner(account) && !self.has_approved(account) && !self.enough()
        }

        /// Its number is next and the approvals meet the threshold — with
        /// the owner's own, as `msg.sender`, when it has not approved.
        pub fn can_execute(&self, account: &str) -> bool {
            if account.is_empty() || !self.open() || self.item.status == ItemStatus::Queued {
                return false;
            }
            if self.item.rename_undeployed || (!self.members.deployed && self.genesis.is_none()) {
                return false;
            }
            let own = u64::from(self.is_owner(account) && !self.has_approved(account));
            self.item.approved + own >= self.item.threshold
        }

        /// An owner, on a transaction that is not a cancellation itself.
        pub fn can_reject(&self, account: &str) -> bool {
            self.open()
                && self.is_owner(account)
                && !matches!(self.item.review.action, Some(multisig::Action::Cancel))
        }

        /// What `account` may do here now.
        pub fn actions(&self, account: &str) -> OutsideActions {
            OutsideActions {
                account: account.to_owned(),
                is_owner: self.is_owner(account),
                has_approved: self.has_approved(account),
                open: self.open(),
                enough: self.enough(),
                can_approve: self.can_approve(account),
                can_approve_on_chain: self.can_approve(account) && self.members.deployed,
                can_execute: self.can_execute(account),
                can_reject: self.can_reject(account),
            }
        }
    }

    /// What one account may do on the page.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideActions"))]
    pub struct OutsideActions {
        pub account: String,
        pub is_owner: bool,
        pub has_approved: bool,
        pub open: bool,
        pub enough: bool,
        pub can_approve: bool,
        pub can_approve_on_chain: bool,
        pub can_execute: bool,
        pub can_reject: bool,
    }

    /// What an execution costs the owner's wallet, before it is sent.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideExecutionFee"))]
    pub struct ExecutionFee {
        /// Wei, decimal: all of the transactions it can price now.
        pub wei: String,
        /// Transactions it takes (a multisig not deployed yet is deployed first).
        pub count: u32,
        /// Some could not be priced before the ones ahead of them land.
        pub partial: bool,
        /// r8-15: the most it can cost (wei, decimal): every transaction at
        /// twice the gas price now (a wallet's own ceiling while the price
        /// moves), and one not priced yet at [`UNPRICED_GAS`] — said beside
        /// the estimate, so what the wallet takes is never more than said.
        #[serde(default)]
        pub max_wei: String,
    }

    /// The gas bound for a transaction that cannot be priced before the
    /// ones ahead of it land (an `execTransaction` behind its deployment).
    pub const UNPRICED_GAS: u64 = 600_000;

    /// The fee estimated for an account (`fee: None`: it cannot say).
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideFeeView"))]
    pub struct FeeView {
        pub account: String,
        pub fee: Option<ExecutionFee>,
    }

    /// An account's coin (`wei: None`: the network did not say).
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideBalanceView"))]
    pub struct BalanceView {
        pub account: String,
        pub wei: Option<String>,
    }

    /// The dApp side of a live session, as it named itself.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideDapp"))]
    pub struct DappInfo {
        pub name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub icon: Option<String>,
    }

    /// Where a live request stands on this page.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideStage"))]
    pub enum Stage {
        /// Shown here, waiting for the owner's tap.
        Review,
        /// The wallet cannot sign typed data: the `eth_sign` form, on a tap.
        Fallback,
        /// Handed to the wallet.
        Wallet,
        Answered,
    }

    /// How a live request ended.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideOutcome"))]
    pub enum Outcome {
        Done,
        Rejected,
        Failed,
        Withdrawn,
    }

    /// A transaction a live request sends, as asked.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSendSummary"))]
    pub struct SendSummary {
        pub to: String,
        pub data: String,
        pub value: String,
    }

    /// One request from Vela, as this page logged it (r4C-12).
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideActivityEntry"))]
    pub struct ActivityEntry {
        pub id: String,
        pub at: f64,
        pub session: String,
        pub origin: Option<String>,
        pub method: String,
        /// The account it signs or sends as, lowercase.
        pub account: Option<String>,
        /// The SafeTx a typed-data request asks for; `None`: not one.
        pub safe_tx: Option<SafeTxRequest>,
        pub send: Option<SendSummary>,
        pub stage: Stage,
        pub wallet_at: Option<f64>,
        pub outcome: Option<Outcome>,
        pub problem: Option<WalletProblem>,
        #[cfg_attr(feature = "bindings", ts(type = "'typedData' | 'ethSign' | null"))]
        pub form: Option<EcdsaForm>,
        pub result: Option<String>,
        #[serde(skip)]
        #[cfg_attr(feature = "bindings", ts(skip))]
        params: Value,
    }

    /// A live session, as the page lists it.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionRow"))]
    pub struct SessionRow {
        pub id: String,
        pub state: String,
        pub dapp: Option<DappInfo>,
        pub approved: bool,
        /// The person declined its connection here; Vela was told so.
        pub declined: bool,
        pub created: f64,
        pub state_since: f64,
        pub relay: relay::RelayStatus,
    }

    /// A session asking to connect, waiting for the person.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideConnectPrompt"))]
    pub struct ConnectPrompt {
        pub session: String,
        pub dapp: DappInfo,
    }

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsidePageView"))]
    pub struct OutsidePageView {
        pub link: Option<TxLink>,
        pub load: OutsideLoad,
        pub tx: Option<OutsideTxView>,
        pub step: OutsideStep,
        /// What the wallet's first account may do.
        pub actions: Option<OutsideActions>,
        pub fee: Option<FeeView>,
        pub balance: Option<BalanceView>,
        /// The injected wallet's accounts, lowercase.
        pub accounts: Vec<String>,
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        pub wallet_chain_id: Option<u64>,
        /// Something signed here is kept, not taken by the mailbox yet.
        pub unsent: bool,
        pub sessions: Vec<SessionRow>,
        pub activity: Vec<ActivityEntry>,
        /// The request waiting for the owner's tap (one at a time).
        pub request: Option<ActivityEntry>,
        pub connect_prompt: Option<ConnectPrompt>,
        /// An invite did not open a room (damaged): nothing was saved.
        pub join_failed: Option<String>,
        /// The page's own way onto the relay: the worst of its rooms'.
        pub relay: relay::RelayStatus,
        /// The wallet would not connect this page (`eth_requestAccounts`
        /// refused or failed): said with what to do.
        #[serde(default)]
        pub wallet_failure: Option<WalletProblem>,
        /// r5reg-23: the registry's genesis of the request's multisig, when
        /// the core derives its address from it (it names the multisig).
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        #[serde(default)]
        pub request_genesis: Option<Genesis>,
        /// r4C-14: the request's approvals already meet the threshold, and
        /// its account has not approved: nothing to sign.
        #[serde(default)]
        pub request_enough: bool,
        /// r7-11: the request's review, as the link's own is drawn (its
        /// title and hashes at once, the review once gathered).
        #[serde(default)]
        pub request_review: Option<reading::TxShown>,
        /// r4C-12: every row of the list, newest first: each request from
        /// Vela (its id) and what was done here on the link's transaction.
        #[serde(default)]
        pub rows: Vec<reading::ActivityRow>,
    }

    // -- state -------------------------------------------------------------------

    /// What was signed here and not taken (`Kept`), as the web keeps it.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Kept {
        safe_tx: SafeTx,
        approval: EcdsaApproval,
        then: KeptThen,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    enum KeptThen {
        Approved { form: ApprovalForm },
        Rejected,
    }

    impl KeptThen {
        fn step(self) -> OutsideStep {
            match self {
                Self::Approved { form } => OutsideStep::Approved { form },
                Self::Rejected => OutsideStep::Rejected,
            }
        }

        fn what(self) -> OutsideAction {
            match self {
                Self::Approved { .. } => OutsideAction::Approve,
                Self::Rejected => OutsideAction::Reject,
            }
        }
    }

    /// A saved live session, as the web keeps it (`vela.connect.sessions.v1`).
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct SavedSession {
        id: String,
        record: String,
        #[serde(serialize_with = "shell::int_ms")]
        created: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dapp: Option<DappInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        members: Option<Vec<String>>,
        #[serde(default)]
        store: SessionStore,
    }

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    struct SessionStore {
        approved: Vec<String>,
        chains: BTreeMap<String, String>,
        records: Vec<(String, Option<Value>)>,
        forwarded: Vec<String>,
    }

    impl SavedSession {
        fn approved(&self) -> bool {
            self.store.approved.iter().any(|n| n == "eip155")
        }

        fn chain(&self) -> Option<&String> {
            self.store.chains.get("eip155")
        }

        fn record(&self, id: &str) -> Option<&Option<Value>> {
            self.store
                .records
                .iter()
                .find(|(r, _)| r == id)
                .map(|(_, v)| v)
        }

        fn set_record(&mut self, id: &str, response: Option<Value>) {
            match self.store.records.iter_mut().find(|(r, _)| r == id) {
                Some(entry) => entry.1 = response,
                None => self.store.records.push((id.to_owned(), response)),
            }
            let len = self.store.records.len();
            if len > 64 {
                self.store.records.drain(..len - 64);
            }
        }
    }

    #[derive(Clone, Debug, Default)]
    struct Live {
        room: String,
        state: String,
        since: f64,
        declined: bool,
        prompting: bool,
        waiters: Vec<String>,
        answered_connects: BTreeSet<String>,
    }

    /// The link's transaction, read, with what the gate uses.
    #[derive(Clone, Debug)]
    struct Loaded {
        view: OutsideTxView,
        /// §12: the person asked for its review again (`review_again`):
        /// the next read's answer stands, failed or not.
        again: bool,
    }

    #[derive(Default)]
    pub struct PageState {
        service: Service,
        clock: f64,
        /// r8-24: what became of a used number, per `chainId:safeTxHash`
        /// (final once found: read once).
        done: Shared<BTreeMap<String, OutsideDone>>,
        link: Option<TxLink>,
        load: OutsideLoad,
        loaded: Option<Loaded>,
        step: OutsideStep,
        run: u64,
        unsent: Option<Kept>,
        resumed: bool,
        seen: Shared<OnChainSeen>,
        fee: Option<FeeView>,
        balance: Option<BalanceView>,
        accounts: Vec<String>,
        wallet_chain: Option<u64>,
        wallet_name: String,
        sessions: Vec<SavedSession>,
        live: BTreeMap<String, Live>,
        activity: Vec<ActivityEntry>,
        queue: Vec<(String, String)>,
        connect_prompt: Option<ConnectPrompt>,
        join_failed: Option<String>,
        relays: BTreeMap<String, relay::RelayStatus>,
        wallet_failure: Option<WalletProblem>,
        /// Vela's requests read (by safeTxHash, lowercase).
        request_reads: BTreeMap<String, RequestRead>,
        started: bool,
        /// What was done here on the link's transaction, newest first.
        own: Vec<reading::ActivityRow>,
        /// The step its last row was logged for.
        logged: OutsideStep,
    }

    /// Vela's request as this page read it ([`read_request`]).
    #[derive(Clone, Debug, Default)]
    struct RequestRead {
        genesis: Option<Genesis>,
        tx: Option<OutsideTxView>,
        /// Its review, as the page draws it (r7-11).
        shown: Option<reading::TxShown>,
        /// The facts of the tokens it moves the registry does not name.
        tokens: BTreeMap<String, reading::TokenFacts>,
        /// r8-11: what was asked, kept so a review that could not be
        /// gathered is gathered again (the page's refresh) — a paired
        /// request is never reviewed on its title and hashes alone.
        request: Option<SafeTxRequest>,
        /// Being read now: one read at a time.
        reading: bool,
    }

    /// r12 (the review-state merge's regression): Vela's request for the
    /// very transaction the link names is the same SafeTx (its key is the
    /// same safeTxHash), so a review the link read is that request's review
    /// too. It was read a second time instead — each page refresh read the
    /// unreviewed requests first, one after another — and the request's
    /// "Sign with …" stayed on "Reading the transaction…" past the person's
    /// patience. A review read once is never waited for again: the link's
    /// ready review is the request's.
    fn adopt_link_review(s: &mut PageState) {
        let Some((hash, view)) = s
            .link
            .as_ref()
            .map(|l| l.safe_tx_hash.to_ascii_lowercase())
            .zip(s.loaded.as_ref().map(|l| l.view.clone()))
        else {
            return;
        };
        if !view.review_state.is_ready() || view.review.is_none() {
            return;
        }
        let Some(read) = s.request_reads.get_mut(&hash) else {
            return;
        };
        let Some(shown) = read.shown.as_mut() else {
            return;
        };
        if shown.review_state.is_ready() && shown.review.is_some() {
            return;
        }
        shown.review = view.review.clone();
        shown.review_state = ReviewState::Ready;
        if read.tx.is_none() {
            read.tx = Some(view);
        }
    }

    impl PageState {
        fn view_tx(&self) -> Option<&OutsideTxView> {
            self.loaded.as_ref().map(|l| &l.view)
        }

        fn session_of_room(&self, room: &str) -> Option<String> {
            self.live
                .iter()
                .find(|(_, l)| l.room == room)
                .map(|(id, _)| id.clone())
        }

        fn session_mut(&mut self, id: &str) -> Option<&mut SavedSession> {
            self.sessions.iter_mut().find(|s| s.id == id)
        }

        fn session(&self, id: &str) -> Option<&SavedSession> {
            self.sessions.iter().find(|s| s.id == id)
        }

        fn entry_mut(&mut self, session: &str, id: &str) -> Option<&mut ActivityEntry> {
            self.activity
                .iter_mut()
                .find(|a| a.session == session && a.id == id)
        }

        /// A session's state as the page shows it to Vela (core §4.2).
        fn pair_state(&self, session: &str) -> Value {
            match self.session(session) {
                Some(s) if s.approved() && !self.accounts.is_empty() => match s.chain() {
                    Some(chain) => {
                        json!({ "eip155": { "chain": chain, "accounts": self.accounts } })
                    }
                    None => json!({}),
                },
                _ => json!({}),
            }
        }
    }

    /// r4C-12: how each row is said (what it was and how it ended): the
    /// core reading what was asked, never the shell.
    impl PageState {
        /// Where a request from Vela ended, as its row says it.
        fn entry_outcome(a: &ActivityEntry) -> reading::RowOutcome {
            use reading::RowOutcome as O;
            match a.outcome {
                Some(Outcome::Done) => match a.method.as_str() {
                    "eth_sendTransaction" => O::Sent,
                    "wallet_switchEthereumChain" => O::Switched,
                    _ if a.form == Some(EcdsaForm::EthSign) => O::SignedHash,
                    _ => O::Done,
                },
                Some(Outcome::Withdrawn) => O::Withdrawn,
                Some(Outcome::Rejected) => {
                    if matches!(a.problem, Some(WalletProblem::Declined)) {
                        O::WalletDeclined
                    } else {
                        O::Declined
                    }
                }
                Some(Outcome::Failed) => O::Failed {
                    problem: a.problem.clone(),
                },
                None if a.stage == Stage::Wallet => O::InWallet,
                None => O::Waiting,
            }
        }

        /// What a request from Vela was: the SafeTx it asks to approve, read
        /// by the core (its tokens as read for it), or the SafeTx an
        /// `execTransaction` runs, matched to one this page has seen.
        fn entry_label(&self, a: &ActivityEntry) -> reading::RowLabel {
            use reading::RowLabel as L;
            match a.method.as_str() {
                "eth_signTypedData_v4" => a
                    .safe_tx
                    .as_ref()
                    .and_then(|t| {
                        let review = multisig::review(t.chain_id, &t.safe, &t.tx).ok()?;
                        let tokens = multisig::safe_tx_hash(t.chain_id, &t.safe, &t.tx)
                            .ok()
                            .and_then(|h| self.request_reads.get(&h.to_ascii_lowercase()))
                            .map(|r| r.tokens.clone())
                            .unwrap_or_default();
                        Some(reading::approve_label(t.chain_id, &review, &tokens))
                    })
                    .unwrap_or(L::SignTx),
                "eth_sendTransaction" => a
                    .send
                    .as_ref()
                    .and_then(|send| {
                        let mut known: Vec<(String, SafeTx)> = self
                            .activity
                            .iter()
                            .filter_map(|x| x.safe_tx.as_ref())
                            .map(|t| (t.safe.clone(), t.tx.clone()))
                            .collect();
                        if let Some(v) = self.view_tx() {
                            known.push((v.safe.clone(), v.tx.clone()));
                        }
                        reading::executed_nonce(&send.to, &send.data, &known)
                    })
                    .map_or(L::SendTx, |nonce| L::Execute { nonce }),
                "wallet_switchEthereumChain" => L::SwitchChain,
                other => L::Other {
                    method: other.to_owned(),
                },
            }
        }

        /// What was done here on the transaction of the link, logged once each
        /// time its step ends somewhere new: approved, rejected, executed, failed.
        fn log_step(&mut self, at: f64) {
            use reading::{RowLabel as L, RowOutcome as O};
            if self.step == self.logged {
                return;
            }
            self.logged = self.step.clone();
            let Some(v) = self.view_tx() else { return };
            let nonce = v.item.nonce.clone();
            let what = match &self.step {
                OutsideStep::Approved { .. } => OutsideAction::Approve,
                OutsideStep::Rejected => OutsideAction::Reject,
                OutsideStep::Executed { .. } => OutsideAction::Execute,
                OutsideStep::Failed { what, .. } => *what,
                _ => return,
            };
            let label = match what {
                OutsideAction::Execute => L::Execute { nonce },
                OutsideAction::Reject => L::Reject { nonce },
                OutsideAction::Approve | OutsideAction::ApproveOnChain => {
                    if matches!(v.item.review.action, Some(multisig::Action::Cancel)) {
                        L::Reject { nonce }
                    } else {
                        match v.title.recipient().map(str::to_owned) {
                            Some(to) => L::ApproveTo {
                                chain_id: v.chain_id,
                                nonce,
                                title: v.title.clone(),
                                to,
                            },
                            None => L::Approve {
                                chain_id: v.chain_id,
                                nonce,
                                title: v.title.clone(),
                            },
                        }
                    }
                }
            };
            let outcome = match &self.step {
                OutsideStep::Executed { .. } => O::Sent,
                OutsideStep::Failed { unsent: true, .. } => O::NotSent,
                OutsideStep::Failed { problem, .. } => O::Failed {
                    problem: problem.clone(),
                },
                _ => O::Done,
            };
            let id = format!("here-{at}-{}", self.own.len());
            self.own.insert(
                0,
                reading::ActivityRow {
                    id,
                    at,
                    bad: outcome.bad(),
                    label,
                    outcome,
                },
            );
        }

        /// Every row, newest first: the requests from Vela and what was done here.
        fn rows(&self) -> Vec<reading::ActivityRow> {
            let mut rows: Vec<reading::ActivityRow> = self
                .activity
                .iter()
                .map(|a| {
                    let outcome = Self::entry_outcome(a);
                    reading::ActivityRow {
                        id: a.id.clone(),
                        at: a.at,
                        label: self.entry_label(a),
                        bad: outcome.bad(),
                        outcome,
                    }
                })
                .chain(self.own.iter().cloned())
                .collect();
            rows.sort_by(|x, y| y.at.total_cmp(&x.at));
            rows
        }
    }

    #[derive(Default)]
    pub struct Model {
        state: Shared<PageState>,
    }

    #[derive(Default)]
    pub struct OutsidePage;

    type Cmd = Command<MultisigEffect, Event>;

    fn flow<F, Fut>(model: &Model, f: F) -> Cmd
    where
        F: FnOnce(Io<Event>, Shared<PageState>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let state = model.state.clone();
        Command::all([
            render(),
            Command::new(move |ctx| async move {
                let io = Io::new(ctx);
                let now = io.now().await;
                lock(&state).clock = now;
                f(io.clone(), state.clone()).await;
                let now = io.now().await;
                lock(&state).log_step(now);
                io.emit(Event::Changed);
            }),
        ])
    }

    /// What a live-session decision asks of the shell.
    enum Out {
        Send { room: String, message: Value },
        Close { room: String, end: bool },
        Persist,
        Perform { session: String, id: String },
        Fallback { session: String, id: String },
        Switch { session: String, id: String },
        Accounts { session: String },
        Read { request: Box<SafeTxRequest> },
    }

    impl crux_core::App for OutsidePage {
        type Event = Event;
        type Model = Model;
        type ViewModel = OutsidePageView;
        type Effect = MultisigEffect;

        #[allow(clippy::too_many_lines)]
        fn update(&self, event: Event, model: &mut Model) -> Cmd {
            match event {
                Event::Changed => render(),
                Event::Start {
                    service_url,
                    wallet_name,
                } => {
                    {
                        let mut s = lock(&model.state);
                        s.service.set_url(&service_url);
                        s.wallet_name = wallet_name;
                        // Up already (the wallet picked since): its sessions stand.
                        if std::mem::replace(&mut s.started, true) {
                            return render();
                        }
                    }
                    flow(model, |io, st| async move {
                        let loaded = io.load(&[SESSIONS_KEY]).await;
                        let sessions: Vec<SavedSession> = loaded
                            .get(SESSIONS_KEY)
                            .cloned()
                            .flatten()
                            .and_then(|r| serde_json::from_str::<Vec<Value>>(&r).ok())
                            .map(|rows| {
                                rows.into_iter()
                                    .filter_map(|r| serde_json::from_value(r).ok())
                                    .collect()
                            })
                            .unwrap_or_default();
                        lock(&st).sessions = sessions;
                    })
                }
                Event::Wallet { accounts, chain_id } => {
                    let outs = {
                        let mut s = lock(&model.state);
                        wallet_changed(&mut s, &accounts, chain_id)
                    };
                    perform_outs(model, outs)
                }
                Event::ConnectWallet => {
                    lock(&model.state).wallet_failure = None;
                    flow(model, |io, st| async move {
                        match wallet_ask(&io, "eth_requestAccounts", json!([])).await {
                            Ok(list) => {
                                let accounts = lowercase_list(&list);
                                let chain = wallet_chain(&io).await.ok();
                                let mut s = lock(&st);
                                s.accounts = accounts;
                                s.wallet_chain = chain;
                            }
                            Err((c, m)) => lock(&st).wallet_failure = Some(wallet_problem(c, &m)),
                        }
                    })
                }
                Event::Link { link } => {
                    {
                        let mut s = lock(&model.state);
                        s.run += 1;
                        s.link = Some(link);
                        s.load = OutsideLoad::Loading;
                        s.loaded = None;
                        s.step = OutsideStep::Idle;
                        s.unsent = None;
                        s.resumed = false;
                        s.fee = None;
                        s.balance = None;
                        s.logged = OutsideStep::Idle;
                    }
                    flow(model, |io, st| async move { refresh(&io, &st).await })
                }
                Event::Refresh => flow(model, |io, st| async move { refresh(&io, &st).await }),
                Event::ReviewAgain => {
                    {
                        let mut s = lock(&model.state);
                        if let Some(l) = s.loaded.as_mut() {
                            if !l.view.review_state.is_ready() {
                                l.view.review_state = ReviewState::Loading;
                                l.again = true;
                            }
                        }
                        for r in s.request_reads.values_mut() {
                            if let Some(x) = r.shown.as_mut().filter(|x| x.review.is_none()) {
                                x.review_state = ReviewState::Loading;
                            }
                        }
                    }
                    flow(model, |io, st| async move { refresh(&io, &st).await })
                }
                Event::Cancel => {
                    let mut s = lock(&model.state);
                    s.run += 1;
                    s.step = OutsideStep::Idle;
                    render()
                }
                Event::Approve { account, form } => action(model, move |io, st, run| async move {
                    approve(
                        &io,
                        &st,
                        run,
                        &account,
                        form.unwrap_or(EcdsaForm::TypedData),
                    )
                    .await;
                }),
                Event::ApproveOnChain { account } => action(model, move |io, st, run| async move {
                    approve_on_chain(&io, &st, run, &account).await;
                }),
                Event::Reject { account } => action(model, move |io, st, run| async move {
                    reject(&io, &st, run, &account).await;
                }),
                Event::Execute { account } => action(model, move |io, st, run| async move {
                    execute(&io, &st, run, &account).await;
                }),
                Event::EstimateFee { account } => flow(model, move |io, st| async move {
                    let fee = fee(&io, &st, &account).await;
                    lock(&st).fee = Some(FeeView { account, fee });
                }),
                Event::Balance { account } => flow(model, move |io, st| async move {
                    let chain = lock(&st).view_tx().map(|v| v.chain_id);
                    let Some(chain) = chain else { return };
                    let wei = io
                        .rpc(chain, "eth_getBalance", json!([account, "latest"]))
                        .await
                        .ok()
                        .and_then(|r| r.as_str().and_then(|q| big(Some(&json!(q)))))
                        .map(|w| w.to_string());
                    lock(&st).balance = Some(BalanceView { account, wei });
                }),
                Event::SendAgain => flow(model, |io, st| async move { send_again(&io, &st).await }),
                Event::ResumeAll => {
                    let opens: Vec<(String, String)> = {
                        let s = lock(&model.state);
                        s.sessions
                            .iter()
                            .filter(|x| !s.live.contains_key(&x.id))
                            .map(|x| (x.id.clone(), x.record.clone()))
                            .collect()
                    };
                    Command::all(
                        opens
                            .into_iter()
                            .map(|(id, record)| open_saved(model, id, record)),
                    )
                }
                Event::Join { invite, members } => {
                    lock(&model.state).join_failed = None;
                    flow(model, move |io, st| async move {
                        join(&io, &st, &invite, &members).await;
                    })
                }
                Event::Opened { session, room } => {
                    let outs = {
                        let mut s = lock(&model.state);
                        let clock = s.clock;
                        s.live.insert(
                            session.clone(),
                            Live {
                                room: room.clone(),
                                state: "offline".to_owned(),
                                since: clock,
                                ..Live::default()
                            },
                        );
                        // After a restart, queued events may have been lost (core §4.3).
                        let state = s.pair_state(&session);
                        vec![Out::Send {
                            room: room.clone(),
                            message: pair_state_event(state),
                        }]
                    };
                    Command::all([listen(room), perform_outs(model, outs)])
                }
                Event::Arrived { room, message } => {
                    let state = model.state.clone();
                    let outs = {
                        let mut s = lock(&state);
                        arrived(&mut s, &room, &message)
                    };
                    perform_outs(model, outs)
                }
                Event::Closed { room, reason } => {
                    let mut s = lock(&model.state);
                    if let Some(id) = s.session_of_room(&room) {
                        if reason == "left" {
                            if let Some(l) = s.live.get_mut(&id) {
                                l.state = "offline".to_owned();
                            }
                        } else {
                            forget(&mut s, &id);
                        }
                    }
                    drop(s);
                    perform_outs(model, vec![Out::Persist])
                }
                Event::PeerState { room, state } => {
                    let mut s = lock(&model.state);
                    let clock = s.clock;
                    if let Some(id) = s.session_of_room(&room) {
                        if let Some(l) = s.live.get_mut(&id) {
                            l.state.clone_from(&state);
                            l.since = clock;
                        }
                        let was = s.relays.get(&id).cloned().unwrap_or_default();
                        s.relays.insert(
                            id,
                            relay::on_state(&was, &state, 0.0, relay::DEFAULT_RELAY).status,
                        );
                    }
                    render()
                }
                Event::Paired { room, pairing } => {
                    let mut s = lock(&model.state);
                    if let Some(id) = s.session_of_room(&room) {
                        if let Some(saved) = s.session_mut(&id) {
                            saved.record = pairing;
                        }
                    }
                    drop(s);
                    perform_outs(model, vec![Out::Persist])
                }
                Event::Relay { room, status } => {
                    let mut s = lock(&model.state);
                    if let Some(id) = s.session_of_room(&room) {
                        s.relays.insert(id, status);
                    }
                    render()
                }
                Event::AnswerConnect { session, approve } => {
                    let outs = {
                        let mut s = lock(&model.state);
                        s.connect_prompt = None;
                        if approve {
                            if let Some(l) = s.live.get_mut(&session) {
                                l.declined = false;
                            }
                            vec![Out::Accounts { session }]
                        } else {
                            connect_declined(&mut s, &session)
                        }
                    };
                    perform_outs(model, outs)
                }
                Event::AnswerRequest { session, id, go } => {
                    let outs = {
                        let mut s = lock(&model.state);
                        answer_request(&mut s, &session, &id, go)
                    };
                    perform_outs(model, outs)
                }
                Event::End { session } => {
                    let outs = {
                        let mut s = lock(&model.state);
                        let room = s.live.get(&session).map(|l| l.room.clone());
                        forget(&mut s, &session);
                        let mut outs = vec![Out::Persist];
                        if let Some(room) = room {
                            outs.push(Out::Close { room, end: true });
                        }
                        outs
                    };
                    perform_outs(model, outs)
                }
            }
        }

        fn view(&self, model: &Model) -> OutsidePageView {
            let s = lock(&model.state);
            let tx = s.view_tx().cloned();
            let actions = tx
                .as_ref()
                .zip(s.accounts.first())
                .map(|(t, a)| t.actions(a));
            let request = s.queue.first().and_then(|(session, id)| {
                s.activity
                    .iter()
                    .find(|a| &a.session == session && &a.id == id)
                    .cloned()
            });
            let request_read = request
                .as_ref()
                .and_then(|r| r.safe_tx.as_ref())
                .and_then(|t| multisig::safe_tx_hash(t.chain_id, &t.safe, &t.tx).ok())
                .and_then(|h| s.request_reads.get(&h.to_ascii_lowercase()));
            let request_enough = request
                .as_ref()
                .zip(request_read.and_then(|r| r.tx.as_ref()))
                .is_some_and(|(r, t)| {
                    t.enough() && !t.has_approved(r.account.as_deref().unwrap_or(""))
                });
            OutsidePageView {
                link: s.link.clone(),
                load: s.load.clone(),
                tx,
                step: s.step.clone(),
                actions,
                fee: s.fee.clone(),
                balance: s.balance.clone(),
                accounts: s.accounts.clone(),
                wallet_chain_id: s.wallet_chain,
                unsent: s.unsent.is_some(),
                sessions: s
                    .sessions
                    .iter()
                    .map(|x| {
                        let live = s.live.get(&x.id);
                        SessionRow {
                            id: x.id.clone(),
                            state: live.map_or_else(|| "offline".to_owned(), |l| l.state.clone()),
                            dapp: x.dapp.clone(),
                            approved: x.approved(),
                            declined: live.is_some_and(|l| l.declined),
                            created: x.created,
                            state_since: live.map_or(x.created, |l| l.since),
                            relay: s.relays.get(&x.id).cloned().unwrap_or_default(),
                        }
                    })
                    .collect(),
                activity: s.activity.clone(),
                request,
                connect_prompt: s.connect_prompt.clone(),
                join_failed: s.join_failed.clone(),
                wallet_failure: s.wallet_failure.clone(),
                request_genesis: request_read.and_then(|r| r.genesis.clone()),
                request_enough,
                request_review: request_read.and_then(|r| r.shown.clone()),
                rows: s.rows(),
                relay: relay::worst(
                    s.live
                        .keys()
                        .filter_map(|id| s.relays.get(id))
                        .collect::<Vec<_>>(),
                ),
            }
        }
    }

    fn lowercase_list(v: &Value) -> Vec<String> {
        v.as_array()
            .map(|l| {
                l.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_ascii_lowercase)
                    .collect()
            })
            .unwrap_or_default()
    }

    // -- the link's transaction ----------------------------------------------------

    /// Run an action: nothing while another is under way; `run` is the
    /// generation a Cancel (or another link) ends.
    fn action<F, Fut>(model: &Model, f: F) -> Cmd
    where
        F: FnOnce(Io<Event>, Shared<PageState>, u64) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        let run = {
            let s = lock(&model.state);
            if s.step.busy() {
                return render();
            }
            s.run
        };
        flow(model, move |io, st| f(io, st, run))
    }

    fn current(st: &Shared<PageState>, run: u64) -> bool {
        lock(st).run == run
    }

    fn set_step(st: &Shared<PageState>, run: u64, step: OutsideStep) {
        let mut s = lock(st);
        if s.run == run {
            s.step = step;
        }
    }

    /// Read it again: the mailbox's copy, the chain's members, what the chain approved.
    async fn refresh(io: &Io<Event>, st: &Shared<PageState>) {
        // r8-11: a request whose review could not be gathered yet is read
        // again: its warnings are the same as the link's own review's.
        let unreviewed: Vec<SafeTxRequest> = lock(st)
            .request_reads
            .values()
            .filter(|r| !r.reading && r.shown.as_ref().is_none_or(|x| x.review.is_none()))
            .filter_map(|r| r.request.clone())
            .collect();
        for request in unreviewed {
            read_request(io, st, request).await;
        }
        let (link, service, seen, done) = {
            let s = lock(st);
            (
                s.link.clone(),
                s.service.clone(),
                s.seen.clone(),
                s.done.clone(),
            )
        };
        let Some(link) = link else { return };
        let read = read_link(io, &service, &seen, &done, &link).await;
        let used = {
            let mut s = lock(st);
            if s.link.as_ref() != Some(&link) {
                return;
            }
            match read {
                Err(load) => {
                    s.load = load;
                    return;
                }
                Ok(mut view) => {
                    let used = view.item.status == ItemStatus::Done;
                    // §12: a review read earlier of this very transaction is
                    // not unsaid by a read that failed now (the gate reads
                    // the chain again before any signature) — unless the
                    // person asked for it again.
                    if let Some(old) = s.loaded.as_ref().filter(|l| {
                        !l.again
                            && l.view.review_state.is_ready()
                            && !view.review_state.is_ready()
                            && l.view.hashes == view.hashes
                    }) {
                        view.review = old.view.review.clone();
                        view.review_state = ReviewState::Ready;
                        view.gate = old.view.gate.clone();
                    }
                    s.loaded = Some(Loaded { view, again: false });
                    s.load = OutsideLoad::Ready;
                    adopt_link_review(&mut s);
                    used
                }
            }
        };
        resume(io, st, &link, used).await;
    }

    /// Who the members of `safe` are on `chain_id` now: read with its
    /// genesis, or from its code once deployed; an undeployed one with no
    /// genesis to say who they are cannot be read.
    async fn read_members(
        io: &Io<Event>,
        chain_id: u64,
        safe: &str,
        genesis: Option<&Genesis>,
    ) -> Result<ChainMembers, OutsideLoad> {
        let link = TxLink {
            chain_id,
            safe: safe.to_owned(),
            safe_tx_hash: String::new(),
            for_members: Vec::new(),
        };
        let genesis = genesis.cloned();
        let chain = shell::Chain::default();
        let members = match &genesis {
            Some(g) => chain.read_members(io, link.chain_id, &link.safe, g).await,
            None => match shell::Chain::has_code(io, link.chain_id, &link.safe).await {
                Ok(true) => {
                    // Deployed: the genesis is not read.
                    let none = Genesis {
                        factory: String::new(),
                        singleton: String::new(),
                        salt_nonce: "0x0".to_owned(),
                        initializer: "0x".to_owned(),
                        name: None,
                    };
                    chain
                        .read_members(io, link.chain_id, &link.safe, &none)
                        .await
                }
                Ok(false) => {
                    // Undeployed, and no genesis to say who its members are.
                    return Err(OutsideLoad::Unavailable {
                        what: "chain".to_owned(),
                        detail: "no genesis".to_owned(),
                    });
                }
                Err(e) => Err(e),
            },
        };
        let members = members.map_err(|e| OutsideLoad::Unavailable {
            what: "chain".to_owned(),
            detail: e.detail,
        })?;
        Ok(members)
    }

    /// A transaction a link names, read as this page reads it: the mailbox's
    /// copy (believed only re-hashed to the link's hash), the chain's
    /// members, what the chain approved, who can act for it (§3.5) and the
    /// one review (§3.6) — or why it cannot be read now.
    async fn read_link(
        io: &Io<Event>,
        service: &Service,
        seen: &Shared<OnChainSeen>,
        done_seen: &Shared<BTreeMap<String, OutsideDone>>,
        link: &TxLink,
    ) -> Result<OutsideTxView, OutsideLoad> {
        let answer = service
            .fetch_proposals(io, link.chain_id, &link.safe, false)
            .await
            .map_err(|e| OutsideLoad::Unavailable {
                what: "mailbox".to_owned(),
                detail: service_detail(&e),
            })?;
        let list = answer.map(|a| a.proposals).unwrap_or_default();
        let p = match match_proposal(link, &list) {
            Matched::Found(p) => *p,
            Matched::Missing => return Err(OutsideLoad::Missing),
            Matched::Mismatch => return Err(OutsideLoad::Mismatch),
        };
        let genesis = verified_genesis(io, service, &link.safe).await;
        let members = read_members(io, link.chain_id, &link.safe, genesis.as_ref()).await?;
        let on_chain = if members.deployed {
            let open: Vec<String> = members
                .owners
                .iter()
                .filter(|o| {
                    !p.approvals
                        .iter()
                        .any(|a| same_address(a.approval.owner(), o))
                })
                .cloned()
                .collect();
            let now = io.now().await;
            on_chain_approvals(
                io,
                seen,
                link.chain_id,
                &link.safe,
                &p.safe_tx_hash,
                &open,
                now,
            )
            .await
        } else {
            Vec::new()
        };
        let item = read_item(&link.safe, &members, &p, &on_chain).ok_or(OutsideLoad::Mismatch)?;
        let findings = shell::judge_on(
            io,
            link.chain_id,
            &link.safe,
            genesis.as_ref(),
            members.deployed,
        )
        .await;
        let (review, review_state) = review_read(
            io,
            link.chain_id,
            &link.safe,
            &p.safe_tx,
            &members,
            genesis.as_ref(),
        )
        .await;
        let hashes = multisig::safe_tx_hashes(link.chain_id, &link.safe, &p.safe_tx)
            .map_err(|_| OutsideLoad::Mismatch)?;
        let gate = gate_of(
            link.chain_id,
            &link.safe,
            &p.safe_tx,
            findings.clone(),
            review.as_ref(),
        );
        let tokens = reading::read_tokens(io, link.chain_id, item.review.action.as_ref()).await;
        let title = reading::title_of(link.chain_id, &item.review, &tokens);
        let name = reading::shown_name(genesis.as_ref());
        let done = if item.status == ItemStatus::Done {
            what_ran(io, done_seen, link, &item, p.created_ms).await
        } else {
            None
        };
        Ok(OutsideTxView {
            done,
            chain_id: link.chain_id,
            safe: link.safe.clone(),
            tx: p.safe_tx.clone(),
            title,
            name,
            item,
            members,
            hashes,
            genesis,
            findings,
            gate,
            review: review.and_then(|r| serde_json::to_value(r).ok()),
            review_state,
        })
    }

    /// r8-24: what became of the link's transaction once its number is
    /// used — its own `ExecutionSuccess` since it was proposed says it ran
    /// (and in which transaction); none says another ran at its number. A
    /// chain that does not answer says nothing yet (asked again next read).
    async fn what_ran(
        io: &Io<Event>,
        done_seen: &Shared<BTreeMap<String, OutsideDone>>,
        link: &TxLink,
        item: &OutsideItem,
        since_ms: f64,
    ) -> Option<OutsideDone> {
        let key = format!(
            "{}:{}",
            link.chain_id,
            item.safe_tx_hash.to_ascii_lowercase()
        );
        if let Some(known) = lock(done_seen).get(&key) {
            return Some(known.clone());
        }
        let found = super::super::multisig_proposals::find_execution(
            io,
            link.chain_id,
            &link.safe,
            &item.safe_tx_hash,
            since_ms,
        )
        .await
        .ok()?;
        let done = match found {
            Some(tx_hash) => OutsideDone::Executed { tx_hash },
            None => OutsideDone::Cancelled,
        };
        lock(done_seen).insert(key, done.clone());
        Some(done)
    }

    /// r4C-14 / r5reg-23: Vela's request, read as the link's own is — its
    /// multisig named from the registry's genesis (only if the core derives
    /// the address from it), and counted as the mailbox and the chain hold
    /// it now: its approvals already enough, there is nothing to sign.
    async fn read_request(io: &Io<Event>, st: &Shared<PageState>, request: SafeTxRequest) {
        let Ok(hash) = multisig::safe_tx_hash(request.chain_id, &request.safe, &request.tx) else {
            return;
        };
        let hash = hash.to_ascii_lowercase();
        let (service, seen, done, known) = {
            let s = lock(st);
            (
                s.service.clone(),
                s.seen.clone(),
                s.done.clone(),
                // Read already (with its review), or being read: nothing to do.
                s.request_reads.get(&hash).is_some_and(|r| {
                    r.reading || r.shown.as_ref().is_some_and(|x| x.review.is_some())
                }),
            )
        };
        if known {
            return;
        }
        {
            let mut s = lock(st);
            let entry = s.request_reads.entry(hash.clone()).or_default();
            entry.reading = true;
            entry.request = Some(request.clone());
        }
        let genesis = verified_genesis(io, &service, &request.safe)
            .await
            .or_else(|| request.genesis.clone());
        let (chain_id, safe, tx) = (request.chain_id, request.safe.clone(), request.tx.clone());
        let read = multisig::review(chain_id, &safe, &tx).ok();
        let tokens = match &read {
            Some(r) => reading::read_tokens(io, chain_id, r.action.as_ref()).await,
            None => BTreeMap::new(),
        };
        let shown = |review: Option<Value>, state: ReviewState, chain_nonce: Option<String>| {
            let r = read.as_ref()?;
            Some(reading::TxShown {
                chain_id,
                safe: safe.clone(),
                nonce: reading::decimal(&r.nonce),
                hashes: multisig::safe_tx_hashes(chain_id, &safe, &tx).ok()?,
                name: reading::shown_name(genesis.as_ref()),
                title: reading::title_of(chain_id, r, &tokens),
                review,
                review_state: state,
                waits_for: match (
                    chain_nonce.as_deref().and_then(|n| n.parse::<u128>().ok()),
                    reading::decimal(&r.nonce).parse::<u128>().ok(),
                ) {
                    (Some(chain), Some(own)) => {
                        crate::app::multisig_proposals::waits_for(chain, own)
                    }
                    _ => Vec::new(),
                },
                chain_nonce,
            })
        };
        // Its title and hashes at once; the review once it is gathered —
        // at once too when the link already read this very transaction.
        {
            let mut s = lock(st);
            s.request_reads.insert(
                hash.clone(),
                RequestRead {
                    genesis: genesis.clone(),
                    tx: None,
                    shown: shown(None, ReviewState::Loading, None),
                    tokens: tokens.clone(),
                    request: Some(request.clone()),
                    reading: true,
                },
            );
            adopt_link_review(&mut s);
            let adopted = s
                .request_reads
                .get_mut(&hash)
                .filter(|r| r.shown.as_ref().is_some_and(|x| x.review_state.is_ready()));
            if let Some(read) = adopted {
                read.reading = false;
                return;
            }
        }
        let link = TxLink {
            chain_id,
            safe: safe.to_ascii_lowercase(),
            safe_tx_hash: hash.clone(),
            for_members: Vec::new(),
        };
        // §12: the whole read is bounded (the mailbox, the members, the
        // review): "Reading the transaction…" ends within REVIEW_TIMEOUT_MS,
        // ready or failed with its reason — never a spinner for ever.
        let read = review_state::bounded(io, async {
            match read_link(io, &service, &seen, &done, &link).await {
                Ok(view) => {
                    let review = (view.review.clone(), view.review_state.clone());
                    let nonce = view.members.nonce.to_string();
                    (Some(view), review, Some(nonce))
                }
                // Not in the mailbox yet (Vela asks before it proposes), or not
                // readable there: what Vela asks is reviewed from the chain alone.
                Err(_) => match read_members(io, chain_id, &safe, genesis.as_ref()).await {
                    Ok(members) => {
                        let (review, state) =
                            review_read(io, chain_id, &safe, &tx, &members, genesis.as_ref()).await;
                        let review = review.and_then(|r| serde_json::to_value(r).ok());
                        (None, (review, state), Some(members.nonce.to_string()))
                    }
                    Err(_) => (
                        None,
                        (None, ReviewState::failed(review_state::REASON_UNREADABLE)),
                        None,
                    ),
                },
            }
        })
        .await;
        let (view, (review, state), chain_nonce) = match read {
            Ok(read) => read,
            Err(failed) => (None, (None, failed), None),
        };
        let shown = shown(review, state, chain_nonce);
        let mut s = lock(st);
        // The link may have read it meanwhile: a ready review is not unsaid
        // by this read's failure.
        let kept_ready = s
            .request_reads
            .get(&hash)
            .and_then(|r| r.shown.clone())
            .filter(|x| x.review_state.is_ready() && x.review.is_some());
        let shown = match (kept_ready, shown) {
            (Some(ready), Some(now)) if !now.review_state.is_ready() => Some(ready),
            (_, now) => now,
        };
        s.request_reads.insert(
            hash,
            RequestRead {
                genesis,
                tx: view,
                shown,
                tokens,
                request: Some(request),
                reading: false,
            },
        );
        adopt_link_review(&mut s);
    }

    fn service_detail(e: &ServiceFail) -> String {
        match e {
            ServiceFail::Unavailable(d) => d.clone(),
            ServiceFail::Busy(_) => "busy".to_owned(),
        }
    }

    /// The gate's verdict for what the page shows: §3.5's findings and the
    /// review, decided as for a Vela member ([`multisig_gather::decide`]).
    fn gate_of(
        chain_id: u64,
        safe: &str,
        tx: &SafeTx,
        findings: Option<Vec<Value>>,
        review: Option<&crate::multisig_review::MultisigReview>,
    ) -> PageGate {
        multisig_gather::decide(chain_id, safe, tx, findings, review).into()
    }

    fn kept_prefix(link: &TxLink) -> String {
        format!(
            "{}:{}:{}:",
            link.chain_id,
            link.safe.to_ascii_lowercase(),
            link.safe_tx_hash.to_ascii_lowercase()
        )
    }

    async fn read_kept(io: &Io<Event>) -> serde_json::Map<String, Value> {
        io.load(&[KEPT_KEY])
            .await
            .get(KEPT_KEY)
            .cloned()
            .flatten()
            .and_then(|r| serde_json::from_str::<Value>(&r).ok())
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default()
    }

    async fn write_kept(io: &Io<Event>, kept: &serde_json::Map<String, Value>) {
        let value = (!kept.is_empty()).then(|| Value::Object(kept.clone()).to_string());
        io.store(KEPT_KEY, value).await;
    }

    async fn keep(io: &Io<Event>, st: &Shared<PageState>, link: &TxLink, kept: Kept) {
        let key = format!(
            "{}{}",
            kept_prefix(link),
            kept.approval.owner.to_ascii_lowercase()
        );
        let value = serde_json::to_value(&kept).unwrap_or(Value::Null);
        lock(st).unsent = Some(kept);
        let mut all = read_kept(io).await;
        all.insert(key, value);
        write_kept(io, &all).await;
    }

    async fn drop_kept(io: &Io<Event>, st: &Shared<PageState>, link: &TxLink) {
        let prefix = kept_prefix(link);
        let mut all = read_kept(io).await;
        all.retain(|k, _| !k.starts_with(&prefix));
        write_kept(io, &all).await;
        lock(st).unsent = None;
    }

    /// r5reg-12: what was signed here before and not taken is said as not
    /// sent, and sent again once, on its own; once its number is used there
    /// is nothing left to send it for.
    async fn resume(io: &Io<Event>, st: &Shared<PageState>, link: &TxLink, used: bool) {
        let prefix = kept_prefix(link);
        let all = read_kept(io).await;
        let kept = all
            .iter()
            .find(|(k, _)| k.starts_with(&prefix))
            .map(|(_, v)| v.clone());
        if used {
            let had = kept.is_some() || lock(st).unsent.is_some();
            if had {
                drop_kept(io, st, link).await;
            }
            let mut s = lock(st);
            if matches!(s.step, OutsideStep::Failed { unsent: true, .. }) {
                s.step = OutsideStep::Idle;
            }
            return;
        }
        {
            let mut s = lock(st);
            if s.resumed {
                return;
            }
            s.resumed = true;
            if s.step != OutsideStep::Idle {
                return;
            }
        }
        // Only what this page wrote reads back: anything else is left alone.
        let Some(kept) = kept.and_then(|v| serde_json::from_value::<Kept>(v).ok()) else {
            return;
        };
        {
            let mut s = lock(st);
            s.step = OutsideStep::Failed {
                what: kept.then.what(),
                problem: None,
                reason: Some(FailReason::NotSent),
                refused: None,
                unsent: true,
            };
            s.unsent = Some(kept);
        }
        send_again(io, st).await;
    }

    /// Why an action stopped.
    enum Stop {
        /// Cancelled, or another link: nothing said.
        Quiet,
        /// The wallet cannot sign typed data.
        Unsupported,
        Fail {
            problem: Option<WalletProblem>,
            reason: Option<FailReason>,
            refused: Option<RefusalKind>,
        },
    }

    impl Stop {
        fn reason(reason: FailReason) -> Self {
            Self::Fail {
                problem: None,
                reason: Some(reason),
                refused: None,
            }
        }

        fn wallet(code: Option<i64>, message: &str) -> Self {
            Self::Fail {
                problem: Some(wallet_problem(code, message)),
                reason: None,
                refused: None,
            }
        }
    }

    fn failed(st: &Shared<PageState>, run: u64, what: OutsideAction, stop: Stop) {
        let mut s = lock(st);
        if s.run != run {
            return;
        }
        s.step = match stop {
            Stop::Quiet => OutsideStep::Idle,
            Stop::Unsupported => OutsideStep::Unsupported,
            Stop::Fail {
                problem,
                reason,
                refused,
            } => {
                let unsent = matches!(
                    reason,
                    Some(FailReason::NotSent | FailReason::Refused { .. })
                ) && s.unsent.is_some();
                OutsideStep::Failed {
                    what,
                    problem,
                    reason,
                    refused,
                    unsent,
                }
            }
        };
    }

    fn loaded(st: &Shared<PageState>) -> Option<(TxLink, OutsideTxView)> {
        let s = lock(st);
        Some((s.link.clone()?, s.view_tx()?.clone()))
    }

    /// §3.5/§3.6 right before a signature or a transaction: who can act for
    /// it on this network and what `tx` changes, read again now (never a
    /// cache); a risk asks the person, every time.
    async fn gate_now(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        what: OutsideAction,
        v: &OutsideTxView,
        tx: &SafeTx,
    ) -> Result<(), Stop> {
        set_step(st, run, OutsideStep::Checking { what });
        let deployed = shell::Chain::has_code(io, v.chain_id, &v.safe)
            .await
            .map_err(|_| Stop::reason(FailReason::Unchecked))?;
        let findings = shell::judge_on(io, v.chain_id, &v.safe, v.genesis.as_ref(), deployed).await;
        let mut members = v.members.clone();
        members.deployed = deployed;
        let (review, _) =
            review_read(io, v.chain_id, &v.safe, tx, &members, v.genesis.as_ref()).await;
        if !current(st, run) {
            return Err(Stop::Quiet);
        }
        // The one gate, as a Vela member's signature gets it.
        let gate = multisig_gather::decide(v.chain_id, &v.safe, tx, findings, review.as_ref());
        use crate::multisig_review::Gate;
        match gate {
            Gate::Open => Ok(()),
            Gate::Unchecked => Err(Stop::reason(FailReason::Unchecked)),
            Gate::Refused { .. } => Err(Stop::reason(FailReason::ReadOnly)),
            Gate::Blocked => Err(Stop::reason(FailReason::Blocked)),
            Gate::Risk { findings } => {
                let answer = io
                    .ask(MultisigOperation::AskRisk {
                        chain_id: v.chain_id,
                        safe: v.safe.clone(),
                        findings,
                    })
                    .await;
                match answer {
                    MultisigShellResult::RiskAnswered { signs: true } if current(st, run) => Ok(()),
                    _ => Err(Stop::Quiet),
                }
            }
        }
    }

    /// The wallet on the transaction's network, moved there if it is not.
    async fn on_chain(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        chain_id: u64,
    ) -> Result<(), Stop> {
        let now = wallet_chain(io)
            .await
            .map_err(|(c, m)| Stop::wallet(c, &m))?;
        if now == chain_id {
            return Ok(());
        }
        let since = io.now().await;
        set_step(st, run, OutsideStep::Switching { since });
        wallet_ask(
            io,
            "wallet_switchEthereumChain",
            json!([{ "chainId": format!("0x{chain_id:x}") }]),
        )
        .await
        .map_err(|(c, m)| Stop::wallet(c, &m))?;
        if !current(st, run) {
            return Err(Stop::Quiet);
        }
        let after = wallet_chain(io)
            .await
            .map_err(|(c, m)| Stop::wallet(c, &m))?;
        if after != chain_id {
            return Err(Stop::wallet(Some(code::CHAIN_MISMATCH), "other chain"));
        }
        Ok(())
    }

    /// A verified approval of `tx` by `account`, from its wallet: checked here
    /// as Vela and the Safe will check it, never handed on otherwise.
    async fn sign(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        account: &str,
        v: &OutsideTxView,
        tx: &SafeTx,
        form: EcdsaForm,
    ) -> Result<EcdsaApproval, Stop> {
        on_chain(io, st, run, v.chain_id).await?;
        if !current(st, run) {
            return Err(Stop::Quiet);
        }
        let hash = multisig::safe_tx_hash(v.chain_id, &v.safe, tx)
            .map_err(|_| Stop::reason(FailReason::BadSignature))?;
        let since = io.now().await;
        set_step(
            st,
            run,
            OutsideStep::Wallet {
                what: OutsideAction::Approve,
                since,
                index: 1,
                total: 1,
            },
        );
        let answer = match form {
            EcdsaForm::TypedData => {
                let typed = multisig::safe_tx_typed_data(v.chain_id, &v.safe, tx)
                    .map_err(|_| Stop::reason(FailReason::BadSignature))?;
                wallet_ask(
                    io,
                    "eth_signTypedData_v4",
                    json!([account, typed.to_string()]),
                )
                .await
            }
            EcdsaForm::EthSign => wallet_ask(io, "personal_sign", json!([hash, account])).await,
        };
        if !current(st, run) {
            return Err(Stop::Quiet);
        }
        let signature = match answer {
            Ok(Value::String(s)) => s,
            Ok(_) => return Err(Stop::reason(FailReason::BadSignature)),
            Err((c, m)) => {
                if form == EcdsaForm::TypedData
                    && wallet_problem(c, &m) == WalletProblem::Unsupported
                {
                    return Err(Stop::Unsupported);
                }
                return Err(Stop::wallet(c, &m));
            }
        };
        let signature = match form {
            EcdsaForm::TypedData => signature,
            EcdsaForm::EthSign => {
                eth_sign_form(&signature).map_err(|_| Stop::reason(FailReason::BadSignature))?
            }
        };
        let checked = multisig::verify_ecdsa_approval(
            &hash,
            &EcdsaApproval {
                owner: account.to_owned(),
                signature,
            },
        )
        .map_err(|_| Stop::reason(FailReason::BadSignature))?;
        if !same_address(&checked.owner, account) {
            return Err(Stop::reason(FailReason::BadSignature));
        }
        Ok(checked)
    }

    /// Why the mailbox has not taken it.
    enum PostFail {
        NotSent,
        Refused(Value),
    }

    /// Hand a verified approval to the mailbox: kept on this device first,
    /// sent again while the mailbox is busy or away, then said. A refusal
    /// keeps it too — Send again, nothing signed twice. The multisig's setup
    /// goes with it; refused for that alone, it is sent once more without.
    async fn post(
        io: &Io<Event>,
        st: &Shared<PageState>,
        link: &TxLink,
        v: &OutsideTxView,
        safe_tx: &SafeTx,
        approval: &EcdsaApproval,
        then: KeptThen,
    ) -> Result<(), PostFail> {
        keep(
            io,
            st,
            link,
            Kept {
                safe_tx: safe_tx.clone(),
                approval: approval.clone(),
                then,
            },
        )
        .await;
        let service = lock(st).service.clone();
        let started = io.now().await;
        let mut with_genesis = v.genesis.is_some();
        let mut first: Option<Value> = None;
        let mut tries: u32 = 0;
        loop {
            lock(st).step = OutsideStep::Sending { retry_at: None };
            let genesis = if with_genesis {
                v.genesis
                    .as_ref()
                    .and_then(|g| multisig::named_genesis(g).ok())
            } else {
                None
            };
            let answer = service
                .submit_approval(
                    io,
                    &shell::Submission {
                        chain_id: v.chain_id,
                        safe: v.safe.clone(),
                        safe_tx: safe_tx.clone(),
                        approval: Approval::Ecdsa(approval.clone()),
                        genesis,
                    },
                )
                .await;
            match answer {
                Ok(shell::SubmitAnswer::Took { .. }) => {
                    drop_kept(io, st, link).await;
                    lock(st).step = then.step();
                    io.notice(shell::Notice::MailboxTook {
                        safe: v.safe.clone(),
                    })
                    .await;
                    return Ok(());
                }
                Ok(shell::SubmitAnswer::Refused { refusal }) => {
                    if with_genesis
                        && refusal.get("kind").and_then(Value::as_str) == Some("genesis")
                    {
                        first = Some(refusal);
                        with_genesis = false;
                        continue;
                    }
                    return Err(PostFail::Refused(first.unwrap_or(refusal)));
                }
                Err(fail) => {
                    let named = match fail {
                        ServiceFail::Busy(wait) => wait,
                        ServiceFail::Unavailable(_) => None,
                    };
                    let backoff =
                        (2_000.0 * 2f64.powi(i32::try_from(tries).unwrap_or(30))).min(30_000.0);
                    let wait = named.unwrap_or(backoff).max(1_000.0);
                    let now = io.now().await;
                    if now + wait > started + POST_PATIENCE_MS {
                        return Err(PostFail::NotSent);
                    }
                    lock(st).step = OutsideStep::Sending {
                        retry_at: Some(now + wait),
                    };
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    io.wait(wait as u64).await;
                    tries += 1;
                }
            }
        }
    }

    fn post_stop(fail: PostFail) -> Stop {
        match fail {
            PostFail::NotSent => Stop::reason(FailReason::NotSent),
            PostFail::Refused(r) => Stop::Fail {
                problem: None,
                reason: Some(FailReason::Refused {
                    detail: short_refusal(&r),
                }),
                refused: Some(refusal_kind(&r)),
            },
        }
    }

    async fn approve(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        account: &str,
        form: EcdsaForm,
    ) {
        let what = OutsideAction::Approve;
        let Some((link, v)) = loaded(st) else { return };
        if !v.open() {
            return;
        }
        if !v.is_owner(account) {
            return failed(st, run, what, Stop::reason(FailReason::NotMember));
        }
        if let Err(stop) = gate_now(io, st, run, what, &v, &v.tx).await {
            return failed(st, run, what, stop);
        }
        let approval = match sign(io, st, run, account, &v, &v.tx, form).await {
            Ok(a) => a,
            Err(stop) => return failed(st, run, what, stop),
        };
        let signed = match multisig::ecdsa_form(&approval.signature) {
            Ok(EcdsaForm::EthSign) => ApprovalForm::EthSign,
            _ => ApprovalForm::TypedData,
        };
        match post(
            io,
            st,
            &link,
            &v,
            &v.tx,
            &approval,
            KeptThen::Approved { form: signed },
        )
        .await
        {
            Ok(()) => refresh(io, st).await,
            // r4C-16: a mailbox that takes only typed data never will take this form.
            Err(PostFail::Refused(r)) if signed == ApprovalForm::EthSign && refused_form(&r) => {
                drop_kept(io, st, &link).await;
                failed(st, run, what, Stop::reason(FailReason::NoEthSign));
            }
            Err(fail) => failed(st, run, what, post_stop(fail)),
        }
    }

    async fn approve_on_chain(io: &Io<Event>, st: &Shared<PageState>, run: u64, account: &str) {
        let what = OutsideAction::ApproveOnChain;
        let Some((_, v)) = loaded(st) else { return };
        if !v.open() || !v.members.deployed {
            return;
        }
        if !v.is_owner(account) {
            return failed(st, run, what, Stop::reason(FailReason::NotMember));
        }
        if let Err(stop) = gate_now(io, st, run, what, &v, &v.tx).await {
            return failed(st, run, what, stop);
        }
        let call = multisig::Call {
            to: v.safe.clone(),
            value: "0x0".to_owned(),
            data: approve_hash_call(&v.item.safe_tx_hash),
        };
        match send(io, st, run, account, v.chain_id, &[call], what).await {
            Ok(_) => {
                set_step(
                    st,
                    run,
                    OutsideStep::Approved {
                        form: ApprovalForm::OnChain,
                    },
                );
                refresh(io, st).await;
            }
            Err(stop) => failed(st, run, what, stop),
        }
    }

    async fn reject(io: &Io<Event>, st: &Shared<PageState>, run: u64, account: &str) {
        let what = OutsideAction::Reject;
        let Some((link, v)) = loaded(st) else { return };
        if !v.can_reject(account) {
            if v.open() && !v.is_owner(account) {
                failed(st, run, what, Stop::reason(FailReason::NotMember));
            }
            return;
        }
        let Ok(cancel) = multisig::cancel_tx(&v.safe, &v.item.nonce) else {
            return;
        };
        if let Err(stop) = gate_now(io, st, run, what, &v, &cancel).await {
            return failed(st, run, what, stop);
        }
        let approval = match sign(io, st, run, account, &v, &cancel, EcdsaForm::TypedData).await {
            Ok(a) => a,
            Err(stop) => return failed(st, run, what, stop),
        };
        if let Err(fail) = post(io, st, &link, &v, &cancel, &approval, KeptThen::Rejected).await {
            failed(st, run, what, post_stop(fail));
        }
    }

    async fn send_again(io: &Io<Event>, st: &Shared<PageState>) {
        let (unsent, run) = {
            let s = lock(st);
            (s.unsent.clone(), s.run)
        };
        let Some(kept) = unsent else { return };
        let Some((link, v)) = loaded(st) else { return };
        match post(io, st, &link, &v, &kept.safe_tx, &kept.approval, kept.then).await {
            // Read again as its own event: a refresh is what resumes a send.
            Ok(()) => io.emit(Event::Refresh),
            Err(fail) => failed(st, run, kept.then.what(), post_stop(fail)),
        }
    }

    /// The transactions `account`'s wallet sends to execute it: whatever must
    /// be deployed first, then `execTransaction` — the approvals the members
    /// gave, the owner's own as `msg.sender` only where those fall short
    /// (r7-3). A deploy only from a genesis Verified
    /// on code read right now (§3.5).
    async fn execution_calls(
        io: &Io<Event>,
        v: &OutsideTxView,
        account: &str,
    ) -> Result<Vec<multisig::Call>, Stop> {
        let owner = v.is_owner(account);
        // A passkey member's check needs its account's code; a plain
        // account's approval is ready as it is.
        let mut others: Vec<(Approval, bool)> = Vec::new();
        for a in v
            .item
            .approvals
            .iter()
            .filter(|a| a.counts && !(owner && same_address(&a.owner, account)))
        {
            let ready = match &a.approval {
                Approval::Passkey(k) => shell::Chain::has_code(io, v.chain_id, &k.owner)
                    .await
                    .map_err(|_| Stop::reason(FailReason::Unchecked))?,
                _ => true,
            };
            others.push((a.approval.clone(), ready));
        }
        // r7-3: the approvals the members gave come first; the owner
        // approves as `msg.sender` only where those fall short.
        let (chosen, executor_approves) =
            crate::app::multisig_proposals::carried_approvals(v.members.threshold, owner, others);
        let mut deploy_owners = Vec::new();
        for (a, ready) in &chosen {
            if let (Approval::Passkey(k), false) = (a, ready) {
                deploy_owners.push(k.owner_genesis.clone());
            }
        }
        let approvals: Vec<Approval> = chosen.into_iter().map(|(a, _)| a).collect();
        let deploy_multisig = if v.members.deployed {
            None
        } else {
            v.genesis.clone()
        };
        let mut deploy_facts = None;
        if let Some(g) = &deploy_multisig {
            let facts = shell::counterfactual_facts(io, v.chain_id, &v.safe, g).await;
            match multisig::counterfactual_authority(g, &v.safe, &facts) {
                Ok(multisig::Authority::Verified { .. }) => {}
                Ok(multisig::Authority::Unknown { .. }) | Err(_) => {
                    return Err(Stop::reason(FailReason::DeployUnverified))
                }
                Ok(multisig::Authority::Refused { .. }) => {
                    return Err(Stop::reason(FailReason::DeployRefused))
                }
            }
            deploy_facts = Some(facts);
        }
        multisig::execution_calls(&multisig::ExecutionPlan {
            multisig: v.safe.clone(),
            deploy_multisig,
            deploy_owners,
            tx: v.tx.clone(),
            approvals,
            executor: executor_approves.then(|| account.to_owned()),
            deploy_facts,
        })
        .map_err(|_| Stop::reason(FailReason::DeployRefused))
    }

    /// What executing costs the wallet, estimated on the chain now.
    async fn fee(io: &Io<Event>, st: &Shared<PageState>, account: &str) -> Option<ExecutionFee> {
        let (_, v) = loaded(st)?;
        let calls = execution_calls(io, &v, account).await.ok()?;
        let price = io.rpc(v.chain_id, "eth_gasPrice", json!([])).await.ok()?;
        let price = big(Some(&price))?;
        let mut gas = U256::ZERO;
        let mut partial = false;
        let mut unpriced = 0u64;
        for call in &calls {
            let estimate = io
                .rpc(
                    v.chain_id,
                    "eth_estimateGas",
                    json!([{ "from": account, "to": call.to, "data": call.data, "value": call.value }]),
                )
                .await;
            match estimate.ok().and_then(|e| big(Some(&e))) {
                Some(g) => gas += g,
                None => {
                    partial = true;
                    unpriced += 1;
                }
            }
        }
        if gas.is_zero() {
            return None;
        }
        let ceiling =
            (gas + U256::from(unpriced) * U256::from(UNPRICED_GAS)) * price * U256::from(2);
        Some(ExecutionFee {
            wei: (gas * price).to_string(),
            count: u32::try_from(calls.len()).unwrap_or(u32::MAX),
            partial,
            max_wei: ceiling.to_string(),
        })
    }

    async fn execute(io: &Io<Event>, st: &Shared<PageState>, run: u64, account: &str) {
        let what = OutsideAction::Execute;
        let Some((_, v)) = loaded(st) else { return };
        if !v.can_execute(account) {
            return;
        }
        // The fee is said before anything is sent.
        let quoted = lock(st)
            .fee
            .as_ref()
            .is_some_and(|f| same_address(&f.account, account));
        if !quoted {
            let fee = fee(io, st, account).await;
            lock(st).fee = Some(FeeView {
                account: account.to_owned(),
                fee,
            });
            return;
        }
        // A deploy is built only from a genesis Verified on code read now;
        // then the gate reads the rest again (§3.5, §3.6).
        let calls = match execution_calls(io, &v, account).await {
            Ok(c) => c,
            Err(stop) => return failed(st, run, what, stop),
        };
        if let Err(stop) = gate_now(io, st, run, what, &v, &v.tx).await {
            return failed(st, run, what, stop);
        }
        match send(io, st, run, account, v.chain_id, &calls, what).await {
            Ok(hash) => {
                set_step(st, run, OutsideStep::Executed { tx_hash: hash });
                refresh(io, st).await;
            }
            Err(stop) => failed(st, run, what, stop),
        }
    }

    /// Send `calls` from `account`, one after another, each waited for on
    /// chain. Returns the last hash.
    async fn send(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        account: &str,
        chain_id: u64,
        calls: &[multisig::Call],
        what: OutsideAction,
    ) -> Result<String, Stop> {
        on_chain(io, st, run, chain_id).await?;
        if !current(st, run) {
            return Err(Stop::Quiet);
        }
        let total = u32::try_from(calls.len()).unwrap_or(u32::MAX);
        let mut last = String::new();
        for (i, call) in calls.iter().enumerate() {
            let since = io.now().await;
            set_step(
                st,
                run,
                OutsideStep::Wallet {
                    what,
                    since,
                    index: u32::try_from(i + 1).unwrap_or(u32::MAX),
                    total,
                },
            );
            let answer = wallet_ask(
                io,
                "eth_sendTransaction",
                json!([{ "from": account, "to": call.to, "data": call.data, "value": call.value }]),
            )
            .await;
            if !current(st, run) {
                return Err(Stop::Quiet);
            }
            let hash = match answer {
                Ok(Value::String(h)) => h,
                Ok(_) => return Err(Stop::reason(FailReason::NoTransaction)),
                Err((c, m)) => return Err(Stop::wallet(c, &m)),
            };
            let since = io.now().await;
            set_step(
                st,
                run,
                OutsideStep::Mining {
                    what,
                    since,
                    tx_hash: hash.clone(),
                },
            );
            match receipt(io, st, run, chain_id, &hash).await? {
                true => last = hash,
                false => return Err(Stop::reason(FailReason::Reverted { tx_hash: hash })),
            }
        }
        Ok(last)
    }

    /// `true` once `hash` landed and succeeded, `false` if it reverted.
    async fn receipt(
        io: &Io<Event>,
        st: &Shared<PageState>,
        run: u64,
        chain_id: u64,
        hash: &str,
    ) -> Result<bool, Stop> {
        let until = io.now().await + RECEIPT_TIMEOUT_MS;
        loop {
            if !current(st, run) {
                return Err(Stop::Quiet);
            }
            let answer = io
                .rpc(chain_id, "eth_getTransactionReceipt", json!([hash]))
                .await;
            if let Some(status) = answer
                .ok()
                .and_then(|r| r.get("status").and_then(Value::as_str).map(str::to_owned))
            {
                return Ok(shell::quantity(&status) == Some(1));
            }
            if io.now().await >= until {
                return Err(Stop::reason(FailReason::NotLanded {
                    tx_hash: hash.to_owned(),
                }));
            }
            io.wait(RECEIPT_EVERY_MS).await;
        }
    }

    // -- live sessions (the page's side of WalletPair, `bridge.ts`) ---------------

    fn listen(room: String) -> Cmd {
        Command::new(move |ctx| async move {
            let io = Io::new(ctx);
            loop {
                match io
                    .ask(MultisigOperation::PeerReceive { room: room.clone() })
                    .await
                {
                    MultisigShellResult::PeerMessage { message } => io.emit(Event::Arrived {
                        room: room.clone(),
                        message,
                    }),
                    MultisigShellResult::PeerClosed { reason } => {
                        io.emit(Event::Closed { room, reason });
                        break;
                    }
                    _ => {
                        io.emit(Event::Closed {
                            room,
                            reason: "closed".to_owned(),
                        });
                        break;
                    }
                }
            }
        })
    }

    fn open_saved(model: &Model, id: String, record: String) -> Cmd {
        flow(model, move |io, _st| async move {
            if let MultisigShellResult::PeerOpened { room, .. } = io
                .ask(MultisigOperation::PeerOpen {
                    relay: None,
                    invite: None,
                    pairing: Some(record),
                })
                .await
            {
                io.emit(Event::Opened { session: id, room });
            }
        })
    }

    async fn persist(io: &Io<Event>, st: &Shared<PageState>) {
        let raw = serde_json::to_string(&lock(st).sessions).unwrap_or_else(|_| "[]".to_owned());
        io.store(SESSIONS_KEY, Some(raw)).await;
    }

    async fn join(io: &Io<Event>, st: &Shared<PageState>, invite: &str, members: &[String]) {
        let answer = io
            .ask(MultisigOperation::PeerOpen {
                relay: None,
                invite: Some(invite.to_owned()),
                pairing: None,
            })
            .await;
        let (room, pairing) = match answer {
            MultisigShellResult::PeerOpened { room, pairing, .. } => (room, pairing),
            MultisigShellResult::PeerClosed { reason } => {
                lock(st).join_failed = Some(reason);
                return;
            }
            other => {
                lock(st).join_failed = Some(format!("{other:?}"));
                return;
            }
        };
        let id = io
            .random_hex(16)
            .await
            .unwrap_or_else(|| room.clone())
            .trim_start_matches("0x")
            .to_owned();
        {
            let mut s = lock(st);
            let created = s.clock;
            s.sessions.push(SavedSession {
                id: id.clone(),
                record: pairing.unwrap_or_else(|| invite.to_owned()),
                created,
                dapp: None,
                members: Some(members.iter().map(|m| m.to_ascii_lowercase()).collect()),
                store: SessionStore::default(),
            });
        }
        persist(io, st).await;
        io.emit(Event::Opened { session: id, room });
    }

    fn forget(s: &mut PageState, id: &str) {
        s.live.remove(id);
        s.relays.remove(id);
        s.sessions.retain(|x| x.id != id);
        s.queue.retain(|(session, _)| session != id);
        if s.connect_prompt.as_ref().is_some_and(|p| p.session == id) {
            s.connect_prompt = None;
        }
    }

    fn perform_outs(model: &Model, outs: Vec<Out>) -> Cmd {
        if outs.is_empty() {
            return render();
        }
        flow(model, move |io, st| async move {
            for out in outs {
                match out {
                    Out::Send { room, message } => {
                        let _ = io.ask(MultisigOperation::PeerSend { room, message }).await;
                    }
                    Out::Close { room, end } => {
                        let _ = io
                            .ask(MultisigOperation::PeerClose {
                                room,
                                cancel: false,
                                end,
                            })
                            .await;
                    }
                    Out::Persist => persist(&io, &st).await,
                    Out::Perform { session, id } => perform(&io, &st, &session, &id).await,
                    Out::Fallback { session, id } => fallback(&io, &st, &session, &id).await,
                    Out::Switch { session, id } => switch(&io, &st, &session, &id).await,
                    Out::Accounts { session } => connect_approved(&io, &st, &session).await,
                    Out::Read { request } => read_request(&io, &st, *request).await,
                }
            }
        })
    }

    /// The response to `id`, recorded first (core §6), and sent.
    fn respond(s: &mut PageState, session: &str, id: &str, response: Value) -> Vec<Out> {
        let room = s.live.get(session).map(|l| l.room.clone());
        if let Some(saved) = s.session_mut(session) {
            saved.set_record(id, Some(response.clone()));
            saved.store.forwarded.retain(|f| f != id);
        }
        let mut outs = vec![Out::Persist];
        if let Some(room) = room {
            outs.push(Out::Send {
                room,
                message: response,
            });
        }
        outs
    }

    async fn respond_now(
        io: &Io<Event>,
        st: &Shared<PageState>,
        session: &str,
        id: &str,
        r: Value,
    ) {
        let outs = respond(&mut lock(st), session, id, r);
        for out in outs {
            match out {
                Out::Send { room, message } => {
                    let _ = io.ask(MultisigOperation::PeerSend { room, message }).await;
                }
                Out::Persist => persist(io, st).await,
                _ => {}
            }
        }
    }

    async fn send_states(io: &Io<Event>, st: &Shared<PageState>) {
        let sends: Vec<(String, Value)> = {
            let s = lock(st);
            s.live
                .iter()
                .filter(|(id, _)| s.session(id).is_some_and(SavedSession::approved))
                .map(|(id, l)| (l.room.clone(), pair_state_event(s.pair_state(id))))
                .collect()
        };
        for (room, message) in sends {
            let _ = io.ask(MultisigOperation::PeerSend { room, message }).await;
        }
    }

    fn wallet_changed(s: &mut PageState, accounts: &[String], chain_id: Option<u64>) -> Vec<Out> {
        s.accounts = accounts.iter().map(|a| a.to_ascii_lowercase()).collect();
        if !s.accounts.is_empty() {
            s.wallet_failure = None;
        }
        if let Some(c) = chain_id {
            s.wallet_chain = Some(c);
            for saved in &mut s.sessions {
                saved
                    .store
                    .chains
                    .insert("eip155".to_owned(), format!("eip155:{c}"));
            }
        }
        let mut outs = vec![Out::Persist];
        let live: Vec<(String, String)> = s
            .live
            .iter()
            .map(|(id, l)| (id.clone(), l.room.clone()))
            .collect();
        for (id, room) in live {
            // No account left: the wallet disconnected this page; the
            // session stays, unapproved (core §4.2).
            if s.accounts.is_empty() {
                if let Some(saved) = s.session_mut(&id) {
                    saved.store.approved.clear();
                }
            }
            outs.push(Out::Send {
                room,
                message: pair_state_event(s.pair_state(&id)),
            });
        }
        outs
    }

    fn connect_declined(s: &mut PageState, session: &str) -> Vec<Out> {
        let waiters = match s.live.get_mut(session) {
            Some(l) => {
                l.declined = true;
                l.prompting = false;
                std::mem::take(&mut l.waiters)
            }
            None => Vec::new(),
        };
        let mut outs = Vec::new();
        for id in waiters {
            outs.extend(answer_connect(s, session, &id));
        }
        outs
    }

    /// A `walletpair_connect` answered with the state now (core §4.1).
    fn answer_connect(s: &mut PageState, session: &str, id: &str) -> Vec<Out> {
        if let Some(l) = s.live.get_mut(session) {
            l.answered_connects.insert(id.to_owned());
        }
        let state = s.pair_state(session);
        let Some(room) = s.live.get(session).map(|l| l.room.clone()) else {
            return Vec::new();
        };
        let message = if state.as_object().is_none_or(serde_json::Map::is_empty) {
            pair_error(id, code::USER_REJECTED, "User rejected the request")
        } else {
            pair_result(
                id,
                json!({ "wallet": { "name": wallet_label(&s.wallet_name) }, "state": state }),
            )
        };
        vec![Out::Send { room, message }]
    }

    /// The wallet's own name, never this page's (ms-13).
    fn wallet_label(name: &str) -> String {
        let n = name.trim();
        if n.is_empty() {
            "Browser wallet".to_owned()
        } else {
            n.to_owned()
        }
    }

    /// The person approved a session's connection: the wallet's own prompt,
    /// if it has not connected this page yet; then everyone waiting answered.
    async fn connect_approved(io: &Io<Event>, st: &Shared<PageState>, session: &str) {
        let accounts = wallet_ask(io, "eth_requestAccounts", json!([]))
            .await
            .map(|l| lowercase_list(&l));
        let chain = wallet_chain(io).await.ok();
        let outs = {
            let mut s = lock(st);
            match accounts {
                Ok(list) if !list.is_empty() => {
                    s.accounts = list;
                    if let Some(c) = chain {
                        s.wallet_chain = Some(c);
                        for saved in &mut s.sessions {
                            saved
                                .store
                                .chains
                                .insert("eip155".to_owned(), format!("eip155:{c}"));
                        }
                    }
                    if let Some(saved) = s.session_mut(session) {
                        if !saved.approved() {
                            saved.store.approved.push("eip155".to_owned());
                        }
                    }
                    let mut outs = Vec::new();
                    if let Some(room) = s.live.get(session).map(|l| l.room.clone()) {
                        // The state changed: the event goes before the responses (core §4.3).
                        outs.push(Out::Send {
                            room,
                            message: pair_state_event(s.pair_state(session)),
                        });
                    }
                    let waiters = s
                        .live
                        .get_mut(session)
                        .map(|l| {
                            l.prompting = false;
                            std::mem::take(&mut l.waiters)
                        })
                        .unwrap_or_default();
                    for id in waiters {
                        outs.extend(answer_connect(&mut s, session, &id));
                    }
                    outs.extend(replaced(&mut s, session));
                    outs.push(Out::Persist);
                    outs
                }
                _ => connect_declined(&mut s, session),
            }
        };
        for out in outs {
            match out {
                Out::Send { room, message } => {
                    let _ = io.ask(MultisigOperation::PeerSend { room, message }).await;
                }
                Out::Close { room, end } => {
                    let _ = io
                        .ask(MultisigOperation::PeerClose {
                            room,
                            cancel: false,
                            end,
                        })
                        .await;
                }
                Out::Persist => persist(io, st).await,
                _ => {}
            }
        }
    }

    /// `current` was just approved: an older session of the same dApp that
    /// is not connected now is one it gave up, and is ended (ms-4).
    fn replaced(s: &mut PageState, current: &str) -> Vec<Out> {
        let Some(dapp) = s.session(current).and_then(|x| x.dapp.clone()) else {
            return Vec::new();
        };
        let others: Vec<String> = s
            .sessions
            .iter()
            .filter(|o| o.id != current)
            .filter(|o| {
                o.dapp
                    .as_ref()
                    .is_some_and(|d| d.name == dapp.name && d.origin == dapp.origin)
            })
            .filter(|o| s.live.get(&o.id).is_none_or(|l| l.state != "connected"))
            .map(|o| o.id.clone())
            .collect();
        let mut outs = Vec::new();
        for id in others {
            if let Some(room) = s.live.get(&id).map(|l| l.room.clone()) {
                outs.push(Out::Close { room, end: true });
            }
            forget(s, &id);
        }
        outs
    }

    fn is_origin(v: &str) -> bool {
        let Some((scheme, rest)) = v.split_once("://") else {
            return false;
        };
        matches!(scheme, "http" | "https")
            && !rest.is_empty()
            && !rest.contains(['/', '?', '#', ' '])
            && v.len() <= 2048
    }

    /// Steps 1–9 of a Wallet's receive procedure (core §3.4), with the page's
    /// own rules: a request as anyone but the link's members is refused; a
    /// signature or a transaction is shown and held for the owner's tap.
    #[allow(clippy::too_many_lines)]
    fn arrived(s: &mut PageState, room: &str, message: &Value) -> Vec<Out> {
        let Some(session) = s.session_of_room(room) else {
            return Vec::new();
        };
        let Some(o) = message.as_object() else {
            return Vec::new();
        };
        let Some(id) = o.get("id").and_then(printable).map(str::to_owned) else {
            return Vec::new();
        };
        let to_room = |message: Value| {
            vec![Out::Send {
                room: room.to_owned(),
                message,
            }]
        };
        let invalid = |m: &str| to_room(pair_error(&id, code::INVALID_REQUEST, m));
        let Some(method) = o.get("method").and_then(printable).map(str::to_owned) else {
            return invalid("Invalid request");
        };
        let params = o.get("params").cloned().unwrap_or(Value::Null);
        if !(params.is_null() || params.is_array() || params.is_object()) {
            return invalid("Invalid request");
        }
        let origin = match o.get("origin") {
            None => None,
            Some(Value::String(v)) if is_origin(v) => Some(v.clone()),
            Some(_) => return invalid("Invalid request"),
        };
        // A Vela withdrawing a request is said, even once the wallet has it.
        if method == "walletpair_cancel" {
            let target = params
                .as_array()
                .filter(|l| l.len() == 1)
                .and_then(|l| printable(&l[0]))
                .map(str::to_owned);
            let Some(target) = target else {
                return to_room(pair_error(&id, code::INVALID_PARAMS, "Invalid params"));
            };
            let mut outs = Vec::new();
            let mut held = false;
            if let Some(entry) = s.entry_mut(&session, &target) {
                if entry.stage != Stage::Answered {
                    entry.outcome = Some(Outcome::Withdrawn);
                    held = matches!(entry.stage, Stage::Review | Stage::Fallback);
                    if held {
                        entry.stage = Stage::Answered;
                    }
                }
            }
            if held {
                s.queue.retain(|(x, y)| !(x == &session && y == &target));
                outs.extend(respond(
                    s,
                    &session,
                    &target,
                    pair_error(&target, code::USER_REJECTED, "User rejected the request"),
                ));
            }
            outs.extend(to_room(pair_result(&id, Value::Null)));
            return outs;
        }
        if let Some(recorded) = s.session(&session).and_then(|x| x.record(&id).cloned()) {
            return recorded.map(to_room).unwrap_or_default();
        }
        if method == "walletpair_connect" {
            return walletpair_connect(s, &session, &id, &params, origin);
        }
        let Some(saved) = s.session(&session).cloned() else {
            return Vec::new();
        };
        let state = s.pair_state(&session);
        let Some(active) = parse_state(&state).flatten() else {
            return to_room(pair_error(&id, code::UNAUTHORIZED, "Not connected"));
        };
        if !matches!(
            method.as_str(),
            "eth_signTypedData_v4" | "eth_sendTransaction" | "wallet_switchEthereumChain"
        ) {
            return to_room(pair_error(&id, code::UNSUPPORTED, "Unsupported method"));
        }
        if let Some((c, m)) = check(&method, &params, &active) {
            return to_room(pair_error(&id, c, &m));
        }
        let account = signing_account(&method, &params);
        let mut entry = ActivityEntry {
            id: id.clone(),
            at: s.clock,
            session: session.clone(),
            origin,
            method: method.clone(),
            account: account.clone(),
            safe_tx: None,
            send: None,
            stage: Stage::Review,
            wallet_at: None,
            outcome: None,
            problem: None,
            form: None,
            result: None,
            params: params.clone(),
        };
        let list = params.as_array().cloned().unwrap_or_default();
        if method == "eth_signTypedData_v4" {
            entry.safe_tx = list.get(1).and_then(read_safe_tx_request).map(|mut r| {
                r.genesis = genesis_for(&r.safe, o.get("genesis"));
                r
            });
        }
        if method == "eth_sendTransaction" {
            let t = list.first().cloned().unwrap_or(Value::Null);
            entry.send = Some(SendSummary {
                to: t.get("to").and_then(Value::as_str).unwrap_or("").to_owned(),
                data: t
                    .get("data")
                    .and_then(Value::as_str)
                    .unwrap_or("0x")
                    .to_owned(),
                value: t
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or("0x0")
                    .to_owned(),
            });
        }
        if let Some(saved) = s.session_mut(&session) {
            saved.set_record(&id, None);
        }
        let refused = account
            .as_deref()
            .is_some_and(|a| !expected(saved.members.as_deref().unwrap_or(&[]), a));
        let hash = entry.safe_tx.as_ref().and_then(|r| {
            multisig::safe_tx_hash(r.chain_id, &r.safe, &r.tx)
                .ok()
                .map(|h| h.to_ascii_lowercase())
        });
        s.activity.insert(0, entry);
        s.activity.truncate(50);
        let mut outs = vec![Out::Persist];
        if refused {
            if let Some(e) = s.entry_mut(&session, &id) {
                e.stage = Stage::Answered;
                e.outcome = Some(Outcome::Rejected);
            }
            outs.extend(respond(
                s,
                &session,
                &id,
                pair_error(&id, code::USER_REJECTED, "User rejected the request"),
            ));
            return outs;
        }
        if method == "wallet_switchEthereumChain" {
            let clock = s.clock;
            if let Some(e) = s.entry_mut(&session, &id) {
                e.stage = Stage::Wallet;
                e.wallet_at = Some(clock);
            }
            outs.push(Out::Switch { session, id });
            return outs;
        }
        // r4C-15: Vela asking again for the same transaction replaces what it
        // asked before, which nobody waits on any more.
        if let Some(hash) = hash {
            let stale: Vec<String> = s
                .queue
                .iter()
                .filter(|(x, _)| x == &session)
                .filter(|(x, y)| {
                    s.activity
                        .iter()
                        .find(|a| &a.session == x && &a.id == y)
                        .and_then(|a| a.safe_tx.as_ref())
                        .and_then(|r| multisig::safe_tx_hash(r.chain_id, &r.safe, &r.tx).ok())
                        .is_some_and(|h| h.eq_ignore_ascii_case(&hash))
                })
                .map(|(_, y)| y.clone())
                .collect();
            for old in stale {
                s.queue.retain(|(x, y)| !(x == &session && y == &old));
                if let Some(e) = s.entry_mut(&session, &old) {
                    e.stage = Stage::Answered;
                    e.outcome = Some(Outcome::Withdrawn);
                }
                outs.extend(respond(
                    s,
                    &session,
                    &old,
                    pair_error(&old, code::USER_REJECTED, "User rejected the request"),
                ));
            }
        }
        if let Some(request) = s.entry_mut(&session, &id).and_then(|e| e.safe_tx.clone()) {
            outs.push(Out::Read {
                request: Box::new(request),
            });
        }
        s.queue.push((session, id));
        outs
    }

    /// The network's own check of a request (spec networks/evm.md): invalid
    /// params, an account that is not approved, another chain.
    fn check(method: &str, params: &Value, state: &PairState) -> Option<(i64, String)> {
        let invalid = |m: &str| Some((code::INVALID_PARAMS, m.to_owned()));
        let approved = |a: &str| state.accounts.iter().any(|x| x.eq_ignore_ascii_case(a));
        let active = chain_id_of(&state.chain);
        let single = params
            .as_array()
            .filter(|l| l.len() == 1)
            .and_then(|l| l[0].as_object());
        let quantity = |v: &Value| {
            v.as_str().is_some_and(|s| {
                s.strip_prefix("0x").is_some_and(|h| {
                    !h.is_empty()
                        && h.len() <= 64
                        && (h == "0" || !h.starts_with('0'))
                        && h.bytes().all(|b| b.is_ascii_hexdigit())
                })
            })
        };
        match method {
            "wallet_switchEthereumChain" => {
                let ok = single
                    .and_then(|o| o.get("chainId"))
                    .is_some_and(|c| quantity(c) && c != "0x0");
                if ok {
                    None
                } else {
                    invalid("Invalid params")
                }
            }
            "eth_signTypedData_v4" => {
                let Some(list) = params.as_array().filter(|l| l.len() == 2) else {
                    return invalid("Invalid params");
                };
                let Some(account) = list[0].as_str().filter(|a| is_address(a)) else {
                    return invalid("Invalid params");
                };
                let Some(text) = list[1].as_str() else {
                    return invalid("typedData must be a JSON string");
                };
                let Ok(typed) = serde_json::from_str::<Value>(text) else {
                    return invalid("typedData is not JSON");
                };
                if !typed.get("types").is_some_and(Value::is_object)
                    || !typed.get("primaryType").is_some_and(Value::is_string)
                {
                    return invalid("Invalid params");
                }
                if !typed["types"]
                    .get("EIP712Domain")
                    .is_some_and(Value::is_array)
                {
                    return invalid("types must include EIP712Domain");
                }
                if !typed.get("domain").is_some_and(Value::is_object)
                    || !typed.get("message").is_some_and(Value::is_object)
                {
                    return invalid("Invalid params");
                }
                if !approved(account) {
                    return Some((code::UNAUTHORIZED, "Account is not approved".to_owned()));
                }
                if let Some(c) = typed["domain"].get("chainId") {
                    let Some(c) = big(Some(c)).and_then(|c| u64::try_from(c).ok()) else {
                        return invalid("Invalid domain.chainId");
                    };
                    if Some(c) != active {
                        return Some((code::CHAIN_MISMATCH, "Chain is not active".to_owned()));
                    }
                }
                None
            }
            "eth_sendTransaction" => {
                let Some(tx) = single else {
                    return invalid("Invalid params");
                };
                let Some(from) = tx
                    .get("from")
                    .and_then(Value::as_str)
                    .filter(|a| is_address(a))
                else {
                    return invalid("Invalid params");
                };
                if tx
                    .get("to")
                    .is_some_and(|t| !t.as_str().is_some_and(is_address))
                {
                    return invalid("Invalid params");
                }
                for field in ["value", "gas", "gasPrice", "nonce", "chainId"] {
                    if tx.get(field).is_some_and(|v| !quantity(v)) {
                        return invalid(&format!("Invalid {field}"));
                    }
                }
                if !approved(from) {
                    return Some((code::UNAUTHORIZED, "Account is not approved".to_owned()));
                }
                if let Some(c) = tx.get("chainId") {
                    if big(Some(c)).and_then(|c| u64::try_from(c).ok()) != active {
                        return Some((code::CHAIN_MISMATCH, "Chain is not active".to_owned()));
                    }
                }
                None
            }
            _ => Some((code::UNSUPPORTED, "Unsupported method".to_owned())),
        }
    }

    fn walletpair_connect(
        s: &mut PageState,
        session: &str,
        id: &str,
        params: &Value,
        origin: Option<String>,
    ) -> Vec<Out> {
        let room = s
            .live
            .get(session)
            .map(|l| l.room.clone())
            .unwrap_or_default();
        let send = |message: Value| {
            vec![Out::Send {
                room: room.clone(),
                message,
            }]
        };
        let input = params
            .as_array()
            .filter(|l| l.len() == 1)
            .and_then(|l| l[0].as_object());
        let parsed = input.and_then(|o| {
            let name = o.get("name").filter(|n| is_name(n))?.as_str()?.to_owned();
            let namespaces: Vec<&str> = o
                .get("namespaces")?
                .as_array()?
                .iter()
                .map(Value::as_str)
                .collect::<Option<Vec<_>>>()?;
            if namespaces.is_empty() || namespaces.len() > 16 {
                return None;
            }
            let icon = o.get("icon").and_then(Value::as_str).map(str::to_owned);
            Some((name, namespaces.contains(&"eip155"), icon))
        });
        let Some((name, eip155, icon)) = parsed else {
            return send(pair_error(id, code::INVALID_PARAMS, "Invalid params"));
        };
        if !eip155 {
            return send(pair_error(
                id,
                code::UNSUPPORTED,
                "No requested namespace is supported",
            ));
        }
        let approved = s.session(session).is_some_and(SavedSession::approved);
        let repeated = s
            .live
            .get(session)
            .is_some_and(|l| l.answered_connects.contains(id));
        if (approved && !s.accounts.is_empty()) || repeated {
            return answer_connect(s, session, id);
        }
        let dapp = DappInfo { name, origin, icon };
        if let Some(saved) = s.session_mut(session) {
            saved.dapp = Some(dapp.clone());
        }
        let Some(live) = s.live.get_mut(session) else {
            return Vec::new();
        };
        live.waiters.push(id.to_owned());
        if !live.prompting {
            live.prompting = true;
            s.connect_prompt = Some(ConnectPrompt {
                session: session.to_owned(),
                dapp,
            });
        }
        vec![Out::Persist]
    }

    fn answer_request(s: &mut PageState, session: &str, id: &str, go: bool) -> Vec<Out> {
        let at = s.queue.iter().position(|(x, y)| x == session && y == id);
        let Some(at) = at else { return Vec::new() };
        // §12: a multisig transaction is handed to the wallet only once its
        // review is read and its sign rule allows it — never blind. A tap
        // before that changes nothing (the request stays asked).
        if go && !request_signable(s, session, id) {
            return Vec::new();
        }
        s.queue.remove(at);
        let clock = s.clock;
        let Some(entry) = s.entry_mut(session, id) else {
            return Vec::new();
        };
        let fallback = entry.stage == Stage::Fallback;
        if !go {
            entry.stage = Stage::Answered;
            entry.outcome.get_or_insert(Outcome::Rejected);
            let message = if fallback {
                "Declined on the connect page"
            } else {
                "User rejected the request"
            };
            return respond(s, session, id, pair_error(id, code::USER_REJECTED, message));
        }
        entry.stage = Stage::Wallet;
        entry.wallet_at = Some(clock);
        if let Some(saved) = s.session_mut(session) {
            saved.store.forwarded.push(id.to_owned());
        }
        let next = if fallback {
            Out::Fallback {
                session: session.to_owned(),
                id: id.to_owned(),
            }
        } else {
            Out::Perform {
                session: session.to_owned(),
                id: id.to_owned(),
            }
        };
        vec![Out::Persist, next]
    }

    /// §12: a request that is not a multisig transaction, or one whose
    /// review is `ready` and allows it ([`review_state::door`]).
    fn request_signable(s: &PageState, session: &str, id: &str) -> bool {
        let Some(entry) = s
            .activity
            .iter()
            .find(|a| a.session == session && a.id == id)
        else {
            return false;
        };
        let Some(t) = entry.safe_tx.as_ref() else {
            return true;
        };
        multisig::safe_tx_hash(t.chain_id, &t.safe, &t.tx)
            .ok()
            .and_then(|h| s.request_reads.get(&h.to_ascii_lowercase()))
            .and_then(|r| r.shown.as_ref())
            .is_some_and(|x| review_state::door(Some(&x.review_state), x.review.as_ref()).enabled)
    }

    /// What a request needs to be performed.
    fn held(s: &PageState, session: &str, id: &str) -> Option<(String, Value, Option<String>)> {
        let entry = s
            .activity
            .iter()
            .find(|a| a.session == session && a.id == id)?;
        let chain = s.session(session)?.chain().cloned();
        Some((entry.method.clone(), entry.params.clone(), chain))
    }

    /// Hand a request the owner tapped to the wallet: performed against the
    /// network the session saw — another one is not acted on (core §8).
    async fn perform(io: &Io<Event>, st: &Shared<PageState>, session: &str, id: &str) {
        let Some((method, params, chain)) = held(&lock(st), session, id) else {
            return;
        };
        let now_chain = wallet_chain(io).await.ok().map(|c| format!("eip155:{c}"));
        if now_chain.is_none() || now_chain != chain {
            let r = {
                let mut s = lock(st);
                if let Some(e) = s.entry_mut(session, id) {
                    e.stage = Stage::Answered;
                    e.problem = Some(WalletProblem::OtherChain);
                    e.outcome.get_or_insert(Outcome::Failed);
                }
                pair_error(id, code::CHAIN_MISMATCH, "The wallet is on another chain")
            };
            return respond_now(io, st, session, id, r).await;
        }
        let answer = wallet_ask(io, &method, params).await;
        let response = {
            let mut s = lock(st);
            let Some(e) = s.entry_mut(session, id) else {
                return;
            };
            match answer {
                Ok(result) => {
                    e.stage = Stage::Answered;
                    e.outcome.get_or_insert(Outcome::Done);
                    e.result = result.as_str().map(str::to_owned);
                    if method == "eth_signTypedData_v4" {
                        e.form.get_or_insert(EcdsaForm::TypedData);
                    }
                    pair_result(id, result)
                }
                Err((c, m)) => {
                    let problem = wallet_problem(c, &m);
                    // r4C-16: no typed data in the wallet — the hash itself, on a tap.
                    if method == "eth_signTypedData_v4"
                        && problem == WalletProblem::Unsupported
                        && e.safe_tx.is_some()
                    {
                        e.stage = Stage::Fallback;
                        s.queue.push((session.to_owned(), id.to_owned()));
                        return;
                    }
                    e.stage = Stage::Answered;
                    e.outcome
                        .get_or_insert(if problem == WalletProblem::Declined {
                            Outcome::Rejected
                        } else {
                            Outcome::Failed
                        });
                    e.problem = Some(problem);
                    pair_error(id, c.unwrap_or(code::INTERNAL), &m)
                }
            }
        };
        respond_now(io, st, session, id, response).await;
    }

    /// Safe's `eth_sign` form, on the owner's tap: `personal_sign` over the
    /// safeTxHash, `v + 4` — checked on Vela's side as any answer.
    async fn fallback(io: &Io<Event>, st: &Shared<PageState>, session: &str, id: &str) {
        let (request, account) = {
            let s = lock(st);
            let Some(e) = s
                .activity
                .iter()
                .find(|a| a.session == session && a.id == id)
            else {
                return;
            };
            (e.safe_tx.clone(), e.account.clone())
        };
        let Some(request) = request else { return };
        let Ok(hash) = multisig::safe_tx_hash(request.chain_id, &request.safe, &request.tx) else {
            return;
        };
        let answer = wallet_ask(io, "personal_sign", json!([hash, account])).await;
        let response = {
            let mut s = lock(st);
            let Some(e) = s.entry_mut(session, id) else {
                return;
            };
            match answer.map(|r| r.as_str().map(eth_sign_form)) {
                Ok(Some(Ok(sig))) => {
                    e.stage = Stage::Answered;
                    e.outcome.get_or_insert(Outcome::Done);
                    e.form = Some(EcdsaForm::EthSign);
                    e.result = Some(sig.clone());
                    pair_result(id, json!(sig))
                }
                Ok(_) => {
                    e.stage = Stage::Answered;
                    e.outcome.get_or_insert(Outcome::Failed);
                    pair_error(id, code::INTERNAL, "No signature")
                }
                Err((c, m)) => {
                    let problem = wallet_problem(c, &m);
                    e.stage = Stage::Answered;
                    e.outcome
                        .get_or_insert(if problem == WalletProblem::Declined {
                            Outcome::Rejected
                        } else {
                            Outcome::Failed
                        });
                    e.problem = Some(problem);
                    pair_error(id, c.unwrap_or(code::INTERNAL), &m)
                }
            }
        };
        respond_now(io, st, session, id, response).await;
    }

    /// A network switch goes straight to the wallet, which asks itself.
    async fn switch(io: &Io<Event>, st: &Shared<PageState>, session: &str, id: &str) {
        let Some((method, params, _)) = held(&lock(st), session, id) else {
            return;
        };
        let answer = wallet_ask(io, &method, params).await;
        let chain = wallet_chain(io).await.ok();
        let response = {
            let mut s = lock(st);
            if let Some(c) = chain {
                s.wallet_chain = Some(c);
                for saved in &mut s.sessions {
                    saved
                        .store
                        .chains
                        .insert("eip155".to_owned(), format!("eip155:{c}"));
                }
            }
            let Some(e) = s.entry_mut(session, id) else {
                return;
            };
            e.stage = Stage::Answered;
            match answer {
                Ok(_) => {
                    e.outcome.get_or_insert(Outcome::Done);
                    pair_result(id, Value::Null)
                }
                Err((c, m)) => {
                    let problem = wallet_problem(c, &m);
                    e.outcome
                        .get_or_insert(if problem == WalletProblem::Declined {
                            Outcome::Rejected
                        } else {
                            Outcome::Failed
                        });
                    e.problem = Some(problem);
                    pair_error(id, c.unwrap_or(code::INTERNAL), &m)
                }
            }
        };
        // The state event goes before the response (core §4.3).
        send_states(io, st).await;
        respond_now(io, st, session, id, response).await;
    }
}

// ===========================================================================
// Vela's side of a live session
// ===========================================================================

/// Vela's side of a live session with an outside owner's wallet page
/// (WalletPair's DApp side, `session.svelte.ts` with the sheet's rules).
pub mod session {
    use super::relay::RelayStatus;
    use super::*;

    /// A wallet still silent after this long is said to be waiting, maybe in
    /// another window (r4C-3).
    pub const STILL_WAITING_MS: f64 = 20_000.0;
    /// How often the mailbox is looked at for an approval made from the link
    /// alone (r4C-2): the shell sends [`Event::MailboxTick`] this often.
    pub const MAILBOX_EVERY_MS: f64 = 8_000.0;
    const KEEP_PAIRINGS: usize = 8;
    /// The name the connect page once gave itself: no wallet's (ms-13).
    const PAGE_NAME: &str = "Vela connect";

    /// A transaction the outside wallet sends (an execution from Vela with no
    /// Vela account here, §11.2).
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSendTx"))]
    pub struct SendTx {
        pub from: String,
        pub to: String,
        pub data: String,
        #[serde(default = "zero")]
        pub value: String,
    }

    fn zero() -> String {
        "0x0".to_owned()
    }

    /// What Vela asks of an outside owner's wallet.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionRequest"))]
    pub struct SessionRequest {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        pub chain_id: u64,
        pub safe: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        pub tx: SafeTx,
        pub safe_tx_hash: String,
        /// Who may sign: the plain-account owners that have not approved
        /// ([`plain_accounts`]).
        pub owners: Vec<String>,
        /// Send this transaction instead of signing (`None`: an approval).
        #[serde(default)]
        pub send: Option<SendTx>,
        /// r12: the multisig's genesis, sent beside the signing request so
        /// the page can review a multisig no service records and no chain
        /// holds yet (it checks the address derives from it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "bindings", ts(type = "unknown", optional))]
        pub genesis: Option<Genesis>,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    #[serde(tag = "type", rename_all = "snake_case")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionEvent"))]
    pub enum Event {
        /// The sheet opened for `request`: a pairing kept before for one of
        /// its owners is reopened, else a new room. `origin`: Vela's own,
        /// named to the page.
        Start {
            request: Box<SessionRequest>,
            service_url: String,
            #[serde(default)]
            relay: Option<String>,
            #[serde(default)]
            origin: Option<String>,
        },
        /// The room's connection state (`offline`, `waiting`, `connected`,
        /// `closed`), from the shell's transport.
        PeerState { room: String, state: String },
        /// The room paired: the record that reopens it.
        Paired { room: String, pairing: String },
        /// This side's way onto the relay, from the shell's transport.
        Relay { status: RelayStatus },
        /// Every [`MAILBOX_EVERY_MS`]: an approval from the link alone ends it.
        MailboxTick,
        /// Withdraw the wallet's ask and make it again (r4C-3).
        AskAgain,
        /// Stop waiting on the wallet: the page is told; the sheet stays.
        StopWaiting,
        /// r4C-6: a new room — another wallet (`unpair`: the pairing ended).
        AnotherWallet {
            #[serde(default)]
            unpair: bool,
        },
        /// The sheet closed: the ask withdrawn, the pairing kept.
        Close,
        #[serde(skip)]
        Opened {
            room: String,
            invite: Option<String>,
            pairing: Option<String>,
        },
        #[serde(skip)]
        OpenFailed { reason: String },
        #[serde(skip)]
        Arrived { room: String, message: Value },
        #[serde(skip)]
        Closed { room: String, reason: String },
        #[serde(skip)]
        Changed,
    }

    /// Where the session stands, for the sheet to say.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionPhase"))]
    pub enum Phase {
        /// The room is open; the page has not joined it yet.
        #[default]
        Waiting,
        /// The page is here; the owner is approving the connection in it.
        Connecting,
        /// Connected, but none of its accounts may sign.
        WrongAccount,
        /// Asking the wallet to switch to the transaction's network.
        Switching,
        /// The wallet is asking the owner.
        Confirming,
        Done,
    }

    /// Why the ask ended without an answer, said with what to do.
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionFailure"))]
    pub enum Failure {
        /// ms-12: the connection was declined on the page.
        ConnectDeclined,
        WalletDeclined,
        /// ms-26: the wallet stayed on another network.
        OtherChain,
        /// The answer is not an owner's for this transaction.
        BadSignature,
        /// The wallet answered nothing usable.
        NoAnswer,
        /// The person stopped waiting.
        StoppedWaiting,
        /// The room could not be opened.
        NoRoom {
            reason: String,
        },
        Wallet {
            problem: WalletProblem,
        },
    }

    /// What the ask came to.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionOutcome"))]
    pub enum SessionOutcome {
        /// Verified here: the owner's, for this safeTxHash.
        Approved {
            #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
            approval: EcdsaApproval,
        },
        /// The wallet sent it (followed on chain by whoever asked).
        Sent { tx_hash: String },
        /// The sheet closed.
        Cancelled,
    }

    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    #[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSessionView"))]
    pub struct OutsideSessionView {
        pub open: bool,
        /// A pairing kept before: the page only needs reopening.
        pub resumed: bool,
        /// The new room's invite (for the link and the code); `None` for a
        /// room paired before. The shell builds the connect link from it,
        /// `members` and `tx` with `multisig_link`.
        pub invite: Option<String>,
        /// The members who may sign, lowercase.
        pub members: Vec<String>,
        pub tx: Option<TxLink>,
        pub phase: Phase,
        /// When the phase began (ms), for "for 0:23".
        pub since: f64,
        /// ms-15: the page went away mid-request.
        pub away: bool,
        pub relay: RelayStatus,
        /// The accounts the page reports, lowercase.
        pub accounts: Vec<String>,
        pub chain: Option<String>,
        /// The wallet's own name, when it gave one.
        pub wallet: Option<String>,
        /// r4C-6: "Paired with 0x…".
        pub paired_account: Option<String>,
        pub failure: Option<Failure>,
        pub outcome: Option<SessionOutcome>,
    }

    impl OutsideSessionView {
        /// r4C-3: long enough in the wallet to say where to look.
        pub fn stalled(&self, now: f64) -> bool {
            self.phase == Phase::Confirming
                && !self.away
                && self.failure.is_none()
                && now - self.since > STILL_WAITING_MS
        }
    }

    /// A pairing, as the web keeps it (`vela.multisig.externalWallets.v1`).
    #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Saved {
        id: String,
        record: String,
        #[serde(default)]
        accounts: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wallet: Option<String>,
        #[serde(default, serialize_with = "shell::int_ms")]
        saved_ms: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        Connect,
        Switch,
        Sign,
        Send,
        Cancel,
    }

    #[derive(Default)]
    pub struct SessionState {
        service: Service,
        request: Option<SessionRequest>,
        relay_url: Option<String>,
        origin: Option<String>,
        saved: Saved,
        resumed: bool,
        invite: Option<String>,
        room: Option<String>,
        connected: bool,
        was_connected: bool,
        approved: bool,
        accounts: Vec<String>,
        chain: Option<String>,
        wallet: Option<String>,
        phase: Phase,
        phase_seq: u64,
        since: f64,
        away: bool,
        relay: RelayStatus,
        tag: String,
        next_id: u64,
        pending: BTreeMap<String, (Call, Value)>,
        cancelled: BTreeSet<String>,
        switched: bool,
        failure: Option<Failure>,
        outcome: Option<SessionOutcome>,
    }

    impl SessionState {
        fn to(&mut self, phase: Phase) {
            if self.phase != phase {
                self.phase = phase;
                self.phase_seq += 1;
            }
        }

        fn finished(&self) -> bool {
            self.outcome.is_some()
        }

        fn signers(&self) -> Vec<String> {
            let owners = self
                .request
                .as_ref()
                .map(|r| r.owners.clone())
                .unwrap_or_default();
            self.accounts
                .iter()
                .filter(|a| owners.iter().any(|o| same_address(o, a)))
                .cloned()
                .collect()
        }

        fn next_request(&mut self, call: Call, method: &str, params: Value) -> Option<Out> {
            let room = self.room.clone()?;
            self.next_id += 1;
            let id = format!("{}-{}", self.tag, self.next_id);
            let message = pair_request(&id, method, params, self.origin.as_deref());
            self.pending.insert(id, (call, message.clone()));
            Some(Out::Send { room, message })
        }

        /// Withdraw what is asked (`walletpair_cancel`): its late answer is not
        /// waited on (a valid signature is still welcome).
        fn withdraw(&mut self) -> Vec<Out> {
            let asked: Vec<String> = self
                .pending
                .iter()
                .filter(|(_, (c, _))| *c != Call::Cancel)
                .map(|(id, _)| id.clone())
                .collect();
            let mut outs = Vec::new();
            for id in asked {
                if let Some((call, _)) = self.pending.remove(&id) {
                    if call == Call::Sign || call == Call::Send {
                        self.cancelled.insert(id.clone());
                    }
                }
                if let Some(out) = self.next_request(Call::Cancel, "walletpair_cancel", json!([id]))
                {
                    outs.push(out);
                }
            }
            outs
        }

        fn fail(&mut self, failure: Failure) {
            if !self.finished() {
                self.failure = Some(failure);
            }
        }

        /// What to ask next: connect, find the owner, switch, then sign.
        fn advance(&mut self) -> Vec<Out> {
            if self.finished() || self.failure.is_some() || self.room.is_none() {
                return Vec::new();
            }
            if self
                .pending
                .values()
                .any(|(c, _)| matches!(c, Call::Connect | Call::Switch | Call::Sign | Call::Send))
            {
                return Vec::new();
            }
            let Some(request) = self.request.clone() else {
                return Vec::new();
            };
            if !self.approved {
                let mut dapp = json!({ "name": "Vela", "namespaces": ["eip155"] });
                if let Some(o) = &self.origin {
                    dapp["url"] = json!(o);
                }
                return self
                    .next_request(Call::Connect, "walletpair_connect", json!([dapp]))
                    .into_iter()
                    .collect();
            }
            let owner = match &request.send {
                Some(send) => self
                    .signers_among(std::slice::from_ref(&send.from))
                    .into_iter()
                    .next(),
                None => self.signers().into_iter().next(),
            };
            let Some(owner) = owner else {
                self.to(Phase::WrongAccount);
                return Vec::new();
            };
            let target = format!("eip155:{}", request.chain_id);
            if self.chain.as_deref() != Some(target.as_str()) {
                if self.switched {
                    self.fail(Failure::OtherChain);
                    return Vec::new();
                }
                self.switched = true;
                self.to(Phase::Switching);
                return self
                    .next_request(
                        Call::Switch,
                        "wallet_switchEthereumChain",
                        json!([{ "chainId": format!("0x{:x}", request.chain_id) }]),
                    )
                    .into_iter()
                    .collect();
            }
            self.to(Phase::Confirming);
            if let Some(send) = &request.send {
                return self
                    .next_request(
                        Call::Send,
                        "eth_sendTransaction",
                        json!([{ "from": owner.to_ascii_lowercase(), "to": send.to, "data": send.data, "value": send.value }]),
                    )
                    .into_iter()
                    .collect();
            }
            let Ok(typed) =
                multisig::safe_tx_typed_data(request.chain_id, &request.safe, &request.tx)
            else {
                self.fail(Failure::BadSignature);
                return Vec::new();
            };
            let genesis = request
                .genesis
                .as_ref()
                .and_then(|g| serde_json::to_value(g).ok());
            let mut out = self.next_request(
                Call::Sign,
                "eth_signTypedData_v4",
                json!([owner, typed.to_string()]),
            );
            // r12: beside the request, the multisig's genesis (the page
            // derives the address from it before it believes a word).
            if let (Some(Out::Send { message, .. }), Some(g)) = (out.as_mut(), genesis) {
                message["genesis"] = g;
                if let Some(id) = message.get("id").and_then(Value::as_str).map(str::to_owned) {
                    if let Some((_, kept)) = self.pending.get_mut(&id) {
                        *kept = message.clone();
                    }
                }
            }
            out.into_iter().collect()
        }

        fn signers_among(&self, list: &[String]) -> Vec<String> {
            self.accounts
                .iter()
                .filter(|a| list.iter().any(|o| same_address(o, a)))
                .cloned()
                .collect()
        }

        /// The approval is in: done, the ask withdrawn, the room left — the
        /// pairing stays for next time.
        fn finish(&mut self, outcome: SessionOutcome) -> Vec<Out> {
            if self.finished() {
                return Vec::new();
            }
            self.outcome = Some(outcome);
            self.failure = None;
            self.to(Phase::Done);
            let mut outs = self.withdraw();
            outs.push(Out::Save);
            if let Some(room) = self.room.take() {
                outs.push(Out::Close { room, end: false });
            }
            outs
        }

        fn apply_state(&mut self, state: Option<PairState>) {
            match state {
                Some(p) => {
                    self.accounts = p.accounts.iter().map(|a| a.to_ascii_lowercase()).collect();
                    self.chain = Some(p.chain);
                    self.approved = true;
                }
                None => {
                    self.accounts.clear();
                    self.chain = None;
                    self.approved = false;
                }
            }
            if self.approved && self.phase == Phase::Waiting {
                self.to(Phase::Connecting);
            }
        }

        fn arrived(&mut self, room: &str, message: &Value) -> Vec<Out> {
            if self.room.as_deref() != Some(room) {
                return Vec::new();
            }
            match parse_from_wallet(message) {
                FromWallet::Ignore => Vec::new(),
                FromWallet::Event { event, data } => {
                    if event != "state" {
                        return Vec::new();
                    }
                    let Some(state) = parse_state(&data) else {
                        return Vec::new();
                    };
                    self.apply_state(state);
                    let mut outs = vec![Out::Save];
                    if self.phase == Phase::WrongAccount {
                        outs.extend(self.advance());
                    }
                    outs
                }
                FromWallet::Response { id, answer } => self.answered(&id, answer),
            }
        }

        fn answered(&mut self, id: &str, answer: Result<Value, (i64, String)>) -> Vec<Out> {
            let late = self.cancelled.remove(id);
            let Some((call, _)) = self.pending.remove(id) else {
                if late {
                    // Withdrawn: only a valid approval is still welcome.
                    if let Ok(Value::String(sig)) = &answer {
                        return self.signed(sig, true);
                    }
                }
                return Vec::new();
            };
            match (call, answer) {
                (Call::Cancel, _) => Vec::new(),
                (Call::Connect, Ok(result)) => {
                    let wallet = result
                        .get("wallet")
                        .and_then(|w| w.get("name"))
                        .filter(|n| is_name(n))
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let Some(state) = result.get("state").and_then(parse_state) else {
                        self.fail(Failure::NoAnswer);
                        return Vec::new();
                    };
                    if let Some(w) = wallet.filter(|w| w != PAGE_NAME) {
                        self.wallet = Some(w);
                    }
                    self.apply_state(state);
                    let mut outs = vec![Out::Save];
                    outs.extend(self.advance());
                    outs
                }
                (Call::Connect, Err((code, message))) => {
                    self.approved = false;
                    if code == code::USER_REJECTED {
                        self.fail(Failure::ConnectDeclined);
                    } else {
                        self.fail(Failure::Wallet {
                            problem: wallet_problem(Some(code), &message),
                        });
                    }
                    Vec::new()
                }
                (Call::Switch, Ok(_)) => {
                    let target = self
                        .request
                        .as_ref()
                        .map(|r| format!("eip155:{}", r.chain_id));
                    if self.chain != target {
                        self.fail(Failure::OtherChain);
                        return Vec::new();
                    }
                    self.advance()
                }
                (Call::Sign, Ok(Value::String(sig))) => self.signed(&sig, false),
                (Call::Send, Ok(Value::String(hash))) => {
                    self.finish(SessionOutcome::Sent { tx_hash: hash })
                }
                (Call::Sign | Call::Send, Ok(_)) => {
                    self.fail(Failure::NoAnswer);
                    Vec::new()
                }
                (Call::Switch | Call::Sign | Call::Send, Err((code, message))) => {
                    if code == code::USER_REJECTED {
                        let connecting = matches!(self.phase, Phase::Waiting | Phase::Connecting);
                        self.fail(if connecting {
                            Failure::ConnectDeclined
                        } else {
                            Failure::WalletDeclined
                        });
                    } else if code == code::CHAIN_MISMATCH {
                        self.fail(Failure::OtherChain);
                    } else {
                        self.fail(Failure::Wallet {
                            problem: wallet_problem(Some(code), &message),
                        });
                    }
                    Vec::new()
                }
            }
        }

        /// The wallet's signature, checked here before anything is done with
        /// it: the owner's, for this safeTxHash — else said, never used.
        fn signed(&mut self, signature: &str, late: bool) -> Vec<Out> {
            let Some(request) = self.request.clone() else {
                return Vec::new();
            };
            let found = self.signers().iter().find_map(|owner| {
                checked(&request.safe_tx_hash, &request.owners, owner, signature)
            });
            match found {
                Some(approval) => self.finish(SessionOutcome::Approved { approval }),
                None => {
                    if !late {
                        self.fail(Failure::BadSignature);
                    }
                    Vec::new()
                }
            }
        }

        fn peer_state(&mut self, state: &str) -> Vec<Out> {
            let connected = state == "connected";
            let mut outs = Vec::new();
            if connected && !self.connected {
                if self.was_connected {
                    // Back after a break: the page may have lost responses (core §6).
                    if let Some(room) = self.room.clone() {
                        for (_, message) in self.pending.values() {
                            outs.push(Out::Send {
                                room: room.clone(),
                                message: message.clone(),
                            });
                        }
                    }
                }
                self.was_connected = true;
            }
            self.connected = connected;
            if connected && self.phase == Phase::Waiting {
                self.to(Phase::Connecting);
            }
            self.away = !connected
                && matches!(
                    self.phase,
                    Phase::Switching | Phase::Confirming | Phase::Connecting
                );
            outs
        }

        /// The room ended: by either side's end, the pairing is forgotten; a
        /// leave keeps it.
        fn closed(&mut self, room: &str, reason: &str) -> Vec<Out> {
            if self.room.as_deref() != Some(room) {
                return Vec::new();
            }
            self.room = None;
            self.connected = false;
            let mut outs = Vec::new();
            if reason != "left" {
                outs.push(Out::Forget {
                    id: self.saved.id.clone(),
                });
            }
            if !self.finished() {
                self.pending.clear();
                self.fail(Failure::Wallet {
                    problem: WalletProblem::Disconnected,
                });
            }
            outs
        }
    }

    /// What a session decision asks of the shell.
    enum Out {
        Open { pairing: Option<String> },
        Send { room: String, message: Value },
        Close { room: String, end: bool },
        Save,
        Forget { id: String },
    }

    #[derive(Default)]
    pub struct Model {
        state: Shared<SessionState>,
    }

    #[derive(Default)]
    pub struct OutsideSession;

    type Cmd = Command<MultisigEffect, Event>;

    /// Decide in `update` (the order events came in), then do it: the outs
    /// in order, and the phase timed from when it changed.
    fn react(model: &Model, f: impl FnOnce(&mut SessionState) -> Vec<Out>) -> Cmd {
        let (outs, seq, changed) = {
            let mut s = lock(&model.state);
            let before = s.phase_seq;
            let outs = f(&mut s);
            (outs, s.phase_seq, s.phase_seq != before)
        };
        if outs.is_empty() && !changed {
            return render();
        }
        let state = model.state.clone();
        Command::all([
            render(),
            Command::new(move |ctx| async move {
                let io = Io::new(ctx);
                if changed {
                    let now = io.now().await;
                    let mut s = lock(&state);
                    if s.phase_seq == seq {
                        s.since = now;
                    }
                }
                for out in outs {
                    perform(&io, &state, out).await;
                }
                io.emit(Event::Changed);
            }),
        ])
    }

    async fn load_saved(io: &Io<Event>) -> Vec<Saved> {
        io.load(&[PAIRINGS_KEY])
            .await
            .get(PAIRINGS_KEY)
            .cloned()
            .flatten()
            .and_then(|r| serde_json::from_str::<Vec<Value>>(&r).ok())
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|r| serde_json::from_value::<Saved>(r).ok())
                    .filter(|r| !r.id.is_empty() && !r.record.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }

    async fn perform(io: &Io<Event>, st: &Shared<SessionState>, out: Out) {
        match out {
            Out::Open { pairing } => {
                let relay = lock(st).relay_url.clone();
                let answer = io
                    .ask(MultisigOperation::PeerOpen {
                        relay,
                        invite: None,
                        pairing,
                    })
                    .await;
                match answer {
                    MultisigShellResult::PeerOpened {
                        room,
                        invite,
                        pairing,
                    } => io.emit(Event::Opened {
                        room,
                        invite,
                        pairing,
                    }),
                    MultisigShellResult::PeerClosed { reason } => {
                        io.emit(Event::OpenFailed { reason });
                    }
                    other => io.emit(Event::OpenFailed {
                        reason: format!("{other:?}"),
                    }),
                }
            }
            Out::Send { room, message } => {
                let _ = io.ask(MultisigOperation::PeerSend { room, message }).await;
            }
            Out::Close { room, end } => {
                let _ = io
                    .ask(MultisigOperation::PeerClose {
                        room,
                        cancel: false,
                        end,
                    })
                    .await;
            }
            Out::Save => {
                let mine = {
                    let mut s = lock(st);
                    let accounts = s.accounts.clone();
                    s.saved.accounts = accounts;
                    s.saved.wallet = s.wallet.clone();
                    // Only a paired room is worth keeping: an unused invite is spent.
                    if !s.approved && s.saved.accounts.is_empty() {
                        return;
                    }
                    s.saved.clone()
                };
                let now = io.now().await;
                let mut rows: Vec<Saved> = load_saved(io)
                    .await
                    .into_iter()
                    .filter(|r| r.id != mine.id)
                    .collect();
                rows.push(Saved {
                    saved_ms: now,
                    ..mine
                });
                let skip = rows.len().saturating_sub(KEEP_PAIRINGS);
                let kept = serde_json::to_string(&rows[skip..]).ok();
                io.store(PAIRINGS_KEY, kept).await;
            }
            Out::Forget { id } => {
                let rows: Vec<Saved> = load_saved(io)
                    .await
                    .into_iter()
                    .filter(|r| r.id != id)
                    .collect();
                io.store(PAIRINGS_KEY, serde_json::to_string(&rows).ok())
                    .await;
            }
        }
    }

    fn listen(room: String) -> Cmd {
        Command::new(move |ctx| async move {
            let io = Io::new(ctx);
            loop {
                match io
                    .ask(MultisigOperation::PeerReceive { room: room.clone() })
                    .await
                {
                    MultisigShellResult::PeerMessage { message } => io.emit(Event::Arrived {
                        room: room.clone(),
                        message,
                    }),
                    MultisigShellResult::PeerClosed { reason } => {
                        io.emit(Event::Closed { room, reason });
                        break;
                    }
                    _ => {
                        io.emit(Event::Closed {
                            room,
                            reason: "closed".to_owned(),
                        });
                        break;
                    }
                }
            }
        })
    }

    /// A new room whatever this device paired before (r4C-6).
    fn fresh(s: &mut SessionState, id: String) -> Vec<Out> {
        s.saved = Saved {
            id,
            ..Saved::default()
        };
        s.resumed = false;
        s.invite = None;
        s.accounts.clear();
        s.chain = None;
        s.wallet = None;
        s.approved = false;
        s.connected = false;
        s.was_connected = false;
        s.switched = false;
        s.failure = None;
        s.away = false;
        s.to(Phase::Waiting);
        vec![Out::Open { pairing: None }]
    }

    impl crux_core::App for OutsideSession {
        type Event = Event;
        type Model = Model;
        type ViewModel = OutsideSessionView;
        type Effect = MultisigEffect;

        #[allow(clippy::too_many_lines)]
        fn update(&self, event: Event, model: &mut Model) -> Cmd {
            match event {
                Event::Changed => render(),
                Event::Start {
                    request,
                    service_url,
                    relay,
                    origin,
                } => {
                    {
                        let mut s = lock(&model.state);
                        *s = SessionState::default();
                        s.service.set_url(&service_url);
                        s.relay_url = relay;
                        s.origin = origin;
                        s.request = Some(*request);
                    }
                    let state = model.state.clone();
                    Command::all([
                        render(),
                        Command::new(move |ctx| async move {
                            let io = Io::new(ctx);
                            let tag = io.random_hex(8).await.unwrap_or_default();
                            let id = io.random_hex(16).await.unwrap_or_default();
                            let now = io.now().await;
                            let saved = load_saved(&io).await;
                            let outs = {
                                let mut s = lock(&state);
                                s.tag = format!("vela-{}", tag.trim_start_matches("0x"));
                                s.since = now;
                                let owners = s
                                    .request
                                    .as_ref()
                                    .map(|r| r.owners.clone())
                                    .unwrap_or_default();
                                let known = saved.into_iter().rev().find(|x| {
                                    x.accounts
                                        .iter()
                                        .any(|a| owners.iter().any(|o| same_address(o, a)))
                                });
                                match known {
                                    Some(k) => {
                                        s.resumed = true;
                                        s.accounts.clone_from(&k.accounts);
                                        s.wallet = k.wallet.clone().filter(|w| w != PAGE_NAME);
                                        let record = k.record.clone();
                                        s.saved = k;
                                        vec![Out::Open {
                                            pairing: Some(record),
                                        }]
                                    }
                                    None => fresh(&mut s, id.trim_start_matches("0x").to_owned()),
                                }
                            };
                            for out in outs {
                                perform(&io, &state, out).await;
                            }
                            io.emit(Event::Changed);
                        }),
                    ])
                }
                Event::Opened {
                    room,
                    invite,
                    pairing,
                } => {
                    let cmd = react(model, |s| {
                        s.room = Some(room.clone());
                        if !s.resumed {
                            s.invite = invite;
                        }
                        if let Some(p) = pairing {
                            s.saved.record = p;
                        }
                        s.advance()
                    });
                    Command::all([listen(room), cmd])
                }
                Event::OpenFailed { reason } => react(model, |s| {
                    s.fail(Failure::NoRoom { reason });
                    Vec::new()
                }),
                Event::Arrived { room, message } => react(model, |s| s.arrived(&room, &message)),
                Event::Closed { room, reason } => react(model, |s| s.closed(&room, &reason)),
                Event::PeerState { room, state } => react(model, |s| {
                    if s.room.as_deref() != Some(room.as_str()) {
                        return Vec::new();
                    }
                    if state == "closed" {
                        return Vec::new();
                    }
                    s.peer_state(&state)
                }),
                Event::Paired { room, pairing } => react(model, |s| {
                    if s.room.as_deref() == Some(room.as_str()) {
                        s.saved.record = pairing;
                        return vec![Out::Save];
                    }
                    Vec::new()
                }),
                Event::Relay { status } => {
                    lock(&model.state).relay = status;
                    render()
                }
                Event::AskAgain => react(model, |s| {
                    if s.finished() {
                        return Vec::new();
                    }
                    let mut outs = s.withdraw();
                    s.failure = None;
                    s.switched = false;
                    outs.extend(s.advance());
                    outs
                }),
                Event::StopWaiting => react(model, |s| {
                    if s.finished() {
                        return Vec::new();
                    }
                    let outs = s.withdraw();
                    s.failure = Some(Failure::StoppedWaiting);
                    outs
                }),
                Event::AnotherWallet { unpair } => {
                    let state = model.state.clone();
                    Command::all([
                        render(),
                        Command::new(move |ctx| async move {
                            let io = Io::new(ctx);
                            let id = io.random_hex(16).await.unwrap_or_default();
                            let (outs, seq) = {
                                let mut s = lock(&state);
                                if s.finished() {
                                    return;
                                }
                                let mut outs = s.withdraw();
                                s.pending.clear();
                                if unpair {
                                    outs.push(Out::Forget {
                                        id: s.saved.id.clone(),
                                    });
                                }
                                if let Some(room) = s.room.take() {
                                    outs.push(Out::Close { room, end: unpair });
                                }
                                outs.extend(fresh(&mut s, id.trim_start_matches("0x").to_owned()));
                                (outs, s.phase_seq)
                            };
                            let now = io.now().await;
                            {
                                let mut s = lock(&state);
                                if s.phase_seq == seq {
                                    s.since = now;
                                }
                            }
                            for out in outs {
                                perform(&io, &state, out).await;
                            }
                            io.emit(Event::Changed);
                        }),
                    ])
                }
                Event::Close => react(model, |s| {
                    if s.finished() {
                        return Vec::new();
                    }
                    let mut outs = s.withdraw();
                    s.outcome = Some(SessionOutcome::Cancelled);
                    if let Some(room) = s.room.take() {
                        outs.push(Out::Close { room, end: false });
                    }
                    outs
                }),
                Event::MailboxTick => {
                    let (request, service, done) = {
                        let s = lock(&model.state);
                        (s.request.clone(), s.service.clone(), s.finished())
                    };
                    let Some(request) = request.filter(|_| !done) else {
                        return render();
                    };
                    let state = model.state.clone();
                    Command::new(move |ctx| async move {
                        let io = Io::new(ctx);
                        let Ok(Some(answer)) = service
                            .fetch_proposals(&io, request.chain_id, &request.safe, true)
                            .await
                        else {
                            return;
                        };
                        let found = answer
                            .proposals
                            .iter()
                            .find(|p| p.safe_tx_hash.eq_ignore_ascii_case(&request.safe_tx_hash))
                            .and_then(|p| {
                                p.approvals.iter().find_map(|a| match &a.approval {
                                    Approval::Ecdsa(e)
                                        if request
                                            .owners
                                            .iter()
                                            .any(|o| same_address(o, &e.owner)) =>
                                    {
                                        checked(
                                            &request.safe_tx_hash,
                                            &request.owners,
                                            &e.owner,
                                            &e.signature,
                                        )
                                    }
                                    _ => None,
                                })
                            });
                        let Some(approval) = found else { return };
                        let outs = lock(&state).finish(SessionOutcome::Approved { approval });
                        for out in outs {
                            perform(&io, &state, out).await;
                        }
                        io.emit(Event::Changed);
                    })
                }
            }
        }

        fn view(&self, model: &Model) -> OutsideSessionView {
            let s = lock(&model.state);
            let request = s.request.as_ref();
            OutsideSessionView {
                open: request.is_some() && !s.finished(),
                resumed: s.resumed,
                invite: s.invite.clone(),
                members: request
                    .map(|r| {
                        let mut out: Vec<String> = Vec::new();
                        for o in &r.owners {
                            let lower = o.to_ascii_lowercase();
                            if is_address(&lower) && !out.contains(&lower) {
                                out.push(lower);
                            }
                        }
                        out
                    })
                    .unwrap_or_default(),
                tx: request.map(|r| TxLink {
                    chain_id: r.chain_id,
                    safe: r.safe.clone(),
                    safe_tx_hash: r.safe_tx_hash.clone(),
                    for_members: Vec::new(),
                }),
                phase: s.phase,
                since: s.since,
                away: s.away,
                relay: s.relay.clone(),
                accounts: s.accounts.clone(),
                chain: s.chain.clone(),
                wallet: s.wallet.clone(),
                paired_account: s.accounts.first().cloned(),
                failure: s.failure.clone(),
                outcome: s.outcome.clone(),
            }
        }
    }
}
