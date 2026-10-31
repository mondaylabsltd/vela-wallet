//! Machine — a multisig's transactions in flight (F0.4): what the mailbox
//! holds, re-checked against the chain, and the four actions — propose,
//! approve, execute, cancel — each one passkey prompt, each safe to retry.
//!
//! A port of the web's `proposals.svelte.ts`, `gate.svelte.ts`,
//! `proposal-review.svelte.ts` (the review's reads), `execute.ts`,
//! `execution-watch.svelte.ts`, `records.ts` (the history lookup),
//! `external/onchain.ts` and `actor.ts` (docs/multisig-vault-design.md
//! §3.4–§3.6, §5):
//!
//! - **Nothing from the mailbox is trusted.** Every proposal is re-hashed
//!   (`safe_tx_hash`), re-read (`review`), every approval verified
//!   (passkey: `verify_approval`; plain account: `verify_ecdsa_approval`
//!   must recover its owner); an approval counts only if it verifies AND
//!   its owner is a member on that chain NOW (members and nonce read from
//!   the chain through the wallet's own RPC, never the service's). An
//!   on-chain approval (`approvedHashes`) is the chain's to say.
//! - **One read across chains** (`GET /api/proposals/{safe}`); a service
//!   without it is asked network by network; a network something says it
//!   is on is used (a notice to the accounts machine); one this device
//!   lacks is listed as missing, never read or counted.
//! - **No nonce from a list that is not there**: a proposal takes the next
//!   free nonce from the mailbox's list read NOW; unreadable → nothing is
//!   proposed.
//! - **The signing gate before every passkey** (§3.6): the chain read again
//!   (who can act for it, its members, the review's state-diff guard) and
//!   the core's `signing_gate` — refused / blocked / unchecked stop; risk
//!   asks the person every time ([`MultisigOperation::AskRisk`]).
//! - **The outbox**: what was signed is kept before it is sent; a busy
//!   (429) or absent mailbox is no failure — sent again on the service's
//!   word or a backoff for [`OUTBOX_PATIENCE_MS`], then "not sent, kept",
//!   with Send again. Nothing is ever signed twice.
//! - **Execution**: a preview first (enough funds, the call does not
//!   revert, the plan, the fee quote, whether the executor can pay); the
//!   deploy gate re-reads the code right before a deployment; the
//!   executor's user operation is the shell's send pipeline
//!   ([`MultisigOperation::SendCalls`]); then it is watched until the
//!   core's `execution_verdict` says how it ended.
//!
//! The shell drives the poll: it sends [`Event::Tick`] every
//! [`ProposalsView::pace_ms`] while the multisig is shown and the page is
//! visible, [`Event::Refresh`] when the page is shown again or back online.

use std::collections::{BTreeMap, BTreeSet};

use crux_core::{render::render, App, Command};
use futures::future;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_gather;
use super::multisig_shell::{
    self as shell, keys, local_owners, lock, same_address, AccountUpdate, ApprovalRoute, Chain,
    ChainMembers, Io, LocalAccount, MultisigAccount, MultisigEffect, MultisigOperation,
    MultisigShellResult, Notice, PasskeyPurpose, Proposal, SendPurpose, Service, ServiceFail,
    Shared, StoredApproval, Submission, SubmitAnswer,
};
use super::multisig_signers::{self as signers, SignerKind, SignerRead};
use crate::multisig::{self, Action, Approval, Call, SafeTx};
use crate::multisig_review::{self, Known, ReviewInput};
use crate::multisig_watch::{
    self, ExecutedIn, ExecutionFacts, ExecutionVerdict, OurOp, OurReceipt,
};

/// A refresh still out after this long: the list may be stale.
pub const SLOW_REFRESH_MS: f64 = 8_000.0;
/// The poll's pace while the multisig is shown.
pub const POLL_MS: u32 = 12_000;
/// The pace while a sheet on one of its waiting transactions is open.
pub const OPEN_POLL_MS: u32 = 4_000;
/// A quiet network's chain is read every this many polls.
pub const QUIET_POLLS: u64 = 5;
/// How long a kept approval is sent again on its own.
pub const OUTBOX_PATIENCE_MS: f64 = 120_000.0;
const OUTBOX_BACKOFF_MAX_MS: f64 = 30_000.0;
const WAITING_CAP: usize = 20;
/// How long one read of a review or a preview is waited for.
pub const REVIEW_READ_MS: u32 = 8_000;
/// A review gathered this recently, at these very members, is signed from.
pub const REVIEW_FRESH_MS: f64 = 30_000.0;
/// A priced look this recent is what Execute signs (`PREVIEW_FRESH_MS`);
/// an older one, or one that did not price it, is taken again first.
pub const PREVIEW_FRESH_MS: f64 = 15_000.0;
/// A look that could not price the fee (or was not checked) is taken again sooner.
pub const UNPRICED_FRESH_MS: f64 = 5_000.0;
/// Round 5: the longest one look is waited for. "Approve and execute" once
/// stayed disabled four minutes on a look that never answered; past this
/// the look says `unchecked` — approving goes ahead, and Execute looks
/// again before its passkey.
pub const PREVIEW_CAP_MS: u64 = 15_000;
/// How long the home lets a ready transaction's look stand (its "Can't run
/// now"): looked at again on the poll after this.
pub const HOME_FRESH_MS: f64 = 60_000.0;
const RECENT_CAP: usize = 16;
/// An on-chain "no" is asked again after this.
pub const ONCHAIN_RECHECK_MS: f64 = 30_000.0;
/// How long an execution is followed.
pub const EXECUTION_LIMIT_MS: f64 = 180_000.0;
pub const EXECUTION_PAUSE_MS: u64 = 2_500;
pub const UNKNOWN_GRACE_MS: f64 = 30_000.0;
/// A user operation the relay has not seen after this was not taken.
pub const NOT_TAKEN_AFTER_MS: f64 = 6_000.0;
/// `keccak256("ExecutionSuccess(bytes32,uint256)")`.
pub const EXECUTION_SUCCESS: &str =
    "0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e";
const USER_OPERATION_EVENT: &str =
    "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f";
pub const FIRST_LOG_SPAN: u64 = 10_000;
pub const MAX_LOG_WINDOWS: u32 = 400;
const CREATED_MARGIN_MS: f64 = 60.0 * 60.0 * 1000.0;
const LOOKUPS_CAP: usize = 500;
const APPROVED_HASHES: &str = "0x7d832974";
const NONCE_CALL: &str = "0xaffed0e0";

// ---------------------------------------------------------------------------
// What one proposal is, as this device reads it
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigProposalStatus")
)]
pub enum ProposalStatus {
    /// Enough approvals, its nonce next: it can be executed now.
    Ready,
    /// Waiting for approvals, its nonce next.
    Pending,
    /// Behind an earlier nonce not executed or voided yet.
    Queued,
    /// Its nonce is used: executed, or voided.
    Done,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigApprovalRow"))]
pub struct ApprovalRow {
    pub owner: String,
    /// Verified, and its owner is a member on this chain now.
    pub counts: bool,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub approval: Approval,
    pub created_ms: f64,
    /// Signed here and not in the mailbox yet.
    pub unsent: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigProposalItem"))]
pub struct ProposalItem {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub tx: SafeTx,
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub review: multisig::Review,
    #[serde(with = "shell::dec_u128")]
    #[cfg_attr(feature = "bindings", ts(type = "string"))]
    pub nonce: u128,
    pub proposer: String,
    pub approvals: Vec<ApprovalRow>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub approved: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    pub status: ProposalStatus,
    /// This device's members that have not approved yet.
    pub can_approve: Vec<String>,
    /// Who on this device can execute it now.
    pub can_execute: Vec<String>,
    /// A cancellation at the same nonce that voids this one.
    pub cancelled_by: Option<String>,
    /// The other open proposals at its number.
    pub same_nonce: Vec<String>,
    /// This device sent its execution and it may still land.
    pub executing: bool,
    pub created_ms: f64,
    /// Something signed here for it has not reached the mailbox yet.
    pub unsent: bool,
    /// A rename where the multisig is not deployed: never executed there.
    pub rename_undeployed: bool,
}

/// Read one stored proposal (`readProposal`); `None` when it does not hash as itself.
pub fn read_proposal(
    safe: &str,
    members: &ChainMembers,
    p: &Proposal,
    on_chain: &[String],
    locals: &[LocalAccount],
    executing: bool,
) -> Option<ProposalItem> {
    let hash = multisig::safe_tx_hash(p.chain_id, safe, &p.safe_tx).ok()?;
    if !hash.eq_ignore_ascii_case(&p.safe_tx_hash) {
        return None;
    }
    let review = multisig::review(p.chain_id, safe, &p.safe_tx).ok()?;
    let nonce = shell::quantity(&p.safe_tx.nonce)?;
    let member = |o: &str| members.owners.iter().any(|m| same_address(m, o));
    let mut approvals: Vec<ApprovalRow> = Vec::new();
    for a in &p.approvals {
        let owner = a.approval.owner().to_owned();
        if approvals.iter().any(|r| same_address(&r.owner, &owner)) {
            continue;
        }
        let valid = match &a.approval {
            Approval::OnChain(_) => false,
            Approval::Ecdsa(e) => multisig::verify_ecdsa_approval(&hash, e)
                .is_ok_and(|v| same_address(&v.owner, &owner)),
            Approval::Passkey(k) => multisig::verify_approval(p.chain_id, &hash, k).is_ok(),
        };
        approvals.push(ApprovalRow {
            counts: valid && member(&owner),
            owner,
            approval: a.approval.clone(),
            created_ms: a.created_ms,
            unsent: false,
        });
    }
    for owner in on_chain {
        let at = approvals.iter().position(|r| same_address(&r.owner, owner));
        if at.is_some_and(|i| approvals[i].counts) {
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
            unsent: false,
        };
        match at {
            Some(i) => approvals[i] = row,
            None => approvals.push(row),
        }
    }
    let approved = approvals.iter().filter(|a| a.counts).count() as u64;
    let local = local_owners(locals, &members.owners);
    let approved_by = |o: &str| {
        approvals
            .iter()
            .any(|a| a.counts && same_address(&a.owner, o))
    };
    let can_approve: Vec<String> = local
        .iter()
        .filter(|o| !approved_by(&o.address))
        .map(|o| o.address.clone())
        .collect();
    let status = if nonce < members.nonce {
        ProposalStatus::Done
    } else if nonce > members.nonce {
        ProposalStatus::Queued
    } else if approved >= members.threshold {
        ProposalStatus::Ready
    } else {
        ProposalStatus::Pending
    };
    let owners_who: Vec<String> = local
        .iter()
        .filter(|o| approved + u64::from(!approved_by(&o.address)) >= members.threshold)
        .map(|o| o.address.clone())
        .collect();
    let anyone: Vec<String> = if approved >= members.threshold {
        locals
            .iter()
            .map(|a| a.address.clone())
            .filter(|a| !owners_who.iter().any(|x| same_address(x, a)))
            .collect()
    } else {
        Vec::new()
    };
    let rename_undeployed =
        matches!(review.action, Some(Action::Rename { .. })) && !members.deployed;
    let executing = status != ProposalStatus::Done && executing;
    let unreadable = review.action.is_none();
    let can_execute = if matches!(status, ProposalStatus::Done | ProposalStatus::Queued)
        || unreadable
        || rename_undeployed
        || executing
    {
        Vec::new()
    } else {
        owners_who.into_iter().chain(anyone).collect()
    };
    Some(ProposalItem {
        chain_id: p.chain_id,
        safe_tx_hash: hash,
        tx: p.safe_tx.clone(),
        nonce,
        proposer: p.proposer.clone(),
        approved,
        threshold: members.threshold,
        status,
        can_approve: if unreadable || status == ProposalStatus::Done {
            Vec::new()
        } else {
            can_approve
        },
        can_execute,
        cancelled_by: None,
        same_nonce: Vec::new(),
        executing,
        created_ms: p.created_ms,
        unsent: false,
        rename_undeployed,
        review,
        approvals,
    })
}

/// A cancellation voids the others at its nonce; every open proposal knows
/// the others waiting at its number (`markCancellations`).
pub fn mark_cancellations(items: &mut [ProposalItem]) {
    let cancels: Vec<(u128, String)> = items
        .iter()
        .filter(|c| matches!(c.review.action, Some(Action::Cancel)))
        .map(|c| (c.nonce, c.safe_tx_hash.clone()))
        .collect();
    for (nonce, hash) in &cancels {
        for i in items.iter_mut() {
            if i.safe_tx_hash != *hash && i.nonce == *nonce {
                i.cancelled_by = Some(hash.clone());
            }
        }
    }
    let open: Vec<(u128, String)> = items
        .iter()
        .filter(|i| i.status != ProposalStatus::Done)
        .map(|i| (i.nonce, i.safe_tx_hash.clone()))
        .collect();
    for i in items.iter_mut() {
        if i.status == ProposalStatus::Done {
            continue;
        }
        i.same_nonce = open
            .iter()
            .filter(|(n, h)| *n == i.nonce && *h != i.safe_tx_hash)
            .map(|(_, h)| h.clone())
            .collect();
    }
}

/// One chain of the multisig as read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigChainState"))]
pub struct ChainState {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub members: ChainMembers,
    pub items: Vec<ProposalItem>,
    /// The mailbox's list was read just now (else no nonce may come from it).
    pub mailbox: bool,
}

/// The nonce a new proposal takes: after every open one (`nextNonceOf`).
pub fn next_nonce_of(chain: &ChainState) -> u128 {
    let mut n = chain.members.nonce;
    for i in &chain.items {
        if i.status != ProposalStatus::Done && i.nonce >= n {
            n = i.nonce + 1;
        }
    }
    n
}

/// A network the mailbox holds proposals on that this device lacks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMissingChain"))]
pub struct MissingChain {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub count: u64,
}

/// The cross-chain list split by chain (`splitListed`).
pub fn split_listed(
    listed: &[Proposal],
    chain_ids: &[u64],
    networks: &[u64],
) -> (BTreeMap<u64, Vec<Proposal>>, Vec<u64>, Vec<MissingChain>) {
    let mut lists: BTreeMap<u64, Vec<Proposal>> = BTreeMap::new();
    let mut proposed: Vec<u64> = Vec::new();
    for p in listed {
        if !proposed.contains(&p.chain_id) {
            proposed.push(p.chain_id);
        }
        lists.entry(p.chain_id).or_default().push(p.clone());
    }
    let mut have: Vec<u64> = Vec::new();
    for id in proposed.iter().chain(chain_ids) {
        if networks.contains(id) && !have.contains(id) {
            have.push(*id);
        }
    }
    let used = multisig::multisig_networks(&multisig::NetworkEvidence {
        proposals: proposed.clone(),
        kept: chain_ids.to_vec(),
        have: Some(have.clone()),
        ..multisig::NetworkEvidence::default()
    });
    let missing = multisig::missing_networks(&multisig::NetworkEvidence {
        proposals: proposed,
        have: Some(have),
        ..multisig::NetworkEvidence::default()
    })
    .into_iter()
    .map(|chain_id| MissingChain {
        chain_id,
        count: lists.get(&chain_id).map_or(0, |l| l.len() as u64),
    })
    .collect();
    (lists, used, missing)
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// What a new proposal is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigProposalKind"))]
pub enum ProposalKind {
    /// The send flow's confirm: these transfers.
    Transfers {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        transfers: Vec<multisig::Transfer>,
    },
    /// Rename the record (a transaction of the multisig's own).
    Rename { name: String },
    /// A change of members or rule, built against the chain's members now
    /// (the members machine's [`super::multisig_members::PlanNetwork::proposal`]
    /// is this, per network).
    MemberChange { change: multisig::MemberChange },
    /// A dApp's calls, as one transaction.
    Calls {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        calls: Vec<multisig::CallArg>,
        origin: Option<String>,
    },
    /// Void the proposal at this nonce.
    Cancel {
        #[cfg_attr(feature = "bindings", ts(type = "string"))]
        nonce: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "MultisigProposalsEvent")
)]
pub enum Event {
    /// The store is read (outbox, kept lists, executions in flight).
    Start {
        service_url: String,
        accounts: Vec<LocalAccount>,
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        networks: Vec<u64>,
        /// Contacts, own accounts and multisigs the person knows (for the review).
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        known: Vec<Known>,
    },
    /// Follow this multisig (or none): read now, then on every tick.
    Follow {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        account: Option<MultisigAccount>,
    },
    /// The followed multisig's record changed (a network added).
    Account {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        account: MultisigAccount,
    },
    LocalAccounts {
        accounts: Vec<LocalAccount>,
    },
    Networks {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        networks: Vec<u64>,
    },
    Known {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        known: Vec<Known>,
    },
    Service {
        url: String,
    },
    /// The poll's round.
    Tick,
    /// Read now (shown again, focus, after an action). `fresh`: a read
    /// begun after this event, never one under way.
    Refresh {
        #[serde(default)]
        fresh: bool,
    },
    /// The person's Retry, or back online: ask the service now.
    Retry,
    Online {
        online: bool,
    },
    /// A sheet on a waiting transaction opened (`watch`) / closed.
    Watch,
    Unwatch,
    /// Gather the one review of a proposal (its sheet), or of the draft.
    /// `blind`: the clear reading's `sign_anyway` — a call Vela cannot read
    /// (`ProposalReview.read(blind)`: the Blind concern, `sign.anyway`).
    Review {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        #[serde(default)]
        blind: bool,
    },
    /// Draw a new proposal at the next free nonce, with its review — no
    /// passkey yet (`reviewFirst`).
    Draft {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        proposal: ProposalKind,
    },
    /// "Propose and approve": the gate, then `as_member`'s passkey (else the
    /// one used here last, else the first).
    ConfirmDraft {
        as_member: Option<String>,
    },
    /// Withdraw the draft (nothing more is asked or sent).
    Withdraw,
    Approve {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        owner: String,
    },
    /// What executing it would be: the preview.
    Preview {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        executor: String,
    },
    Execute {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        executor: String,
    },
    /// core-ms-2: "Approve and execute", one press, offered where
    /// [`ProposalsView::approve_execute`] names `executor`: its execution —
    /// the same look, the same gate and review as Execute — counts its own
    /// approval as the executor's (`msg.sender`). Should only the execution
    /// be unable to run, stopped before any signature (a fee it cannot pay,
    /// a transaction that would fail, a look that could not price or check
    /// it), the approval is still given and kept (r4-11): then
    /// `actions["execute:<hash>"]` is `stopped` (why) and
    /// `actions["approve:<hash>"]` the approval's own outcome.
    ApproveExecute {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        executor: String,
    },
    /// core-ms-2: [`Event::ConfirmDraft`], then — when the proposer's own
    /// approval makes it ready ([`DraftView::execute_now`]: a site's
    /// transaction a one-approval multisig runs at once) — its execution by
    /// that member, as [`Event::ApproveExecute`]: the site gets its
    /// transaction, not a wait for approvals nobody else has to give.
    ProposeExecute {
        #[serde(default)]
        as_member: Option<String>,
    },
    /// core-ms-2: the shell answered the site's `wallet_getCallsStatus` read
    /// with `calls_status[id]`: forgotten (answers not taken are kept at most
    /// [`flows::CALLS_STATUS_KEPT`], the oldest going first).
    CallsStatusTaken {
        id: String,
    },
    /// Send again what is kept (one transaction's, or all).
    SendAgain {
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        chain_id: Option<u64>,
        safe_tx_hash: Option<String>,
    },
    #[serde(skip)]
    Changed,
    // -- presence and the timeline (`presence.svelte.ts`, `timeline.ts`) ----
    /// Every multisig on this device (the accounts machine's list): what
    /// waits on each is said in `presence`.
    Accounts {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        accounts: Vec<MultisigAccount>,
    },
    /// Read every multisig not shown again: the shell sends it every
    /// [`super::multisig_presence::OTHERS_EVERY_MS`] while a page is visible,
    /// and when the page is shown again. Quiet; skipped during an outage.
    OthersTick,
    /// A done proposal's sheet opened: read how it ran (its execution's
    /// receipt, else what this device recorded) for its timeline.
    Timeline {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
    },
    // -- a dApp's request from a multisig (`dapp-request.ts`) ---------------
    /// A site asks `account` (the multisig it was granted, followed for the
    /// request's life) for `method`: refused (`multisigRefusal`), or read
    /// (`dapp_rpc::multisig_calls`) and drawn as a proposal with its review —
    /// nothing signed before `ConfirmDraft`. `limit_ms`: how long from the
    /// claim before the passkey the request can still be answered.
    DappRequest {
        id: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        account: Box<MultisigAccount>,
        method: String,
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        params: Value,
        origin: String,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        #[serde(default)]
        limit_ms: Option<f64>,
    },
    /// The person declined or closed the request's sheet, or its page left:
    /// `4001` before it was proposed, "proposed, waiting" after.
    DappLeave {
        id: String,
    },
    /// The shell answered the site with the request's answer: forgotten.
    DappSettled {
        id: String,
    },
    /// `wallet_getCallsStatus` of a batch this multisig answered with
    /// (`calls_id`, the proposal's safeTxHash): answered in `calls_status[id]`.
    CallsStatus {
        id: String,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        calls_id: String,
    },
    // -- a proposal link (`linked.svelte.ts`) -------------------------------
    /// Open the proposal a link named ([`Notice::OpenProposal`]): on the
    /// followed multisig, or `preview` — one with no member here, read only.
    OpenLinked {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        preview: Option<Box<MultisigAccount>>,
    },
    CloseLinked,
    // -- an outside member's wallet (`approveExternally`) -------------------
    /// Approve this proposal from an outside member's wallet (no member on
    /// this device): the gate, then [`Notice::OutsideSign`].
    ApproveOutside {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
    },
    /// What the outside session came to for the SafeTx it was handed.
    OutsideAnswered {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
        outcome: super::multisig_outside::session::SessionOutcome,
    },
    /// Members the passkey index knows as Vela accounts: never offered to an
    /// outside wallet to sign for.
    IndexedMembers {
        addresses: Vec<String>,
    },
    /// r7: how each member still to approve this proposal signs (its sheet
    /// opened): the chain read for each — code on the multisig's networks,
    /// Vela's passkey signer configuration — and the one rule
    /// ([`ProposalsView::signers`]).
    ReadSigners {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
        safe_tx_hash: String,
    },
}

/// One member still to approve, and how it signs (`signers.kindOf`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigSignerRow"))]
pub struct SignerRow {
    pub owner: String,
    /// `None` while that is still being read: nothing is offered on a guess.
    pub kind: Option<SignerKind>,
}

/// How a proposal's remaining approvals can come (r4C-10, r7).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigItemSigners"))]
pub struct ItemSigners {
    /// The members on its network that have not approved yet, in the
    /// chain's order, each with how it signs.
    pub rows: Vec<SignerRow>,
    /// Of them, those an outside wallet may be asked to sign for — the
    /// outside-wallet link names them: a plain account, or one nothing
    /// says (beside the Vela way). Never this device's, a Vela account, a
    /// smart contract account, or one still being read.
    pub outside: Vec<String>,
    /// r7-8: those that approve in Vela on their own device (the Vela link)
    /// — only a member something shows is a Vela account; never an outside
    /// member nothing has said yet.
    pub in_vela: Vec<String>,
    /// r7-8: those nothing says how they sign yet: either way is offered,
    /// and said once — never listed as Vela's and the outside wallet's both.
    pub either: Vec<String>,
    /// Smart contract members: they approve from their own app, which Vela
    /// can't collect yet (said; no link offered).
    pub contract: Vec<String>,
    /// Enough approvals are in: nobody else is asked (every list empty).
    pub enough: bool,
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

/// Where one action stands.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigActionState"))]
pub enum ActionState {
    Working,
    /// Done: its safeTxHash; `kept`: signed and kept, on its way to the mailbox.
    Done {
        safe_tx_hash: String,
        kept: bool,
    },
    Cancelled,
    /// The mailbox's list could not be read: nothing was proposed.
    MailboxUnreadable,
    /// No member of it is on this device (an outside wallet proposes it —
    /// the outside machine).
    NoMemberHere,
    /// Read-only here: its genesis is refused on this network.
    SigningRefused {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    /// Vela does not sign this transaction.
    SigningBlocked,
    /// Not checked yet: try again later.
    SigningUnchecked,
    /// The person declined the risk.
    RiskDeclined,
    /// The core refused it (an unreadable transaction, a member change it will not build).
    Refused {
        reason: String,
    },
    /// The mailbox refused it.
    MailboxRefused {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        refusal: Value,
    },
    /// The preview stops it before the passkey.
    Stopped {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        preview: Value,
    },
    Failed {
        detail: String,
    },
    /// No member here: an outside member's wallet is asked to sign it over
    /// a live session ([`Notice::OutsideSign`]); `owners`: who may.
    OutsideAsked {
        safe_tx_hash: String,
        owners: Vec<String>,
    },
    /// No member here, and none can sign through an outside wallet (every
    /// member is a Vela account, which approves in Vela on its own device).
    /// Nothing was proposed.
    NoOutsideSigner,
}

/// Where one kept approval stands (`OutboxEntry`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigOutboxEntry"))]
pub struct OutboxEntry {
    /// `busy`: the service said wait; `away`: nothing answered.
    pub why: String,
    /// When it is sent again; `None`: on the person's word (Send again).
    pub retry_at: Option<f64>,
}

/// The draft the person reviews before the passkey.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigDraft"))]
pub struct DraftView {
    pub item: ProposalItem,
    /// The one review (`multisig_review`), when gathered.
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub review: Option<Value>,
    /// The members here that could propose it.
    pub members_here: Vec<String>,
    /// The one offered first (picked last time, else the first).
    pub default_member: Option<String>,
    /// core-ms-2: a site's transaction the proposer's own approval makes
    /// ready: "Approve and execute" ([`Event::ProposeExecute`]), its fee said.
    pub execute_now: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigProposalsView"))]
pub struct ProposalsView {
    pub followed: Option<String>,
    pub chains: Vec<ChainState>,
    /// Every proposal, by nonce then age.
    pub items: Vec<ProposalItem>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub open_count: u64,
    /// Open proposals one of this device's members can approve (the
    /// switcher's "N waiting", presence).
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub waiting_on_me: u64,
    pub missing: Vec<MissingChain>,
    /// What is shown is the list an earlier page kept (when it was read).
    pub cached_at: Option<f64>,
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub unreadable: Vec<u64>,
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub history_unreadable: Vec<u64>,
    pub offline: bool,
    pub read_at: Option<f64>,
    pub loading: bool,
    pub stale: bool,
    /// Stale because the service is away (said with a Retry).
    pub unreachable: bool,
    /// The poll's pace now.
    pub pace_ms: u32,
    pub outbox: BTreeMap<String, OutboxEntry>,
    pub draft: Option<DraftView>,
    /// Per action (`propose`, or `approve:<hash>`, `execute:<hash>`).
    pub actions: BTreeMap<String, ActionState>,
    /// The one review per `<chainId>:<safeTxHash>`.
    #[cfg_attr(feature = "bindings", ts(type = "Record<string, unknown>"))]
    pub reviews: BTreeMap<String, Value>,
    /// The execution preview per executor (`preflight`'s one look per
    /// execution and executor), keyed
    /// `<chainId>:<safeTxHash lowercase>:<executor lowercase>`.
    #[cfg_attr(feature = "bindings", ts(type = "Record<string, unknown>"))]
    pub previews: BTreeMap<String, Value>,
    /// How each execution followed here ended (or stands), per
    /// `<chainId>:<safeTxHash lowercase>`; also what ran when a look found
    /// its number used (`settleSpent`).
    #[cfg_attr(feature = "bindings", ts(type = "Record<string, unknown>"))]
    pub executions: BTreeMap<String, Value>,
    /// An executed proposal's frozen facts, per `<chainId>:<safeTxHash
    /// lowercase>` (read on [`Event::Timeline`]).
    pub frozen: BTreeMap<String, super::multisig_presence::FrozenFacts>,
    /// The followed multisig's member used here last (lowercase), noted when
    /// more than one of its members is on this device (`lastActor`).
    pub last_actor: Option<String>,
    /// What waits on this device, per multisig on it (lowercase address):
    /// `None` until read — never a zero it did not see; a multisig not read
    /// yet on this page says what the list kept from an earlier one says.
    pub presence: BTreeMap<String, Option<super::multisig_presence::Waiting>>,
    /// A multisig other than the followed one has something waiting (the
    /// dot on the switcher's button).
    pub others_waiting: bool,
    /// Everyone who is a member of the followed multisig on some network
    /// (`membersAnywhere`): its members now where it is deployed, its
    /// original members wherever it is not. Who may sign on one network is
    /// that network's alone; this only says whether any member is here.
    pub members_anywhere: Vec<String>,
    /// Each open proposal's one word, per `<chainId>:<safeTxHash>`.
    pub standings: BTreeMap<String, super::multisig_presence::Standing>,
    /// What the waiting group says of the list.
    pub waiting_note: Option<super::multisig_presence::WaitingNote>,
    /// Each proposal's story, per `<chainId>:<safeTxHash>`.
    pub timelines: BTreeMap<String, Vec<super::multisig_presence::TimelineStep>>,
    /// The dApp requests held here, by request id.
    pub dapp: BTreeMap<String, super::multisig_dapp::HeldRequest>,
    /// `wallet_getCallsStatus` answers, by request id.
    pub calls_status: BTreeMap<String, super::multisig_dapp::DappAnswer>,
    /// The proposal a link opened.
    pub linked: Option<flows::LinkedView>,
    /// r7: per `<chainId>:<safeTxHash>` asked ([`Event::ReadSigners`]), how
    /// its remaining approvals can come.
    pub signers: BTreeMap<String, ItemSigners>,
    /// core-ms-2: per `<chainId>:<safeTxHash lowercase>`, the members here
    /// whose one press approves and executes it ([`Event::ApproveExecute`]):
    /// short of one approval, theirs reaches the threshold, and their last
    /// look (at these approvals) does not say they cannot pay (priced and
    /// payable, or not priced or checked in time: it looks again first). Any
    /// other member here approves only; an executor of a ready one executes.
    pub approve_execute: BTreeMap<String, Vec<String>>,
    /// core-ms-2: the header's rule chip for the multisig followed — the
    /// rule in force on each network it is used on (its members now where it
    /// is deployed, its original ones where not; the original alone before
    /// any is read): "Needs 2 of 3", or that it differs by network.
    pub rule_chip: Option<super::multisig_members::RuleChip>,
    /// core-ms-2: per multisig here (lowercase address), the waiting line's
    /// parts in order, only the counts that are not zero ("1 needs your
    /// approval · 1 ready to execute") — the switcher's rows.
    pub waiting_parts: BTreeMap<String, Vec<super::multisig_presence::WaitingPart>>,
    /// core-ms-2: the line at the top of the followed multisig's home, when
    /// something waits there — its parts and the transaction it opens.
    pub banner: Option<super::multisig_presence::WaitingBanner>,
    /// core-ms-2: the followed multisig is deployed on a network read (its
    /// first steps are behind it).
    pub set_up: bool,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// An execution sent from this device (`InFlight`), kept under `vela.multisigExecuting`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InFlight {
    pub chain_id: u64,
    pub multisig: String,
    pub safe_tx_hash: String,
    pub nonce: String,
    pub user_op_hash: String,
    pub by: String,
    pub maybe_sent: bool,
    #[serde(serialize_with = "shell::int_ms")]
    pub started_ms: f64,
    #[serde(serialize_with = "shell::int_ms")]
    pub created_ms: f64,
}

const EXECUTING_KEY: &str = "vela.multisigExecuting";

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeptChain {
    #[serde(rename = "chainId")]
    chain_id: u64,
    members: ChainMembers,
    proposals: Vec<Proposal>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KeptList {
    service: String,
    #[serde(serialize_with = "shell::int_ms")]
    at: f64,
    chains: Vec<KeptChain>,
}

struct Draft {
    chain: ChainState,
    tx: SafeTx,
    item: ProposalItem,
    review: Option<Value>,
}

#[derive(Default)]
pub struct State {
    service: Service,
    chain: Chain,
    locals: Vec<LocalAccount>,
    networks: Vec<u64>,
    known: Vec<Known>,
    account: Option<MultisigAccount>,
    chains: Vec<ChainState>,
    lists: BTreeMap<u64, Vec<Proposal>>,
    source: Option<String>,
    missing: Vec<MissingChain>,
    cached_at: Option<f64>,
    unreadable: Vec<u64>,
    history_unreadable: Vec<u64>,
    offline: bool,
    read_at: Option<f64>,
    loading: bool,
    stale: bool,
    ticks: u64,
    inflight: bool,
    queued: bool,
    held: u32,
    outbox: BTreeMap<String, Submission>,
    outbox_entries: BTreeMap<String, OutboxEntry>,
    outbox_since: BTreeMap<String, f64>,
    outbox_tries: BTreeMap<String, u32>,
    waiting: BTreeMap<String, KeptList>,
    on_chain_seen: BTreeMap<String, (bool, f64)>,
    recent: Vec<(String, f64, ReviewInput)>,
    executing: BTreeMap<String, InFlight>,
    lookups: BTreeMap<String, String>,
    spans: BTreeMap<u64, u64>,
    last_actor: BTreeMap<String, String>,
    draft: Option<Draft>,
    draft_round: u64,
    actions: BTreeMap<String, ActionState>,
    reviews: BTreeMap<String, Value>,
    previews: BTreeMap<String, Value>,
    /// Each executor's last look ([`preview_key`]): when it was taken, what it said.
    preview_plans: BTreeMap<String, (f64, Preview)>,
    /// The looks under way ([`preview_key`]): never two at once.
    looking: BTreeSet<String>,
    verdicts: BTreeMap<String, Value>,
    /// The executions being watched ([`exec_key`]): each flight followed once.
    watching: BTreeSet<String>,
    /// Presence, timelines, dApp requests, links, outside wallets.
    more: flows::More,
    /// r7: the proposals whose signers were asked (`<chainId>:<safeTxHash>`).
    signers_asked: BTreeSet<(u64, String)>,
    /// What the chains said of each member read (lowercase address).
    signer_reads: BTreeMap<String, SignerRead>,
}

impl State {
    /// How each member still to approve `hash` on `chain_id` signs, and who
    /// an outside wallet may sign for (`kindOf`, `plainAccounts`).
    fn item_signers(
        &self,
        items: &[ProposalItem],
        chain_id: u64,
        hash: &str,
    ) -> Option<ItemSigners> {
        // The followed list's proposal, or a link's read-only one (ms-21):
        // its own list says who approved with what.
        let (item, chain, account) = flows::find_item(self, chain_id, hash)?;
        let members = chain.members;
        let items = if self.followed(&account.address) {
            items
        } else {
            &chain.items[..]
        };
        let account = &account;
        let rows_of = items.iter().flat_map(|i| {
            i.approvals
                .iter()
                .map(|a| (a.owner.as_str(), a.counts, &a.approval))
        });
        let facts = |owner: &str| {
            let local = self.locals.iter().any(|l| same_address(&l.address, owner));
            let kept = account
                .members
                .as_ref()
                .and_then(|m| m.get(&owner.to_ascii_lowercase()))
                .and_then(|i| i.kind.clone());
            let approved = signers::approved_with_in(rows_of.clone(), owner);
            match self.signer_reads.get(&owner.to_ascii_lowercase()) {
                Some(read) => read.facts(Some(chain_id), local, approved, kept),
                None => signers::SignerFacts {
                    local,
                    approved_with: approved,
                    kept,
                    ..signers::SignerFacts::default()
                },
            }
        };
        let missing: Vec<String> = members
            .owners
            .iter()
            .filter(|o| {
                !item
                    .approvals
                    .iter()
                    .any(|a| a.counts && same_address(&a.owner, o))
            })
            .cloned()
            .collect();
        let rows: Vec<SignerRow> = missing
            .iter()
            .map(|o| SignerRow {
                owner: o.clone(),
                kind: signers::signer_kind(&facts(o)),
            })
            .collect();
        // Nobody else is asked once enough approved, nor for one done or one
        // Vela cannot read (r4D-6).
        let enough = item.approved >= item.threshold;
        let asks = !enough && item.status != ProposalStatus::Done && item.review.action.is_some();
        let of = |kinds: &[SignerKind]| -> Vec<String> {
            if !asks {
                return Vec::new();
            }
            rows.iter()
                .filter(|r| r.kind.is_some_and(|k| kinds.contains(&k)))
                .map(|r| r.owner.clone())
                .collect()
        };
        Some(ItemSigners {
            outside: of(&[SignerKind::Wallet, SignerKind::Unsure]),
            in_vela: of(&[SignerKind::Vela]),
            either: of(&[SignerKind::Unsure]),
            contract: of(&[SignerKind::Contract]),
            enough,
            rows,
        })
    }

    fn followed(&self, address: &str) -> bool {
        self.account
            .as_ref()
            .is_some_and(|a| same_address(&a.address, address))
    }

    fn items(&self) -> Vec<ProposalItem> {
        let mut items: Vec<ProposalItem> =
            self.chains.iter().flat_map(|c| c.items.clone()).collect();
        items.sort_by(|a, b| {
            a.nonce
                .cmp(&b.nonce)
                .then(a.created_ms.total_cmp(&b.created_ms))
        });
        items
    }

    fn item(&self, chain_id: u64, hash: &str) -> Option<(ProposalItem, ChainMembers)> {
        let c = self.chains.iter().find(|c| c.chain_id == chain_id)?;
        let i = c
            .items
            .iter()
            .find(|i| i.safe_tx_hash.eq_ignore_ascii_case(hash))?;
        Some((i.clone(), c.members.clone()))
    }

    fn is_executing(&self, chain_id: u64, safe: &str, hash: &str, now: f64) -> bool {
        self.executing
            .get(&exec_key(chain_id, safe, hash))
            .is_some_and(|f| now - f.started_ms < EXECUTION_LIMIT_MS + 60_000.0)
    }
}

fn exec_key(chain_id: u64, safe: &str, hash: &str) -> String {
    format!(
        "{chain_id}:{}:{}",
        safe.to_ascii_lowercase(),
        hash.to_ascii_lowercase()
    )
}

/// One execution by one executor (`previewKey`):
/// `<chainId>:<safeTxHash lowercase>:<executor lowercase>`.
fn preview_key(chain_id: u64, hash: &str, executor: &str) -> String {
    format!(
        "{chain_id}:{}:{}",
        hash.to_ascii_lowercase(),
        executor.to_ascii_lowercase()
    )
}

/// An execution's verdict (and a frozen fact) per `<chainId>:<safeTxHash lowercase>`.
fn verdict_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{}", hash.to_ascii_lowercase())
}

#[derive(Default)]
pub struct Model {
    state: Shared<State>,
}

/// Presence, timelines, a dApp's requests, links and outside wallets: the
/// machine's later rules, kept in their own file.
#[path = "multisig_proposals_flows.rs"]
pub mod flows;

#[derive(Default)]
pub struct MultisigProposals;

type Cmd = Command<MultisigEffect, Event>;

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

impl App for MultisigProposals {
    type Event = Event;
    type Model = Model;
    type ViewModel = ProposalsView;
    type Effect = MultisigEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        match event {
            Event::Changed => render(),
            Event::Start {
                service_url,
                accounts,
                networks,
                known,
            } => {
                {
                    let mut s = lock(&model.state);
                    s.service.set_url(&service_url);
                    s.locals = accounts;
                    s.networks = networks;
                    s.known = known;
                }
                flow(model, |io, st| async move {
                    load(&io, &st).await;
                    flush_outbox(&io, &st, None).await;
                    // A Follow that came before the store was read.
                    let now = io.now().await;
                    let flights = flights_to_resume(&mut lock(&st), now);
                    resume_flights(&io, &st, flights);
                })
            }
            Event::Follow { account } => {
                {
                    let mut s = lock(&model.state);
                    let same = match (&s.account, &account) {
                        (Some(a), Some(b)) => same_address(&a.address, &b.address),
                        (None, None) => true,
                        _ => false,
                    };
                    if !same {
                        s.chains.clear();
                        s.missing.clear();
                        s.lists.clear();
                        s.stale = false;
                        s.cached_at = None;
                        s.unreadable.clear();
                        s.history_unreadable.clear();
                        s.read_at = None;
                        s.draft = None;
                        s.reviews.clear();
                        s.previews.clear();
                        s.signers_asked.clear();
                        s.signer_reads.clear();
                    }
                    s.account = account.clone();
                    if account.is_some() && s.chains.is_empty() {
                        seed(&mut s);
                    }
                }
                if account.is_none() {
                    return render();
                }
                flow(model, |io, st| async move {
                    // r4b-7: what this device sent for it before a reload is
                    // watched again, once its list is read.
                    let now = io.now().await;
                    let flights = flights_to_resume(&mut lock(&st), now);
                    refresh(&io, &st, false, false).await;
                    resume_flights(&io, &st, flights);
                })
            }
            Event::Account { account } => {
                let mut s = lock(&model.state);
                if s.followed(&account.address) {
                    s.account = Some(account);
                }
                render()
            }
            Event::LocalAccounts { accounts } => {
                lock(&model.state).locals = accounts;
                render()
            }
            Event::Networks { networks } => {
                lock(&model.state).networks = networks;
                render()
            }
            Event::Known { known } => {
                lock(&model.state).known = known;
                render()
            }
            Event::Service { url } => {
                lock(&model.state).service.set_url(&url);
                render()
            }
            Event::Tick => flow(model, |io, st| async move {
                refresh(&io, &st, true, false).await;
                look_at_ready(&io, &st).await;
            }),
            Event::Refresh { fresh } => flow(model, move |io, st| async move {
                refresh(&io, &st, false, fresh).await
            }),
            Event::Retry => flow(model, |io, st| async move {
                {
                    let s = lock(&st);
                    lock(&s.service.outage).reset();
                }
                lock(&st).history_unreadable.clear();
                refresh(&io, &st, false, false).await;
            }),
            Event::Online { online } => {
                lock(&model.state).offline = !online;
                if !online {
                    return render();
                }
                flow(model, |io, st| async move {
                    {
                        let s = lock(&st);
                        lock(&s.service.outage).reset();
                    }
                    refresh(&io, &st, false, false).await;
                })
            }
            Event::Watch => {
                let stale = {
                    let mut s = lock(&model.state);
                    s.held += 1;
                    s.read_at
                };
                flow(model, move |io, st| async move {
                    let now = io.now().await;
                    if stale.is_none_or(|at| now - at > f64::from(OPEN_POLL_MS)) {
                        refresh(&io, &st, false, false).await;
                    }
                })
            }
            Event::Unwatch => {
                let mut s = lock(&model.state);
                s.held = s.held.saturating_sub(1);
                render()
            }
            Event::Review {
                chain_id,
                safe_tx_hash,
                blind,
            } => flow(model, move |io, st| async move {
                // The followed list's proposal, or a link's read-only one.
                let found = flows::find_item(&lock(&st), chain_id, &safe_tx_hash);
                let Some((item, chain, account)) = found else {
                    return;
                };
                let mut input =
                    gather_review(&io, &st, &account, chain_id, &item.tx, &chain.members, None)
                        .await;
                input.blind = blind;
                let out = multisig_review::multisig_review(&input)
                    .ok()
                    .and_then(|r| serde_json::to_value(r).ok())
                    .unwrap_or(Value::Null);
                lock(&st)
                    .reviews
                    .insert(format!("{chain_id}:{}", item.safe_tx_hash), out);
            }),
            Event::ReadSigners {
                chain_id,
                safe_tx_hash,
            } => {
                let asked = {
                    let mut s = lock(&model.state);
                    // The followed list's proposal, or a link's read-only one (ms-21).
                    let found = flows::find_item(&s, chain_id, &safe_tx_hash)
                        .map(|(item, chain, account)| ((item, chain.members), account));
                    found.map(|((item, members), account)| {
                        s.signers_asked
                            .insert((chain_id, item.safe_tx_hash.clone()));
                        let owners: Vec<String> = members
                            .owners
                            .iter()
                            .filter(|o| !s.locals.iter().any(|l| same_address(&l.address, o)))
                            .cloned()
                            .collect();
                        let mut chain_ids = vec![chain_id];
                        for id in &account.chain_ids {
                            if !chain_ids.contains(id) {
                                chain_ids.push(*id);
                            }
                        }
                        (owners, chain_ids)
                    })
                };
                let Some((owners, chain_ids)) = asked else {
                    return render();
                };
                flow(model, move |io, st| async move {
                    let reads = owners.iter().map(|o| {
                        let (io, st, chain_ids) = (&io, &st, &chain_ids);
                        async move {
                            let read = signers::read_signer(io, chain_ids, o).await;
                            lock(st).signer_reads.insert(o.to_ascii_lowercase(), read);
                            io.emit(Event::Changed);
                        }
                    });
                    future::join_all(reads).await;
                })
            }
            Event::Draft { chain_id, proposal } => {
                {
                    let mut s = lock(&model.state);
                    s.draft = None;
                    s.draft_round += 1;
                    s.actions.insert("propose".to_owned(), ActionState::Working);
                }
                flow(model, move |io, st| async move {
                    draft(&io, &st, chain_id, proposal).await
                })
            }
            Event::ConfirmDraft { as_member } => propose_draft(model, as_member, false),
            Event::ProposeExecute { as_member } => propose_draft(model, as_member, true),
            Event::ApproveExecute {
                chain_id,
                safe_tx_hash,
                executor,
            } => {
                {
                    let mut s = lock(&model.state);
                    let hash = safe_tx_hash.to_ascii_lowercase();
                    s.actions
                        .insert(format!("execute:{hash}"), ActionState::Working);
                    s.actions.remove(&format!("approve:{hash}"));
                }
                flow(model, move |io, st| async move {
                    approve_execute(&io, &st, chain_id, &safe_tx_hash, &executor).await;
                })
            }
            Event::Withdraw => {
                let mut s = lock(&model.state);
                flows::on_withdraw(&mut s);
                s.draft = None;
                s.draft_round += 1;
                s.actions
                    .insert("propose".to_owned(), ActionState::Cancelled);
                render()
            }
            Event::Approve {
                chain_id,
                safe_tx_hash,
                owner,
            } => {
                let key = format!("approve:{}", safe_tx_hash.to_ascii_lowercase());
                lock(&model.state)
                    .actions
                    .insert(key.clone(), ActionState::Working);
                flow(model, move |io, st| async move {
                    let outcome = approve(&io, &st, chain_id, &safe_tx_hash, &owner).await;
                    if matches!(outcome, ActionState::Done { .. }) {
                        note_actor(&io, &st, chain_id, &owner).await;
                    }
                    lock(&st).actions.insert(key, outcome);
                    refresh(&io, &st, false, true).await;
                })
            }
            Event::Preview {
                chain_id,
                safe_tx_hash,
                executor,
            } => flow(model, move |io, st| async move {
                let Some(input) = execution_input(&st, chain_id, &safe_tx_hash, &executor) else {
                    return;
                };
                look_if_stale(&io, &st, &input, PREVIEW_FRESH_MS).await;
            }),
            Event::Execute {
                chain_id,
                safe_tx_hash,
                executor,
            } => {
                let key = format!("execute:{}", safe_tx_hash.to_ascii_lowercase());
                lock(&model.state)
                    .actions
                    .insert(key.clone(), ActionState::Working);
                flow(model, move |io, st| async move {
                    let input = execution_input(&st, chain_id, &safe_tx_hash, &executor);
                    let outcome = execute(&io, &st, chain_id, &safe_tx_hash, &executor).await;
                    flows::remember_execution(&io, &st, input.as_ref(), &outcome).await;
                    if matches!(outcome, ActionState::Done { .. }) {
                        note_actor(&io, &st, chain_id, &executor).await;
                    }
                    lock(&st).actions.insert(key, outcome);
                    refresh(&io, &st, false, true).await;
                })
            }
            Event::Accounts { .. }
            | Event::OthersTick
            | Event::Timeline { .. }
            | Event::DappRequest { .. }
            | Event::DappLeave { .. }
            | Event::DappSettled { .. }
            | Event::CallsStatus { .. }
            | Event::CallsStatusTaken { .. }
            | Event::OpenLinked { .. }
            | Event::CloseLinked
            | Event::ApproveOutside { .. }
            | Event::OutsideAnswered { .. }
            | Event::IndexedMembers { .. } => flows::update(event, model),
            Event::SendAgain {
                chain_id,
                safe_tx_hash,
            } => flow(model, move |io, st| async move {
                let safe = lock(&st).account.as_ref().map(|a| a.address.clone());
                let prefix = match (chain_id, safe_tx_hash, safe) {
                    (Some(c), Some(h), Some(s)) => Some(format!(
                        "{c}:{}:{}:",
                        s.to_ascii_lowercase(),
                        h.to_ascii_lowercase()
                    )),
                    _ => None,
                };
                flush_outbox(&io, &st, prefix).await;
                refresh(&io, &st, false, true).await;
            }),
        }
    }

    fn view(&self, model: &Model) -> ProposalsView {
        let mut s = lock(&model.state);
        let more = flows::view_more(&mut s);
        let items = s.items();
        let open: Vec<&ProposalItem> = items
            .iter()
            .filter(|i| i.status != ProposalStatus::Done)
            .collect();
        let down = lock(&s.service.outage).down;
        // core-ms-2: the waiting line's parts per multisig, and the followed
        // one's banner.
        let presence_parts: BTreeMap<String, Vec<super::multisig_presence::WaitingPart>> = more
            .presence
            .iter()
            .filter_map(|(k, w)| {
                let parts = w.as_ref()?.parts();
                (!parts.is_empty()).then(|| (k.clone(), parts))
            })
            .collect();
        let banner = s
            .account
            .as_ref()
            .and_then(|a| more.presence.get(&a.address.to_ascii_lowercase()))
            .and_then(Option::as_ref)
            .and_then(super::multisig_presence::banner_of);
        ProposalsView {
            followed: s.account.as_ref().map(|a| a.address.clone()),
            chains: s.chains.clone(),
            open_count: open.len() as u64,
            waiting_on_me: open.iter().filter(|i| !i.can_approve.is_empty()).count() as u64,
            items: items.clone(),
            missing: s.missing.clone(),
            cached_at: s.cached_at,
            unreadable: s.unreadable.clone(),
            history_unreadable: s.history_unreadable.clone(),
            offline: s.offline,
            read_at: s.read_at,
            loading: s.loading,
            stale: s.stale,
            unreachable: s.stale && down,
            pace_ms: if s.held > 0 || flows::watching(&s) {
                OPEN_POLL_MS
            } else {
                POLL_MS
            },
            outbox: s.outbox_entries.clone(),
            draft: s.draft.as_ref().map(|d| {
                let here = local_owners(&s.locals, &d.chain.members.owners);
                let address = s.account.as_ref().map(|a| a.address.to_ascii_lowercase());
                let last = address.and_then(|a| s.last_actor.get(&a).cloned());
                let default_member = last
                    .and_then(|l| {
                        here.iter()
                            .find(|o| same_address(&o.address, &l))
                            .map(|o| o.address.clone())
                    })
                    .or_else(|| here.first().map(|o| o.address.clone()));
                DraftView {
                    execute_now: !d.item.can_execute.is_empty() && flows::reviewing_request(&s),
                    item: d.item.clone(),
                    review: d.review.clone(),
                    members_here: here.into_iter().map(|o| o.address).collect(),
                    default_member,
                }
            }),
            actions: s.actions.clone(),
            reviews: s.reviews.clone(),
            previews: s.previews.clone(),
            executions: s.verdicts.clone(),
            frozen: more.frozen,
            last_actor: s
                .account
                .as_ref()
                .and_then(|a| s.last_actor.get(&a.address.to_ascii_lowercase()).cloned()),
            presence: more.presence,
            others_waiting: more.others_waiting,
            members_anywhere: s
                .account
                .as_ref()
                .map(|a| {
                    let chains: Vec<&ChainMembers> = s.chains.iter().map(|c| &c.members).collect();
                    super::multisig_members::members_anywhere(&a.owners, &chains)
                })
                .unwrap_or_default(),
            standings: more.standings,
            waiting_note: more.waiting_note,
            timelines: more.timelines,
            dapp: more.dapp,
            calls_status: more.calls_status,
            linked: more.linked,
            signers: s
                .signers_asked
                .iter()
                .filter_map(|(chain_id, hash)| {
                    let signers = s.item_signers(&items, *chain_id, hash)?;
                    Some((format!("{chain_id}:{hash}"), signers))
                })
                .collect(),
            approve_execute: items
                .iter()
                .filter_map(|i| {
                    let who = approve_executors(&s, i);
                    (!who.is_empty()).then(|| {
                        (
                            format!("{}:{}", i.chain_id, i.safe_tx_hash.to_ascii_lowercase()),
                            who,
                        )
                    })
                })
                .collect(),
            rule_chip: s.account.as_ref().and_then(|a| {
                use super::multisig_members::{current_rules, rule_chip, MemberRule};
                let chains: Vec<&ChainMembers> = s.chains.iter().map(|c| &c.members).collect();
                rule_chip(&current_rules(
                    MemberRule::of(a.threshold, a.owners.len()),
                    &chains,
                ))
            }),
            waiting_parts: presence_parts,
            banner,
            set_up: s.account.is_some() && s.chains.iter().any(|c| c.members.deployed),
        }
    }
}

// ---------------------------------------------------------------------------
// The store
// ---------------------------------------------------------------------------

async fn load(io: &Io<Event>, st: &Shared<State>) {
    let got = io
        .load(&[
            keys::OUTBOX,
            keys::WAITING,
            keys::DEPLOYED,
            keys::EXECUTIONS,
            EXECUTING_KEY,
            keys::LAST_ACTOR,
        ])
        .await;
    let get = |k: &str| got.get(k).cloned().flatten();
    let mut s = lock(st);
    s.outbox = get(keys::OUTBOX)
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    s.waiting = get(keys::WAITING)
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    s.chain.seed_deployed(get(keys::DEPLOYED).as_deref());
    s.lookups = get(keys::EXECUTIONS)
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    s.executing = get(EXECUTING_KEY)
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
    s.last_actor = get(keys::LAST_ACTOR)
        .and_then(|r| serde_json::from_str(&r).ok())
        .unwrap_or_default();
}

async fn save_json<T: Serialize>(io: &Io<Event>, key: &str, value: &T, empty_removes: bool) {
    let json = serde_json::to_value(value).unwrap_or(Value::Null);
    let empty = json.as_object().is_some_and(serde_json::Map::is_empty);
    io.store(
        key,
        if empty && empty_removes {
            None
        } else {
            Some(json.to_string())
        },
    )
    .await;
}

/// The list an earlier page kept, re-read against the members kept with it.
fn seed(s: &mut State) {
    let Some(account) = s.account.clone() else {
        return;
    };
    let Some(kept) = s
        .waiting
        .get(&account.address.to_ascii_lowercase())
        .cloned()
    else {
        return;
    };
    if kept.service != s.service.url() {
        return;
    }
    let mut chains = Vec::new();
    for c in kept.chains {
        let mut items: Vec<ProposalItem> = c
            .proposals
            .iter()
            .filter_map(|p| read_proposal(&account.address, &c.members, p, &[], &s.locals, false))
            .collect();
        mark_cancellations(&mut items);
        s.lists.insert(c.chain_id, c.proposals.clone());
        chains.push(ChainState {
            chain_id: c.chain_id,
            members: c.members,
            items,
            mailbox: false,
        });
    }
    s.chains = chains;
    s.source = Some(s.service.url());
    s.cached_at = Some(kept.at);
    s.read_at = Some(kept.at);
    s.stale = true;
}

// ---------------------------------------------------------------------------
// Reading (`readChain`, `refresh`)
// ---------------------------------------------------------------------------

fn outbox_key(req: &Submission) -> String {
    let hash = multisig::safe_tx_hash(req.chain_id, &req.safe, &req.safe_tx).unwrap_or_default();
    format!(
        "{}:{}:{}:{}",
        req.chain_id,
        req.safe.to_ascii_lowercase(),
        hash,
        req.approval.owner().to_ascii_lowercase()
    )
}

/// The mailbox's list with what this device signed and it lacks (`withKept`).
fn with_kept(
    s: &State,
    safe: &str,
    chain_id: u64,
    list: &[Proposal],
    now: f64,
) -> (Vec<Proposal>, BTreeMap<String, BTreeSet<String>>) {
    let mut out = list.to_vec();
    let mut unsent: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for req in s
        .outbox
        .values()
        .filter(|r| r.chain_id == chain_id && same_address(&r.safe, safe))
    {
        let Ok(hash) = multisig::safe_tx_hash(chain_id, safe, &req.safe_tx) else {
            continue;
        };
        let hash = hash.to_ascii_lowercase();
        let at = match out
            .iter()
            .position(|p| p.safe_tx_hash.eq_ignore_ascii_case(&hash))
        {
            Some(i) => i,
            None => {
                out.push(Proposal {
                    chain_id,
                    safe: safe.to_owned(),
                    safe_tx_hash: hash.clone(),
                    safe_tx: req.safe_tx.clone(),
                    proposer: req.approval.owner().to_owned(),
                    approvals: Vec::new(),
                    created_ms: now,
                    updated_ms: now,
                });
                out.len() - 1
            }
        };
        let owner = req.approval.owner().to_owned();
        if out[at]
            .approvals
            .iter()
            .any(|a| same_address(a.approval.owner(), &owner))
        {
            continue;
        }
        out[at].approvals.push(StoredApproval {
            approval: req.approval.clone(),
            created_ms: now,
        });
        unsent
            .entry(hash)
            .or_default()
            .insert(owner.to_ascii_lowercase());
    }
    (out, unsent)
}

/// Members with no approval in the mailbox that approved on chain (`approvedOnChain`).
async fn approved_on_chain(
    io: &Io<Event>,
    st: &Shared<State>,
    safe: &str,
    chain_id: u64,
    members: &ChainMembers,
    list: &[Proposal],
) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    if !members.deployed {
        return out;
    }
    let now = io.now().await;
    for p in list {
        let Some(nonce) = shell::quantity(&p.safe_tx.nonce) else {
            continue;
        };
        if nonce < members.nonce {
            continue;
        }
        let ask: Vec<String> = members
            .owners
            .iter()
            .filter(|o| {
                !p.approvals.iter().any(|a| {
                    same_address(a.approval.owner(), o)
                        && !matches!(a.approval, Approval::OnChain(_))
                })
            })
            .cloned()
            .collect();
        let reads = ask.iter().map(|owner| {
            let key = format!("{chain_id}:{safe}:{}:{owner}", p.safe_tx_hash).to_ascii_lowercase();
            let hash = p.safe_tx_hash.clone();
            async move {
                let known = lock(st).on_chain_seen.get(&key).copied();
                if let Some((yes, at)) = known {
                    if yes || now - at < ONCHAIN_RECHECK_MS {
                        return yes.then(|| owner.clone());
                    }
                }
                let data = format!(
                    "{APPROVED_HASHES}{:0>64}{:0>64}",
                    owner.trim_start_matches("0x").to_ascii_lowercase(),
                    hash.trim_start_matches("0x").to_ascii_lowercase()
                );
                let ret = io.eth_call(chain_id, safe, &data).await.ok()?;
                let yes = !ret.trim_start_matches("0x").bytes().all(|b| b == b'0');
                lock(st).on_chain_seen.insert(key, (yes, now));
                yes.then(|| owner.clone())
            }
        });
        let found: Vec<String> = future::join_all(reads)
            .await
            .into_iter()
            .flatten()
            .collect();
        if !found.is_empty() {
            out.insert(p.safe_tx_hash.to_ascii_lowercase(), found);
        }
    }
    out
}

struct ChainRead {
    state: ChainState,
    proposals: Vec<Proposal>,
}

/// One chain read now: members from the chain, proposals from the mailbox.
/// `Err`: the members could not be read.
#[allow(clippy::too_many_arguments)]
async fn read_chain(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    previous: Option<Vec<Proposal>>,
    background: bool,
    ask_mailbox: bool,
    listed: Option<Vec<Proposal>>,
    known: Option<ChainMembers>,
) -> Result<ChainRead, shell::ChainUnavailable> {
    let (service, chain) = {
        let s = lock(st);
        (s.service.clone(), s.chain.clone())
    };
    let members_read = async {
        match known {
            Some(m) => Ok(m),
            None => {
                chain
                    .read_members(io, chain_id, &account.address, &account.genesis)
                    .await
            }
        }
    };
    let list_read = async {
        if let Some(listed) = listed {
            return Some(listed);
        }
        if !ask_mailbox {
            return None;
        }
        match service
            .fetch_proposals(io, chain_id, &account.address, background)
            .await
        {
            Ok(a) => Some(a.map(|a| a.proposals).unwrap_or_default()),
            Err(_) => None,
        }
    };
    let (members, answer) = futures::join!(members_read, list_read);
    let members = members?;
    let mailbox = answer.is_some();
    let list = answer.unwrap_or_else(|| previous.unwrap_or_default());
    let now = io.now().await;
    let (shown, unsent) = with_kept(&lock(st), &account.address, chain_id, &list, now);
    let on_chain = approved_on_chain(io, st, &account.address, chain_id, &members, &shown).await;
    let s = lock(st);
    let mut items: Vec<ProposalItem> = shown
        .iter()
        .filter_map(|p| {
            let hash = p.safe_tx_hash.to_ascii_lowercase();
            let executing = s.is_executing(chain_id, &account.address, &hash, now);
            let mut item = read_proposal(
                &account.address,
                &members,
                p,
                on_chain.get(&hash).map_or(&[][..], Vec::as_slice),
                &s.locals,
                executing,
            )?;
            if let Some(owners) = unsent.get(&hash) {
                item.unsent = true;
                for row in &mut item.approvals {
                    row.unsent = owners.contains(&row.owner.to_ascii_lowercase());
                }
            }
            Some(item)
        })
        .collect();
    mark_cancellations(&mut items);
    Ok(ChainRead {
        state: ChainState {
            chain_id,
            members,
            items,
            mailbox,
        },
        proposals: list,
    })
}

fn has_open(list: &[Proposal], nonce: u128) -> bool {
    list.iter()
        .any(|p| shell::quantity(&p.safe_tx.nonce).is_none_or(|n| n >= nonce))
}

/// Re-read every chain the multisig is used on (`refresh`). One at a time:
/// a read asked for during another runs once it ends when `fresh`.
async fn refresh(io: &Io<Event>, st: &Shared<State>, tick: bool, fresh: bool) {
    {
        let mut s = lock(st);
        if s.inflight {
            if fresh {
                s.queued = true;
            }
            return;
        }
        s.inflight = true;
    }
    loop {
        refresh_once(io, st, tick).await;
        let again = {
            let mut s = lock(st);
            let again = s.queued;
            s.queued = false;
            if !again {
                s.inflight = false;
            }
            again
        };
        if !again {
            break;
        }
    }
    flows::after_refresh(io, st).await;
}

async fn refresh_once(io: &Io<Event>, st: &Shared<State>, tick: bool) {
    let (account, service, round, networks) = {
        let mut s = lock(st);
        let Some(account) = s.account.clone() else {
            return;
        };
        let round = if tick {
            s.ticks += 1;
            s.ticks
        } else {
            s.ticks = 0;
            0
        };
        let source = s.service.url();
        if s.source.as_deref() != Some(source.as_str()) {
            s.lists.clear();
            if s.source.is_some() {
                s.chains.clear();
            }
            s.source = Some(source);
        }
        s.loading = s.chains.is_empty();
        (account, s.service.clone(), round, s.networks.clone())
    };
    let started = io.now().await;
    io.emit(Event::Changed);
    let chain_ids = account.chain_ids.clone();
    let mut mailbox = service.available(io).await;
    let mut split = None;
    if mailbox {
        match service
            .fetch_all_proposals(io, &account.address, true)
            .await
        {
            Ok(Some(all)) => split = Some(split_listed(&all.proposals, &chain_ids, &networks)),
            Ok(None) => {}
            Err(_) => mailbox = false,
        }
    }
    let mut reading = chain_ids.clone();
    if let Some((_, used, missing)) = &split {
        for id in used {
            if !account.chain_ids.contains(id) {
                io.notice(Notice::Account {
                    address: account.address.clone(),
                    update: AccountUpdate::UseChain { chain_id: *id },
                })
                .await;
            }
            if !reading.contains(id) {
                reading.push(*id);
            }
        }
        let mut s = lock(st);
        if s.followed(&account.address) {
            s.missing.clone_from(missing);
        }
    }
    let reads = reading.iter().map(|chain_id| {
        let chain_id = *chain_id;
        let account = account.clone();
        let (known, previous) = {
            let s = lock(st);
            let followed = s.followed(&account.address);
            (
                followed.then(|| s.lists.get(&chain_id).cloned()).flatten(),
                followed
                    .then(|| s.chains.iter().find(|c| c.chain_id == chain_id).cloned())
                    .flatten(),
            )
        };
        let listed = split
            .as_ref()
            .map(|(lists, _, _)| lists.get(&chain_id).cloned().unwrap_or_default());
        async move {
            let quiet = tick
                && round % QUIET_POLLS != 0
                && previous.as_ref().is_some_and(|p| {
                    p.mailbox
                        && listed
                            .as_ref()
                            .is_some_and(|l| !has_open(l, p.members.nonce))
                        && !p
                            .items
                            .iter()
                            .any(|i| i.status != ProposalStatus::Done || i.executing)
                });
            let known_members = quiet
                .then(|| previous.as_ref().map(|p| p.members.clone()))
                .flatten();
            let read = read_chain(
                io,
                st,
                &account,
                chain_id,
                known,
                true,
                mailbox,
                listed,
                known_members,
            )
            .await;
            (chain_id, read, previous)
        }
    });
    let results = future::join_all(reads).await;
    let now = io.now().await;
    let mut failed = false;
    let mut unread = Vec::new();
    let mut next = Vec::new();
    {
        let mut s = lock(st);
        let followed = s.followed(&account.address);
        for (chain_id, read, previous) in results {
            match read {
                Ok(read) => {
                    if !read.state.mailbox {
                        failed = true;
                    } else if followed {
                        s.lists.insert(chain_id, read.proposals);
                    }
                    next.push(read.state);
                }
                Err(_) => {
                    failed = true;
                    unread.push(chain_id);
                    if let Some(mut p) = previous {
                        p.mailbox = false;
                        next.push(p);
                    }
                }
            }
        }
        if followed {
            let rank = |id: u64| {
                s.networks
                    .iter()
                    .position(|n| *n == id)
                    .unwrap_or(usize::MAX)
            };
            next.sort_by_key(|c| rank(c.chain_id));
            s.chains = next;
            s.stale = failed || now - started > SLOW_REFRESH_MS && failed;
            s.unreadable = unread;
            if !failed {
                s.cached_at = None;
                s.read_at = Some(now);
                let kept = KeptList {
                    service: s.service.url(),
                    at: now,
                    chains: s
                        .chains
                        .iter()
                        .map(|c| KeptChain {
                            chain_id: c.chain_id,
                            members: c.members.clone(),
                            proposals: s
                                .lists
                                .get(&c.chain_id)
                                .cloned()
                                .unwrap_or_default()
                                .into_iter()
                                .filter(|p| {
                                    shell::quantity(&p.safe_tx.nonce)
                                        .is_some_and(|n| n >= c.members.nonce)
                                })
                                .collect(),
                        })
                        .collect(),
                };
                s.waiting.insert(account.address.to_ascii_lowercase(), kept);
                let skip = s.waiting.len().saturating_sub(WAITING_CAP);
                let old: Vec<String> = s.waiting.keys().take(skip).cloned().collect();
                for k in old {
                    s.waiting.remove(&k);
                }
            }
            let done: Vec<String> = s
                .items()
                .iter()
                .filter(|i| i.status == ProposalStatus::Done)
                .map(|i| exec_key(i.chain_id, &account.address, &i.safe_tx_hash))
                .collect();
            for k in done {
                s.executing.remove(&k);
            }
        }
        s.loading = false;
    }
    let (waiting, executing) = {
        let s = lock(st);
        (s.waiting.clone(), s.executing.clone())
    };
    if !failed {
        save_json(io, keys::WAITING, &waiting, false).await;
    }
    save_json(io, EXECUTING_KEY, &executing, true).await;
    record_remote_executions(io, st, &account).await;
}

// ---------------------------------------------------------------------------
// The review's reads (`gatherReview`)
// ---------------------------------------------------------------------------

fn subject_key(
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
    members: &ChainMembers,
    origin: Option<&str>,
) -> String {
    json!([
        chain_id,
        safe.to_ascii_lowercase(),
        tx,
        members.deployed,
        members.threshold,
        members
            .owners
            .iter()
            .map(|o| o.to_ascii_lowercase())
            .collect::<Vec<_>>(),
        members.nonce.to_string(),
        origin
    ])
    .to_string()
}

/// The one review's reads ([`multisig_gather::gather_review`], the very reads
/// the connect page makes for an outside owner), kept a moment for the gate.
async fn gather_review(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    tx: &SafeTx,
    members: &ChainMembers,
    origin: Option<String>,
) -> ReviewInput {
    let subject = multisig_gather::ReviewSubject {
        chain_id,
        safe: account.address.clone(),
        tx: tx.clone(),
        members: members.clone(),
        genesis: Some(account.genesis.clone()),
        known: lock(st).known.clone(),
        member_names: account
            .members
            .as_ref()
            .map(|ms| {
                ms.iter()
                    .filter_map(|(a, i)| Some((a.to_ascii_lowercase(), i.name.clone()?)))
                    .collect()
            })
            .unwrap_or_default(),
        origin: origin.clone(),
    };
    let input = multisig_gather::gather_review(io, &subject).await;
    let key = subject_key(chain_id, &account.address, tx, members, origin.as_deref());
    let now = io.now().await;
    let mut s = lock(st);
    s.recent.retain(|(k, _, _)| *k != key);
    s.recent.push((key, now, input.clone()));
    let skip = s.recent.len().saturating_sub(RECENT_CAP);
    s.recent.drain(..skip);
    input
}

// ---------------------------------------------------------------------------
// The signing gate (`authorizeSigning`)
// ---------------------------------------------------------------------------

/// Why nothing was signed.
enum Stop {
    State(ActionState),
}

/// Before any signature for `account` on `chain_id` over `tx`: read the
/// chain now (who can act for it; its members; the state diff), then the
/// core's gate. `Ok` when signing may go on.
async fn authorize_signing(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    tx: &SafeTx,
) -> Result<(), Stop> {
    let chain = lock(st).chain.clone();
    let found = async { shell::judge(io, chain_id, &account.address, &account.genesis).await };
    let rule = async {
        let members = chain
            .read_members(io, chain_id, &account.address, &account.genesis)
            .await
            .ok()?;
        let key = subject_key(chain_id, &account.address, tx, &members, None);
        let now = io.now().await;
        let recent = lock(st)
            .recent
            .iter()
            .find(|(k, at, _)| *k == key && now - at <= REVIEW_FRESH_MS)
            .map(|(_, _, i)| i.clone());
        let input = match recent {
            Some(i) => i,
            None => gather_review(io, st, account, chain_id, tx, &members, None).await,
        };
        Some(multisig_gather::review_of(&input))
    };
    let (found, review) = futures::join!(found, rule);
    let (Some(found), Some(review)) = (found, review) else {
        return Err(Stop::State(ActionState::SigningUnchecked));
    };
    io.notice(Notice::Account {
        address: account.address.clone(),
        update: AccountUpdate::FindingsOn {
            chain_id,
            findings: found.clone(),
        },
    })
    .await;
    match multisig_gather::decide(chain_id, &account.address, tx, Some(found), review.as_ref()) {
        multisig_review::Gate::Open => Ok(()),
        multisig_review::Gate::Unchecked => Err(Stop::State(ActionState::SigningUnchecked)),
        multisig_review::Gate::Refused { findings } => {
            Err(Stop::State(ActionState::SigningRefused { findings }))
        }
        multisig_review::Gate::Blocked => Err(Stop::State(ActionState::SigningBlocked)),
        multisig_review::Gate::Risk { findings } => match io
            .ask(MultisigOperation::AskRisk {
                chain_id,
                safe: account.address.clone(),
                findings,
            })
            .await
        {
            MultisigShellResult::RiskAnswered { signs: true } => Ok(()),
            _ => Err(Stop::State(ActionState::RiskDeclined)),
        },
    }
}

/// `owner` approves `hash`: one passkey prompt, in Vela or on its page.
async fn sign_approval(
    io: &Io<Event>,
    owner: &LocalAccount,
    account: &MultisigAccount,
    chain_id: u64,
    hash: &str,
    tx: &SafeTx,
) -> Result<Approval, Stop> {
    if owner.route == ApprovalRoute::Blocked {
        return Err(Stop::State(ActionState::Failed {
            detail: "the signing venue blocks this member".to_owned(),
        }));
    }
    let challenge = multisig::approval_challenge(chain_id, &owner.address, hash).map_err(|e| {
        Stop::State(ActionState::Failed {
            detail: e.to_string(),
        })
    })?;
    let answer = io
        .ask(MultisigOperation::Passkey {
            owner: owner.address.clone(),
            chain_id,
            challenge,
            purpose: PasskeyPurpose::Approve {
                multisig: account.address.clone(),
                safe_tx_hash: hash.to_owned(),
                name: account
                    .genesis
                    .name
                    .clone()
                    .or_else(|| account.label.clone()),
                tx: Box::new(tx.clone()),
            },
        })
        .await;
    let approval = match answer {
        MultisigShellResult::Asserted {
            credential_id,
            authenticator_data,
            client_data_json,
            signature,
        } => multisig::approval_from_assertion(
            &owner.wallet_keys(),
            &credential_id,
            &authenticator_data,
            &client_data_json,
            &signature,
        )
        .map(|a| Approval::Passkey(Box::new(a)))
        .map_err(|e| {
            Stop::State(ActionState::Failed {
                detail: format!("the passkey's signature did not check out: {e}"),
            })
        })?,
        MultisigShellResult::PageApproved { approval } => {
            serde_json::from_value::<Approval>(approval).map_err(|e| {
                Stop::State(ActionState::Failed {
                    detail: e.to_string(),
                })
            })?
        }
        MultisigShellResult::PasskeyCancelled => return Err(Stop::State(ActionState::Cancelled)),
        MultisigShellResult::PasskeyFailed { detail } => {
            return Err(Stop::State(ActionState::Failed { detail }))
        }
        other => {
            return Err(Stop::State(ActionState::Failed {
                detail: format!("unexpected answer {other:?}"),
            }))
        }
    };
    // Whatever route it came by, it is this member's approval of this hash.
    let verified = match &approval {
        Approval::Passkey(k) => multisig::verify_approval(chain_id, hash, k).is_ok(),
        _ => false,
    };
    if !same_address(approval.owner(), &owner.address) || !verified {
        return Err(Stop::State(ActionState::Failed {
            detail: "the approval does not check out as this member's".to_owned(),
        }));
    }
    Ok(approval)
}

// ---------------------------------------------------------------------------
// The outbox
// ---------------------------------------------------------------------------

enum Sent {
    Took,
    Refused(Value),
    /// Kept, sent again on its own.
    Kept,
}

/// Hand one signed approval to the mailbox, kept until it answers.
async fn send_kept(io: &Io<Event>, st: &Shared<State>, req: Submission) -> Sent {
    let key = outbox_key(&req);
    let outbox = {
        let mut s = lock(st);
        s.outbox.insert(key.clone(), req.clone());
        s.outbox.clone()
    };
    save_json(io, keys::OUTBOX, &outbox, true).await;
    let first = attempt(io, st, &key, &req).await;
    if matches!(first, Sent::Kept) {
        let (k, r) = (key.clone(), req.clone());
        let state = st.clone();
        io.spawn(move |io| async move { resend_loop(&io, &state, &k, &r).await });
    }
    first
}

async fn drop_kept(io: &Io<Event>, st: &Shared<State>, key: &str) {
    let outbox = {
        let mut s = lock(st);
        s.outbox.remove(key);
        s.outbox_entries.remove(key);
        s.outbox_since.remove(key);
        s.outbox_tries.remove(key);
        s.outbox.clone()
    };
    save_json(io, keys::OUTBOX, &outbox, true).await;
}

async fn attempt(io: &Io<Event>, st: &Shared<State>, key: &str, req: &Submission) -> Sent {
    let service = lock(st).service.clone();
    match service.submit_approval(io, req).await {
        Ok(SubmitAnswer::Took { .. }) => {
            drop_kept(io, st, key).await;
            io.notice(Notice::MailboxTook {
                safe: req.safe.clone(),
            })
            .await;
            Sent::Took
        }
        Ok(SubmitAnswer::Refused { refusal }) => {
            drop_kept(io, st, key).await;
            Sent::Refused(refusal)
        }
        Err(fail) => {
            let now = io.now().await;
            let mut s = lock(st);
            let since = *s.outbox_since.entry(key.to_owned()).or_insert(now);
            let tries = {
                let t = s.outbox_tries.entry(key.to_owned()).or_insert(0);
                *t += 1;
                *t
            };
            let backoff = OUTBOX_BACKOFF_MAX_MS
                .min(2_000.0 * 2f64.powi(i32::try_from(tries).unwrap_or(30) - 1));
            let (why, wait) = match fail {
                ServiceFail::Busy(w) => ("busy", w.unwrap_or(backoff).max(1_000.0)),
                ServiceFail::Unavailable(_) => ("away", backoff.max(1_000.0)),
            };
            let retry_at = (now + wait <= since + OUTBOX_PATIENCE_MS).then_some(now + wait);
            s.outbox_entries.insert(
                key.to_owned(),
                OutboxEntry {
                    why: why.to_owned(),
                    retry_at,
                },
            );
            Sent::Kept
        }
    }
}

async fn resend_loop(io: &Io<Event>, st: &Shared<State>, key: &str, req: &Submission) {
    loop {
        io.emit(Event::Changed);
        let (retry_at, still) = {
            let s = lock(st);
            (
                s.outbox_entries.get(key).and_then(|e| e.retry_at),
                s.outbox.contains_key(key),
            )
        };
        let (Some(at), true) = (retry_at, still) else {
            return;
        };
        let now = io.now().await;
        if at > now {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            io.wait((at - now) as u64).await;
        }
        if !lock(st).outbox.contains_key(key) {
            return;
        }
        if !matches!(attempt(io, st, key, req).await, Sent::Kept) {
            io.emit(Event::Changed);
            return;
        }
    }
}

/// Send again what is kept (with a fresh patience): a page coming up, or Send again.
async fn flush_outbox(io: &Io<Event>, st: &Shared<State>, prefix: Option<String>) {
    let kept: Vec<(String, Submission)> = lock(st)
        .outbox
        .iter()
        .filter(|(k, _)| {
            prefix
                .as_ref()
                .is_none_or(|p| k.to_ascii_lowercase().starts_with(p))
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (key, req) in kept {
        {
            let mut s = lock(st);
            s.outbox_since.remove(&key);
            s.outbox_tries.remove(&key);
        }
        if matches!(attempt(io, st, &key, &req).await, Sent::Kept) {
            let state = st.clone();
            io.spawn(move |io| async move { resend_loop(&io, &state, &key, &req).await });
        }
    }
}

async fn submit(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    tx: &SafeTx,
    approval: Approval,
) -> ActionState {
    let hash = multisig::safe_tx_hash(chain_id, &account.address, tx).unwrap_or_default();
    let req = Submission {
        chain_id,
        safe: account.address.clone(),
        safe_tx: tx.clone(),
        approval,
        genesis: multisig::named_genesis(&account.genesis).ok(),
    };
    let sent = send_kept(io, st, req).await;
    match sent {
        Sent::Refused(refusal) => ActionState::MailboxRefused { refusal },
        Sent::Took | Sent::Kept => {
            if !account.chain_ids.contains(&chain_id) {
                io.notice(Notice::Account {
                    address: account.address.clone(),
                    update: AccountUpdate::UseChain { chain_id },
                })
                .await;
            }
            ActionState::Done {
                safe_tx_hash: hash,
                kept: matches!(sent, Sent::Kept),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Proposing (`proposeBuilt`, `reviewFirst`)
// ---------------------------------------------------------------------------

fn hex_nonce(n: u128) -> String {
    format!("0x{n:x}")
}

fn build(kind: &ProposalKind, safe: &str, chain: &ChainState) -> Result<SafeTx, String> {
    let nonce = hex_nonce(next_nonce_of(chain));
    match kind {
        ProposalKind::Transfers { transfers } => {
            multisig::transfers_tx(transfers, &nonce).map_err(|e| e.to_string())
        }
        ProposalKind::Rename { name } => {
            multisig::rename_tx(name, &nonce).map_err(|e| e.to_string())
        }
        ProposalKind::Calls { calls, .. } => {
            multisig::calls_tx(calls, &nonce).map_err(|e| e.to_string())
        }
        ProposalKind::Cancel { nonce } => {
            multisig::cancel_tx(safe, nonce).map_err(|e| e.to_string())
        }
        ProposalKind::MemberChange { change } => {
            let state = multisig::MemberState {
                owners: chain.members.owners.clone(),
                threshold: chain.members.threshold,
                change_waiting: chain.items.iter().any(|i| {
                    i.status != ProposalStatus::Done
                        && matches!(
                            i.review.action,
                            Some(
                                Action::AddOwner { .. }
                                    | Action::RemoveOwner { .. }
                                    | Action::SwapOwner { .. }
                                    | Action::ChangeThreshold { .. }
                            )
                        )
                }),
            };
            match multisig::member_change_tx(safe, &state, change, &nonce) {
                Ok(multisig::MemberVerdict::Ready { tx, .. }) => Ok(*tx),
                Ok(multisig::MemberVerdict::Refused { refusal }) => {
                    Err(serde_json::to_string(&refusal).unwrap_or_default())
                }
                Err(e) => Err(e.to_string()),
            }
        }
    }
}

async fn draft(io: &Io<Event>, st: &Shared<State>, chain_id: u64, kind: ProposalKind) {
    let (account, round) = {
        let s = lock(st);
        (s.account.clone(), s.draft_round)
    };
    let set = |st: &Shared<State>, state: ActionState| {
        let mut s = lock(st);
        if s.draft_round == round {
            s.actions.insert("propose".to_owned(), state);
        }
    };
    let Some(account) = account else {
        return set(
            st,
            ActionState::Failed {
                detail: "no multisig shown".to_owned(),
            },
        );
    };
    // A delegatecall into nothing spends the nonce doing nothing: the batch
    // contract must be on this chain.
    let read = read_chain(io, st, &account, chain_id, None, false, true, None, None).await;
    let Ok(read) = read else {
        return set(st, ActionState::MailboxUnreadable);
    };
    if !read.state.mailbox {
        return set(st, ActionState::MailboxUnreadable);
    }
    let tx = match build(&kind, &account.address, &read.state) {
        Ok(tx) => tx,
        Err(reason) => return set(st, ActionState::Refused { reason }),
    };
    let review = match multisig::review(chain_id, &account.address, &tx) {
        Ok(r) => r,
        Err(e) => {
            return set(
                st,
                ActionState::Refused {
                    reason: e.to_string(),
                },
            )
        }
    };
    if review.action.is_none() {
        return set(
            st,
            ActionState::Refused {
                reason: review.refusal.unwrap_or_else(|| "refused".to_owned()),
            },
        );
    }
    if tx.operation == 1 && !Chain::has_code(io, chain_id, &tx.to).await.unwrap_or(false) {
        return set(
            st,
            ActionState::Refused {
                reason: format!("{} has no code on chain {chain_id}", tx.to),
            },
        );
    }
    let now = io.now().await;
    let hash = multisig::safe_tx_hash(chain_id, &account.address, &tx).unwrap_or_default();
    let locals = lock(st).locals.clone();
    let Some(item) = read_proposal(
        &account.address,
        &read.state.members,
        &Proposal {
            chain_id,
            safe: account.address.clone(),
            safe_tx_hash: hash,
            safe_tx: tx.clone(),
            proposer: String::new(),
            approvals: Vec::new(),
            created_ms: now,
            updated_ms: now,
        },
        &[],
        &locals,
        false,
    ) else {
        return set(
            st,
            ActionState::Failed {
                detail: "the transaction does not hash as itself".to_owned(),
            },
        );
    };
    let origin = match &kind {
        ProposalKind::Calls { origin, .. } => origin.clone(),
        _ => None,
    };
    {
        let mut s = lock(st);
        if s.draft_round != round {
            return;
        }
        s.draft = Some(Draft {
            chain: read.state.clone(),
            tx: tx.clone(),
            item,
            review: None,
        });
        s.actions.remove("propose");
    }
    io.emit(Event::Changed);
    let input = gather_review(io, st, &account, chain_id, &tx, &read.state.members, origin).await;
    let out = multisig_review::multisig_review(&input)
        .ok()
        .and_then(|r| serde_json::to_value(r).ok());
    let mut s = lock(st);
    if s.draft_round == round {
        if let Some(d) = s.draft.as_mut() {
            d.review = out;
        }
    }
}

async fn confirm_draft(
    io: &Io<Event>,
    st: &Shared<State>,
    as_member: Option<String>,
    round: u64,
) -> ActionState {
    let (account, chain, tx, locals, last) = {
        let s = lock(st);
        let Some(d) = s.draft.as_ref() else {
            return ActionState::Failed {
                detail: "no draft".to_owned(),
            };
        };
        let Some(account) = s.account.clone() else {
            return ActionState::Failed {
                detail: "no multisig shown".to_owned(),
            };
        };
        let last = s
            .last_actor
            .get(&account.address.to_ascii_lowercase())
            .cloned();
        (
            account,
            d.chain.clone(),
            d.tx.clone(),
            s.locals.clone(),
            last,
        )
    };
    let chain_id = chain.chain_id;
    let here = local_owners(&locals, &chain.members.owners);
    let picked = as_member
        .and_then(|p| here.iter().find(|o| same_address(&o.address, &p)).cloned())
        .or_else(|| last.and_then(|l| here.iter().find(|o| same_address(&o.address, &l)).cloned()))
        .or_else(|| here.first().cloned());
    let Some(owner) = picked else {
        // No member here: an outside member's wallet proposes it.
        return flows::propose_outside(io, st, &account, &chain, &tx, round).await;
    };
    if here.len() > 1 {
        let map = {
            let mut s = lock(st);
            s.last_actor.insert(
                account.address.to_ascii_lowercase(),
                owner.address.to_ascii_lowercase(),
            );
            s.last_actor.clone()
        };
        save_json(io, keys::LAST_ACTOR, &map, false).await;
    }
    if let Err(Stop::State(s)) = authorize_signing(io, st, &account, chain_id, &tx).await {
        return s;
    }
    if lock(st).draft_round != round {
        return ActionState::Cancelled;
    }
    let hash = multisig::safe_tx_hash(chain_id, &account.address, &tx).unwrap_or_default();
    let approval = match sign_approval(io, &owner, &account, chain_id, &hash, &tx).await {
        Ok(a) => a,
        Err(Stop::State(s)) => return s,
    };
    // Withdrawn while the passkey was up: what it signed goes nowhere.
    if lock(st).draft_round != round {
        return ActionState::Cancelled;
    }
    let outcome = submit(io, st, &account, chain_id, &tx, approval).await;
    refresh(io, st, false, true).await;
    outcome
}

async fn approve(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    owner: &str,
) -> ActionState {
    let (found, account, owner) = {
        let s = lock(st);
        (
            s.item(chain_id, hash),
            s.account.clone(),
            s.locals
                .iter()
                .find(|a| same_address(&a.address, owner))
                .cloned(),
        )
    };
    let (Some((item, _)), Some(account), Some(owner)) = (found, account, owner) else {
        return ActionState::Failed {
            detail: "nothing to approve".to_owned(),
        };
    };
    if let Err(Stop::State(s)) = authorize_signing(io, st, &account, chain_id, &item.tx).await {
        return s;
    }
    approve_authorized(io, st, &account, &item, &owner).await
}

async fn approve_authorized(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    item: &ProposalItem,
    owner: &LocalAccount,
) -> ActionState {
    let approval = match sign_approval(
        io,
        owner,
        account,
        item.chain_id,
        &item.safe_tx_hash,
        &item.tx,
    )
    .await
    {
        Ok(a) => a,
        Err(Stop::State(s)) => return s,
    };
    submit(io, st, account, item.chain_id, &item.tx, approval).await
}

// ---------------------------------------------------------------------------
// Execution (`execute.ts`)
// ---------------------------------------------------------------------------

/// What `executor` executing an item is (`ExecutionInput`).
#[derive(Clone, Debug)]
pub struct ExecutionInput {
    pub chain_id: u64,
    pub account: MultisigAccount,
    pub members: ChainMembers,
    pub item: ProposalItem,
    pub approvals: Vec<Approval>,
    pub executor: LocalAccount,
}

fn execution_input(
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    executor: &str,
) -> Option<ExecutionInput> {
    let s = lock(st);
    let (item, members) = s.item(chain_id, hash)?;
    let executor = s
        .locals
        .iter()
        .find(|a| same_address(&a.address, executor))?
        .clone();
    Some(ExecutionInput {
        chain_id,
        account: s.account.clone()?,
        approvals: item
            .approvals
            .iter()
            .filter(|a| a.counts)
            .map(|a| a.approval.clone())
            .collect(),
        members,
        item,
        executor,
    })
}

/// The calls executing it takes (`planExecution`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Planned {
    pub calls: Vec<Call>,
    pub deploys_multisig: bool,
    /// Approving members whose Vela account this execution deploys (their
    /// passkey approval is checked through it).
    pub activates: Vec<String>,
    /// The members whose given approvals it carries (the executor's own is
    /// never one).
    pub carries: Vec<String>,
    /// The executor approves by sending it (`msg.sender`), one approval
    /// besides `carries`.
    pub executor_approves: bool,
}

/// Which approvals an execution carries (`carriedApprovals`, r4-7, r7-3):
/// only the approvals the threshold needs — those needing nothing deployed
/// first (`ready`: a plain account's, or a Vela account with code there) —
/// and the executor (an owner) approves as `msg.sender` only when those that
/// need nothing deployed fall short, never in place of one given. `others`:
/// the approvals besides the executor's own, each with whether it is ready.
/// Returns the chosen approvals (with their readiness) and whether the
/// executor approves.
pub fn carried_approvals<T: Clone>(
    threshold: u64,
    executor_is_owner: bool,
    others: Vec<(T, bool)>,
) -> (Vec<(T, bool)>, bool) {
    let ready_count = others.iter().filter(|(_, r)| *r).count() as u64;
    let executor_approves = executor_is_owner && ready_count < threshold;
    let needed = threshold.saturating_sub(u64::from(executor_approves));
    let mut chosen = others;
    // Ready first, the given order kept within each (a stable sort).
    chosen.sort_by_key(|(_, r)| !*r);
    chosen.truncate(usize::try_from(needed).unwrap_or(usize::MAX));
    (chosen, executor_approves)
}

/// About how much of `fee` is the one-time setup (`look`'s `setup.amount`):
/// each deployment call's `eth_estimateGas` (`gas`), less the 21,000 each
/// carries alone (inside the one operation they do not), as a share of the
/// quote's `total_gas`, never more than the fee. `None` (unmeasured) with no
/// deployment, a quote without its gas, an executor whose own first
/// deployment rides in its initCode (unmeasured here), or a gas read that
/// did not answer.
pub fn setup_share(
    fee: u128,
    total_gas: u128,
    executor_deployed: bool,
    gas: &[Option<u128>],
) -> Option<u128> {
    use alloy_primitives::U256;
    if gas.is_empty() || total_gas == 0 || !executor_deployed {
        return None;
    }
    let mut setup_gas = U256::ZERO;
    for g in gas {
        setup_gas += U256::from(g.as_ref()?.saturating_sub(21_000));
    }
    let share = U256::from(fee) * setup_gas / U256::from(total_gas);
    Some(u128::try_from(share).unwrap_or(u128::MAX).min(fee))
}

enum PlanFail {
    Refused(Value),
    Other,
}

async fn plan_execution(io: &Io<Event>, input: &ExecutionInput) -> Result<Planned, PlanFail> {
    let owner = input
        .members
        .owners
        .iter()
        .any(|o| same_address(o, &input.executor.address));
    let others: Vec<Approval> = input
        .approvals
        .iter()
        .filter(|a| !owner || !same_address(a.owner(), &input.executor.address))
        .cloned()
        .collect();
    let ready = future::join_all(others.iter().map(|a| async move {
        match a {
            Approval::Passkey(_) => Chain::has_code(io, input.chain_id, a.owner())
                .await
                .unwrap_or(false),
            _ => true,
        }
    }))
    .await;
    let (chosen, executor_approves) = carried_approvals(
        input.members.threshold,
        owner,
        others.into_iter().zip(ready).collect(),
    );
    let mut deploy_owners = Vec::new();
    let mut activates = Vec::new();
    for (a, ready) in &chosen {
        if let (false, Approval::Passkey(p)) = (ready, a) {
            deploy_owners.push(p.owner_genesis.clone());
            activates.push(p.owner.clone());
        }
    }
    let deploy_multisig = (!input.members.deployed).then(|| input.account.genesis.clone());
    let deploy_facts = if deploy_multisig.is_some() {
        Some(verified_deploy_facts(io, input).await?)
    } else {
        None
    };
    let calls = multisig::execution_calls(&multisig::ExecutionPlan {
        multisig: input.account.address.clone(),
        deploy_multisig: deploy_multisig.clone(),
        deploy_owners,
        tx: input.item.tx.clone(),
        approvals: chosen.iter().map(|(a, _)| a.clone()).collect(),
        executor: executor_approves.then(|| input.executor.address.clone()),
        deploy_facts,
    })
    .map_err(|_| PlanFail::Other)?;
    Ok(Planned {
        calls,
        deploys_multisig: deploy_multisig.is_some(),
        activates,
        carries: chosen.iter().map(|(a, _)| a.owner().to_owned()).collect(),
        executor_approves,
    })
}

/// The deploy gate: read the code now, never kept; Verified or no deployment.
async fn verified_deploy_facts(
    io: &Io<Event>,
    input: &ExecutionInput,
) -> Result<multisig::ChainFacts, PlanFail> {
    let facts = shell::counterfactual_facts(
        io,
        input.chain_id,
        &input.account.address,
        &input.account.genesis,
    )
    .await;
    match multisig::counterfactual_authority(&input.account.genesis, &input.account.address, &facts)
    {
        Ok(multisig::Authority::Verified { .. }) => Ok(facts),
        Ok(other) => Err(PlanFail::Refused(
            serde_json::to_value(other).unwrap_or(Value::Null),
        )),
        Err(_) => Err(PlanFail::Other),
    }
}

/// What the preview says (`ExecutionPreview`).
#[derive(Clone, Debug, PartialEq)]
pub enum Preview {
    Refused(Value),
    Fails(Value),
    Unpriced,
    Unchecked,
    Spent,
    Priced {
        planned: Planned,
        fee: Value,
        balance: Option<u128>,
        can_pay: bool,
        executor_deployed: bool,
        /// About how much of the fee is the one-time setup ([`setup_share`]).
        setup_amount: Option<u128>,
    },
}

impl Preview {
    pub fn to_value(&self) -> Value {
        match self {
            Self::Refused(a) => json!({"kind": "refused", "authority": a}),
            Self::Fails(why) => json!({"kind": "fails", "why": why}),
            Self::Unpriced => json!({"kind": "unpriced"}),
            Self::Unchecked => json!({"kind": "unchecked"}),
            Self::Spent => json!({"kind": "spent"}),
            Self::Priced {
                planned,
                fee,
                balance,
                can_pay,
                executor_deployed,
                setup_amount,
            } => json!({
                "kind": "priced",
                "calls": planned.calls,
                "fee": fee,
                "balance": balance.map(|b| b.to_string()),
                "canPay": can_pay,
                "carries": planned.carries,
                "executorApproves": planned.executor_approves,
                "setup": {
                    "multisig": planned.deploys_multisig,
                    "members": planned.activates,
                    "executor": !executor_deployed,
                    // Decimal, as `balance`; `null` when unmeasured.
                    "amount": setup_amount.map(|a| a.to_string()),
                },
            }),
        }
    }
}

/// What the action moves out of the multisig (`spendingOf`).
pub fn spending_of(action: Option<&Action>) -> Vec<(Option<String>, u128)> {
    let mut out: Vec<(Option<String>, u128)> = Vec::new();
    let mut add = |token: Option<String>, amount: u128| {
        if amount == 0 {
            return;
        }
        let hit = out.iter_mut().find(|(t, _)| match (t, &token) {
            (None, None) => true,
            (Some(a), Some(b)) => same_address(a, b),
            _ => false,
        });
        match hit {
            Some((_, a)) => *a += amount,
            None => out.push((token, amount)),
        }
    };
    let value = |v: &str| shell::quantity(if v.is_empty() { "0" } else { v }).unwrap_or(0);
    match action {
        Some(Action::Transfer { token, amount, .. }) => add(token.clone(), value(amount)),
        Some(Action::Call { value: v, .. }) => add(None, value(v)),
        Some(Action::Batch { calls }) => {
            for c in calls {
                match &c.action {
                    Action::Transfer { token, amount, .. } => add(token.clone(), value(amount)),
                    _ => add(None, value(&c.value)),
                }
            }
        }
        _ => {}
    }
    out
}

/// A revert's `Error(string)` text (`revertText`).
pub fn revert_text(data: &str) -> Option<String> {
    let lower = data.to_ascii_lowercase();
    let body = lower.strip_prefix("0x08c379a0")?;
    let length = usize::from_str_radix(body.get(64..128)?, 16).ok()?;
    let hex = body.get(128..128 + length * 2)?;
    let bytes = crate::primitives::from_hex(hex).ok()?;
    let text = String::from_utf8_lossy(&bytes).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// A node's wording kept only when it says something (`plainReason`).
pub fn plain_reason(reason: Option<&str>) -> Option<String> {
    let r = reason?.trim();
    let lower = r.to_ascii_lowercase();
    if lower.contains("target call")
        || lower.contains("executeuserop")
        || lower.ends_with("execution reverted")
        || lower == "reverted"
    {
        return None;
    }
    (!r.is_empty()).then(|| r.to_owned())
}

async fn balance_of(
    io: &Io<Event>,
    chain_id: u64,
    token: Option<&str>,
    holder: &str,
) -> Option<u128> {
    let read = match token {
        None => io
            .rpc_within(chain_id, "eth_getBalance", json!([holder, "latest"]), Some(REVIEW_READ_MS))
            .await,
        Some(t) => io
            .rpc_within(
                chain_id,
                "eth_call",
                json!([{ "to": t, "data": format!("0x70a08231{:0>64}", holder.trim_start_matches("0x").to_ascii_lowercase()) }, "latest"]),
                Some(REVIEW_READ_MS),
            )
            .await,
    };
    let raw = read.ok()?;
    let raw = raw.as_str()?;
    shell::quantity(if raw.len() > 66 { &raw[..66] } else { raw })
}

async fn call_refusal(io: &Io<Event>, input: &ExecutionInput) -> Option<Value> {
    let tx = &input.item.tx;
    if tx.operation != 0 {
        return None;
    }
    let data = if tx.data.is_empty() { "0x" } else { &tx.data };
    match io
        .rpc_within(
            input.chain_id,
            "eth_call",
            json!([{ "from": input.account.address, "to": tx.to, "value": tx.value, "data": data }, "latest"]),
            Some(REVIEW_READ_MS),
        )
        .await
    {
        Err(shell::RpcFail::Error { code, message, .. }) => {
            if code != Some(3) && !message.to_ascii_lowercase().contains("revert") {
                return None;
            }
            let stripped = message
                .trim_start_matches("execution reverted")
                .trim_start_matches(':')
                .trim();
            let reason = revert_text(&message).or_else(|| plain_reason((!stripped.is_empty()).then_some(stripped)));
            Some(json!({"kind": "reverts", "reason": reason}))
        }
        _ => None,
    }
}

async fn preview_execution(io: &Io<Event>, st: &Shared<State>, input: &ExecutionInput) -> Preview {
    let preview = look(io, input).await;
    if !matches!(preview, Preview::Fails(_)) {
        return preview;
    }
    let chain = lock(st).chain.clone();
    let nonce = chain
        .read_nonce(io, input.chain_id, &input.account.address)
        .await
        .ok();
    if nonce.is_some_and(|n| n > input.item.nonce) {
        Preview::Spent
    } else {
        preview
    }
}

async fn look(io: &Io<Event>, input: &ExecutionInput) -> Preview {
    for (token, need) in spending_of(input.item.review.action.as_ref()) {
        let have = balance_of(io, input.chain_id, token.as_deref(), &input.account.address).await;
        if let Some(have) = have {
            if have < need {
                return Preview::Fails(
                    json!({"kind": "short", "token": token, "have": have.to_string(), "need": need.to_string()}),
                );
            }
        }
    }
    if let Some(refusal) = call_refusal(io, input).await {
        return Preview::Fails(refusal);
    }
    let planned = match plan_execution(io, input).await {
        Ok(p) => p,
        Err(PlanFail::Refused(a)) => return Preview::Refused(a),
        Err(PlanFail::Other) => return Preview::Unpriced,
    };
    let executor = input.executor.address.clone();
    let quote = async {
        io.ask(MultisigOperation::QuoteCalls {
            chain_id: input.chain_id,
            from: executor.clone(),
            calls: planned.calls.clone(),
        })
        .await
    };
    let (quote, balance) = futures::join!(quote, balance_of(io, input.chain_id, None, &executor));
    let MultisigShellResult::Quoted { quote } = quote else {
        return Preview::Unpriced;
    };
    match quote.get("kind").and_then(Value::as_str) {
        Some("reverts") => {
            return Preview::Fails(json!({
                "kind": "reverts",
                "reason": plain_reason(quote.get("reason").and_then(Value::as_str)),
            }))
        }
        Some("failed") => return Preview::Unpriced,
        _ => {}
    }
    let fee = quote.get("fee").cloned().unwrap_or(Value::Null);
    let amount = fee.get("amount").and_then(|a| {
        a.as_str()
            .and_then(shell::quantity)
            .or_else(|| a.as_u64().map(u128::from))
    });
    let is_native = fee.get("token").is_none_or(Value::is_null);
    let can_pay = match (balance, amount) {
        (Some(b), Some(a)) if is_native => b >= a,
        _ => true,
    };
    let executor_deployed = quote
        .get("executorDeployed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let total_gas = quote
        .get("totalGas")
        .and_then(|g| {
            g.as_str()
                .and_then(shell::quantity)
                .or_else(|| g.as_u64().map(u128::from))
        })
        .unwrap_or(0);
    // The deployments: every call before execTransaction, each measured on
    // its own — only when a share could be said at all.
    let deployments = &planned.calls[..planned.calls.len().saturating_sub(1)];
    let setup_amount = match amount {
        Some(fee) if !deployments.is_empty() && total_gas > 0 && executor_deployed => {
            let gas = future::join_all(
                deployments
                    .iter()
                    .map(|c| gas_of(io, input.chain_id, &executor, c)),
            )
            .await;
            setup_share(fee, total_gas, executor_deployed, &gas)
        }
        _ => None,
    };
    Preview::Priced {
        planned,
        fee,
        balance,
        can_pay,
        executor_deployed,
        setup_amount,
    }
}

/// One call's gas on its own (`readGas`): `eth_estimateGas` from `from`,
/// bounded; `None` when unmeasured.
async fn gas_of(io: &Io<Event>, chain_id: u64, from: &str, call: &Call) -> Option<u128> {
    let answer = io
        .rpc_within(
            chain_id,
            "eth_estimateGas",
            json!([{ "from": from, "to": call.to, "value": call.value, "data": call.data }]),
            Some(REVIEW_READ_MS),
        )
        .await
        .ok()?;
    let raw = answer.as_str()?;
    if !raw.starts_with("0x") || raw.len() < 3 {
        return None;
    }
    shell::quantity(raw)
}

/// Propose the draft (`ConfirmDraft`) — and, `then_execute`, execute it
/// once the proposer's own approval made it ready (`ProposeExecute`).
fn propose_draft(model: &Model, as_member: Option<String>, then_execute: bool) -> Cmd {
    let (round, chain_id) = {
        let mut s = lock(&model.state);
        s.actions.insert("propose".to_owned(), ActionState::Working);
        (s.draft_round, s.draft.as_ref().map(|d| d.chain.chain_id))
    };
    flow(model, move |io, st| async move {
        let claimed = flows::claim(&io, &st, round).await;
        let outcome = confirm_draft(&io, &st, as_member, round).await;
        // `execute:<hash>` is working from the moment it is proposed (a
        // sheet waits on it); it goes again when the execution is not for now.
        let executing = match (&outcome, then_execute, chain_id) {
            (ActionState::Done { safe_tx_hash, .. }, true, Some(chain_id)) => {
                Some((chain_id, safe_tx_hash.clone()))
            }
            _ => None,
        };
        {
            let mut s = lock(&st);
            if s.draft_round == round {
                if matches!(outcome, ActionState::Done { .. }) {
                    s.draft = None;
                }
                s.actions.insert("propose".to_owned(), outcome.clone());
            }
            if let Some((_, hash)) = &executing {
                s.actions.insert(
                    format!("execute:{}", hash.to_ascii_lowercase()),
                    ActionState::Working,
                );
            }
        }
        flows::on_proposed(&io, &st, round, &outcome, claimed).await;
        let Some((chain_id, hash)) = executing else {
            return;
        };
        // The proposal as the mailbox has it now: executed, by the member
        // who proposed it, if their approval made it ready.
        refresh(&io, &st, false, true).await;
        let proposer = lock(&st).item(chain_id, &hash).and_then(|(i, _)| {
            (i.status == ProposalStatus::Ready
                && i.can_execute.iter().any(|e| same_address(e, &i.proposer)))
            .then(|| i.proposer.clone())
        });
        match proposer {
            Some(proposer) => approve_execute(&io, &st, chain_id, &hash, &proposer).await,
            None => {
                lock(&st)
                    .actions
                    .remove(&format!("execute:{}", hash.to_ascii_lowercase()));
            }
        }
    })
}

/// A stop before any signature (r4-11): the look could not price or check
/// it, it would fail, or the executor cannot pay — not a number used
/// meanwhile, nor who controls it refused.
fn stopped_before_signing(outcome: &ActionState) -> bool {
    match outcome {
        ActionState::Stopped { preview } => matches!(
            preview.get("kind").and_then(Value::as_str),
            Some("fails" | "unpriced" | "unchecked" | "priced")
        ),
        _ => false,
    }
}

/// "Approve and execute" in one press (`ApproveExecute`): the execution,
/// counting the executor's approval; stopped before its signature, the
/// approval alone, kept.
async fn approve_execute(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    executor: &str,
) {
    let lower = hash.to_ascii_lowercase();
    let input = execution_input(st, chain_id, hash, executor);
    let approving = input.as_ref().is_some_and(|i| {
        i.item.approved < i.item.threshold
            && i.item.can_approve.iter().any(|o| same_address(o, executor))
    });
    let outcome = execute(io, st, chain_id, hash, executor).await;
    flows::remember_execution(io, st, input.as_ref(), &outcome).await;
    if matches!(outcome, ActionState::Done { .. }) {
        note_actor(io, st, chain_id, executor).await;
    }
    let approve_too = approving && stopped_before_signing(&outcome);
    {
        let mut s = lock(st);
        s.actions.insert(format!("execute:{lower}"), outcome);
        if approve_too {
            s.actions
                .insert(format!("approve:{lower}"), ActionState::Working);
        }
    }
    if approve_too {
        io.emit(Event::Changed);
        let approved = approve(io, st, chain_id, hash, executor).await;
        if matches!(approved, ActionState::Done { .. }) {
            note_actor(io, st, chain_id, executor).await;
        }
        lock(st)
            .actions
            .insert(format!("approve:{lower}"), approved);
    }
    refresh(io, st, false, true).await;
}

/// core-ms-2: the members here whose one press approves and executes `item`
/// — short of one approval, theirs reaches the threshold, and their look at
/// these approvals says they can pay. Round 5 (S2): a look that could not
/// price or check it in time (`unpriced`, `unchecked`) does not say they
/// cannot — the press is offered, and the execution looks again before any
/// signature (stopped then, the approval is still given). A look still out,
/// or one that says it would fail, cannot be paid, is refused or its number
/// is used, offers nothing.
fn approve_executors(s: &State, item: &ProposalItem) -> Vec<String> {
    if item.status != ProposalStatus::Pending {
        return Vec::new();
    }
    item.can_execute
        .iter()
        .filter(|e| item.can_approve.iter().any(|o| same_address(o, e)))
        .filter(|e| {
            let key = preview_key(item.chain_id, &item.safe_tx_hash, e);
            let fresh = s
                .more
                .looks
                .get(&key)
                .is_some_and(|(approved, _)| *approved == item.approved);
            fresh
                && s.preview_plans.get(&key).is_some_and(|(_, p)| {
                    matches!(
                        p,
                        Preview::Priced { can_pay: true, .. }
                            | Preview::Unpriced
                            | Preview::Unchecked
                    )
                })
        })
        .cloned()
        .collect()
}

async fn execute(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    executor: &str,
) -> ActionState {
    let Some(input) = execution_input(st, chain_id, hash, executor) else {
        return ActionState::Failed {
            detail: "nothing to execute".to_owned(),
        };
    };
    if input
        .item
        .can_execute
        .iter()
        .all(|e| !same_address(e, executor))
    {
        return ActionState::Failed {
            detail: "this account cannot execute it now".to_owned(),
        };
    }
    // A look that says it stops, stops before the gate asks anything. This
    // executor's own look when it priced it within PREVIEW_FRESH_MS (its
    // calls and fee are what is signed), else a new one, kept the same way —
    // so what it carried is what is remembered.
    let key = preview_key(chain_id, hash, &input.executor.address);
    let looked = io.now().await;
    let shown = lock(st)
        .preview_plans
        .get(&key)
        // Only a look that lets it run is signed from: one that says the executor
        // cannot pay is asked again first — the funds may have arrived since.
        .filter(|(at, p)| {
            matches!(p, Preview::Priced { can_pay: true, .. }) && looked - at < PREVIEW_FRESH_MS
        })
        .map(|(_, p)| p.clone());
    let preview = match shown {
        Some(p) => p,
        None => look_now(io, st, &input).await,
    };
    if preview == Preview::Spent {
        // r5reg-21: its number was used meanwhile — what ran is said.
        settle_spent(io, st, &input).await;
    }
    let Preview::Priced { planned, fee, .. } = &preview else {
        return ActionState::Stopped {
            preview: preview.to_value(),
        };
    };
    if let Preview::Priced {
        can_pay: false,
        balance: Some(_),
        ..
    } = &preview
    {
        return ActionState::Stopped {
            preview: preview.to_value(),
        };
    }
    if let Err(Stop::State(s)) =
        authorize_signing(io, st, &input.account, chain_id, &input.item.tx).await
    {
        return s;
    }
    // r7-3: one signature, the executor's own operation — on its page when
    // it signs on one (the shell hands the page the operation and
    // `SendPurpose::Execute`). It counts as its approval as `msg.sender`
    // where one is missing; no approval of its own is asked first, which
    // would be a second trip to the page for nothing the chain needs.
    // The deploy gate, right before the deployment's calls are sent.
    if !input.members.deployed {
        if let Err(PlanFail::Refused(a)) = verified_deploy_facts(io, &input).await {
            return ActionState::Stopped {
                preview: Preview::Refused(a).to_value(),
            };
        }
    }
    let now = io.now().await;
    let sent = io
        .ask(MultisigOperation::SendCalls {
            chain_id,
            from: input.executor.address.clone(),
            calls: planned.calls.clone(),
            fee: Some(fee.clone()),
            purpose: Some(SendPurpose::Execute {
                multisig: input.account.address.clone(),
                name: input.account.genesis.name.clone(),
                threshold: input.members.threshold,
                tx: Box::new(input.item.tx.clone()),
            }),
        })
        .await;
    let (user_op_hash, maybe_sent, own) = match sent {
        MultisigShellResult::CallsSent {
            user_op_hash,
            tx_hash,
        } => (user_op_hash, false, tx_hash),
        MultisigShellResult::CallsFailed {
            maybe_sent: true,
            detail,
        } => (detail, true, None),
        MultisigShellResult::CallsFailed { detail, .. } => return ActionState::Failed { detail },
        MultisigShellResult::CallsCancelled => return ActionState::Cancelled,
        other => {
            return ActionState::Failed {
                detail: format!("unexpected answer {other:?}"),
            }
        }
    };
    let flight = InFlight {
        chain_id,
        multisig: input.account.address.clone(),
        safe_tx_hash: input.item.safe_tx_hash.clone(),
        nonce: input.item.nonce.to_string(),
        user_op_hash,
        by: input.executor.address.clone(),
        maybe_sent,
        started_ms: now,
        created_ms: input.item.created_ms,
    };
    let executing = {
        let mut s = lock(st);
        let k = exec_key(chain_id, &flight.multisig, &flight.safe_tx_hash);
        // Watched from now: a Follow never watches it a second time.
        s.watching.insert(k.clone());
        s.executing.insert(k, flight.clone());
        s.executing.clone()
    };
    save_json(io, EXECUTING_KEY, &executing, true).await;
    io.emit(Event::Changed);
    follow_flight(io, st, &flight, own, Some(&input.item)).await;
    ActionState::Done {
        safe_tx_hash: input.item.safe_tx_hash.clone(),
        kept: false,
    }
}

/// One look at `input`, kept for its executor (`preflight.check`'s `keep`):
/// when it was taken and what it said — for the sheet, the home's "Can't run
/// now", and Execute.
async fn look_now(io: &Io<Event>, st: &Shared<State>, input: &ExecutionInput) -> Preview {
    let preview = preview_execution(io, st, input).await;
    let at = io.now().await;
    let key = preview_key(
        input.chain_id,
        &input.item.safe_tx_hash,
        &input.executor.address,
    );
    let mut s = lock(st);
    flows::keep_look(&mut s, input, &preview);
    s.previews.insert(key.clone(), preview.to_value());
    s.preview_plans.insert(key, (at, preview.clone()));
    preview
}

/// Look at `input` unless its executor's last look is fresh (`fresh_ms`;
/// an unpriced or unchecked one [`UNPRICED_FRESH_MS`] at most) or one is
/// under way (`preflight.check`): one look per execution and executor, never
/// a second answer to the same question. A look that does not answer within
/// [`PREVIEW_CAP_MS`] says `unchecked` (never a guess); a number found used
/// is settled as another member's execution (`settleSpent`).
pub(crate) async fn look_if_stale(
    io: &Io<Event>,
    st: &Shared<State>,
    input: &ExecutionInput,
    fresh_ms: f64,
) {
    let key = preview_key(
        input.chain_id,
        &input.item.safe_tx_hash,
        &input.executor.address,
    );
    let now = io.now().await;
    {
        let mut s = lock(st);
        if s.looking.contains(&key) {
            return;
        }
        let fresh = s.preview_plans.get(&key).is_some_and(|(at, p)| {
            // r7: a look that stops it (nobody can pay, it would fail) is
            // asked again at the sheet's pace even on the home — an
            // executor funded is noticed on the next poll, not a minute on.
            let fresh = match p {
                Preview::Unpriced | Preview::Unchecked => fresh_ms.min(UNPRICED_FRESH_MS),
                Preview::Fails(_) | Preview::Priced { can_pay: false, .. } => {
                    fresh_ms.min(PREVIEW_FRESH_MS)
                }
                _ => fresh_ms,
            };
            // A look taken at other approvals is not this execution's.
            let same = s
                .more
                .looks
                .get(&key)
                .is_some_and(|(approved, _)| *approved == input.item.approved);
            same && now - at < fresh
        });
        if fresh {
            return;
        }
        s.looking.insert(key.clone());
    }
    let read = Box::pin(preview_execution(io, st, input));
    let cap = Box::pin(io.wait(PREVIEW_CAP_MS));
    let preview = match future::select(read, cap).await {
        future::Either::Left((preview, _)) => preview,
        future::Either::Right(_) => Preview::Unchecked,
    };
    let at = io.now().await;
    {
        let mut s = lock(st);
        s.looking.remove(&key);
        flows::keep_look(&mut s, input, &preview);
        s.previews.insert(key.clone(), preview.to_value());
        s.preview_plans.insert(key, (at, preview.clone()));
    }
    if preview == Preview::Spent {
        settle_spent(io, st, input).await;
    }
}

/// The home's "Can't run now" (r4-5): each ready transaction of the followed
/// multisig is looked at by each executor here — again once its look is
/// older than [`HOME_FRESH_MS`] — so `standings` says why one cannot run,
/// with the look's own detail.
async fn look_at_ready(io: &Io<Event>, st: &Shared<State>) {
    let inputs: Vec<ExecutionInput> = {
        let s = lock(st);
        let ready: Vec<(u64, String, Vec<String>)> = s
            .items()
            .iter()
            .filter(|i| {
                i.status == ProposalStatus::Ready && !i.executing && i.cancelled_by.is_none()
            })
            .map(|i| (i.chain_id, i.safe_tx_hash.clone(), i.can_execute.clone()))
            .collect();
        drop(s);
        ready
            .into_iter()
            .flat_map(|(chain_id, hash, executors)| {
                executors
                    .into_iter()
                    .filter_map(|e| execution_input(st, chain_id, &hash, &e))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    future::join_all(
        inputs
            .iter()
            .map(|input| look_if_stale(io, st, input, HOME_FRESH_MS)),
    )
    .await;
}

/// r5reg-21 (`settleSpent`): a look found its number used — another member
/// executed it, or another transaction at its number ran — so what ran is
/// read (its execution, final at its number) and said the way a lost race
/// is, never "it would fail"; then the list is read again. Not while this
/// device's own execution of it is followed, nor over a verdict already
/// said (an execution's own watch says how that one ended).
async fn settle_spent(io: &Io<Event>, st: &Shared<State>, input: &ExecutionInput) {
    let (chain_id, safe, hash) = (
        input.chain_id,
        input.account.address.as_str(),
        input.item.safe_tx_hash.as_str(),
    );
    let key = verdict_key(chain_id, hash);
    let followed = |s: &State, now: f64| {
        s.watching.contains(&exec_key(chain_id, safe, hash))
            || s.is_executing(chain_id, safe, hash, now)
            || s.verdicts.contains_key(&key)
    };
    let now = io.now().await;
    if followed(&lock(st), now) {
        return;
    }
    let found = lookup_execution(
        io,
        st,
        safe,
        chain_id,
        hash,
        input.item.created_ms,
        true,
        Some(input.item.nonce),
    )
    .await;
    let verdict = multisig_watch::execution_verdict(&ExecutionFacts {
        nonce_used: Some(true),
        executed_in: match found {
            Ok(Some(tx_hash)) => ExecutedIn::Found { tx_hash },
            Ok(None) => ExecutedIn::None,
            Err(_) => ExecutedIn::Unknown,
        },
        // Nothing of this device's was sent.
        our_op: OurOp::Unknown,
        receipt: OurReceipt::Unread,
        elapsed_ms: 0,
        limit_ms: 0,
    });
    {
        let mut s = lock(st);
        if followed(&s, now) {
            return;
        }
        s.verdicts
            .insert(key, serde_json::to_value(&verdict).unwrap_or(Value::Null));
    }
    io.emit(Event::Changed);
    refresh(io, st, false, true).await;
}

/// The member used here last for the followed multisig (`lastActor.note`),
/// noted only when more than one of its members on `chain_id` is on this
/// device.
async fn note_actor(io: &Io<Event>, st: &Shared<State>, chain_id: u64, actor: &str) {
    let map = {
        let mut s = lock(st);
        let Some(account) = s.account.clone() else {
            return;
        };
        let here = s
            .chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map_or(0, |c| local_owners(&s.locals, &c.members.owners).len());
        if here <= 1 {
            return;
        }
        s.last_actor.insert(
            account.address.to_ascii_lowercase(),
            actor.to_ascii_lowercase(),
        );
        s.last_actor.clone()
    };
    save_json(io, keys::LAST_ACTOR, &map, false).await;
}

/// One activity row for an executed proposal: the facts; the shell's own
/// transaction store titles it and reads its token (`executionRows`).
fn execution_row(
    multisig: &str,
    item: &ProposalItem,
    tx_hash: &str,
    user_op_hash: &str,
    at_ms: f64,
) -> Value {
    json!({
        "id": format!("multisig:{}:{}:0", item.chain_id, item.safe_tx_hash),
        "userOpHash": user_op_hash,
        "txHash": tx_hash,
        "from": multisig,
        "chainId": item.chain_id,
        "timestamp": (at_ms / 1000.0).floor(),
        "status": "confirmed",
        "safeTxHash": item.safe_tx_hash,
        "nonce": item.nonce.to_string(),
        "action": item.review.action,
        "rows": execution_rows(multisig, item),
    })
}

/// The Activity rows executing `item` writes (`executionRows`, W9): one per
/// transfer — a transfer, or a batch of transfers alone, is said by its
/// transfers — and otherwise one more row titled as its proposal (a call,
/// calls in a batch, a member change, a rename, a cancellation), moving only
/// the coin a call sends, to what it calls (a batch's own target, else the
/// multisig). Amounts are base units in decimal; the shell only words them
/// (a token's symbol and decimals, the proposal's title).
pub fn execution_rows(multisig: &str, item: &ProposalItem) -> Vec<Value> {
    let action = item.review.action.as_ref();
    let mut rows: Vec<Value> = Vec::new();
    let send = |token: &Option<String>, to: &str, value: &str, rows: &mut Vec<Value>| {
        rows.push(json!({
            "index": rows.len(),
            "kind": "send",
            "to": to,
            "token": token,
            "amount": shell::quantity(value).map(|a| a.to_string()).unwrap_or_default(),
        }));
    };
    match action {
        Some(Action::Transfer { token, to, amount }) => {
            send(token, to, amount, &mut rows);
            return rows;
        }
        Some(Action::Batch { calls }) => {
            for c in calls {
                if let Action::Transfer { token, to, amount } = &c.action {
                    send(token, to, amount, &mut rows);
                }
            }
            let only_transfers = calls
                .iter()
                .all(|c| matches!(c.action, Action::Transfer { .. }));
            if only_transfers && !rows.is_empty() {
                return rows;
            }
        }
        _ => {}
    }
    let to = match action {
        Some(Action::Call { to, .. }) => to.clone(),
        Some(Action::Batch { .. }) => item.tx.to.clone(),
        _ => multisig.to_owned(),
    };
    let sends = match action {
        Some(Action::Call { value, .. }) => shell::quantity(value).unwrap_or(0),
        _ => 0,
    };
    let calls = match action {
        Some(Action::Batch { calls }) => calls.len(),
        _ => 1,
    };
    rows.push(json!({
        "index": rows.len(),
        "kind": "call",
        "to": to,
        "sends": sends.to_string(),
        "calls": calls,
        "contract": to.to_ascii_lowercase(),
    }));
    rows
}

// ---------------------------------------------------------------------------
// Following an execution (`execution-watch.svelte.ts`)
// ---------------------------------------------------------------------------

async fn our_op(io: &Io<Event>, flight: &InFlight, own: Option<&str>) -> OurOp {
    if let Some(own) = own {
        return OurOp::Landed {
            tx_hash: own.to_owned(),
        };
    }
    if flight.maybe_sent && !flight.user_op_hash.starts_with("0x") {
        return OurOp::Refused;
    }
    let MultisigShellResult::UserOpStatus { status, tx_hash } = io
        .ask(MultisigOperation::UserOpStatus {
            chain_id: flight.chain_id,
            user_op_hash: flight.user_op_hash.clone(),
        })
        .await
    else {
        return OurOp::Unknown;
    };
    let Some(status) = status else {
        return OurOp::Unknown;
    };
    let in_block = matches!(status.as_str(), "included" | "failed" | "rejected");
    if let (Some(tx), true) = (tx_hash, in_block) {
        return OurOp::Landed { tx_hash: tx };
    }
    if status == "rejected" {
        return OurOp::Refused;
    }
    let now = io.now().await;
    if status == "not_found" && flight.maybe_sent && now - flight.started_ms >= NOT_TAKEN_AFTER_MS {
        return OurOp::Refused;
    }
    OurOp::Pending
}

async fn receipt_logs(io: &Io<Event>, chain_id: u64, tx_hash: &str) -> Option<Vec<Value>> {
    io.rpc(chain_id, "eth_getTransactionReceipt", json!([tx_hash]))
        .await
        .ok()?
        .get("logs")?
        .as_array()
        .cloned()
}

fn executes_in(logs: &[Value], multisig: &str, hash: &str) -> bool {
    let hash = hash.to_ascii_lowercase();
    logs.iter().any(|l| {
        let topic = |i: usize| {
            l.get("topics")
                .and_then(|t| t.get(i))
                .and_then(Value::as_str)
                .map(str::to_ascii_lowercase)
        };
        l.get("address")
            .and_then(Value::as_str)
            .is_some_and(|a| same_address(a, multisig))
            && topic(0).as_deref() == Some(EXECUTION_SUCCESS)
            && (topic(1).as_deref() == Some(hash.as_str())
                || l.get("data")
                    .and_then(Value::as_str)
                    .is_some_and(|d| d.to_ascii_lowercase().starts_with(&hash)))
    })
}

fn carries_op(logs: &[Value], user_op_hash: &str) -> bool {
    logs.iter().any(|l| {
        let t = l.get("topics").and_then(Value::as_array);
        t.and_then(|t| t.first())
            .and_then(Value::as_str)
            .is_some_and(|x| x.eq_ignore_ascii_case(USER_OPERATION_EVENT))
            && t.and_then(|t| t.get(1))
                .and_then(Value::as_str)
                .is_some_and(|x| x.eq_ignore_ascii_case(user_op_hash))
    })
}

async fn judge_execution(
    io: &Io<Event>,
    st: &Shared<State>,
    flight: &InFlight,
    own: Option<&str>,
) -> ExecutionVerdict {
    let chain = lock(st).chain.clone();
    let (nonce, op) = futures::join!(
        chain.read_nonce(io, flight.chain_id, &flight.multisig),
        our_op(io, flight, own)
    );
    let flight_nonce = shell::quantity(&flight.nonce).unwrap_or(0);
    let nonce_used = nonce.ok().map(|n| n > flight_nonce);
    let mut executed_in = ExecutedIn::None;
    let mut landed = op.clone();
    let mut receipt = OurReceipt::Unread;
    if let OurOp::Landed { tx_hash } = &op {
        if let Some(logs) = receipt_logs(io, flight.chain_id, tx_hash).await {
            receipt = if executes_in(&logs, &flight.multisig, &flight.safe_tx_hash) {
                OurReceipt::Executed
            } else {
                OurReceipt::NotExecuted
            };
        }
    }
    let is_landed = matches!(op, OurOp::Landed { .. });
    if receipt != OurReceipt::Executed && (nonce_used != Some(false) || is_landed) {
        let found = lookup_execution(
            io,
            st,
            &flight.multisig,
            flight.chain_id,
            &flight.safe_tx_hash,
            flight.created_ms,
            nonce_used == Some(true),
            Some(flight_nonce),
        )
        .await;
        executed_in = match found {
            Err(_) => ExecutedIn::Unknown,
            Ok(None) => ExecutedIn::None,
            Ok(Some(found)) => match &op {
                OurOp::Landed { tx_hash } if found.eq_ignore_ascii_case(tx_hash) => {
                    ExecutedIn::Found { tx_hash: found }
                }
                _ => match receipt_logs(io, flight.chain_id, &found).await {
                    Some(logs) if executes_in(&logs, &flight.multisig, &flight.safe_tx_hash) => {
                        if carries_op(&logs, &flight.user_op_hash) {
                            landed = OurOp::Landed {
                                tx_hash: found.clone(),
                            };
                            receipt = OurReceipt::Executed;
                        }
                        ExecutedIn::Found { tx_hash: found }
                    }
                    _ => ExecutedIn::Unknown,
                },
            },
        };
    }
    let now = io.now().await;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let elapsed = (now - flight.started_ms).max(0.0) as u64;
    multisig_watch::execution_verdict(&ExecutionFacts {
        nonce_used,
        executed_in,
        our_op: landed,
        receipt,
        elapsed_ms: elapsed,
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        limit_ms: EXECUTION_LIMIT_MS as u64,
    })
}

async fn watch_execution(
    io: &Io<Event>,
    st: &Shared<State>,
    flight: &InFlight,
    own: Option<String>,
) -> ExecutionVerdict {
    let mut unknown_since: Option<f64> = None;
    loop {
        let verdict = judge_execution(io, st, flight, own.as_deref()).await;
        let now = io.now().await;
        match verdict {
            ExecutionVerdict::Unknown => {
                let since = *unknown_since.get_or_insert(now);
                if now - flight.started_ms >= EXECUTION_LIMIT_MS || now - since >= UNKNOWN_GRACE_MS
                {
                    return verdict;
                }
            }
            ExecutionVerdict::Waiting => unknown_since = None,
            other => return other,
        }
        if now - flight.started_ms >= EXECUTION_LIMIT_MS {
            return ExecutionVerdict::TimedOut;
        }
        let key = exec_key(flight.chain_id, &flight.multisig, &flight.safe_tx_hash);
        lock(st).verdicts.insert(
            format!(
                "{}:{}",
                flight.chain_id,
                flight.safe_tx_hash.to_ascii_lowercase()
            ),
            json!({"kind": "waiting"}),
        );
        let _ = key;
        io.emit(Event::Changed);
        io.wait(EXECUTION_PAUSE_MS).await;
    }
}

/// Follow `flight` to its end (the sheet's `follow`): the verdict kept for
/// its sheet; any end but "waiting" ends it in flight here
/// (`executions.end`); executed, its lookup remembered and `item`'s
/// activity row handed on.
async fn follow_flight(
    io: &Io<Event>,
    st: &Shared<State>,
    flight: &InFlight,
    own: Option<String>,
    item: Option<&ProposalItem>,
) -> ExecutionVerdict {
    let key = exec_key(flight.chain_id, &flight.multisig, &flight.safe_tx_hash);
    lock(st).watching.insert(key.clone());
    let verdict = watch_execution(io, st, flight, own).await;
    let ended = {
        let mut s = lock(st);
        s.watching.remove(&key);
        s.verdicts.insert(
            verdict_key(flight.chain_id, &flight.safe_tx_hash),
            serde_json::to_value(&verdict).unwrap_or(Value::Null),
        );
        let ended = verdict != ExecutionVerdict::Waiting && s.executing.remove(&key).is_some();
        ended.then(|| s.executing.clone())
    };
    if let Some(executing) = ended {
        save_json(io, EXECUTING_KEY, &executing, true).await;
    }
    if let ExecutionVerdict::Executed { tx_hash } = &verdict {
        remember_lookup(
            io,
            st,
            flight.chain_id,
            &flight.multisig,
            &flight.safe_tx_hash,
            Some(tx_hash),
        )
        .await;
        if let Some(item) = item {
            io.notice(Notice::ActivityRows {
                rows: vec![execution_row(
                    &flight.multisig,
                    item,
                    tx_hash,
                    &flight.user_op_hash,
                    flight.started_ms,
                )],
            })
            .await;
        }
    }
    io.emit(Event::Changed);
    verdict
}

/// r4b-7: the executions this device sent for the followed multisig that
/// may still land (`executions.get`) and are not watched yet — marked
/// watched, so each flight is followed once.
fn flights_to_resume(s: &mut State, now: f64) -> Vec<InFlight> {
    let Some(account) = s.account.clone() else {
        return Vec::new();
    };
    let found: Vec<(String, InFlight)> = s
        .executing
        .values()
        .filter(|f| {
            same_address(&f.multisig, &account.address)
                && now - f.started_ms < EXECUTION_LIMIT_MS + 60_000.0
        })
        .map(|f| {
            (
                exec_key(f.chain_id, &f.multisig, &f.safe_tx_hash),
                f.clone(),
            )
        })
        .filter(|(k, _)| !s.watching.contains(k))
        .collect();
    for (k, _) in &found {
        s.watching.insert(k.clone());
    }
    found.into_iter().map(|(_, f)| f).collect()
}

/// Watch each flight again (the sheet's resume effect) — never offered as
/// if nothing was sent: followed to its end as `execute`'s own is, then the
/// list read again.
fn resume_flights(io: &Io<Event>, st: &Shared<State>, flights: Vec<InFlight>) {
    for flight in flights {
        let state = st.clone();
        io.spawn(move |io| async move {
            let item = lock(&state)
                .item(flight.chain_id, &flight.safe_tx_hash)
                .map(|(i, _)| i);
            follow_flight(&io, &state, &flight, None, item.as_ref()).await;
            refresh(&io, &state, false, true).await;
            io.emit(Event::Changed);
        });
    }
}

// ---------------------------------------------------------------------------
// The chain's history, in windows (`records.ts`)
// ---------------------------------------------------------------------------

fn lookup_key(chain_id: u64, multisig: &str, hash: &str) -> String {
    format!(
        "{chain_id}:{}:{}",
        multisig.to_ascii_lowercase(),
        hash.to_ascii_lowercase()
    )
}

async fn remember_lookup(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    multisig: &str,
    hash: &str,
    found: Option<&str>,
) {
    let all = {
        let mut s = lock(st);
        s.lookups.insert(
            lookup_key(chain_id, multisig, hash),
            found.unwrap_or("none").to_owned(),
        );
        while s.lookups.len() > LOOKUPS_CAP {
            let Some(first) = s.lookups.keys().next().cloned() else {
                break;
            };
            s.lookups.remove(&first);
        }
        s.lookups.clone()
    };
    save_json(io, keys::EXECUTIONS, &all, false).await;
}

/// The history could not be read: nothing can be said of what it holds.
#[derive(Debug)]
struct HistoryUnreadable;

/// A node's "range too wide" error: the width it names, else `Some(0)` (halve).
fn range_cap(message: &str) -> Option<u64> {
    let lower = message.to_ascii_lowercase();
    let wide = [
        "range",
        "block",
        "limit",
        "too many",
        "exceed",
        "10000",
        "query returned more",
    ]
    .iter()
    .any(|w| lower.contains(w));
    if !wide {
        return None;
    }
    let digits: Vec<u64> = lower
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|d| d.parse().ok())
        .filter(|n| *n >= 10)
        .collect();
    Some(digits.into_iter().min().unwrap_or(0))
}

async fn block_of(
    io: &Io<Event>,
    chain_id: u64,
    tag: &str,
) -> Result<(u64, u64), HistoryUnreadable> {
    let b = io
        .rpc(chain_id, "eth_getBlockByNumber", json!([tag, false]))
        .await
        .map_err(|_| HistoryUnreadable)?;
    let n = b
        .get("number")
        .and_then(Value::as_str)
        .and_then(shell::quantity);
    let t = b
        .get("timestamp")
        .and_then(Value::as_str)
        .and_then(shell::quantity);
    match (n, t) {
        (Some(n), Some(t)) => Ok((
            u64::try_from(n).unwrap_or(u64::MAX),
            u64::try_from(t).unwrap_or(u64::MAX),
        )),
        _ => Err(HistoryUnreadable),
    }
}

async fn block_before(
    io: &Io<Event>,
    chain_id: u64,
    ms: f64,
) -> Result<(u64, u64), HistoryUnreadable> {
    let (head, head_time) = block_of(io, chain_id, "latest").await?;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let at = (ms / 1000.0).floor() as u64;
    if at >= head_time {
        return Ok((head, head));
    }
    let back = head.min(FIRST_LOG_SPAN);
    if back == 0 {
        return Ok((0, head));
    }
    let (_, sample_time) = block_of(io, chain_id, &format!("0x{:x}", head - back)).await?;
    #[allow(clippy::cast_precision_loss)]
    let per_block = (((head_time - sample_time.min(head_time)) as f64) / back as f64).max(0.001);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    let mut distance = ((((head_time - at) as f64) / per_block) * 1.1).ceil() as u64 + 100;
    for _ in 0..6 {
        if distance >= head {
            return Ok((0, head));
        }
        let (n, t) = block_of(io, chain_id, &format!("0x{:x}", head - distance)).await?;
        if t <= at {
            return Ok((n, head));
        }
        distance *= 2;
    }
    Ok((0, head))
}

async fn search_logs(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    address: &str,
    hash: &str,
    from: u64,
    to: u64,
) -> Result<Option<(String, u64)>, HistoryUnreadable> {
    let mut hi = to;
    let mut span = lock(st)
        .spans
        .get(&chain_id)
        .copied()
        .unwrap_or(FIRST_LOG_SPAN);
    let mut windows = 0;
    loop {
        if hi < from {
            return Ok(None);
        }
        windows += 1;
        if windows > MAX_LOG_WINDOWS {
            return Err(HistoryUnreadable);
        }
        let lo = if hi + 1 > from + span {
            hi + 1 - span
        } else {
            from
        };
        let answer = io
            .rpc(
                chain_id,
                "eth_getLogs",
                json!([{ "address": address, "topics": [EXECUTION_SUCCESS, hash], "fromBlock": format!("0x{lo:x}"), "toBlock": format!("0x{hi:x}") }]),
            )
            .await;
        match answer {
            Err(shell::RpcFail::Error { message, .. }) => {
                let Some(cap) = range_cap(&message) else {
                    return Err(HistoryUnreadable);
                };
                if span <= 1 {
                    return Err(HistoryUnreadable);
                }
                span = if cap > 0 && cap < span {
                    cap
                } else {
                    (span / 2).max(1)
                };
                lock(st).spans.insert(chain_id, span);
                windows -= 1;
            }
            Err(shell::RpcFail::Unanswered(_)) => return Err(HistoryUnreadable),
            Ok(Value::Array(logs)) => {
                if let Some(last) = logs.last() {
                    let tx = last
                        .get("transactionHash")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let block = last
                        .get("blockNumber")
                        .and_then(Value::as_str)
                        .and_then(shell::quantity)
                        .and_then(|b| u64::try_from(b).ok())
                        .unwrap_or(0);
                    return Ok(Some((tx, block)));
                }
                if lo == 0 {
                    return Ok(None);
                }
                hi = lo - 1;
            }
            Ok(_) => return Err(HistoryUnreadable),
        }
    }
}

async fn nonce_at(io: &Io<Event>, chain_id: u64, multisig: &str, block: u64) -> Option<u128> {
    io.rpc(
        chain_id,
        "eth_call",
        json!([{ "to": multisig, "data": NONCE_CALL }, format!("0x{block:x}")]),
    )
    .await
    .ok()
    .and_then(|r| r.as_str().and_then(shell::quantity))
}

/// The transaction that executed `hash` (`lookupExecution`): `Ok(None)` —
/// none (final once its number was used); `Err` — the history could not say.
#[allow(clippy::too_many_arguments)]
async fn lookup_execution(
    io: &Io<Event>,
    st: &Shared<State>,
    multisig: &str,
    chain_id: u64,
    hash: &str,
    since_ms: f64,
    is_final: bool,
    nonce: Option<u128>,
) -> Result<Option<String>, HistoryUnreadable> {
    lookup_execution_at(io, st, multisig, chain_id, hash, since_ms, is_final, nonce)
        .await
        .map(|found| found.map(|(tx, _)| tx))
}

/// [`lookup_execution`], with the block it ran in when the search found it
/// now (`None` for one this device already knew).
#[allow(clippy::too_many_arguments)]
async fn lookup_execution_at(
    io: &Io<Event>,
    st: &Shared<State>,
    multisig: &str,
    chain_id: u64,
    hash: &str,
    since_ms: f64,
    is_final: bool,
    nonce: Option<u128>,
) -> Result<Option<(String, Option<u64>)>, HistoryUnreadable> {
    let kept = lock(st)
        .lookups
        .get(&lookup_key(chain_id, multisig, hash))
        .cloned();
    match kept.as_deref() {
        Some("none") => return Ok(None),
        Some(tx) => return Ok(Some((tx.to_owned(), None))),
        None => {}
    }
    let (from, head) = if since_ms > 0.0 {
        block_before(io, chain_id, (since_ms - CREATED_MARGIN_MS).max(0.0)).await?
    } else {
        (0, block_of(io, chain_id, "latest").await?.0)
    };
    let found = search_logs(io, st, chain_id, multisig, hash, from, head).await?;
    if found.is_none() {
        if let Some(nonce) = nonce {
            if from > 0
                && nonce_at(io, chain_id, multisig, from)
                    .await
                    .is_some_and(|n| n > nonce)
            {
                return Err(HistoryUnreadable);
            }
            if is_final
                && nonce_at(io, chain_id, multisig, head)
                    .await
                    .is_none_or(|n| n <= nonce)
            {
                return Err(HistoryUnreadable);
            }
        }
    }
    if found.is_some() || is_final {
        remember_lookup(
            io,
            st,
            chain_id,
            multisig,
            hash,
            found.as_ref().map(|(t, _)| t.as_str()),
        )
        .await;
    }
    Ok(found.map(|(t, b)| (t, (b > 0).then_some(b))))
}

/// Executions on another member's device: found by their `ExecutionSuccess`
/// and handed to the activity once (`recordRemoteExecutions`).
async fn record_remote_executions(io: &Io<Event>, st: &Shared<State>, account: &MultisigAccount) {
    let done: Vec<ProposalItem> = lock(st)
        .items()
        .into_iter()
        .filter(|i| i.status == ProposalStatus::Done)
        .collect();
    if done.is_empty() {
        return;
    }
    let mut unreadable: Vec<u64> = Vec::new();
    let mut rows = Vec::new();
    for item in done {
        if unreadable.contains(&item.chain_id) {
            continue;
        }
        let known = lock(st).lookups.contains_key(&lookup_key(
            item.chain_id,
            &account.address,
            &item.safe_tx_hash,
        ));
        if known {
            continue;
        }
        match lookup_execution_at(
            io,
            st,
            &account.address,
            item.chain_id,
            &item.safe_tx_hash,
            item.created_ms,
            true,
            Some(item.nonce),
        )
        .await
        {
            Ok(Some((tx, block))) => {
                // r8-17: dated when it ran (its block's time), not when this
                // device found it; now only when the block's time can't be read.
                let ran = match block {
                    Some(b) => {
                        flows::block_time(io, item.chain_id, Some(&json!(format!("0x{b:x}")))).await
                    }
                    None => None,
                };
                let at = match ran {
                    Some(at) => at,
                    None => io.now().await,
                };
                rows.push(execution_row(&account.address, &item, &tx, "", at));
            }
            Ok(None) => {}
            Err(_) => unreadable.push(item.chain_id),
        }
    }
    {
        let mut s = lock(st);
        if s.followed(&account.address) {
            s.history_unreadable = unreadable;
        }
    }
    if !rows.is_empty() {
        io.notice(Notice::ActivityRows { rows }).await;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn revert_text_reads_error_string() {
        let data = "0x08c379a0\
            0000000000000000000000000000000000000000000000000000000000000020\
            000000000000000000000000000000000000000000000000000000000000000a\
            4e6f7420656e6f75676800000000000000000000000000000000000000000000";
        assert_eq!(revert_text(data).as_deref(), Some("Not enough"));
        assert_eq!(revert_text("0x1234"), None);
    }

    #[test]
    fn plain_reason_drops_what_says_nothing() {
        assert_eq!(plain_reason(Some("execution reverted")), None);
        assert_eq!(plain_reason(Some("Reverted")), None);
        assert_eq!(plain_reason(Some("target call failed")), None);
        assert_eq!(plain_reason(Some("GS013")).as_deref(), Some("GS013"));
        assert_eq!(plain_reason(None), None);
    }

    #[test]
    fn spending_sums_per_token() {
        let a = Action::Batch {
            calls: vec![
                multisig::BatchCall {
                    to: "0x1".to_owned(),
                    value: "5".to_owned(),
                    data: "0x".to_owned(),
                    action: Action::Transfer {
                        token: None,
                        to: "0x2".to_owned(),
                        amount: "5".to_owned(),
                    },
                },
                multisig::BatchCall {
                    to: "0xT".to_owned(),
                    value: "0".to_owned(),
                    data: "0x".to_owned(),
                    action: Action::Transfer {
                        token: Some("0xT".to_owned()),
                        to: "0x2".to_owned(),
                        amount: "7".to_owned(),
                    },
                },
                multisig::BatchCall {
                    to: "0x1".to_owned(),
                    value: "0".to_owned(),
                    data: "0x".to_owned(),
                    action: Action::Transfer {
                        token: None,
                        to: "0x3".to_owned(),
                        amount: "1".to_owned(),
                    },
                },
            ],
        };
        assert_eq!(
            spending_of(Some(&a)),
            vec![(None, 6), (Some("0xT".to_owned()), 7)]
        );
        assert!(spending_of(Some(&Action::Cancel)).is_empty());
    }

    #[test]
    fn a_range_error_names_its_width() {
        assert_eq!(range_cap("query exceeds max block range 2000"), Some(2000));
        assert_eq!(range_cap("block range too wide"), Some(0));
        assert_eq!(range_cap("header not found"), None);
    }
}
