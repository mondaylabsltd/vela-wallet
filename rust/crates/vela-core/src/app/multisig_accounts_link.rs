//! A multisig link opened on this device, as ONE flow (the web's
//! `MultisigHost.answerLink` / `openProposal`, `linked.svelte.ts`
//! `openProposalLink` and `kept-link.ts`; ms-21, ms-22, r4b-9, r4b-13):
//!
//! - a damaged link is said at once, never ignored (ms-22): what it held and
//!   what is wrong with it;
//! - an invite (`#msa=`) opens the add sheet — adding is the person's tap;
//! - a proposal link (`#ms=`) on a network this device lacks is said, with
//!   Add network, and opens again once the network is here;
//! - otherwise the multisig is found on this device, else looked up (one
//!   lookup on the link's network, kept nowhere): a member of it here (its
//!   founding members, or the chain's members now) — it is kept if new,
//!   shown, and its proposal opens ([`Notice::OpenProposal`]); none here —
//!   its transaction is read only, nothing added, nothing switched; not
//!   found, or who can act for it is not only its members (§3.5) — nothing
//!   is read or kept, and the add sheet says why;
//! - a link opened with no Vela account yet is kept for a day
//!   ([`Event::KeepLink`]) and opened once the wallet is there
//!   (`OpenLink { take_kept: true }`);
//! - a proposal link waiting for its network is the machine's record
//!   ([`link::PENDING_LINK_KEY`], a day): a reload still waits for it, the
//!   network added after the reload opens it, and the person closing its
//!   sheet lets it go ([`Event::DropLink`]).

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::*;
use crate::multisig_link::{self as link, AddLink, AddLinkFault, BrokenLink, ProposalLink};

/// Where the link opened stands, for the shell to draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigLinkStand"))]
pub enum LinkStand {
    /// ms-22: meant as a link, and it does not read: what it held (`text`)
    /// and, for an invite, what is wrong with it (r5reg-22).
    Broken {
        link: BrokenLink,
        text: String,
        fault: Option<AddLinkFault>,
    },
    /// r4D-4: an invite — the add sheet, filled in; adding is the person's tap.
    Invite { link: AddLink },
    /// ms-22: the link's network is not on this device: Add network. The
    /// link opens again once it is here ([`Event::Networks`]).
    MissingNetwork { link: ProposalLink },
    /// Reading the multisig (ms-16): said, never a blank wait.
    Opening { link: ProposalLink },
    /// A member is here: shown, its proposal opening. `kept_before`: it was
    /// on this device already (else it was just kept).
    Member {
        link: ProposalLink,
        address: String,
        kept_before: bool,
    },
    /// ms-21: none is — its transaction read only; nothing added, nothing
    /// switched (following it is `KeepFound`, the person's tap). `kept`: it
    /// is on this device already.
    Preview { link: ProposalLink, kept: bool },
    /// Not found or not read — or who can act for it is not only its members
    /// (refused, high risk, not checkable yet): the add sheet says why, with
    /// the multisig filled in; its transaction opens once it is added.
    Failed {
        link: ProposalLink,
        reason: AddOutcome,
    },
    /// r4b-9: kept until the wallet is there.
    Kept,
}

#[derive(Default)]
pub struct LinkState {
    pub stand: Option<LinkStand>,
    /// A link waiting for its network (or for its multisig to be added).
    pending: Option<ProposalLink>,
    /// The pending link is in the store (it waits for its network).
    held: bool,
}

/// ms-22: `l` waits for its network — kept in the store, so a reload (the
/// Add network wizard, another tab) still opens it once the network is here.
async fn hold(io: &Io<Event>, st: &Shared<State>, l: &ProposalLink) {
    let now = io.now().await;
    {
        let mut s = lock(st);
        s.link.pending = Some(l.clone());
        s.link.held = true;
        s.link.stand = Some(LinkStand::MissingNetwork { link: l.clone() });
    }
    io.store(
        link::PENDING_LINK_KEY,
        link::keep_link(&link::proposal_hash(l), now),
    )
    .await;
}

/// The stored link waiting for its network is no longer (opened, replaced
/// or let go).
pub async fn forget(io: &Io<Event>, st: &Shared<State>) {
    let held = std::mem::take(&mut lock(st).link.held);
    if held {
        io.store(link::PENDING_LINK_KEY, None).await;
    }
}

/// The person closed the missing network's sheet: the link waiting for it
/// is let go (they open it again by hand).
pub async fn drop_pending(io: &Io<Event>, st: &Shared<State>) {
    {
        let mut s = lock(st);
        s.link.pending = None;
        s.link.held = false;
    }
    io.store(link::PENDING_LINK_KEY, None).await;
}

/// On start (and a reload): the link that waited for its network before —
/// opened now if the network is here, else waited for again, quietly (its
/// sheet was closed for the wizard); one older than a day is dropped.
pub async fn restore(io: &Io<Event>, st: &Shared<State>) {
    let stored = io
        .load(&[link::PENDING_LINK_KEY])
        .await
        .get(link::PENDING_LINK_KEY)
        .cloned()
        .flatten();
    let Some(stored) = stored else {
        // Let go in another document: not waited for here either.
        let mut s = lock(st);
        if std::mem::take(&mut s.link.held) {
            s.link.pending = None;
        }
        return;
    };
    let now = io.now().await;
    let waiting = link::take_kept_link(Some(&stored), now)
        .as_deref()
        .and_then(link::parse_proposal_link);
    let Some(waiting) = waiting else {
        lock(st).link.held = false;
        io.store(link::PENDING_LINK_KEY, None).await;
        return;
    };
    let here = {
        let mut s = lock(st);
        s.link.held = true;
        let here = s.has_network(waiting.chain_id);
        if !here {
            s.link.pending = Some(waiting.clone());
        }
        here
    };
    if here {
        open_proposal(io, st, waiting).await;
    }
}

/// The pending link, once its network is on this device.
pub fn pending_ready(s: &mut State) -> Option<ProposalLink> {
    let ready = s
        .link
        .pending
        .as_ref()
        .is_some_and(|l| s.networks.contains(&l.chain_id));
    if ready {
        s.link.pending.take()
    } else {
        None
    }
}

/// The multisig a failed link named was added: its transaction opens now.
pub fn pending_for(s: &mut State, address: &str) -> Option<ProposalLink> {
    let hit = s
        .link
        .pending
        .as_ref()
        .is_some_and(|l| same_address(&l.multisig, address));
    if hit {
        s.link.pending.take()
    } else {
        None
    }
}

fn say(st: &Shared<State>, stand: LinkStand) {
    lock(st).link.stand = Some(stand);
}

/// r4b-9: keep `hash` (a multisig link) while there is no wallet here.
pub async fn keep(io: &Io<Event>, st: &Shared<State>, hash: &str) {
    let now = io.now().await;
    if let Some(record) = link::keep_link(hash, now) {
        io.store(link::KEPT_LINK_KEY, Some(record)).await;
        say(st, LinkStand::Kept);
    }
}

/// Open `hash` — or, `take_kept`, the link kept while there was no wallet
/// here: a link in the address is the one answered, and a different one
/// kept on the way is kept for later.
pub async fn open(io: &Io<Event>, st: &Shared<State>, hash: String, take_kept: bool) {
    let mut hash = hash;
    if take_kept {
        let now = io.now().await;
        let stored = io
            .load(&[link::KEPT_LINK_KEY])
            .await
            .get(link::KEPT_LINK_KEY)
            .cloned()
            .flatten();
        if stored.is_some() {
            io.store(link::KEPT_LINK_KEY, None).await;
        }
        let kept = link::take_kept_link(stored.as_deref(), now);
        if !link::is_multisig_link(&hash) {
            if let Some(kept) = kept {
                hash = kept;
            }
        } else if let Some(kept) = kept {
            if link::carried_link_hash(&kept) != link::carried_link_hash(&hash) {
                if let Some(record) = link::keep_link(&kept, now) {
                    io.store(link::KEPT_LINK_KEY, Some(record)).await;
                }
            }
        }
    }
    if let Some(broken) = link::broken_link(&hash) {
        let fault = (broken == BrokenLink::Add)
            .then(|| link::add_link_fault(&hash).unwrap_or(AddLinkFault::Damaged));
        return say(
            st,
            LinkStand::Broken {
                link: broken,
                text: link::broken_link_text(&hash),
                fault,
            },
        );
    }
    if let Some(invite) = link::parse_add_link(&hash) {
        return say(st, LinkStand::Invite { link: invite });
    }
    if let Some(proposal) = link::parse_proposal_link(&hash) {
        open_proposal(io, st, proposal).await;
    }
}

/// A proposal link's transaction (`openProposal` + `openProposalLink`).
pub async fn open_proposal(io: &Io<Event>, st: &Shared<State>, l: ProposalLink) {
    if !lock(st).has_network(l.chain_id) {
        return hold(io, st, &l).await;
    }
    // One link waits at a time: this one is in front now.
    forget(io, st).await;
    say(st, LinkStand::Opening { link: l.clone() });
    io.emit(Event::Changed);
    let (stored, locals) = {
        let s = lock(st);
        (s.find(&l.multisig).cloned(), s.local.clone())
    };
    // Its founding members are enough to know a member is here.
    if let Some(stored) = &stored {
        if !local_owners(&locals, &stored.owners).is_empty() {
            return open_member(io, st, &l, stored, true).await;
        }
    }
    let account = match stored.clone() {
        Some(a) => a,
        None => {
            // The link names its network: that is where it is used. r4b-13:
            // one lookup, answered in a few seconds — asked once more at most.
            match find_multisig(
                io,
                st,
                &l.multisig,
                &[l.chain_id],
                Some(l.chain_id),
                1,
                2500,
            )
            .await
            {
                Found::Account(account, findings) if findings.is_empty() => {
                    lock(st).found = Some(((*account).clone(), Vec::new()));
                    *account
                }
                Found::Account(_, findings) => {
                    return fail(st, l, AddOutcome::HighRisk { findings });
                }
                Found::No(AddOutcome::ChooseNetwork) => {
                    return fail(st, l, AddOutcome::NotFound);
                }
                Found::No(reason) => return fail(st, l, reason),
            }
        }
    };
    // The members now are the chain's: one added since the genesis counts.
    let chain = lock(st).chain.clone();
    let owners = chain
        .read_members(io, l.chain_id, &account.address, &account.genesis)
        .await
        .map_or_else(|_| account.owners.clone(), |m| m.owners);
    if !local_owners(&locals, &owners).is_empty() {
        let kept = if stored.is_some() {
            Some(account.clone())
        } else {
            keep_multisig(io, st, &account, Vec::new()).await
        };
        let Some(kept) = kept else {
            return fail(st, l, AddOutcome::NotFound);
        };
        return open_member(io, st, &l, &kept, stored.is_some()).await;
    }
    io.notice(Notice::OpenProposal {
        address: account.address.clone(),
        chain_id: l.chain_id,
        safe_tx_hash: l.safe_tx_hash.clone(),
        preview: Some(Box::new(account)),
    })
    .await;
    say(
        st,
        LinkStand::Preview {
            link: l,
            kept: stored.is_some(),
        },
    );
}

fn fail(st: &Shared<State>, l: ProposalLink, reason: AddOutcome) {
    let mut s = lock(st);
    s.add.address = Some(l.multisig.clone());
    s.link.pending = Some(l.clone());
    s.link.stand = Some(LinkStand::Failed { link: l, reason });
}

/// The link's transaction, on its multisig (on this device now): shown,
/// used on the link's network, its proposal opened.
pub async fn open_member(
    io: &Io<Event>,
    st: &Shared<State>,
    l: &ProposalLink,
    account: &MultisigAccount,
    kept_before: bool,
) {
    lock(st).link.pending = None;
    activate(io, st, Some(account.address.clone())).await;
    if lock(st).use_chain(&account.address, l.chain_id) {
        save(io, st).await;
    }
    io.notice(Notice::OpenProposal {
        address: account.address.clone(),
        chain_id: l.chain_id,
        safe_tx_hash: l.safe_tx_hash.clone(),
        preview: None,
    })
    .await;
    say(
        st,
        LinkStand::Member {
            link: l.clone(),
            address: account.address.clone(),
            kept_before,
        },
    );
}
