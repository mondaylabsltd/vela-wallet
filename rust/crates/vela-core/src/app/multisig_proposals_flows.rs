//! The proposals machine's later rules, in their own file (a child module, so
//! they read the machine's state as its own code does):
//!
//! - **presence** (`presence.svelte.ts`): what waits on this device per
//!   multisig — the followed one from its own items and the looks at its
//!   executors (`blockOf`), the others read every
//!   [`OTHERS_EVERY_MS`](super::super::multisig_presence::OTHERS_EVERY_MS) on
//!   the shell's [`Event::OthersTick`] (the cross-chain list, then each used
//!   network's chain, quiet, skipped during an outage), and before any read
//!   the list an earlier page kept (r4b-11) — never a zero it did not see;
//! - **the timeline** (`timeline.ts`, `executed.ts`): each proposal's story as
//!   data, with an executed one's frozen facts read from its receipt (else
//!   what this device recorded when it executed it);
//! - **a dApp's request** (`dapp-request.ts`): held until its proposal
//!   executes, answered once, from the view;
//! - **a proposal link** (`linked.svelte.ts`): its proposal on the followed
//!   multisig, or read only on one with no member here;
//! - **an outside member's wallet** (`approveExternally`): the reviewed draft,
//!   or an open proposal, handed to the session machine and its verified
//!   answer submitted through the outbox.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::super::dapp_rpc;
use super::super::multisig_dapp::{self as dapp, DappAnswer, DappError, HeldRequest, HeldStage};
use super::super::multisig_outside::session::{SessionOutcome, SessionRequest};
use super::super::multisig_outside::{plain_accounts, ApprovedWith, SignerFacts};
use super::super::multisig_presence::{
    self as presence, block_of, facts_of_execution, standing_of, waiting_note, waiting_of,
    DeviceFacts, FactsSource, FrozenFacts, ListFacts, Look, ReadyBlock, Standing, TimelineEnd,
    TimelineStep, Waiting, WaitingNote, EXECUTED_KEEP, EXECUTED_KEY,
};
use super::super::parent_safe_guard::{
    self as parent_guard, OwnerFact, ParentVerdict, SafeOwnerAnswer,
};
use super::*;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

struct Held {
    view: HeldRequest,
    params: Value,
    round: u64,
    limit_ms: Option<f64>,
    claimed: Option<f64>,
}

struct Linked {
    safe: String,
    chain_id: u64,
    hash: String,
    preview: Option<MultisigAccount>,
    chain: Option<ChainState>,
    list: Option<Vec<Proposal>>,
    loading: bool,
    stale: bool,
    unreadable: bool,
    /// r13-3: which link this is (a deadline speaks only for its own).
    opened: u64,
    /// r13-3: its transaction was not read within `OPEN_TIMEOUT_MS`: the
    /// key that names the read still out.
    failed: Option<String>,
}

struct OutsidePending {
    account: MultisigAccount,
    tx: SafeTx,
    owners: Vec<String>,
    action: String,
    round: Option<u64>,
}

/// core-ms-2: how many `wallet_getCallsStatus` answers wait to be taken
/// ([`Event::CallsStatusTaken`]) before the oldest goes.
pub const CALLS_STATUS_KEPT: usize = 16;

/// What these rules keep in the machine's state.
#[derive(Default)]
pub struct More {
    accounts: Vec<MultisigAccount>,
    others: BTreeMap<String, Waiting>,
    kept_waiting: BTreeMap<String, Option<Waiting>>,
    others_reading: bool,
    /// One executor's look, by `<chainId>:<hash>:<executor>`: the count of
    /// approvals it was taken at, and what it said.
    pub(crate) looks: BTreeMap<String, (u64, Look)>,
    frozen: BTreeMap<String, FrozenFacts>,
    dapp: BTreeMap<String, Held>,
    calls_status: BTreeMap<String, DappAnswer>,
    /// The ids of `calls_status`, oldest first (its bound).
    calls_order: std::collections::VecDeque<String>,
    linked: Option<Linked>,
    /// r13-3: links opened so far (each link's `opened`).
    linked_opened: u64,
    outside: BTreeMap<String, OutsidePending>,
    indexed: Vec<String>,
}

/// r10-6: the one review of this transaction refuses it for what it is (a
/// delegatecall, a batch too deep, a refund…), not for the state guard's
/// finding: nothing about whether it would run is said beside the refusal —
/// no look at it, no "expected to fail" (`StateVerdict::Withheld`).
pub(crate) fn refused_withheld(s: &State, chain_id: u64, hash: &str) -> bool {
    s.reviews
        .get(&item_key(chain_id, hash))
        .and_then(|r| r.get("refused"))
        .and_then(|r| r.get("why"))
        .and_then(Value::as_str)
        .is_some_and(|why| why != "stateChange")
}

fn item_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{}", hash.to_ascii_lowercase())
}

fn look_key(chain_id: u64, hash: &str, executor: &str) -> String {
    format!(
        "{chain_id}:{}:{}",
        hash.to_ascii_lowercase(),
        executor.to_ascii_lowercase()
    )
}

// ---------------------------------------------------------------------------
// The view
// ---------------------------------------------------------------------------

/// Where a link's proposal stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigLinkedStand"))]
pub enum LinkedStand {
    /// Still reading (said, never a blank wait — ms-16).
    Reading,
    /// Read, re-hashed to the link's hash: its sheet opens on `item`.
    Found,
    /// The mailbox was read and holds no transaction that hashes to the
    /// link's (not there yet, or a copy that does not hash as the link
    /// says): said as that (ms-22), read again on every tick.
    NotInMailbox,
    /// The mailbox could not be read: nothing can be said of it yet.
    NotFound,
    /// The network did not answer (r4b-3): what was read stays.
    Unreadable,
    /// r13-3: not read within `OPEN_TIMEOUT_MS` — said with the read still
    /// out (`reason_key`) and Try again (`retry`), never an endless
    /// "Opening the transaction…".
    Failed,
}

/// The proposal a link opened (`LinkedProposals`, the sheet's source).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigLinkedView"))]
pub struct LinkedView {
    pub safe: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
    /// ms-21: no member of it is here — read only; nothing added, nothing
    /// switched. Its members may still sign with their own wallets (the
    /// connect page, or [`Event::ApproveOutside`]).
    pub preview: bool,
    pub stand: LinkedStand,
    pub item: Option<ProposalItem>,
    /// The chain's members now (for the connect link's members).
    pub members: Option<ChainMembers>,
    /// The last read failed: what is shown may be stale.
    pub stale: bool,
    /// r13-3: what to say when it is not shown — `failed` (the read still
    /// out when the bound ran out), `unreadable` (the network did not
    /// answer) or `notFound` (the service did not): the corpus key
    /// (`home.multisig.proposal.unread.link*`, `{{network}}`: the network's
    /// name). Null while reading, found, or `notInMailbox`.
    pub reason_key: Option<String>,
    /// r13-3: offer "Try again" (the proposals machine's `retry`): every
    /// stand with a `reason_key`.
    pub retry: bool,
}

/// What these rules add to [`ProposalsView`].
pub struct MoreView {
    pub presence: BTreeMap<String, Option<Waiting>>,
    pub others_waiting: bool,
    pub standings: BTreeMap<String, Standing>,
    pub waiting_note: Option<WaitingNote>,
    pub timelines: BTreeMap<String, Vec<TimelineStep>>,
    pub dapp: BTreeMap<String, HeldRequest>,
    pub calls_status: BTreeMap<String, DappAnswer>,
    pub linked: Option<LinkedView>,
    /// What [`read_frozen`] read, per `<chainId>:<safeTxHash lowercase>`.
    pub frozen: BTreeMap<String, FrozenFacts>,
}

/// Why a ready `item` of the followed multisig cannot run here, from the
/// looks at each executor here (`readyBlock`): `None` while one is out.
fn ready_block(s: &State, item: &ProposalItem) -> Option<ReadyBlock> {
    // r10-6: a review refused for what it is says only its refusal — no
    // "can't run now, expected to fail" from a look at it.
    if item.can_execute.is_empty() || refused_withheld(s, item.chain_id, &item.safe_tx_hash) {
        return None;
    }
    let looks: Vec<Option<Look>> = item
        .can_execute
        .iter()
        .map(|e| {
            s.more
                .looks
                .get(&look_key(item.chain_id, &item.safe_tx_hash, e))
                .filter(|(approved, _)| *approved == item.approved)
                .map(|(_, l)| l.clone())
        })
        .collect();
    block_of(&looks)
}

/// r4b-11: what the list an earlier page kept says waits on `account` —
/// read once a page; `None` when none is kept.
fn kept_of(s: &mut State, account: &MultisigAccount) -> Option<Waiting> {
    let key = account.address.to_ascii_lowercase();
    if let Some(w) = s.more.kept_waiting.get(&key) {
        return w.clone();
    }
    let kept = s.waiting.get(&key)?.clone();
    if kept.service != s.service.url() {
        return None;
    }
    let mut items = Vec::new();
    for c in &kept.chains {
        let mut read: Vec<ProposalItem> = c
            .proposals
            .iter()
            .filter_map(|p| read_proposal(&account.address, &c.members, p, &[], &s.locals, false))
            .collect();
        mark_cancellations(&mut read);
        items.extend(read);
    }
    let w = waiting_of(&items, |_| None);
    s.more.kept_waiting.insert(key, Some(w.clone()));
    Some(w)
}

/// How `item` (of `safe`) ended, as far as this device knows.
fn end_of(s: &State, items: &[ProposalItem], safe: &str, item: &ProposalItem) -> TimelineEnd {
    if item.status != ProposalStatus::Done {
        return TimelineEnd::Open;
    }
    let tx_of = |i: &ProposalItem| {
        s.lookups
            .get(&lookup_key(i.chain_id, safe, &i.safe_tx_hash))
            .cloned()
    };
    match tx_of(item) {
        Some(tx) if tx != "none" => TimelineEnd::Executed {
            frozen: s
                .more
                .frozen
                .get(&item_key(item.chain_id, &item.safe_tx_hash))
                .cloned(),
            tx_hash: tx,
        },
        Some(_) => {
            let by = items.iter().find(|o| {
                o.chain_id == item.chain_id
                    && o.nonce == item.nonce
                    && o.safe_tx_hash != item.safe_tx_hash
                    && tx_of(o).is_some_and(|t| t != "none")
            });
            TimelineEnd::Replaced {
                by: by.map(|o| o.safe_tx_hash.clone()),
                cancellation: by.is_some_and(|o| matches!(o.review.action, Some(Action::Cancel))),
            }
        }
        None => TimelineEnd::Open,
    }
}

fn linked_view(s: &State) -> Option<LinkedView> {
    let l = s.more.linked.as_ref()?;
    let (chain, loading, unreadable, stale) = match &l.preview {
        Some(_) => (l.chain.clone(), l.loading, l.unreadable, l.stale),
        None => (
            s.chains.iter().find(|c| c.chain_id == l.chain_id).cloned(),
            s.loading || s.read_at.is_none() && s.chains.is_empty(),
            s.unreadable.contains(&l.chain_id),
            s.stale,
        ),
    };
    let item = chain.as_ref().and_then(|c| {
        c.items
            .iter()
            .find(|i| i.safe_tx_hash.eq_ignore_ascii_case(&l.hash))
            .cloned()
    });
    let stand = if item.is_some() {
        LinkedStand::Found
    } else if unreadable {
        LinkedStand::Unreadable
    } else if loading || chain.is_none() {
        if l.failed.is_some() {
            LinkedStand::Failed
        } else {
            LinkedStand::Reading
        }
    } else if chain.as_ref().is_some_and(|c| c.mailbox) {
        LinkedStand::NotInMailbox
    } else {
        LinkedStand::NotFound
    };
    // Round 16 leftover 3: a service that answered with its rate limit is
    // said busy (the home's words), never "Couldn't reach the multisig
    // service".
    let busy = lock(&s.service.outage).rate_limited;
    let reason_key = match stand {
        LinkedStand::Failed => l.failed.clone(),
        LinkedStand::Unreadable => Some(review_state::UnreadRead::Chain.link_key().to_owned()),
        LinkedStand::NotFound if busy => Some(review_state::MISSING_BUSY_KEY.to_owned()),
        LinkedStand::NotFound => Some(review_state::UnreadRead::Service.link_key().to_owned()),
        _ => None,
    };
    Some(LinkedView {
        safe: l.safe.clone(),
        chain_id: l.chain_id,
        safe_tx_hash: l.hash.clone(),
        preview: l.preview.is_some(),
        stand,
        item,
        members: chain.map(|c| c.members),
        stale,
        retry: reason_key.is_some(),
        reason_key,
    })
}

/// r13-3: "Try again" on a link's transaction (the proposals `retry`): its
/// bound starts again; a preview's transaction is read again here (`true`),
/// the followed multisig's by the refresh the retry runs anyway.
pub(super) async fn retry_linked(io: &Io<Event>, st: &Shared<State>) -> bool {
    let preview = {
        let mut s = lock(st);
        let next = s.more.linked_opened + 1;
        let Some(l) = s.more.linked.as_mut() else {
            return false;
        };
        l.failed = None;
        l.opened = next;
        l.loading = true;
        l.unreadable = false;
        let preview = l.preview.is_some();
        s.more.linked_opened = next;
        preview
    };
    io.emit(Event::Changed);
    link_deadline(io, st);
    if preview {
        read_linked(io, st).await;
    }
    preview
}

/// r13-3: a link's transaction still not read when its bound ran out says
/// which read is still out — the reads go on, and it opens when they come.
fn link_deadline(io: &Io<Event>, st: &Shared<State>) {
    let Some(opened) = lock(st).more.linked.as_ref().map(|l| l.opened) else {
        return;
    };
    let st = st.clone();
    io.spawn(move |io| async move {
        io.wait(u64::from(review_state::OPEN_TIMEOUT_MS)).await;
        {
            let mut s = lock(&st);
            let reading = linked_view(&s).is_some_and(|v| v.stand == LinkedStand::Reading);
            let Some(chain_id) = s
                .more
                .linked
                .as_ref()
                .filter(|l| l.opened == opened)
                .map(|l| l.chain_id)
            else {
                return;
            };
            if !reading {
                return;
            }
            let unread = s.unread_now(chain_id).unwrap_or_else(|| {
                if s.mailbox_missed.contains(&chain_id) {
                    review_state::UnreadRead::Service
                } else {
                    review_state::UnreadRead::Chain
                }
            });
            if let Some(l) = s.more.linked.as_mut() {
                l.failed = Some(unread.link_key().to_owned());
            }
        }
        io.emit(Event::Changed);
    });
}

/// The view's later fields.
pub fn view_more(s: &mut State) -> MoreView {
    let items = s.items();
    let followed = s.account.clone();
    let mut accounts = s.more.accounts.clone();
    if let Some(f) = &followed {
        if !accounts
            .iter()
            .any(|a| same_address(&a.address, &f.address))
        {
            accounts.push(f.clone());
        }
    }
    let mut presence_map = BTreeMap::new();
    let mut others_waiting = false;
    for account in &accounts {
        let key = account.address.to_ascii_lowercase();
        let shown = followed
            .as_ref()
            .is_some_and(|f| same_address(&f.address, &account.address));
        let w = if shown && (s.read_at.is_some() || !s.chains.is_empty()) {
            let mut w = waiting_of(&items, |i| ready_block(s, i));
            // r11-1: a list shown from what an earlier page kept was read by nobody here.
            w.read_at = s
                .read_at
                .filter(|at| at.is_finite() && *at >= 0.0)
                .map(|at| at as u64);
            Some(w)
        } else if let Some(w) = (!shown).then(|| s.more.others.get(&key).cloned()).flatten() {
            Some(w)
        } else {
            kept_of(s, account)
        };
        if !shown && w.as_ref().is_some_and(Waiting::any) {
            others_waiting = true;
        }
        presence_map.insert(key, w);
    }
    let standings = items
        .iter()
        .filter(|i| i.status != ProposalStatus::Done)
        .map(|i| {
            (
                item_key(i.chain_id, &i.safe_tx_hash),
                standing_of(i, ready_block(s, i)),
            )
        })
        .collect();
    let (down, limited) = {
        let outage = lock(&s.service.outage);
        (outage.down, outage.rate_limited)
    };
    let note = waiting_note(&ListFacts {
        offline: s.offline,
        read_at: s.read_at,
        unreachable: service_unreachable(s, down),
        busy: service_unreachable(s, limited),
        unreadable: s.unreadable.clone(),
        cached_at: s.cached_at,
        stale: s.stale,
        history_unreadable: s.history_unreadable.clone(),
        loading: s.loading,
        nothing_shown: !items.iter().any(|i| i.status != ProposalStatus::Done)
            && s.missing.is_empty(),
    });
    let mut timelines = BTreeMap::new();
    if let Some(f) = &followed {
        for i in &items {
            let end = end_of(s, &items, &f.address, i);
            timelines.insert(
                item_key(i.chain_id, &i.safe_tx_hash),
                presence::timeline_of(i, &end),
            );
        }
    }
    let linked = linked_view(s);
    if let (Some(l), Some(item)) = (&s.more.linked, linked.as_ref().and_then(|v| v.item.clone())) {
        if l.preview.is_some() {
            let end = end_of(s, &[], &l.safe, &item);
            timelines.insert(
                item_key(item.chain_id, &item.safe_tx_hash),
                presence::timeline_of(&item, &end),
            );
        }
    }
    MoreView {
        presence: presence_map,
        others_waiting,
        standings,
        waiting_note: note,
        timelines,
        dapp: s
            .more
            .dapp
            .iter()
            .map(|(k, h)| (k.clone(), h.view.clone()))
            .collect(),
        calls_status: s.more.calls_status.clone(),
        linked,
        frozen: s.more.frozen.clone(),
    }
}

/// Something is followed closely: a dApp's request waiting for its
/// execution, or a link's proposal open (`LINKED_POLL_MS`).
pub fn watching(s: &State) -> bool {
    s.more.linked.is_some()
        || s.more
            .dapp
            .values()
            .any(|h| h.view.stage == HeldStage::Waiting)
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Follow `account` (a dApp's request names the multisig it was granted).
fn follow_for(s: &mut State, account: MultisigAccount) {
    let same = s
        .account
        .as_ref()
        .is_some_and(|a| same_address(&a.address, &account.address));
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
        s.review_states.clear();
        s.review_blind.clear();
        s.previews.clear();
    }
    s.account = Some(account);
    if s.chains.is_empty() {
        seed(s);
    }
}

fn settle_held(s: &mut State, id: &str, answer: DappAnswer) {
    if let Some(h) = s.more.dapp.get_mut(id) {
        h.view.settle(answer);
    }
}

pub(super) fn update(event: Event, model: &mut Model) -> Cmd {
    match event {
        Event::Accounts { accounts } => {
            let mut s = lock(&model.state);
            s.more
                .others
                .retain(|k, _| accounts.iter().any(|a| a.address.eq_ignore_ascii_case(k)));
            s.more.accounts = accounts;
            render()
        }
        Event::OthersTick => flow(model, |io, st| async move { others_tick(&io, &st).await }),
        Event::Timeline {
            chain_id,
            safe_tx_hash,
        } => flow(model, move |io, st| async move {
            read_frozen(&io, &st, chain_id, &safe_tx_hash).await;
        }),
        Event::DappRequest {
            id,
            account,
            method,
            params,
            origin,
            chain_id,
            limit_ms,
        } => dapp_request(
            model, id, *account, method, params, origin, chain_id, limit_ms,
        ),
        Event::DappLeave { id } => {
            let mut s = lock(&model.state);
            let Some((stage, round, refusal, hash)) = s.more.dapp.get(&id).map(|h| {
                (
                    h.view.stage,
                    h.round,
                    h.view.refusal.clone(),
                    h.view.safe_tx_hash.clone(),
                )
            }) else {
                return render();
            };
            match stage {
                HeldStage::Refused => {
                    let refusal = refusal.unwrap_or_else(dapp::declined);
                    settle_held(&mut s, &id, DappAnswer::err(refusal));
                }
                HeldStage::Reviewing => {
                    if round == s.draft_round {
                        on_withdraw(&mut s);
                        s.draft = None;
                        s.draft_round += 1;
                        s.actions
                            .insert("propose".to_owned(), ActionState::Cancelled);
                    }
                    settle_held(&mut s, &id, dapp::closing(hash.as_deref()));
                }
                HeldStage::Waiting => settle_held(&mut s, &id, dapp::closing(hash.as_deref())),
                HeldStage::Answered => {}
            }
            render()
        }
        Event::DappSettled { id } => {
            lock(&model.state).more.dapp.remove(&id);
            render()
        }
        Event::CallsStatus {
            id,
            chain_id,
            calls_id,
        } => flow(model, move |io, st| async move {
            let answer = calls_status(&io, &st, chain_id, &calls_id).await;
            let mut s = lock(&st);
            let more = &mut s.more;
            more.calls_order.retain(|k| *k != id);
            more.calls_order.push_back(id.clone());
            more.calls_status.insert(id, answer);
            // core-ms-2: answers not taken are kept at most CALLS_STATUS_KEPT.
            while more.calls_order.len() > CALLS_STATUS_KEPT {
                if let Some(old) = more.calls_order.pop_front() {
                    more.calls_status.remove(&old);
                }
            }
        }),
        Event::CallsStatusTaken { id } => {
            let mut s = lock(&model.state);
            s.more.calls_order.retain(|k| *k != id);
            if s.more.calls_status.remove(&id).is_none() {
                return Command::done();
            }
            render()
        }
        Event::OpenLinked {
            chain_id,
            safe_tx_hash,
            preview,
        } => {
            let preview = preview.map(|p| *p);
            {
                let mut s = lock(&model.state);
                let safe = preview
                    .as_ref()
                    .map(|p| p.address.clone())
                    .or_else(|| s.account.as_ref().map(|a| a.address.clone()))
                    .unwrap_or_default();
                s.more.linked = Some(Linked {
                    safe,
                    chain_id,
                    hash: safe_tx_hash.to_ascii_lowercase(),
                    preview: preview.clone(),
                    chain: None,
                    list: None,
                    loading: true,
                    stale: false,
                    unreadable: false,
                    opened: s.more.linked_opened + 1,
                    failed: None,
                });
                s.more.linked_opened += 1;
            }
            flow(model, move |io, st| async move {
                link_deadline(&io, &st);
                if preview.is_some() {
                    read_linked(&io, &st).await;
                } else {
                    refresh(&io, &st, false, true).await;
                }
            })
        }
        Event::CloseLinked => {
            lock(&model.state).more.linked = None;
            render()
        }
        Event::ApproveOutside {
            chain_id,
            safe_tx_hash,
        } => {
            let key = format!("approve:{}", safe_tx_hash.to_ascii_lowercase());
            lock(&model.state)
                .actions
                .insert(key.clone(), ActionState::Working);
            flow(model, move |io, st| async move {
                let outcome = approve_outside(&io, &st, chain_id, &safe_tx_hash, &key).await;
                lock(&st).actions.insert(key, outcome);
            })
        }
        Event::OutsideAnswered {
            chain_id,
            safe_tx_hash,
            outcome,
        } => flow(model, move |io, st| async move {
            outside_answered(&io, &st, chain_id, &safe_tx_hash, outcome).await;
        }),
        Event::IndexedMembers { addresses } => {
            lock(&model.state).more.indexed = addresses;
            render()
        }
        _ => render(),
    }
}

// ---------------------------------------------------------------------------
// Presence: the others (`refreshOthers`, `readOther`)
// ---------------------------------------------------------------------------

async fn others_tick(io: &Io<Event>, st: &Shared<State>) {
    let (service, accounts, followed, networks) = {
        let mut s = lock(st);
        if s.more.others_reading {
            return;
        }
        s.more.others_reading = true;
        (
            s.service.clone(),
            s.more.accounts.clone(),
            s.account.as_ref().map(|a| a.address.clone()),
            s.networks.clone(),
        )
    };
    let now = io.now().await;
    let blocked = lock(&service.outage).blocked(now);
    if !blocked {
        for account in accounts {
            if followed
                .as_deref()
                .is_some_and(|f| same_address(f, &account.address))
            {
                continue;
            }
            if let Some(mut w) = read_other(io, st, &service, &account, &networks).await {
                // r11-1: read on this page, by a tick that began at `now` —
                // what a round compares with when it began (a kept list has
                // none; a tick begun before the round is not the round's).
                w.read_at = (now.is_finite() && now >= 0.0).then_some(now as u64);
                lock(st)
                    .more
                    .others
                    .insert(account.address.to_ascii_lowercase(), w);
                io.emit(Event::Changed);
            }
        }
    }
    lock(st).more.others_reading = false;
}

/// One multisig not shown: its proposals on every network it has some on,
/// read as the shown one's are; `None`: nothing to say (an older service
/// without the cross-chain list, or a read that failed).
async fn read_other(
    io: &Io<Event>,
    st: &Shared<State>,
    service: &Service,
    account: &MultisigAccount,
    networks: &[u64],
) -> Option<Waiting> {
    let all = service
        .fetch_all_proposals(io, &account.address, true)
        .await
        .ok()??;
    if all.proposals.is_empty() {
        return Some(Waiting::default());
    }
    let (lists, used, _) = split_listed(&all.proposals, &account.chain_ids, networks);
    let mut items = Vec::new();
    for (chain_id, listed) in lists {
        if !used.contains(&chain_id) {
            continue;
        }
        let read = read_chain(
            io,
            st,
            account,
            chain_id,
            None,
            true,
            true,
            Some(listed),
            None,
        )
        .await
        .ok()?;
        items.extend(read.state.items);
    }
    Some(waiting_of(&items, |_| None))
}

/// Keep one executor's look (the home's "Can't run now" reads the same one).
pub fn keep_look(s: &mut State, input: &ExecutionInput, preview: &Preview) {
    let look = match preview {
        Preview::Fails(why) => Look::Fails(why.clone()),
        Preview::Priced { can_pay, .. } => Look::Priced { can_pay: *can_pay },
        _ => Look::Other,
    };
    s.more.looks.insert(
        look_key(
            input.chain_id,
            &input.item.safe_tx_hash,
            &input.executor.address,
        ),
        (input.item.approved, look),
    );
    // r16-4: a press stopped only because its fee could not be worked out or
    // checked (`stopped {unpriced | unchecked}`) says nothing true once a
    // look prices it — the stop goes, the new figure is the line drawn.
    if matches!(preview, Preview::Priced { .. }) {
        let key = format!("execute:{}", input.item.safe_tx_hash.to_ascii_lowercase());
        let unlooked = matches!(
            s.actions.get(&key),
            Some(ActionState::Stopped { preview }) if matches!(
                preview.get("kind").and_then(Value::as_str),
                Some("unpriced" | "unchecked")
            )
        );
        if unlooked {
            s.actions.remove(&key);
        }
    }
}

// ---------------------------------------------------------------------------
// The timeline's facts (`executed.ts`)
// ---------------------------------------------------------------------------

async fn load_executed(io: &Io<Event>) -> BTreeMap<String, DeviceFacts> {
    io.load(&[EXECUTED_KEY])
        .await
        .get(EXECUTED_KEY)
        .cloned()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// What this device executed: the approvals it carried, the threshold then,
/// itself (`rememberExecution`).
pub async fn remember_execution(
    io: &Io<Event>,
    st: &Shared<State>,
    input: Option<&ExecutionInput>,
    outcome: &ActionState,
) {
    let (Some(input), ActionState::Done { .. }) = (input, outcome) else {
        return;
    };
    let executor = input.executor.address.clone();
    // r7-3: what its calls carried (`planExecution`'s `carries`), never
    // every approval the mailbox holds — one it did not carry did not count;
    // and the executor itself only when it approved by sending it.
    // This executor's own look: another's carried other approvals.
    let key = preview_key(
        input.chain_id,
        &input.item.safe_tx_hash,
        &input.executor.address,
    );
    let planned = match lock(st).preview_plans.get(&key) {
        Some((_, Preview::Priced { planned, .. })) => Some(planned.clone()),
        _ => None,
    };
    let (approvals, executor_approves) = match planned {
        Some(p) => (p.carries, p.executor_approves),
        None => {
            let counted: Vec<String> = input
                .item
                .approvals
                .iter()
                .filter(|a| a.counts)
                .map(|a| a.owner.clone())
                .collect();
            let owner = input
                .members
                .owners
                .iter()
                .any(|o| same_address(o, &executor));
            let approved = counted.iter().any(|a| same_address(a, &executor));
            (counted, owner && !approved)
        }
    };
    let now = io.now().await;
    let facts = facts_of_execution(
        &approvals,
        &executor,
        executor_approves,
        input.members.threshold,
        now,
    );
    let mut all = load_executed(io).await;
    all.insert(
        lookup_key(
            input.chain_id,
            &input.account.address,
            &input.item.safe_tx_hash,
        ),
        facts,
    );
    if all.len() > EXECUTED_KEEP {
        let mut by_age: Vec<(String, f64)> = all.iter().map(|(k, v)| (k.clone(), v.at)).collect();
        by_age.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (k, _) in by_age.into_iter().skip(EXECUTED_KEEP) {
            all.remove(&k);
        }
    }
    let _ = st;
    io.store(
        EXECUTED_KEY,
        Some(serde_json::to_string(&all).unwrap_or_default()),
    )
    .await;
}

/// When `block` was made (ms), or `None` when the chain does not say.
pub(super) async fn block_time(
    io: &Io<Event>,
    chain_id: u64,
    block: Option<&Value>,
) -> Option<f64> {
    let block = block?.as_str()?;
    let answer = io
        .rpc(chain_id, "eth_getBlockByNumber", json!([block, false]))
        .await
        .ok()?;
    let time = shell::quantity(answer.get("timestamp")?.as_str()?)?;
    #[allow(clippy::cast_precision_loss)]
    Some(time as f64 * 1000.0)
}

/// The proposal (and its multisig) the followed list or the link holds.
pub(super) fn find_item(
    s: &State,
    chain_id: u64,
    hash: &str,
) -> Option<(ProposalItem, ChainState, MultisigAccount)> {
    if let Some(account) = &s.account {
        if let Some(c) = s.chains.iter().find(|c| c.chain_id == chain_id) {
            if let Some(i) = c
                .items
                .iter()
                .find(|i| i.safe_tx_hash.eq_ignore_ascii_case(hash))
            {
                return Some((i.clone(), c.clone(), account.clone()));
            }
        }
    }
    let l = s.more.linked.as_ref()?;
    let c = l.chain.as_ref()?;
    let i = c
        .items
        .iter()
        .find(|i| i.chain_id == chain_id && i.safe_tx_hash.eq_ignore_ascii_case(hash))?;
    Some((i.clone(), c.clone(), l.preview.clone()?))
}

/// The frozen facts of a done proposal's execution (`frozenFacts`).
async fn read_frozen(io: &Io<Event>, st: &Shared<State>, chain_id: u64, hash: &str) {
    let Some((item, _, account)) = find_item(&lock(st), chain_id, hash) else {
        return;
    };
    if item.status != ProposalStatus::Done {
        return;
    }
    let kept = lock(st)
        .lookups
        .get(&lookup_key(chain_id, &account.address, hash))
        .cloned();
    let tx = match kept.as_deref() {
        Some("none") => return,
        Some(tx) => tx.to_owned(),
        None => match lookup_execution(
            io,
            st,
            &account.address,
            chain_id,
            hash,
            item.created_ms,
            true,
            Some(item.nonce),
        )
        .await
        {
            Ok(Some(tx)) => tx,
            _ => return,
        },
    };
    let mut facts = None;
    if let Ok(receipt) = io
        .rpc(chain_id, "eth_getTransactionReceipt", json!([tx]))
        .await
    {
        let logs: Option<Vec<crate::multisig_reading::ReceiptLog>> = receipt
            .get("logs")
            .and_then(|l| serde_json::from_value(l.clone()).ok());
        if let Some(logs) = logs {
            if let Ok(Some(f)) =
                crate::multisig_reading::execution_facts(chain_id, &account.address, hash, &logs)
            {
                facts = Some(FrozenFacts {
                    executor: Some(f.executor),
                    threshold: f.threshold,
                    approvers: f.approvers,
                    executor_approved: f.executor_approved,
                    source: FactsSource::Chain,
                    at_ms: block_time(io, chain_id, receipt.get("blockNumber")).await,
                });
            }
        }
    }
    if facts.is_none() {
        facts = load_executed(io)
            .await
            .remove(&lookup_key(chain_id, &account.address, hash))
            .map(|d| FrozenFacts {
                executor: d.executor,
                threshold: d.threshold,
                approvers: d.approvers,
                executor_approved: d.executor_approved,
                source: FactsSource::Device,
                at_ms: Some(d.at),
            });
    }
    if let Some(f) = facts {
        lock(st).more.frozen.insert(item_key(chain_id, hash), f);
    }
}

// ---------------------------------------------------------------------------
// A dApp's request (`openMultisigReview`, `followMultisig`)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn dapp_request(
    model: &mut Model,
    id: String,
    account: MultisigAccount,
    method: String,
    params: Value,
    origin: String,
    chain_id: u64,
    limit_ms: Option<f64>,
) -> Cmd {
    let mut held = Held {
        view: HeldRequest {
            id: id.clone(),
            method: method.clone(),
            origin: origin.clone(),
            chain_id,
            safe: account.address.clone(),
            stage: HeldStage::Reviewing,
            calls: Vec::new(),
            safe_tx_hash: None,
            nonce: None,
            tell_at: None,
            told: false,
            refusal: None,
            answer: None,
        },
        params: params.clone(),
        round: 0,
        limit_ms,
        claimed: None,
    };
    let mut s = lock(&model.state);
    // dapp-7: said in the window first; the site hears it once read.
    if let Some(refusal) = dapp::multisig_refusal(&method) {
        held.view.stage = HeldStage::Refused;
        held.view.refusal = Some(refusal);
        s.more.dapp.insert(id, held);
        return render();
    }
    if chain_id == 0 {
        held.view.settle(DappAnswer::err(dapp::no_chain()));
        s.more.dapp.insert(id, held);
        return render();
    }
    let chain = u32::try_from(chain_id).unwrap_or(0);
    let calls = match dapp_rpc::multisig_calls(&method, &params, chain) {
        Ok(calls) => calls,
        Err((code, message)) => {
            held.view.settle(DappAnswer::err(DappError {
                code: i64::from(code),
                message,
                data: None,
            }));
            s.more.dapp.insert(id, held);
            return render();
        }
    };
    held.view.calls.clone_from(&calls);
    // §35 (SEC-PARENT-2): nor make it call itself to change who controls it
    // — the self-call rule the account is held to, held to the multisig's
    // own address. Refused at once; the proposal step checks again.
    if let Some(block) = self_call_of(&calls, &account.address) {
        held.view.stage = HeldStage::Refused;
        held.view.refusal = Some(dapp::self_call_refusal(&block, &account.address));
        s.more.dapp.insert(id, held);
        return render();
    }
    // §31: a site may never make this multisig act for a Safe it owns — one
    // of this device's multisigs, refused at once; any other Safe a call
    // would act for, read (`isOwner`) before anything is drafted.
    let targets = parent_targets_of(&calls, &account.address, chain);
    let known: Vec<String> = s.more.accounts.iter().map(|a| a.address.clone()).collect();
    match parent_guard::verdict(&targets, &known, &[]) {
        ParentVerdict::Refuse { target, reason } => {
            held.view.stage = HeldStage::Refused;
            held.view.refusal = Some(dapp::parent_refusal(&target, reason));
            s.more.dapp.insert(id, held);
            render()
        }
        ParentVerdict::Clear => {
            start_dapp_draft(&mut s, id, held, account);
            drop(s);
            flow(model, move |io, st| async move {
                draft(
                    &io,
                    &st,
                    chain_id,
                    ProposalKind::Calls {
                        calls,
                        origin: Some(origin),
                    },
                )
                .await;
            })
        }
        ParentVerdict::Read { reads } => {
            // Held, unreviewable, while the chain is read: no draft round is
            // its yet, so leaving now is the person's 4001.
            held.round = u64::MAX;
            s.more.dapp.insert(id.clone(), held);
            drop(s);
            flow(model, move |io, st| async move {
                let signer = account.address.clone();
                let mut facts = Vec::new();
                // R22 (SEC-PARENT-3): an unlisted Safe's `isOwner` is read
                // of two independent nodes; one node's "no", or two that
                // differ, is "couldn't check" — refused.
                let guard = io.corroborating();
                for read in reads {
                    let answer = match guard
                        .eth_call(
                            u64::from(read.chain_id),
                            &read.safe,
                            &parent_guard::is_owner_calldata(&signer),
                        )
                        .await
                    {
                        Ok(data) => SafeOwnerAnswer::Returned { data },
                        Err(shell::RpcFail::Error { code, message, .. }) => {
                            SafeOwnerAnswer::RpcError { code, message }
                        }
                        Err(shell::RpcFail::Unanswered(detail)) => {
                            SafeOwnerAnswer::Unreachable { detail }
                        }
                    };
                    facts.push(OwnerFact {
                        chain_id: read.chain_id,
                        safe: read.safe,
                        reading: parent_guard::classify_owner_answer(&answer),
                    });
                }
                let refused = {
                    let mut s = lock(&st);
                    let Some(held) = s.more.dapp.remove(&id) else {
                        return;
                    };
                    if held.view.settled() {
                        s.more.dapp.insert(id, held);
                        return;
                    }
                    let mut held = held;
                    // `enforce`: a Safe still unread (none should be) refuses.
                    match parent_guard::enforce(&targets, &known, &facts) {
                        ParentVerdict::Refuse { target, reason } => {
                            held.view.stage = HeldStage::Refused;
                            held.view.refusal = Some(dapp::parent_refusal(&target, reason));
                            s.more.dapp.insert(id, held);
                            true
                        }
                        _ => {
                            start_dapp_draft(&mut s, id, held, account);
                            false
                        }
                    }
                };
                io.emit(Event::Changed);
                if !refused {
                    draft(
                        &io,
                        &st,
                        chain_id,
                        ProposalKind::Calls {
                            calls,
                            origin: Some(origin),
                        },
                    )
                    .await;
                }
            })
        }
    }
}

/// A site's request becomes the draft under review (its round).
fn start_dapp_draft(s: &mut State, id: String, mut held: Held, account: MultisigAccount) {
    follow_for(s, account);
    s.draft = None;
    s.draft_round += 1;
    s.actions.insert("propose".to_owned(), ActionState::Working);
    held.round = s.draft_round;
    s.more.dapp.insert(id, held);
}

/// §35: the self-call rule (`self_call_guard`) over a site's calls, with the
/// multisig as the account they would run as.
fn self_call_of(
    calls: &[crate::multisig::CallArg],
    multisig: &str,
) -> Option<super::super::self_call_guard::SelfCallBlock> {
    let calls: Vec<Value> = calls
        .iter()
        .map(|c| json!({ "to": c.to, "data": c.data }))
        .collect();
    super::super::self_call_guard::detect_self_call(
        "wallet_sendCalls",
        Some(&json!([{ "calls": calls }])),
        multisig,
    )
}

/// §35: the proposal step's check of a site's draft — the transaction about
/// to be proposed (one call to `to`, or a batch through MultiSendCallOnly),
/// read by the same rule. `Some` refuses: the held request is refused in
/// the arrival's words, and nothing is proposed.
pub fn self_call_at_proposal(
    s: &mut State,
    round: u64,
    multisig: &str,
    tx: &crate::multisig::SafeTx,
) -> Option<super::super::self_call_guard::SelfCallBlock> {
    let block = super::super::self_call_guard::detect_self_call(
        "eth_sendTransaction",
        Some(&json!([{ "to": tx.to, "data": tx.data }])),
        multisig,
    )?;
    if let Some(id) = reviewing(s, round) {
        if let Some(held) = s.more.dapp.get_mut(&id) {
            held.view.stage = HeldStage::Refused;
            held.view.refusal = Some(dapp::self_call_refusal(&block, multisig));
        }
    }
    Some(block)
}

/// §31: the Safes other than `safe` these calls would act for.
fn parent_targets_of(
    calls: &[crate::multisig::CallArg],
    safe: &str,
    chain: u32,
) -> Vec<parent_guard::ParentTarget> {
    let calls: Vec<Value> = calls
        .iter()
        .map(|c| json!({ "to": c.to, "data": c.data }))
        .collect();
    parent_guard::parent_targets(
        "wallet_sendCalls",
        &json!([{ "calls": calls }]),
        safe,
        chain,
    )
}

/// core-ms-2: the draft is a site's request under review.
pub fn reviewing_request(s: &State) -> bool {
    reviewing(s, s.draft_round).is_some()
}

fn reviewing(s: &State, round: u64) -> Option<String> {
    s.more
        .dapp
        .iter()
        .find(|(_, h)| h.round == round && h.view.stage == HeldStage::Reviewing)
        .map(|(k, _)| k.clone())
}

/// RB5: the claim before the passkey — a dApp's request's limit counts from it.
pub async fn claim(io: &Io<Event>, st: &Shared<State>, round: u64) -> Option<f64> {
    let held = reviewing(&lock(st), round).is_some();
    if held {
        Some(io.now().await)
    } else {
        None
    }
}

/// The draft was withdrawn: a dApp's request it was is declined (4001).
pub fn on_withdraw(s: &mut State) {
    if let Some(id) = reviewing(s, s.draft_round) {
        settle_held(s, &id, DappAnswer::err(dapp::declined()));
    }
}

/// A draft was proposed (or handed to an outside wallet): a dApp's request
/// it was is answered (a batch's id), or held for its execution — told
/// "proposed" before its limit. `drafted`: the transaction it was drafted
/// and signed as — its number stands when the list has not read the
/// proposal back yet (a read already on its way queues the next one).
pub async fn on_proposed(
    io: &Io<Event>,
    st: &Shared<State>,
    round: u64,
    outcome: &ActionState,
    claimed: Option<f64>,
    drafted: Option<&SafeTx>,
) {
    let tell = {
        let mut s = lock(st);
        let Some(id) = reviewing(&s, round) else {
            return;
        };
        let hash = match outcome {
            ActionState::OutsideAsked { .. } => {
                if let Some(h) = s.more.dapp.get_mut(&id) {
                    h.claimed = claimed.or(h.claimed);
                }
                return;
            }
            ActionState::Done { safe_tx_hash, .. } => safe_tx_hash.clone(),
            _ => return,
        };
        let chain_id = s.more.dapp[&id].view.chain_id;
        let nonce = s
            .item(chain_id, &hash)
            .map(|(i, _)| i.nonce.to_string())
            .or_else(|| {
                drafted
                    .and_then(|tx| shell::quantity(&tx.nonce))
                    .map(|n| n.to_string())
            });
        let Some(h) = s.more.dapp.get_mut(&id) else {
            return;
        };
        let claimed = claimed.or(h.claimed);
        h.view.safe_tx_hash = Some(hash.clone());
        h.view.nonce = nonce;
        if h.view.method == "wallet_sendCalls" {
            let object = dapp_rpc::send_calls_answers_object(&h.params);
            h.view
                .settle(DappAnswer::ok(dapp_rpc::send_calls_result(&hash, object)));
            None
        } else {
            h.view.stage = HeldStage::Waiting;
            h.view.tell_at = claimed.and_then(|c| dapp::tell_at(c, h.limit_ms));
            h.view.tell_at.map(|at| (id.clone(), at))
        }
    };
    if let Some((id, at)) = tell {
        let state = st.clone();
        io.spawn(move |io| async move {
            let now = io.now().await;
            if at > now {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                io.wait((at - now) as u64).await;
            }
            {
                let mut s = lock(&state);
                if let Some(h) = s.more.dapp.get_mut(&id) {
                    if h.view.stage == HeldStage::Waiting {
                        let hash = h.view.safe_tx_hash.clone().unwrap_or_default();
                        h.view.told = true;
                        h.view.settle(DappAnswer::err(dapp::left_proposed(&hash)));
                    }
                }
            }
            io.emit(Event::Changed);
        });
    }
}

/// After every read: a held `eth_sendTransaction` whose proposal is done is
/// answered — the transaction that executed it, or (its number used by
/// another) the members' cancellation; a link's read-only proposal is read
/// again.
pub async fn after_refresh(io: &Io<Event>, st: &Shared<State>) {
    super::reviews_after_refresh(io, st).await;
    follow_held(io, st).await;
    let preview = lock(st)
        .more
        .linked
        .as_ref()
        .is_some_and(|l| l.preview.is_some());
    if preview {
        read_linked(io, st).await;
    }
}

async fn follow_held(io: &Io<Event>, st: &Shared<State>) {
    let (account, waiting) = {
        let s = lock(st);
        let Some(account) = s.account.clone() else {
            return;
        };
        let waiting: Vec<(String, ProposalItem)> = s
            .more
            .dapp
            .values()
            .filter(|h| {
                h.view.stage == HeldStage::Waiting && same_address(&h.view.safe, &account.address)
            })
            .filter_map(|h| {
                let (item, _) = s.item(h.view.chain_id, h.view.safe_tx_hash.as_deref()?)?;
                (item.status == ProposalStatus::Done).then(|| (h.view.id.clone(), item))
            })
            .collect();
        (account, waiting)
    };
    for (id, item) in waiting {
        let found = lookup_execution(
            io,
            st,
            &account.address,
            item.chain_id,
            &item.safe_tx_hash,
            item.created_ms,
            true,
            Some(item.nonce),
        )
        .await;
        let answer = match found {
            Ok(Some(tx)) => DappAnswer::ok(Value::String(tx)),
            Ok(None) => DappAnswer::err(dapp::cancelled_by_members()),
            // The chain could not say: asked again, never read as cancelled.
            Err(_) => continue,
        };
        settle_held(&mut lock(st), &id, answer);
    }
}

/// `wallet_getCallsStatus` from the proposal's state (`multisig_calls_status`).
async fn calls_status(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    calls_id: &str,
) -> DappAnswer {
    let chain = u32::try_from(chain_id).unwrap_or(0);
    let status = |receipt: Option<&Value>, used: bool| {
        DappAnswer::ok(dapp_rpc::multisig_calls_status(
            calls_id, chain, receipt, used,
        ))
    };
    let (account, found, read) = {
        let s = lock(st);
        (
            s.account.clone(),
            s.item(chain_id, calls_id).map(|(i, _)| i),
            s.chains
                .iter()
                .find(|c| c.chain_id == chain_id)
                .is_some_and(|c| c.mailbox),
        )
    };
    let unknown = || {
        DappAnswer::err(DappError {
            code: 5730,
            message: "Unknown bundle id".to_owned(),
            data: None,
        })
    };
    let Some(account) = account else {
        return unknown();
    };
    let Some(item) = found else {
        // Only a list read now says it is not there.
        return if read { unknown() } else { status(None, false) };
    };
    if item.status != ProposalStatus::Done {
        return status(None, false);
    }
    match lookup_execution(
        io,
        st,
        &account.address,
        chain_id,
        &item.safe_tx_hash,
        item.created_ms,
        true,
        Some(item.nonce),
    )
    .await
    {
        Ok(Some(tx)) => {
            let receipt = io
                .rpc(chain_id, "eth_getTransactionReceipt", json!([tx]))
                .await
                .ok();
            status(receipt.as_ref(), false)
        }
        Ok(None) => status(None, true),
        Err(_) => status(None, false),
    }
}

// ---------------------------------------------------------------------------
// A link's read-only proposal (`LinkedProposals.refresh`)
// ---------------------------------------------------------------------------

async fn read_linked(io: &Io<Event>, st: &Shared<State>) {
    let Some((account, chain_id, hash, list)) = lock(st).more.linked.as_ref().and_then(|l| {
        Some((
            l.preview.clone()?,
            l.chain_id,
            l.hash.clone(),
            l.list.clone(),
        ))
    }) else {
        return;
    };
    let read = read_chain(io, st, &account, chain_id, list, false, true, None, None).await;
    let found = {
        let mut s = lock(st);
        let Some(l) = s.more.linked.as_mut().filter(|l| l.hash == hash) else {
            return;
        };
        l.loading = false;
        match read {
            Ok(r) => {
                if r.state.mailbox {
                    l.list = Some(r.proposals);
                }
                l.stale = !r.state.mailbox;
                l.unreadable = false;
                let item = r
                    .state
                    .items
                    .iter()
                    .find(|i| i.safe_tx_hash.eq_ignore_ascii_case(&hash))
                    .cloned();
                let members = r.state.members.clone();
                l.chain = Some(r.state);
                item.map(|i| (i, members))
            }
            Err(_) => {
                // What was read stays, said as stale — and the network named.
                l.stale = true;
                l.unreadable = true;
                None
            }
        }
    };
    io.emit(Event::Changed);
    let Some((item, members)) = found else {
        return;
    };
    let key = format!("{chain_id}:{}", item.safe_tx_hash);
    // §12: `loading` until read, bounded; read once (a sheet's `review` and
    // `review_again` read it again).
    let run = {
        let mut s = lock(st);
        if s.reviews.contains_key(&key) || s.reviewing.contains_key(&key) {
            return;
        }
        s.review_states.insert(key.clone(), ReviewState::Loading);
        start_review_run(&mut s, &key)
    };
    io.emit(Event::Changed);
    let gathered = review_state::bounded(io, async {
        let gathered = gather_review(io, st, &account, chain_id, &item.tx, &members, None).await;
        super::settle(&gathered, false)
    })
    .await
    .and_then(|landed| landed)
    .map(Some);
    keep_review(&mut lock(st), &key, run, gathered);
}

// ---------------------------------------------------------------------------
// An outside member's wallet (`approveExternally`, `plainAccounts`)
// ---------------------------------------------------------------------------

/// Of `owners`, those an outside wallet may be asked to sign for on
/// `chain_id` (r4C-10): not this device's, no code at their address there,
/// not a Vela account the passkey index knows, none that approved here with
/// a passkey, none kept here as a Vela account or a contract.
async fn plain_owners(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    owners: &[String],
    items: &[ProposalItem],
) -> Vec<String> {
    let (locals, indexed) = {
        let s = lock(st);
        (s.locals.clone(), s.more.indexed.clone())
    };
    let mut facts: BTreeMap<String, SignerFacts> = BTreeMap::new();
    for owner in owners {
        let local = locals.iter().any(|a| same_address(&a.address, owner));
        let mut approved_with = None;
        for row in items.iter().flat_map(|i| i.approvals.iter()) {
            if !row.counts || !same_address(&row.owner, owner) {
                continue;
            }
            match row.approval {
                Approval::Passkey(_) => approved_with = Some(ApprovedWith::Passkey),
                Approval::Ecdsa(_) if approved_with.is_none() => {
                    approved_with = Some(ApprovedWith::Wallet);
                }
                _ => {}
            }
        }
        let kept = account
            .members
            .as_ref()
            .and_then(|m| m.get(&owner.to_ascii_lowercase()))
            .and_then(|m| m.kind.clone());
        let skip = local
            || approved_with == Some(ApprovedWith::Passkey)
            || matches!(kept.as_deref(), Some("passkey" | "contract"));
        let code = if kept.as_deref() == Some("contract") {
            Some(true)
        } else if skip {
            None
        } else {
            // A read that failed is no code (`hasContractCode(...).catch(() => false)`).
            Some(
                Chain::has_contract_code(io, chain_id, owner)
                    .await
                    .unwrap_or(false),
            )
        };
        facts.insert(
            owner.to_ascii_lowercase(),
            SignerFacts {
                local,
                approved_with,
                indexed: indexed.iter().any(|a| same_address(a, owner)),
                index_answered: true,
                kept,
                code,
                configured: None,
            },
        );
    }
    plain_accounts(owners, |o| {
        facts
            .get(&o.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    })
}

/// Hand `tx` to the outside session: the gate first, then the notice.
#[allow(clippy::too_many_arguments)]
async fn ask_outside(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain_id: u64,
    tx: &SafeTx,
    owners: Vec<String>,
    action: &str,
    round: Option<u64>,
) -> ActionState {
    if owners.is_empty() {
        return ActionState::NoOutsideSigner;
    }
    if let Err(Stop::State(s)) = authorize_signing(io, st, account, chain_id, tx).await {
        return s;
    }
    if round.is_some_and(|r| lock(st).draft_round != r) {
        return ActionState::Cancelled;
    }
    let hash = multisig::safe_tx_hash(chain_id, &account.address, tx).unwrap_or_default();
    lock(st).more.outside.insert(
        item_key(chain_id, &hash),
        OutsidePending {
            account: account.clone(),
            tx: tx.clone(),
            owners: owners.clone(),
            action: action.to_owned(),
            round,
        },
    );
    io.notice(Notice::OutsideSign {
        request: Box::new(SessionRequest {
            chain_id,
            safe: account.address.clone(),
            tx: tx.clone(),
            safe_tx_hash: hash.clone(),
            owners: owners.clone(),
            send: None,
            genesis: Some(account.genesis.clone()),
        }),
    })
    .await;
    ActionState::OutsideAsked {
        safe_tx_hash: hash,
        owners,
    }
}

/// "Propose and approve" with no member here: the reviewed draft goes to an
/// outside member's wallet (only a plain account's can sign — r4C-10).
pub async fn propose_outside(
    io: &Io<Event>,
    st: &Shared<State>,
    account: &MultisigAccount,
    chain: &ChainState,
    tx: &SafeTx,
    round: u64,
) -> ActionState {
    let owners = plain_owners(
        io,
        st,
        account,
        chain.chain_id,
        &chain.members.owners,
        &chain.items,
    )
    .await;
    ask_outside(
        io,
        st,
        account,
        chain.chain_id,
        tx,
        owners,
        "propose",
        Some(round),
    )
    .await
}

/// An open proposal approved from an outside member's wallet.
async fn approve_outside(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    action: &str,
) -> ActionState {
    let Some((item, chain, account)) = find_item(&lock(st), chain_id, hash) else {
        return ActionState::Failed {
            detail: crate::app::multisig_failure::core_reason("nothing_to_approve"),
        };
    };
    if item.status == ProposalStatus::Done || item.review.action.is_none() {
        return ActionState::Failed {
            detail: crate::app::multisig_failure::core_reason("nothing_to_approve"),
        };
    }
    let owners: Vec<String> = plain_owners(
        io,
        st,
        &account,
        chain_id,
        &chain.members.owners,
        &chain.items,
    )
    .await
    .into_iter()
    .filter(|o| {
        !item
            .approvals
            .iter()
            .any(|a| a.counts && same_address(&a.owner, o))
    })
    .collect();
    ask_outside(io, st, &account, chain_id, &item.tx, owners, action, None).await
}

/// The outside session answered: an approval is verified here — the owner's
/// for this safeTxHash, one of those asked — and submitted through the
/// outbox; anything else is said.
async fn outside_answered(
    io: &Io<Event>,
    st: &Shared<State>,
    chain_id: u64,
    hash: &str,
    outcome: SessionOutcome,
) {
    let Some(p) = lock(st).more.outside.remove(&item_key(chain_id, hash)) else {
        return;
    };
    let state = match outcome {
        SessionOutcome::Cancelled => ActionState::Cancelled,
        SessionOutcome::Sent { .. } => ActionState::Failed {
            detail: crate::app::multisig_failure::core_reason("wallet_sent_transaction"),
        },
        SessionOutcome::Approved { approval } => {
            let verified = multisig::verify_ecdsa_approval(hash, &approval)
                .ok()
                .filter(|v| {
                    same_address(&v.owner, &approval.owner)
                        && p.owners.iter().any(|o| same_address(o, &v.owner))
                });
            match verified {
                None => ActionState::Failed {
                    detail: crate::app::multisig_failure::passkey_check(),
                },
                Some(_) if p.round.is_some_and(|r| lock(st).draft_round != r) => {
                    // Withdrawn while the wallet was asked: what it signed goes nowhere.
                    ActionState::Cancelled
                }
                Some(v) => submit(io, st, &p.account, chain_id, &p.tx, Approval::Ecdsa(v)).await,
            }
        }
    };
    {
        let mut s = lock(st);
        let current = p.round.is_none_or(|r| s.draft_round == r);
        if current {
            if p.round.is_some() && matches!(state, ActionState::Done { .. }) {
                s.draft = None;
            }
            s.actions.insert(p.action.clone(), state.clone());
        }
    }
    if matches!(state, ActionState::Done { .. }) {
        refresh(io, st, false, true).await;
        if let Some(round) = p.round {
            on_proposed(io, st, round, &state, None, Some(&p.tx)).await;
        }
    }
}
