//! One review of a multisig transaction, the same on every member's device
//! and for every action (docs/multisig-vault-design.md §3.6) — what a Vela
//! account's signing sheet gives a signer, and a guard a Vela account does
//! not need:
//!
//! - **The reading** ([`crate::multisig::review`]) and what it changes about
//!   the members ([`crate::multisig_reading::member_changes`]).
//! - **What moves**: the inner calls simulated as the Safe (`eth_simulateV1`
//!   from the Safe's address, the signing sheet's own path), read by
//!   [`crate::app::sim_outcome`]; where a network does not offer it, the
//!   Safe's own balances before and after its execution, read by the state
//!   probe below. Neither answering is said: never "nothing moves".
//! - **The state-diff guard** ([`state_guard`]): independent of any decoding,
//!   the execution is run against the Safe's current state and every change to
//!   what decides who controls it — its singleton (slot 0), modules, guard,
//!   module guard, fallback handler, owners and threshold — is compared with
//!   exactly what a decoded member change declares. Anything else is a red
//!   "this changes who controls this multisig": nothing is signed, no
//!   override. Two sources, whichever the network offers:
//!   - `debug_traceCall` with the prestate tracer in diff mode, of the real
//!     `execTransaction` (the approvals put in place by a state override of
//!     `approvedHashes`): every storage write to the Safe, each one named or
//!     refused;
//!   - an `eth_call` of the Safe's own `simulateAndRevert` (StorageAccessible,
//!     Safe v1.3.0 – v1.5.0) DELEGATECALLing `SafeStateProbe`
//!     (`contracts/SafeStateProbe.sol`), whose code a state override puts at a
//!     fresh address — never deployed: it reads the controlling state, runs
//!     the inner call as the Safe does, reads it again. It sees the named
//!     state and the `owners`/`modules` entries of every address the
//!     transaction mentions, not every slot — said beside it.
//!   Neither answering: said, and only the decoding rules stand.
//! - **Recipient risk** for every address a leg sends to, calls, or makes a
//!   member: first time, a contract, and an address that looks like one the
//!   person knows (same first and last four characters, another address).
//! - **The hashes** a hardware wallet shows (domain, message, safeTxHash) and
//!   the SafeTx fields and EIP-712 typed data, exactly as signed.
//! - **The verdict and the sign rule** — refused (no signature, no
//!   override), "sign anyway" (the signing sheet's own friction: a call Vela
//!   cannot read, or a danger), or plain.
//! - **What the AI's opinion is bound to** ([`MultisigReview::advice_request`]):
//!   the proposal's safeTxHash on its chain, read by `agent_review` like any
//!   signing request.
//!
//! The signing gate before every signature is here too ([`signing_gate`]):
//! the chain's verdict on who can act for the multisig, read again right
//! before each passkey, and this review's sign rule.

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::app::corroborate::{self, Standing};
use crate::app::multisig_outside::reading::TokenFacts;
use crate::app::sim_outcome::{self, SimOutcome, SimVerdict};
use crate::app::token_trust::{TrustAssetDelta, TrustDeltaKind};
use crate::error::CoreError;
use crate::multisig::{
    self, Action, Genesis, Review, SafeTx, SafeTxHashes, FALLBACK_HANDLER_STORAGE_SLOT,
    GUARD_STORAGE_SLOT, MODULE_GUARD_STORAGE_SLOT,
};
use crate::multisig_reading::{self, MemberChanges};

#[path = "multisig_review_reading.rs"]
pub mod reading;

fn err(message: impl Into<String>) -> CoreError {
    CoreError::Multisig(message.into())
}

fn addr(s: &str) -> Result<Address, CoreError> {
    s.trim()
        .parse::<Address>()
        .map_err(|_| err(format!("not an address: {s}")))
}

fn hex(data: &[u8]) -> String {
    crate::primitives::to_hex(data, true)
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    crate::primitives::from_hex(s).ok()
}

fn quantity(s: &str) -> Option<U256> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some("") => Some(U256::ZERO),
        Some(h) => U256::from_str_radix(h, 16).ok(),
        None => U256::from_str_radix(s, 10).ok(),
    }
}

/// A quantity (hex or decimal) as a decimal string; unreadable stays as is.
pub(crate) fn quantity_dec(s: &str) -> String {
    quantity(s).map_or_else(|| s.to_owned(), |v| v.to_string())
}

fn hex_quantity(v: U256) -> String {
    format!("{v:#x}")
}

fn word(v: U256) -> [u8; 32] {
    v.to_be_bytes()
}

fn word_addr(a: Address) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[12..].copy_from_slice(a.as_slice());
    w
}

fn slot_hex(w: B256) -> String {
    hex(w.as_slice())
}

fn checksum(a: Address) -> String {
    a.to_checksum(None)
}

const SENTINEL: Address =
    Address::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);

/// Safe's storage layout (v1.3.0 – v1.5.0): singleton 0, `modules` 1,
/// `owners` 2, `ownerCount` 3, `threshold` 4, `nonce` 5, `approvedHashes` 8.
const SLOT_SINGLETON: u64 = 0;
const SLOT_MODULES: u64 = 1;
const SLOT_OWNERS: u64 = 2;
const SLOT_COUNT: u64 = 3;
const SLOT_THRESHOLD: u64 = 4;
const SLOT_NONCE: u64 = 5;
const SLOT_APPROVED: u64 = 8;

/// The longest list the probe walks (`SafeStateProbe.MAX_LIST`).
const PROBE_MAX_LIST: usize = 64;
/// The most addresses a transaction's data is searched for.
const MAX_WATCHED: usize = 48;
/// The most tokens the probe reads a balance of.
const MAX_TOKENS: usize = 24;

/// `SafeStateProbe`'s runtime code (`contracts/build.sh` writes it).
const PROBE_CODE: &str = include_str!("multisig_probe_code.hex");

/// Where the state override puts the probe: an address nobody holds a key or
/// code for, `keccak256("vela/multisig-state-probe/v1")[12..]`.
pub fn probe_address() -> Address {
    Address::from_slice(&keccak256(b"vela/multisig-state-probe/v1")[12..])
}

fn slot(n: u64) -> B256 {
    B256::from(U256::from(n))
}

fn fixed_slot(s: &str) -> B256 {
    s.parse().unwrap_or_default()
}

/// The storage slot of `mapping(address => …)` at `slot` for `key`.
fn entry(key: Address, at: u64) -> B256 {
    let mut p = word_addr(key).to_vec();
    p.extend_from_slice(&word(U256::from(at)));
    keccak256(p)
}

/// `approvedHashes[owner][hash]`.
fn approved_entry(owner: Address, hash: B256) -> B256 {
    let mut p = hash.to_vec();
    p.extend_from_slice(entry(owner, SLOT_APPROVED).as_slice());
    keccak256(p)
}

// ---------------------------------------------------------------------------
// The calls a transaction makes, and the addresses it names
// ---------------------------------------------------------------------------

/// One inner call, as the signing sheet simulates calls (`eth_simulateV1`
/// from the Safe): `value` a hex quantity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SimCall {
    pub to: String,
    pub value: String,
    pub data: String,
}

fn decimal_hex(v: &str) -> String {
    quantity(v).map_or_else(|| "0x0".to_owned(), hex_quantity)
}

/// The calls the Safe makes when it executes `tx`, read from `review`: one
/// CALL, or a MultiSendCallOnly batch's calls. A DELEGATECALL anywhere else
/// cannot be simulated as a call from the Safe — only the state probe or a
/// trace runs it (empty here).
fn inner_calls(tx: &SafeTx, review: &Review) -> Vec<SimCall> {
    if let Some(Action::Batch { calls }) = &review.action {
        return calls
            .iter()
            .map(|c| SimCall {
                to: c.to.clone(),
                value: decimal_hex(&c.value),
                data: c.data.clone(),
            })
            .collect();
    }
    if tx.operation != 0 {
        return Vec::new();
    }
    vec![SimCall {
        to: tx.to.clone(),
        value: tx.value.clone(),
        data: tx.data.clone(),
    }]
}

/// Every address-shaped word in `data`, at any offset: twelve zero bytes and
/// an address that is no small number. A transaction cannot hide an address
/// it passes to code from this search by misaligning it.
fn addresses_in(data: &[u8], out: &mut Vec<Address>) {
    if data.len() < 32 {
        return;
    }
    for i in 0..=data.len() - 32 {
        if out.len() >= MAX_WATCHED {
            return;
        }
        let w = &data[i..i + 32];
        if w[..12].iter().any(|b| *b != 0) || w[12..24].iter().all(|b| *b == 0) {
            continue;
        }
        let a = Address::from_slice(&w[12..]);
        if !out.contains(&a) {
            out.push(a);
        }
    }
}

/// The addresses whose `owners`/`modules` entries the guard reads: the
/// sentinel, the members, every address a member change names, and every
/// address in the transaction.
fn watched(tx: &SafeTx, owners: &[Address], declared: &[Address]) -> Vec<Address> {
    let mut out = vec![SENTINEL];
    for a in owners.iter().chain(declared) {
        if !out.contains(a) {
            out.push(*a);
        }
    }
    if let Ok(to) = tx.to.parse::<Address>() {
        if !out.contains(&to) {
            out.push(to);
        }
    }
    addresses_in(&unhex(&tx.data).unwrap_or_default(), &mut out);
    out
}

// ---------------------------------------------------------------------------
// What a member change declares
// ---------------------------------------------------------------------------

/// The members and threshold `action` leaves, applied to `owners` and
/// `threshold` as Safe applies them, with every address it names — or `None`
/// when it changes no member. A change that does not fit (the Safe would
/// revert) leaves them as they were: the execution says so itself.
fn declared(
    action: Option<&Action>,
    owners: &[Address],
    threshold: u64,
) -> (BTreeSet<Address>, U256, Vec<Address>, bool) {
    let mut set: BTreeSet<Address> = owners.iter().copied().collect();
    let mut t = U256::from(threshold);
    let mut named = Vec::new();
    let mut declares = false;
    let parse = |s: &str| s.parse::<Address>().ok();
    let mut apply = |a: &Action| {
        match a {
            Action::AddOwner { owner, threshold } => {
                if let (Some(o), Some(th)) = (parse(owner), quantity(threshold)) {
                    set.insert(o);
                    t = th;
                    named.push(o);
                }
            }
            Action::RemoveOwner { owner, threshold } => {
                if let (Some(o), Some(th)) = (parse(owner), quantity(threshold)) {
                    set.remove(&o);
                    t = th;
                    named.push(o);
                }
            }
            Action::SwapOwner {
                old_owner,
                new_owner,
            } => {
                if let (Some(old), Some(new)) = (parse(old_owner), parse(new_owner)) {
                    set.remove(&old);
                    set.insert(new);
                    named.push(old);
                    named.push(new);
                }
            }
            Action::ChangeThreshold { threshold } => {
                if let Some(th) = quantity(threshold) {
                    t = th;
                }
            }
            _ => return,
        }
        declares = true;
    };
    match action {
        Some(Action::Batch { calls }) => calls.iter().for_each(|c| apply(&c.action)),
        Some(a) => apply(a),
        None => {}
    }
    (set, t, named, declares)
}

// ---------------------------------------------------------------------------
// The reads
// ---------------------------------------------------------------------------

/// What the shell knows before it reads: the Safe on its chain, the members
/// there now (`getOwners` order) and the threshold, whether it is deployed
/// there, its genesis (for one that is not), `eth_getCode` of what an earlier
/// call named in `needCode`, and the tokens whose balances to read (the
/// tokens it holds there; the transaction's own are added).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReadsInput {
    pub chain_id: u64,
    pub safe: String,
    pub tx: Option<SafeTx>,
    pub owners: Vec<String>,
    pub threshold: u64,
    pub deployed: bool,
    pub genesis: Option<Genesis>,
    pub codes: BTreeMap<String, Option<String>>,
    pub tokens: Vec<String>,
}

/// One JSON-RPC request for the shell to send as it is.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RpcRequest {
    pub method: String,
    pub params: Value,
}

/// The reads a review needs: `eth_simulateV1` of `simCalls` from the Safe
/// (the signing sheet's simulation), the state-diff `trace` (asked first) and
/// the state `probe` (asked when the trace is not offered), and the facts of
/// each address in `recipients` (`eth_getCode`; whether this device sent to
/// it before). `needCode`: read `eth_getCode` of these and ask again.
/// `unavailable`: why no state read can be made.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reads {
    pub need_code: Vec<String>,
    pub sim_calls: Vec<SimCall>,
    /// Real-device S2: the state override that puts an undeployed multisig
    /// in place (its deployment's code and storage) for `eth_simulateV1` —
    /// the simulation includes the deploy, as the execution will.
    pub sim_state_overrides: Option<Value>,
    pub trace: Option<RpcRequest>,
    pub probe: Option<RpcRequest>,
    pub unavailable: Option<String>,
    pub recipients: Vec<String>,
    pub tokens: Vec<String>,
}

/// The Safe as the state override puts it in place where it is not deployed
/// yet: the code of the singleton its setup leaves, and its storage then.
struct Placed {
    owners: Vec<Address>,
    threshold: u64,
    code: Option<String>,
    storage: Vec<(B256, U256)>,
}

/// Why the Safe cannot be put in place (yet): the code to read first, or why not at all.
#[derive(Default)]
struct NotPlaced {
    need_code: Vec<String>,
    unavailable: Option<String>,
}

fn placed(input: &ReadsInput, safe: Address) -> Result<Placed, NotPlaced> {
    let owners = input
        .owners
        .iter()
        .filter_map(|o| o.parse().ok())
        .collect::<Vec<Address>>();
    if input.deployed {
        return Ok(Placed {
            owners,
            threshold: input.threshold,
            code: None,
            storage: Vec::new(),
        });
    }
    let unavailable = |why: &str| NotPlaced {
        unavailable: Some(why.to_owned()),
        ..NotPlaced::default()
    };
    let Some(g) = input.genesis.as_ref() else {
        return Err(unavailable("noGenesis"));
    };
    let Some(start) = multisig::counterfactual_start(g, safe, input.chain_id) else {
        return Err(unavailable("genesisRefused"));
    };
    let code = input
        .codes
        .iter()
        .find(|(k, _)| k.parse::<Address>().ok() == Some(start.singleton))
        .map(|(_, v)| v.clone());
    let code = match code {
        None => {
            return Err(NotPlaced {
                need_code: vec![checksum(start.singleton)],
                ..NotPlaced::default()
            })
        }
        Some(None) => return Err(unavailable("unreachable")),
        Some(Some(code)) if !multisig::canonical_singleton_code(start.singleton, &code) => {
            return Err(unavailable("singletonCode"))
        }
        Some(Some(code)) => code,
    };
    let mut storage = vec![
        (
            slot(SLOT_SINGLETON),
            U256::from_be_slice(&word_addr(start.singleton)),
        ),
        (slot(SLOT_COUNT), U256::from(start.owners.len())),
        (slot(SLOT_THRESHOLD), U256::from(start.threshold)),
        (
            entry(SENTINEL, SLOT_MODULES),
            U256::from_be_slice(&word_addr(SENTINEL)),
        ),
    ];
    let mut prev = SENTINEL;
    for o in &start.owners {
        storage.push((
            entry(prev, SLOT_OWNERS),
            U256::from_be_slice(&word_addr(*o)),
        ));
        prev = *o;
    }
    storage.push((
        entry(prev, SLOT_OWNERS),
        U256::from_be_slice(&word_addr(SENTINEL)),
    ));
    if start.handler != Address::ZERO {
        storage.push((
            fixed_slot(FALLBACK_HANDLER_STORAGE_SLOT),
            U256::from_be_slice(&word_addr(start.handler)),
        ));
    }
    Ok(Placed {
        owners: start.owners,
        threshold: start.threshold,
        code: Some(code),
        storage,
    })
}

fn override_of(code: Option<&str>, storage: &[(B256, U256)]) -> Value {
    let mut o = Map::new();
    if let Some(code) = code {
        o.insert("code".into(), json!(code));
    }
    if !storage.is_empty() {
        let diff: Map<String, Value> = storage
            .iter()
            .map(|(k, v)| (slot_hex(*k), json!(hex(&word(*v)))))
            .collect();
        o.insert("stateDiff".into(), Value::Object(diff));
    }
    Value::Object(o)
}

/// Head-and-tail ABI encoding of `(address, uint256, bytes, uint8, address[], address[])`.
fn probe_calldata(t: &Inner, watch: &[Address], tokens: &[Address]) -> Vec<u8> {
    let sel = &keccak256(b"probe(address,uint256,bytes,uint8,address[],address[])")[..4];
    let pad = |n: usize| n.div_ceil(32) * 32;
    let data_at = 6 * 32;
    let watch_at = data_at + 32 + pad(t.data.len());
    let tokens_at = watch_at + 32 + 32 * watch.len();
    let mut out = sel.to_vec();
    out.extend_from_slice(&word_addr(t.to));
    out.extend_from_slice(&word(t.value));
    out.extend_from_slice(&word(U256::from(data_at)));
    out.extend_from_slice(&word(U256::from(t.operation)));
    out.extend_from_slice(&word(U256::from(watch_at)));
    out.extend_from_slice(&word(U256::from(tokens_at)));
    out.extend_from_slice(&word(U256::from(t.data.len())));
    out.extend_from_slice(&t.data);
    out.resize(out.len() + pad(t.data.len()) - t.data.len(), 0);
    for list in [watch, tokens] {
        out.extend_from_slice(&word(U256::from(list.len())));
        for a in list {
            out.extend_from_slice(&word_addr(*a));
        }
    }
    out
}

/// `simulateAndRevert(target, payload)`.
fn simulate_and_revert(target: Address, payload: &[u8]) -> Vec<u8> {
    let mut out = keccak256(b"simulateAndRevert(address,bytes)")[..4].to_vec();
    out.extend_from_slice(&word_addr(target));
    out.extend_from_slice(&word(U256::from(64)));
    out.extend_from_slice(&word(U256::from(payload.len())));
    out.extend_from_slice(payload);
    out.resize(
        out.len() + payload.len().div_ceil(32) * 32 - payload.len(),
        0,
    );
    out
}

struct Inner {
    to: Address,
    value: U256,
    data: Vec<u8>,
    operation: u8,
}

fn inner_of(tx: &SafeTx) -> Result<Inner, CoreError> {
    Ok(Inner {
        to: addr(&tx.to)?,
        value: quantity(&tx.value).ok_or_else(|| err("value"))?,
        data: unhex(&tx.data).ok_or_else(|| err("data"))?,
        operation: tx.operation,
    })
}

/// Every address a transaction sends to, calls, or makes a member, by leg.
fn recipients_of(tx: &SafeTx, review: &Review) -> Vec<(Option<u32>, RecipientRole, String)> {
    let mut out = Vec::new();
    let mut leg = |call: Option<u32>, a: &Action| match a {
        Action::Transfer { to, .. } => out.push((call, RecipientRole::Recipient, to.clone())),
        Action::Call { to, .. } => out.push((call, RecipientRole::Contract, to.clone())),
        Action::AddOwner { owner, .. } => out.push((call, RecipientRole::Member, owner.clone())),
        Action::SwapOwner { new_owner, .. } => {
            out.push((call, RecipientRole::Member, new_owner.clone()));
        }
        _ => {}
    };
    match &review.action {
        Some(Action::Batch { calls }) => {
            for (i, c) in calls.iter().enumerate() {
                leg(Some(u32::try_from(i + 1).unwrap_or(u32::MAX)), &c.action);
            }
        }
        Some(a) => leg(None, a),
        None => {
            if let Ok(to) = tx.to.parse::<Address>() {
                out.push((None, RecipientRole::Contract, checksum(to)));
            }
        }
    }
    out
}

fn tokens_of(review: &Review, extra: &[String]) -> Vec<Address> {
    let mut out: Vec<Address> = Vec::new();
    let mut add = |a: &str| {
        if let Ok(a) = a.parse::<Address>() {
            if a != Address::ZERO && !out.contains(&a) && out.len() < MAX_TOKENS {
                out.push(a);
            }
        }
    };
    let legs: Vec<&Action> = match &review.action {
        Some(Action::Batch { calls }) => calls.iter().map(|c| &c.action).collect(),
        Some(a) => vec![a],
        None => Vec::new(),
    };
    for leg in legs {
        match leg {
            Action::Transfer { token: Some(t), .. } => add(t),
            Action::Call { to, .. } => add(to),
            _ => {}
        }
    }
    extra.iter().for_each(|t| add(t));
    out
}

/// The reads [`multisig_review`] takes for `input` ([`Reads`]).
pub fn review_reads(input: &ReadsInput) -> Result<Reads, CoreError> {
    let tx = input.tx.as_ref().ok_or_else(|| err("tx"))?;
    let safe = addr(&input.safe)?;
    let review = multisig::review(input.chain_id, &input.safe, tx)?;
    let tokens = tokens_of(&review, &input.tokens);
    let mut reads = Reads {
        sim_calls: inner_calls(tx, &review),
        recipients: recipients_of(tx, &review)
            .into_iter()
            .map(|(_, _, a)| a)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        tokens: tokens.iter().map(|t| checksum(*t)).collect(),
        ..Reads::default()
    };
    let placed = match placed(input, safe) {
        Ok(p) => p,
        Err(r) => {
            reads.need_code = r.need_code;
            reads.unavailable = r.unavailable;
            return Ok(reads);
        }
    };
    if placed.code.is_some() {
        let mut overrides = Map::new();
        overrides.insert(
            checksum(safe),
            override_of(placed.code.as_deref(), &placed.storage),
        );
        reads.sim_state_overrides = Some(Value::Object(overrides));
    }
    let (_, _, named, _) = declared(review.action.as_ref(), &placed.owners, placed.threshold);
    let watch = watched(tx, &placed.owners, &named);
    let inner = inner_of(tx)?;
    let probe = probe_address();

    // The trace: the real execTransaction, the first `threshold` members (by
    // address, as Safe orders signatures) approving on chain — put in place
    // by the override, never sent.
    let hash: B256 = multisig::safe_tx_hash(input.chain_id, &input.safe, tx)?
        .parse()
        .map_err(|_| err("safeTxHash"))?;
    let mut signers = placed.owners.clone();
    signers.sort();
    signers.truncate(usize::try_from(placed.threshold).unwrap_or(usize::MAX));
    let mut signatures = Vec::new();
    for s in &signers {
        signatures.extend_from_slice(&word_addr(*s));
        signatures.extend_from_slice(&[0u8; 32]);
        signatures.push(1);
    }
    let mut storage = placed.storage.clone();
    storage.extend(
        signers
            .iter()
            .map(|s| (approved_entry(*s, hash), U256::from(1))),
    );
    // At its own number: a proposal queued behind others is traced as it will
    // run, not refused by a hash that does not match the nonce now.
    storage.push((slot(SLOT_NONCE), quantity(&tx.nonce).unwrap_or_default()));
    let exec = multisig::exec_transaction_data(tx, &hex(&signatures))?;
    let mut trace_overrides = Map::new();
    trace_overrides.insert(
        checksum(safe),
        override_of(placed.code.as_deref(), &storage),
    );
    reads.trace = Some(RpcRequest {
        method: "debug_traceCall".into(),
        params: json!([
            { "from": checksum(probe), "to": checksum(safe), "data": exec },
            "latest",
            {
                "tracer": "prestateTracer",
                "tracerConfig": { "diffMode": true },
                "stateOverrides": Value::Object(trace_overrides)
            }
        ]),
    });

    // The probe: the Safe delegatecalls SafeStateProbe and reverts with its readings.
    let payload = probe_calldata(&inner, &watch, &tokens);
    let mut probe_overrides = Map::new();
    probe_overrides.insert(checksum(probe), json!({ "code": PROBE_CODE.trim() }));
    if placed.code.is_some() {
        probe_overrides.insert(
            checksum(safe),
            override_of(placed.code.as_deref(), &placed.storage),
        );
    }
    reads.probe = Some(RpcRequest {
        method: "eth_call".into(),
        params: json!([
            { "to": checksum(safe), "data": hex(&simulate_and_revert(probe, &payload)) },
            "latest",
            Value::Object(probe_overrides)
        ]),
    });
    Ok(reads)
}

// ---------------------------------------------------------------------------
// The state-diff guard
// ---------------------------------------------------------------------------

/// Where the guard's reading came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StateSource {
    /// `debug_traceCall`, prestate diff: every write to the Safe's storage.
    Trace,
    /// `simulateAndRevert` + `SafeStateProbe`: the controlling state and the
    /// entries of every address the transaction names.
    Probe,
    /// Neither: only the decoding rules stand.
    None,
}

/// One change to what decides who controls the multisig.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CriticalWrite {
    /// Slot 0: the code the proxy runs — the Bybit change.
    Singleton {
        before: String,
        after: String,
    },
    /// A module enabled or removed: a module moves funds without members.
    Module {
        address: String,
        enabled: bool,
    },
    Guard {
        before: String,
        after: String,
    },
    ModuleGuard {
        before: String,
        after: String,
    },
    Handler {
        before: String,
        after: String,
    },
    /// The members after the execution are not the ones it declares.
    Owners {
        added: Vec<String>,
        removed: Vec<String>,
    },
    /// An `owners` entry written for an address that is no member after:
    /// signing rights with no place in the member list.
    OwnerEntry {
        address: String,
    },
    Threshold {
        before: String,
        after: String,
    },
    OwnerCount {
        before: String,
        after: String,
    },
    /// The nonce moved other than by the execution itself.
    Nonce,
    /// A write to a slot of the Safe's storage Vela cannot name.
    Storage {
        slot: String,
    },
}

/// What the guard found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum StateVerdict {
    /// Nothing that decides who controls it changes.
    Unchanged,
    /// Exactly the member change the transaction declares, and nothing else.
    Declared,
    /// Changes it does not declare: nothing is signed, no override.
    Undeclared { writes: Vec<CriticalWrite> },
    /// The execution reverts: it would change nothing.
    Reverts,
    /// No reading: `notOffered` (the network offers neither source),
    /// `unreachable`, `unreadable` (an answer the core cannot read, or lists
    /// longer than the probe walks), `noGenesis` / `genesisRefused` /
    /// `singletonCode` (not deployed, and the start state cannot be put in
    /// place).
    Unavailable { reason: String },
    /// r9-9: the transaction is refused for another reason
    /// ([`MultisigReview::refused`]) — a reassurance ("changes nothing about
    /// who controls the multisig") or a could-not-check beside "Vela won't
    /// sign this" would contradict it. Nothing is said in the guard's place.
    Withheld,
}

/// The guard's reading.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateGuard {
    pub source: StateSource,
    pub verdict: StateVerdict,
}

/// A JSON-RPC answer as the shell got it: the envelope (`result` or
/// `error`), or `{"unreachable": true}`.
pub type RpcAnswer = Value;

fn unreachable(a: &RpcAnswer) -> bool {
    a.get("unreachable").and_then(Value::as_bool) == Some(true)
}

/// The node does not offer the method (or refused its params): -32601,
/// -32602, -32600 or a "not supported/not found" message.
fn not_offered(a: &RpcAnswer) -> bool {
    let Some(e) = a.get("error").filter(|e| !e.is_null()) else {
        return false;
    };
    let code = e.get("code").and_then(Value::as_i64);
    let msg = e
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    matches!(code, Some(-32601 | -32602 | -32600 | -32000 | -32004))
        && (msg.contains("not supported")
            || msg.contains("not found")
            || msg.contains("not available")
            || msg.contains("does not exist")
            || msg.contains("unsupported")
            || msg.contains("method")
            || code == Some(-32601))
}

/// What the members and rule look like after, against what is declared.
struct Expect {
    before: Vec<Address>,
    owners: BTreeSet<Address>,
    threshold: U256,
    before_threshold: U256,
}

fn owners_diff(before: &[Address], after: &BTreeSet<Address>) -> CriticalWrite {
    let b: BTreeSet<Address> = before.iter().copied().collect();
    CriticalWrite::Owners {
        added: after.difference(&b).map(|a| checksum(*a)).collect(),
        removed: b.difference(after).map(|a| checksum(*a)).collect(),
    }
}

fn word_hex(v: U256) -> String {
    if v >> 160 == U256::ZERO {
        checksum(Address::from_word(B256::from(v)))
    } else {
        hex(&word(v))
    }
}

/// The trace's storage writes to `safe`: `slot → (before, after)`.
fn trace_writes(result: &Value, safe: Address) -> Option<BTreeMap<B256, (U256, U256)>> {
    let side = |name: &str| -> Option<Map<String, Value>> {
        let accounts = result.get(name)?.as_object()?;
        Some(
            accounts
                .iter()
                .find(|(k, _)| k.parse::<Address>().ok() == Some(safe))
                .and_then(|(_, v)| v.get("storage"))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default(),
        )
    };
    let pre = side("pre")?;
    let post = side("post")?;
    let mut out = BTreeMap::new();
    for k in pre.keys().chain(post.keys()) {
        let key: B256 = quantity(k).map(B256::from)?;
        let read = |m: &Map<String, Value>| {
            m.get(k)
                .and_then(Value::as_str)
                .map_or(Some(U256::ZERO), quantity)
        };
        let (b, a) = (read(&pre)?, read(&post)?);
        // A slot only in `pre` was cleared; one only in `post` was zero.
        let a = if post.contains_key(k) { a } else { U256::ZERO };
        if a != b {
            out.insert(key, (b, a));
        }
    }
    Some(out)
}

fn as_address(v: U256) -> Address {
    Address::from_word(B256::from(v))
}

/// Judge the trace's writes against what the transaction declares.
fn judge_trace(
    writes: &BTreeMap<B256, (U256, U256)>,
    e: &Expect,
    candidates: &[Address],
) -> Vec<CriticalWrite> {
    let mut out = Vec::new();
    let mut owners_entries: BTreeMap<Address, U256> = BTreeMap::new();
    let mut prev = SENTINEL;
    for o in &e.before {
        owners_entries.insert(prev, U256::from_be_slice(&word_addr(*o)));
        prev = *o;
    }
    owners_entries.insert(prev, U256::from_be_slice(&word_addr(SENTINEL)));
    let owner_slot: BTreeMap<B256, Address> = candidates
        .iter()
        .map(|c| (entry(*c, SLOT_OWNERS), *c))
        .collect();
    let module_slot: BTreeMap<B256, Address> = candidates
        .iter()
        .map(|c| (entry(*c, SLOT_MODULES), *c))
        .collect();
    let mut threshold = e.before_threshold;
    let mut count: Option<(U256, U256)> = None;
    let mut written_owner_entries = Vec::new();
    for (k, (b, a)) in writes {
        let named = |s: &str| fixed_slot(s) == *k;
        if *k == slot(SLOT_SINGLETON) {
            out.push(CriticalWrite::Singleton {
                before: word_hex(*b),
                after: word_hex(*a),
            });
        } else if *k == slot(SLOT_NONCE) {
            if *a != b.saturating_add(U256::from(1)) {
                out.push(CriticalWrite::Nonce);
            }
        } else if *k == slot(SLOT_THRESHOLD) {
            threshold = *a;
        } else if *k == slot(SLOT_COUNT) {
            count = Some((*b, *a));
        } else if named(GUARD_STORAGE_SLOT) {
            out.push(CriticalWrite::Guard {
                before: word_hex(*b),
                after: word_hex(*a),
            });
        } else if named(MODULE_GUARD_STORAGE_SLOT) {
            out.push(CriticalWrite::ModuleGuard {
                before: word_hex(*b),
                after: word_hex(*a),
            });
        } else if named(FALLBACK_HANDLER_STORAGE_SLOT) {
            out.push(CriticalWrite::Handler {
                before: word_hex(*b),
                after: word_hex(*a),
            });
        } else if let Some(o) = owner_slot.get(k) {
            owners_entries.insert(*o, *a);
            written_owner_entries.push(*o);
        } else if let Some(m) = module_slot.get(k) {
            // modules[SENTINEL] is the list head: a new head is a new module.
            let enabled = *a != U256::ZERO;
            let address = if *m == SENTINEL { as_address(*a) } else { *m };
            if !(*m == SENTINEL && as_address(*a) == SENTINEL) {
                out.push(CriticalWrite::Module {
                    address: checksum(address),
                    enabled,
                });
            }
        } else {
            out.push(CriticalWrite::Storage { slot: slot_hex(*k) });
        }
    }
    // The list as it stands after: walked from the sentinel.
    let mut after = BTreeSet::new();
    let mut at = SENTINEL;
    let mut whole = false;
    for _ in 0..=PROBE_MAX_LIST * 2 {
        let next = owners_entries
            .get(&at)
            .map_or(Address::ZERO, |v| as_address(*v));
        if next == SENTINEL {
            whole = true;
            break;
        }
        if next == Address::ZERO || !after.insert(next) {
            break;
        }
        at = next;
    }
    if !whole || after != e.owners {
        out.push(owners_diff(&e.before, &after));
    }
    for o in written_owner_entries {
        let v = owners_entries.get(&o).copied().unwrap_or_default();
        if o != SENTINEL && !after.contains(&o) && v != U256::ZERO {
            out.push(CriticalWrite::OwnerEntry {
                address: checksum(o),
            });
        }
    }
    if threshold != e.threshold {
        out.push(CriticalWrite::Threshold {
            before: e.before_threshold.to_string(),
            after: threshold.to_string(),
        });
    }
    if let Some((b, a)) = count {
        if a != U256::from(after.len()) {
            out.push(CriticalWrite::OwnerCount {
                before: b.to_string(),
                after: a.to_string(),
            });
        }
    } else if after.len() != e.before.len() {
        out.push(CriticalWrite::OwnerCount {
            before: e.before.len().to_string(),
            after: e.before.len().to_string(),
        });
    }
    out
}

/// One reading of the probe (`SafeStateProbe._read`).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    singleton: U256,
    count: U256,
    threshold: U256,
    nonce: U256,
    guard: U256,
    module_guard: U256,
    handler: U256,
    balance: U256,
    owners: Option<Vec<Address>>,
    modules: Option<Vec<Address>>,
    watched: Vec<(U256, U256)>,
    tokens: Vec<Option<U256>>,
}

fn read_snapshot(w: &[U256], watched: usize, tokens: usize) -> Option<Snapshot> {
    let mut i = 0usize;
    let mut next = || {
        let v = *w.get(i)?;
        i += 1;
        Some(v)
    };
    let mut fixed = [U256::ZERO; 8];
    for f in &mut fixed {
        *f = next()?;
    }
    let list = |next: &mut dyn FnMut() -> Option<U256>| -> Option<Option<Vec<Address>>> {
        let complete = next()? == U256::from(1);
        let n: usize = next()?.try_into().ok()?;
        if n > PROBE_MAX_LIST {
            return None;
        }
        let items = (0..n)
            .map(|_| next().map(as_address))
            .collect::<Option<Vec<_>>>()?;
        Some(complete.then_some(items))
    };
    let owners = list(&mut next)?;
    let modules = list(&mut next)?;
    let watched = (0..watched)
        .map(|_| Some((next()?, next()?)))
        .collect::<Option<Vec<_>>>()?;
    let tokens = (0..tokens)
        .map(|_| {
            let ok = next()? == U256::from(1);
            let b = next()?;
            Some(ok.then_some(b))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Snapshot {
        singleton: fixed[0],
        count: fixed[1],
        threshold: fixed[2],
        nonce: fixed[3],
        guard: fixed[4],
        module_guard: fixed[5],
        handler: fixed[6],
        balance: fixed[7],
        owners,
        modules,
        watched,
        tokens,
    })
}

/// What the probe answered: both readings, and whether the inner call ran.
#[derive(Clone)]
struct ProbeAnswer {
    before: Snapshot,
    after: Snapshot,
    success: bool,
    ret: Vec<u8>,
}

/// Read the revert payload `simulateAndRevert` answers with:
/// `success ‖ length ‖ abi.encode(uint256[], uint256[], bool, bytes)`.
fn probe_answer(a: &RpcAnswer, watched: usize, tokens: usize) -> Option<ProbeAnswer> {
    let e = a.get("error")?;
    let data = e
        .get("data")
        .and_then(|d| d.as_str().or_else(|| d.get("data").and_then(Value::as_str)))?;
    let bytes = unhex(data)?;
    let w = |at: usize| bytes.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    if w(0)? != U256::from(1) {
        return None;
    }
    let len: usize = w(32)?.try_into().ok()?;
    let body = bytes.get(64..64usize.checked_add(len)?)?;
    let bw = |at: usize| body.get(at..at.checked_add(32)?).map(U256::from_be_slice);
    let array = |at: usize| -> Option<Vec<U256>> {
        let n: usize = bw(at)?.try_into().ok()?;
        if n > 4096 {
            return None;
        }
        (0..n).map(|i| bw(at + 32 + i * 32)).collect()
    };
    let before = array(bw(0)?.try_into().ok()?)?;
    let after = array(bw(32)?.try_into().ok()?)?;
    let success = bw(64)? == U256::from(1);
    let ret_at: usize = bw(96)?.try_into().ok()?;
    let ret_len: usize = bw(ret_at)?.try_into().ok()?;
    let ret = body
        .get(ret_at + 32..(ret_at + 32).checked_add(ret_len)?)?
        .to_vec();
    Some(ProbeAnswer {
        before: read_snapshot(&before, watched, tokens)?,
        after: read_snapshot(&after, watched, tokens)?,
        success,
        ret,
    })
}

fn judge_probe(p: &ProbeAnswer, e: &Expect, watch: &[Address]) -> Option<Vec<CriticalWrite>> {
    let (b, a) = (&p.before, &p.after);
    let mut out = Vec::new();
    let pair = |x: U256, y: U256| (word_hex(x), word_hex(y));
    if b.singleton != a.singleton {
        let (before, after) = pair(b.singleton, a.singleton);
        out.push(CriticalWrite::Singleton { before, after });
    }
    if b.guard != a.guard {
        let (before, after) = pair(b.guard, a.guard);
        out.push(CriticalWrite::Guard { before, after });
    }
    if b.module_guard != a.module_guard {
        let (before, after) = pair(b.module_guard, a.module_guard);
        out.push(CriticalWrite::ModuleGuard { before, after });
    }
    if b.handler != a.handler {
        let (before, after) = pair(b.handler, a.handler);
        out.push(CriticalWrite::Handler { before, after });
    }
    if b.nonce != a.nonce {
        out.push(CriticalWrite::Nonce);
    }
    let (bm, am) = (b.modules.as_ref()?, a.modules.as_ref());
    match am {
        None => out.push(CriticalWrite::Storage {
            slot: "modules".into(),
        }),
        Some(am) => {
            for m in am.iter().filter(|m| !bm.contains(m)) {
                out.push(CriticalWrite::Module {
                    address: checksum(*m),
                    enabled: true,
                });
            }
            for m in bm.iter().filter(|m| !am.contains(m)) {
                out.push(CriticalWrite::Module {
                    address: checksum(*m),
                    enabled: false,
                });
            }
        }
    }
    b.owners.as_ref()?;
    let after: BTreeSet<Address> = a.owners.clone().unwrap_or_default().into_iter().collect();
    if a.owners.is_none() || after != e.owners {
        out.push(owners_diff(&e.before, &after));
    }
    if a.threshold != e.threshold {
        out.push(CriticalWrite::Threshold {
            before: b.threshold.to_string(),
            after: a.threshold.to_string(),
        });
    }
    if a.count != U256::from(after.len()) {
        out.push(CriticalWrite::OwnerCount {
            before: b.count.to_string(),
            after: a.count.to_string(),
        });
    }
    for (i, x) in watch.iter().enumerate() {
        let (Some((bo, bmod)), Some((ao, amod))) = (b.watched.get(i), a.watched.get(i)) else {
            continue;
        };
        if *x != SENTINEL && bo != ao && !after.contains(x) && *ao != U256::ZERO {
            out.push(CriticalWrite::OwnerEntry {
                address: checksum(*x),
            });
        }
        let listed = am.is_some_and(|am| am.contains(x) != bm.contains(x));
        if bmod != amod && !listed && *x != SENTINEL {
            out.push(CriticalWrite::Module {
                address: checksum(*x),
                enabled: *amod != U256::ZERO,
            });
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// The review
// ---------------------------------------------------------------------------

/// What the shell read for one address a leg names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecipientFacts {
    pub address: String,
    /// It has code (an EIP-7702 delegation is an account); `None`: unread.
    pub is_contract: Option<bool>,
    /// This device never sent to it; `None`: unread.
    pub first_time: Option<bool>,
}

/// An address the person knows: a contact, an account of theirs, a member.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Known {
    pub address: String,
    pub label: String,
    /// Whose it is (r4a-18, r5reg-8): a contact (the default, as before), one
    /// of this device's own accounts, or a member of the multisig.
    pub kind: KnownKind,
}

/// Whose an address the person knows is ([`Known::kind`]), the closest last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KnownKind {
    /// A contact: named by the person, still somebody else's.
    #[default]
    Contact,
    /// A member of the multisig the review is of.
    Member,
    /// One of this device's own accounts (a Vela account, a multisig here).
    Own,
}

impl KnownKind {
    /// Not a stranger's: never "first time sending here", never "coins sent
    /// to a contract are lost" — a smart account the person controls, or one
    /// of the multisig's own members.
    fn trusted(self) -> bool {
        matches!(self, KnownKind::Own | KnownKind::Member)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecipientRole {
    /// Coin or tokens go to it.
    Recipient,
    /// It is called.
    Contract,
    /// It becomes a member.
    Member,
}

/// One address a leg names, and what is known of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipient {
    /// The leg (from 1) in a batch; `None` for the transaction itself.
    pub call: Option<u32>,
    pub role: RecipientRole,
    pub address: String,
    pub is_contract: Option<bool>,
    pub first_time: Option<bool>,
    /// It is not, but looks like, an address the person knows.
    pub lookalike: Option<Known>,
    /// It is an address the person knows (r4a-18): the closest of them —
    /// their own account over a member over a contact.
    pub known: Option<Known>,
    /// The review says "first time sending here" of it: money goes to it,
    /// this device never sent there, and it is neither one of the person's
    /// own accounts nor a member. A member being added receives nothing
    /// (r5reg-16).
    pub first_time_cue: bool,
    /// The review says money goes to a contract (coins may be lost there):
    /// never of the person's own (smart) accounts nor of a member (r5reg-8).
    pub contract_cue: bool,
}

/// The address the person knows that `address` is, exactly (any case): the
/// closest of them — their own account over a member over a contact.
fn known_as(known: &[Known], address: &str) -> Option<Known> {
    known
        .iter()
        .filter(|k| k.address.eq_ignore_ascii_case(address))
        .max_by_key(|k| k.kind)
        .cloned()
}

/// An address that looks like `known` (address poisoning): another address
/// with the same first four and last four hex characters — what a person
/// checking "the start and the end" sees.
pub fn looks_like(address: &str, known: &str) -> bool {
    let (Ok(a), Ok(k)) = (address.parse::<Address>(), known.parse::<Address>()) else {
        return false;
    };
    if a == k {
        return false;
    }
    let (a, k) = (
        crate::primitives::to_hex(a.as_slice(), false),
        crate::primitives::to_hex(k.as_slice(), false),
    );
    a[..4] == k[..4] && a[36..] == k[36..]
}

/// What is said, and in which tone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tone {
    Ok,
    Caution,
    Danger,
    /// Nothing is signed: no button, no override.
    Refused,
}

/// One thing the review says, besides its blocks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Concern {
    /// Not a transaction Vela signs ([`Review::refusal`]).
    NotSigned {
        why: String,
    },
    /// The state-diff guard found an undeclared change.
    StateChange,
    /// No state-diff source: only the decoding rules stand.
    StateUnread {
        reason: String,
    },
    /// The execution reverts.
    Reverts,
    /// Nothing could be simulated.
    Unsimulated,
    /// Real-device pass 2 (D1): NEITHER safety check could be read — the
    /// state-diff guard is unavailable AND nothing could be simulated — so
    /// Vela could not check what the transaction changes. A danger: it is
    /// signed only through "… anyway", under the review door's line
    /// (`home.multisig.review.state.unchecked`) and its Try again. One of
    /// the two unread alone stays a caution ([`Concern::StateUnread`] or
    /// [`Concern::Unsimulated`]).
    Unchecked,
    Lookalike {
        address: String,
        of: String,
        label: String,
    },
    FirstTime {
        address: String,
    },
    /// Coin or tokens sent to a contract.
    ToContract {
        address: String,
    },
    /// A call Vela cannot read.
    Blind,
    /// A call grants an unlimited allowance, or all of a collection
    /// (r8-1): `token` the token or collection, `spender` who may spend.
    UnlimitedApproval {
        token: String,
        spender: String,
    },
    /// A batch inside a batch (r8-5): every leg is listed, and the inner
    /// legs run from the batch contract.
    NestedBatch,
    /// A batch nested too deep, or whose legs do not read (r8-5): nothing
    /// is signed.
    UnreadableBatch,
    /// A token whose decimals are not known (r8-2): its amount is said raw.
    DecimalsUnknown {
        token: String,
    },
    /// Tokens sent to the token's own contract: they are lost.
    ToOwnToken {
        token: String,
    },
}

impl Concern {
    fn tone(&self) -> Tone {
        match self {
            Concern::NotSigned { .. } | Concern::StateChange | Concern::UnreadableBatch => {
                Tone::Refused
            }
            Concern::Reverts
            | Concern::Unchecked
            | Concern::Lookalike { .. }
            | Concern::UnlimitedApproval { .. }
            | Concern::NestedBatch
            | Concern::ToOwnToken { .. } => Tone::Danger,
            Concern::StateUnread { .. }
            | Concern::Unsimulated
            | Concern::FirstTime { .. }
            | Concern::ToContract { .. }
            | Concern::DecimalsUnknown { .. }
            | Concern::Blind => Tone::Caution,
        }
    }
}

/// Whether a signature may be given, and with what friction: `allowed`
/// false — nothing is signed, no override; `anyway` — the signing sheet's
/// own friction, a danger-styled "sign anyway".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignRule {
    pub allowed: bool,
    pub anyway: bool,
}

/// r9-9: why Vela signs nothing for a transaction — each reason its own
/// words, never another's ("would change who controls the multisig" is the
/// state guard's alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RefusedWhy {
    /// A delegatecall (the transaction's, or a batch leg's): it runs another
    /// contract's code as the multisig.
    Delegatecall,
    /// A batch nested past [`reading::MAX_NESTING`], or one whose bytes do
    /// not read in full.
    TooDeep,
    /// It pays gas or a refund out of the multisig.
    Refund,
    /// The state guard found a change it does not declare.
    StateChange,
    /// Not a kind of transaction Vela signs.
    Other,
}

impl RefusedWhy {
    /// The one line that says it (a corpus key).
    pub fn key(self) -> &'static str {
        match self {
            RefusedWhy::Delegatecall => "home.multisig.review.refusedWhy.delegatecall",
            RefusedWhy::TooDeep => "home.multisig.review.refusedWhy.tooDeep",
            RefusedWhy::Refund => "home.multisig.review.refusedWhy.refund",
            RefusedWhy::StateChange => "home.multisig.review.blocked",
            RefusedWhy::Other => "home.multisig.review.refusedWhy.other",
        }
    }

    /// The reason a [`Review::refusal`] (the core's own English) names.
    fn of_refusal(why: &str) -> RefusedWhy {
        if why.contains("delegatecall") {
            RefusedWhy::Delegatecall
        } else if why.contains("cannot read in full") {
            RefusedWhy::TooDeep
        } else if why.contains("refund") {
            RefusedWhy::Refund
        } else {
            RefusedWhy::Other
        }
    }
}

/// [`MultisigReview::refused`]: the reason, and its line's corpus key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Refused {
    pub why: RefusedWhy,
    pub key: String,
}

/// The first reason the concerns refuse for: what the transaction is
/// before what its batch reads as, before what the state guard found.
fn refused_of(concerns: &[Concern]) -> Option<RefusedWhy> {
    let not_signed = concerns.iter().find_map(|c| match c {
        Concern::NotSigned { why } => Some(RefusedWhy::of_refusal(why)),
        _ => None,
    });
    not_signed
        .or_else(|| {
            concerns
                .contains(&Concern::UnreadableBatch)
                .then_some(RefusedWhy::TooDeep)
        })
        .or_else(|| {
            concerns
                .contains(&Concern::StateChange)
                .then_some(RefusedWhy::StateChange)
        })
}

/// D1: whether neither safety check was read — the state-diff guard is
/// unavailable ([`Concern::StateUnread`]) AND nothing could be simulated
/// ([`Concern::Unsimulated`]). One of them alone is not it.
fn unchecked_of(concerns: &[Concern]) -> bool {
    concerns
        .iter()
        .any(|c| matches!(c, Concern::StateUnread { .. }))
        && concerns.contains(&Concern::Unsimulated)
}

/// Why Vela refuses `tx` for WHAT IT IS — a delegatecall, a refund, a batch
/// it cannot read in full (r9-9's reasons but the state guard's) — read from
/// the transaction alone, before any chain is asked: `None` when nothing in
/// the transaction itself refuses it. Nobody can approve such a transaction
/// on any device, so nothing counts it as waiting for someone's approval
/// (r11 S4: "9 need your approval" counted deep4 and the delegatecall).
/// Agrees with [`multisig_review`]'s `refused` for every reason but
/// [`RefusedWhy::StateChange`], which needs the chain.
#[must_use]
pub fn refused_for_what_it_is(
    chain_id: u64,
    tx: &SafeTx,
    review: &multisig::Review,
) -> Option<RefusedWhy> {
    if let Some(why) = &review.refusal {
        return Some(RefusedWhy::of_refusal(why));
    }
    reading::read(chain_id, tx, review, &BTreeMap::new())
        .unreadable
        .then_some(RefusedWhy::TooDeep)
}

/// One SafeTx field, as signed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: String,
}

/// What the shell read for [`multisig_review`] (see [`review_reads`]).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ReviewInput {
    #[serde(flatten)]
    pub reads: ReadsInput,
    /// The `eth_simulateV1` answer for `simCalls` (the signing sheet's
    /// envelope, `{"unreachable": true}` when the pool gave up); `None`: no
    /// calls to simulate, or not asked yet.
    pub sim_reply: Option<String>,
    /// The trace's and the probe's answers (envelopes).
    pub trace: Option<RpcAnswer>,
    pub probe: Option<RpcAnswer>,
    pub recipient_facts: Vec<RecipientFacts>,
    pub known: Vec<Known>,
    /// The clear reading could not read the call (`ClearSigningView.sign_anyway`).
    /// Only ever stricter: the core's own [`reading::Reading`] says a call
    /// it cannot read on every platform.
    pub blind: bool,
    /// The tokens the core read for the reading (r8-2): each one's symbol
    /// and decimals as its contract answered. The registry's are not needed
    /// here; a token missing reads as "decimals unknown".
    pub token_facts: Vec<TokenFacts>,
    /// The site that asked, for a dApp's calls.
    pub origin: Option<String>,
}

/// The one review ([`multisig_review`]).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultisigReview {
    pub review: Review,
    /// What each call does, every leg of every batch (r8-1, r8-5): draw it,
    /// never decode again.
    pub reading: reading::Reading,
    pub member_changes: Option<MemberChanges>,
    pub hashes: SafeTxHashes,
    pub fields: Vec<Field>,
    pub typed_data: Value,
    pub recipients: Vec<Recipient>,
    pub state: StateGuard,
    /// What moves. Its `kind` is the signing sheet's, or `withheld`
    /// ([`sim_withheld`]): refused for a reason other than the state
    /// guard's, nothing is drawn in its place.
    pub simulation: SimVerdict,
    /// `simulate` (`eth_simulateV1`), `probe` (the Safe's balances, read by
    /// the state probe: the coin and `tokens` only), or `none`.
    pub simulation_source: String,
    pub concerns: Vec<Concern>,
    pub tone: Tone,
    pub sign: SignRule,
    /// r9-9: why nothing is signed (`sign.allowed` false), as ONE line — its
    /// corpus key. Draw it in place of any other refusal words; `None` when
    /// the transaction may be signed.
    pub refused: Option<Refused>,
    /// r11 S4: the heading over what moves, when anything is drawn there
    /// ([`MOVES_TITLE_KEY`], "Balance changes" — the signing sheet's own),
    /// `None` when the simulation is withheld. Every surface draws this key
    /// (Android drew "SIMULATION RESULT").
    #[serde(default)]
    pub moves_title_key: Option<String>,
    /// What `agent_review` reads as the request on screen (a `SignFacts`):
    /// an opinion is bound to this proposal on this chain.
    pub advice_request: Value,
}

/// The heading over a review's balance changes ([`MultisigReview::moves_title_key`]).
pub const MOVES_TITLE_KEY: &str = "componentsUi.signing.balanceChangesTitle";

fn sim_unavailable() -> SimVerdict {
    sim_outcome::verdict_of(SimOutcome::NotOffered)
}

/// The kind of [`MultisigReview::simulation`] when the transaction is
/// refused for anything but the state guard's own finding
/// ([`MultisigReview::refused`] `why` ≠ `stateChange`).
pub const SIM_WITHHELD: &str = "withheld";

/// What moves, withheld: no deltas, no notice, no "No asset changes" — a
/// shell draws nothing in the simulation's place (r9-9, the state guard's
/// [`StateVerdict::Withheld`] for the balances). Only the multisig review
/// says it; the signing sheet's [`SimVerdict`] never has this kind.
#[must_use]
pub fn sim_withheld() -> SimVerdict {
    SimVerdict {
        kind: SIM_WITHHELD.to_owned(),
        deltas: Vec::new(),
        revert_reason: None,
        notice_risk: None,
        notice_key: None,
        no_change_key: None,
        recheck: false,
        source: None,
        source_key: None,
    }
}

/// The Safe's own moves, read from the probe's balances.
fn probe_moves(p: &ProbeAnswer, tokens: &[Address]) -> SimVerdict {
    if !p.success {
        let reason = sim_outcome::revert_reason(&json!({ "returnData": hex(&p.ret) }));
        return sim_outcome::verdict_of(SimOutcome::Reverts { reason });
    }
    let signed = |b: U256, a: U256| {
        if a >= b {
            (a - b).to_string()
        } else {
            format!("-{}", b - a)
        }
    };
    let mut deltas = Vec::new();
    if p.before.balance != p.after.balance {
        deltas.push(TrustAssetDelta {
            kind: TrustDeltaKind::Native,
            token: None,
            delta: signed(p.before.balance, p.after.balance),
        });
    }
    for (i, t) in tokens.iter().enumerate() {
        if let (Some(Some(b)), Some(Some(a))) = (p.before.tokens.get(i), p.after.tokens.get(i)) {
            if b != a {
                deltas.push(TrustAssetDelta {
                    kind: TrustDeltaKind::Erc20,
                    token: Some(format!("{t:#x}")),
                    delta: signed(*b, *a),
                });
            }
        }
    }
    sim_outcome::verdict_of(SimOutcome::Deltas { deltas })
}

/// The probe's answer, and the second node's beside it (R22).
struct Probed {
    answer: ProbeAnswer,
    witness: Option<ProbeAnswer>,
    own_node: bool,
}

/// What the probe's balances say moves, believed only when a second,
/// independent node's balances agree (or the person's own node answered
/// alone): otherwise "couldn't confirm" — never "nothing moves" on one
/// node's word.
fn probe_moves_corroborated(p: &Probed, tokens: &[Address]) -> SimVerdict {
    let first = probe_moves(&p.answer, tokens);
    if first.kind != "deltas" {
        return first;
    }
    if p.own_node {
        let mut v = first;
        v.source = Some("own_node".into());
        v.source_key = Some(sim_outcome::KEY_OWN_NODE.into());
        return v;
    }
    let unconfirmed = |disagreed| sim_outcome::verdict_of(SimOutcome::Unconfirmed { disagreed });
    let Some(witness) = &p.witness else {
        return unconfirmed(false);
    };
    let theirs = probe_moves(witness, tokens);
    if theirs.kind != "deltas" {
        return unconfirmed(theirs.kind == "reverts");
    }
    match sim_outcome::agreed_deltas(&first.deltas, &theirs.deltas) {
        Some(deltas) => {
            let mut v = sim_outcome::verdict_of(SimOutcome::Deltas { deltas });
            v.source = Some("corroborated".into());
            v
        }
        None => unconfirmed(true),
    }
}

/// R22: one state source's reading — the first node's verdict and the
/// second, independent node's, either of which may be missing.
struct StateRead {
    first: Option<StateVerdict>,
    second: Option<StateVerdict>,
    own_node: bool,
    /// §39 (r22-2): one node, on a network whose pool holds no other.
    sole_provider: bool,
}

impl StateRead {
    /// The source's reading of envelope `e` (`first` read from its first
    /// answer; `again` reads the witness's); `None` when neither node gave
    /// a verdict.
    fn of(
        e: &Value,
        first: Option<StateVerdict>,
        again: impl Fn(&Value) -> Option<StateVerdict>,
    ) -> Option<Self> {
        let second = corroborate::witness_of(e).and_then(again);
        (first.is_some() || second.is_some()).then(|| Self {
            first,
            second,
            own_node: corroborate::own_node(e),
            sole_provider: corroborate::sole_provider(e),
        })
    }

    /// What stands: a refusal (or a revert) EITHER node read — a lying node
    /// cannot hide the honest one's "this changes who controls it" — else
    /// the first node's verdict when the person's own node gave it alone or
    /// the second node read the same; else nothing.
    fn stands(&self) -> Option<StateVerdict> {
        if let Some(v) = self
            .first
            .iter()
            .chain(&self.second)
            .find(|v| refuses_alone(v))
        {
            return Some(v.clone());
        }
        let first = self.first.as_ref()?;
        (self.own_node || self.second.as_ref() == Some(first)).then(|| first.clone())
    }

    /// Who stands behind it (the could-not-check's reason).
    fn standing(&self) -> Standing {
        match (&self.first, &self.second) {
            _ if self.own_node => Standing::OwnNode,
            (Some(a), Some(b)) if a == b => Standing::Corroborated,
            (Some(_), Some(_)) => Standing::Disagreed,
            _ => Standing::Single,
        }
    }
}

/// A state verdict that needs no second word: a refusal (an undeclared
/// change) or a revert. A lying node can only make Vela refuse or warn by
/// it, never sign plainly.
fn refuses_alone(v: &StateVerdict) -> bool {
    matches!(v, StateVerdict::Undeclared { .. } | StateVerdict::Reverts)
}

/// The one review of `input.tx` for every member ([`MultisigReview`]).
pub fn multisig_review(input: &ReviewInput) -> Result<MultisigReview, CoreError> {
    let r = &input.reads;
    let tx = r.tx.as_ref().ok_or_else(|| err("tx"))?;
    let safe = addr(&r.safe)?;
    let review = multisig::review(r.chain_id, &r.safe, tx)?;
    let read = reading::read(
        r.chain_id,
        tx,
        &review,
        &input
            .token_facts
            .iter()
            .filter_map(|t| Some((t.token.clone()?.to_ascii_lowercase(), t.clone())))
            .collect(),
    );
    let hashes = multisig::safe_tx_hashes(r.chain_id, &r.safe, tx)?;
    let typed_data = multisig::safe_tx_typed_data(r.chain_id, &r.safe, tx)?;
    let member_changes = match &review.action {
        Some(a) => multisig_reading::member_changes(a, &r.owners, r.threshold)?,
        None => None,
    };
    let field = |name: &str, value: String| Field {
        name: name.into(),
        value,
    };
    let dec = |s: &str| quantity(s).map_or_else(|| s.to_owned(), |v| v.to_string());
    let fields = vec![
        field("to", checksum(addr(&tx.to)?)),
        field("value", dec(&tx.value)),
        field("data", tx.data.clone()),
        field("operation", tx.operation.to_string()),
        field("safeTxGas", dec(&tx.safe_tx_gas)),
        field("baseGas", dec(&tx.base_gas)),
        field("gasPrice", dec(&tx.gas_price)),
        field("gasToken", checksum(addr(&tx.gas_token)?)),
        field("refundReceiver", checksum(addr(&tx.refund_receiver)?)),
        field("nonce", dec(&tx.nonce)),
    ];

    // The guard.
    let reads = review_reads(r)?;
    let tokens: Vec<Address> = reads.tokens.iter().filter_map(|t| t.parse().ok()).collect();
    let mut probe_read: Option<Probed> = None;
    let state = match placed(r, safe) {
        Err(p) => StateGuard {
            source: StateSource::None,
            verdict: StateVerdict::Unavailable {
                reason: p.unavailable.unwrap_or_else(|| "unreadable".into()),
            },
        },
        Ok(placed) => {
            let (owners, threshold, named, _) =
                declared(review.action.as_ref(), &placed.owners, placed.threshold);
            let e = Expect {
                before: placed.owners.clone(),
                owners,
                threshold,
                before_threshold: U256::from(placed.threshold),
            };
            let watch = watched(tx, &placed.owners, &named);
            let verdict = |writes: Vec<CriticalWrite>, declares: bool| {
                if !writes.is_empty() {
                    StateVerdict::Undeclared { writes }
                } else if declares {
                    StateVerdict::Declared
                } else {
                    StateVerdict::Unchanged
                }
            };
            let declares = e.owners != e.before.iter().copied().collect::<BTreeSet<_>>()
                || e.threshold != e.before_threshold;
            // A trace counts only when `execTransaction` ran: it always writes the
            // Safe (its nonce, at least). A reverted call leaves an EMPTY diff —
            // never "nothing changes" — and the probe speaks instead.
            let trace_of = |t: &Value| {
                let res = t.get("result").filter(|v| !v.is_null())?;
                let w = trace_writes(res, safe)?;
                (!w.is_empty()).then(|| verdict(judge_trace(&w, &e, &watch), declares))
            };
            let probe_of = |p: &Value| {
                let answer = probe_answer(p, watch.len(), tokens.len())?;
                let v = if !answer.success {
                    StateVerdict::Reverts
                } else {
                    match judge_probe(&answer, &e, &watch) {
                        Some(w) => verdict(w, declares),
                        None => StateVerdict::Unavailable {
                            reason: "unreadable".into(),
                        },
                    }
                };
                Some((v, answer))
            };
            // R22 (review 1-2): each source is the first node's answer, and
            // the second, independent node's beside it. Their verdicts are
            // compared — never their bytes (two honest clients list
            // different accounts in a prestate diff).
            let traced = input.trace.as_ref().and_then(|t| {
                StateRead::of(t, trace_of(&corroborate::first_answer(t)), |w| trace_of(w))
            });
            let probed = input.probe.as_ref().and_then(|p| {
                let first = probe_of(&corroborate::first_answer(p));
                let witness = corroborate::witness_of(p).and_then(&probe_of);
                if let Some((_, answer)) = &first {
                    probe_read = Some(Probed {
                        answer: answer.clone(),
                        witness: witness.as_ref().map(|(_, a)| a.clone()),
                        own_node: corroborate::own_node(p),
                    });
                }
                StateRead::of(p, first.map(|(v, _)| v), |w| probe_of(w).map(|(v, _)| v))
            });
            let stands = |found: &Option<StateRead>| found.as_ref().and_then(StateRead::stands);
            match (stands(&traced), stands(&probed)) {
                (Some(v), _) => StateGuard {
                    source: StateSource::Trace,
                    verdict: v,
                },
                (None, Some(v)) => StateGuard {
                    source: StateSource::Probe,
                    verdict: v,
                },
                // An answer nobody independent confirmed: unread, and why.
                (None, None) if traced.is_some() || probed.is_some() => {
                    let disagreed = [&traced, &probed]
                        .into_iter()
                        .flatten()
                        .any(|read| read.standing() == Standing::Disagreed);
                    // §39 (r22-2): one node, where the pool holds no other
                    // provider — said "only one node could check", with the
                    // way to add one's own.
                    let one_node = [&traced, &probed]
                        .into_iter()
                        .flatten()
                        .all(|read| read.sole_provider);
                    StateGuard {
                        source: StateSource::None,
                        verdict: StateVerdict::Unavailable {
                            reason: if disagreed {
                                corroborate::DISAGREED
                            } else if one_node {
                                corroborate::ONE_NODE
                            } else {
                                corroborate::UNCORROBORATED
                            }
                            .into(),
                        },
                    }
                }
                (None, None) => {
                    let all: Vec<&RpcAnswer> =
                        [&input.trace, &input.probe].into_iter().flatten().collect();
                    let reason = if all.is_empty() {
                        "notAsked"
                    } else if all.iter().all(|a| unreachable(a)) {
                        "unreachable"
                    } else if all.iter().all(|a| not_offered(a) || unreachable(a)) {
                        "notOffered"
                    } else {
                        "unreadable"
                    };
                    StateGuard {
                        source: StateSource::None,
                        verdict: StateVerdict::Unavailable {
                            reason: reason.into(),
                        },
                    }
                }
            }
        }
    };

    // §39 (R22 follow-up, r22-2): on a network whose pool holds no other
    // provider, a state check nobody could read is said as what it is —
    // "only one node could check" — with the way to add one's own node;
    // never "checked". (A disagreement stays a disagreement.)
    let one_node = [&input.trace, &input.probe]
        .into_iter()
        .flatten()
        .any(corroborate::sole_provider)
        || input
            .sim_reply
            .as_deref()
            .and_then(|r| serde_json::from_str::<Value>(r).ok())
            .is_some_and(|v| corroborate::sole_provider(&v));
    let state = match state {
        StateGuard {
            source,
            verdict: StateVerdict::Unavailable { reason },
        } if one_node && reason != corroborate::DISAGREED && reason != "unreachable" => {
            StateGuard {
                source,
                verdict: StateVerdict::Unavailable {
                    reason: corroborate::ONE_NODE.into(),
                },
            }
        }
        other => other,
    };

    // What moves: the signing sheet's simulation; where the network does
    // not offer one, the probe's balances.
    // R22: the simulation's envelope carries the second node's answer; what
    // moves is believed only when the two agree (`sim_outcome`).
    let simulated = input
        .sim_reply
        .as_deref()
        .map(|reply| sim_outcome::verdict_of_envelope_json(reply, &r.safe));
    let (simulation, simulation_source) = match (simulated, &probe_read) {
        (Some(v), _) if v.kind == "deltas" || v.kind == "reverts" => (v, "simulate"),
        (_, Some(p)) => (probe_moves_corroborated(p, &tokens), "probe"),
        (Some(v), None) => (v, "none"),
        (None, None) => (sim_unavailable(), "none"),
    };
    // r9-2: a native coin with an ERC-20 interface is one balance, said once.
    let simulation = sim_outcome::one_line_per_balance(r.chain_id, simulation);

    // Recipients.
    let recipients: Vec<Recipient> = recipients_of(tx, &review)
        .into_iter()
        .map(|(call, role, address)| {
            let facts = input
                .recipient_facts
                .iter()
                .find(|f| f.address.eq_ignore_ascii_case(&address));
            let lookalike = input
                .known
                .iter()
                .find(|k| looks_like(&address, &k.address))
                .cloned();
            let known = known_as(&input.known, &address);
            let trusted = known.as_ref().is_some_and(|k| k.kind.trusted());
            let is_contract = facts.and_then(|f| f.is_contract);
            let first_time = facts.and_then(|f| f.first_time);
            let paid = role == RecipientRole::Recipient && !trusted;
            Recipient {
                call,
                role,
                address,
                is_contract,
                first_time,
                lookalike,
                known,
                first_time_cue: paid && first_time == Some(true),
                contract_cue: paid && is_contract == Some(true),
            }
        })
        .collect();

    // What is said, and the rule.
    let mut concerns = Vec::new();
    if let Some(why) = &review.refusal {
        concerns.push(Concern::NotSigned { why: why.clone() });
    }
    match &state.verdict {
        StateVerdict::Undeclared { .. } => concerns.push(Concern::StateChange),
        StateVerdict::Unavailable { reason } => concerns.push(Concern::StateUnread {
            reason: reason.clone(),
        }),
        _ => {}
    }
    match simulation.kind.as_str() {
        "reverts" => concerns.push(Concern::Reverts),
        "deltas" => {}
        _ => {
            if state.verdict == StateVerdict::Reverts {
                concerns.push(Concern::Reverts);
            } else {
                concerns.push(Concern::Unsimulated);
            }
        }
    }
    let mut seen = BTreeSet::new();
    for rc in &recipients {
        if !seen.insert((rc.address.to_lowercase(), rc.role as u8)) {
            continue;
        }
        if let Some(k) = &rc.lookalike {
            concerns.push(Concern::Lookalike {
                address: rc.address.clone(),
                of: k.address.clone(),
                label: k.label.clone(),
            });
        }
        if rc.first_time_cue {
            concerns.push(Concern::FirstTime {
                address: rc.address.clone(),
            });
        }
        if rc.contract_cue {
            concerns.push(Concern::ToContract {
                address: rc.address.clone(),
            });
        }
    }
    // What each call does (r8-1): unlimited grants, nested batches, a call
    // nobody reads, decimals unknown — the same on every platform.
    concerns.extend(reading::concerns(&read));
    let blind = input.blind || read.blind;
    if input.blind {
        concerns.push(Concern::Blind);
    }
    // r8-14: each thing said once.
    let mut said = Vec::new();
    concerns.retain(|c| {
        if said.contains(c) {
            false
        } else {
            said.push(c.clone());
            true
        }
    });
    // r9-9: refused for what it is, the guard's reassurance or its
    // could-not-check is not said beside it.
    let refused = refused_of(&concerns);
    let withheld = refused.is_some_and(|w| w != RefusedWhy::StateChange);
    let mut state = state;
    if withheld
        && matches!(
            state.verdict,
            StateVerdict::Unchanged
                | StateVerdict::Declared
                | StateVerdict::Unavailable { .. }
                | StateVerdict::Reverts
        )
    {
        state.verdict = StateVerdict::Withheld;
        concerns.retain(|c| !matches!(c, Concern::StateUnread { .. }));
    }
    // The same for what moves: a delegatecall runs another contract's code
    // as the multisig, a batch too deep is not read in full — "No asset
    // changes" (or its could-not-check, or a revert only the simulation
    // said) beside "Vela won't sign this" is a reassurance nobody can give.
    // r10-6: nor "expected to fail" — the probe of a transaction Vela does
    // not read in full says nothing it can stand behind; the refusal is the
    // one line (deep4 still showed it on desktop and Android).
    let simulation = if withheld {
        concerns.retain(|c| !matches!(c, Concern::Unsimulated | Concern::Reverts));
        sim_withheld()
    } else {
        simulation
    };
    // D1 (real-device pass 2): neither check read — not the guard, not the
    // simulation (a slow network gives each read up at its bound) — is not
    // a caution any more: Vela could not check what it changes at all. It
    // is said (the door's danger line, with Try again) and signed only
    // "… anyway". Either one read keeps today's caution.
    if unchecked_of(&concerns) {
        concerns.push(Concern::Unchecked);
    }
    let tone = concerns.iter().map(Concern::tone).max().unwrap_or(Tone::Ok);
    // R22 (review 1-3): ANY safety check unread — the state diff, or what
    // moves — is "… anyway", not only both. One of two read used to sign
    // with a plain press.
    let unread = concerns
        .iter()
        .any(|c| matches!(c, Concern::StateUnread { .. } | Concern::Unsimulated));
    let allowed = tone != Tone::Refused;
    let sign = SignRule {
        allowed,
        // A refusal has no "… anyway": nothing is signed, no override.
        anyway: allowed && (tone == Tone::Danger || blind || unread),
    };
    // One refusal box (r10 follow-up): a batch too deep is refused for
    // being unreadable, so "Part of this batch can't be read" beside "Vela
    // won't sign… nested too deep" is the same reason twice. The refusal
    // (`refused`) is the one line; the concern it came from leaves the list
    // here, at the source, after it decided the tone — no shell filters it.
    if refused == Some(RefusedWhy::TooDeep) {
        concerns.retain(|c| *c != Concern::UnreadableBatch);
    }
    let refused = refused.map(|why| Refused {
        why,
        key: why.key().to_owned(),
    });

    let advice_request = json!({
        "id": format!("multisig:{}:{}", r.chain_id, hashes.safe_tx_hash),
        "method": "vela_multisigApprove",
        "kind": "transaction",
        "params_json": json!({
            "chainId": r.chain_id,
            "safe": checksum(safe),
            "safeTxHash": hashes.safe_tx_hash,
            "safeTx": tx,
            "reading": review.action,
            "refusal": review.refusal,
            "memberChanges": member_changes,
            "state": state,
        }).to_string(),
        "origin": input.origin.clone().unwrap_or_default(),
        "chain_id": r.chain_id,
        "signer_address": checksum(safe),
    });

    let moves_title_key = (simulation.kind != SIM_WITHHELD).then(|| MOVES_TITLE_KEY.to_owned());
    Ok(MultisigReview {
        review,
        reading: read,
        member_changes,
        hashes,
        fields,
        typed_data,
        recipients,
        state,
        simulation,
        simulation_source: simulation_source.into(),
        concerns,
        tone,
        sign,
        refused,
        moves_title_key,
        advice_request,
    })
}

// ---------------------------------------------------------------------------
// The signing gate
// ---------------------------------------------------------------------------

/// What the shell knows right before a signature: the findings the chain
/// gave just now on `chainId` (`{from: "genesis" | "chain", chainId, …}` —
/// `None`: the chain did not answer), and the review's sign rule for the
/// transaction about to be signed, when there is one.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GateInput {
    pub chain_id: u64,
    pub findings: Option<Vec<Value>>,
    pub sign: Option<SignRule>,
}

/// Whether this device signs, right now, for a multisig on a network.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Gate {
    /// Signed as always.
    Open,
    /// Who can act for it could not be read: nothing is signed yet.
    Unchecked,
    /// Its genesis is refused here: read-only, nothing is signed, ever.
    Refused { findings: Vec<Value> },
    /// This transaction is one Vela does not sign (not readable, or it
    /// changes who controls the multisig undeclared): no override.
    Blocked,
    /// Something besides its members can act for it here: each signature asks.
    Risk { findings: Vec<Value> },
}

/// The gate ([`Gate`]), in this order: unread, refused network, refused
/// transaction, risk, open.
pub fn signing_gate(input: &GateInput) -> Gate {
    let Some(found) = &input.findings else {
        return Gate::Unchecked;
    };
    let here: Vec<&Value> = found
        .iter()
        .filter(|f| f.get("chainId").and_then(Value::as_u64) == Some(input.chain_id))
        .collect();
    let of = |from: &str| -> Vec<Value> {
        here.iter()
            .filter(|f| f.get("from").and_then(Value::as_str) == Some(from))
            .map(|f| (*f).clone())
            .collect()
    };
    let refused = of("genesis");
    if !refused.is_empty() {
        return Gate::Refused { findings: refused };
    }
    if input.sign.is_some_and(|s| !s.allowed) {
        return Gate::Blocked;
    }
    let risks = of("chain");
    if !risks.is_empty() {
        return Gate::Risk { findings: risks };
    }
    Gate::Open
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "multisig_review_tests.rs"]
mod tests;
