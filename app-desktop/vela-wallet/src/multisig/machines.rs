//! The desktop's one host of the core's multisig machines, without gpui:
//! the accounts, the proposals and the members machines, one each for the
//! process, and Vela's side of an outside owner's session while one is open
//! — the web's `shell/machines.svelte.ts` plus `proposals-host.svelte.ts`'s
//! router, in-process over [`CoreHost`].
//!
//! It decides nothing. It starts the machines from the app's facts, hands
//! every operation they ask for to whoever performs it (as a [`Job`] naming
//! its machine), feeds one machine's view to another where the web's single
//! store did (the multisigs on this device to the proposals machine, the
//! proposals waiting to the members machine, the multisig shown to the
//! proposals machine), and routes every [`Notice`] ([`Machines::route`],
//! docs/multisig-machines.md §1.2). What a fact means is a machine's.
//!
//! `gpui` drives it from `host.rs`; the scenario tests drive it with a
//! simulated world, synchronously.

use serde_json::Value;
use vela_core::app::multisig_accounts::{self as accounts, MultisigAccounts, MultisigAccountsView};
use vela_core::app::multisig_members::{self as members, MultisigMembers, MultisigMembersView};
use vela_core::app::multisig_outside::session::{
    self, OutsideSession, OutsideSessionView, SessionRequest,
};
use vela_core::app::multisig_proposals::{self as proposals, MultisigProposals, ProposalsView};
use vela_core::app::multisig_shell::{
    LocalAccount, MultisigAccount, MultisigOperation as Op, MultisigShellResult as Res, Notice,
};

use crate::core_host::{CoreHost, Pending};

/// Which machine an operation belongs to (and its answer goes back to).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Which {
    Accounts,
    Proposals,
    Members,
    /// The outside session open now, by its generation: an answer meant for
    /// a session that was closed is dropped, never handed to the next one.
    Session(u64),
}

/// One operation to perform, and where its answer goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub which: Which,
    pub id: u64,
    pub op: Op,
}

/// What the screen does with a notice the machines cannot take themselves.
#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    /// A link's transaction opened (the sheet shows it; the proposals
    /// machine was told `open_linked`).
    OpenProposal {
        address: String,
        chain_id: u64,
        safe_tx_hash: String,
        preview: bool,
    },
    /// Executed transactions, for the activity (`activity_rows`): the core's
    /// plan, the shell words them.
    ActivityRows(Vec<Value>),
    /// The multisig shown changed (`None`: the Vela account).
    Activated(Option<String>),
    /// An outside member's wallet is asked: the session's sheet opens.
    OutsideSession,
}

/// The facts the machines start from, and are told again as they move.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub service_url: String,
    /// This device's Vela accounts.
    pub accounts: Vec<LocalAccount>,
    /// Every network this device has.
    pub networks: Vec<u64>,
    /// The space's networks (mainnets or testnets).
    pub space: Vec<u64>,
    /// The PeerConnect relay a session's room is opened on (`None`: the
    /// default one).
    pub relay: Option<String>,
    /// Contacts, own accounts and multisigs the person knows (the review's
    /// look-alikes and first sends), as `Known` values.
    pub known: Vec<Value>,
}

/// The outside session open now.
struct Open {
    generation: u64,
    host: CoreHost<OutsideSession>,
    request: SessionRequest,
    view: OutsideSessionView,
    /// Its outcome was handed back to the proposals machine.
    answered: bool,
    /// The rooms it opened (the transport's handles), for the person's Retry.
    rooms: Vec<String>,
}

/// The machines and their latest views.
pub struct Machines {
    accounts: CoreHost<MultisigAccounts>,
    proposals: CoreHost<MultisigProposals>,
    members: CoreHost<MultisigMembers>,
    session: Option<Open>,
    next_session: u64,
    pub accounts_view: MultisigAccountsView,
    pub proposals_view: ProposalsView,
    pub members_view: MultisigMembersView,
    facts: Facts,
    /// What was last fed across (said once per change).
    fed_accounts: Option<Vec<MultisigAccount>>,
    fed_followed: Option<Option<MultisigAccount>>,
    fed_waiting: Option<Vec<members::WaitingProposal>>,
    /// The notices the screen acts on, in order (taken by [`Self::take_screen`]).
    screen: Vec<Screen>,
    /// The `activated` the accounts machine last said (or `None` before it said any).
    started: bool,
}

impl Default for Machines {
    fn default() -> Self {
        Self::new()
    }
}

fn tag<Op2>(which: Which, pending: Vec<Pending<Op2>>, map: impl Fn(Op2) -> Op) -> Vec<Job> {
    pending
        .into_iter()
        .map(|p| Job {
            which,
            id: p.id,
            op: map(p.operation),
        })
        .collect()
}

impl Machines {
    #[must_use]
    pub fn new() -> Self {
        let accounts = CoreHost::<MultisigAccounts>::new();
        let proposals = CoreHost::<MultisigProposals>::new();
        let members = CoreHost::<MultisigMembers>::new();
        Self {
            accounts_view: accounts.view(),
            proposals_view: proposals.view(),
            members_view: members.view(),
            accounts,
            proposals,
            members,
            session: None,
            next_session: 0,
            facts: Facts::default(),
            fed_accounts: None,
            fed_followed: None,
            fed_waiting: None,
            screen: Vec::new(),
            started: false,
        }
    }

    /// Boot (the web's `MultisigMachines.boot`): every machine started from
    /// the same facts. Once.
    pub fn start(&mut self, facts: Facts) -> Vec<Job> {
        if self.started {
            return Vec::new();
        }
        self.started = true;
        self.facts = facts.clone();
        let mut jobs = self.accounts(accounts::Event::Start {
            service_url: facts.service_url.clone(),
            accounts: facts.accounts.clone(),
            networks: facts.networks.clone(),
        });
        jobs.extend(self.accounts(accounts::Event::Space {
            chain_ids: facts.space.clone(),
        }));
        jobs.extend(self.proposals(proposals::Event::Start {
            service_url: facts.service_url.clone(),
            accounts: facts.accounts.clone(),
            networks: facts.networks.clone(),
            known: known_of(&facts.known),
        }));
        jobs.extend(self.feed());
        jobs
    }

    /// The facts as they move (another account signed in, a network added,
    /// another service): each one that changed is told as its event.
    pub fn facts(&mut self, facts: Facts) -> Vec<Job> {
        if !self.started {
            return self.start(facts);
        }
        let was = std::mem::replace(&mut self.facts, facts.clone());
        let mut jobs = Vec::new();
        if was.accounts != facts.accounts {
            jobs.extend(self.accounts(accounts::Event::LocalAccounts {
                accounts: facts.accounts.clone(),
            }));
            jobs.extend(self.members(members::Event::LocalAccounts {
                local: facts.accounts.clone(),
            }));
            jobs.extend(self.proposals(proposals::Event::LocalAccounts {
                accounts: facts.accounts.clone(),
            }));
        }
        if was.networks != facts.networks {
            jobs.extend(self.accounts(accounts::Event::Networks {
                networks: facts.networks.clone(),
            }));
            jobs.extend(self.proposals(proposals::Event::Networks {
                networks: facts.networks.clone(),
            }));
        }
        if was.space != facts.space {
            jobs.extend(self.accounts(accounts::Event::Space {
                chain_ids: facts.space.clone(),
            }));
        }
        if was.service_url != facts.service_url {
            jobs.extend(self.accounts(accounts::Event::Service {
                url: facts.service_url.clone(),
            }));
            jobs.extend(self.proposals(proposals::Event::Service {
                url: facts.service_url.clone(),
            }));
        }
        if was.known != facts.known {
            jobs.extend(self.proposals(proposals::Event::Known {
                known: known_of(&facts.known),
            }));
        }
        jobs
    }

    /// The facts the machines were last told.
    #[must_use]
    pub fn current_facts(&self) -> &Facts {
        &self.facts
    }

    // -- events ------------------------------------------------------------

    pub fn accounts(&mut self, event: accounts::Event) -> Vec<Job> {
        let pending = self.accounts.dispatch(event);
        self.accounts_view = self.accounts.view();
        let mut jobs = tag(Which::Accounts, pending, |op| op);
        jobs.extend(self.feed());
        jobs
    }

    pub fn proposals(&mut self, event: proposals::Event) -> Vec<Job> {
        let pending = self.proposals.dispatch(event);
        self.proposals_view = self.proposals.view();
        let mut jobs = tag(Which::Proposals, pending, |op| op);
        jobs.extend(self.feed());
        jobs
    }

    pub fn members(&mut self, event: members::Event) -> Vec<Job> {
        let pending = self.members.dispatch(event);
        self.members_view = self.members.view();
        tag(Which::Members, pending, |op| op)
    }

    /// An event for the session open now (none: nothing happens).
    pub fn session(&mut self, event: session::Event) -> Vec<Job> {
        let Some(open) = self.session.as_mut() else {
            return Vec::new();
        };
        let pending = open.host.dispatch(event);
        open.view = open.host.view();
        let which = Which::Session(open.generation);
        let mut jobs = tag(which, pending, |op| op);
        jobs.extend(self.settle_session());
        jobs
    }

    /// The rooms the session open now opened.
    #[must_use]
    pub fn session_rooms(&self) -> Vec<String> {
        self.session
            .as_ref()
            .map(|s| s.rooms.clone())
            .unwrap_or_default()
    }

    /// The session open now: its view and the request it asks.
    #[must_use]
    pub fn session_view(&self) -> Option<(&OutsideSessionView, &SessionRequest)> {
        self.session.as_ref().map(|s| (&s.view, &s.request))
    }

    /// The session's sheet closed and its outcome is in: it goes.
    pub fn end_session(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if self.session.as_ref().is_some_and(|s| !s.answered) {
            jobs.extend(self.session(session::Event::Close));
        }
        if self.session.as_ref().is_some_and(|s| s.answered) {
            self.session = None;
        }
        jobs
    }

    /// An answer to one operation. An answer for a machine that went (a
    /// session closed) changes nothing.
    pub fn resolve(&mut self, which: Which, id: u64, result: Res) -> Vec<Job> {
        match which {
            Which::Accounts => {
                let pending = self.accounts.resolve(id, result);
                self.accounts_view = self.accounts.view();
                let mut jobs = tag(which, pending, |op| op);
                jobs.extend(self.feed());
                jobs
            }
            Which::Proposals => {
                let pending = self.proposals.resolve(id, result);
                self.proposals_view = self.proposals.view();
                let mut jobs = tag(which, pending, |op| op);
                jobs.extend(self.feed());
                jobs
            }
            Which::Members => {
                let pending = self.members.resolve(id, result);
                self.members_view = self.members.view();
                tag(which, pending, |op| op)
            }
            Which::Session(generation) => {
                let Some(open) = self.session.as_mut().filter(|s| s.generation == generation)
                else {
                    return Vec::new();
                };
                if let Res::PeerOpened { room, .. } = &result {
                    open.rooms.push(room.clone());
                }
                let pending = open.host.resolve(id, result);
                open.view = open.host.view();
                let mut jobs = tag(which, pending, |op| op);
                jobs.extend(self.settle_session());
                jobs
            }
        }
    }

    /// The notices the screen acts on since the last call.
    pub fn take_screen(&mut self) -> Vec<Screen> {
        std::mem::take(&mut self.screen)
    }

    // -- the one router (docs/multisig-machines.md §1.2) --------------------

    /// Where a notice goes: a multisig's record to the accounts machine; a
    /// link's transaction to the proposals machine and the sheet; executed
    /// rows to the activity; a mailbox that took something to the list
    /// shown; an outside member's wallet to a session. Answered `ack` by the
    /// caller.
    pub fn route(&mut self, notice: Notice) -> Vec<Job> {
        match notice {
            Notice::Account { address, update } => {
                self.accounts(accounts::Event::Apply { address, update })
            }
            Notice::ActivityRows { rows } => {
                if !rows.is_empty() {
                    self.screen.push(Screen::ActivityRows(rows));
                }
                Vec::new()
            }
            Notice::MailboxTook { safe } => {
                let shown = self
                    .proposals_view
                    .followed
                    .as_deref()
                    .is_some_and(|f| f.eq_ignore_ascii_case(&safe));
                if shown {
                    self.proposals(proposals::Event::Refresh { fresh: true })
                } else {
                    Vec::new()
                }
            }
            Notice::Activated { address } => {
                self.screen.push(Screen::Activated(address));
                Vec::new()
            }
            Notice::OpenProposal {
                address,
                chain_id,
                safe_tx_hash,
                preview,
            } => {
                self.screen.push(Screen::OpenProposal {
                    address,
                    chain_id,
                    safe_tx_hash: safe_tx_hash.clone(),
                    preview: preview.is_some(),
                });
                // Not a preview: the multisig is on this device and was
                // activated — followed first, so the sheet reads its list.
                let mut jobs = self.feed();
                jobs.extend(self.proposals(proposals::Event::OpenLinked {
                    chain_id,
                    safe_tx_hash,
                    preview,
                }));
                jobs
            }
            Notice::OutsideSign { request } => self.open_session(*request),
        }
    }

    /// A session for `request` (one at a time: an earlier one is closed and
    /// its outcome, if any, already went back).
    fn open_session(&mut self, request: SessionRequest) -> Vec<Job> {
        let mut jobs = Vec::new();
        if self.session.as_ref().is_some_and(|s| !s.answered) {
            jobs.extend(self.session(session::Event::Close));
        }
        self.next_session += 1;
        let generation = self.next_session;
        let host = CoreHost::<OutsideSession>::new();
        let view = host.view();
        self.session = Some(Open {
            generation,
            host,
            request: request.clone(),
            view,
            answered: false,
            rooms: Vec::new(),
        });
        self.screen.push(Screen::OutsideSession);
        jobs.extend(self.session(session::Event::Start {
            request: Box::new(request),
            service_url: self.facts.service_url.clone(),
            relay: self.facts.relay.clone(),
            origin: None,
        }));
        jobs
    }

    /// The session's outcome, once, back to the proposals machine
    /// (`outside_answered`).
    fn settle_session(&mut self) -> Vec<Job> {
        let Some(open) = self.session.as_mut() else {
            return Vec::new();
        };
        if open.answered {
            return Vec::new();
        }
        let Some(outcome) = open.view.outcome.clone() else {
            return Vec::new();
        };
        open.answered = true;
        let (chain_id, safe_tx_hash) = (open.request.chain_id, open.request.safe_tx_hash.clone());
        self.proposals(proposals::Event::OutsideAnswered {
            chain_id,
            safe_tx_hash,
            outcome,
        })
    }

    // -- what one machine's view tells another -----------------------------

    /// The web's `$effect`s between machines, each said once per change:
    /// every multisig on this device to the proposals machine (`accounts`),
    /// the one shown (`follow` / `account`), and the proposals waiting on it
    /// to the members machine (`waiting`).
    fn feed(&mut self) -> Vec<Job> {
        let mut jobs = Vec::new();
        if !self.accounts_view.loaded {
            return jobs;
        }
        let list: Vec<MultisigAccount> = self
            .accounts_view
            .accounts
            .iter()
            .map(|row| row.account.clone())
            .collect();
        if self.fed_accounts.as_ref() != Some(&list) {
            self.fed_accounts = Some(list.clone());
            jobs.extend(self.proposals_raw(proposals::Event::Accounts {
                accounts: list.clone(),
            }));
        }
        let active = self.accounts_view.active.as_deref().and_then(|address| {
            list.iter()
                .find(|a| a.address.eq_ignore_ascii_case(address))
                .cloned()
        });
        let was = self.fed_followed.clone();
        if was.as_ref() != Some(&active) {
            self.fed_followed = Some(active.clone());
            let same = match (&was, &active) {
                (Some(Some(a)), Some(b)) => a.address.eq_ignore_ascii_case(&b.address),
                _ => false,
            };
            jobs.extend(match (&active, same) {
                // The followed one's record changed (a network, a name).
                (Some(account), true) => self.proposals_raw(proposals::Event::Account {
                    account: account.clone(),
                }),
                _ => self.proposals_raw(proposals::Event::Follow {
                    account: active.clone(),
                }),
            });
        }
        let waiting = waiting_of(&self.proposals_view);
        if self.fed_waiting.as_ref() != Some(&waiting) {
            self.fed_waiting = Some(waiting.clone());
            jobs.extend(self.members(members::Event::Waiting { proposals: waiting }));
        }
        jobs
    }

    /// A proposals event from inside [`Self::feed`] (no feed again: the
    /// caller is feeding).
    fn proposals_raw(&mut self, event: proposals::Event) -> Vec<Job> {
        let pending = self.proposals.dispatch(event);
        self.proposals_view = self.proposals.view();
        tag(Which::Proposals, pending, |op| op)
    }

    /// The multisig shown, if any (the accounts machine's active one).
    #[cfg(all(test, feature = "dev-fixtures"))]
    #[must_use]
    pub fn active(&self) -> Option<&MultisigAccount> {
        let address = self.accounts_view.active.as_deref()?;
        self.accounts_view
            .accounts
            .iter()
            .map(|row| &row.account)
            .find(|a| a.address.eq_ignore_ascii_case(address))
    }

    /// Settings shows this multisig's members (the members machine's `show`),
    /// on its networks, named from this device's accounts and what the
    /// multisigs here keep.
    pub fn show_members(&mut self, account: &MultisigAccount, chain_ids: Vec<u64>) -> Vec<Job> {
        let known: Vec<MultisigAccount> = self
            .accounts_view
            .accounts
            .iter()
            .map(|row| row.account.clone())
            .collect();
        let local = self.facts.accounts.clone();
        self.members(members::Event::Show {
            account: account.clone(),
            chain_ids,
            local,
            known,
        })
    }
}

/// The `known` facts, as the proposals machine reads them (anything that
/// does not read as one is left out).
fn known_of(values: &[Value]) -> Vec<vela_core::multisig_review::Known> {
    values
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

/// The proposals read for the multisig shown, as the members machine's
/// `waiting` (the web's `waitingOf`).
fn waiting_of(view: &ProposalsView) -> Vec<members::WaitingProposal> {
    view.items
        .iter()
        .map(|i| members::WaitingProposal {
            chain_id: i.chain_id,
            safe_tx_hash: i.safe_tx_hash.clone(),
            nonce: i.nonce.to_string(),
            action: i.review.action.clone(),
            approved: i.approved,
            threshold: i.threshold,
            status: match i.status {
                proposals::ProposalStatus::Ready => members::WaitingStatus::Ready,
                proposals::ProposalStatus::Pending => members::WaitingStatus::Pending,
                proposals::ProposalStatus::Queued => members::WaitingStatus::Queued,
                proposals::ProposalStatus::Done => members::WaitingStatus::Done,
            },
        })
        .collect()
}
