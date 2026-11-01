//! Presence, where a waiting transaction stands, the waiting group's note
//! and a proposal's timeline — the web's `presence.test.ts`,
//! `activity*.test.ts` (their logic), `executed.test.ts`
//! (`factsOfExecution`) and `timeline.test.ts`, as the core's rules
//! (`multisig_presence`) and the proposals machine that draws its view with
//! them.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_presence::{
    block_of, facts_of_execution, standing_of, timeline_of, waiting_note, waiting_of, FactsSource,
    FrozenFacts, ListFacts, Look, ReadyBlock, Standing, TimelineEnd, TimelineStep, WaitingNote,
};
use vela_core::app::multisig_proposals::{
    ApprovalRow, Event, MultisigProposals, ProposalItem, ProposalKind, ProposalStatus,
};
use vela_core::app::multisig_shell::MultisigAccount;
use vela_core::multisig::{self, Approval, EcdsaApproval, Transfer};
use world::{fixture, genesis, settle, DeployedSafe, Landing, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

const SAFE: &str = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";
const ME: &str = "0x00000000000000000000000000000000000000aa";
const ALICE: &str = "0x00000000000000000000000000000000000000a1";
const BOB: &str = "0x00000000000000000000000000000000000000b0";
const CAROL: &str = "0x00000000000000000000000000000000000000c0";

fn hash_n(n: u128) -> String {
    format!("0x{n:064x}")
}

fn item(nonce: u128, status: ProposalStatus) -> ProposalItem {
    let tx = multisig::cancel_tx(SAFE, &format!("0x{nonce:x}")).unwrap();
    ProposalItem {
        chain_id: 100,
        safe_tx_hash: hash_n(nonce),
        review: multisig::review(100, SAFE, &tx).unwrap(),
        tx,
        nonce,
        proposer: ALICE.to_owned(),
        approvals: Vec::new(),
        approved: 0,
        threshold: 2,
        status,
        can_approve: Vec::new(),
        members_here: Vec::new(),
        can_execute: Vec::new(),
        cancelled_by: None,
        same_nonce: Vec::new(),
        executing: false,
        created_ms: 1_000.0,
        unsent: false,
        rename_undeployed: false,
    }
}

fn row(owner: &str, created_ms: f64, counts: bool) -> ApprovalRow {
    ApprovalRow {
        owner: owner.to_owned(),
        counts,
        approval: Approval::Ecdsa(EcdsaApproval {
            owner: owner.to_owned(),
            signature: "0x".to_owned(),
        }),
        created_ms,
        unsent: false,
    }
}

// -- what waits here (presence.test.ts) --------------------------------------------------

/// r4D-7: one needing this device's approval and one ready to execute.
#[test]
fn counts_one_needing_an_approval_here_and_one_ready() {
    let mut needs = item(1, ProposalStatus::Pending);
    needs.can_approve = vec![ME.to_owned()];
    needs.approved = 1;
    let mut ready = item(0, ProposalStatus::Ready);
    ready.approved = 2;
    ready.can_approve = vec![ME.to_owned()];
    ready.can_execute = vec![ME.to_owned()];
    // Someone else's turn; done; voided: nothing for this device.
    let queued = item(2, ProposalStatus::Queued);
    let mut done = item(3, ProposalStatus::Done);
    done.can_approve = vec![ME.to_owned()];
    let mut voided = item(4, ProposalStatus::Pending);
    voided.can_approve = vec![ME.to_owned()];
    voided.cancelled_by = Some("0x1".to_owned());
    let w = waiting_of(&[needs, ready, queued, done, voided], |_| None);
    assert_eq!(w.needs_you, 1);
    assert_eq!(w.ready, 1);
    assert_eq!(w.first.unwrap().safe_tx_hash, hash_n(1));
}

/// r5reg-6: "ready to execute" counts only what someone here can run now.
#[test]
fn a_ready_one_nobody_here_can_pay_for_is_not_ready() {
    let ready = |n: u128, here: bool| {
        let mut i = item(n, ProposalStatus::Ready);
        i.approved = 2;
        if here {
            i.can_execute = vec![ME.to_owned()];
        }
        i
    };
    let items = [
        ready(0, true),
        ready(1, true),
        ready(2, false),
        ready(3, true),
    ];
    let looks = |i: &ProposalItem| match i.nonce {
        0 => vec![Some(Look::Priced { can_pay: true })],
        1 => vec![Some(Look::Priced { can_pay: false })],
        _ => vec![None],
    };
    let w = waiting_of(&items, |i| block_of(&looks(i)));
    // The one that can be paid, and the one whose look is still out.
    assert_eq!(w.ready, 2);
    assert_eq!(w.waiting_payer, 2);
    // Without looks, a ready one with a member here counts as ready.
    let w = waiting_of(&[ready(0, true)], |_| None);
    assert_eq!((w.ready, w.waiting_payer), (1, 0));
}

/// `blockOf`: any look that fails; every executor priced and none can pay;
/// otherwise nothing is said.
#[test]
fn why_a_ready_one_cannot_run() {
    let short = serde_json::json!({"kind": "short", "token": null, "have": "1", "need": "5"});
    let fails = Some(Look::Fails(short.clone()));
    let broke = Some(Look::Priced { can_pay: false });
    let pays = Some(Look::Priced { can_pay: true });
    // W9: the row's "Can't run now" says the look's own detail.
    assert_eq!(
        block_of(&[pays.clone(), fails]),
        Some(ReadyBlock::Fails { why: short.clone() })
    );
    assert_eq!(
        block_of(&[broke.clone(), broke.clone()]),
        Some(ReadyBlock::NobodyPays)
    );
    assert_eq!(block_of(&[broke.clone(), pays]), None);
    assert_eq!(block_of(&[broke, None]), None, "a look still out");
    assert_eq!(block_of(&[Some(Look::Other)]), None);
    assert_eq!(block_of(&[]), None);
    // The detail crosses the wire whole, under its kind.
    assert_eq!(
        serde_json::to_value(ReadyBlock::Fails { why: short.clone() }).unwrap(),
        serde_json::json!({"kind": "fails", "why": short})
    );
    assert_eq!(
        serde_json::to_value(ReadyBlock::NobodyPays).unwrap(),
        serde_json::json!({"kind": "nobodyPays"})
    );
}

// -- a waiting row's one word, and the group's note (activity*.test.ts) -------------------

/// r4D-13, r4b-7, r4b-17, r4-5: one word on where it stands.
#[test]
fn a_waiting_row_says_one_thing_about_where_it_stands() {
    let mut unsent = item(0, ProposalStatus::Pending);
    unsent.unsent = true;
    unsent.executing = true;
    assert_eq!(standing_of(&unsent, None), Standing::Unsent);
    let mut executing = item(0, ProposalStatus::Ready);
    executing.executing = true;
    assert_eq!(standing_of(&executing, None), Standing::Executing);
    assert_eq!(
        standing_of(&item(1, ProposalStatus::Queued), None),
        Standing::Queued
    );
    let mut ready = item(0, ProposalStatus::Ready);
    ready.approved = 3;
    assert_eq!(standing_of(&ready, None), Standing::Ready);
    assert_eq!(
        standing_of(&ready, Some(ReadyBlock::NobodyPays)),
        Standing::Blocked {
            block: ReadyBlock::NobodyPays
        }
    );
    let mut mine = item(0, ProposalStatus::Pending);
    mine.can_approve = vec![ME.to_owned()];
    assert_eq!(standing_of(&mine, None), Standing::YourTurn);
    let mut over = item(0, ProposalStatus::Pending);
    over.approved = 3;
    over.threshold = 2;
    assert_eq!(
        standing_of(&over, None),
        Standing::Progress {
            approved: 2,
            threshold: 2
        },
        "never 3/2"
    );
}

/// r4b-15, r4b-3, r4b-11, r4b-2: what the waiting group says, first match wins.
#[test]
fn the_waiting_group_says_where_its_list_stands() {
    let base = ListFacts::default();
    assert_eq!(waiting_note(&base), None);
    let offline = ListFacts {
        offline: true,
        stale: true,
        unreachable: true,
        read_at: Some(5.0),
        ..base.clone()
    };
    let note = waiting_note(&offline).unwrap();
    assert_eq!(note, WaitingNote::Offline { since: Some(5.0) });
    assert!(!note.retry(), "offline has no Retry");
    let away = ListFacts {
        unreachable: true,
        stale: true,
        unreadable: vec![100],
        ..base.clone()
    };
    assert_eq!(waiting_note(&away), Some(WaitingNote::Unreachable));
    let unread = ListFacts {
        stale: true,
        unreadable: vec![100],
        ..base.clone()
    };
    let note = waiting_note(&unread).unwrap();
    assert_eq!(
        note,
        WaitingNote::ChainUnreachable {
            chain_ids: vec![100]
        }
    );
    assert!(note.retry());
    let cached = ListFacts {
        stale: true,
        cached_at: Some(7.0),
        ..base.clone()
    };
    assert_eq!(
        waiting_note(&cached),
        Some(WaitingNote::LastKnown { at: 7.0 })
    );
    let history = ListFacts {
        history_unreadable: vec![100],
        ..base.clone()
    };
    assert!(waiting_note(&history).unwrap().retry());
    let loading = ListFacts {
        loading: true,
        nothing_shown: true,
        ..base.clone()
    };
    assert_eq!(waiting_note(&loading), Some(WaitingNote::Checking));
    // Over rows, the rows speak: "checking" only with nothing shown (W9).
    let over_rows = ListFacts {
        loading: true,
        ..base
    };
    assert_eq!(waiting_note(&over_rows), None);
}

// -- what an execution from this device counts (executed.test.ts) ------------------------

#[test]
fn an_executor_who_approved_already_counts_once() {
    let f = facts_of_execution(&[ALICE.into(), CAROL.into()], CAROL, true, 2, 1.0);
    assert_eq!(f.approvers, vec![ALICE, CAROL]);
    assert!(f.executor_approved);
}

#[test]
fn never_more_than_the_threshold_in_address_order() {
    let f = facts_of_execution(&[CAROL.into(), BOB.into(), ALICE.into()], BOB, true, 2, 1.0);
    assert_eq!(f.approvers, vec![ALICE, BOB]);
    assert_eq!(f.approvers.len() as u64, f.threshold);
}

#[test]
fn an_executor_that_is_no_owner_adds_no_approval() {
    let other = "0x9999999999999999999999999999999999999999";
    let f = facts_of_execution(&[ALICE.into(), BOB.into()], other, false, 2, 1.0);
    assert_eq!(f.approvers, vec![ALICE, BOB]);
    assert!(!f.executor_approved);
}

// -- an executed transaction's story (timeline.test.ts) ----------------------------------

const TX: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn executed() -> ProposalItem {
    let mut i = item(0, ProposalStatus::Done);
    // Given out of order; Carol's is not a member's now.
    i.approvals = vec![
        row(BOB, 3_000.0, true),
        row(ALICE, 2_000.0, true),
        row(CAROL, 2_500.0, false),
    ];
    i
}

fn frozen(executor: &str, approvers: &[&str], executor_approved: bool) -> FrozenFacts {
    FrozenFacts {
        executor: Some(executor.to_owned()),
        threshold: approvers.len() as u64,
        approvers: approvers.iter().map(|a| (*a).to_owned()).collect(),
        executor_approved,
        source: FactsSource::Chain,
        at_ms: Some(9_000.0),
    }
}

fn approved(who: &str, at: Option<f64>, counted: Option<bool>, by_executing: bool) -> TimelineStep {
    TimelineStep::Approved {
        who: who.to_owned(),
        at_ms: at,
        counted,
        by_executing,
        unsent: false,
        on_chain: false,
    }
}

#[test]
fn created_each_approval_in_order_then_executed() {
    let end = TimelineEnd::Executed {
        tx_hash: TX.to_owned(),
        frozen: Some(frozen(BOB, &[ALICE, BOB], true)),
    };
    assert_eq!(
        timeline_of(&executed(), &end),
        vec![
            TimelineStep::Created {
                who: ALICE.to_owned(),
                at_ms: Some(1_000.0)
            },
            approved(ALICE, Some(2_000.0), Some(true), false),
            approved(BOB, Some(3_000.0), Some(true), false),
            TimelineStep::Executed {
                who: Some(BOB.to_owned()),
                tx_hash: TX.to_owned(),
                at_ms: Some(9_000.0)
            },
        ]
    );
}

#[test]
fn an_approval_given_but_not_counted_when_it_ran_says_so() {
    let end = TimelineEnd::Executed {
        tx_hash: TX.to_owned(),
        frozen: Some(frozen(CAROL, &[ALICE], false)),
    };
    let steps: Vec<TimelineStep> = timeline_of(&executed(), &end)
        .into_iter()
        .filter(|s| matches!(s, TimelineStep::Approved { .. }))
        .collect();
    assert_eq!(
        steps,
        vec![
            approved(ALICE, Some(2_000.0), Some(true), false),
            approved(BOB, Some(3_000.0), Some(false), false),
        ]
    );
}

#[test]
fn the_executor_approving_by_sending_it_is_a_step_of_its_own() {
    let mut i = executed();
    i.approvals = vec![row(ALICE, 2_000.0, true)];
    let mut f = frozen(BOB, &[ALICE, BOB], true);
    f.source = FactsSource::Device;
    let end = TimelineEnd::Executed {
        tx_hash: TX.to_owned(),
        frozen: Some(f),
    };
    assert_eq!(
        timeline_of(&i, &end)[2],
        approved(BOB, Some(9_000.0), Some(true), true)
    );
}

#[test]
fn before_its_facts_are_read_nothing_is_made_up() {
    let mut i = executed();
    i.created_ms = 0.0;
    let end = TimelineEnd::Executed {
        tx_hash: TX.to_owned(),
        frozen: None,
    };
    let steps = timeline_of(&i, &end);
    assert_eq!(
        steps[0],
        TimelineStep::Created {
            who: ALICE.to_owned(),
            at_ms: None
        }
    );
    let counted: Vec<Option<bool>> = steps
        .iter()
        .filter_map(|s| match s {
            TimelineStep::Approved { counted, .. } => Some(*counted),
            _ => None,
        })
        .collect();
    assert_eq!(counted, vec![None, None]);
    assert_eq!(
        steps.last().unwrap(),
        &TimelineStep::Executed {
            who: None,
            tx_hash: TX.to_owned(),
            at_ms: None
        }
    );
}

/// An open one's story: kept (unsent) approvals are steps, said as kept; a
/// cancellation waiting at its number ends it; a replaced one says so.
#[test]
fn an_open_story_says_what_is_kept_and_what_waits() {
    let mut i = item(0, ProposalStatus::Pending);
    let mut kept = row(BOB, 4_000.0, true);
    kept.unsent = true;
    i.approvals = vec![row(ALICE, 2_000.0, true), kept, row(CAROL, 3_000.0, false)];
    i.cancelled_by = Some(hash_n(9));
    let steps = timeline_of(&i, &TimelineEnd::Open);
    assert_eq!(
        steps.len(),
        4,
        "created, Alice, Bob (kept), the cancellation"
    );
    assert!(matches!(
        &steps[2],
        TimelineStep::Approved { who, unsent: true, counted: Some(true), .. } if who == BOB
    ));
    assert_eq!(steps[3], TimelineStep::CancelProposed { by: hash_n(9) });
    let replaced = timeline_of(
        &item(0, ProposalStatus::Done),
        &TimelineEnd::Replaced {
            by: Some(hash_n(9)),
            cancellation: true,
        },
    );
    assert_eq!(
        replaced.last().unwrap(),
        &TimelineStep::Replaced {
            by: Some(hash_n(9)),
            cancellation: true
        }
    );
}

// -- the machine ---------------------------------------------------------------------------

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

fn account_of(salt: &str) -> MultisigAccount {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = genesis(&refs, 2, salt);
    serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap()
}

fn stored(account: &MultisigAccount, chain: u64, tx: &multisig::SafeTx) -> Value {
    json!({
        "chainId": chain, "safe": account.address,
        "safeTxHash": multisig::safe_tx_hash(chain, &account.address, tx).unwrap(),
        "safeTx": tx, "proposer": "", "approvals": [], "createdMs": 1, "updatedMs": 1,
    })
}

fn start(sut: &mut Sut, world: &mut World) {
    let ev = Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(sut, world, ev);
}

/// r4b-11 × r4D-7: a multisig not read yet on this page counts what its
/// kept waiting list says — never a zero it did not see, nothing at all
/// when none is kept; the others are read on the shell's tick.
#[test]
fn the_others_start_from_the_kept_list_then_are_read() {
    let shown = account_of("0x31");
    let other = account_of("0x32");
    let unread = account_of("0x33");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let tx = multisig::cancel_tx(&other.address, "0x0").unwrap();
    let kept = json!({
        other.address.to_ascii_lowercase(): {
            "service": SERVICE, "at": 5,
            "chains": [{"chainId": 100,
                "members": {"deployed": false, "owners": other.owners, "threshold": 2, "nonce": "0"},
                "proposals": [stored(&other, 100, &tx)]}],
        }
    });
    world
        .store
        .insert("vela.multisigWaiting".to_owned(), kept.to_string());
    let mut sut = Sut::new();
    start(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(shown.clone()),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Accounts {
            accounts: vec![shown.clone(), other.clone(), unread.clone()],
        },
    );
    let view = sut.view();
    let of = |a: &MultisigAccount| view.presence[&a.address.to_ascii_lowercase()].clone();
    assert_eq!(of(&other).unwrap().needs_you, 1, "the kept list");
    assert_eq!(of(&unread), None, "never a zero it did not see");
    assert_eq!(of(&shown).unwrap().needs_you, 0, "read: nothing waits");
    assert!(view.others_waiting);

    // The shell's tick reads them: the other's list is gone, the third has one.
    let tx3 = multisig::cancel_tx(&unread.address, "0x0").unwrap();
    world.mailbox.push(stored(&unread, 100, &tx3));
    run(&mut sut, &mut world, Event::OthersTick);
    let view = sut.view();
    let of = |a: &MultisigAccount| view.presence[&a.address.to_ascii_lowercase()].clone();
    assert_eq!(of(&other).unwrap().needs_you, 0);
    assert_eq!(of(&unread).unwrap().needs_you, 1);
    let first = of(&unread).unwrap().first.unwrap();
    assert_eq!(
        first.safe_tx_hash,
        multisig::safe_tx_hash(100, &unread.address, &tx3).unwrap()
    );
}

/// During an outage the others are not read: what was said stays.
#[test]
fn the_others_are_not_read_while_the_service_is_away() {
    let shown = account_of("0x31");
    let other = account_of("0x32");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let mut sut = Sut::new();
    start(&mut sut, &mut world);
    world.service_down = true;
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(shown.clone()),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Accounts {
            accounts: vec![shown, other.clone()],
        },
    );
    world.service_down = false;
    let tx = multisig::cancel_tx(&other.address, "0x0").unwrap();
    world.mailbox.push(stored(&other, 100, &tx));
    let before = world.http_log.len();
    run(&mut sut, &mut world, Event::OthersTick);
    assert_eq!(
        world.http_log.len(),
        before,
        "nothing asked during the outage"
    );
    assert_eq!(
        sut.view().presence[&other.address.to_ascii_lowercase()],
        None
    );
}

/// The followed one: a proposal waiting on a member here, then ready, its
/// one word and its story; executed here, its story ends with who sent it
/// and the transaction (the facts this device recorded).
#[test]
fn the_followed_ones_presence_standing_and_timeline() {
    let account = account_of("0x31");
    let mut world = World {
        locals: vec![fixture(0), fixture(1)],
        ..World::default()
    };
    let mut sut = Sut::new();
    start(&mut sut, &mut world);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::Transfers {
                transfers: vec![Transfer {
                    token: None,
                    to: "0x00000000000000000000000000000000000000bb".to_owned(),
                    amount: "1".to_owned(),
                }],
            },
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let view = sut.view();
    let hash = view.items[0].safe_tx_hash.clone();
    let key = format!("100:{hash}");
    let me = account.address.to_ascii_lowercase();
    assert_eq!(view.presence[&me].clone().unwrap().needs_you, 1);
    assert_eq!(view.standings[&key], Standing::YourTurn);
    assert!(matches!(
        view.timelines[&key][..],
        [TimelineStep::Created { .. }, TimelineStep::Approved { .. }]
    ));

    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    let view = sut.view();
    assert_eq!(view.presence[&me].clone().unwrap().ready, 1);
    assert_eq!(view.standings[&key], Standing::Ready);

    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    let tx_hash = format!("0x{}", "cd".repeat(32));
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.clone(),
        tx_hash: tx_hash.clone(),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    assert!(world.store.contains_key("vela.multisig.executed"));
    run(
        &mut sut,
        &mut world,
        Event::Timeline {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
        },
    );
    let view = sut.view();
    assert_eq!(view.presence[&me].clone().unwrap().ready, 0);
    let steps = &view.timelines[&key];
    assert_eq!(
        steps.last().unwrap(),
        &TimelineStep::Executed {
            who: Some(fixture(0).address),
            tx_hash,
            at_ms: steps.last().and_then(|s| match s {
                TimelineStep::Executed { at_ms, .. } => *at_ms,
                _ => None,
            }),
        }
    );
    assert!(steps
        .iter()
        .filter(|s| matches!(s, TimelineStep::Approved { .. }))
        .all(|s| matches!(
            s,
            TimelineStep::Approved {
                counted: Some(true),
                ..
            }
        )));
}

// -- what an executed proposal writes in Activity (records.ts executionRows, W9) ----------

/// One row per transfer; a transfer or a batch of transfers alone is said by
/// its transfers; anything else adds one row titled as its proposal, moving
/// only the coin a call sends, to what it calls.
#[test]
fn an_execution_writes_one_row_per_transfer_else_one_for_its_proposal() {
    use vela_core::app::multisig_proposals::execution_rows;
    let with = |action: Value| {
        let mut i = item(0, ProposalStatus::Done);
        i.review.action = Some(serde_json::from_value(action).unwrap());
        i.tx.to = CAROL.to_owned();
        i
    };
    let send = with(json!({"kind": "transfer", "token": null, "to": BOB, "amount": "0x10"}));
    let rows = execution_rows(SAFE, &send);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["kind"], "send");
    assert_eq!(rows[0]["amount"], "16", "base units, decimal");
    let transfer = json!({"kind": "transfer", "token": CAROL, "to": BOB, "amount": "5"});
    let leg = |action: Value| json!({"to": CAROL, "value": "0", "data": "0x", "action": action});
    let only =
        with(json!({"kind": "batch", "calls": [leg(transfer.clone()), leg(transfer.clone())]}));
    let rows = execution_rows(SAFE, &only);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert!(rows.iter().all(|r| r["kind"] == "send"));
    let call = json!({"kind": "call", "to": ALICE, "value": "0x0", "data": "0x095ea7b3"});
    let mixed = with(json!({"kind": "batch", "calls": [leg(transfer), leg(call)]}));
    let rows = execution_rows(SAFE, &mixed);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[1]["kind"], "call");
    assert_eq!(rows[1]["calls"], 2);
    assert_eq!(rows[1]["to"], CAROL, "a batch: its own target");
    let paying = with(json!({"kind": "call", "to": ALICE, "value": "0x5", "data": "0x"}));
    let rows = execution_rows(SAFE, &paying);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["sends"], "5");
    assert_eq!(rows[0]["contract"], ALICE.to_ascii_lowercase());
    let change = with(json!({"kind": "changeThreshold", "threshold": "1"}));
    let rows = execution_rows(SAFE, &change);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["to"], SAFE, "a member change: the multisig itself");
    assert_eq!(rows[0]["sends"], "0");
}
