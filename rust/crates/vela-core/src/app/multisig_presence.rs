//! What waits on this device per multisig (presence, r4D-7, r5reg-6), where
//! each waiting transaction stands (its one word, r4D-13), what the waiting
//! group says of the list (r4b-3, r4b-11, r4b-15, r4b-2), and a proposal's
//! story (its timeline, r4D-17) — the rules of the web's
//! `presence.svelte.ts`, `preflight.svelte.ts` (`blockOf`),
//! `activity.svelte.ts` (`standing`, the group's note), `executed.ts`
//! (`factsOfExecution`) and `timeline.ts`, as plain functions the proposals
//! machine draws its view with.
//!
//! Nothing here reads anything: a count is what the machine read, a time is
//! one the mailbox or a receipt holds, and a fact nobody holds is `None` —
//! never a zero it did not see, never a time made up.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_proposals::{ProposalItem, ProposalStatus};
use super::multisig_shell::same_address;

/// How often the multisigs not shown are read again while a page is up: the
/// shell sends `OthersTick` this often while the page is visible.
pub const OTHERS_EVERY_MS: f64 = 60_000.0;

// ---------------------------------------------------------------------------
// Presence
// ---------------------------------------------------------------------------

/// The transaction to open first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingFirst"))]
pub struct WaitingFirst {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
}

/// What waits on this device for one multisig (`Waiting`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaiting"))]
pub struct Waiting {
    /// Open, short of approvals, and a member on this device has not approved.
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub needs_you: u64,
    /// Enough approvals, its nonce next, and a member here can execute it now
    /// (not one nobody here can pay for, nor one that would fail).
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub ready: u64,
    /// Enough approvals, its nonce next, but no member here can pay its fee
    /// (or none is here): it waits for a member who can.
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub waiting_payer: u64,
    /// The one needing an approval first, else the first ready.
    pub first: Option<WaitingFirst>,
}

impl Waiting {
    /// Something here is this device's to do (the switcher's dot).
    pub fn any(&self) -> bool {
        self.needs_you + self.ready > 0
    }

    /// core-ms-2: what is said of it, in order ("1 needs your approval ·
    /// 1 ready to execute · 1 waiting for a member who can pay"): only the
    /// counts that are not zero.
    pub fn parts(&self) -> Vec<WaitingPart> {
        [
            (WaitingKind::NeedsYou, self.needs_you),
            (WaitingKind::Ready, self.ready),
            (WaitingKind::WaitingPayer, self.waiting_payer),
        ]
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(kind, count)| WaitingPart { kind, count })
        .collect()
    }
}

/// core-ms-2: which count a part of the waiting line says.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingKind"))]
pub enum WaitingKind {
    /// `presence.needsYou`: short of approvals, a member here has not approved.
    NeedsYou,
    /// `presence.ready`: a member here can execute it now.
    Ready,
    /// `presence.waitingPayer`: ready, but no member here can pay its fee.
    WaitingPayer,
}

/// core-ms-2: one part of the waiting line, its count (a plural's).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingPart"))]
pub struct WaitingPart {
    pub kind: WaitingKind,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub count: u64,
}

/// core-ms-2: the multisig shown's line at the top of its home — "1 needs
/// your approval · 1 ready to execute ›" — said only when something waits
/// and there is one to open.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingBanner"))]
pub struct WaitingBanner {
    /// The words, in order: never empty.
    pub parts: Vec<WaitingPart>,
    /// The one the line opens.
    pub first: WaitingFirst,
}

/// core-ms-2: the banner for `w`, when there is one to show.
pub fn banner_of(w: &Waiting) -> Option<WaitingBanner> {
    let parts = w.parts();
    if parts.is_empty() {
        return None;
    }
    Some(WaitingBanner {
        parts,
        first: w.first.clone()?,
    })
}

/// Why a ready transaction cannot run here now (`blockOf`'s word).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigReadyBlock"))]
pub enum ReadyBlock {
    /// A look says it would fail (the transaction's own fault). `why` is that
    /// look's own detail, as the preview says it — `{kind: "short", symbol,
    /// decimals, have, need}` or `{kind: "reverts", reason}` — the words
    /// after "Can't run now".
    Fails {
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        why: serde_json::Value,
    },
    /// Every executor here was priced and none holds the fee.
    NobodyPays,
}

/// One executor's look at an execution, as `blockOf` reads it.
#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    /// It would fail: the preview's detail (`ReadyBlock::Fails::why`).
    Fails(serde_json::Value),
    Priced {
        can_pay: bool,
    },
    /// Unpriced, unchecked, refused, spent: says nothing of the fee.
    Other,
}

/// Why a transaction cannot run now, from the looks at its executors here
/// (`blockOf`): any look says it fails, or every one was priced and none can
/// pay. `None` while any look is missing or unpriced, or someone can run it.
pub fn block_of(looks: &[Option<Look>]) -> Option<ReadyBlock> {
    if let Some(why) = looks.iter().find_map(|l| match l {
        Some(Look::Fails(why)) => Some(why.clone()),
        _ => None,
    }) {
        return Some(ReadyBlock::Fails { why });
    }
    if looks.is_empty() {
        return None;
    }
    looks
        .iter()
        .all(|l| *l == Some(Look::Priced { can_pay: false }))
        .then_some(ReadyBlock::NobodyPays)
}

/// What `items` (one multisig's, as this device reads them) ask of this
/// device (`waitingOf`). `blocked_of`: why a ready one cannot run here;
/// `None` for every one, a ready one with a member here counts as ready.
pub fn waiting_of(
    items: &[ProposalItem],
    blocked_of: impl Fn(&ProposalItem) -> Option<ReadyBlock>,
) -> Waiting {
    let open: Vec<&ProposalItem> = items
        .iter()
        .filter(|i| i.status != ProposalStatus::Done && i.cancelled_by.is_none())
        .collect();
    let needs: Vec<&&ProposalItem> = open
        .iter()
        .filter(|i| {
            i.status != ProposalStatus::Ready
                && i.approved < i.threshold
                && !i.can_approve.is_empty()
        })
        .collect();
    let ready_all: Vec<&&ProposalItem> = open
        .iter()
        .filter(|i| i.status == ProposalStatus::Ready)
        .collect();
    // r5reg-6: ready only where someone here can run it; a would-fail one is
    // its row's to say, counted nowhere here.
    let ready: Vec<&&&ProposalItem> = ready_all
        .iter()
        .filter(|i| !i.can_execute.is_empty() && blocked_of(i).is_none())
        .collect();
    let waiting_payer = ready_all
        .iter()
        .filter(|i| i.can_execute.is_empty() || blocked_of(i) == Some(ReadyBlock::NobodyPays))
        .count();
    let first = needs
        .first()
        .map(|i| (**i).clone())
        .or_else(|| ready.first().map(|i| (***i).clone()));
    Waiting {
        needs_you: needs.len() as u64,
        ready: ready.len() as u64,
        waiting_payer: waiting_payer as u64,
        first: first.map(|i| WaitingFirst {
            chain_id: i.chain_id,
            safe_tx_hash: i.safe_tx_hash.clone(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Where one waiting transaction stands (its row's one word)
// ---------------------------------------------------------------------------

/// The one word a waiting row says about where it stands (`standing`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigStanding"))]
pub enum Standing {
    /// Signed here, not in the mailbox yet.
    Unsent,
    /// Sent from here and may still land: not offered again (r4b-7).
    Executing,
    /// Behind an earlier number.
    Queued,
    /// Every approval is in, and it can run.
    Ready,
    /// Every approval is in, but it cannot run here now (r4-5).
    Blocked { block: ReadyBlock },
    /// A member here has not approved: the one thing to do.
    YourTurn,
    /// Waiting for others: `approved` of `threshold`, never more than needed
    /// (r4b-17: never "3/2").
    Progress {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        approved: u64,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold: u64,
    },
}

/// Where `item` stands; `block`: why it cannot run here, when it is ready.
pub fn standing_of(item: &ProposalItem, block: Option<ReadyBlock>) -> Standing {
    if item.unsent {
        return Standing::Unsent;
    }
    if item.executing {
        return Standing::Executing;
    }
    match item.status {
        ProposalStatus::Queued => return Standing::Queued,
        ProposalStatus::Ready => {
            return block.map_or(Standing::Ready, |block| Standing::Blocked { block })
        }
        _ => {}
    }
    if !item.can_approve.is_empty() {
        return Standing::YourTurn;
    }
    Standing::Progress {
        approved: item.approved.min(item.threshold),
        threshold: item.threshold,
    }
}

// ---------------------------------------------------------------------------
// What the waiting group says of the list
// ---------------------------------------------------------------------------

/// What the waiting group says of the list it shows, first match wins
/// (`group`'s note): offline, the service away, a network that did not
/// answer, the list kept from an earlier page, stale, the history unread,
/// still reading.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigWaitingNote"))]
pub enum WaitingNote {
    /// r4b-15: offline — since when the list shown was read, if it was.
    Offline {
        since: Option<f64>,
    },
    /// The service is away: said once, with a Retry.
    Unreachable,
    /// r4b-3: these networks did not answer — named, with a Retry, never
    /// "nothing waits".
    ChainUnreachable {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
    },
    /// r4b-11: the list an earlier page kept, said as the last known.
    LastKnown {
        at: f64,
    },
    Stale,
    /// r4b-2: the history could not be read there, with a Retry.
    HistoryUnreadable {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
    },
    /// Still reading with nothing to show: a cue from the first frame.
    Checking,
}

impl WaitingNote {
    /// The note comes with a Retry.
    pub fn retry(&self) -> bool {
        matches!(
            self,
            Self::Unreachable | Self::ChainUnreachable { .. } | Self::HistoryUnreadable { .. }
        )
    }
}

/// What the machine knows of its list, for [`waiting_note`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListFacts {
    pub offline: bool,
    pub read_at: Option<f64>,
    pub unreachable: bool,
    pub unreadable: Vec<u64>,
    pub cached_at: Option<f64>,
    pub stale: bool,
    pub history_unreadable: Vec<u64>,
    pub loading: bool,
    /// The group shows no row (no open transaction, no network missing):
    /// only then is "checking" said — over rows, the rows speak.
    pub nothing_shown: bool,
}

/// The waiting group's note; `None`: nothing to say of the list.
pub fn waiting_note(f: &ListFacts) -> Option<WaitingNote> {
    if f.offline {
        return Some(WaitingNote::Offline { since: f.read_at });
    }
    if f.unreachable {
        return Some(WaitingNote::Unreachable);
    }
    if !f.unreadable.is_empty() {
        return Some(WaitingNote::ChainUnreachable {
            chain_ids: f.unreadable.clone(),
        });
    }
    if let Some(at) = f.cached_at {
        return Some(WaitingNote::LastKnown { at });
    }
    if f.stale {
        return Some(WaitingNote::Stale);
    }
    if !f.history_unreadable.is_empty() {
        return Some(WaitingNote::HistoryUnreadable {
            chain_ids: f.history_unreadable.clone(),
        });
    }
    (f.loading && f.nothing_shown).then_some(WaitingNote::Checking)
}

// ---------------------------------------------------------------------------
// An executed transaction, as it was when it ran (`executed.ts`)
// ---------------------------------------------------------------------------

/// Where a transaction's frozen facts were read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigFactsSource"))]
pub enum FactsSource {
    /// Its receipt (Safe L2's `SafeMultiSigTransaction`).
    Chain,
    /// What this device recorded when it executed it.
    Device,
}

/// An executed multisig transaction as it was when it ran (`FrozenFacts`):
/// never re-counted against today's owners.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigFrozenFacts"))]
pub struct FrozenFacts {
    /// Who sent it; `None` when the record does not say.
    pub executor: Option<String>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    /// The members whose approvals counted.
    pub approvers: Vec<String>,
    /// The executor approved by sending it (it is among `approvers`).
    pub executor_approved: bool,
    pub source: FactsSource,
    /// When it ran (Unix ms): its block's time, else when this device saw it
    /// land; `None` when neither says.
    pub at_ms: Option<f64>,
}

/// What this device keeps of an execution it sent (`vela.multisig.executed`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceFacts {
    pub executor: Option<String>,
    pub threshold: u64,
    pub approvers: Vec<String>,
    pub executor_approved: bool,
    #[serde(serialize_with = "super::multisig_shell::int_ms")]
    pub at: f64,
}

/// The store key of what this device executed (the web's own).
pub const EXECUTED_KEY: &str = "vela.multisig.executed";
/// How many executions this device keeps the facts of.
pub const EXECUTED_KEEP: usize = 200;

/// The facts an execution from this device leaves, as Safe counts them
/// (`factsOfExecution`): the approvals it carries and, when it approves by
/// sending it, the executor — in ascending address order, each once, the
/// first `threshold` of them.
pub fn facts_of_execution(
    approvals: &[String],
    executor: &str,
    executor_approves: bool,
    threshold: u64,
    at: f64,
) -> DeviceFacts {
    let mut all: Vec<String> = approvals.to_vec();
    if executor_approves {
        all.push(executor.to_owned());
    }
    let mut unique: Vec<String> = Vec::new();
    for a in all {
        if !unique.iter().any(|u| same_address(u, &a)) {
            unique.push(a);
        }
    }
    unique.sort_by_key(|a| a.to_ascii_lowercase());
    let approvers: Vec<String> = unique
        .into_iter()
        .take(usize::try_from(threshold).unwrap_or(usize::MAX))
        .collect();
    DeviceFacts {
        executor: Some(executor.to_owned()),
        threshold,
        executor_approved: executor_approves && approvers.iter().any(|a| same_address(a, executor)),
        approvers,
        at,
    }
}

// ---------------------------------------------------------------------------
// The timeline (`timeline.ts`)
// ---------------------------------------------------------------------------

/// One step of a proposal's story.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigTimelineStep"))]
pub enum TimelineStep {
    /// Proposed by `who` (the mailbox's proposer).
    Created { who: String, at_ms: Option<f64> },
    /// `who` approved.
    Approved {
        who: String,
        at_ms: Option<f64>,
        /// Executed: counted when it ran (the receipt's signers, or what this
        /// device sent; `None` while that is not read). Open: whether it
        /// counts now.
        counted: Option<bool>,
        /// The executor approved by sending it: no approval of its own.
        by_executing: bool,
        /// Signed here and kept, not in the mailbox yet.
        unsent: bool,
        /// Given on chain (`approveHash`).
        on_chain: bool,
    },
    /// Sent from this device; it may still land.
    Executing,
    /// A cancellation waits at its number (ms-9).
    CancelProposed { by: String },
    /// Executed by `tx_hash`.
    Executed {
        who: Option<String>,
        tx_hash: String,
        at_ms: Option<f64>,
    },
    /// Its number was used by another transaction (`by`, when it is known
    /// here — `cancellation`: the cancellation proposed at its number).
    Replaced {
        by: Option<String>,
        cancellation: bool,
    },
}

/// How a proposal ended, as far as this device knows.
#[derive(Clone, Debug, PartialEq)]
pub enum TimelineEnd {
    /// Still open (or its end not read yet).
    Open,
    /// Executed by this transaction, with its frozen facts when read.
    Executed {
        tx_hash: String,
        frozen: Option<FrozenFacts>,
    },
    /// Its number was used by another.
    Replaced {
        by: Option<String>,
        cancellation: bool,
    },
}

fn known(ms: f64) -> Option<f64> {
    (ms > 0.0).then_some(ms)
}

/// The story of `item` (`timelineOf`): created → each approval in the order
/// given → how it ends. An approval the mailbox holds that is not a member's
/// is no step of an executed one's story unless it counted when it ran.
pub fn timeline_of(item: &ProposalItem, end: &TimelineEnd) -> Vec<TimelineStep> {
    let mut steps = vec![TimelineStep::Created {
        who: item.proposer.clone(),
        at_ms: known(item.created_ms),
    }];
    let frozen = match end {
        TimelineEnd::Executed { frozen, .. } => frozen.as_ref(),
        _ => None,
    };
    let executed = matches!(end, TimelineEnd::Executed { .. });
    let counted_then =
        |owner: &str| frozen.is_some_and(|f| f.approvers.iter().any(|o| same_address(o, owner)));
    let mut given: Vec<&super::multisig_proposals::ApprovalRow> = item
        .approvals
        .iter()
        .filter(|a| a.counts || counted_then(&a.owner) || (!executed && a.unsent))
        .collect();
    given.sort_by(|a, b| {
        known(a.created_ms)
            .unwrap_or(f64::INFINITY)
            .total_cmp(&known(b.created_ms).unwrap_or(f64::INFINITY))
    });
    for a in &given {
        steps.push(TimelineStep::Approved {
            who: a.owner.clone(),
            at_ms: known(a.created_ms),
            counted: if executed {
                frozen.map(|_| counted_then(&a.owner))
            } else {
                Some(a.counts)
            },
            by_executing: false,
            unsent: !executed && a.unsent,
            on_chain: matches!(a.approval, crate::multisig::Approval::OnChain(_)),
        });
    }
    match end {
        TimelineEnd::Open => {
            if item.executing {
                steps.push(TimelineStep::Executing);
            }
            if let Some(by) = &item.cancelled_by {
                steps.push(TimelineStep::CancelProposed { by: by.clone() });
            }
        }
        TimelineEnd::Executed { tx_hash, frozen } => {
            // Signers the receipt shows that the mailbox held no approval of:
            // the executor, by sending it (or an approval given on chain).
            for owner in frozen.iter().flat_map(|f| f.approvers.iter()) {
                if given.iter().any(|a| same_address(&a.owner, owner)) {
                    continue;
                }
                let f = frozen.as_ref();
                let executor = f
                    .and_then(|f| f.executor.as_deref())
                    .is_some_and(|e| same_address(e, owner));
                steps.push(TimelineStep::Approved {
                    who: owner.clone(),
                    at_ms: if executor {
                        f.and_then(|f| f.at_ms)
                    } else {
                        None
                    },
                    counted: Some(true),
                    by_executing: executor && f.is_some_and(|f| f.executor_approved),
                    unsent: false,
                    on_chain: false,
                });
            }
            steps.push(TimelineStep::Executed {
                who: frozen.as_ref().and_then(|f| f.executor.clone()),
                tx_hash: tx_hash.clone(),
                at_ms: frozen.as_ref().and_then(|f| f.at_ms),
            });
        }
        TimelineEnd::Replaced { by, cancellation } => steps.push(TimelineStep::Replaced {
            by: by.clone(),
            cancellation: *cancellation,
        }),
    }
    steps
}
