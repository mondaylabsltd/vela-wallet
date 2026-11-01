//! Multisig notifications (AUDIT §6.3, M6): what moved on the multisigs on
//! this device since the last look, said once — "Needs your approval",
//! "Approval counted: 2 of 3", "Ready to execute", "Executed".
//!
//! **Off by default** (`vela.desktop.multisigNotifications`, Settings →
//! Appearance): the web has no equivalent, so nothing is posted unless the
//! person turns it on. There is no push service — the host's own polls (the
//! accounts' and the others' every 60 s, the followed multisig's at its
//! pace) move the machines' views, and every move is compared here with the
//! last one.
//!
//! Nothing here decides anything about a multisig. Every fact is a view's:
//! "needs you" and "ready" are the core's own `waiting_of` (per item for the
//! followed multisig, its presence counts for the others), "approved" is the
//! item's count, "executed" is the execution verdict or the timeline the
//! proposals machine keeps. This only notices that a fact rose.
//!
//! The rules, each conservative — only what the view proves:
//!
//! - **Baseline.** The first look at anything is silent: the first snapshot
//!   after launch (or after the switch is turned on), a multisig first seen,
//!   a followed multisig just switched to, a network whose list was not read
//!   before. The others' presence is compared only once it is `live` — the
//!   host says so a full poll after its first round, so the list an earlier
//!   session kept never counts as news.
//! - **Once.** Each `(multisig, safeTxHash, kind)` is said at most once a
//!   session; everything seen in a state (said or not) is remembered, so a
//!   multisig switched away from — whose presence falls back to an older
//!   read — never repeats what was already known.
//! - **Not the person's own move.** A proposal proposed here, an approval
//!   given by a member on this device (it was in the item's `can_approve`),
//!   and the "ready" that approval caused are not news to the person who did
//!   them.
//! - **Executed** is said only for an item seen open this session, now done,
//!   with a verdict or a timeline step that says it executed — never because
//!   an item merely left a list (it may have been replaced).
//!
//! [`transitions`] is the whole rule: two snapshots and the memory in, the
//! notices out. [`deliver`] words them and posts them through the platform.

use std::collections::{BTreeMap, BTreeSet};

use vela_core::app::multisig_accounts::MultisigAccountsView;
use vela_core::app::multisig_presence::{TimelineStep, Waiting, waiting_of};
use vela_core::app::multisig_proposals::{
    ActionState, ProposalItem, ProposalStatus, ProposalsView,
};
use vela_core::multisig::Action;

pub mod attention;
pub mod deliver;
#[cfg(target_os = "macos")]
mod macos;

/// What happened, as one notification says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A member on this device can approve it, and it is short of approvals.
    ApprovalNeeded,
    /// Someone else's approval counted: `approved` of `threshold` now.
    ApprovalCounted { approved: u64, threshold: u64 },
    /// Enough approvals, its number next.
    ReadyToExecute,
    /// It ran.
    Executed,
}

/// What a transaction is, when the view says (the followed multisig's
/// items carry their review; the others' presence only names one).
#[derive(Clone, Debug, PartialEq)]
pub struct What {
    pub action: Option<Action>,
    pub nonce: u128,
}

/// One notification.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    /// The multisig's address, as its row has it.
    pub multisig: String,
    /// Its name (the record's, else the person's label), if it has one.
    pub name: Option<String>,
    pub kind: Kind,
    pub chain_id: u64,
    pub safe_tx_hash: String,
    pub what: Option<What>,
}

/// One proposal of the followed multisig, as far as notifications care.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ItemSnap {
    pub chain_id: u64,
    pub safe_tx_hash: String,
    pub nonce: u128,
    pub action: Option<Action>,
    pub approved: u64,
    pub threshold: u64,
    /// Whose approvals count now (lowercase).
    pub approvers: BTreeSet<String>,
    /// This device's members that have not approved (lowercase).
    pub can_approve: BTreeSet<String>,
    /// The core's `waiting_of`: it needs an approval a member here can give.
    pub needs_you: bool,
    /// Enough approvals, its number next, not voided.
    pub ready: bool,
    /// Not done (its number unused).
    pub open: bool,
    /// Done, and the verdict or the timeline says it executed.
    pub executed: bool,
    /// Signed here, not in the mailbox yet.
    pub unsent: bool,
}

/// The followed multisig's list, once it was read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Followed {
    /// Lowercase.
    pub address: String,
    /// Per `<chainId>:<safeTxHash lowercase>`.
    pub items: BTreeMap<String, ItemSnap>,
    /// The networks whose list was read (not unreadable).
    pub read_chains: BTreeSet<u64>,
    /// Proposed from this device this session (lowercase hashes).
    pub proposed_here: BTreeSet<String>,
}

/// What the views say now, as far as notifications care.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    /// Every multisig on this device (lowercase) → its row's address and
    /// name.
    pub multisigs: BTreeMap<String, (String, Option<String>)>,
    /// What waits on each multisig other than the followed one, once read.
    pub presence: BTreeMap<String, Waiting>,
    /// The others' presence is a live read, not what an earlier session
    /// kept (the host's word: a full poll since its first round).
    pub presence_live: bool,
    /// The followed multisig's list, once read.
    pub followed: Option<Followed>,
}

/// What has been seen this session: never said twice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    /// `(multisig, safeTxHash, kind)`, lowercase: seen in that state.
    known: BTreeSet<(String, String, Kind)>,
    /// `(multisig, safeTxHash)`, lowercase: seen open.
    open: BTreeSet<(String, String)>,
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

fn item_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{}", lower(hash))
}

/// `executions` / `timelines` keyed `<chainId>:<safeTxHash>`, in whichever
/// case the machine wrote it.
fn keyed<'a, V>(map: &'a BTreeMap<String, V>, chain_id: u64, hash: &str) -> Option<&'a V> {
    map.get(&item_key(chain_id, hash))
        .or_else(|| map.get(&format!("{chain_id}:{hash}")))
        .or_else(|| {
            let want = item_key(chain_id, hash);
            map.iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&want))
                .map(|(_, v)| v)
        })
}

impl ItemSnap {
    /// One proposal as the view has it.
    #[must_use]
    pub fn of(item: &ProposalItem, view: &ProposalsView) -> Self {
        let waiting = waiting_of(std::slice::from_ref(item), |_| None);
        let verdict_executed = keyed(&view.executions, item.chain_id, &item.safe_tx_hash)
            .and_then(|v| v.get("kind"))
            .and_then(|k| k.as_str())
            .is_some_and(|k| k == "executed" || k == "executedByOther");
        let timeline_executed = keyed(&view.timelines, item.chain_id, &item.safe_tx_hash)
            .is_some_and(|steps| {
                steps
                    .iter()
                    .any(|s| matches!(s, TimelineStep::Executed { .. }))
            });
        let done = item.status == ProposalStatus::Done;
        Self {
            chain_id: item.chain_id,
            safe_tx_hash: item.safe_tx_hash.clone(),
            nonce: item.nonce,
            action: item.review.action.clone(),
            approved: item.approved,
            threshold: item.threshold,
            approvers: item
                .approvals
                .iter()
                .filter(|a| a.counts)
                .map(|a| lower(&a.owner))
                .collect(),
            can_approve: item.can_approve.iter().map(|a| lower(a)).collect(),
            needs_you: waiting.needs_you > 0,
            ready: item.status == ProposalStatus::Ready && item.cancelled_by.is_none(),
            open: !done,
            executed: done && (verdict_executed || timeline_executed),
            unsent: item.unsent,
        }
    }
}

impl Snapshot {
    /// The views now. `presence_live`: the others' presence is a live read
    /// (the host's word).
    #[must_use]
    pub fn of(
        accounts: &MultisigAccountsView,
        proposals: &ProposalsView,
        presence_live: bool,
    ) -> Self {
        let multisigs: BTreeMap<String, (String, Option<String>)> = accounts
            .accounts
            .iter()
            .map(|row| {
                let name = row
                    .name
                    .clone()
                    .or_else(|| row.account.label.clone())
                    .filter(|n| !n.trim().is_empty());
                (
                    lower(&row.account.address),
                    (row.account.address.clone(), name),
                )
            })
            .collect();
        // Only a multisig on this device is followed for notices (a link's
        // preview of somebody else's is not).
        let followed = proposals
            .followed
            .as_deref()
            .map(lower)
            .filter(|f| multisigs.contains_key(f));
        let presence = proposals
            .presence
            .iter()
            .filter(|(m, _)| followed.as_deref() != Some(&lower(m)))
            .filter(|(m, _)| multisigs.contains_key(&lower(m)))
            .filter_map(|(m, w)| w.clone().map(|w| (lower(m), w)))
            .collect();
        let followed = followed
            .filter(|_| proposals.read_at.is_some())
            .map(|address| Followed {
                address,
                items: proposals
                    .items
                    .iter()
                    .map(|i| {
                        (
                            item_key(i.chain_id, &i.safe_tx_hash),
                            ItemSnap::of(i, proposals),
                        )
                    })
                    .collect(),
                read_chains: proposals
                    .chains
                    .iter()
                    .map(|c| c.chain_id)
                    .filter(|c| !proposals.unreadable.contains(c))
                    .collect(),
                proposed_here: proposals
                    .actions
                    .values()
                    .filter_map(|a| match a {
                        ActionState::Done { safe_tx_hash, .. } => Some(lower(safe_tx_hash)),
                        _ => None,
                    })
                    .collect(),
            });
        Self {
            multisigs,
            presence,
            presence_live,
            followed,
        }
    }
}

/// What moved from `prev` to `next`, said once: the whole rule (see the
/// module's head). `memory` is what this session has seen; it is updated.
#[must_use]
pub fn transitions(prev: &Snapshot, next: &Snapshot, memory: &mut Memory) -> Vec<Notice> {
    let mut out = Vec::new();
    let notice = |m: &str, kind: Kind, chain_id: u64, hash: &str, what: Option<What>| {
        let (address, name) = next
            .multisigs
            .get(m)
            .cloned()
            .unwrap_or_else(|| (m.to_owned(), None));
        Notice {
            multisig: address,
            name,
            kind,
            chain_id,
            safe_tx_hash: hash.to_owned(),
            what,
        }
    };

    // The followed multisig: its items, one by one.
    if let Some(f) = &next.followed {
        let before = prev.followed.as_ref().filter(|p| p.address == f.address);
        let m = f.address.as_str();
        for (key, item) in &f.items {
            let hash = lower(&item.safe_tx_hash);
            let what = || {
                Some(What {
                    action: item.action.clone(),
                    nonce: item.nonce,
                })
            };
            let was = before.and_then(|p| p.items.get(key));
            // News only on a list read before, on a network read before:
            // an item that merely came into view is the baseline.
            let loud = before.is_some_and(|p| p.read_chains.contains(&item.chain_id));
            let seen_open = memory.open.contains(&(m.to_owned(), hash.clone()));
            if item.open {
                memory.open.insert((m.to_owned(), hash.clone()));
            }
            // The approvals that arrived since, and whether they were all
            // given by members on this device (the person's own move).
            let arrived: BTreeSet<&String> = was
                .map(|w| item.approvers.difference(&w.approvers).collect())
                .unwrap_or_default();
            let own_move = was.is_some_and(|w| {
                !arrived.is_empty() && arrived.iter().all(|a| w.can_approve.contains(*a))
            });
            let others_arrived =
                was.is_some_and(|w| arrived.iter().any(|a| !w.can_approve.contains(*a)));

            let mut remember = |kind: Kind| memory.known.insert((m.to_owned(), hash.clone(), kind));

            if item.needs_you
                && remember(Kind::ApprovalNeeded)
                && loud
                && !item.unsent
                && !f.proposed_here.contains(&hash)
            {
                out.push(notice(
                    m,
                    Kind::ApprovalNeeded,
                    item.chain_id,
                    &item.safe_tx_hash,
                    what(),
                ));
            }
            let mut said_ready = false;
            if item.ready && remember(Kind::ReadyToExecute) && loud && !own_move {
                said_ready = true;
                out.push(notice(
                    m,
                    Kind::ReadyToExecute,
                    item.chain_id,
                    &item.safe_tx_hash,
                    what(),
                ));
            }
            if let Some(w) = was
                && item.open
                && !item.ready
                && item.approved > w.approved
                && others_arrived
            {
                let kind = Kind::ApprovalCounted {
                    approved: item.approved.min(item.threshold),
                    threshold: item.threshold,
                };
                if remember(kind) && loud && !said_ready {
                    out.push(notice(m, kind, item.chain_id, &item.safe_tx_hash, what()));
                }
            }
            if item.executed && seen_open && remember(Kind::Executed) && before.is_some() {
                out.push(notice(
                    m,
                    Kind::Executed,
                    item.chain_id,
                    &item.safe_tx_hash,
                    what(),
                ));
            }
        }
    }

    // The others: what their presence names first. Counts alone prove
    // nothing about WHICH transaction; `first` names one.
    for (m, w) in &next.presence {
        let named = if w.needs_you > 0 {
            w.first.as_ref().map(|f| (Kind::ApprovalNeeded, f))
        } else if w.ready > 0 {
            w.first.as_ref().map(|f| (Kind::ReadyToExecute, f))
        } else {
            None
        };
        let Some((kind, first)) = named else {
            continue;
        };
        let loud = next.presence_live && prev.presence_live && prev.presence.contains_key(m);
        if memory
            .known
            .insert((m.clone(), lower(&first.safe_tx_hash), kind))
            && loud
        {
            out.push(notice(m, kind, first.chain_id, &first.safe_tx_hash, None));
        }
    }
    out
}

/// The notifier the host keeps: the last snapshot and the memory.
#[derive(Debug, Default)]
pub struct Notifier {
    last: Option<Snapshot>,
    memory: Memory,
}

impl Notifier {
    /// A look at the views now: the notices since the last look (none on
    /// the first — the baseline).
    pub fn observe(&mut self, next: Snapshot) -> Vec<Notice> {
        let prev = self.last.take().unwrap_or_default();
        let notices = transitions(&prev, &next, &mut self.memory);
        self.last = Some(next);
        notices
    }

    /// Forget everything (the switch turned off): turned on again, the next
    /// look is a baseline.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Nothing looked at yet.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.last.is_none()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use vela_core::app::multisig_presence::WaitingFirst;

    const A: &str = "0x00000000000000000000000000000000000000aa";
    const B: &str = "0x00000000000000000000000000000000000000bb";
    const ME: &str = "0x0000000000000000000000000000000000000001";
    const ME2: &str = "0x0000000000000000000000000000000000000002";
    const THEM: &str = "0x0000000000000000000000000000000000000003";
    const H1: &str = "0x1111";
    const H2: &str = "0x2222";

    fn names() -> BTreeMap<String, (String, Option<String>)> {
        BTreeMap::from([
            (
                A.to_owned(),
                (A.to_owned(), Some("Quiet Harbor".to_owned())),
            ),
            (B.to_owned(), (B.to_owned(), None)),
        ])
    }

    /// An open 2-of-3 item with `approvers` in, a member here (`ME`) not
    /// yet approved unless listed.
    fn item(hash: &str, approvers: &[&str]) -> ItemSnap {
        let approvers: BTreeSet<String> = approvers.iter().map(|a| (*a).to_owned()).collect();
        let can_approve: BTreeSet<String> = [ME, ME2]
            .iter()
            .filter(|m| !approvers.contains(**m))
            .map(|m| (*m).to_owned())
            .collect();
        let approved = approvers.len() as u64;
        ItemSnap {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            nonce: 4,
            action: Some(Action::Cancel),
            approved,
            threshold: 2,
            needs_you: approved < 2 && !can_approve.is_empty(),
            ready: approved >= 2,
            open: true,
            approvers,
            can_approve,
            ..ItemSnap::default()
        }
    }

    fn done(hash: &str, executed: bool) -> ItemSnap {
        ItemSnap {
            open: false,
            ready: false,
            needs_you: false,
            executed,
            ..item(hash, &[ME, THEM])
        }
    }

    fn followed(address: &str, items: &[ItemSnap]) -> Followed {
        Followed {
            address: address.to_owned(),
            items: items
                .iter()
                .map(|i| (item_key(i.chain_id, &i.safe_tx_hash), i.clone()))
                .collect(),
            read_chains: BTreeSet::from([100]),
            proposed_here: BTreeSet::new(),
        }
    }

    fn snap(f: Option<Followed>, presence: &[(&str, Waiting)], live: bool) -> Snapshot {
        Snapshot {
            multisigs: names(),
            presence: presence
                .iter()
                .map(|(m, w)| ((*m).to_owned(), w.clone()))
                .collect(),
            presence_live: live,
            followed: f,
        }
    }

    fn waiting(needs_you: u64, ready: u64, first: Option<&str>) -> Waiting {
        Waiting {
            needs_you,
            ready,
            waiting_payer: 0,
            first: first.map(|h| WaitingFirst {
                chain_id: 100,
                safe_tx_hash: h.to_owned(),
            }),
        }
    }

    fn kinds(notices: &[Notice]) -> Vec<(String, Kind)> {
        notices
            .iter()
            .map(|n| (n.safe_tx_hash.clone(), n.kind))
            .collect()
    }

    #[test]
    fn the_first_look_after_launch_is_silent() {
        let mut n = Notifier::default();
        let first = snap(
            Some(followed(A, &[item(H1, &[]), item(H2, &[ME, THEM])])),
            &[(B, waiting(1, 0, Some(H1)))],
            true,
        );
        assert!(n.observe(first.clone()).is_empty(), "the baseline");
        assert!(n.observe(first).is_empty(), "nothing moved");
    }

    #[test]
    fn a_new_proposal_needing_you_is_said_once() {
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[])), &[], true));
        let got = n.observe(snap(Some(followed(A, &[item(H1, &[THEM])])), &[], true));
        assert_eq!(kinds(&got), vec![(H1.to_owned(), Kind::ApprovalNeeded)]);
        assert_eq!(got[0].name.as_deref(), Some("Quiet Harbor"));
        assert_eq!(got[0].what.as_ref().map(|w| w.nonce), Some(4));
        // Read again, and again: never twice.
        let again = n.observe(snap(Some(followed(A, &[item(H1, &[THEM])])), &[], true));
        assert!(again.is_empty());
    }

    #[test]
    fn what_was_proposed_here_is_not_news() {
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[])), &[], true));
        let mut f = followed(A, &[item(H1, &[ME])]);
        f.proposed_here.insert(H1.to_owned());
        // ME2 is here too and can still approve: still not news.
        assert!(n.observe(snap(Some(f), &[], true)).is_empty());
    }

    #[test]
    fn someone_elses_approval_is_counted_and_said() {
        let three = |approvers: &[&str]| ItemSnap {
            threshold: 3,
            ready: false,
            needs_you: true,
            ..item(H1, approvers)
        };
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[three(&[])])), &[], true));
        let got = n.observe(snap(Some(followed(A, &[three(&[THEM])])), &[], true));
        assert_eq!(
            kinds(&got),
            vec![(
                H1.to_owned(),
                Kind::ApprovalCounted {
                    approved: 1,
                    threshold: 3
                }
            )]
        );
        // The same count read again: not twice.
        assert!(
            n.observe(snap(Some(followed(A, &[three(&[THEM])])), &[], true))
                .is_empty()
        );
    }

    #[test]
    fn your_own_approval_is_not_news_nor_the_ready_it_caused() {
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[item(H1, &[THEM])])), &[], true));
        // ME approved here: 2 of 2, ready — the person did that.
        let got = n.observe(snap(Some(followed(A, &[item(H1, &[THEM, ME])])), &[], true));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn ready_by_someone_elses_approval_is_said_instead_of_counted() {
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[item(H1, &[ME])])), &[], true));
        let got = n.observe(snap(Some(followed(A, &[item(H1, &[ME, THEM])])), &[], true));
        assert_eq!(kinds(&got), vec![(H1.to_owned(), Kind::ReadyToExecute)]);
        assert!(
            n.observe(snap(Some(followed(A, &[item(H1, &[ME, THEM])])), &[], true))
                .is_empty()
        );
    }

    #[test]
    fn executed_is_said_only_for_what_was_seen_open_and_proven() {
        let mut n = Notifier::default();
        // H2 is history already done at launch: never said.
        n.observe(snap(
            Some(followed(A, &[item(H1, &[ME, THEM]), done(H2, true)])),
            &[],
            true,
        ));
        // H1's number used, but nothing says by what: not said.
        let got = n.observe(snap(
            Some(followed(A, &[done(H1, false), done(H2, true)])),
            &[],
            true,
        ));
        assert!(got.is_empty(), "{got:?}");
        // The verdict arrives: executed, once.
        let got = n.observe(snap(
            Some(followed(A, &[done(H1, true), done(H2, true)])),
            &[],
            true,
        ));
        assert_eq!(kinds(&got), vec![(H1.to_owned(), Kind::Executed)]);
        assert!(
            n.observe(snap(Some(followed(A, &[done(H1, true)])), &[], true))
                .is_empty()
        );
    }

    #[test]
    fn an_unread_network_coming_into_view_is_the_baseline() {
        let mut n = Notifier::default();
        let mut before = followed(A, &[]);
        before.read_chains.clear();
        n.observe(snap(Some(before), &[], true));
        // The network is read now: an old proposal there is not news.
        assert!(
            n.observe(snap(Some(followed(A, &[item(H1, &[])])), &[], true))
                .is_empty()
        );
    }

    #[test]
    fn the_others_presence_names_what_needs_you() {
        let mut n = Notifier::default();
        n.observe(snap(None, &[(B, waiting(0, 0, None))], true));
        let got = n.observe(snap(None, &[(B, waiting(1, 0, Some(H1)))], true));
        assert_eq!(kinds(&got), vec![(H1.to_owned(), Kind::ApprovalNeeded)]);
        assert!(got[0].what.is_none(), "the presence names no action");
        assert_eq!(got[0].name, None);
        // Approved elsewhere, then a second one becomes first: said.
        n.observe(snap(None, &[(B, waiting(0, 0, None))], true));
        let got = n.observe(snap(None, &[(B, waiting(1, 0, Some(H2)))], true));
        assert_eq!(kinds(&got), vec![(H2.to_owned(), Kind::ApprovalNeeded)]);
        // H1 back as first (the list flapped): never twice.
        assert!(
            n.observe(snap(None, &[(B, waiting(2, 0, Some(H1)))], true))
                .is_empty()
        );
        // Nothing needs you, one is ready to run here.
        let got = n.observe(snap(None, &[(B, waiting(0, 1, Some(H1)))], true));
        assert_eq!(kinds(&got), vec![(H1.to_owned(), Kind::ReadyToExecute)]);
    }

    #[test]
    fn the_list_an_earlier_session_kept_is_never_news() {
        let mut n = Notifier::default();
        // Launch: the kept list, then the first live round lands.
        n.observe(snap(None, &[(B, waiting(1, 0, Some(H1)))], false));
        let got = n.observe(snap(None, &[(B, waiting(1, 0, Some(H2)))], false));
        assert!(got.is_empty(), "not live yet");
        let got = n.observe(snap(None, &[(B, waiting(1, 0, Some(H2)))], true));
        assert!(got.is_empty(), "live now: the baseline, still silent");
        let got = n.observe(snap(None, &[(B, waiting(1, 0, Some(H1)))], true));
        assert!(got.is_empty(), "H1 was seen at launch");
    }

    #[test]
    fn a_multisig_first_seen_is_the_baseline_and_one_removed_is_dropped() {
        let mut n = Notifier::default();
        n.observe(snap(None, &[], true));
        // B added (or first read): silent.
        assert!(
            n.observe(snap(None, &[(B, waiting(1, 0, Some(H1)))], true))
                .is_empty()
        );
        // B removed: nothing said, nothing kept but the memory.
        assert!(n.observe(snap(None, &[], true)).is_empty());
        // B added back with something new: a first sight again, silent.
        assert!(
            n.observe(snap(None, &[(B, waiting(1, 0, Some(H2)))], true))
                .is_empty()
        );
        // From then on, news is news.
        let mut later = waiting(1, 0, Some(H1));
        later.first = Some(WaitingFirst {
            chain_id: 100,
            safe_tx_hash: "0x3333".to_owned(),
        });
        let got = n.observe(snap(None, &[(B, later)], true));
        assert_eq!(
            kinds(&got),
            vec![("0x3333".to_owned(), Kind::ApprovalNeeded)]
        );
    }

    #[test]
    fn switching_the_followed_multisig_says_nothing_already_known() {
        let mut n = Notifier::default();
        // A followed with H1 needing you; B elsewhere, nothing waiting.
        n.observe(snap(
            Some(followed(A, &[item(H1, &[])])),
            &[(B, waiting(0, 0, None))],
            true,
        ));
        // Switched to B: A's presence falls back to an older read naming H1;
        // B's list is read for the first time with H2 needing you.
        let got = n.observe(snap(
            Some(followed(B, &[item(H2, &[])])),
            &[(A, waiting(1, 0, Some(H1)))],
            true,
        ));
        assert!(got.is_empty(), "{got:?}");
        // A, now an "other", reads live: H1 is still first — known.
        assert!(
            n.observe(snap(
                Some(followed(B, &[item(H2, &[])])),
                &[(A, waiting(1, 0, Some(H1)))],
                true,
            ))
            .is_empty()
        );
        // Back to A: H1 known; a new H2 on A is news only once A's list
        // has been read once as followed again.
        let got = n.observe(snap(
            Some(followed(A, &[item(H1, &[]), item("0x4444", &[])])),
            &[(B, waiting(1, 0, Some(H2)))],
            true,
        ));
        assert!(got.is_empty(), "{got:?}");
    }

    #[test]
    fn turned_off_and_on_again_starts_from_a_new_baseline() {
        let mut n = Notifier::default();
        n.observe(snap(Some(followed(A, &[])), &[], true));
        n.reset();
        assert!(n.is_fresh());
        assert!(
            n.observe(snap(Some(followed(A, &[item(H1, &[])])), &[], true))
                .is_empty()
        );
    }

    #[test]
    fn approvals_counted_never_say_more_than_needed() {
        let mut n = Notifier::default();
        let over = |approvers: &[&str], approved: u64| ItemSnap {
            approved,
            ready: false,
            needs_you: false,
            ..item(H1, approvers)
        };
        n.observe(snap(Some(followed(A, &[over(&[], 0)])), &[], true));
        let got = n.observe(snap(Some(followed(A, &[over(&[THEM], 3)])), &[], true));
        assert_eq!(
            kinds(&got),
            vec![(
                H1.to_owned(),
                Kind::ApprovalCounted {
                    approved: 2,
                    threshold: 2
                }
            )]
        );
    }
}
