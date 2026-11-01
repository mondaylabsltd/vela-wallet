//! Machine — a multisig's members, network by network (F0.4): who they were
//! at its creation, who they are on each network now, and the person's change
//! to them — add, replace, remove, the approvals needed — checked on every
//! network it is deployed on before anything is signed.
//!
//! A port of the web's `member-changes.ts`, `members.ts`, `member-entry.ts`,
//! `names.svelte.ts` (`keptMember`), `shown.ts` (`currentRules`, `ruleChip`)
//! and the logic of `MemberChanges.svelte` / `MultisigSettings.svelte`, rule
//! for rule (docs/multisig-vault-design.md §3.4, §7, 实现状态 "成员管理"):
//!
//! - **members are each network's** (§3.4): the genesis holds the original
//!   members; on each network the multisig is used on, the members there now
//!   are read from that chain through the wallet's own RPC
//!   ([`Chain::read_members`]), never the service's. A network whose read
//!   does not answer within [`MEMBERS_READ_MS`] (or answers nothing) is said
//!   as unreadable — never as "the same as the original members";
//! - **one action, one proposal per DEPLOYED network** (§7): the change is
//!   built by the core (`member_change_tx`) against THAT network's current
//!   members — its own list order, so the previous owner comes from it — and
//!   where the core refuses it (too many approvals for the members left,
//!   someone already a member, the multisig itself, a member change already
//!   waiting there…) the refusal is that network's, said in one sentence. A
//!   network it is not deployed on "starts with the original members": said,
//!   and nothing that would deploy it is offered;
//! - **the changes waiting** (the proposals machine's open proposals,
//!   [`Event::Waiting`]) read as one change per action, with each network's
//!   progress ("Arc 1/2 · Base 0/2"), and a network with a member change
//!   already waiting refuses another;
//! - **who joins** is typed as an address, an ENS name (resolved forward by
//!   the core's walk, the address always shown beside it), or picked from this
//!   device's accounts and contacts. Names and kinds are labels only, kept on
//!   this device (`Notice::Account { update: AccountUpdate::NameMembers }` to
//!   the accounts machine, `typed`), never authority.
//!
//! **Proposing is not here.** The view says, per deployed network, the
//! proposal to make there ([`PlanNetwork::proposal`]: a
//! [`ProposalKind::MemberChange`] carrying the [`MemberChange`]); the shell
//! dispatches it as it is into the proposals machine's
//! `Event::Draft { chain_id, proposal }` (`chain_id` and `proposal` both
//! taken from the [`PlanNetwork`]), which builds the transaction AGAIN
//! against that chain at propose time (its members and nonce read then) —
//! the plan here is the picture shown before anything is signed, not the
//! transaction.

use std::collections::BTreeMap;

use crux_core::{render::render, App, Command};
use futures::future::{self, Either};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_proposals::ProposalKind;
use super::multisig_shell::{
    checksummed, keys, local_owners, lock, same_address, AccountUpdate, Chain, ChainMembers, Io,
    LocalAccount, MemberInfo, MultisigAccount, MultisigEffect, Notice, Shared,
};
use super::multisig_signers::{self as signers, KindChoice, SignerFacts, SignerRead};
use super::name_verify::{self, ResolveState, ResolveStep};
use crate::multisig::{self, Action, MemberChange, MemberRefusal, MemberState, MemberVerdict};
use crate::primitives::{address_entry, AddressEntry};
use crate::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};

/// How long one network's members are waited for: past it, the network is
/// said as unreadable (the web's `PREVIEW_READ_MS`).
pub const MEMBERS_READ_MS: u64 = 8_000;
/// r10 (web: "Couldn't read its members" stayed after the RPC recovered): a
/// network whose members did not answer is read again on its own, after
/// each of these waits, while it is still shown — the machine decides when
/// to look again, no shell has to. A `refresh` or `show` starts it over.
pub const MEMBERS_REREAD_MS: [u64; 6] = [10_000, 20_000, 40_000, 60_000, 60_000, 60_000];
/// At most this many suggestions under the entry.
pub const MAX_SUGGESTIONS: usize = 5;
/// A walk is a handful of rounds; this only stops a contract bug from spinning.
const WALK_ROUNDS: usize = 16;
/// ENS on Ethereum: every name but a Basename.
pub const ENS_REGISTRY: &str = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";
/// Basenames on Base (`*.base.eth`).
pub const BASENAME_REGISTRY: &str = "0xb94704422c2a1e396835a571837aa5ae53285a95";
const ETHEREUM: u32 = 1;
const BASE: u32 = 8453;

// ---------------------------------------------------------------------------
// What the proposals machine says is waiting
// ---------------------------------------------------------------------------

/// Where an open proposal stands (`ProposalStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingStatus"))]
pub enum WaitingStatus {
    /// Enough approvals, its nonce next.
    Ready,
    /// Waiting for approvals, its nonce next.
    Pending,
    /// Behind an earlier nonce.
    Queued,
    /// Its nonce is used: executed, or voided.
    Done,
}

/// One proposal the proposals machine reads (`ProposalItem`), as much of it
/// as the members need: where it is, what the core read it as, and how far
/// its approvals have come.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigWaitingProposal")
)]
pub struct WaitingProposal {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
    /// Decimal.
    #[serde(default)]
    pub nonce: String,
    /// The core's `Review.action` (`None`: not one Vela signs).
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub action: Option<Action>,
    /// Approvals that count.
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub approved: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    pub status: WaitingStatus,
}

// ---------------------------------------------------------------------------
// Pure rules (`member-changes.ts`, `members.ts`, `member-entry.ts`, `shown.ts`)
// ---------------------------------------------------------------------------

/// Whether `action` changes who controls the multisig (alone, or a call of a
/// batch) — `changesMembers`.
pub fn changes_members(action: Option<&Action>) -> bool {
    match action {
        None => false,
        Some(Action::Batch { calls }) => calls.iter().any(|c| changes_members(Some(&c.action))),
        Some(a) => matches!(
            a,
            Action::AddOwner { .. }
                | Action::RemoveOwner { .. }
                | Action::SwapOwner { .. }
                | Action::ChangeThreshold { .. }
        ),
    }
}

/// A network's members as the core builds a change against them: its owners
/// in the chain's list order, and whether a member change already waits
/// there (`items`: that network's open proposals) — `memberStateOf`.
pub fn member_state_of(members: &ChainMembers, items: &[WaitingProposal]) -> MemberState {
    MemberState {
        owners: members.owners.clone(),
        threshold: members.threshold,
        change_waiting: items
            .iter()
            .any(|i| i.status != WaitingStatus::Done && changes_members(i.action.as_ref())),
    }
}

/// A network as a plan reads it: its members now, and what waits there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkMembers {
    pub chain_id: u64,
    pub members: ChainMembers,
    #[serde(default)]
    pub items: Vec<WaitingProposal>,
}

/// One deployed network's part of a plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigPlanNetwork"))]
pub struct PlanNetwork {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    /// Before → after where the core builds it; its refusal where it does not.
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub verdict: MemberVerdict,
    /// The proposal to make on this network (where the core built it):
    /// always a [`ProposalKind::MemberChange`]. The shell dispatches it as
    /// it is — `{ type: "draft", chain_id, proposal }`, both fields from
    /// this row — into the proposals machine's `Event::Draft`, which builds
    /// the transaction again against the chain then.
    pub proposal: Option<ProposalKind>,
}

/// What a change would be on each network (`MemberPlan`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberPlan"))]
pub struct MemberPlan {
    /// Each network it is deployed on, and what the change is there (or why not).
    pub networks: Vec<PlanNetwork>,
    /// The networks a proposal goes to: those where the core built it.
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub ready: Vec<u64>,
    /// Networks it is not deployed on: they start with the original members;
    /// nothing is offered there.
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub undeployed: Vec<u64>,
    /// Networks whose members could not be read: nothing is said or offered
    /// there but that.
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub unreadable: Vec<u64>,
}

/// What `change` would be on each network, as shown before anything is
/// signed (`planMemberChange`). The nonce is the proposal's to take when it
/// is made; this is the picture, not the transaction.
pub fn plan_member_change(
    multisig: &str,
    chains: &[NetworkMembers],
    change: &MemberChange,
) -> MemberPlan {
    let networks: Vec<PlanNetwork> = chains
        .iter()
        .filter(|c| c.members.deployed)
        .map(|c| {
            let state = member_state_of(&c.members, &c.items);
            let verdict = multisig::member_change_tx(multisig, &state, change, "0x0")
                .unwrap_or_else(|_| MemberVerdict::Refused {
                    refusal: MemberRefusal::NotAnAddress {
                        address: multisig.to_owned(),
                    },
                });
            let proposal = matches!(verdict, MemberVerdict::Ready { .. }).then(|| {
                ProposalKind::MemberChange {
                    change: change.clone(),
                }
            });
            PlanNetwork {
                chain_id: c.chain_id,
                verdict,
                proposal,
            }
        })
        .collect();
    MemberPlan {
        ready: networks
            .iter()
            .filter(|n| n.proposal.is_some())
            .map(|n| n.chain_id)
            .collect(),
        undeployed: chains
            .iter()
            .filter(|c| !c.members.deployed)
            .map(|c| c.chain_id)
            .collect(),
        networks,
        unreadable: Vec::new(),
    }
}

/// Everyone who is a member on some network it is deployed on, in the first
/// network's order and then whoever only another has (`deployedMembers`).
pub fn deployed_members(chains: &[&ChainMembers]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in chains.iter().filter(|c| c.deployed) {
        for o in &c.owners {
            if !out.iter().any(|x| same_address(x, o)) {
                out.push(o.clone());
            }
        }
    }
    out
}

/// One change as the person asked for it, whatever network it waits on
/// (`changeKey`); `None`: not a member change on its own.
pub fn change_key(action: Option<&Action>) -> Option<String> {
    match action? {
        Action::AddOwner { owner, .. } => Some(format!("add:{}", owner.to_ascii_lowercase())),
        Action::RemoveOwner { owner, .. } => Some(format!("remove:{}", owner.to_ascii_lowercase())),
        Action::SwapOwner {
            old_owner,
            new_owner,
        } => Some(format!(
            "replace:{}:{}",
            old_owner.to_ascii_lowercase(),
            new_owner.to_ascii_lowercase()
        )),
        Action::ChangeThreshold { threshold } => Some(format!("threshold:{threshold}")),
        _ => None,
    }
}

/// One network a waiting change is on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigChangePart"))]
pub struct ChangePart {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    /// The proposal to open for this network.
    pub safe_tx_hash: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub approved: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    /// The approvals shown: never more than the threshold ("3/2" never, r4b-17).
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub shown: u64,
    pub status: WaitingStatus,
}

/// The member changes of one action, waiting (`ChangeGroup`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigChangeGroup"))]
pub struct ChangeGroup {
    pub key: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub action: Action,
    /// One per network it waits on, in the wallet's network order.
    pub parts: Vec<ChangePart>,
}

/// The member changes still open, one group per change; each group's
/// networks in `order`, whichever was proposed first (`memberChangeGroups`).
pub fn member_change_groups(items: &[WaitingProposal], order: &[u64]) -> Vec<ChangeGroup> {
    let rank = |id: u64| order.iter().position(|o| *o == id).unwrap_or(order.len());
    let mut groups: Vec<ChangeGroup> = Vec::new();
    for i in items {
        if i.status == WaitingStatus::Done {
            continue;
        }
        let (Some(key), Some(action)) = (change_key(i.action.as_ref()), i.action.as_ref()) else {
            continue;
        };
        let part = ChangePart {
            chain_id: i.chain_id,
            safe_tx_hash: i.safe_tx_hash.clone(),
            approved: i.approved,
            threshold: i.threshold,
            shown: i.approved.min(i.threshold),
            status: i.status,
        };
        match groups.iter_mut().find(|g| g.key == key) {
            Some(g) => g.parts.push(part),
            None => groups.push(ChangeGroup {
                key,
                action: action.clone(),
                parts: vec![part],
            }),
        }
    }
    for g in &mut groups {
        g.parts.sort_by_key(|p| rank(p.chain_id));
    }
    groups
}

/// Whether a member change open in `was` is done in `now`, or no longer
/// listed there: its network's members may have moved.
pub fn member_change_ended(was: &[WaitingProposal], now: &[WaitingProposal]) -> bool {
    was.iter()
        .filter(|w| w.status != WaitingStatus::Done && changes_members(w.action.as_ref()))
        .any(|w| {
            !now.iter().any(|n| {
                n.chain_id == w.chain_id
                    && n.safe_tx_hash.eq_ignore_ascii_case(&w.safe_tx_hash)
                    && n.status != WaitingStatus::Done
            })
        })
}

/// "Arc 1/2 · Base 0/2": each network's approvals so far, of what it needs
/// there (`progressLine`).
pub fn progress_line(group: &ChangeGroup, chain_name: impl Fn(u64) -> String) -> String {
    group
        .parts
        .iter()
        .map(|p| format!("{} {}/{}", chain_name(p.chain_id), p.shown, p.threshold))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Why a typed address cannot join `owners` (`MemberProblem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberProblem"))]
pub enum MemberProblem {
    InvalidAddress,
    /// Mixed case that is not its checksum: a typo or an altered copy.
    Checksum,
    /// The zero address or Safe's sentinel: `setup` refuses both.
    NotMember,
    Duplicate,
}

/// Why `typed` cannot join `owners`, or `None` when it can (`memberProblem`).
pub fn member_problem(typed: &str, owners: &[String]) -> Option<MemberProblem> {
    let value = typed.trim();
    match address_entry(value) {
        AddressEntry::BadChecksum => return Some(MemberProblem::Checksum),
        AddressEntry::Malformed => return Some(MemberProblem::InvalidAddress),
        AddressEntry::Valid => {}
    }
    if !multisig::valid_owner(value) {
        return Some(MemberProblem::NotMember);
    }
    if owners.iter().any(|o| same_address(o, value)) {
        return Some(MemberProblem::Duplicate);
    }
    None
}

/// Whether `typed` reads as a name to resolve rather than an address
/// (`looksLikeName`): no `0x`, two or more dot-separated labels, no space.
pub fn looks_like_name(typed: &str) -> bool {
    let value = typed.trim();
    if value.starts_with("0x") {
        return false;
    }
    let labels: Vec<&str> = value.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && !l.chars().any(char::is_whitespace))
}

/// Where a typed name is looked up: Basenames on Base, every other name ENS
/// on Ethereum.
pub fn name_service(name: &str) -> (u32, &'static str) {
    if name.trim().to_lowercase().ends_with(".base.eth") {
        (BASE, BASENAME_REGISTRY)
    } else {
        (ETHEREUM, ENS_REGISTRY)
    }
}

/// "M of N", and whether it is a multisig of one ("One member", never "1 of 1").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberRule"))]
pub struct MemberRule {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub m: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub n: u64,
    /// One member: its words are `titleSolo` / `chipSolo`.
    pub solo: bool,
}

impl MemberRule {
    pub fn of(threshold: u64, owners: usize) -> Self {
        let n = owners as u64;
        Self {
            m: threshold,
            n,
            solo: n == 1,
        }
    }
}

/// The rule in force on each network the multisig is used on (r4D-7): its
/// members there now where it is deployed, its original ones where it is not
/// — or the original alone before any network is read. Distinct rules, in
/// the networks' order (`currentRules`).
pub fn current_rules(original: MemberRule, chains: &[&ChainMembers]) -> Vec<MemberRule> {
    if chains.is_empty() {
        return vec![original];
    }
    let mut out: Vec<MemberRule> = Vec::new();
    for c in chains {
        let r = if c.deployed {
            MemberRule::of(c.threshold, c.owners.len())
        } else {
            original
        };
        if !out.contains(&r) {
            out.push(r);
        }
    }
    out
}

/// The header's rule chip (`ruleChip`): what it needs, or that it differs by
/// network.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigRuleChip"))]
pub enum RuleChip {
    /// "Needs 2 of 3" (or "One member").
    Needs { rule: MemberRule },
    /// "Rule differs by network"; its title lists each rule.
    Differs { rules: Vec<MemberRule> },
}

pub fn rule_chip(rules: &[MemberRule]) -> Option<RuleChip> {
    match rules {
        [] => None,
        [one] => Some(RuleChip::Needs { rule: *one }),
        many => Some(RuleChip::Differs {
            rules: many.to_vec(),
        }),
    }
}

/// What the multisigs on this device keep of `address`: the first name and
/// the first kind any of them holds (`keptMember`). Labels only.
pub fn kept_member(accounts: &[MultisigAccount], address: &str) -> MemberInfo {
    let key = address.to_ascii_lowercase();
    let mut out = MemberInfo::default();
    for a in accounts {
        let Some(info) = a.members.as_ref().and_then(|m| m.get(&key)) else {
            continue;
        };
        if out.name.is_none() {
            out.name.clone_from(&info.name);
        }
        if out.kind.is_none() {
            out.kind.clone_from(&info.kind);
        }
    }
    out
}

/// This multisig's members anywhere (`membersAnywhere`): on each network it
/// is deployed on, and the original ones wherever it is not (or nothing is
/// read yet). Each once.
pub fn members_anywhere(original: &[String], chains: &[&ChainMembers]) -> Vec<String> {
    let deployed: Vec<&&ChainMembers> = chains.iter().filter(|c| c.deployed).collect();
    let undeployed = deployed.len() < chains.len() || deployed.is_empty();
    let mut out: Vec<String> = Vec::new();
    let all = deployed
        .iter()
        .flat_map(|c| c.owners.iter())
        .chain(original.iter().filter(|_| undeployed));
    for o in all {
        if !out.iter().any(|x| same_address(x, o)) {
            out.push(o.clone());
        }
    }
    out
}

/// A Vela account, an outside wallet or a smart contract account: the one
/// rule's kind ([`super::multisig_signers`]).
pub use super::multisig_signers::MemberKind;

/// A contact, as much of it as a member entry reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberContact"))]
pub struct MemberContact {
    pub address: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub resolved_name: Option<String>,
    /// The contacts book's kind (`eoa`, `account`, `unknown`).
    #[serde(default)]
    pub kind: Option<String>,
}

/// An account or contact offered under the entry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigMemberSuggestion")
)]
pub struct Suggestion {
    pub address: String,
    pub name: Option<String>,
    pub kind: MemberKind,
}

fn candidates(local: &[LocalAccount], contacts: &[MemberContact]) -> Vec<Suggestion> {
    let mine = local.iter().map(|a| Suggestion {
        address: checksummed(&a.address),
        name: Some(a.name.clone()).filter(|n| !n.is_empty()),
        kind: MemberKind::Vela,
    });
    let book = contacts.iter().map(|c| Suggestion {
        address: checksummed(&c.address),
        name: c.name.clone().or_else(|| c.resolved_name.clone()),
        kind: if c.kind.as_deref() == Some("eoa") {
            MemberKind::Wallet
        } else {
            MemberKind::Vela
        },
    });
    let mut out: Vec<Suggestion> = Vec::new();
    for s in mine.chain(book) {
        if !out.iter().any(|x| same_address(&x.address, &s.address)) {
            out.push(s);
        }
    }
    out
}

/// This device's accounts and the contacts, matching what is typed: by name,
/// or by the start of the address. At most [`MAX_SUGGESTIONS`].
pub fn suggestions(
    typed: &str,
    local: &[LocalAccount],
    contacts: &[MemberContact],
) -> Vec<Suggestion> {
    let q = typed.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    candidates(local, contacts)
        .into_iter()
        .filter(|s| {
            s.name
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(&q))
                || s.address.to_lowercase().starts_with(&q)
        })
        .take(MAX_SUGGESTIONS)
        .collect()
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// What the person is composing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberCompose"))]
pub enum Compose {
    Add,
    Remove { owner: String },
    Replace { owner: String },
    Threshold,
}

#[allow(clippy::large_enum_variant)] // a wire type: its JSON shape is the shells'
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMembersEvent"))]
pub enum Event {
    /// The multisig shown (Settings): each of its networks' members are read
    /// now. `known`: the other multisigs on this device, for the names they
    /// keep of members.
    Show {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        account: MultisigAccount,
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
        local: Vec<LocalAccount>,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        known: Vec<MultisigAccount>,
    },
    /// Read every network's members again.
    Refresh,
    /// The session's Vela accounts changed.
    LocalAccounts { local: Vec<LocalAccount> },
    /// The contacts book, for suggestions.
    Contacts { contacts: Vec<MemberContact> },
    /// The open proposals the proposals machine reads (all networks).
    Waiting { proposals: Vec<WaitingProposal> },
    /// Open (or close, `None`) a change: Add, Remove or Replace a member, or
    /// the approvals needed.
    Compose { compose: Option<Compose> },
    /// The approvals stepper, by `by` (±1). Outside a change, it starts one
    /// of the approvals needed.
    Step {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        by: i64,
    },
    /// A change composed by the shell itself (a sheet of its own, a link):
    /// checked the same way. `None` clears it.
    Draft { change: Option<MemberChange> },
    /// What is typed in the member entry.
    Entry { text: String },
    /// The entry is read (blur, Enter): an address checked, a name resolved.
    ReadEntry,
    /// A suggestion picked.
    Pick { address: String },
    /// The name this device keeps for the new member.
    EntryName { name: String },
    /// The person's pick of what the new member is (`None`: as it reads).
    EntryKind { kind: Option<MemberKind> },
    /// The person pressed Propose (before the proposals machine is asked):
    /// the new member's name and kind are kept with the multisig on this
    /// device.
    Proposing,
    #[serde(skip)]
    Changed,
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

/// A member as a list draws it. Labels only, never authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberRow"))]
pub struct MemberRow {
    /// EIP-55, however it came (r4D-19).
    pub address: String,
    /// This device's name for its account, else what a multisig here keeps.
    pub name: Option<String>,
    /// `passkey` or `wallet`, as far as this device can tell.
    pub kind: Option<String>,
    /// A Vela account on this device.
    pub on_device: bool,
    /// Not among the original members (a network's list).
    pub added: bool,
}

/// One network the multisig is used on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigNetworkMembersState")
)]
pub enum NetworkState {
    Reading,
    /// Its chain did not answer (or not in time): nothing is said of its members.
    Unreadable {
        detail: String,
    },
    /// Not deployed: it starts with the original members.
    Undeployed,
    Deployed {
        rule: MemberRule,
        /// Its members now, in its list's order (`added` marked).
        members: Vec<MemberRow>,
        /// Original members no longer members here.
        removed: Vec<MemberRow>,
        /// The same as the original members ("Same as the original members").
        same: bool,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold_was: u64,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold_is: u64,
        /// Decimal.
        nonce: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigNetworkMembers")
)]
pub struct NetworkView {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub state: NetworkState,
}

/// The founding members (the genesis's).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigOriginalMembers")
)]
pub struct OriginalView {
    pub rule: Option<MemberRule>,
    pub members: Vec<MemberRow>,
}

/// The passkey-native recovery note under the approvals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigRecovery"))]
pub enum Recovery {
    /// "If one of the n loses theirs, m of the others replace it."
    Replaceable {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        m: u64,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        n: u64,
    },
    /// Every member must approve: a lost passkey cannot be replaced.
    EveryMember,
}

/// The approvals stepper for the change being composed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigStepper"))]
pub struct Stepper {
    /// The members after the change.
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub n: u64,
    /// The approvals: the person's pick, else the rule as it stands ("3 of 2"
    /// when removing would leave too few — the plan says why).
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub value: u64,
    pub can_fewer: bool,
    pub can_more: bool,
}

/// A member typed or picked, resolved to its address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberEntry"))]
pub struct MemberEntry {
    /// The member is the address: shown in full.
    pub address: String,
    /// The name it was found by (a resolved ENS name).
    pub name: Option<String>,
}

/// Why what is typed is no member (said under the entry).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigEntryNote"))]
pub enum EntryNote {
    InvalidAddress,
    InvalidChecksum,
    /// The zero address or the sentinel (`refuse.noOne`).
    NoOne,
    NameNotFound {
        name: String,
    },
    NameUnavailable {
        name: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigEntryView"))]
pub struct EntryView {
    pub typed: String,
    pub suggestions: Vec<Suggestion>,
    pub entry: Option<MemberEntry>,
    pub note: Option<EntryNote>,
    pub resolving: bool,
    /// The name kept for it on this device.
    pub name: String,
    /// The kind kept for it when it is proposed (`choice.kind`); `None`:
    /// nothing kept — the member stays unsure.
    pub kind: Option<MemberKind>,
    /// r7: what its kind choice shows (`kindChoice`) — found as a Vela
    /// account, a smart contract account, "Other wallet" hinted, or asked —
    /// from what the chain says of it ([`signers::kind_evidence`]) and the
    /// person's word. `None` without an entry.
    pub choice: Option<KindChoice>,
}

/// "Replace C with Dana": who replaces whom, before anything is proposed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigReplacePreview")
)]
pub struct ReplacePreview {
    pub old: MemberRow,
    pub new_address: String,
    /// The name just typed, else the name it was found by, else what this
    /// device keeps for it.
    pub new_name: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMembersView"))]
pub struct MultisigMembersView {
    pub address: Option<String>,
    pub original: OriginalView,
    /// Each network it is used on, in the networks' order.
    pub networks: Vec<NetworkView>,
    /// Its members on this device, anywhere.
    pub on_device: Vec<String>,
    /// None of its members is on this device.
    pub read_only: bool,
    pub rules: Vec<MemberRule>,
    pub rule_chip: Option<RuleChip>,
    /// Every network was read and it is deployed on none: a change would
    /// deploy it, so nothing is offered.
    pub none_deployed: bool,
    /// Every deployed network's members, once: the rows Replace and Remove are on.
    pub rows: Vec<MemberRow>,
    /// The rule the stepper starts from: the first deployed network's.
    pub base: Option<MemberRule>,
    pub recovery: Option<Recovery>,
    pub compose: Option<Compose>,
    pub stepper: Option<Stepper>,
    pub entry: EntryView,
    /// The change as it stands (each ready network's
    /// [`PlanNetwork::proposal`] carries it).
    pub change: Option<MemberChange>,
    /// What it is on each network.
    pub plan: Option<MemberPlan>,
    /// Somewhere to propose it.
    pub can_propose: bool,
    pub replace_preview: Option<ReplacePreview>,
    /// The member changes waiting, one per action, with each network's progress.
    pub groups: Vec<ChangeGroup>,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum Read {
    Reading,
    Answered(ChainMembers),
    Unreadable(String),
}

#[derive(Default)]
struct Entry {
    typed: String,
    entry: Option<MemberEntry>,
    note: Option<EntryNote>,
    resolving: bool,
    name: String,
    kind_pick: Option<MemberKind>,
    /// What the chains said of the entry's address (lowercase), once read.
    read: Option<(String, SignerRead)>,
    /// Bumped by every new text and read: a late answer for older text is dropped.
    ask: u64,
}

#[derive(Default)]
pub struct State {
    account: Option<MultisigAccount>,
    known: Vec<MultisigAccount>,
    chain_ids: Vec<u64>,
    local: Vec<LocalAccount>,
    contacts: Vec<MemberContact>,
    chain: Chain,
    seeded: bool,
    reads: BTreeMap<u64, Read>,
    /// Bumped when another multisig is shown: its reads are not this one's.
    round: u64,
    /// r10: which read's re-reads are live — a later read ends the earlier
    /// one's.
    reread_gen: u64,
    waiting: Vec<WaitingProposal>,
    compose: Option<Compose>,
    picked: Option<u64>,
    draft: Option<MemberChange>,
    entry: Entry,
}

impl State {
    /// Networks read, in order.
    fn read(&self) -> Vec<(u64, &ChainMembers)> {
        self.chain_ids
            .iter()
            .filter_map(|id| match self.reads.get(id) {
                Some(Read::Answered(m)) => Some((*id, m)),
                _ => None,
            })
            .collect()
    }

    fn base(&self) -> Option<ChainMembers> {
        self.read()
            .into_iter()
            .find(|(_, m)| m.deployed)
            .map(|(_, m)| m.clone())
    }

    fn kept_from(&self) -> Vec<MultisigAccount> {
        let mut all: Vec<MultisigAccount> = self.account.iter().cloned().collect();
        for k in &self.known {
            if !all.iter().any(|a| same_address(&a.address, &k.address)) {
                all.push(k.clone());
            }
        }
        all
    }

    fn on_device(&self) -> Vec<String> {
        let Some(account) = &self.account else {
            return Vec::new();
        };
        let read: Vec<&ChainMembers> = self.read().into_iter().map(|(_, m)| m).collect();
        local_owners(&self.local, &members_anywhere(&account.owners, &read))
            .into_iter()
            .map(|o| checksummed(&o.address))
            .collect()
    }

    fn row(
        &self,
        kept: &[MultisigAccount],
        here: &[String],
        address: &str,
        added: bool,
    ) -> MemberRow {
        let local = self
            .local
            .iter()
            .find(|l| same_address(&l.address, address));
        let info = kept_member(kept, address);
        MemberRow {
            address: checksummed(address),
            name: local
                .map(|l| l.name.clone())
                .filter(|n| !n.is_empty())
                .or(info.name),
            kind: if local.is_some() {
                Some(MemberKind::Vela.stored().to_owned())
            } else {
                info.kind
            },
            on_device: here.iter().any(|h| same_address(h, address)),
            added,
        }
    }

    /// The entry's kind choice (`kindChoice(signers.evidence(chainIds,
    /// address), kindPick ?? keptMember(address).kind)`).
    fn entry_choice(&self) -> Option<KindChoice> {
        let e = self.entry.entry.as_ref()?;
        let local = self
            .local
            .iter()
            .any(|l| same_address(&l.address, &e.address));
        let kept = kept_member(&self.kept_from(), &e.address).kind;
        let facts = match self
            .entry
            .read
            .as_ref()
            .filter(|(a, _)| same_address(a, &e.address))
        {
            Some((_, read)) => read.facts(None, local, None, kept.clone()),
            None => SignerFacts {
                local,
                kept: kept.clone(),
                ..SignerFacts::default()
            },
        };
        let said = self
            .entry
            .kind_pick
            .or_else(|| kept.as_deref().and_then(MemberKind::from_stored));
        Some(signers::kind_choice(signers::kind_evidence(&facts), said))
    }

    /// The change as it stands: the shell's draft, else what is composed.
    fn change(&self) -> Option<MemberChange> {
        if let Some(d) = &self.draft {
            return Some(d.clone());
        }
        let entry = self.entry.entry.as_ref();
        match self.compose.as_ref()? {
            Compose::Add => entry.map(|e| MemberChange::Add {
                owner: e.address.clone(),
                threshold: self.picked,
            }),
            Compose::Remove { owner } => Some(MemberChange::Remove {
                owner: owner.clone(),
                threshold: self.picked,
            }),
            Compose::Replace { owner } => entry.map(|e| MemberChange::Replace {
                old_owner: owner.clone(),
                new_owner: e.address.clone(),
            }),
            Compose::Threshold => self
                .picked
                .map(|threshold| MemberChange::Threshold { threshold }),
        }
    }

    /// The stepper: its members after the change, and the approvals.
    fn stepper(&self) -> Option<Stepper> {
        let base = self.base()?;
        let owners = base.owners.len() as i64;
        let n = match &self.compose {
            None | Some(Compose::Threshold) => owners,
            Some(Compose::Add) => owners + 1,
            Some(Compose::Remove { .. }) => owners - 1,
            Some(Compose::Replace { .. }) => return None,
        };
        let n = u64::try_from(n.max(0)).unwrap_or(0);
        let value = self.picked.unwrap_or(base.threshold);
        Some(Stepper {
            n,
            value,
            can_fewer: value > 1,
            can_more: value < n,
        })
    }

    fn step(&mut self, by: i64) {
        if self.draft.is_some() {
            return;
        }
        if self.compose.is_none() {
            self.compose = Some(Compose::Threshold);
            self.picked = None;
        }
        let Some(s) = self.stepper() else { return };
        let n = i64::try_from(s.n).unwrap_or(i64::MAX);
        let value = i64::try_from(s.value).unwrap_or(i64::MAX);
        // Math.max(1, Math.min(n, value + by)).
        let next = value.saturating_add(by).min(n).max(1);
        self.picked = u64::try_from(next).ok();
    }

    fn open(&mut self, compose: Option<Compose>) {
        self.compose = compose;
        self.picked = None;
        self.draft = None;
        let ask = self.entry.ask + 1;
        self.entry = Entry {
            ask,
            ..Entry::default()
        };
    }

    fn networks_for_plan(&self) -> Vec<NetworkMembers> {
        self.read()
            .into_iter()
            .map(|(id, m)| NetworkMembers {
                chain_id: id,
                members: m.clone(),
                items: self
                    .waiting
                    .iter()
                    .filter(|w| w.chain_id == id)
                    .cloned()
                    .collect(),
            })
            .collect()
    }

    fn view(&self) -> MultisigMembersView {
        let Some(account) = &self.account else {
            return MultisigMembersView::default();
        };
        let kept = self.kept_from();
        let here = self.on_device();
        let read = self.read();
        let read_members: Vec<&ChainMembers> = read.iter().map(|(_, m)| *m).collect();
        let original_rule = MemberRule::of(account.threshold, account.owners.len());
        let networks: Vec<NetworkView> = self
            .chain_ids
            .iter()
            .map(|id| NetworkView {
                chain_id: *id,
                state: match self.reads.get(id) {
                    None | Some(Read::Reading) => NetworkState::Reading,
                    Some(Read::Unreadable(detail)) => NetworkState::Unreadable {
                        detail: detail.clone(),
                    },
                    Some(Read::Answered(m)) if !m.deployed => NetworkState::Undeployed,
                    Some(Read::Answered(m)) => self.deployed_state(account, &kept, &here, m),
                },
            })
            .collect();
        let rules = current_rules(original_rule, &read_members);
        let rows: Vec<MemberRow> = deployed_members(&read_members)
            .iter()
            .map(|o| self.row(&kept, &here, o, false))
            .collect();
        let all_read = self
            .chain_ids
            .iter()
            .all(|id| matches!(self.reads.get(id), Some(Read::Answered(_))));
        let base = self.base();
        let change = self.change();
        let plan = change.as_ref().map(|c| {
            let mut plan = plan_member_change(&account.address, &self.networks_for_plan(), c);
            plan.unreadable = self
                .chain_ids
                .iter()
                .filter(|id| matches!(self.reads.get(id), Some(Read::Unreadable(_))))
                .copied()
                .collect();
            plan
        });
        let entry = &self.entry;
        let replace_preview = match (&self.compose, &entry.entry, &plan) {
            (Some(Compose::Replace { owner }), Some(e), Some(_)) => Some(ReplacePreview {
                old: self.row(&kept, &here, owner, false),
                new_address: e.address.clone(),
                new_name: Some(entry.name.trim().to_owned())
                    .filter(|n| !n.is_empty())
                    .or_else(|| e.name.clone())
                    .or_else(|| self.row(&kept, &here, &e.address, false).name),
            }),
            _ => None,
        };
        MultisigMembersView {
            address: Some(account.address.clone()),
            original: OriginalView {
                rule: Some(original_rule),
                members: account
                    .owners
                    .iter()
                    .map(|o| self.row(&kept, &here, o, false))
                    .collect(),
            },
            networks,
            read_only: here.is_empty(),
            on_device: here.clone(),
            rule_chip: rule_chip(&rules),
            rules,
            none_deployed: all_read && rows.is_empty(),
            rows,
            base: base
                .as_ref()
                .map(|b| MemberRule::of(b.threshold, b.owners.len())),
            recovery: base.as_ref().map(|b| {
                if (b.threshold as usize) < b.owners.len() {
                    Recovery::Replaceable {
                        m: b.threshold,
                        n: b.owners.len() as u64,
                    }
                } else {
                    Recovery::EveryMember
                }
            }),
            compose: self.compose.clone(),
            stepper: if self.draft.is_some() {
                None
            } else {
                self.stepper()
            },
            entry: EntryView {
                typed: entry.typed.clone(),
                suggestions: if entry.entry.is_some() {
                    Vec::new()
                } else {
                    suggestions(&entry.typed, &self.local, &self.contacts)
                },
                entry: entry.entry.clone(),
                note: entry.note.clone(),
                resolving: entry.resolving,
                name: entry.name.clone(),
                kind: self.entry_choice().and_then(|c| c.kind),
                choice: self.entry_choice(),
            },
            can_propose: plan.as_ref().is_some_and(|p| !p.ready.is_empty()),
            change,
            plan,
            replace_preview,
            groups: member_change_groups(&self.waiting, &self.chain_ids),
        }
    }

    fn deployed_state(
        &self,
        account: &MultisigAccount,
        kept: &[MultisigAccount],
        here: &[String],
        m: &ChainMembers,
    ) -> NetworkState {
        let drift =
            multisig::member_drift(&account.owners, account.threshold, &m.owners, m.threshold).ok();
        let added = |o: &str| {
            drift
                .as_ref()
                .is_some_and(|d| d.added.iter().any(|a| same_address(a, o)))
        };
        NetworkState::Deployed {
            rule: MemberRule::of(m.threshold, m.owners.len()),
            members: m
                .owners
                .iter()
                .map(|o| self.row(kept, here, o, added(o)))
                .collect(),
            removed: drift
                .as_ref()
                .map(|d| {
                    d.removed
                        .iter()
                        .map(|o| self.row(kept, here, o, false))
                        .collect()
                })
                .unwrap_or_default(),
            // A drift the core could not read is never "the same".
            same: drift.as_ref().is_some_and(|d| d.same),
            threshold_was: account.threshold,
            threshold_is: m.threshold,
            nonce: m.nonce.to_string(),
        }
    }
}

#[derive(Default)]
pub struct Model {
    state: Shared<State>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MultisigMembers;

type Cmd = Command<MultisigEffect, Event>;

/// Run `flow` as an async command over the model's state, rendering as it ends.
fn flow<F, Fut>(model: &Model, f: F) -> Cmd
where
    F: FnOnce(Io<Event>, Shared<State>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = model.state.clone();
    Command::all([
        render(),
        Command::new(move |ctx| async move {
            let io = Io::new(ctx);
            f(io.clone(), state).await;
            io.emit(Event::Changed);
        }),
    ])
}

impl App for MultisigMembers {
    type Event = Event;
    type Model = Model;
    type ViewModel = MultisigMembersView;
    type Effect = MultisigEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        match event {
            Event::Changed => render(),
            Event::Show {
                account,
                chain_ids,
                local,
                known,
            } => {
                {
                    let mut s = lock(&model.state);
                    let same = s
                        .account
                        .as_ref()
                        .is_some_and(|a| same_address(&a.address, &account.address));
                    if !same {
                        s.round += 1;
                        s.reads.clear();
                        s.waiting.clear();
                        s.open(None);
                    }
                    let mut ids: Vec<u64> = Vec::new();
                    for id in chain_ids {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                    s.reads.retain(|id, _| ids.contains(id));
                    for id in &ids {
                        s.reads.entry(*id).or_insert(Read::Reading);
                    }
                    s.chain_ids = ids;
                    s.account = Some(account);
                    s.local = local;
                    s.known = known;
                }
                flow(model, |io, st| async move { read_all(&io, &st).await })
            }
            Event::Refresh => flow(model, |io, st| async move { read_all(&io, &st).await }),
            Event::LocalAccounts { local } => {
                lock(&model.state).local = local;
                render()
            }
            Event::Contacts { contacts } => {
                lock(&model.state).contacts = contacts;
                render()
            }
            Event::Waiting { proposals } => {
                let moved = {
                    let mut s = lock(&model.state);
                    let moved = member_change_ended(&s.waiting, &proposals);
                    s.waiting = proposals;
                    moved
                };
                // A member change that was waiting ran (or was voided): each
                // network's members are read again, as they are now.
                if moved {
                    flow(model, |io, st| async move { read_all(&io, &st).await })
                } else {
                    render()
                }
            }
            Event::Compose { compose } => {
                lock(&model.state).open(compose);
                render()
            }
            Event::Step { by } => {
                lock(&model.state).step(by);
                render()
            }
            Event::Draft { change } => {
                let mut s = lock(&model.state);
                s.compose = None;
                s.picked = None;
                s.draft = change;
                render()
            }
            Event::Entry { text } => {
                // S4 (r8): a whole address typed or pasted is read at once —
                // Propose is ready on the input, not only once the field blurs.
                let whole = {
                    let t = text.trim();
                    t.len() == 42
                        && (t.starts_with("0x") || t.starts_with("0X"))
                        && t[2..].bytes().all(|b| b.is_ascii_hexdigit())
                };
                let mut s = lock(&model.state);
                let e = &mut s.entry;
                e.typed = text;
                e.entry = None;
                e.note = None;
                e.kind_pick = None;
                e.resolving = false;
                e.ask += 1;
                if whole {
                    drop(s);
                    return read_entry(model);
                }
                render()
            }
            Event::ReadEntry => read_entry(model),
            Event::Pick { address } => {
                let mut s = lock(&model.state);
                let found = candidates(&s.local, &s.contacts)
                    .into_iter()
                    .find(|c| same_address(&c.address, &address));
                let Some(c) = found else {
                    // Not one offered: read as typed.
                    s.entry.typed = address;
                    drop(s);
                    return read_entry(model);
                };
                let e = &mut s.entry;
                e.ask += 1;
                e.resolving = false;
                e.note = None;
                e.typed.clone_from(&c.address);
                e.entry = Some(MemberEntry {
                    address: c.address.clone(),
                    name: None,
                });
                if e.name.is_empty() {
                    if let Some(n) = c.name {
                        e.name = n;
                    }
                }
                // r7: picked, it is asked like one typed — what the chain says
                // of it, else the person's word; never the list's guess.
                e.kind_pick = None;
                let (ask, address) = (e.ask, c.address.clone());
                drop(s);
                read_kind(model, ask, address)
            }
            Event::EntryName { name } => {
                lock(&model.state).entry.name = name;
                render()
            }
            Event::EntryKind { kind } => {
                lock(&model.state).entry.kind_pick = kind;
                render()
            }
            Event::Proposing => {
                let notice = {
                    let s = lock(&model.state);
                    let who = match (&s.compose, &s.entry.entry) {
                        (Some(Compose::Add | Compose::Replace { .. }), Some(e)) => {
                            Some(e.address.clone())
                        }
                        _ => None,
                    };
                    who.zip(s.account.as_ref()).map(|(who, account)| {
                        let name = Some(s.entry.name.trim().to_owned()).filter(|n| !n.is_empty());
                        Notice::Account {
                            address: account.address.clone(),
                            update: AccountUpdate::NameMembers {
                                members: BTreeMap::from([(
                                    who,
                                    MemberInfo {
                                        name,
                                        // Only what the choice keeps: an
                                        // unsure member keeps no kind.
                                        kind: s
                                            .entry_choice()
                                            .and_then(|c| c.kind)
                                            .map(|k| k.stored().to_owned()),
                                        typed: None,
                                    },
                                )]),
                                typed: true,
                            },
                        }
                    })
                };
                match notice {
                    Some(n) => flow(model, move |io, _| async move { io.notice(n).await }),
                    None => render(),
                }
            }
        }
    }

    fn view(&self, model: &Model) -> MultisigMembersView {
        lock(&model.state).view()
    }
}

// ---------------------------------------------------------------------------
// Reading the networks
// ---------------------------------------------------------------------------

/// Every network's members, read at once; each drawn as it answers.
async fn read_all(io: &Io<Event>, st: &Shared<State>) {
    let seeded = lock(st).seeded;
    if !seeded {
        let kept = io.load(&[keys::DEPLOYED]).await;
        let s = &mut lock(st);
        if !s.seeded {
            s.chain
                .seed_deployed(kept.get(keys::DEPLOYED).cloned().flatten().as_deref());
            s.seeded = true;
        }
    }
    let (account, chain_ids, chain, round, gen) = {
        let mut s = lock(st);
        let Some(account) = s.account.clone() else {
            return;
        };
        s.reread_gen += 1;
        (
            account,
            s.chain_ids.clone(),
            s.chain.clone(),
            s.round,
            s.reread_gen,
        )
    };
    read_some(io, st, &account, &chain, &chain_ids, round).await;
    // r10: an unread network is read again on its own until it answers.
    let unread = |s: &State| -> Option<Vec<u64>> {
        let ids: Vec<u64> = s
            .chain_ids
            .iter()
            .copied()
            .filter(|id| matches!(s.reads.get(id), Some(Read::Unreadable(_))))
            .collect();
        (s.reread_gen == gen && s.round == round && !ids.is_empty()).then_some(ids)
    };
    for ms in MEMBERS_REREAD_MS {
        if unread(&lock(st)).is_none() {
            return;
        }
        io.wait(ms).await;
        let Some(ids) = unread(&lock(st)) else {
            return;
        };
        read_some(io, st, &account, &chain, &ids, round).await;
    }
}

/// Read `chain_ids`' members, each answer kept as it lands (this round's).
async fn read_some(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain: &Chain,
    chain_ids: &[u64],
    round: u64,
) {
    let reads = chain_ids.iter().map(|id| {
        async move {
            let read = read_bounded(io, chain, *id, account).await;
            {
                let mut s = lock(st);
                if s.round == round && s.chain_ids.contains(id) {
                    s.reads.insert(*id, read);
                }
            }
            io.emit(Event::Changed);
        }
    });
    future::join_all(reads).await;
}

/// One network's members, or why they cannot be said: a read that does not
/// answer within [`MEMBERS_READ_MS`] is unreadable, never "the same".
async fn read_bounded(
    io: &Io<Event>,
    chain: &Chain,
    chain_id: u64,
    account: &MultisigAccount,
) -> Read {
    let read = chain
        .read_members(io, chain_id, &account.address, &account.genesis)
        .boxed();
    let bound = io.wait(MEMBERS_READ_MS).boxed();
    match future::select(read, bound).await {
        Either::Left((Ok(members), _)) => Read::Answered(members),
        Either::Left((Err(e), _)) => Read::Unreadable(e.detail),
        Either::Right(_) => Read::Unreadable(format!("no answer within {MEMBERS_READ_MS} ms")),
    }
}

// ---------------------------------------------------------------------------
// The member entry
// ---------------------------------------------------------------------------

/// Read what is typed (`readEntry`): a name is resolved forward, an address
/// checked; a name being searched among the suggestions is no wrong address.
fn read_entry(model: &Model) -> Cmd {
    let mut s = lock(&model.state);
    let value = s.entry.typed.trim().to_owned();
    s.entry.ask += 1;
    s.entry.entry = None;
    s.entry.note = None;
    s.entry.resolving = false;
    if value.is_empty() {
        return render();
    }
    if looks_like_name(&value) {
        s.entry.resolving = true;
        let ask = s.entry.ask;
        drop(s);
        return flow(model, move |io, st| async move {
            let answer = resolve_typed_name(&io, &value).await;
            let found = {
                let mut s = lock(&st);
                if s.entry.ask != ask {
                    return;
                }
                s.entry.resolving = false;
                match answer {
                    NameAnswer::Resolved { address, name } => {
                        if s.entry.name.is_empty() {
                            s.entry.name.clone_from(&name);
                        }
                        s.entry.entry = Some(MemberEntry {
                            address: address.clone(),
                            name: Some(name),
                        });
                        Some((address, s.chain_ids.clone()))
                    }
                    NameAnswer::NotFound => {
                        s.entry.note = Some(EntryNote::NameNotFound { name: value });
                        None
                    }
                    NameAnswer::Unavailable => {
                        s.entry.note = Some(EntryNote::NameUnavailable { name: value });
                        None
                    }
                }
            };
            if let Some((address, chain_ids)) = found {
                io.emit(Event::Changed);
                read_kind_now(&io, &st, ask, &address, &chain_ids).await;
            }
        });
    }
    match member_problem(&value, &[]) {
        None => {
            s.entry.entry = Some(MemberEntry {
                address: value.clone(),
                name: None,
            });
            let ask = s.entry.ask;
            drop(s);
            return read_kind(model, ask, value);
        }
        Some(MemberProblem::InvalidAddress)
            if !suggestions(&value, &s.local, &s.contacts).is_empty() => {}
        Some(MemberProblem::Checksum) => s.entry.note = Some(EntryNote::InvalidChecksum),
        Some(MemberProblem::NotMember) => s.entry.note = Some(EntryNote::NoOne),
        Some(_) => s.entry.note = Some(EntryNote::InvalidAddress),
    }
    render()
}

/// r7: what the chains say of the entry's address — its code on the
/// multisig's networks and Vela's passkey signer configuration for it — read
/// once it is an address (an account here needs none).
fn read_kind(model: &Model, ask: u64, address: String) -> Cmd {
    let chain_ids = lock(&model.state).chain_ids.clone();
    flow(model, move |io, st| async move {
        read_kind_now(&io, &st, ask, &address, &chain_ids).await;
    })
}

async fn read_kind_now(
    io: &Io<Event>,
    st: &Shared<State>,
    ask: u64,
    address: &str,
    chain_ids: &[u64],
) {
    let local = lock(st)
        .local
        .iter()
        .any(|l| same_address(&l.address, address));
    if local {
        return;
    }
    let read = signers::read_signer(io, chain_ids, address).await;
    let mut s = lock(st);
    if s.entry.ask == ask {
        s.entry.read = Some((address.to_ascii_lowercase(), read));
    }
}

/// What a typed name resolved to (`NameAnswer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameAnswer {
    Resolved {
        address: String,
        name: String,
    },
    /// The name has no address (or is no name anyone resolves).
    NotFound,
    /// Nobody answered.
    Unavailable,
}

/// `typed`, resolved forward by the core's walk (`resolve_step`) over this
/// wallet's own RPC: Basenames on Base, every other name ENS on Ethereum. A
/// network this device lacks answers nothing, which is "unavailable".
pub async fn resolve_typed_name<Ev: Send + 'static>(io: &Io<Ev>, typed: &str) -> NameAnswer {
    let name = typed.trim();
    let (chain_id, registry) = name_service(name);
    let mut answers: Vec<LookupAnswer> = Vec::new();
    for _ in 0..WALK_ROUNDS {
        match name_verify::resolve_step(chain_id, registry, name, &answers) {
            ResolveStep::Done {
                state,
                address,
                name,
            } => {
                return match (state, address, name) {
                    (ResolveState::Resolved, Some(address), Some(name)) => {
                        NameAnswer::Resolved { address, name }
                    }
                    (ResolveState::Unavailable, _, _) => NameAnswer::Unavailable,
                    _ => NameAnswer::NotFound,
                }
            }
            ResolveStep::Ask { requests } => {
                let done = future::join_all(requests.iter().map(|r| perform(io, r))).await;
                answers.extend(done);
            }
        }
    }
    NameAnswer::Unavailable
}

/// One request of a walk, through the pool. A bare `0x` is an answer; only
/// an error or no result is a silence.
async fn perform<Ev: Send + 'static>(io: &Io<Ev>, request: &LookupRequest) -> LookupAnswer {
    match request {
        LookupRequest::EthCall {
            id,
            chain_id,
            to,
            data,
        } => {
            let result = io
                .rpc(
                    u64::from(*chain_id),
                    "eth_call",
                    json!([{ "to": to, "data": data }, "latest"]),
                )
                .await;
            match result {
                Ok(Value::String(body)) => LookupAnswer {
                    id: id.clone(),
                    outcome: LookupOutcome::Ok,
                    body: Some(body),
                },
                _ => LookupAnswer {
                    id: id.clone(),
                    outcome: LookupOutcome::Failed,
                    body: None,
                },
            }
        }
        LookupRequest::IndexGet { id, .. } => LookupAnswer {
            id: id.clone(),
            outcome: LookupOutcome::Failed,
            body: None,
        },
    }
}
