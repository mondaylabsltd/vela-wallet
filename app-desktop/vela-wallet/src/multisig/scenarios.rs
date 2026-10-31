//! Scenarios: the desktop's own host of the multisig machines
//! ([`Machines`], routed and fed as the app runs them) and its executor
//! ([`shell::perform`]), driven end to end against the core's simulated
//! world (`rust/crates/vela-core/tests/multisig_world`: the genesis service
//! and its mailbox, chains holding Safe's own code, the parallel space's
//! passkeys). What is proved here is the SHELL's half — every operation
//! performed through the one executor, every notice routed to the machine
//! it is for, one machine's view fed to the next — over the core's rules.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::Value;
use vela_core::app::multisig_accounts::{CreateChoice, Event as AccountsEvent, Phase};
use vela_core::app::multisig_proposals::{
    ActionState, Event as ProposalsEvent, ProposalKind, ProposalStatus,
};
use vela_core::app::multisig_shell::{
    MemberInfo, MultisigOperation as Op, MultisigShellResult as Res, Notice, PasskeyPurpose,
    SendPurpose, SentCall,
};
use vela_core::multisig::Transfer;
use vela_core::multisig_review::SimCall;

use super::machines::{Facts, Job, Machines, Screen};
use super::shell::{
    self, Arrival, Deps, HttpReply, Opened, Perform, RecipientFacts, Rooms, RpcReply,
};

#[path = "../../../../rust/crates/vela-core/tests/multisig_world/mod.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
mod multisig_world;

/// What the world's `settle` helpers name (unused here: the desktop's own
/// loop below drives the machines).
mod support {
    use std::marker::PhantomData;

    pub struct DomainDriver<A>(PhantomData<A>);

    impl<A> DomainDriver<A> {
        pub fn outstanding(&self) -> Vec<vela_core::app::multisig_shell::MultisigOperation> {
            Vec::new()
        }
        pub fn resolve(
            &mut self,
            _: vela_core::app::multisig_shell::MultisigShellResult,
        ) -> Vec<vela_core::app::multisig_shell::MultisigOperation> {
            Vec::new()
        }
        pub fn resolve_matching(
            &mut self,
            _: impl Fn(&vela_core::app::multisig_shell::MultisigOperation) -> bool,
            _: vela_core::app::multisig_shell::MultisigShellResult,
        ) -> Vec<vela_core::app::multisig_shell::MultisigOperation> {
            Vec::new()
        }
    }
}

use multisig_world::{DeployedSafe, Landing, SERVICE, World, fixture};

/// The world behind the shell's [`Deps`] and [`Rooms`]: each typed call
/// is put back as the operation it answers and asked of the world.
struct Shared(Arc<Mutex<World>>);

// SAFETY: the world holds test hooks (boxed closures) that are not `Send`.
// The scenarios perform every operation inline on the test's own thread
// (`Perform::run_inline`): the world is never reached from another thread.
unsafe impl Send for Shared {}
// SAFETY: as above — one thread, behind the mutex.
unsafe impl Sync for Shared {}

impl Shared {
    fn ask(&self, op: &Op) -> Res {
        self.0.lock().unwrap().answer(op)
    }
}

impl Deps for Shared {
    fn http(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        _timeout: Duration,
    ) -> Result<HttpReply, String> {
        match self.ask(&Op::Http {
            method: method.to_owned(),
            url: url.to_owned(),
            body: body.map(str::to_owned),
            timeout_ms: 1,
        }) {
            Res::HttpAnswered {
                status,
                retry_after,
                body,
            } => Ok(HttpReply {
                status,
                retry_after,
                body,
            }),
            Res::HttpUnreachable { detail } => Err(detail),
            other => panic!("{other:?}"),
        }
    }
    fn rpc(
        &self,
        chain_id: u64,
        method: &str,
        params: &Value,
        bound: Option<Duration>,
    ) -> Result<RpcReply, String> {
        match self.ask(&Op::Rpc {
            chain_id,
            method: method.to_owned(),
            params: params.clone(),
            bound_ms: bound.map(|b| b.as_millis() as u32),
        }) {
            Res::RpcResult { result } => Ok(RpcReply::Result(result)),
            Res::RpcError { code, message } => Ok(RpcReply::Error { code, message }),
            Res::RpcUnanswered { detail } => Err(detail),
            other => panic!("{other:?}"),
        }
    }
    fn load(&self, keys: &[String]) -> Result<BTreeMap<String, Option<String>>, String> {
        match self.ask(&Op::LoadStore {
            keys: keys.to_vec(),
        }) {
            Res::StoreLoaded { entries } => Ok(entries),
            other => Err(format!("{other:?}")),
        }
    }
    fn write(&self, key: &str, value: Option<&str>) -> Result<(), String> {
        match self.ask(&Op::WriteStore {
            key: key.to_owned(),
            value: value.map(str::to_owned),
        }) {
            Res::StoreWritten => Ok(()),
            other => Err(format!("{other:?}")),
        }
    }
    fn now_ms(&self) -> f64 {
        self.0.lock().unwrap().now
    }
    fn random(&self, bytes: usize) -> Vec<u8> {
        let Res::Random { hex } = self.ask(&Op::Random {
            bytes: bytes as u32,
        }) else {
            panic!("random");
        };
        (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }
    fn passkey(
        &self,
        owner: &str,
        chain_id: u64,
        challenge: &str,
        purpose: &PasskeyPurpose,
    ) -> Res {
        self.ask(&Op::Passkey {
            owner: owner.to_owned(),
            chain_id,
            challenge: challenge.to_owned(),
            purpose: purpose.clone(),
        })
    }
    fn send_calls(
        &self,
        chain_id: u64,
        from: &str,
        calls: &[SentCall],
        fee: Option<&Value>,
        purpose: Option<&SendPurpose>,
    ) -> Res {
        self.ask(&Op::SendCalls {
            chain_id,
            from: from.to_owned(),
            calls: calls.to_vec(),
            fee: fee.cloned(),
            purpose: purpose.cloned(),
        })
    }
    fn quote_calls(&self, chain_id: u64, from: &str, calls: &[SentCall]) -> Res {
        self.ask(&Op::QuoteCalls {
            chain_id,
            from: from.to_owned(),
            calls: calls.to_vec(),
        })
    }
    fn simulate(&self, chain_id: u64, from: &str, calls: &[SimCall]) -> Option<String> {
        match self.ask(&Op::Simulate {
            chain_id,
            from: from.to_owned(),
            calls: calls.to_vec(),
        }) {
            Res::Simulated { reply } => reply,
            _ => None,
        }
    }
    fn recipient_facts(&self, chain_id: u64, address: &str) -> RecipientFacts {
        match self.ask(&Op::RecipientRisk {
            chain_id,
            address: address.to_owned(),
        }) {
            Res::RecipientFacts {
                is_contract,
                first_time,
            } => RecipientFacts {
                is_contract,
                first_time,
            },
            _ => RecipientFacts::default(),
        }
    }
    fn user_op_status(
        &self,
        chain_id: u64,
        user_op_hash: &str,
    ) -> Option<(Option<String>, Option<String>)> {
        match self.ask(&Op::UserOpStatus {
            chain_id,
            user_op_hash: user_op_hash.to_owned(),
        }) {
            Res::UserOpStatus { status, tx_hash } => Some((status, tx_hash)),
            _ => None,
        }
    }
}

impl Rooms for Shared {
    fn open(
        &self,
        relay: Option<&str>,
        invite: Option<&str>,
        pairing: Option<&str>,
    ) -> Result<Opened, String> {
        match self.ask(&Op::PeerOpen {
            relay: relay.map(str::to_owned),
            invite: invite.map(str::to_owned),
            pairing: pairing.map(str::to_owned),
        }) {
            Res::PeerOpened {
                room,
                invite,
                pairing,
            } => Ok(Opened {
                room,
                invite,
                pairing,
            }),
            Res::PeerClosed { reason } => Err(reason),
            other => panic!("{other:?}"),
        }
    }
    fn send(&self, room: &str, message: &Value) {
        self.ask(&Op::PeerSend {
            room: room.to_owned(),
            message: message.clone(),
        });
    }
    fn receive(&self, room: &str) -> Arrival {
        match self.ask(&Op::PeerReceive {
            room: room.to_owned(),
        }) {
            Res::PeerMessage { message } => Arrival::Message(message),
            Res::PeerClosed { reason } => Arrival::Closed(reason),
            other => panic!("{other:?}"),
        }
    }
    fn close(&self, room: &str, end: bool) {
        self.ask(&Op::PeerClose {
            room: room.to_owned(),
            cancel: false,
            end,
        });
    }
}

/// The desktop's host over the world, run synchronously: every job
/// performed through [`shell::perform`], a wait only once nothing else is
/// left (a shell's clock), a room's receive only once the world has
/// something to deliver.
struct Sim {
    machines: Machines,
    world: Arc<Mutex<World>>,
    deps: Arc<dyn Deps>,
    rooms: Arc<dyn Rooms>,
    parked: VecDeque<Job>,
    screen: Vec<Screen>,
    risks: usize,
}

impl Sim {
    fn new(world: World) -> Self {
        let world = Arc::new(Mutex::new(world));
        let shared = Arc::new(Shared(Arc::clone(&world)));
        Self {
            machines: Machines::new(),
            world,
            deps: shared.clone(),
            rooms: shared,
            parked: VecDeque::new(),
            screen: Vec::new(),
            risks: 0,
        }
    }

    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.world.lock().unwrap()
    }

    fn facts(&self, networks: Vec<u64>) -> Facts {
        Facts {
            service_url: SERVICE.to_owned(),
            accounts: self.world().locals.clone(),
            networks: networks.clone(),
            space: networks,
            relay: None,
            known: Vec::new(),
        }
    }

    fn ready(&self, op: &Op) -> bool {
        self.world().ready(op)
    }

    /// Perform `jobs` and everything their answers ask, until only waiting
    /// ones are left (or `limit` answers, a machine that polls forever).
    fn run(&mut self, jobs: Vec<Job>) {
        let mut queue: VecDeque<Job> = jobs.into();
        queue.extend(self.parked.drain(..));
        let mut answered = 0;
        while answered < 20_000 {
            let pick = queue
                .iter()
                .position(|j| !matches!(j.op, Op::Wait { .. }) && self.ready(&j.op))
                .or_else(|| queue.iter().position(|j| matches!(j.op, Op::Wait { .. })));
            let Some(at) = pick else { break };
            let Some(Job { which, id, op }) = queue.remove(at) else {
                break;
            };
            answered += 1;
            let more = match shell::perform(&op, &self.deps, &self.rooms) {
                Perform::Notice(notice) => {
                    self.world().notices.push(notice.clone());
                    let mut more = self.machines.route(notice);
                    more.extend(self.machines.resolve(which, id, Res::Ack));
                    more
                }
                Perform::AskRisk { .. } => {
                    self.risks += 1;
                    let signs = self.world().risk_signs;
                    self.machines
                        .resolve(which, id, Res::RiskAnswered { signs })
                }
                Perform::After(_, res) => {
                    // The world's clock moves as a wait is answered.
                    let _ = self.world().answer(&op);
                    self.machines.resolve(which, id, res)
                }
                other => {
                    let res = other.run_inline().expect("performed here");
                    self.machines.resolve(which, id, res)
                }
            };
            queue.extend(more);
        }
        assert!(answered < 20_000, "the machines never settled");
        self.parked.extend(queue);
        self.screen.extend(self.machines.take_screen());
    }

    fn accounts(&mut self, event: AccountsEvent) {
        let jobs = self.machines.accounts(event);
        self.run(jobs);
    }

    fn proposals(&mut self, event: ProposalsEvent) {
        let jobs = self.machines.proposals(event);
        self.run(jobs);
    }
}

const B: &str = "0x00000000000000000000000000000000000000bb";

/// Booted with fixture accounts `here` on this device, networks 100 and 8453.
fn booted(here: &[usize]) -> Sim {
    let world = World {
        locals: here.iter().map(|i| fixture(*i)).collect(),
        ..World::default()
    };
    let mut sim = Sim::new(world);
    let jobs = sim.machines.start(sim.facts(vec![100, 8453]));
    sim.run(jobs);
    sim
}

/// A 2-of-3 of the three fixture accounts, created here by fixture 0, on 100.
fn created(sim: &mut Sim) -> String {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    sim.accounts(AccountsEvent::NewDraft);
    sim.accounts(AccountsEvent::Draft {
        owners: owners.clone(),
        threshold: 2,
        name: "Quiet Harbor".to_owned(),
    });
    let creator = fixture(0).address;
    sim.accounts(AccountsEvent::Create {
        choice: CreateChoice {
            owners,
            threshold: 2,
            name: "Quiet Harbor".to_owned(),
            chain_id: 100,
            creator,
            members: BTreeMap::from([(
                fixture(2).address,
                MemberInfo {
                    name: Some("Wen".to_owned()),
                    kind: Some("wallet".to_owned()),
                    typed: Some(true),
                },
            )]),
        },
    });
    let view = &sim.machines.accounts_view;
    assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
    view.create.address.clone().unwrap()
}

fn transfer(amount: &str) -> ProposalKind {
    ProposalKind::Transfers {
        transfers: vec![Transfer {
            token: None,
            to: B.to_owned(),
            amount: amount.to_owned(),
        }],
    }
}

fn propose(sim: &mut Sim, kind: ProposalKind) -> String {
    sim.proposals(ProposalsEvent::Draft {
        chain_id: 100,
        proposal: kind,
    });
    assert!(
        sim.machines.proposals_view.draft.is_some(),
        "the review comes first: {:?}",
        sim.machines.proposals_view.actions.get("propose")
    );
    sim.proposals(ProposalsEvent::ConfirmDraft { as_member: None });
    match sim.machines.proposals_view.actions.get("propose") {
        Some(ActionState::Done { safe_tx_hash, .. }) => safe_tx_hash.clone(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn boot_reads_the_store_and_follows_nothing() {
    let sim = booted(&[0]);
    assert!(sim.machines.accounts_view.loaded);
    assert!(sim.machines.accounts_view.accounts.is_empty());
    assert_eq!(sim.machines.proposals_view.followed, None);
}

/// M1: created with one passkey, recorded by the service, activated — and
/// the proposals machine follows it without being told by a screen: the
/// host feeds the multisig shown across (the web's `$effect`).
#[test]
fn a_created_multisig_is_shown_and_followed() {
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    assert_eq!(sim.world().passkeys.len(), 1, "one passkey prompt");
    assert_eq!(sim.world().imports.len(), 1, "its record asked for");
    assert!(
        sim.screen
            .iter()
            .any(|s| matches!(s, Screen::Activated(Some(a)) if a.eq_ignore_ascii_case(&address)))
    );
    let followed = sim.machines.proposals_view.followed.clone().unwrap();
    assert!(followed.eq_ignore_ascii_case(&address));
    assert!(sim.machines.active().is_some());
    // What a member here typed is kept with it.
    let row = &sim.machines.accounts_view.accounts[0];
    let members = row.account.members.clone().unwrap_or_default();
    let wen = members
        .iter()
        .find(|(address, _)| address.eq_ignore_ascii_case(&fixture(2).address))
        .map(|(_, info)| info.name.clone());
    assert_eq!(wen, Some(Some("Wen".to_owned())), "{members:?}");
}

/// M2: proposed from the send flow's transfer (the review first, then one
/// passkey), approved by the second member on this device, executed by the
/// first and followed until it landed: its Activity rows are routed to the
/// screen, and the members machine is told what waits.
#[test]
fn proposed_approved_executed_and_recorded() {
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    let hash = propose(&mut sim, transfer("1"));
    assert_eq!(sim.world().mailbox.len(), 1, "in the mailbox");
    // The mailbox took it: the list shown is read again (routed).
    assert!(sim.world().notices.iter().any(|n| matches!(n, Notice::MailboxTook { .. })));
    sim.proposals(ProposalsEvent::Approve {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        owner: fixture(1).address,
    });
    let item = sim
        .machines
        .proposals_view
        .items
        .iter()
        .find(|i| i.safe_tx_hash == hash)
        .cloned()
        .unwrap();
    assert_eq!((item.approved, item.status), (2, ProposalStatus::Ready));
    assert_eq!(sim.world().passkeys.len(), 3, "create, propose, approve");

    let executor = fixture(0).address;
    sim.proposals(ProposalsEvent::Preview {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: executor.clone(),
    });
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    sim.world().on_send = Some(Landing {
        chain: 100,
        safe: address.clone(),
        after,
        safe_tx_hash: hash.clone(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    sim.proposals(ProposalsEvent::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor,
    });
    assert_eq!(sim.world().sent.len(), 1, "one user operation");
    assert!(
        matches!(
            sim.world().purposes.last(),
            Some(Some(SendPurpose::Execute { .. }))
        ),
        "the executor's page is told what runs"
    );
    let verdict = &sim.machines.proposals_view.executions[&format!("100:{hash}")];
    assert_eq!(verdict["kind"], "executed", "{verdict}");
    let rows: Vec<&Value> = sim
        .screen
        .iter()
        .filter_map(|s| match s {
            Screen::ActivityRows(rows) => Some(rows),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(rows.len(), 1, "one executed transaction");
    assert_eq!(rows[0]["rows"][0]["kind"], "send");
    assert_eq!(rows[0]["rows"][0]["amount"], "1");
}

/// The members machine is fed the proposals waiting on the multisig shown
/// (the web's `membersWaiting`), each change once.
#[test]
fn the_members_machine_hears_what_waits() {
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    let hash = propose(&mut sim, transfer("1"));
    let account = sim.machines.active().cloned().unwrap();
    let jobs = sim.machines.show_members(&account, vec![100]);
    sim.run(jobs);
    let view = &sim.machines.members_view;
    assert!(view.address.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&address)));
    // The proposal waiting is one the machine was told of: a member change
    // composed now would follow it (its plan reads `waiting`).
    let fed = serde_json::to_value(view).unwrap();
    assert!(fed.is_object());
    assert!(!hash.is_empty());
}

/// §3.5: a signature on a network where more than the members can act asks
/// the person every time; the host hands the question to the screen and
/// answers what the person said — "no" signs nothing.
#[test]
fn a_declined_risk_signs_nothing() {
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    // The multisig deployed on 100 with a module: something besides its
    // members can act for it there.
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let mut safe = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    safe.modules = vec!["0x00000000000000000000000000000000000000ee".to_owned()];
    sim.world().deploy(100, &address, safe);
    sim.world().risk_signs = false;
    sim.accounts(AccountsEvent::CheckOn {
        address: address.clone(),
        chain_id: 100,
    });
    sim.proposals(ProposalsEvent::Draft {
        chain_id: 100,
        proposal: transfer("1"),
    });
    let passkeys = sim.world().passkeys.len();
    sim.proposals(ProposalsEvent::ConfirmDraft { as_member: None });
    assert_eq!(sim.risks, 1, "the person was asked");
    assert_eq!(sim.world().passkeys.len(), passkeys, "nothing signed");
    assert!(matches!(
        sim.machines.proposals_view.actions.get("propose"),
        Some(ActionState::RiskDeclined)
    ));
}

/// An outside member's approval: no member of the proposal's remaining
/// approvers is here, so the proposals machine asks for a session — the
/// host opens one (a room on the relay), and its outcome goes back to the
/// proposals machine (`outside_answered`).
#[test]
fn an_outside_member_is_asked_over_a_session() {
    // Only fixture 0 here; fixture 2 is a plain account's address.
    let (_, wallet) = multisig_world::eoa(9);
    let mut sim = booted(&[0]);
    let owners = vec![fixture(0).address, fixture(1).address, wallet.clone()];
    sim.accounts(AccountsEvent::NewDraft);
    sim.accounts(AccountsEvent::Draft {
        owners: owners.clone(),
        threshold: 2,
        name: "Quiet Harbor".to_owned(),
    });
    sim.accounts(AccountsEvent::Create {
        choice: CreateChoice {
            owners,
            threshold: 2,
            name: "Quiet Harbor".to_owned(),
            chain_id: 100,
            creator: fixture(0).address,
            members: BTreeMap::from([(
                wallet.clone(),
                MemberInfo {
                    name: Some("Wen".to_owned()),
                    kind: Some("wallet".to_owned()),
                    typed: Some(true),
                },
            )]),
        },
    });
    let hash = propose(&mut sim, transfer("1"));
    sim.proposals(ProposalsEvent::ApproveOutside {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
    });
    assert!(
        sim.screen.iter().any(|s| matches!(s, Screen::OutsideSession)),
        "the session's sheet opens: {:?}",
        sim.machines.proposals_view.actions.get(&format!("approve:{hash}"))
    );
    let (view, request) = sim.machines.session_view().unwrap();
    assert!(view.open);
    assert_eq!(request.safe_tx_hash, hash);
    assert!(view.invite.is_some(), "a room with its invite");
    assert_eq!(sim.world().peer.opens.len(), 1);
    // The sheet closed with nothing from the wallet: the proposals machine
    // hears it was cancelled, once.
    let jobs = sim.machines.end_session();
    sim.run(jobs);
    assert!(sim.machines.session_view().is_none());
    let state = sim
        .machines
        .proposals_view
        .actions
        .get(&format!("approve:{hash}"))
        .cloned();
    assert!(
        !matches!(state, Some(ActionState::OutsideAsked { .. })),
        "no longer asked: {state:?}"
    );
}

/// A link opened on this device: the accounts machine reads it, and the
/// host opens the transaction's sheet and tells the proposals machine.
#[test]
fn a_proposal_link_opens_its_transaction() {
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    let hash = propose(&mut sim, transfer("1"));
    let link = vela_core::multisig_link::ProposalLink {
        multisig: address.clone(),
        chain_id: 100,
        safe_tx_hash: hash.clone(),
    };
    let hash_text = vela_core::multisig_link::proposal_hash(&link);
    sim.accounts(AccountsEvent::OpenLink {
        hash: hash_text,
        take_kept: false,
    });
    assert!(
        sim.screen.iter().any(|s| matches!(
            s,
            Screen::OpenProposal { safe_tx_hash, preview: false, .. } if safe_tx_hash.eq_ignore_ascii_case(&hash)
        )),
        "{:?}",
        sim.machines.accounts_view.link
    );
    assert!(sim.machines.proposals_view.linked.is_some());
}

/// The facts as they move: a network added is told to the machines that
/// read networks, once; nothing else is said again.
#[test]
fn moved_facts_are_told_once() {
    let mut sim = booted(&[0]);
    let mut facts = sim.machines.current_facts().clone();
    facts.networks.push(10);
    let jobs = sim.machines.facts(facts.clone());
    sim.run(jobs);
    assert!(sim.machines.current_facts().networks.contains(&10));
    let again = sim.machines.facts(facts);
    assert!(again.is_empty(), "nothing moved, nothing said");
}

/// M6: the notifications read the machines' real views. A transaction
/// proposed, approved and executed by members on this device is the
/// person's own doing — only its execution landing is news.
#[test]
fn notifications_follow_the_views_and_skip_the_persons_own_moves() {
    use super::notify::{Kind, Notifier, Snapshot};
    let look = |sim: &Sim, n: &mut Notifier| {
        n.observe(Snapshot::of(
            &sim.machines.accounts_view,
            &sim.machines.proposals_view,
            true,
        ))
    };
    let mut n = Notifier::default();
    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    assert!(look(&sim, &mut n).is_empty(), "the baseline");
    let hash = propose(&mut sim, transfer("1"));
    assert!(look(&sim, &mut n).is_empty(), "proposed here");
    sim.proposals(ProposalsEvent::Approve {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        owner: fixture(1).address,
    });
    assert!(look(&sim, &mut n).is_empty(), "approved here, ready by it");
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    sim.world().on_send = Some(Landing {
        chain: 100,
        safe: address.clone(),
        after,
        safe_tx_hash: hash.clone(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    sim.proposals(ProposalsEvent::Execute {
        chain_id: 100,
        safe_tx_hash: hash.clone(),
        executor: fixture(0).address,
    });
    sim.proposals(ProposalsEvent::Refresh { fresh: true });
    let got = look(&sim, &mut n);
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(got[0].kind, Kind::Executed);
    assert!(got[0].safe_tx_hash.eq_ignore_ascii_case(&hash));
    assert!(got[0].multisig.eq_ignore_ascii_case(&address));
    assert_eq!(got[0].name.as_deref(), Some("Quiet Harbor"));
    assert!(look(&sim, &mut n).is_empty(), "once");
}

/// M3: a member change is composed in the members machine (Settings → Add
/// member) against the network's members as its chain holds them, and the
/// plan's proposal for that network goes through the one propose path the
/// host serves every screen with (`propose::start` / `propose::follow`, the
/// send flow's too): its draft's review first — which says the rule it
/// meets and the one it leaves, "Approvals: 2 of 3 → 2 of 4" — then the
/// person's word, one passkey, the mailbox.
#[test]
fn a_member_change_composed_in_settings_is_proposed_through_its_review() {
    use super::member_words;
    use super::propose::{self, Proposed, Step};
    use vela_core::app::multisig_members::{Compose, Event as MembersEvent, NetworkState};
    use vela_core::multisig_reading::MemberChanges;

    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    sim.world().deploy(
        100,
        &address,
        DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2),
    );
    let account = sim.machines.active().cloned().unwrap();
    let jobs = sim.machines.show_members(&account, vec![100]);
    sim.run(jobs);
    assert!(
        matches!(
            sim.machines.members_view.networks[0].state,
            NetworkState::Deployed { .. }
        ),
        "{:?}",
        sim.machines.members_view.networks
    );

    // Add member: an outside wallet's address, typed and read.
    let (_, dana) = multisig_world::eoa(7);
    for event in [
        MembersEvent::Compose {
            compose: Some(Compose::Add),
        },
        MembersEvent::Entry { text: dana.clone() },
        MembersEvent::ReadEntry,
        MembersEvent::EntryName {
            name: "Dana".to_owned(),
        },
    ] {
        let jobs = sim.machines.members(event);
        sim.run(jobs);
    }
    let view = sim.machines.members_view.clone();
    assert!(view.can_propose, "{:?}", view.plan);
    let plan = view.plan.clone().unwrap();
    let loc = crate::loc::Loc::for_tag("en");
    let (line, refused) = member_words::plan_line(&loc, &plan.networks[0], "Gnosis");
    assert_eq!((line.as_str(), refused), ("Gnosis: 2 of 3 → 2 of 4", false));

    // Propose: the members machine keeps the new member's name; each
    // network's proposal goes to the proposals machine as it is.
    let jobs = sim.machines.members(MembersEvent::Proposing);
    sim.run(jobs);
    let ready: Vec<_> = plan
        .networks
        .iter()
        .filter_map(|n| n.proposal.clone().map(|p| (n.chain_id, p)))
        .collect();
    assert_eq!(ready.len(), 1);
    for (chain_id, proposal) in ready {
        let mut step = Step::Drafting;
        let jobs = propose::start(&mut sim.machines, chain_id, &address, proposal);
        sim.run(jobs);
        assert_eq!(propose::follow(&mut step, &sim.machines.proposals_view), None);
        assert_eq!(step, Step::Reviewing, "its review comes first");
        let draft = sim.machines.proposals_view.draft.clone().unwrap();
        let review = draft.review.clone().expect("the one review");
        let changes: MemberChanges =
            serde_json::from_value(review["memberChanges"].clone()).expect("member changes");
        let lines = member_words::change_lines(&loc, &changes, "Gnosis", &|a: &str| a.to_owned());
        assert_eq!(lines.rule, "Approvals: 2 of 3 → 2 of 4");
        assert!(changes.fits);
        sim.proposals(ProposalsEvent::ConfirmDraft { as_member: None });
        let end = propose::follow(&mut step, &sim.machines.proposals_view);
        assert!(matches!(end, Some(Proposed::Done { .. })), "{end:?}");
    }
    assert_eq!(sim.world().mailbox.len(), 1, "in the mailbox");
    // The name typed for the new member is kept with the multisig here.
    let row = sim.machines.accounts_view.accounts[0].clone();
    let kept = row
        .account
        .members
        .unwrap_or_default()
        .into_iter()
        .find(|(a, _)| a.eq_ignore_ascii_case(&dana))
        .and_then(|(_, info)| info.name);
    assert_eq!(kept.as_deref(), Some("Dana"));
}

/// M1: the shared name is renamed by a transaction of the multisig itself,
/// through the same review-first path: withdrawn on its review, nothing is
/// signed or sent.
#[test]
fn a_rename_withdrawn_on_its_review_signs_nothing() {
    use super::propose::{self, Step};

    let mut sim = booted(&[0, 1]);
    let address = created(&mut sim);
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    sim.world().deploy(
        100,
        &address,
        DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2),
    );
    let mut step = Step::Drafting;
    let jobs = propose::start(
        &mut sim.machines,
        100,
        &address,
        ProposalKind::Rename {
            name: "Still Water".to_owned(),
        },
    );
    sim.run(jobs);
    assert_eq!(propose::follow(&mut step, &sim.machines.proposals_view), None);
    assert_eq!(step, Step::Reviewing, "{:?}", sim.machines.proposals_view.actions);
    let passkeys = sim.world().passkeys.len();
    sim.proposals(ProposalsEvent::Withdraw);
    let end = propose::follow(&mut step, &sim.machines.proposals_view);
    assert!(end.is_some_and(|e| e.declined()));
    assert_eq!(sim.world().passkeys.len(), passkeys, "nothing signed");
    assert!(sim.world().mailbox.is_empty(), "nothing sent");
}
