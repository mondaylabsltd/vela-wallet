//! The proposals machine (`multisig_proposals`), driven against a simulated
//! shell — the web's `proposals`, `gate`, `execute` and `execution-watch`
//! suites: who counts, the mailbox read and its fallbacks, the review before
//! the passkey, the signing gate, the outbox, and executing.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_proposals::{
    ActionState, Event, MultisigProposals, ProposalKind, ProposalStatus,
};
use vela_core::app::multisig_shell::{AccountUpdate, MultisigAccount, Notice};
use vela_core::app::multisig_signers::SignerKind;
use vela_core::multisig::{self, Transfer};
use world::{fixture, genesis, settle, DeployedSafe, Landing, World, HANDLER_141, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

const B: &str = "0x00000000000000000000000000000000000000bb";

fn start(world: &World, networks: Vec<u64>) -> Event {
    Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks,
        known: Vec::new(),
    }
}

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// A 2-of-3 of the three fixture accounts, used on 100; `here`: which of
/// them are on this device.
fn setup(here: &[usize]) -> (World, Sut, MultisigAccount) {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = genesis(&refs, 2, "0x31");
    let account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap();
    let mut world = World {
        locals: here.iter().map(|i| fixture(*i)).collect(),
        ..World::default()
    };
    let mut sut = Sut::new();
    let ev = start(&world, vec![100, 8453]);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    (world, sut, account)
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

fn propose(
    sut: &mut Sut,
    world: &mut World,
    kind: ProposalKind,
    as_member: Option<String>,
) -> ActionState {
    run(
        sut,
        world,
        Event::Draft {
            chain_id: 100,
            proposal: kind,
        },
    );
    if let Some(state) = sut.view().actions.get("propose").cloned() {
        return state;
    }
    assert!(sut.view().draft.is_some());
    run(sut, world, Event::ConfirmDraft { as_member });
    sut.view().actions["propose"].clone()
}

fn hash_of(state: &ActionState) -> String {
    match state {
        ActionState::Done { safe_tx_hash, .. } => safe_tx_hash.clone(),
        other => panic!("{other:?}"),
    }
}

/// A stored proposal of `tx`, with these approvals, as the mailbox holds it.
fn stored(
    account: &MultisigAccount,
    chain: u64,
    tx: &multisig::SafeTx,
    approvals: Vec<Value>,
) -> Value {
    json!({
        "chainId": chain, "safe": account.address,
        "safeTxHash": multisig::safe_tx_hash(chain, &account.address, tx).unwrap(),
        "safeTx": tx, "proposer": approvals.first().map_or(json!(""), |a| a["owner"].clone()),
        "approvals": approvals, "createdMs": 1, "updatedMs": 1,
    })
}

// -- proposing: the review first, then the gate, then one passkey ---------------------

/// §3.6: a transfer from the send flow — its review at its number first, no
/// passkey; "Propose and approve" signs once and the mailbox has it.
#[test]
fn a_transfer_is_reviewed_first_then_proposed_with_one_passkey() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    let draft = sut.view().draft.unwrap();
    assert!(draft.review.is_some(), "the one review is drawn");
    assert_eq!(draft.item.nonce, 0);
    assert_eq!(draft.members_here.len(), 2);
    assert!(
        world.passkeys.is_empty(),
        "no passkey before the review is confirmed"
    );
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let done = sut.view().actions["propose"].clone();
    let hash = hash_of(&done);
    assert_eq!(world.passkeys.len(), 1);
    assert_eq!(
        world.passkeys[0].0,
        fixture(0).address,
        "the first member, nothing remembered"
    );
    assert_eq!(world.mailbox.len(), 1);
    assert_eq!(
        world.submits[0]["genesis"]["name"], "Quiet Harbor",
        "sent named"
    );
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.approved, 1);
    assert_eq!(item.status, ProposalStatus::Pending);
    assert_eq!(item.can_approve, vec![fixture(1).address]);
    assert!(sut.view().draft.is_none());
}

/// r4b-5: withdrawn before it is made — no passkey, nothing proposed.
#[test]
fn a_withdrawn_draft_asks_no_passkey() {
    let (mut world, mut sut, _) = setup(&[0]);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    run(&mut sut, &mut world, Event::Withdraw);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(world.passkeys.is_empty());
    assert!(world.mailbox.is_empty());
}

/// ms-1: a mailbox that cannot be read proposes nothing — no number taken
/// from a missing list.
#[test]
fn an_unreadable_mailbox_proposes_nothing() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.cross_chain = false;
    world.mailbox_down.insert(100);
    let state = propose(&mut sut, &mut world, transfer("1"), None);
    assert_eq!(state, ActionState::MailboxUnreadable);
    assert!(world.passkeys.is_empty());
}

/// The number after every open proposal, once the list is read.
#[test]
fn the_next_number_is_after_every_open_proposal() {
    let (mut world, mut sut, account) = setup(&[0]);
    let open = multisig::cancel_tx(&account.address, "0x0").unwrap();
    world.mailbox.push(stored(&account, 100, &open, vec![]));
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    assert_eq!(sut.view().draft.unwrap().item.tx.nonce, "0x1");
}

/// r5reg-5: the member picked on the review signs, and is offered first next time.
#[test]
fn the_member_picked_is_remembered() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let picked = fixture(1).address;
    hash_of(&propose(
        &mut sut,
        &mut world,
        transfer("1"),
        Some(picked.clone()),
    ));
    assert_eq!(world.passkeys[0].0, picked);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("2"),
        },
    );
    assert_eq!(sut.view().draft.unwrap().default_member, Some(picked));
}

/// A rename the core reads back as a rename; a member change built against
/// the chain's members now; a refusal said, nothing signed.
#[test]
fn a_member_change_the_core_refuses_signs_nothing() {
    let (mut world, mut sut, _) = setup(&[0]);
    let state = propose(
        &mut sut,
        &mut world,
        ProposalKind::MemberChange {
            change: multisig::MemberChange::Add {
                owner: fixture(1).address,
                threshold: None,
            },
        },
        None,
    );
    assert!(
        matches!(state, ActionState::Refused { .. }),
        "already a member: {state:?}"
    );
    assert!(world.passkeys.is_empty());
    let state = propose(
        &mut sut,
        &mut world,
        ProposalKind::Rename {
            name: "Treasury".to_owned(),
        },
        None,
    );
    hash_of(&state);
    assert_eq!(
        world.submits[0]["safeTx"]["to"]
            .as_str()
            .unwrap()
            .to_ascii_lowercase(),
        multisig::GENESIS_REGISTRY.to_ascii_lowercase()
    );
    let item = &sut.view().items[0];
    assert!(item.rename_undeployed, "a rename never deploys it");
    assert!(item.can_execute.is_empty());
}

// -- who counts --------------------------------------------------------------------

/// Only the network's current owners count; any other valid approval is
/// shown, never counted; one that does not verify never counts.
#[test]
fn only_current_members_valid_approvals_count() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    // A member's approval, tampered with: does not verify.
    let mut p = world.mailbox[0].clone();
    let mut bad = p["approvals"][0].clone();
    bad["owner"] = json!(fixture(2).address);
    p["approvals"].as_array_mut().unwrap().push(bad);
    world.mailbox[0] = p;
    // On chain the members are now 0 and 1 only (2 was removed), threshold 2.
    world.deploy(
        100,
        &account.address,
        DeployedSafe::clean(&[&fixture(0).address, &fixture(1).address], 2),
    );
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.approvals.len(), 2);
    assert!(item.approvals[0].counts);
    assert!(
        !item.approvals[1].counts,
        "not verified, and not a member now"
    );
    assert_eq!(item.approved, 1);
}

/// r4C-16: an owner that approved on chain counts — read from the chain.
#[test]
fn an_on_chain_approval_counts() {
    let (mut world, mut sut, account) = setup(&[0]);
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut safe = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    safe.approved.insert((
        fixture(2).address.to_ascii_lowercase(),
        hash.to_ascii_lowercase(),
    ));
    world.deploy(100, &account.address, safe);
    world.mailbox.push(stored(&account, 100, &tx, vec![]));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let item = &sut.view().items[0];
    assert_eq!(item.approved, 1);
    assert_eq!(item.approvals[0].owner, fixture(2).address);
    assert!(item.approvals[0].counts);
    // The executor here is a member: one approval more meets the threshold.
    assert_eq!(item.status, ProposalStatus::Pending);
    assert_eq!(item.can_execute, vec![fixture(0).address]);
}

/// A proposal that does not hash as itself is not shown at all.
#[test]
fn a_proposal_that_does_not_hash_as_itself_is_dropped() {
    let (mut world, mut sut, account) = setup(&[0]);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let mut p = stored(&account, 100, &tx, vec![]);
    p["safeTxHash"] = json!(format!("0x{}", "11".repeat(32)));
    world.mailbox.push(p);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert!(sut.view().items.is_empty());
}

/// r4b-6: proposals at one number each know the others; a cancellation voids them.
#[test]
fn proposals_at_one_number_know_each_other() {
    let (mut world, mut sut, account) = setup(&[0]);
    let a = multisig::transfers_tx(
        &[Transfer {
            token: None,
            to: B.to_owned(),
            amount: "1".to_owned(),
        }],
        "0x0",
    )
    .unwrap();
    let c = multisig::cancel_tx(&account.address, "0x0").unwrap();
    world.mailbox.push(stored(&account, 100, &a, vec![]));
    world.mailbox.push(stored(&account, 100, &c, vec![]));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let items = sut.view().items;
    let cancel_hash = multisig::safe_tx_hash(100, &account.address, &c).unwrap();
    let transfer = items
        .iter()
        .find(|i| i.safe_tx_hash != cancel_hash)
        .unwrap();
    assert_eq!(transfer.cancelled_by.as_deref(), Some(cancel_hash.as_str()));
    assert_eq!(transfer.same_nonce, vec![cancel_hash]);
}

// -- the mailbox, read across chains ----------------------------------------------------

/// One read; a network this device has becomes one of its networks; one it
/// lacks is listed as missing, never read.
#[test]
fn the_cross_chain_list_reaches_networks_and_lists_missing_ones() {
    let (mut world, mut sut, account) = setup(&[0]);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    world.mailbox.push(stored(&account, 8453, &tx, vec![]));
    world.mailbox.push(stored(&account, 10, &tx, vec![]));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert!(world.notices.contains(&Notice::Account {
        address: account.address.clone(),
        update: AccountUpdate::UseChain { chain_id: 8453 },
    }));
    let view = sut.view();
    assert_eq!(view.missing.len(), 1);
    assert_eq!(view.missing[0].chain_id, 10);
    assert_eq!(world.reads_on(10), 0, "never read");
    assert!(view
        .chains
        .iter()
        .any(|c| c.chain_id == 8453 && c.items.len() == 1));
}

/// A service without the list (404): asked network by network.
#[test]
fn without_the_list_each_network_is_asked() {
    let (mut world, mut sut, account) = setup(&[0]);
    world.cross_chain = false;
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    world.mailbox.push(stored(&account, 100, &tx, vec![]));
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    assert_eq!(sut.view().items.len(), 1);
    assert!(world.http_log.iter().any(|l| l.contains("?chainId=100")));
}

/// r4b-3: a network that does not answer is named — never "nothing waits".
#[test]
fn an_unanswering_network_is_named_and_the_list_stale() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.silent.insert(100);
    run(&mut sut, &mut world, Event::Refresh { fresh: true });
    let view = sut.view();
    assert_eq!(view.unreadable, vec![100]);
    assert!(view.stale);
}

/// r4b-11: the list kept across reloads is shown at once, as the last known.
#[test]
fn the_list_kept_is_shown_on_the_next_page() {
    let (mut world, mut sut, account) = setup(&[0]);
    hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    assert!(world.store.contains_key("vela.multisigWaiting"));
    // The next page: the service is away.
    world.service_down = true;
    let mut next = Sut::new();
    let ev = start(&world, vec![100]);
    run(&mut next, &mut world, ev);
    run(
        &mut next,
        &mut world,
        Event::Follow {
            account: Some(account),
        },
    );
    let view = next.view();
    assert_eq!(view.items.len(), 1, "the last list read");
    assert!(view.stale);
    assert!(view.cached_at.is_some());
}

// -- the outbox ----------------------------------------------------------------------

/// A busy mailbox (429): what was signed is kept, sent again after the
/// wait, and never signed twice.
#[test]
fn a_busy_mailbox_keeps_the_approval_and_sends_it_again() {
    let (mut world, mut sut, _) = setup(&[0]);
    world.submit_status = Some((429, json!({"error": "busy"}), Some("3".to_owned())));
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    // The mailbox comes back while the approval waits its 3 s.
    sut.dispatch(Event::ConfirmDraft { as_member: None });
    let mut answered = 0;
    while answered < 20_000 {
        let Some(op) = sut.outstanding().first().cloned() else {
            break;
        };
        if matches!(
            op,
            vela_core::app::multisig_shell::MultisigOperation::Wait { .. }
        ) {
            world.submit_status = None;
        }
        let res = world.answer(&op);
        sut.resolve(res);
        answered += 1;
    }
    assert!(matches!(
        sut.view().actions["propose"],
        ActionState::Done { kept: true, .. }
    ));
    assert_eq!(world.passkeys.len(), 1, "never signed twice");
    assert_eq!(world.mailbox.len(), 1, "sent again after the wait");
    assert!(sut.view().outbox.is_empty());
    assert!(
        !world.store.contains_key("vela.multisigOutbox"),
        "nothing left behind"
    );
}

/// ms-19: kept, and sent again when a page comes up.
#[test]
fn a_kept_approval_is_sent_when_a_page_comes_up() {
    let (mut world, mut sut, account) = setup(&[0]);
    world.service_down = true;
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    assert_eq!(
        sut.view().actions["propose"],
        ActionState::MailboxUnreadable
    );
    // The list was read; then the service went away before the passkey.
    world.service_down = false;
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    world.service_down = true;
    sut.dispatch(Event::ConfirmDraft { as_member: None });
    settle(&mut sut, &mut world, 400);
    assert!(world.store.contains_key("vela.multisigOutbox"));
    let item = sut.view().items.into_iter().next().unwrap();
    assert!(item.unsent, "shown from what was signed");
    // A new page, the service back.
    world.service_down = false;
    let mut next = Sut::new();
    let ev = start(&world, vec![100]);
    run(&mut next, &mut world, ev);
    let _ = account;
    assert_eq!(world.mailbox.len(), 1);
    assert!(!world.store.contains_key("vela.multisigOutbox"));
    assert_eq!(world.passkeys.len(), 1);
}

// -- the gate before every signature ---------------------------------------------------

/// Its genesis refused there: never signs, and asks nothing.
#[test]
fn a_refused_genesis_signs_nothing() {
    let (mut world, mut sut, _) = setup(&[0]);
    world
        .canonical_missing
        .insert((100, HANDLER_141.to_ascii_lowercase()));
    let state = propose(&mut sut, &mut world, transfer("1"), None);
    assert!(
        matches!(state, ActionState::SigningRefused { .. }),
        "{state:?}"
    );
    assert!(world.passkeys.is_empty());
    assert!(world.risks_asked.is_empty());
}

/// A risk there: asks, naming what was found — declined, nothing signed;
/// and asked again for the next signature.
#[test]
fn a_risk_is_asked_every_signature() {
    let (mut world, mut sut, account) = setup(&[0]);
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut safe = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    safe.modules = vec!["0x00000000000000000000000000000000000000ee".to_owned()];
    world.deploy(100, &account.address, safe);
    world.risk_signs = false;
    assert_eq!(
        propose(&mut sut, &mut world, transfer("1"), None),
        ActionState::RiskDeclined
    );
    assert!(world.passkeys.is_empty());
    assert_eq!(world.risks_asked.len(), 1);
    assert_eq!(world.risks_asked[0][0]["risk"]["kind"], "module");
    world.risk_signs = true;
    hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    assert_eq!(world.risks_asked.len(), 2, "never remembered");
    assert_eq!(world.passkeys.len(), 1);
}

/// Who can act for it could not be read: nothing signed yet.
#[test]
fn an_unchecked_network_signs_nothing_yet() {
    let (mut world, mut sut, _) = setup(&[0]);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: transfer("1"),
        },
    );
    world.silent.insert(100);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(sut.view().actions["propose"], ActionState::SigningUnchecked);
    assert!(world.passkeys.is_empty());
}

// -- approving and executing ------------------------------------------------------------

/// Two members approve; the second's approval meets the threshold.
#[test]
fn a_second_approval_makes_it_ready() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    assert!(matches!(
        sut.view().actions[&format!("approve:{hash}")],
        ActionState::Done { .. }
    ));
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.approved, 2);
    assert_eq!(item.status, ProposalStatus::Ready);
    assert!(item.can_approve.is_empty());
    assert_eq!(item.can_execute.len(), 2, "either member here");
}

/// Executing an undeployed multisig: priced, the deploy gate read again,
/// sent through the executor's own pipeline, followed until it executed.
#[test]
fn executed_and_followed_until_it_landed() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    let preview = sut.view().previews[&format!("100:{hash}")].clone();
    assert_eq!(preview["kind"], "priced", "{preview}");
    assert_eq!(
        preview["setup"]["multisig"], true,
        "deployed by its first execution"
    );
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.clone(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    let gates = world.rpc_log.len();
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    assert_eq!(world.sent.len(), 1, "one user operation");
    assert!(
        world.sent[0].2.len() >= 2,
        "the deployment, then execTransaction"
    );
    assert!(
        world.rpc_log[gates..]
            .iter()
            .any(|(_, m, _)| m == "eth_getCode"),
        "the code read again before the deployment"
    );
    let verdict = sut.view().executions[&format!("100:{hash}")].clone();
    assert_eq!(verdict["kind"], "executed", "{verdict}");
    assert!(world
        .notices
        .iter()
        .any(|n| matches!(n, Notice::ActivityRows { .. })));
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(item.status, ProposalStatus::Done);
}

/// r7-3: an executor that signs on its trusted page signs ONE operation —
/// no approval of its own is asked first — and the shell is told what runs
/// (`SendPurpose::Execute`: the transaction, the multisig, its name, its
/// rule), so the page draws it. It approved by sending it, and that is what
/// this device remembers.
#[test]
fn a_page_executor_signs_one_operation_and_its_page_is_told_what_runs() {
    use vela_core::app::multisig_shell::{ApprovalRoute, SendPurpose};
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let state = propose(
        &mut sut,
        &mut world,
        transfer("1"),
        Some(fixture(1).address),
    );
    let hash = hash_of(&state);
    let mut locals = world.locals.clone();
    locals[0].route = ApprovalRoute::Page;
    world.locals = locals.clone();
    run(
        &mut sut,
        &mut world,
        Event::LocalAccounts { accounts: locals },
    );
    let owners = [fixture(0).address, fixture(1).address, fixture(2).address];
    let mut after = DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.on_send = Some(Landing {
        chain: 100,
        safe: account.address.clone(),
        after,
        safe_tx_hash: hash.clone(),
        tx_hash: format!("0x{}", "cd".repeat(32)),
        user_op_hash: format!("0x{}", "ab".repeat(32)),
    });
    let passkeys = world.passkeys.len();
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    assert_eq!(
        world.passkeys.len(),
        passkeys,
        "no approval of its own before its operation"
    );
    assert_eq!(world.sent.len(), 1, "one user operation");
    let Some(Some(SendPurpose::Execute {
        multisig,
        name,
        threshold,
        tx,
    })) = world.purposes.first().cloned()
    else {
        panic!("{:?}", world.purposes);
    };
    assert_eq!(multisig, account.address);
    assert_eq!(name.as_deref(), Some("Quiet Harbor"));
    assert_eq!(threshold, 2);
    let item = sut
        .view()
        .items
        .into_iter()
        .find(|i| i.safe_tx_hash == hash)
        .unwrap();
    assert_eq!(*tx, item.tx);
    let verdict = sut.view().executions[&format!("100:{hash}")].clone();
    assert_eq!(verdict["kind"], "executed", "{verdict}");
    let kept: Value =
        serde_json::from_str(&world.store[vela_core::app::multisig_presence::EXECUTED_KEY])
            .unwrap();
    let facts = kept.as_object().unwrap().values().next().unwrap().clone();
    assert_eq!(facts["executorApproved"], true, "{facts}");
    assert_eq!(
        facts["approvers"].as_array().unwrap().len(),
        2,
        "the proposer's and the executor's: {facts}"
    );
}

/// r4-5: spending more than the multisig holds fails in its own words,
/// before any fee or passkey.
#[test]
fn an_execution_that_would_fail_stops_before_the_passkey() {
    let (mut world, mut sut, account) = setup(&[0, 1]);
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1000"), None));
    run(
        &mut sut,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    world
        .balances
        .insert((100, account.address.to_ascii_lowercase()), 5);
    let passkeys = world.passkeys.len();
    run(
        &mut sut,
        &mut world,
        Event::Execute {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            executor: fixture(0).address,
        },
    );
    let ActionState::Stopped { preview } = sut.view().actions[&format!("execute:{hash}")].clone()
    else {
        panic!("{:?}", sut.view().actions);
    };
    assert_eq!(preview["kind"], "fails");
    assert_eq!(preview["why"]["kind"], "short");
    assert!(world.sent.is_empty());
    assert_eq!(world.passkeys.len(), passkeys);
}

// -- the execution preview's price: carries, executorApproves, the setup share ----------

/// 0.05 of the chain's coin.
const FEE: u128 = 50_000_000_000_000_000;

fn quoted(executor_deployed: bool) -> vela_core::app::multisig_shell::MultisigShellResult {
    vela_core::app::multisig_shell::MultisigShellResult::Quoted {
        quote: json!({
            "kind": "quoted",
            "fee": {"token": null, "amount": format!("0x{FEE:x}")},
            "totalGas": "0x7a120",
            "executorDeployed": executor_deployed,
        }),
    }
}

/// Proposed by fixture 0, approved by fixture 1: the preview fixture 0 executes from.
fn approved_twice(world: &mut World, sut: &mut Sut) -> String {
    let hash = hash_of(&propose(sut, world, transfer("1"), None));
    run(
        sut,
        world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    hash
}

fn preview_of(sut: &mut Sut, world: &mut World, hash: &str, executor: &str) -> Value {
    run(
        sut,
        world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: hash.to_owned(),
            executor: executor.to_owned(),
        },
    );
    sut.view().previews[&format!("100:{hash}")].clone()
}

/// execute.test.ts "is priced at the relay's quote, with the one-time
/// setup's share measured": the multisig and Bob's account deployed by it,
/// two deployments of 221k each less the 21k each carries alone — 400k of
/// the quote's 500k.
#[test]
fn the_preview_measures_the_one_time_setups_share_of_the_fee() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    world.quote_answer = Some(quoted(true));
    world.estimate_gas = Some(221_000);
    let executor = fixture(0).address;
    world
        .balances
        .insert((100, executor.to_ascii_lowercase()), FEE / 5);
    let preview = preview_of(&mut sut, &mut world, &hash, &executor);
    assert_eq!(preview["kind"], "priced", "{preview}");
    assert_eq!(
        preview["setup"],
        json!({
            "multisig": true,
            "members": [fixture(1).address],
            "executor": false,
            "amount": (FEE * 4 / 5).to_string(),
        })
    );
    // r7-3: one short of the threshold with nothing deployed, the executor
    // approves by sending it; the approval given is carried.
    assert_eq!(preview["carries"], json!([fixture(1).address]));
    assert_eq!(preview["executorApproves"], true);
    assert_eq!(preview["balance"], (FEE / 5).to_string());
    assert_eq!(preview["canPay"], false, "holds 0.01 for a 0.05 fee");
    let gas_reads = world
        .rpc_log
        .iter()
        .filter(|(_, m, _)| m == "eth_estimateGas")
        .count();
    assert_eq!(gas_reads, 2, "each deployment measured on its own");
}

/// "says no setup share when the executor's own account is deployed by it too".
#[test]
fn no_setup_share_when_the_executors_own_account_is_deployed_by_it_too() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    world.quote_answer = Some(quoted(false));
    world.estimate_gas = Some(221_000);
    let preview = preview_of(&mut sut, &mut world, &hash, &fixture(0).address);
    assert_eq!(preview["setup"]["executor"], true);
    assert_eq!(preview["setup"]["amount"], Value::Null);
    assert!(
        !world.rpc_log.iter().any(|(_, m, _)| m == "eth_estimateGas"),
        "nothing measured when no share is said"
    );
}

/// A deployment whose gas could not be measured: no share, never a part.
#[test]
fn an_unmeasured_deployment_says_no_setup_share() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    let hash = approved_twice(&mut world, &mut sut);
    world.quote_answer = Some(quoted(true));
    world.estimate_gas = None;
    let preview = preview_of(&mut sut, &mut world, &hash, &fixture(0).address);
    assert_eq!(preview["kind"], "priced", "{preview}");
    assert_eq!(preview["setup"]["amount"], Value::Null);
}

/// "the look says what it carries" (r7-3): approvals that meet the
/// threshold, from accounts already there, are all carried; the executor
/// only sends it — and its execTransaction carries no `msg.sender` approval.
#[test]
fn the_look_says_what_it_carries_and_the_executor_only_sends_it() {
    let (mut world, mut sut, _) = setup(&[0, 1, 2]);
    let hash = approved_twice(&mut world, &mut sut);
    for i in [0, 1] {
        world.codes.insert(
            (100, fixture(i).address.to_ascii_lowercase()),
            "0x6080".to_owned(),
        );
    }
    world.quote_answer = Some(quoted(true));
    world.estimate_gas = Some(300_000);
    let preview = preview_of(&mut sut, &mut world, &hash, &fixture(2).address);
    assert_eq!(preview["kind"], "priced", "{preview}");
    assert_eq!(
        preview["carries"],
        json!([fixture(0).address, fixture(1).address])
    );
    assert_eq!(preview["executorApproves"], false);
    assert_eq!(preview["setup"]["members"], json!([]));
    // One deployment (the multisig) of 300k: 279k of 500k.
    assert_eq!(
        preview["setup"]["amount"],
        (FEE * 279_000 / 500_000).to_string()
    );
}

/// The pure rules behind it, replayed from `tests/vectors/multisig/execution.json`.
#[test]
fn the_execution_vectors_hold() {
    use vela_core::app::multisig_proposals::{carried_approvals, setup_share};
    let path = format!(
        "{}/tests/vectors/multisig/execution.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let carried = v["carried"].as_array().unwrap();
    assert!(carried.len() >= 6);
    for case in carried {
        let name = case["name"].as_str().unwrap();
        let others: Vec<(String, bool)> = case["others"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| {
                (
                    o["owner"].as_str().unwrap().to_owned(),
                    o["ready"].as_bool().unwrap(),
                )
            })
            .collect();
        let (chosen, approves) = carried_approvals(
            case["threshold"].as_u64().unwrap(),
            case["executorIsOwner"].as_bool().unwrap(),
            others,
        );
        let carries: Vec<String> = chosen.into_iter().map(|(o, _)| o).collect();
        assert_eq!(json!(carries), case["expect"]["carries"], "{name}");
        assert_eq!(
            json!(approves),
            case["expect"]["executorApproves"],
            "{name}"
        );
    }
    let shares = v["setupShare"].as_array().unwrap();
    assert!(shares.len() >= 6);
    let n = |v: &Value| v.as_str().unwrap().parse::<u128>().unwrap();
    for case in shares {
        let name = case["name"].as_str().unwrap();
        let gas: Vec<Option<u128>> = case["gas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| g.as_str().map(|s| s.parse().unwrap()))
            .collect();
        let got = setup_share(
            n(&case["fee"]),
            n(&case["totalGas"]),
            case["executorDeployed"].as_bool().unwrap(),
            &gas,
        );
        assert_eq!(
            got.map(|a| a.to_string()).map_or(Value::Null, Value::from),
            case["expect"],
            "{name}"
        );
    }
}

// -- r7: how each member still to approve signs (signers.kindOf, plainAccounts) ---------

/// A 2-of-4 of a Vela member here, a plain address, another Safe (code, Vela's
/// passkey signer not configured for it) and a Vela account set up on Base
/// (its signer configured there, no index record needed); the sheet asks
/// how each remaining member signs.
#[test]
fn each_remaining_member_signs_as_the_one_rule_says() {
    const PLAIN: &str = "0x00000000000000000000000000000000000000a7";
    const OTHER_SAFE: &str = "0x00000000000000000000000000000000000000c5";
    const VELA_ON_BASE: &str = "0x00000000000000000000000000000000000000d3";
    let me = fixture(0).address;
    let g = genesis(&[&me, PLAIN, OTHER_SAFE, VELA_ON_BASE], 2, "0x41");
    let account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.deploy(
        100,
        &g.address,
        DeployedSafe::clean(&[&me, PLAIN, OTHER_SAFE, VELA_ON_BASE], 2),
    );
    world
        .codes
        .insert((100, OTHER_SAFE.to_owned()), "0x6080".to_owned());
    world.configured.insert((8453, VELA_ON_BASE.to_owned()));
    let mut sut = Sut::new();
    let ev = start(&world, vec![100, 8453]);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    let hash = hash_of(&propose(&mut sut, &mut world, transfer("1"), None));
    run(
        &mut sut,
        &mut world,
        Event::ReadSigners {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
        },
    );
    let signers = sut.view().signers[&format!("100:{hash}")].clone();
    let kind = |who: &str| {
        signers
            .rows
            .iter()
            .find(|r| r.owner.eq_ignore_ascii_case(who))
            .unwrap_or_else(|| panic!("{who}: {signers:?}"))
            .kind
    };
    assert!(
        !signers
            .rows
            .iter()
            .any(|r| r.owner.eq_ignore_ascii_case(&me)),
        "the proposer approved"
    );
    assert_eq!(kind(PLAIN), Some(SignerKind::Unsure), "the Vela way first");
    assert_eq!(kind(OTHER_SAFE), Some(SignerKind::Contract));
    assert_eq!(kind(VELA_ON_BASE), Some(SignerKind::Vela));
    // Only a plain account is ever offered to an outside wallet.
    assert_eq!(signers.outside.len(), 1);
    assert!(signers.outside[0].eq_ignore_ascii_case(PLAIN));
}
