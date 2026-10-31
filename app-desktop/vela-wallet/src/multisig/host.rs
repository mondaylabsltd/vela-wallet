//! The multisig machines, alive for the process and driven from gpui: the
//! [`Machines`] host, the operations it asks for performed where they belong
//! ([`shell::perform`]), the timers of docs/multisig-machines.md §1.4, and
//! the facts the machines start from (this device's Vela accounts, its
//! networks and space, the service, the relay, the names the person knows).
//!
//! One entity ([`MultisigHost`]), reached through [`host`]: the screens
//! dispatch through it and read its views; nothing here decides.
//!
//! ```text
//!   screen ── event ──► MultisigHost ── Machines (core) ──► Job { which, id, op }
//!                            │                                     │
//!                            │◄──── resolve(which, id, result) ────┤ shell::perform
//!                            │      Now / thread / timer / the person / a notice
//! ```

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use gpui::{App, AppContext as _, Context, Entity, Global};
use serde_json::Value;
use vela_core::app::multisig_accounts::{self as accounts};
use vela_core::app::multisig_members as members;
use vela_core::app::multisig_outside::session;
use vela_core::app::multisig_proposals as proposals;
use vela_core::app::multisig_shell::{
    ApprovalRoute, LocalAccount, MultisigOperation as Op, MultisigShellResult as Res, OwnerKey,
};
use vela_core::multisig_review::{Known, KnownKind};
use vela_core::signing_venue::{SignatureKind, SigningRoute, signing_route};

use super::machines::{Facts, Job, Machines, Screen, Which};
use super::shell::{self, Deps, Perform, Rooms};
use crate::ceremony::CeremonyChannel;
use crate::executor::{storage, trusted_signer, user_op};

/// How often the records that have not landed are asked about (the web's
/// `RECORD_RETRY_MS`) — also the pace of the others' presence
/// (`OTHERS_EVERY_MS`).
const RECORD_EVERY: Duration = Duration::from_secs(60);
/// The outside session's mailbox look (`MAILBOX_EVERY_MS`).
const SESSION_EVERY: Duration = Duration::from_secs(8);
/// The clock the timers are read on.
const HEARTBEAT: Duration = Duration::from_secs(1);

/// The high-risk question asked now (`ask_risk`): the person answers it.
#[derive(Clone, Debug, PartialEq)]
pub struct RiskAsk {
    which: Which,
    id: u64,
    pub chain_id: u64,
    pub safe: String,
    pub findings: Vec<Value>,
}

/// The rooms nobody can open: until the PeerConnect transport is here, a
/// session is said as what it is — no room.
struct NoRooms;

impl Rooms for NoRooms {
    fn open(
        &self,
        _relay: Option<&str>,
        _invite: Option<&str>,
        _pairing: Option<&str>,
    ) -> Result<shell::Opened, String> {
        Err("no PeerConnect transport on this build".to_owned())
    }
    fn send(&self, _room: &str, _message: &Value) {}
    fn receive(&self, _room: &str) -> shell::Arrival {
        shell::Arrival::Closed("closed".to_owned())
    }
    fn close(&self, _room: &str, _end: bool) {}
}

/// The host: the machines and how their operations are performed.
pub struct MultisigHost {
    machines: Machines,
    deps: Arc<dyn Deps>,
    rooms: Arc<dyn Rooms>,
    /// The screen's half of a passkey ceremony (its PIN, its touch, its QR).
    pub ceremony: Arc<CeremonyChannel>,
    /// The trusted page's waiting sheet (a member that signs on a page).
    pub trusted: Arc<trusted_signer::Channel>,
    /// The risk question waiting on the person.
    pub risk: Option<RiskAsk>,
    /// What the machines asked the screen to do, oldest first.
    screen: Vec<Screen>,
    /// When each timer is due next (ms since the epoch).
    due: Due,
    /// The once-a-session jobs ran.
    checked: bool,
    /// Bumped by every change the screens draw: a cheap "did it move".
    pub generation: u64,
}

#[derive(Default)]
struct Due {
    records: f64,
    proposals: f64,
    others: f64,
    session: f64,
}

/// The host, for the process.
struct Handle(Entity<MultisigHost>);

impl Global for Handle {}

/// The host, booted on first use (from the app's facts).
pub fn host(cx: &mut App) -> Entity<MultisigHost> {
    if let Some(handle) = cx.try_global::<Handle>() {
        return handle.0.clone();
    }
    let ceremony = CeremonyChannel::new();
    let (trusted, mut changed) = trusted_signer::Channel::new();
    let live: Arc<dyn Deps> = Arc::new(super::live::Live::new(
        ceremony.ceremony(0),
        Arc::clone(&trusted),
    ));
    let entity = cx.new(|cx| {
        let mut host = MultisigHost::new(live, Arc::new(NoRooms), ceremony, trusted);
        host.boot(cx);
        host
    });
    // The trusted page asked to be opened, or its sheet moved.
    let weak = entity.downgrade();
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        while changed.next().await.is_some() {
            let alive = weak
                .update(cx, |host, cx| {
                    if let Some(url) = host.trusted.take_page() {
                        cx.open_url(&url);
                    }
                    host.touch(cx);
                })
                .is_ok();
            if !alive {
                break;
            }
        }
    })
    .detach();
    cx.set_global(Handle(entity.clone()));
    entity
}

/// The host, if it was booted (a screen that only reads).
#[must_use]
pub fn booted(cx: &App) -> Option<Entity<MultisigHost>> {
    cx.try_global::<Handle>().map(|h| h.0.clone())
}

/// Forget the host (sign-out, an erase): the next use boots from what is
/// on disk then.
pub fn forget(cx: &mut App) {
    if cx.has_global::<Handle>() {
        cx.remove_global::<Handle>();
    }
}

/// This device's Vela accounts, as the multisig machines need them: the
/// address, the name, the founding keys and how each approves (the web's
/// `localAccounts`).
#[must_use]
pub fn local_accounts() -> Vec<LocalAccount> {
    storage::load_accounts()
        .unwrap_or_default()
        .into_iter()
        .map(|account| {
            let route = match signing_route(&account.signing_plan(), SignatureKind::MultisigApproval)
            {
                SigningRoute::InVela => ApprovalRoute::Vela,
                SigningRoute::Page { .. } => ApprovalRoute::Page,
                SigningRoute::Blocked { .. } => ApprovalRoute::Blocked,
            };
            LocalAccount {
                address: account.address.clone(),
                name: account.name.clone(),
                keys: user_op::key_set_of(&account)
                    .into_iter()
                    .map(|k| OwnerKey {
                        credential_id: k.credential_id,
                        public_key_hex: k.public_key_hex,
                    })
                    .collect(),
                route,
            }
        })
        .collect()
}

/// The facts as they stand on disk now.
#[must_use]
pub fn facts_now(multisigs: &[(String, String)]) -> Facts {
    let accounts = local_accounts();
    let mut known: Vec<Known> = accounts
        .iter()
        .map(|a| Known {
            address: a.address.clone(),
            label: a.name.clone(),
            kind: KnownKind::Own,
        })
        .collect();
    for (address, label) in multisigs {
        known.push(Known {
            address: address.clone(),
            label: label.clone(),
            kind: KnownKind::Own,
        });
    }
    for (address, label) in crate::executor::contacts::named_contacts() {
        known.push(Known {
            address,
            label,
            kind: KnownKind::Contact,
        });
    }
    Facts {
        service_url: super::live::service_url(),
        accounts,
        networks: crate::executor::network_admin::all_wallet_chain_ids()
            .into_iter()
            .map(u64::from)
            .collect(),
        space: crate::network_space::space_chain_ids()
            .into_iter()
            .map(u64::from)
            .collect(),
        relay: super::live::relay_url(),
        known: known
            .into_iter()
            .filter_map(|k| serde_json::to_value(k).ok())
            .collect(),
    }
}

impl MultisigHost {
    #[must_use]
    pub fn new(
        deps: Arc<dyn Deps>,
        rooms: Arc<dyn Rooms>,
        ceremony: Arc<CeremonyChannel>,
        trusted: Arc<trusted_signer::Channel>,
    ) -> Self {
        Self {
            machines: Machines::new(),
            deps,
            rooms,
            ceremony,
            trusted,
            risk: None,
            screen: Vec::new(),
            due: Due::default(),
            checked: false,
            generation: 0,
        }
    }

    /// Hand the session machines a transport (the PeerConnect rooms).
    pub fn set_rooms(&mut self, rooms: Arc<dyn Rooms>) {
        self.rooms = rooms;
    }

    fn boot(&mut self, cx: &mut Context<Self>) {
        let jobs = self.machines.start(facts_now(&[]));
        self.run(jobs, cx);
        self.arm(cx);
    }

    /// The heartbeat the timers are read on (§1.4): the machines say when,
    /// this only ticks.
    fn arm(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(HEARTBEAT).await;
                if this.update(cx, |host, cx| host.beat(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn beat(&mut self, cx: &mut Context<Self>) {
        let now = crate::executor::now_ms();
        if !self.machines.accounts_view.loaded {
            return;
        }
        let mut jobs = Vec::new();
        if !self.checked {
            // Once a session: what each shared record is now, and who can
            // act for each multisig on each of its networks.
            self.checked = true;
            jobs.extend(self.machines.accounts(accounts::Event::CheckRecords));
            jobs.extend(self.machines.accounts(accounts::Event::Recheck));
        }
        if now >= self.due.records {
            self.due.records = now + RECORD_EVERY.as_secs_f64() * 1000.0;
            jobs.extend(self.machines.accounts(accounts::Event::Tick));
        }
        if now >= self.due.others {
            self.due.others = now + RECORD_EVERY.as_secs_f64() * 1000.0;
            jobs.extend(self.machines.proposals(proposals::Event::OthersTick));
        }
        if self.machines.proposals_view.followed.is_some() && now >= self.due.proposals {
            let pace = f64::from(self.machines.proposals_view.pace_ms.max(1_000));
            self.due.proposals = now + pace;
            jobs.extend(self.machines.proposals(proposals::Event::Tick));
        }
        if self
            .machines
            .session_view()
            .is_some_and(|(view, _)| view.open && view.outcome.is_none())
            && now >= self.due.session
        {
            self.due.session = now + SESSION_EVERY.as_secs_f64() * 1000.0;
            jobs.extend(self.machines.session(session::Event::MailboxTick));
        }
        if !jobs.is_empty() {
            self.run(jobs, cx);
        }
    }

    /// The window came back (focus, shown again, online): the records and
    /// the list shown are read now (§1.4).
    pub fn shown_again(&mut self, cx: &mut Context<Self>) {
        let mut jobs = self.machines.accounts(accounts::Event::Tick);
        if self.machines.proposals_view.followed.is_some() {
            jobs.extend(self.machines.proposals(proposals::Event::Refresh { fresh: false }));
        }
        self.run(jobs, cx);
    }

    /// The facts moved (an account signed in, a network added, another
    /// service): read again, each change told as its event.
    pub fn refresh_facts(&mut self, cx: &mut Context<Self>) {
        let multisigs: Vec<(String, String)> = self
            .machines
            .accounts_view
            .accounts
            .iter()
            .map(|row| {
                (
                    row.account.address.clone(),
                    row.name
                        .clone()
                        .or_else(|| row.account.label.clone())
                        .unwrap_or_default(),
                )
            })
            .collect();
        let jobs = self.machines.facts(facts_now(&multisigs));
        self.run(jobs, cx);
    }

    // -- reading -----------------------------------------------------------

    #[must_use]
    pub fn machines(&self) -> &Machines {
        &self.machines
    }

    /// What the machines asked the screen to do since the last call.
    pub fn take_screen(&mut self) -> Vec<Screen> {
        std::mem::take(&mut self.screen)
    }

    // -- events ------------------------------------------------------------

    pub fn accounts(&mut self, event: accounts::Event, cx: &mut Context<Self>) {
        let jobs = self.machines.accounts(event);
        self.run(jobs, cx);
    }

    pub fn proposals(&mut self, event: proposals::Event, cx: &mut Context<Self>) {
        let jobs = self.machines.proposals(event);
        self.run(jobs, cx);
    }

    pub fn members(&mut self, event: members::Event, cx: &mut Context<Self>) {
        let jobs = self.machines.members(event);
        self.run(jobs, cx);
    }

    pub fn session(&mut self, event: session::Event, cx: &mut Context<Self>) {
        let jobs = self.machines.session(event);
        self.run(jobs, cx);
    }

    /// The session's sheet closed.
    pub fn end_session(&mut self, cx: &mut Context<Self>) {
        let jobs = self.machines.end_session();
        self.run(jobs, cx);
    }

    /// Settings shows this multisig's members.
    pub fn show_members(&mut self, address: &str, cx: &mut Context<Self>) {
        let Some(account) = self
            .machines
            .accounts_view
            .accounts
            .iter()
            .find(|row| row.account.address.eq_ignore_ascii_case(address))
            .map(|row| row.account.clone())
        else {
            return;
        };
        let mut chain_ids = account.chain_ids.clone();
        chain_ids.sort_unstable();
        chain_ids.dedup();
        let jobs = self.machines.show_members(&account, chain_ids);
        self.run(jobs, cx);
    }

    /// The person's answer to the risk question.
    pub fn answer_risk(&mut self, signs: bool, cx: &mut Context<Self>) {
        let Some(ask) = self.risk.take() else {
            return;
        };
        let jobs = self
            .machines
            .resolve(ask.which, ask.id, Res::RiskAnswered { signs });
        self.run(jobs, cx);
    }

    /// Something the screens draw moved.
    fn touch(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        cx.notify();
    }

    // -- performing --------------------------------------------------------

    /// Perform every job, and every job its answer asks for, until only the
    /// ones that wait (a socket, a timer, the person) are left.
    fn run(&mut self, jobs: Vec<Job>, cx: &mut Context<Self>) {
        let mut queue: VecDeque<Job> = jobs.into();
        while let Some(job) = queue.pop_front() {
            let Job { which, id, op } = job;
            match shell::perform(&op, &self.deps, &self.rooms) {
                Perform::Now(res) => queue.extend(self.machines.resolve(which, id, res)),
                Perform::Notice(notice) => {
                    queue.extend(self.machines.route(notice));
                    queue.extend(self.machines.resolve(which, id, Res::Ack));
                }
                Perform::After(delay, res) => {
                    cx.spawn(async move |this, cx| {
                        cx.background_executor().timer(delay).await;
                        this.update(cx, |host, cx| {
                            let jobs = host.machines.resolve(which, id, res);
                            host.run(jobs, cx);
                        })
                        .ok();
                    })
                    .detach();
                }
                Perform::Blocking(work) => self.spawn(which, id, op, work, cx),
                Perform::AskRisk {
                    chain_id,
                    safe,
                    findings,
                } => {
                    // One question at a time: an earlier one nobody answered
                    // is answered "no" (nothing is signed on a guess).
                    if let Some(earlier) = self.risk.take() {
                        queue.extend(self.machines.resolve(
                            earlier.which,
                            earlier.id,
                            Res::RiskAnswered { signs: false },
                        ));
                    }
                    self.risk = Some(RiskAsk {
                        which,
                        id,
                        chain_id,
                        safe,
                        findings,
                    });
                }
            }
        }
        let screen = self.machines.take_screen();
        for item in screen {
            match item {
                Screen::ActivityRows(rows) => write_rows(rows),
                other => self.screen.push(other),
            }
        }
        self.touch(cx);
    }

    /// Blocking work on a thread of its own; its answer back on this one.
    /// A thread that dies answers the operation's own failure — never a
    /// machine left waiting.
    fn spawn(
        &mut self,
        which: Which,
        id: u64,
        op: Op,
        work: shell::Work,
        cx: &mut Context<Self>,
    ) {
        let (tx, rx) = futures::channel::oneshot::channel();
        let started = std::thread::Builder::new()
            .name("vela-multisig".to_owned())
            .spawn(move || {
                let _ = tx.send(work());
            });
        if started.is_err() {
            let jobs = self
                .machines
                .resolve(which, id, shell::failure_of(&op, "no thread"));
            self.run(jobs, cx);
            return;
        }
        cx.spawn(async move |this, cx| {
            let res = rx
                .await
                .unwrap_or_else(|_| shell::failure_of(&op, "the work stopped"));
            this.update(cx, |host, cx| {
                let jobs = host.machines.resolve(which, id, res);
                host.run(jobs, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// An executed multisig transaction's rows, worded and kept in Activity
/// (the web's `executionRows`): which rows is the core's plan; a token's
/// symbol and decimals are read here. On a thread: the reads are the
/// chain's.
fn write_rows(rows: Vec<Value>) {
    let _ = std::thread::Builder::new()
        .name("vela-multisig-rows".to_owned())
        .spawn(move || {
            let worded = super::activity::worded_rows(&rows);
            if worded.is_empty() {
                return;
            }
            let _ = storage::update_list(crate::executor::activity_feed::TX_KEY, |stored| {
                for row in &worded {
                    let id = row.get("id").cloned();
                    stored.retain(|r| r.get("id") != id.as_ref());
                    stored.push(row.clone());
                }
                true
            });
        });
}
