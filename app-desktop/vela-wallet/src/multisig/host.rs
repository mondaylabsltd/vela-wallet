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
    /// A proposal on its way through the proposals machine, review first
    /// (the send flow's confirm, Settings' rename and member changes).
    send_propose: Option<Proposing>,
    /// The PeerConnect transport (its Retry; leaving every room).
    peer: Option<Arc<super::peer::PeerRooms>>,
    /// M6: what the views said at the last look, for the notifications.
    notifier: super::notify::Notifier,
    /// When the others' presence was first asked for (ms): a full poll
    /// later it is a live read, never the list an earlier session kept.
    others_since: Option<f64>,
    /// r9-12: when each address the accounts machine wants named
    /// (`index_wanted`) was last asked of the passkey index (ms).
    index_asked: std::collections::BTreeMap<String, f64>,
}

/// How a send flow's proposal ended (its `Proposed` / `ProposeFailed`),
/// and how any proposal did.
pub use super::propose::{ProposeEnd, Proposed};

/// The one proposal being made (the web's `proposalsHost.propose`): where it
/// stands, and who hears how it ended.
struct Proposing {
    step: super::propose::Step,
    reply: Box<dyn FnOnce(Proposed)>,
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
    // The PeerConnect rooms an outside owner's session opens (M4): what a
    // connection says of itself comes back here, as the session's events.
    let (told, mut heard) = futures::channel::mpsc::unbounded::<super::peer::PeerEvent>();
    let rooms = super::peer::PeerRooms::new(Arc::new(move |event| {
        let _ = told.unbounded_send(event);
    }));
    let entity = cx.new(|cx| {
        let mut host = MultisigHost::new(live, rooms.clone(), ceremony, trusted);
        host.peer = Some(rooms);
        host.boot(cx);
        host
    });
    let weak_rooms = entity.downgrade();
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        while let Some(event) = heard.next().await {
            let alive = weak_rooms
                .update(cx, |host, cx| host.peer_event(event, cx))
                .is_ok();
            if !alive {
                break;
            }
        }
    })
    .detach();
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
    super::shown::set(None, cx);
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
            let route =
                match signing_route(&account.signing_plan(), SignatureKind::MultisigApproval) {
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
            index_asked: std::collections::BTreeMap::new(),
            generation: 0,
            send_propose: None,
            peer: None,
            notifier: super::notify::Notifier::default(),
            others_since: None,
        }
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
        // A ceremony's question (a PIN, a touch, a QR) is drawn by the
        // page: redraw while one may be up.
        let asking = self.ceremony.pending_pin().is_some()
            || self.ceremony.pending_choice().is_some()
            || self.ceremony.qr_showing().is_some()
            || self.ceremony.touch_waiting().is_some();
        if asking || self.trusted.waiting() {
            self.touch(cx);
        }
        if !self.machines.accounts_view.loaded {
            return;
        }
        self.ask_index(now, cx);
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
            // The facts as they moved since (an account added, a network,
            // another service in Settings): each change told as its event.
            let multisigs: Vec<(String, String)> = self
                .machines
                .accounts_view
                .accounts
                .iter()
                .map(|row| {
                    let label = row.name.clone().or_else(|| row.account.label.clone());
                    (row.account.address.clone(), label.unwrap_or_default())
                })
                .collect();
            jobs.extend(self.machines.facts(facts_now(&multisigs)));
            jobs.extend(self.machines.accounts(accounts::Event::Tick));
        }
        if now >= self.due.others {
            self.due.others = now + RECORD_EVERY.as_secs_f64() * 1000.0;
            self.others_since.get_or_insert(now);
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
            jobs.extend(
                self.machines
                    .proposals(proposals::Event::Refresh { fresh: false }),
            );
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

    /// r9-12: the passkey index asked about each member the accounts machine
    /// wants named (`index_wanted`), off the UI thread — once per address
    /// every [`INDEX_ASK_AGAIN_MS`] — and its answers handed to the machine
    /// (`index_names`), which names every member from them. An address
    /// nobody answered for is left out: asked again later.
    fn ask_index(&mut self, now: f64, cx: &mut Context<Self>) {
        const INDEX_ASK_AGAIN_MS: f64 = 10_000.;
        let wanted: Vec<String> = self
            .machines
            .accounts_view
            .index_wanted
            .iter()
            .map(|a| a.to_ascii_lowercase())
            .filter(|a| {
                self.index_asked
                    .get(a)
                    .is_none_or(|at| now - at >= INDEX_ASK_AGAIN_MS)
            })
            .collect();
        if wanted.is_empty() {
            return;
        }
        for address in &wanted {
            self.index_asked.insert(address.clone(), now);
        }
        let lookup = cx.background_executor().spawn(async move {
            wanted
                .into_iter()
                .filter_map(|address| {
                    let answer = crate::executor::registry::index_answer(&address)?;
                    Some((
                        address,
                        accounts::IndexAnswer {
                            vela: answer.is_some(),
                            name: answer,
                        },
                    ))
                })
                .collect::<std::collections::BTreeMap<_, _>>()
        });
        cx.spawn(async move |this, cx| {
            let answers = lookup.await;
            if answers.is_empty() {
                return;
            }
            let _ = this.update(cx, |host, cx| {
                host.accounts(accounts::Event::IndexNames { answers }, cx);
            });
        })
        .detach();
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

    /// What a room said of itself, for the session open now.
    fn peer_event(&mut self, event: super::peer::PeerEvent, cx: &mut Context<Self>) {
        use super::peer::PeerEvent;
        let event = match event {
            PeerEvent::State { room, state } => session::Event::PeerState { room, state },
            PeerEvent::Paired { room, pairing } => session::Event::Paired { room, pairing },
            PeerEvent::Relay { status, .. } => session::Event::Relay { status },
        };
        self.session(event, cx);
    }

    /// The person's Retry on the relay: try it again now, every room.
    pub fn retry_relay(&mut self, cx: &mut Context<Self>) {
        if let Some(peer) = &self.peer {
            for room in self.machines.session_rooms() {
                peer.retry(&room);
            }
        }
        self.touch(cx);
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
        self.follow_send_propose(cx);
        self.sync_shown(cx);
        self.notice_moves(cx);
        self.touch(cx);
    }

    /// M6: what moved since the last look, posted as notifications — only
    /// while the person has them on (`notify::deliver::wanted`). Off, the
    /// memory goes, so turning them on again starts from a silent baseline.
    fn notice_moves(&mut self, cx: &mut Context<Self>) {
        if !super::notify::deliver::wanted() {
            if !self.notifier.is_fresh() {
                self.notifier.reset();
            }
            return;
        }
        if !self.machines.accounts_view.loaded {
            return;
        }
        let now = crate::executor::now_ms();
        let live = self
            .others_since
            .is_some_and(|since| now - since >= RECORD_EVERY.as_secs_f64() * 1000.0);
        let snapshot = super::notify::Snapshot::of(
            &self.machines.accounts_view,
            &self.machines.proposals_view,
            live,
        );
        if self.notifier.is_fresh() {
            // r11-10: what is watched — every multisig on this device, the
            // shown one by its list, the others by their presence.
            crate::diag::vlog!(
                "multisig-notify",
                "watching {} multisig(s): the shown one's list at its pace, the others every {} s",
                snapshot.multisigs.len(),
                RECORD_EVERY.as_secs()
            );
        }
        let notices = self.notifier.observe(snapshot);
        super::notify::deliver::post(notices, cx);
    }

    /// The multisig shown, carried to the window (`shown::Shown`): the root
    /// rebuilds the wallet page for it, as an account switch does.
    fn sync_shown(&self, cx: &mut Context<Self>) {
        let view = &self.machines.accounts_view;
        if !view.loaded {
            return;
        }
        // §9.1: one name per address for every surface, the core's. r12-8: a
        // name landing re-titles the executed member changes in Activity,
        // as the sheet says them.
        if super::shown::set_labels(&view.member_labels) {
            reword_member_rows();
        }
        // r10-1: the networks each multisig lives on (its record's), so the
        // hero's "Updated" speaks only for those (§9.3 `lives_on`).
        for row in &view.accounts {
            let chain_ids = row
                .account
                .chain_ids
                .iter()
                .filter_map(|id| u32::try_from(*id).ok())
                .collect();
            crate::executor::balance_dashboard::lives_on(&row.account.address, chain_ids, cx);
        }
        let shown = view.active.as_deref().and_then(|active| {
            view.accounts
                .iter()
                .find(|row| row.account.address.eq_ignore_ascii_case(active))
                .map(|row| super::shown::ShownAccount {
                    address: row.account.address.clone(),
                    name: row.name.clone().or_else(|| row.account.label.clone()),
                    threshold: row.account.threshold,
                    owners: row.account.owners.len() as u64,
                })
        });
        super::shown::set(shown, cx);
    }

    // -- proposing, review first (`ProposeSafeTx`, Settings) -----------------

    /// Propose `transfers` on `chain_id` for the multisig shown, as the send
    /// flow's confirm (§3.6): its review first — drawn by the screen while
    /// [`Self::send_reviewing`] — then the person's word, one passkey, the
    /// mailbox. Answered once, with how it ended.
    pub fn propose_for_send(
        &mut self,
        chain_id: u64,
        multisig: &str,
        transfers: Vec<vela_core::multisig::Transfer>,
        cx: &mut Context<Self>,
    ) -> futures::channel::oneshot::Receiver<ProposeEnd> {
        let (reply, answer) = futures::channel::oneshot::channel();
        self.begin_propose(
            chain_id,
            multisig,
            proposals::ProposalKind::Transfers { transfers },
            Box::new(move |end| {
                let _ = reply.send(super::propose::send_end(&end));
            }),
            cx,
        );
        answer
    }

    /// Propose `proposal` on `chain_id` for `multisig` the same way (the
    /// web's `proposalsHost.propose(.., { review: true })`): Settings'
    /// rename, each network's member change. Its review is drawn by the same
    /// dialog the send flow's is; answered once, with how it ended.
    pub fn propose(
        &mut self,
        chain_id: u64,
        multisig: &str,
        proposal: proposals::ProposalKind,
        cx: &mut Context<Self>,
    ) -> futures::channel::oneshot::Receiver<Proposed> {
        let (reply, answer) = futures::channel::oneshot::channel();
        self.begin_propose(
            chain_id,
            multisig,
            proposal,
            Box::new(move |end| {
                let _ = reply.send(end);
            }),
            cx,
        );
        answer
    }

    fn begin_propose(
        &mut self,
        chain_id: u64,
        multisig: &str,
        proposal: proposals::ProposalKind,
        reply: Box<dyn FnOnce(Proposed)>,
        cx: &mut Context<Self>,
    ) {
        // One at a time: an earlier one nobody finished is withdrawn.
        if let Some(earlier) = self.send_propose.take() {
            (earlier.reply)(Proposed::withdrawn());
        }
        self.send_propose = Some(Proposing {
            step: super::propose::Step::Drafting,
            reply,
        });
        let jobs = super::propose::start(&mut self.machines, chain_id, multisig, proposal);
        self.run(jobs, cx);
    }

    /// A proposal (the send flow's, Settings') is on its review: the screen
    /// draws the draft.
    /// The send flow is proposing (from its draft to its end): a site's
    /// request to a multisig waits for it (AUDIT M5).
    #[must_use]
    pub fn send_proposing(&self) -> bool {
        self.send_propose.is_some()
    }

    /// The send flow's proposal is on its review: the screen draws the draft.
    #[must_use]
    pub fn send_reviewing(&self) -> bool {
        self.send_propose
            .as_ref()
            .is_some_and(|p| p.step == super::propose::Step::Reviewing)
            && self.machines.proposals_view.draft.is_some()
    }

    /// The person's word on the review: propose it, as `as_member`.
    pub fn confirm_draft(&mut self, as_member: Option<String>, cx: &mut Context<Self>) {
        self.proposals(proposals::Event::ConfirmDraft { as_member }, cx);
    }

    /// The person withdrew it on its review: nothing is signed or sent.
    pub fn withdraw_draft(&mut self, cx: &mut Context<Self>) {
        let drawn = self.machines.proposals_view.draft.is_some()
            || matches!(
                self.machines.proposals_view.actions.get("propose"),
                Some(proposals::ActionState::Working)
            );
        if drawn {
            self.proposals(proposals::Event::Withdraw, cx);
        }
        if let Some(p) = self.send_propose.take() {
            (p.reply)(Proposed::withdrawn());
        }
        self.touch(cx);
    }

    /// Where the proposal on its way stands now, from the machine's view.
    fn follow_send_propose(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.send_propose.as_mut() else {
            return;
        };
        let Some(end) = super::propose::follow(&mut p.step, &self.machines.proposals_view) else {
            return;
        };
        let Some(p) = self.send_propose.take() else {
            return;
        };
        // Not proposed: its draft goes too (nothing more is asked or sent).
        if !matches!(end, Proposed::Done { .. }) && self.machines.proposals_view.draft.is_some() {
            let jobs = self.machines.proposals(proposals::Event::Withdraw);
            self.run(jobs, cx);
        }
        (p.reply)(end);
    }

    /// Blocking work on a thread of its own; its answer back on this one.
    /// A thread that dies answers the operation's own failure — never a
    /// machine left waiting.
    fn spawn(&mut self, which: Which, id: u64, op: Op, work: shell::Work, cx: &mut Context<Self>) {
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

/// r12-8: the executed member changes' Activity rows titled again by the
/// core's one rule, off the UI thread (the store is a file).
fn reword_member_rows() {
    let _ = std::thread::Builder::new()
        .name("vela-multisig-reword".to_owned())
        .spawn(|| {
            let loc = crate::loc::Loc::from_env();
            let labels = super::shown::labels();
            let _ = storage::update_list(crate::executor::activity_feed::TX_KEY, |stored| {
                super::activity::reword_member_rows(stored, &loc, &labels) > 0
            });
        });
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
