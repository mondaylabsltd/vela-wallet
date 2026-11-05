//! Proposing through the proposals machine, review first (§3.6): the web's
//! `proposalsHost.propose(account, chainId, proposal, { review: true })`,
//! gpui-free. One path for every proposal a screen makes — the send flow's
//! transfers, Settings' rename and each network's member change: the
//! machine is asked for its `draft`, the screen draws that draft's review
//! (the host's `send_reviewing`), the person's word confirms it
//! (`confirm_draft`) or withdraws it, and how it ended is read from the
//! machine's `actions["propose"]` — never decided here.
//!
//! ```text
//!   start ──► Drafting ── view.draft ──► Reviewing ── Working ──► Confirmed ── settled ──► end
//!                │                           │
//!                └── settled (refused) ──► end   └── draft gone ──► end (withdrawn)
//! ```

use vela_core::app::multisig_proposals::{
    self as proposals, ActionState, ProposalKind, ProposalsView,
};

use super::machines::{Job, Machines};

/// Where a proposal stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Asked for its draft: its review comes first.
    Drafting,
    /// Its review is on screen: the person's word next.
    Reviewing,
    /// Confirmed: the passkey (or an outside wallet), then the mailbox.
    Confirmed,
}

/// How a proposal ended (the proposals machine's word on it).
#[derive(Clone, Debug, PartialEq)]
pub enum Proposed {
    /// In the mailbox.
    Done { safe_tx_hash: String },
    /// Not proposed: the machine's last word on it (`None`: withdrawn on
    /// its review before anything was signed), and whether the machine was
    /// offline then.
    Ended {
        state: Option<ActionState>,
        offline: bool,
    },
}

impl Proposed {
    /// The person stopped it (closed the review, the passkey, or declined
    /// the risk question): nothing to say.
    #[must_use]
    pub fn declined(&self) -> bool {
        matches!(
            self,
            Self::Ended {
                state: None | Some(ActionState::Cancelled | ActionState::RiskDeclined),
                ..
            }
        )
    }

    /// Withdrawn before it went: an earlier proposal replaced by a later one.
    #[must_use]
    pub fn withdrawn() -> Self {
        Self::Ended {
            state: Some(ActionState::Cancelled),
            offline: false,
        }
    }
}

/// How a send flow's proposal ended (its `Proposed` / `ProposeFailed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProposeEnd {
    Proposed {
        safe_tx_hash: String,
    },
    Failed {
        cancelled: bool,
        reason: Option<vela_core::app::send::SendProposeFailure>,
    },
}

/// The send flow's words for how a proposal ended.
#[must_use]
pub fn send_end(proposed: &Proposed) -> ProposeEnd {
    use vela_core::app::send::SendProposeFailure;
    match proposed {
        Proposed::Done { safe_tx_hash } => ProposeEnd::Proposed {
            safe_tx_hash: safe_tx_hash.clone(),
        },
        Proposed::Ended { state, offline } => match state {
            None | Some(ActionState::Cancelled | ActionState::RiskDeclined) => ProposeEnd::Failed {
                cancelled: true,
                reason: None,
            },
            Some(ActionState::MailboxUnreadable) => ProposeEnd::Failed {
                cancelled: false,
                reason: Some(SendProposeFailure::MailboxDown),
            },
            // r13-3: the network did not answer the reads it is built from.
            Some(ActionState::Unread { .. }) => ProposeEnd::Failed {
                cancelled: false,
                reason: Some(SendProposeFailure::ChainDown),
            },
            _ => ProposeEnd::Failed {
                cancelled: false,
                reason: offline.then_some(SendProposeFailure::Offline),
            },
        },
    }
}

/// Ask the proposals machine for `proposal`'s draft on `chain_id`, for
/// `multisig` (followed first when it is not the one followed).
pub fn start(
    machines: &mut Machines,
    chain_id: u64,
    multisig: &str,
    proposal: ProposalKind,
) -> Vec<Job> {
    let followed = machines
        .proposals_view
        .followed
        .as_deref()
        .is_some_and(|f| f.eq_ignore_ascii_case(multisig));
    let mut jobs = Vec::new();
    if !followed {
        let account = machines
            .accounts_view
            .accounts
            .iter()
            .find(|row| row.account.address.eq_ignore_ascii_case(multisig))
            .map(|row| row.account.clone());
        jobs.extend(machines.proposals(proposals::Event::Follow { account }));
    }
    jobs.extend(machines.proposals(proposals::Event::Draft { chain_id, proposal }));
    jobs
}

/// Where the proposal at `step` stands now, from the machine's view: the
/// step moves on, and `Some` once it ended.
pub fn follow(step: &mut Step, view: &ProposalsView) -> Option<Proposed> {
    let state = view.actions.get("propose").cloned();
    let settled = |s: &Option<ActionState>| {
        // A word that is no longer "working" (or asking an outside wallet).
        s.as_ref()
            .is_some_and(|s| !matches!(s, ActionState::Working | ActionState::OutsideAsked { .. }))
    };
    let end = match *step {
        Step::Drafting => {
            if view.draft.is_some() {
                *step = Step::Reviewing;
                None
            } else if settled(&state) {
                Some(state)
            } else {
                None
            }
        }
        // On its review the machine holds no word on it (`actions["propose"]`
        // is cleared when the draft is ready): any word now is the person's
        // — confirmed (working, or already settled when the passkey answered
        // at once) or withdrawn.
        Step::Reviewing => match state {
            Some(ActionState::Working | ActionState::OutsideAsked { .. }) => {
                *step = Step::Confirmed;
                None
            }
            Some(settled) => Some(Some(settled)),
            None if view.draft.is_none() => Some(Some(ActionState::Cancelled)),
            None => None,
        },
        Step::Confirmed => settled(&state).then_some(state),
    }?;
    Some(match end {
        Some(ActionState::Done { safe_tx_hash, .. }) => Proposed::Done { safe_tx_hash },
        state => Proposed::Ended {
            state,
            offline: view.offline,
        },
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use vela_core::app::send::SendProposeFailure;

    #[test]
    fn the_send_flow_hears_its_own_words() {
        assert_eq!(
            send_end(&Proposed::Done {
                safe_tx_hash: "0xab".to_owned()
            }),
            ProposeEnd::Proposed {
                safe_tx_hash: "0xab".to_owned()
            }
        );
        assert_eq!(
            send_end(&Proposed::withdrawn()),
            ProposeEnd::Failed {
                cancelled: true,
                reason: None
            }
        );
        assert_eq!(
            send_end(&Proposed::Ended {
                state: Some(ActionState::MailboxUnreadable),
                offline: false
            }),
            ProposeEnd::Failed {
                cancelled: false,
                reason: Some(SendProposeFailure::MailboxDown)
            }
        );
        assert_eq!(
            send_end(&Proposed::Ended {
                state: Some(ActionState::Failed {
                    detail: "x".to_owned()
                }),
                offline: true
            }),
            ProposeEnd::Failed {
                cancelled: false,
                reason: Some(SendProposeFailure::Offline)
            }
        );
    }

    #[test]
    fn declined_is_the_persons_stop() {
        assert!(Proposed::withdrawn().declined());
        assert!(
            Proposed::Ended {
                state: Some(ActionState::RiskDeclined),
                offline: false
            }
            .declined()
        );
        assert!(
            !Proposed::Ended {
                state: Some(ActionState::MailboxUnreadable),
                offline: false
            }
            .declined()
        );
    }

    #[test]
    fn on_its_review_only_the_persons_word_moves_it() {
        let mut view = ProposalsView::default();
        let mut step = Step::Reviewing;
        // Confirmed, and the passkey answered at once: done in one go (the
        // draft is gone with it — never read as a withdrawal).
        view.actions.insert(
            "propose".to_owned(),
            ActionState::Done {
                safe_tx_hash: "0xab".to_owned(),
                kept: false,
            },
        );
        assert_eq!(
            follow(&mut step, &view),
            Some(Proposed::Done {
                safe_tx_hash: "0xab".to_owned()
            })
        );
        // Withdrawn on its review.
        let mut step = Step::Reviewing;
        view.actions
            .insert("propose".to_owned(), ActionState::Cancelled);
        assert!(follow(&mut step, &view).is_some_and(|p| p.declined()));
    }

    #[test]
    fn a_refusal_before_any_review_ends_it() {
        let mut view = ProposalsView::default();
        let mut step = Step::Drafting;
        assert_eq!(follow(&mut step, &view), None);
        view.actions.insert(
            "propose".to_owned(),
            ActionState::Refused {
                reason: "no".to_owned(),
            },
        );
        assert!(matches!(
            follow(&mut step, &view),
            Some(Proposed::Ended {
                state: Some(ActionState::Refused { .. }),
                ..
            })
        ));
    }
}
